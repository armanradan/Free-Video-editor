#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod browser {
    use js_sys::{Function, Promise, Reflect};
    use media_core::{INPUT_SIZE, MediaError, OUTPUT_SIZE};
    use media_gpu::ResizePipeline;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    use wasm_bindgen::{JsCast, prelude::*};
    use wasm_bindgen_futures::{JsFuture, future_to_promise};
    use web_sys::{HtmlCanvasElement, VideoFrame, VideoFrameInit};

    thread_local! {
        static GENERATION: Cell<u32> = const { Cell::new(0) };
        static GPU_SESSION: RefCell<Option<Rc<GpuSession>>> = const { RefCell::new(None) };
    }

    #[wasm_bindgen(inline_js = r#"
        export function invokeM1(fixture, manifest, processFrame, status, cancelled) {
            return globalThis.__DIAXUS_M1__.run(fixture, manifest, processFrame, status, cancelled);
        }

        export async function describeSelectedAdapter() {
            const adapter = await navigator.gpu?.requestAdapter({ powerPreference: "high-performance" });
            if (!adapter) return "Browser did not return a WebGPU adapter";
            const info = adapter.info ?? await adapter.requestAdapterInfo?.() ?? {};
            const values = [info.description, info.vendor, info.architecture, info.device]
                .filter(value => typeof value === "string" && value.trim().length > 0);
            const unique = [...new Set(values)];
            const identity = unique.length > 0 ? unique.join(" / ") : "identity redacted by browser";
            return `${identity}${info.isFallbackAdapter ? " (fallback adapter)" : ""}`;
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

        #[wasm_bindgen(js_name = describeSelectedAdapter, catch)]
        fn describe_selected_adapter() -> Result<Promise, JsValue>;
    }

    pub fn cancel() {
        GENERATION.with(|generation| generation.set(generation.get().wrapping_add(1)));
    }

    pub fn selected_gpu() -> Option<String> {
        GPU_SESSION.with(|slot| {
            slot.borrow()
                .as_ref()
                .map(|session| session.adapter_label.clone())
        })
    }

    pub async fn run(canvas_id: &str, status: Function) -> Result<String, MediaError> {
        let generation = GENERATION.with(|current| {
            let next = current.get().wrapping_add(1);
            current.set(next);
            next
        });
        let window = web_sys::window().ok_or_else(|| platform("window is unavailable"))?;
        let document = window
            .document()
            .ok_or_else(|| platform("document is unavailable"))?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| platform("export canvas was not mounted"))?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| platform("export element is not a canvas"))?;
        canvas.set_width(OUTPUT_SIZE.width);
        canvas.set_height(OUTPUT_SIZE.height);

        let existing = GPU_SESSION.with(|slot| slot.borrow().clone());
        let gpu = if let Some(existing) = existing {
            existing
        } else {
            let created = Rc::new(GpuSession::new(canvas).await?);
            GPU_SESSION.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&created)));
            created
        };
        let process_gpu = Rc::clone(&gpu);
        let process_frame = Closure::<dyn FnMut(JsValue, f64, f64) -> Promise>::new(
            move |value: JsValue, timestamp: f64, duration: f64| {
                let gpu = Rc::clone(&process_gpu);
                future_to_promise(async move {
                    const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
                    let safe_integer = |value: f64| {
                        value.is_finite() && value.fract() == 0.0 && value.abs() <= MAX_SAFE_INTEGER
                    };
                    if !safe_integer(timestamp) || !safe_integer(duration) {
                        return Err(JsValue::from_str(
                            "WebCodecs timestamp/duration was not a safe integer",
                        ));
                    }
                    let frame = value
                        .dyn_into::<VideoFrame>()
                        .map_err(|_| JsValue::from_str("decoder output was not a VideoFrame"))?;
                    gpu.process(frame, timestamp as i64, duration as i64).await
                })
            },
        );
        let cancelled = Closure::<dyn FnMut() -> bool>::new(move || {
            GENERATION.with(|current| current.get() != generation)
        });
        let fixture =
            js_sys::Uint8Array::from(include_bytes!("../../../fixtures/m1-vp8.ivf").as_slice());
        let manifest = include_str!("../../../fixtures/m1-vp8.json");
        let promise = invoke_m1(
            fixture,
            manifest,
            process_frame.as_ref().unchecked_ref(),
            &status,
            cancelled.as_ref().unchecked_ref(),
        )
        .map_err(js_error)?;
        let result = JsFuture::from(promise).await.map_err(js_error)?;
        Reflect::get(&result, &JsValue::from_str("summary"))
            .map_err(js_error)?
            .as_string()
            .ok_or_else(|| platform("probe returned no summary"))
    }

    struct GpuSession {
        adapter_label: String,
        canvas: HtmlCanvasElement,
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        input: wgpu::Texture,
        pipeline: ResizePipeline,
    }

    struct OwnedVideoFrame(VideoFrame);

    impl Drop for OwnedVideoFrame {
        fn drop(&mut self) {
            self.0.close();
        }
    }

    impl GpuSession {
        async fn new(canvas: HtmlCanvasElement) -> Result<Self, MediaError> {
            let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            instance_descriptor.backends = wgpu::Backends::BROWSER_WEBGPU;
            let instance = wgpu::Instance::new(instance_descriptor);
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
                    platform(format!(
                        "no WebGPU adapter supports the export canvas: {error}"
                    ))
                })?;
            let rust_adapter_label = describe_adapter(&adapter.get_info());
            let adapter_label = match describe_selected_adapter() {
                Ok(promise) => JsFuture::from(promise)
                    .await
                    .ok()
                    .and_then(|value| value.as_string())
                    .filter(|value| !value.trim().is_empty())
                    .map(|value| format!("{value} ({:?})", adapter.get_info().backend))
                    .unwrap_or(rust_adapter_label),
                Err(_) => rust_adapter_label,
            };
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("M1 processing device"),
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
            let format = capabilities
                .formats
                .iter()
                .copied()
                .find(|format| format.is_srgb())
                .or_else(|| capabilities.formats.first().copied())
                .ok_or_else(|| platform("canvas surface reported no texture formats"))?;
            surface.configure(
                &device,
                &wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format,
                    color_space: wgpu::SurfaceColorSpace::Srgb,
                    width: OUTPUT_SIZE.width,
                    height: OUTPUT_SIZE.height,
                    present_mode: wgpu::PresentMode::Fifo,
                    alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                    view_formats: vec![],
                    desired_maximum_frame_latency: 2,
                },
            );
            let input = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("M1 decoded VideoFrame texture"),
                size: wgpu::Extent3d {
                    width: INPUT_SIZE.width,
                    height: INPUT_SIZE.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                // WebGPU external-image copies require RENDER_ATTACHMENT in addition
                // to COPY_DST. TEXTURE_BINDING is needed by the resize shader.
                usage: wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let pipeline = ResizePipeline::new(&device, format);
            Ok(Self {
                adapter_label,
                canvas,
                surface,
                device,
                queue,
                input,
                pipeline,
            })
        }

        async fn process(
            &self,
            decoded: VideoFrame,
            timestamp: i64,
            duration: i64,
        ) -> Result<JsValue, JsValue> {
            let decoded = OwnedVideoFrame(decoded);
            let validation_scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let copy_handle: JsValue = <VideoFrame as AsRef<JsValue>>::as_ref(&decoded.0).clone();
            self.queue.copy_external_image_to_texture(
                &wgpu::CopyExternalImageSourceInfo {
                    source: wgpu::ExternalImageSource::VideoFrame(copy_handle.unchecked_into()),
                    origin: wgpu::Origin2d::ZERO,
                    flip_y: false,
                },
                wgpu::CopyExternalImageDestInfo {
                    texture: &self.input,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                    color_space: wgpu::PredefinedColorSpace::Srgb,
                    premultiplied_alpha: false,
                },
                wgpu::Extent3d {
                    width: INPUT_SIZE.width,
                    height: INPUT_SIZE.height,
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
            let source_view = self.input.create_view(&Default::default());
            let target_view = output.texture.create_view(&Default::default());
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("M1 frame commands"),
                });
            self.pipeline.record_resize(
                &self.device,
                &mut encoder,
                &source_view,
                &target_view,
                OUTPUT_SIZE,
            );
            self.queue.submit([encoder.finish()]);

            // Release/present immediately after submission. Holding the acquired
            // surface texture while waiting for completion eventually back-pressures
            // Edge's canvas swap chain (observed at frame 10). The decoded source
            // frame remains owned by the caller until this future returns.
            self.queue.present(output);

            // Capture immediately after present so Chromium has a consumer for the
            // committed canvas image. Both this snapshot and the decoded source stay
            // alive through the submission-specific completion wait below.
            let init = VideoFrameInit::new();
            init.set_timestamp_f64(timestamp as f64);
            init.set_duration_f64(duration as f64);
            let encoded_input =
                VideoFrame::new_with_html_canvas_element_and_video_frame_init(&self.canvas, &init)?;

            // Edge/wgpu 30 produced black snapshots without this submission-specific
            // completion point. Do not schedule unrelated work in this sequence.
            let (sender, receiver) = futures_channel::oneshot::channel();
            self.queue.on_submitted_work_done(move || {
                let _ = sender.send(());
            });
            let _ = receiver.await;
            if let Some(error) = validation_scope.pop().await {
                encoded_input.close();
                return Err(JsValue::from_str(&format!(
                    "WebGPU validation failed during external copy/resize: {error}"
                )));
            }
            Ok(encoded_input.into())
        }
    }

    fn js_error(value: JsValue) -> MediaError {
        let message = value.as_string().or_else(|| {
            Reflect::get(&value, &JsValue::from_str("message"))
                .ok()
                .and_then(|message| message.as_string())
        });
        MediaError::Platform(message.unwrap_or_else(|| format!("{value:?}")))
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
        let mut details = vec![
            format!("{:?}", info.device_type),
            format!("{:?}", info.backend),
        ];
        if info.vendor != 0 {
            details.push(format!("vendor 0x{:04x}", info.vendor));
        }
        if info.device != 0 {
            details.push(format!("device 0x{:04x}", info.device));
        }
        if !info.driver.trim().is_empty() {
            details.push(format!("driver {}", info.driver.trim()));
        }
        format!("{name} ({})", details.join(", "))
    }
}

#[cfg(target_arch = "wasm32")]
pub use browser::{cancel, run, selected_gpu};

#[cfg(not(target_arch = "wasm32"))]
pub fn cancel() {}

#[cfg(not(target_arch = "wasm32"))]
pub fn selected_gpu() -> Option<String> {
    None
}
