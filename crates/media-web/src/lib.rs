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
    use web_sys::{File, HtmlCanvasElement, HtmlInputElement, VideoFrame, VideoFrameInit};

    thread_local! {
        static GENERATION: Cell<u32> = const { Cell::new(0) };
        static GPU_SESSION: RefCell<Option<Rc<GpuSession>>> = const { RefCell::new(None) };
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

    #[wasm_bindgen(inline_js = r#"
        export function invokeM1(fixture, manifest, processFrame, status, cancelled) {
            return globalThis.__DIAXUS_M1__.run(fixture, manifest, processFrame, status, cancelled);
        }
        export function inspectBrowserInput(file) { return globalThis.__DIAXUS_MEDIA_WEB__.inspect(file); }
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
    }

    pub fn cancel() {
        GENERATION.with(|value| value.set(value.get().wrapping_add(1)));
    }

    pub fn selected_gpu() -> Option<String> {
        GPU_SESSION.with(|slot| {
            slot.borrow()
                .as_ref()
                .map(|session| session.adapter_label.clone())
        })
    }

    pub async fn run_m1(canvas_id: &str, status: Function) -> Result<String, MediaError> {
        let generation = begin_generation();
        let gpu = configured_gpu(canvas_id, INPUT_SIZE, OUTPUT_SIZE).await?;
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
        string_property(&result, "summary", "probe returned no summary")
    }

    pub async fn convert_m3(
        file_input_id: &str,
        canvas_id: &str,
        profile: OutputProfileId,
        status: Function,
    ) -> Result<ConversionResult, MediaError> {
        let generation = begin_generation();
        let file = selected_file(file_input_id)?;
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
        let gpu = configured_gpu(canvas_id, input, output).await?;
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

    fn begin_generation() -> u32 {
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

    async fn configured_gpu(
        canvas_id: &str,
        input: Size,
        output: Size,
    ) -> Result<Rc<GpuSession>, MediaError> {
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| platform("document is unavailable"))?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| platform("export canvas was not mounted"))?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| platform("export element is not a canvas"))?;
        let existing = GPU_SESSION.with(|slot| slot.borrow().clone());
        let gpu = if let Some(existing) = existing {
            existing
        } else {
            let created = Rc::new(GpuSession::new(canvas).await?);
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
        canvas: HtmlCanvasElement,
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        pipeline: ResizePipeline,
        configured: RefCell<Option<ConfiguredResources>>,
    }
    struct OwnedVideoFrame(VideoFrame);
    impl Drop for OwnedVideoFrame {
        fn drop(&mut self) {
            self.0.close();
        }
    }

    impl GpuSession {
        async fn new(canvas: HtmlCanvasElement) -> Result<Self, MediaError> {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            descriptor.backends = wgpu::Backends::BROWSER_WEBGPU;
            let instance = wgpu::Instance::new(descriptor);
            let surface = instance
                .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
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
            self.canvas.set_width(output_size.width);
            self.canvas.set_height(output_size.height);
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
            let encoded_input = {
                let configured = self.configured.borrow();
                let configured = configured
                    .as_ref()
                    .ok_or_else(|| JsValue::from_str("GPU processor is not configured"))?;
                let handle: JsValue = <VideoFrame as AsRef<JsValue>>::as_ref(&decoded.0).clone();
                self.queue.copy_external_image_to_texture(
                    &wgpu::CopyExternalImageSourceInfo {
                        source: wgpu::ExternalImageSource::VideoFrame(handle.unchecked_into()),
                        origin: wgpu::Origin2d::ZERO,
                        flip_y: false,
                    },
                    wgpu::CopyExternalImageDestInfo {
                        texture: &configured.input,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                        color_space: wgpu::PredefinedColorSpace::Srgb,
                        premultiplied_alpha: false,
                    },
                    wgpu::Extent3d {
                        width: configured.input_size.width,
                        height: configured.input_size.height,
                        depth_or_array_layers: 1,
                    },
                );
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
                VideoFrame::new_with_html_canvas_element_and_video_frame_init(&self.canvas, &init)?
            };
            let (sender, receiver) = futures_channel::oneshot::channel();
            self.queue.on_submitted_work_done(move || {
                let _ = sender.send(());
            });
            let _ = receiver.await;
            if let Some(error) = scope.pop().await {
                encoded_input.close();
                return Err(JsValue::from_str(&format!(
                    "WebGPU validation failed: {error}"
                )));
            }
            Ok(encoded_input.into())
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
pub use browser::{ConversionResult, cancel, convert_m3, run_m1, selected_gpu};
#[cfg(not(target_arch = "wasm32"))]
pub fn cancel() {}
#[cfg(not(target_arch = "wasm32"))]
pub fn selected_gpu() -> Option<String> {
    None
}
