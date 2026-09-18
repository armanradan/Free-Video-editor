#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod browser {
    use js_sys::{Function, Promise, Reflect};
    use media_core::{
        INPUT_SIZE, MediaError, OUTPUT_SIZE, OutputProfileId, ResizePreset, Size,
        validate_browser_input_size,
    };
    use media_gpu::ResizePipeline;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    use wasm_bindgen::{JsCast, prelude::*};
    use wasm_bindgen_futures::{JsFuture, future_to_promise};
    use web_sys::{
        File, HtmlCanvasElement, HtmlInputElement, OffscreenCanvas, VideoFrame, VideoFrameInit,
    };

    thread_local! {
        static GENERATION: Cell<u32> = const { Cell::new(0) };
        static GPU_SESSION: RefCell<Option<Rc<GpuSession>>> = const { RefCell::new(None) };
        static BITMAP_COPIES: Cell<u32> = const { Cell::new(0) };
        static REMOTE_GPU: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    #[derive(Clone, Debug)]
    pub struct ConversionResult {
        pub summary: String,
        pub download_url: String,
        pub file_name: String,
        pub frame_count: u32,
        pub duration_seconds: f64,
        pub output_bytes: u64,
    }

    #[derive(Clone, Debug)]
    pub struct OutputProfileCapabilities {
        pub mp4_supported: bool,
        pub mp4_reason: String,
    }

    #[wasm_bindgen(inline_js = r#"
        export function runtimeModuleUrl() { return new URL('../../converter-web.js', import.meta.url).href; }
        export function invokeM1(fixture, manifest, processFrame, status, cancelled) {
            return globalThis.__DIAXUS_M1__.run(fixture, manifest, processFrame, status, cancelled);
        }
        export function inspectBrowserInput(file) { return globalThis.__DIAXUS_MEDIA_WEB__.inspect(file); }
        export async function copyDecodedFrame(queue, texture, frame, width, height) {
            const destination = { texture, colorSpace: 'srgb', premultipliedAlpha: false };
            try {
                queue.copyExternalImageToTexture({ source: frame }, destination, [width, height]);
                return null;
            } catch (directError) {
                if (!(directError instanceof TypeError)) throw directError;
                const bitmap = await createImageBitmap(frame);
                try {
                    queue.copyExternalImageToTexture({ source: bitmap }, destination, [width, height]);
                    return bitmap;
                } catch (error) {
                    bitmap.close();
                    throw new Error(`VideoFrame and ImageBitmap GPU ingress failed: ${error.message}`);
                }
            }
        }
        export function probeBrowserProfiles(file, width, height) {
            return globalThis.__DIAXUS_MEDIA_WEB__.probeProfiles(file, width, height);
        }
        export function invokeBrowserJob(file, width, height, profile, processFrame, status, cancelled) {
            return globalThis.__DIAXUS_MEDIA_WEB__.run(file, width, height, profile, processFrame, status, cancelled);
        }
        export async function describeSelectedAdapter() {
            const adapter = await navigator.gpu?.requestAdapter({ powerPreference: "high-performance" });
            if (!adapter) return "Browser did not return a WebGPU adapter";
            const info = adapter.info ?? await adapter.requestAdapterInfo?.() ?? {};
            const values = [info.description, info.vendor, info.architecture, info.device]
                .filter(value => typeof value === "string" && value.trim().length > 0);
            const unique = [...new Set(values)];
            return `${unique.length ? unique.join(" / ") : "identity redacted by browser"}${info.isFallbackAdapter ? " (fallback adapter)" : ""}`;
        }
    "#)]
    extern "C" {
        #[wasm_bindgen(js_name = runtimeModuleUrl)]
        fn runtime_module_url() -> String;
        #[wasm_bindgen(js_name = copyDecodedFrame, catch)]
        fn copy_decoded_frame(
            queue: &JsValue,
            texture: &JsValue,
            frame: &VideoFrame,
            width: u32,
            height: u32,
        ) -> Result<Promise, JsValue>;
        #[wasm_bindgen(js_name = invokeM1, catch)]
        fn invoke_m1(
            fixture: js_sys::Uint8Array,
            manifest: &str,
            process_frame: &Function,
            status: &Function,
            cancelled: &Function,
        ) -> Result<Promise, JsValue>;
        #[wasm_bindgen(js_name = inspectBrowserInput, catch)]
        fn inspect_browser_input(file: &File) -> Result<Promise, JsValue>;
        #[wasm_bindgen(js_name = probeBrowserProfiles, catch)]
        fn probe_browser_profiles(file: &File, width: u32, height: u32)
        -> Result<Promise, JsValue>;
        #[wasm_bindgen(js_name = invokeBrowserJob, catch)]
        fn invoke_browser_job(
            file: &File,
            width: u32,
            height: u32,
            profile: &str,
            process_frame: &Function,
            status: &Function,
            cancelled: &Function,
        ) -> Result<Promise, JsValue>;
        #[wasm_bindgen(js_name = describeSelectedAdapter, catch)]
        fn describe_selected_adapter() -> Result<Promise, JsValue>;
    }

    #[wasm_bindgen(module = "/src/worker-host.js")]
    extern "C" {
        #[wasm_bindgen(js_name = setupRuntime)]
        fn setup_runtime_js(m1: &str, pipeline: &str, wasm: &str);
        #[wasm_bindgen(js_name = dispatchJob, catch)]
        fn dispatch_job(
            file: Option<File>,
            operation: &str,
            profile: &str,
            status: &Function,
            local: &Function,
        ) -> Result<Promise, JsValue>;
        #[wasm_bindgen(js_name = cancelRemote)]
        fn cancel_remote();
    }

    pub fn setup_runtime(m1: &str, pipeline: &str) {
        setup_runtime_js(m1, pipeline, &runtime_module_url());
    }

    async fn dispatch(
        file: Option<File>,
        operation: &str,
        profile: &str,
        status: Function,
    ) -> Result<JsValue, MediaError> {
        let local = Closure::<dyn FnMut(JsValue, String, String, Function) -> Promise>::new(
            |file: JsValue, operation: String, profile: String, status: Function| {
                future_to_promise(execute_job(
                    file.dyn_into::<File>().ok(),
                    operation,
                    profile,
                    status,
                ))
            },
        );
        let result = JsFuture::from(
            dispatch_job(
                file,
                operation,
                profile,
                &status,
                local.as_ref().unchecked_ref(),
            )
            .map_err(js_error)?,
        )
        .await
        .map_err(js_error)?;
        if let Ok(label) = string_property(&result, "gpu", "") {
            REMOTE_GPU.with(|slot| *slot.borrow_mut() = Some(label));
        }
        Ok(result)
    }

    // Worker exports use the same concrete implementation as the compatibility path.
    #[wasm_bindgen]
    pub async fn initialize_worker(canvas: OffscreenCanvas) -> Result<(), JsValue> {
        let gpu = Rc::new(
            GpuSession::new(ExportCanvas::Offscreen(canvas))
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))?,
        );
        gpu.configure(INPUT_SIZE, OUTPUT_SIZE);
        let init = VideoFrameInit::new();
        init.set_timestamp_f64(0.0);
        gpu.canvas.capture(&init)?.close();
        GPU_SESSION.with(|slot| *slot.borrow_mut() = Some(gpu));
        Ok(())
    }

    #[wasm_bindgen]
    pub async fn execute_job(
        file: Option<File>,
        operation: String,
        profile: String,
        status: Function,
    ) -> Result<JsValue, JsValue> {
        let result = match operation.as_str() {
            "m1" => run_m1_local(status).await,
            "probe" => match file {
                Some(file) => probe_file(file).await,
                None => Err(platform("no source file")),
            },
            "convert" => {
                let profile = match profile.as_str() {
                    "webm-vp8-opus" => OutputProfileId::WebmVp8Opus,
                    "webm-vp8-video-only" => OutputProfileId::WebmVp8VideoOnly,
                    "mp4-h264-aac" => OutputProfileId::Mp4H264Aac,
                    _ => return Err(JsValue::from_str("unknown output profile")),
                };
                match file {
                    Some(file) => convert_file(file, profile, status).await,
                    None => Err(platform("no source file")),
                }
            }
            _ => Err(platform("unknown media command")),
        }
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
        if let Some(gpu) =
            GPU_SESSION.with(|slot| slot.borrow().as_ref().map(|gpu| gpu.adapter_label.clone()))
        {
            Reflect::set(&result, &"gpu".into(), &gpu.into())?;
        }
        Ok(result)
    }

    trait BrowserConversionBackend {
        fn inspect(&self, file: &File) -> Result<Promise, JsValue>;
        fn run(
            &self,
            file: &File,
            output: Size,
            profile: OutputProfileId,
            process_frame: &Function,
            status: &Function,
            cancelled: &Function,
        ) -> Result<Promise, JsValue>;
        fn probe_profiles(&self, file: &File, output: Size) -> Result<Promise, JsValue>;
    }

    struct WebCodecsMediabunnyBackend;

    impl BrowserConversionBackend for WebCodecsMediabunnyBackend {
        fn inspect(&self, file: &File) -> Result<Promise, JsValue> {
            inspect_browser_input(file)
        }

        fn run(
            &self,
            file: &File,
            output: Size,
            profile: OutputProfileId,
            process_frame: &Function,
            status: &Function,
            cancelled: &Function,
        ) -> Result<Promise, JsValue> {
            invoke_browser_job(
                file,
                output.width,
                output.height,
                profile.as_str(),
                process_frame,
                status,
                cancelled,
            )
        }

        fn probe_profiles(&self, file: &File, output: Size) -> Result<Promise, JsValue> {
            probe_browser_profiles(file, output.width, output.height)
        }
    }

    pub fn cancel() {
        cancel_remote();
        cancel_local();
    }

    #[wasm_bindgen]
    pub fn cancel_local() {
        GENERATION.with(|value| value.set(value.get().wrapping_add(1)));
    }

    pub fn selected_gpu() -> Option<String> {
        if let Some(label) = REMOTE_GPU.with(|slot| slot.borrow().clone()) {
            return Some(label);
        }
        GPU_SESSION.with(|slot| {
            slot.borrow()
                .as_ref()
                .map(|session| session.adapter_label.clone())
        })
    }

    pub async fn run_m1(_canvas_id: &str, status: Function) -> Result<String, MediaError> {
        let result = dispatch(None, "m1", "", status).await?;
        string_property(&result, "summary", "probe returned no summary")
    }

    async fn run_m1_local(status: Function) -> Result<JsValue, MediaError> {
        let generation = begin_generation();
        let gpu = configured_gpu(INPUT_SIZE, OUTPUT_SIZE).await?;
        let process = process_callback(gpu);
        let cancelled = cancellation_callback(generation);
        let fixture =
            js_sys::Uint8Array::from(include_bytes!("../../../fixtures/m1-vp8.ivf").as_slice());
        let result = JsFuture::from(
            invoke_m1(
                fixture,
                include_str!("../../../fixtures/m1-vp8.json"),
                process.as_ref().unchecked_ref(),
                &status,
                cancelled.as_ref().unchecked_ref(),
            )
            .map_err(js_error)?,
        )
        .await
        .map_err(js_error)?;
        let summary = format!(
            "{}\nAdditional VideoFrame→ImageBitmap compatibility conversions: {}.",
            string_property(&result, "summary", "probe returned no summary")?,
            BITMAP_COPIES.with(Cell::get)
        );
        Reflect::set(&result, &"summary".into(), &summary.into()).map_err(js_error)?;
        Ok(result)
    }

    pub async fn convert_m3(
        file_input_id: &str,
        _canvas_id: &str,
        profile: OutputProfileId,
        status: Function,
    ) -> Result<ConversionResult, MediaError> {
        let file = selected_file(file_input_id)?;
        let result = dispatch(Some(file), "convert", profile.as_str(), status).await?;
        Ok(ConversionResult {
            summary: string_property(&result, "summary", "conversion returned no summary")?,
            download_url: string_property(
                &result,
                "downloadUrl",
                "conversion returned no download URL",
            )?,
            file_name: string_property(&result, "fileName", "conversion returned no filename")?,
            frame_count: u32_property(&result, "frameCount")?,
            duration_seconds: number_property(&result, "duration")?,
            output_bytes: number_property(&result, "outputBytes")? as u64,
        })
    }

    async fn convert_file(
        file: File,
        profile: OutputProfileId,
        status: Function,
    ) -> Result<JsValue, MediaError> {
        let generation = begin_generation();
        validate_browser_input_size(file.size() as u64)?;
        let backend = WebCodecsMediabunnyBackend;
        let inspection = JsFuture::from(backend.inspect(&file).map_err(js_error)?)
            .await
            .map_err(js_error)?;
        let input = Size::new(
            u32_property(&inspection, "width")?,
            u32_property(&inspection, "height")?,
        )?;
        let output = ResizePreset::Half.output_size(input)?;
        let gpu = configured_gpu(input, output).await?;
        let process = process_callback(gpu);
        let cancelled = cancellation_callback(generation);
        let result = JsFuture::from(
            backend
                .run(
                    &file,
                    output,
                    profile,
                    process.as_ref().unchecked_ref(),
                    &status,
                    cancelled.as_ref().unchecked_ref(),
                )
                .map_err(js_error)?,
        )
        .await
        .map_err(js_error)?;
        let bitmap_copies = BITMAP_COPIES.with(Cell::get);
        let summary = format!(
            "{}\nIngress: {bitmap_copies} additional VideoFrame→ImageBitmap compatibility conversions; browser-internal copies unknown.",
            string_property(&result, "summary", "conversion returned no summary")?
        );
        Reflect::set(&result, &"summary".into(), &summary.into()).map_err(js_error)?;
        Ok(result)
    }

    pub async fn probe_output_profiles(
        file_input_id: &str,
    ) -> Result<OutputProfileCapabilities, MediaError> {
        let file = selected_file(file_input_id)?;
        let capabilities = dispatch(Some(file), "probe", "", Function::new_no_args("")).await?;
        Ok(OutputProfileCapabilities {
            mp4_supported: bool_property(&capabilities, "mp4Supported")?,
            mp4_reason: string_property(
                &capabilities,
                "mp4Reason",
                "MP4 capability probe returned no reason",
            )?,
        })
    }

    async fn probe_file(file: File) -> Result<JsValue, MediaError> {
        validate_browser_input_size(file.size() as u64)?;
        let backend = WebCodecsMediabunnyBackend;
        let inspection = JsFuture::from(backend.inspect(&file).map_err(js_error)?)
            .await
            .map_err(js_error)?;
        let input = Size::new(
            u32_property(&inspection, "width")?,
            u32_property(&inspection, "height")?,
        )?;
        let output = ResizePreset::Half.output_size(input)?;
        let capabilities = JsFuture::from(backend.probe_profiles(&file, output).map_err(js_error)?)
            .await
            .map_err(js_error)?;
        Ok(capabilities)
    }

    fn begin_generation() -> u32 {
        BITMAP_COPIES.with(|value| value.set(0));
        GENERATION.with(|value| {
            let next = value.get().wrapping_add(1);
            value.set(next);
            next
        })
    }

    fn cancellation_callback(generation: u32) -> Closure<dyn FnMut() -> bool> {
        Closure::new(move || GENERATION.with(|value| value.get() != generation))
    }

    fn process_callback(gpu: Rc<GpuSession>) -> Closure<dyn FnMut(JsValue, f64, f64) -> Promise> {
        Closure::new(move |value: JsValue, timestamp: f64, duration: f64| {
            let gpu = Rc::clone(&gpu);
            future_to_promise(async move {
                const MAX_SAFE: f64 = 9_007_199_254_740_991.0;
                let valid = |value: f64| {
                    value.is_finite() && value.fract() == 0.0 && value.abs() <= MAX_SAFE
                };
                if !valid(timestamp) || !valid(duration) {
                    return Err(JsValue::from_str(
                        "timestamp/duration was not a safe integer",
                    ));
                }
                let frame = value
                    .dyn_into::<VideoFrame>()
                    .map_err(|_| JsValue::from_str("decoder output was not a VideoFrame"))?;
                gpu.process(frame, timestamp as i64, duration as i64).await
            })
        })
    }

    async fn configured_gpu(input: Size, output: Size) -> Result<Rc<GpuSession>, MediaError> {
        let existing = GPU_SESSION.with(|slot| slot.borrow().clone());
        let gpu = if let Some(existing) = existing {
            existing
        } else {
            let document = web_sys::window()
                .and_then(|w| w.document())
                .ok_or_else(|| platform("document is unavailable"))?;
            let canvas = document
                .get_element_by_id("export-canvas")
                .ok_or_else(|| platform("export canvas was not mounted"))?
                .dyn_into::<HtmlCanvasElement>()
                .map_err(|_| platform("export element is not a canvas"))?;
            let created = Rc::new(GpuSession::new(ExportCanvas::Html(canvas)).await?);
            GPU_SESSION.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&created)));
            created
        };
        gpu.configure(input, output);
        Ok(gpu)
    }

    fn selected_file(input_id: &str) -> Result<File, MediaError> {
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| platform("document is unavailable"))?;
        let input = document
            .get_element_by_id(input_id)
            .ok_or_else(|| platform("file input was not mounted"))?
            .dyn_into::<HtmlInputElement>()
            .map_err(|_| platform("file selector is not an input element"))?;
        input
            .files()
            .and_then(|files| files.item(0))
            .ok_or_else(|| platform("select an MP4 file first"))
    }

    struct ConfiguredResources {
        input: wgpu::Texture,
        input_size: Size,
        output_size: Size,
    }
    struct GpuSession {
        adapter_label: String,
        canvas: ExportCanvas,
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        pipeline: ResizePipeline,
        configured: RefCell<Option<ConfiguredResources>>,
    }
    struct OwnedVideoFrame(VideoFrame);
    enum ExportCanvas {
        Html(HtmlCanvasElement),
        Offscreen(OffscreenCanvas),
    }
    impl ExportCanvas {
        fn surface_target(&self) -> wgpu::SurfaceTarget<'static> {
            match self {
                Self::Html(canvas) => wgpu::SurfaceTarget::Canvas(canvas.clone()),
                Self::Offscreen(canvas) => wgpu::SurfaceTarget::OffscreenCanvas(canvas.clone()),
            }
        }
        fn resize(&self, size: Size) {
            match self {
                Self::Html(canvas) => {
                    canvas.set_width(size.width);
                    canvas.set_height(size.height);
                }
                Self::Offscreen(canvas) => {
                    canvas.set_width(size.width);
                    canvas.set_height(size.height);
                }
            }
        }
        fn capture(&self, init: &VideoFrameInit) -> Result<VideoFrame, JsValue> {
            match self {
                Self::Html(canvas) => {
                    VideoFrame::new_with_html_canvas_element_and_video_frame_init(canvas, init)
                }
                Self::Offscreen(canvas) => {
                    VideoFrame::new_with_offscreen_canvas_and_video_frame_init(canvas, init)
                }
            }
        }
    }
    struct OwnedBitmap(JsValue);
    impl Drop for OwnedBitmap {
        fn drop(&mut self) {
            if let Ok(close) = Reflect::get(&self.0, &JsValue::from_str("close"))
                && let Some(close) = close.dyn_ref::<Function>()
            {
                let _ = close.call0(&self.0);
            }
        }
    }
    impl Drop for OwnedVideoFrame {
        fn drop(&mut self) {
            self.0.close();
        }
    }

    impl GpuSession {
        async fn new(canvas: ExportCanvas) -> Result<Self, MediaError> {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            descriptor.backends = wgpu::Backends::BROWSER_WEBGPU;
            let instance = wgpu::Instance::new(descriptor);
            let surface = instance
                .create_surface(canvas.surface_target())
                .map_err(|error| platform(format!("WebGPU canvas surface failed: {error}")))?;
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                    apply_limit_buckets: false,
                })
                .await
                .map_err(|error| {
                    platform(format!("no WebGPU adapter supports the canvas: {error}"))
                })?;
            let fallback_label = describe_adapter(&adapter.get_info());
            let adapter_label = match describe_selected_adapter() {
                Ok(promise) => JsFuture::from(promise)
                    .await
                    .ok()
                    .and_then(|value| value.as_string())
                    .filter(|value| !value.trim().is_empty())
                    .map(|value| format!("{value} ({:?})", adapter.get_info().backend))
                    .unwrap_or(fallback_label),
                Err(_) => fallback_label,
            };
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("browser video processing device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                    memory_hints: Default::default(),
                    trace: wgpu::Trace::Off,
                })
                .await
                .map_err(|error| platform(format!("WebGPU device request failed: {error}")))?;
            let capabilities = surface.get_capabilities(&adapter);
            let surface_format = capabilities
                .formats
                .iter()
                .copied()
                .find(|format| format.is_srgb())
                .or_else(|| capabilities.formats.first().copied())
                .ok_or_else(|| platform("canvas reported no texture formats"))?;
            let pipeline = ResizePipeline::new(&device, surface_format);
            Ok(Self {
                adapter_label,
                canvas,
                surface,
                device,
                queue,
                surface_format,
                pipeline,
                configured: RefCell::new(None),
            })
        }

        fn configure(&self, input_size: Size, output_size: Size) {
            if self
                .configured
                .borrow()
                .as_ref()
                .is_some_and(|c| c.input_size == input_size && c.output_size == output_size)
            {
                return;
            }
            self.canvas.resize(output_size);
            self.surface.configure(
                &self.device,
                &wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: self.surface_format,
                    color_space: wgpu::SurfaceColorSpace::Srgb,
                    width: output_size.width,
                    height: output_size.height,
                    present_mode: wgpu::PresentMode::Fifo,
                    alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                    view_formats: vec![],
                    desired_maximum_frame_latency: 2,
                },
            );
            let input = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("decoded VideoFrame texture"),
                size: wgpu::Extent3d {
                    width: input_size.width,
                    height: input_size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            *self.configured.borrow_mut() = Some(ConfiguredResources {
                input,
                input_size,
                output_size,
            });
        }

        async fn process(
            &self,
            decoded: VideoFrame,
            timestamp: i64,
            duration: i64,
        ) -> Result<JsValue, JsValue> {
            let decoded = OwnedVideoFrame(decoded);
            let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let ingress = (|| {
                let configured = self.configured.borrow();
                let configured = configured
                    .as_ref()
                    .ok_or_else(|| JsValue::from_str("GPU processor is not configured"))?;
                copy_decoded_frame(
                    self.queue
                        .as_webgpu()
                        .ok_or_else(|| JsValue::from_str("WebGPU queue unavailable"))?
                        .as_ref(),
                    configured
                        .input
                        .as_webgpu()
                        .ok_or_else(|| JsValue::from_str("WebGPU texture unavailable"))?
                        .as_ref(),
                    &decoded.0,
                    configured.input_size.width,
                    configured.input_size.height,
                )
            })();
            let bitmap = match async { JsFuture::from(ingress?).await }.await {
                Ok(bitmap) => bitmap,
                Err(error) => {
                    let _ = scope.pop().await;
                    return Err(error);
                }
            };
            if !bitmap.is_null() {
                BITMAP_COPIES.with(|value| value.set(value.get() + 1));
            }
            let _bitmap = OwnedBitmap(bitmap);
            let encoded_input = (|| {
                let configured = self.configured.borrow();
                let configured = configured
                    .as_ref()
                    .ok_or_else(|| JsValue::from_str("GPU processor is not configured"))?;
                let output = match self.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(texture)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
                    status => {
                        return Err(JsValue::from_str(&format!(
                            "canvas texture acquisition failed: {status:?}"
                        )));
                    }
                };
                let source = configured.input.create_view(&Default::default());
                let target = output.texture.create_view(&Default::default());
                let mut encoder =
                    self.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("video resize commands"),
                        });
                self.pipeline.record_resize(
                    &self.device,
                    &mut encoder,
                    &source,
                    &target,
                    configured.output_size,
                );
                self.queue.submit([encoder.finish()]);
                self.queue.present(output);
                let init = VideoFrameInit::new();
                init.set_timestamp_f64(timestamp as f64);
                init.set_duration_f64(duration as f64);
                self.canvas.capture(&init)
            })();
            // Even capture/acquisition errors must retire submitted GPU work before
            // dropping decoded/bitmap guards or allowing the next job to reconfigure.
            let (sender, receiver) = futures_channel::oneshot::channel();
            self.queue.on_submitted_work_done(move || {
                let _ = sender.send(());
            });
            let _ = receiver.await;
            if let Some(error) = scope.pop().await {
                if let Ok(frame) = encoded_input {
                    frame.close();
                }
                return Err(JsValue::from_str(&format!(
                    "WebGPU validation failed: {error}"
                )));
            }
            Ok(encoded_input?.into())
        }
    }

    fn string_property(value: &JsValue, name: &str, missing: &str) -> Result<String, MediaError> {
        Reflect::get(value, &JsValue::from_str(name))
            .map_err(js_error)?
            .as_string()
            .ok_or_else(|| platform(missing))
    }
    fn number_property(value: &JsValue, name: &str) -> Result<f64, MediaError> {
        let number = Reflect::get(value, &JsValue::from_str(name))
            .map_err(js_error)?
            .as_f64()
            .ok_or_else(|| platform(format!("invalid {name}")))?;
        if number.is_finite() && number >= 0.0 {
            Ok(number)
        } else {
            Err(platform(format!("invalid {name}")))
        }
    }
    fn bool_property(value: &JsValue, name: &str) -> Result<bool, MediaError> {
        Reflect::get(value, &JsValue::from_str(name))
            .map_err(js_error)?
            .as_bool()
            .ok_or_else(|| platform(format!("invalid {name}")))
    }
    fn u32_property(value: &JsValue, name: &str) -> Result<u32, MediaError> {
        let number = number_property(value, name)?;
        if number.fract() == 0.0 && number <= u32::MAX as f64 {
            Ok(number as u32)
        } else {
            Err(platform(format!("invalid {name}")))
        }
    }
    fn js_error(value: JsValue) -> MediaError {
        let message = value.as_string().or_else(|| {
            Reflect::get(&value, &JsValue::from_str("message"))
                .ok()?
                .as_string()
        });
        platform(message.unwrap_or_else(|| format!("{value:?}")))
    }
    fn platform(message: impl Into<String>) -> MediaError {
        MediaError::Platform(message.into())
    }
    fn describe_adapter(info: &wgpu::AdapterInfo) -> String {
        let name = if info.name.trim().is_empty() {
            "Browser-selected WebGPU adapter"
        } else {
            info.name.trim()
        };
        format!(
            "{name} ({:?}, {:?}, vendor 0x{:04x}, device 0x{:04x})",
            info.device_type, info.backend, info.vendor, info.device
        )
    }
}

#[cfg(target_arch = "wasm32")]
pub use browser::{
    ConversionResult, OutputProfileCapabilities, cancel, convert_m3, probe_output_profiles, run_m1,
    selected_gpu, setup_runtime,
};
#[cfg(not(target_arch = "wasm32"))]
pub fn cancel() {}
#[cfg(not(target_arch = "wasm32"))]
pub fn selected_gpu() -> Option<String> {
    None
}
