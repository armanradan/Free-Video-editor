#![forbid(unsafe_code)]

pub use media_core::VideoSettings;

#[cfg(target_arch = "wasm32")]
mod browser {
    use js_sys::{Function, Promise, Reflect};
    use media_core::equalization::Equalization;
    use media_core::{
        BitrateSource, CodecAcceleration, ColorAdjustments, FrameGeometry, FrameRateGrid,
        FrameRateSpec, INPUT_SIZE, MediaError, OUTPUT_SIZE, OutputProfileId, Rect, ResizeSpec,
        Rotation, Size, TrackCoverage, VideoBitrate, VideoCodec, VideoSettings,
        estimate_output_coverage, validate_input_size,
    };
    use media_gpu::ResizePipeline;
    use media_gpu::equalization::{EqualizationStage, PreviewColorStage};
    use std::{
        cell::{Cell, RefCell},
        future::Future,
        pin::Pin,
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };
    use wasm_bindgen::{JsCast, prelude::*};
    use wasm_bindgen_futures::{JsFuture, future_to_promise};
    use web_sys::{
        File, HtmlCanvasElement, HtmlInputElement, OffscreenCanvas, VideoFrame, VideoFrameInit,
    };

    const GPU_POOL_SIZE: usize = 4;

    thread_local! {
        static GENERATION: Cell<u32> = const { Cell::new(0) };
        static NEXT_DEVICE_GENERATION: Cell<u32> = const { Cell::new(0) };
        static GPU_SESSION: RefCell<Option<Rc<GpuSession>>> = const { RefCell::new(None) };
        static GPU_CANVAS: RefCell<Option<ExportCanvas>> = const { RefCell::new(None) };
        static DEVICE_LOSS_INJECTION_CONSUMED: Cell<bool> = const { Cell::new(false) };
        static BITMAP_COPIES: Cell<u32> = const { Cell::new(0) };
        static BITMAP_INGRESS_REQUIRED: Cell<bool> = const { Cell::new(false) };
        static PROCESSING_METRICS: Cell<ProcessingMetrics> = const { Cell::new(ProcessingMetrics::ZERO) };
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
        pub hevc_supported: bool,
        pub hevc_reason: String,
        pub output_size: Size,
        pub source: SourceMetadata,
    }

    #[derive(Clone, Debug)]
    pub struct SourceMetadata {
        pub video_bitrate_bps: Option<u64>,
        pub variable_frame_rate: bool,
        pub video_start_us: i64,
        pub video_end_us: i64,
        pub audio_start_us: Option<i64>,
        pub audio_end_us: Option<i64>,
        pub file_name: String,
        pub file_size: u64,
        pub display_size: Size,
        pub coded_size: Size,
        pub video_codec: String,
        pub duration_seconds: f64,
        pub frame_rate: f64,
        pub frame_count: u64,
        pub audio_codec: Option<String>,
        pub audio_channels: u32,
        pub audio_sample_rate: u32,
    }

    impl SourceMetadata {
        pub fn resolve_bitrate(
            &self,
            output: Size,
            profile: OutputProfileId,
            settings: VideoSettings,
        ) -> Result<u32, MediaError> {
            // Average rate is used only for bitrate policy, never to replace VFR timestamps.
            let num = (self.frame_rate * 1_000_000.0).round();
            if !num.is_finite() || num < 1.0 || num > f64::from(u32::MAX) {
                return Err(MediaError::InvalidFrameRate);
            }
            settings.resolve_bitrate(
                output,
                profile.video_codec(),
                BitrateSource {
                    size: self.display_size,
                    fps_num: num as u32,
                    fps_den: 1_000_000,
                    codec: if self.video_codec.starts_with("hvc")
                        || self.video_codec.starts_with("hev")
                    {
                        VideoCodec::H265
                    } else {
                        VideoCodec::H264
                    },
                    bitrate_bps: self.video_bitrate_bps,
                },
            )
        }

        pub fn estimate(
            &self,
            output: Size,
            profile: OutputProfileId,
            settings: VideoSettings,
        ) -> Result<(u32, Option<u64>), MediaError> {
            let bps = self.resolve_bitrate(output, profile, settings)?;
            let audio = profile != OutputProfileId::WebmVp8VideoOnly && self.audio_codec.is_some();
            Ok((
                bps,
                estimate_output_coverage(
                    bps,
                    TrackCoverage {
                        start_us: i128::from(self.video_start_us),
                        end_us: Some(i128::from(self.video_end_us)),
                    },
                    audio.then_some((
                        TrackCoverage {
                            start_us: i128::from(
                                self.audio_start_us.unwrap_or(self.video_start_us),
                            ),
                            end_us: self
                                .audio_end_us
                                .filter(|_| self.audio_start_us.is_some())
                                .map(i128::from),
                        },
                        if profile == OutputProfileId::WebmVp8Opus {
                            128_000
                        } else {
                            192_000
                        },
                    )),
                ),
            ))
        }
    }

    #[wasm_bindgen(inline_js = r#"
        export function runtimeModuleUrl() { return new URL('../../converter-web.js', import.meta.url).href; }
        export function performanceNow() { return performance.now(); }
        export function setCaptureGeometry(init, width, height) {
            init.visibleRect = {x: 0, y: 0, width, height};
            init.displayWidth = width;
            init.displayHeight = height;
            init.alpha = 'discard';
        }
        export function invokeM1(fixture, manifest, processFrame, status, cancelled) {
            return globalThis.__DIAXUS_M1__.run(fixture, manifest, processFrame, status, cancelled);
        }
        export function inspectBrowserInput(file) { return globalThis.__DIAXUS_MEDIA_WEB__.inspect(file); }
        export function previewInfo(file, key, seconds, streaming) { return globalThis.__DIAXUS_MEDIA_WEB__.previewInfo(file, key, seconds, streaming); }
        export function previewRender(processFrame) { return globalThis.__DIAXUS_MEDIA_WEB__.previewRender(processFrame); }
        export function previewHistory(file, processFrame, windowUs) { return globalThis.__DIAXUS_MEDIA_WEB__.previewHistory(file, processFrame, windowUs); }
        export function clearPreview() { globalThis.__DIAXUS_MEDIA_WEB__.clearPreview(); }
        export function previewDiagnosticsEnabled() {
            return new URL(globalThis.location.href).searchParams.get('diagnostic-preview') === '1';
        }
        async function diagnosticIngress(device, texture, width, height, bitmap) {
            if (!globalThis.__DIAXUS_PREVIEW_DIAGNOSTIC_ACTIVE__) return bitmap;
            const bytesPerRow = Math.ceil(width * 4 / 256) * 256;
            const buffer = device.createBuffer({size: bytesPerRow * height,
                usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ});
            try {
                const encoder = device.createCommandEncoder();
                encoder.copyTextureToBuffer({texture}, {buffer, bytesPerRow}, [width, height]);
                device.queue.submit([encoder.finish()]);
                await buffer.mapAsync(GPUMapMode.READ);
                const mapped = new Uint8Array(buffer.getMappedRange());
                const pixels = new Uint8Array(width * height * 4);
                for (let y = 0; y < height; y++) pixels.set(mapped.subarray(y * bytesPerRow, y * bytesPerRow + width * 4), y * width * 4);
                globalThis.__DIAXUS_PREVIEW_INGRESS__ = {width, height, pixels};
                return bitmap;
            } catch (error) {
                bitmap?.close();
                throw error;
            } finally { buffer.unmap(); buffer.destroy(); }
        }
        export async function copyDecodedFrame(queue, device, texture, frame, preparedBitmap, width, height) {
            const destination = { texture, colorSpace: 'srgb', premultipliedAlpha: false };
            if (preparedBitmap != null) {
                try {
                    queue.copyExternalImageToTexture({ source: preparedBitmap }, destination, [width, height]);
                    return globalThis.__DIAXUS_PREVIEW_DIAGNOSTIC_ACTIVE__
                        ? diagnosticIngress(device, texture, width, height, preparedBitmap) : preparedBitmap;
                } catch (error) {
                    preparedBitmap.close();
                    throw new Error(`Prepared ImageBitmap GPU ingress failed: ${error.message}`);
                }
            }
            try {
                queue.copyExternalImageToTexture({ source: frame }, destination, [width, height]);
                return globalThis.__DIAXUS_PREVIEW_DIAGNOSTIC_ACTIVE__
                    ? diagnosticIngress(device, texture, width, height, null) : null;
            } catch (directError) {
                if (!(directError instanceof TypeError)) throw directError;
                const bitmap = await createImageBitmap(frame);
                try {
                    queue.copyExternalImageToTexture({ source: bitmap }, destination, [width, height]);
                    return globalThis.__DIAXUS_PREVIEW_DIAGNOSTIC_ACTIVE__
                        ? diagnosticIngress(device, texture, width, height, bitmap) : bitmap;
                } catch (error) {
                    bitmap.close();
                    throw new Error(`VideoFrame and ImageBitmap GPU ingress failed: ${error.message}`);
                }
            }
        }
        export function probeBrowserProfiles(file, width, height, options) {
            return globalThis.__DIAXUS_MEDIA_WEB__.probeProfiles(file, width, height, options);
        }
        export function invokeBrowserJob(file, width, height, profile, acceleration, outputMode, verifyOutput, failureMode, processFrame, bitmapIngressRequired, status, cancelled, options, grid) {
            return globalThis.__DIAXUS_MEDIA_WEB__.run(file, width, height, profile, acceleration, outputMode, verifyOutput, failureMode, processFrame, bitmapIngressRequired, status, cancelled, options, grid);
        }
        export function describeSelectedDevice(device) {
            const info = device.adapterInfo ?? {};
            const values = [info.description, info.vendor, info.architecture, info.device]
                .filter(value => typeof value === "string" && value.trim().length > 0);
            const unique = [...new Set(values)];
            return unique.length ? `${unique.join(" / ")}${info.isFallbackAdapter ? " (fallback adapter)" : ""}` : "";
        }
    "#)]
    extern "C" {
        #[wasm_bindgen(js_name = previewInfo, catch)]
        fn preview_info(
            file: &File,
            key: &str,
            seconds: f64,
            streaming: bool,
        ) -> Result<Promise, JsValue>;
        #[wasm_bindgen(js_name = previewRender, catch)]
        fn preview_render(process: &Function) -> Result<Promise, JsValue>;
        #[wasm_bindgen(js_name = previewHistory, catch)]
        fn preview_history(
            file: &File,
            process: &Function,
            window_us: i64,
        ) -> Result<Promise, JsValue>;
        #[wasm_bindgen(js_name = clearPreview)]
        fn clear_preview();
        #[wasm_bindgen(js_name = runtimeModuleUrl)]
        fn runtime_module_url() -> String;
        #[wasm_bindgen(js_name = performanceNow)]
        fn performance_now() -> f64;
        #[wasm_bindgen(js_name = setCaptureGeometry)]
        fn set_capture_geometry(init: &VideoFrameInit, width: u32, height: u32);
        #[wasm_bindgen(js_name = copyDecodedFrame, catch)]
        fn copy_decoded_frame(
            queue: &JsValue,
            device: &JsValue,
            texture: &JsValue,
            frame: &VideoFrame,
            prepared_bitmap: &JsValue,
            width: u32,
            height: u32,
        ) -> Result<Promise, JsValue>;
        #[wasm_bindgen(js_name = previewDiagnosticsEnabled)]
        fn preview_diagnostics_enabled() -> bool;
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
        fn probe_browser_profiles(
            file: &File,
            width: u32,
            height: u32,
            options: &JsValue,
        ) -> Result<Promise, JsValue>;
        #[wasm_bindgen(js_name = invokeBrowserJob, catch)]
        fn invoke_browser_job(
            file: &File,
            width: u32,
            height: u32,
            profile: &str,
            acceleration: &str,
            output_mode: &str,
            verify_output: bool,
            failure_mode: &str,
            process_frame: &Function,
            bitmap_ingress_required: &Function,
            status: &Function,
            cancelled: &Function,
            options: &JsValue,
            grid: &Function,
        ) -> Result<Promise, JsValue>;
        #[wasm_bindgen(js_name = describeSelectedDevice, catch)]
        fn describe_selected_device(device: &JsValue) -> Result<String, JsValue>;
    }

    #[wasm_bindgen(module = "/src/worker-host.js")]
    extern "C" {
        #[wasm_bindgen(js_name = setupRuntime)]
        fn setup_runtime_js(m1: &str, pipeline: &str, wasm: &str);
        #[wasm_bindgen(js_name = setFfmpegAssets)]
        fn set_ffmpeg_assets_js(core: &str, wasm: &str, worker: &str);
        #[wasm_bindgen(js_name = dispatchJob, catch)]
        fn dispatch_job(
            file: Option<File>,
            operation: &str,
            profile: &str,
            acceleration: &str,
            resize: &str,
            status: &Function,
            local: &Function,
        ) -> Result<Promise, JsValue>;
        #[wasm_bindgen(js_name = cancelRemote)]
        fn cancel_remote();
    }

    pub fn setup_runtime(m1: &str, pipeline: &str) {
        setup_runtime_js(m1, pipeline, &runtime_module_url());
    }

    pub fn setup_ffmpeg_assets(core: &str, wasm: &str, worker: &str) {
        set_ffmpeg_assets_js(core, wasm, worker);
    }

    #[wasm_bindgen(module = "/src/preview-clock.js")]
    extern "C" {
        #[wasm_bindgen(js_name = startPreviewClock, catch)]
        fn start_preview_clock(
            file: &File,
            seconds: f64,
            start: f64,
            duration: f64,
            muted: bool,
        ) -> Result<Promise, JsValue>;
        #[wasm_bindgen(js_name = pausePreviewClock)]
        pub fn pause_preview_playback();
        #[wasm_bindgen(js_name = releasePreviewClock)]
        pub fn release_preview_playback();
        #[wasm_bindgen(js_name = seekPreviewClock)]
        pub fn seek_preview_playback(seconds: f64);
        #[wasm_bindgen(js_name = mutePreviewClock)]
        pub fn mute_preview_playback(muted: bool);
        #[wasm_bindgen(js_name = previewClockPosition)]
        pub fn preview_playback_position() -> f64;
        #[wasm_bindgen(js_name = previewClockPlaying)]
        pub fn preview_playback_playing() -> bool;
        #[wasm_bindgen(js_name = previewClockReady)]
        pub fn preview_playback_ready() -> bool;
        #[wasm_bindgen(js_name = previewClockError)]
        pub fn preview_playback_error() -> String;
    }

    pub fn start_preview_playback(
        seconds: f64,
        start: f64,
        duration: f64,
        muted: bool,
    ) -> Result<Promise, MediaError> {
        start_preview_clock(
            &selected_file("source-file")?,
            seconds,
            start,
            duration,
            muted,
        )
        .map_err(js_error)
    }

    async fn dispatch(
        file: Option<File>,
        operation: &str,
        profile: &str,
        acceleration: &str,
        resize: ResizeSpec,
        settings: Option<VideoSettings>,
        status: Function,
    ) -> Result<JsValue, MediaError> {
        let mut resize = resize_command(resize);
        if let Some(settings) = settings {
            let bitrate = match settings.bitrate {
                VideoBitrate::Smaller => "smaller".into(),
                VideoBitrate::Recommended => "recommended".into(),
                VideoBitrate::Higher => "higher".into(),
                VideoBitrate::BitsPerSecond(value) => format!("bps={value}"),
            };
            let fps = match settings.frame_rate {
                FrameRateSpec::Original => "original".into(),
                FrameRateSpec::Constant {
                    numerator,
                    denominator,
                } => format!("{numerator}/{denominator}"),
            };
            let (brightness, contrast, saturation) = settings.color.values();
            resize.push_str(&format!(
                "~{bitrate}~{fps}~{brightness}/{contrast}/{saturation}~{}",
                settings.equalization.command()
            ));
        }
        let local = Closure::<
            dyn FnMut(JsValue, String, String, String, String, String, Function) -> Promise,
        >::new(
            |file: JsValue,
             operation: String,
             profile: String,
             acceleration: String,
             resize: String,
             execution_options: String,
             status: Function| {
                future_to_promise(execute_job(
                    file.dyn_into::<File>().ok(),
                    operation,
                    profile,
                    acceleration,
                    resize,
                    execution_options,
                    status,
                ))
            },
        );
        let result = JsFuture::from(
            dispatch_job(
                file,
                operation,
                profile,
                acceleration,
                &resize,
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
    pub async fn initialize_worker(
        canvas: OffscreenCanvas,
        stage: Function,
    ) -> Result<(), JsValue> {
        let canvas = ExportCanvas::Offscreen(canvas);
        GPU_CANVAS.with(|slot| *slot.borrow_mut() = Some(canvas.clone()));
        let gpu = Rc::new(
            GpuSession::new(canvas, Some(&stage))
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))?,
        );
        startup_stage(Some(&stage), "surface configure/capture");
        gpu.configure(INPUT_SIZE, OUTPUT_SIZE, Rotation::Deg0, false, false)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
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
        acceleration: String,
        resize: String,
        execution_options: String,
        status: Function,
    ) -> Result<JsValue, JsValue> {
        let mut parts = resize.split('~');
        let resize = resize_from_command(parts.next().unwrap_or(""))
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let settings = match (parts.next(), parts.next(), parts.next()) {
            (None, None, None) => None,
            (Some(rate), Some(fps), None) => Some(
                settings_from_command(rate, fps)
                    .map_err(|error| JsValue::from_str(&error.to_string()))?,
            ),
            (Some(rate), Some(fps), Some(color)) => {
                let equalization = parts
                    .next()
                    .map(str::parse::<Equalization>)
                    .transpose()
                    .map_err(|e| JsValue::from_str(&e.to_string()))?
                    .unwrap_or_default();
                if parts.next().is_some() {
                    return Err(JsValue::from_str("invalid video settings"));
                }
                let mut values = color.split('/');
                let parsed = ColorAdjustments::new(
                    values
                        .next()
                        .unwrap_or("")
                        .parse()
                        .map_err(|_| JsValue::from_str("invalid brightness"))?,
                    values
                        .next()
                        .unwrap_or("")
                        .parse()
                        .map_err(|_| JsValue::from_str("invalid contrast"))?,
                    values
                        .next()
                        .unwrap_or("")
                        .parse()
                        .map_err(|_| JsValue::from_str("invalid saturation"))?,
                )
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
                if values.next().is_some() {
                    return Err(JsValue::from_str("invalid color settings"));
                }
                let mut settings = settings_from_command(rate, fps)
                    .map_err(|error| JsValue::from_str(&error.to_string()))?;
                settings.color = parsed;
                settings.equalization = equalization;
                Some(settings)
            }
            _ => return Err(JsValue::from_str("invalid video settings")),
        };
        let execution_options = execution_options_from_command(&execution_options)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let result = match operation.as_str() {
            "preview" => match file {
                Some(file) => {
                    preview_file(
                        file,
                        &profile,
                        resize,
                        settings.unwrap_or_default(),
                        status,
                        &execution_options.failure_mode,
                    )
                    .await
                }
                None => Err(platform("no source file")),
            },
            "clear-preview" => {
                clear_preview_state();
                Ok(js_sys::Object::new().into())
            }
            "m1" => run_m1_local(status).await,
            "probe" => match file {
                Some(file) => probe_file(file, resize, settings).await,
                None => Err(platform("no source file")),
            },
            "convert" => {
                let profile = match profile.as_str() {
                    "webm-vp8-opus" => OutputProfileId::WebmVp8Opus,
                    "webm-vp8-video-only" => OutputProfileId::WebmVp8VideoOnly,
                    "mp4-h264-aac" => OutputProfileId::Mp4H264Aac,
                    "mp4-h265-aac" => OutputProfileId::Mp4H265Aac,
                    _ => return Err(JsValue::from_str("unknown output profile")),
                };
                let acceleration = match acceleration.as_str() {
                    "prefer-hardware" => CodecAcceleration::PreferHardware,
                    _ => CodecAcceleration::NoPreference,
                };
                match file {
                    Some(file) => {
                        convert_file(
                            file,
                            profile,
                            acceleration,
                            resize,
                            settings,
                            execution_options,
                            status,
                        )
                        .await
                    }
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
            request: ConversionRequest<'_>,
            process_frame: &Function,
            bitmap_ingress_required: &Function,
            status: &Function,
            cancelled: &Function,
        ) -> Result<Promise, JsValue>;
        fn probe_profiles(
            &self,
            file: &File,
            output: Size,
            options: &JsValue,
        ) -> Result<Promise, JsValue>;
    }

    struct WebCodecsMediabunnyBackend;

    struct ExecutionOptions {
        output_mode: String,
        verify_output: bool,
        failure_mode: String,
    }

    #[derive(Clone, Copy)]
    struct ConversionRequest<'a> {
        output: Size,
        profile: OutputProfileId,
        acceleration: CodecAcceleration,
        output_mode: &'a str,
        verify_output: bool,
        failure_mode: &'a str,
        options: &'a JsValue,
        grid: &'a Function,
    }

    impl BrowserConversionBackend for WebCodecsMediabunnyBackend {
        fn inspect(&self, file: &File) -> Result<Promise, JsValue> {
            inspect_browser_input(file)
        }

        fn run(
            &self,
            file: &File,
            request: ConversionRequest<'_>,
            process_frame: &Function,
            bitmap_ingress_required: &Function,
            status: &Function,
            cancelled: &Function,
        ) -> Result<Promise, JsValue> {
            invoke_browser_job(
                file,
                request.output.width,
                request.output.height,
                request.profile.as_str(),
                request.acceleration.as_str(),
                request.output_mode,
                request.verify_output,
                request.failure_mode,
                process_frame,
                bitmap_ingress_required,
                status,
                cancelled,
                request.options,
                request.grid,
            )
        }

        fn probe_profiles(
            &self,
            file: &File,
            output: Size,
            options: &JsValue,
        ) -> Result<Promise, JsValue> {
            probe_browser_profiles(file, output.width, output.height, options)
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
        let result = dispatch(None, "m1", "", "", ResizeSpec::DEFAULT, None, status).await?;
        string_property(&result, "summary", "probe returned no summary")
    }

    pub async fn preview_source(
        source_key: u64,
        seconds: f64,
        resize: ResizeSpec,
        color: ColorAdjustments,
        equalization: Equalization,
    ) -> Result<String, MediaError> {
        preview_source_mode(source_key, seconds, resize, color, equalization, false).await
    }

    pub async fn preview_playback_source(
        source_key: u64,
        seconds: f64,
        resize: ResizeSpec,
        color: ColorAdjustments,
        equalization: Equalization,
    ) -> Result<String, MediaError> {
        preview_source_mode(source_key, seconds, resize, color, equalization, true).await
    }

    async fn preview_source_mode(
        source_key: u64,
        seconds: f64,
        resize: ResizeSpec,
        color: ColorAdjustments,
        equalization: Equalization,
        streaming: bool,
    ) -> Result<String, MediaError> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(platform("invalid preview position"));
        }
        let result = dispatch(
            Some(selected_file("source-file")?),
            "preview",
            &format!("{source_key}@{seconds}@{}", u8::from(streaming)),
            "",
            resize,
            Some(VideoSettings {
                color,
                equalization,
                ..Default::default()
            }),
            js_sys::Function::new_no_args(""),
        )
        .await?;
        string_property(&result, "summary", "preview returned no status")
    }

    pub async fn clear_source_preview() -> Result<(), MediaError> {
        dispatch(
            None,
            "clear-preview",
            "",
            "",
            ResizeSpec::DEFAULT,
            None,
            js_sys::Function::new_no_args(""),
        )
        .await?;
        Ok(())
    }

    // Called only at serialized command boundaries after preview submissions
    // drain; the conversion runner owns its own subsequent stage.
    fn clear_preview_state() {
        clear_preview();
        GPU_SESSION.with(|slot| {
            if let Some(gpu) = slot.borrow().as_ref() {
                *gpu.equalization.borrow_mut() = None;
            }
        });
    }

    async fn preview_file(
        file: File,
        key: &str,
        resize: ResizeSpec,
        settings: VideoSettings,
        status: Function,
        failure_mode: &str,
    ) -> Result<JsValue, MediaError> {
        let generation = GENERATION.with(Cell::get);
        let inject_device_loss = settings.equalization.active()
            && failure_mode == "device-loss-once"
            && DEVICE_LOSS_INJECTION_CONSUMED.with(|consumed| {
                if consumed.get() {
                    false
                } else {
                    consumed.set(true);
                    true
                }
            });
        let result = preview_file_inner(
            file,
            key,
            resize,
            settings,
            generation,
            status,
            inject_device_loss,
        )
        .await;
        let result = if GENERATION.with(Cell::get) != generation {
            Err(platform("CANCELLED: preview stopped and drained"))
        } else {
            result
        };
        if result.is_err() {
            clear_preview_state();
            if inject_device_loss
                || GPU_SESSION.with(|slot| {
                    slot.borrow()
                        .as_ref()
                        .is_some_and(|gpu| gpu.device_lost.load(Ordering::Relaxed))
                })
            {
                GPU_SESSION.with(|slot| *slot.borrow_mut() = None);
            }
        }
        result
    }

    async fn preview_file_inner(
        file: File,
        key: &str,
        resize: ResizeSpec,
        settings: VideoSettings,
        generation: u32,
        status: Function,
        inject_device_loss: bool,
    ) -> Result<JsValue, MediaError> {
        let mut parts = key.split('@');
        let source_key = parts
            .next()
            .ok_or_else(|| platform("invalid preview key"))?;
        let seconds: f64 = parts
            .next()
            .unwrap_or("")
            .parse()
            .map_err(|_| platform("invalid preview position"))?;
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(platform("invalid preview position"));
        }
        let playback = parts.next() == Some("1");
        // Active CLAHE samples the same bounded exact preroll as paused preview.
        // The separate audio clock continues; no skipped source statistics are
        // approximated by the old low-rate streaming preview cache.
        let streaming = playback && !settings.equalization.active();
        let inspection =
            JsFuture::from(preview_info(&file, source_key, seconds, streaming).map_err(js_error)?)
                .await
                .map_err(js_error)?;
        let geometry = frame_geometry(&inspection)?;
        let selected = resize.output_size(geometry.display_size())?;
        let output = ResizeSpec::Exact {
            width: selected.width.min(640),
            height: selected.height.min(360),
            preserve_aspect_ratio: true,
        }
        .output_size(selected)?;
        let gpu = configured_gpu(
            geometry.square_pixel,
            output,
            geometry.rotation,
            geometry.flip_horizontal,
            false,
        )
        .await?;
        let history_key = format!(
            "{key}:{}:{}:{}",
            file.size(),
            file.last_modified(),
            resize_command(resize)
        );
        let needs_preroll =
            gpu.prepare_preview_equalization(&history_key, settings.equalization, selected)?;
        let process = process_callback(
            Rc::clone(&gpu),
            inject_device_loss,
            settings.color,
            Some((generation, status)),
        );
        let result = async {
            let mut predecessors = None;
            if needs_preroll {
                let history = JsFuture::from(
                    preview_history(
                        &file,
                        process.as_ref().unchecked_ref(),
                        media_core::equalization::HISTORY_US,
                    )
                    .map_err(js_error)?,
                )
                .await
                .map_err(js_error)?;
                predecessors = Some(u32_property(&history, "processed")?);
                if let Some(effect) = gpu.equalization.borrow_mut().as_mut() {
                    effect.preview_predecessors = predecessors.unwrap_or(0);
                }
            }
            let result = JsFuture::from(preview_render(process.as_ref().unchecked_ref()).map_err(js_error)?)
                .await
                .map_err(js_error)?;
            if settings.equalization.active() {
                let source_summary = string_property(&result, "summary", "preview returned no summary")?;
                let source_summary = if playback {
                    source_summary.replacen("Paused source preview:", "Playing sampled source preview:", 1)
                } else { source_summary };
                let count = gpu.equalization.borrow().as_ref().map_or(0, |e| e.preview_predecessors);
                let history_status = if predecessors.is_some() { "rebuilt" } else { "reused" };
                let summary = format!(
                    "{} CLAHE paused preroll: {count} predecessor frames; 100 ms window; history {history_status} for this request (experimental); statistics/adjustments {}×{}, display {}×{}; device generation {}.",
                    source_summary
                    ,selected.width,selected.height,output.width,output.height,gpu.device_generation
                );
                Reflect::set(&result, &"summary".into(), &summary.into()).map_err(js_error)?;
            }
            Ok::<_, MediaError>(result)
        }
        .await
        .map_err(|error| JsValue::from_str(&error.to_string()));
        let drain = gpu.drain_pending().await;
        match (result, drain) {
            (Ok(result), Ok(())) => Ok(result),
            (result, drain) => {
                *gpu.equalization.borrow_mut() = None;
                Err(platform(format!(
                    "preview/drain failed: {result:?}; {drain:?}"
                )))
            }
        }
    }

    async fn run_m1_local(status: Function) -> Result<JsValue, MediaError> {
        clear_preview_state();
        let generation = begin_generation();
        let gpu = configured_gpu(INPUT_SIZE, OUTPUT_SIZE, Rotation::Deg0, false, true).await?;
        let process = process_callback(Rc::clone(&gpu), false, ColorAdjustments::default(), None);
        let cancelled = cancellation_callback(generation);
        let fixture =
            js_sys::Uint8Array::from(include_bytes!("../../../fixtures/m1-vp8.ivf").as_slice());
        let job_result = JsFuture::from(
            invoke_m1(
                fixture,
                include_str!("../../../fixtures/m1-vp8.json"),
                process.as_ref().unchecked_ref(),
                &status,
                cancelled.as_ref().unchecked_ref(),
            )
            .map_err(js_error)?,
        )
        .await;
        let result = settle_gpu_job(&gpu, job_result).await?;
        let summary = format!(
            "{}\nAdditional VideoFrame→ImageBitmap compatibility conversions: {}.\n{}",
            string_property(&result, "summary", "probe returned no summary")?,
            BITMAP_COPIES.with(Cell::get),
            gpu_telemetry()
        );
        Reflect::set(&result, &"summary".into(), &summary.into()).map_err(js_error)?;
        Ok(result)
    }

    pub async fn convert_m3(
        file_input_id: &str,
        _canvas_id: &str,
        profile: OutputProfileId,
        acceleration: CodecAcceleration,
        resize: ResizeSpec,
        settings: VideoSettings,
        status: Function,
    ) -> Result<ConversionResult, MediaError> {
        let file = selected_file(file_input_id)?;
        let result = dispatch(
            Some(file),
            "convert",
            profile.as_str(),
            acceleration.as_str(),
            resize,
            Some(settings),
            status,
        )
        .await?;
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
        acceleration: CodecAcceleration,
        resize: ResizeSpec,
        settings: Option<VideoSettings>,
        execution_options: ExecutionOptions,
        status: Function,
    ) -> Result<JsValue, MediaError> {
        clear_preview_state();
        let generation = begin_generation();
        validate_input_size(file.size() as u64)?;
        let backend = WebCodecsMediabunnyBackend;
        let inspection = JsFuture::from(backend.inspect(&file).map_err(js_error)?)
            .await
            .map_err(js_error)?;
        let geometry = frame_geometry(&inspection)?;
        let output = resize.output_size(geometry.display_size())?;
        let options = video_options(&inspection, output, settings)?;
        let grid = frame_rate_callback(settings)?;
        let gpu = configured_gpu(
            geometry.square_pixel,
            output,
            geometry.rotation,
            geometry.flip_horizontal,
            true,
        )
        .await?;
        let inject_device_loss = execution_options.failure_mode == "device-loss-once"
            && DEVICE_LOSS_INJECTION_CONSUMED.with(|consumed| {
                if consumed.get() {
                    false
                } else {
                    consumed.set(true);
                    true
                }
            });
        let process = process_callback(
            Rc::clone(&gpu),
            inject_device_loss,
            settings.unwrap_or_default().color,
            None,
        );
        gpu.start_equalization(settings.unwrap_or_default().equalization)?;
        let bitmap_ingress_required = bitmap_ingress_callback();
        let cancelled = cancellation_callback(generation);
        let job_result = JsFuture::from(
            backend
                .run(
                    &file,
                    ConversionRequest {
                        output,
                        profile,
                        acceleration,
                        output_mode: &execution_options.output_mode,
                        verify_output: execution_options.verify_output,
                        failure_mode: &execution_options.failure_mode,
                        options: &options,
                        grid: grid.as_ref().unchecked_ref(),
                    },
                    process.as_ref().unchecked_ref(),
                    bitmap_ingress_required.as_ref().unchecked_ref(),
                    &status,
                    cancelled.as_ref().unchecked_ref(),
                )
                .map_err(js_error)?,
        )
        .await;
        let settled = settle_gpu_job(&gpu, job_result).await;
        if inject_device_loss {
            GPU_SESSION.with(|slot| *slot.borrow_mut() = None);
        }
        let result = settled?;
        let bitmap_copies = BITMAP_COPIES.with(Cell::get);
        let summary = format!(
            "{}\nIngress: {bitmap_copies} additional VideoFrame→ImageBitmap compatibility conversions; browser-internal copies unknown.\n{}",
            string_property(&result, "summary", "conversion returned no summary")?,
            gpu_telemetry()
        );
        Reflect::set(&result, &"summary".into(), &summary.into()).map_err(js_error)?;
        Ok(result)
    }

    pub async fn probe_output_profiles(
        file_input_id: &str,
        resize: ResizeSpec,
        settings: VideoSettings,
    ) -> Result<OutputProfileCapabilities, MediaError> {
        let file = selected_file(file_input_id)?;
        let capabilities = dispatch(
            Some(file),
            "probe",
            "",
            "no-preference",
            resize,
            Some(settings),
            Function::new_no_args(""),
        )
        .await?;
        Ok(OutputProfileCapabilities {
            mp4_supported: bool_property(&capabilities, "mp4Supported")?,
            mp4_reason: string_property(
                &capabilities,
                "mp4Reason",
                "MP4 capability probe returned no reason",
            )?,
            hevc_supported: bool_property(&capabilities, "hevcSupported")?,
            hevc_reason: string_property(
                &capabilities,
                "hevcReason",
                "HEVC capability probe returned no reason",
            )?,
            output_size: Size::new(
                u32_property(&capabilities, "outputWidth")?,
                u32_property(&capabilities, "outputHeight")?,
            )?,
            source: source_metadata(
                &Reflect::get(&capabilities, &"source".into()).map_err(js_error)?,
            )?,
        })
    }

    async fn probe_file(
        file: File,
        resize: ResizeSpec,
        settings: Option<VideoSettings>,
    ) -> Result<JsValue, MediaError> {
        validate_input_size(file.size() as u64)?;
        let backend = WebCodecsMediabunnyBackend;
        let inspection = JsFuture::from(backend.inspect(&file).map_err(js_error)?)
            .await
            .map_err(js_error)?;
        let geometry = frame_geometry(&inspection)?;
        let output = resize.output_size(geometry.display_size())?;
        let options = video_options(&inspection, output, settings)?;
        let capabilities = JsFuture::from(
            backend
                .probe_profiles(&file, output, &options)
                .map_err(js_error)?,
        )
        .await
        .map_err(js_error)?;
        Reflect::set(
            &capabilities,
            &"outputWidth".into(),
            &JsValue::from_f64(f64::from(output.width)),
        )
        .map_err(js_error)?;
        Reflect::set(
            &capabilities,
            &"outputHeight".into(),
            &JsValue::from_f64(f64::from(output.height)),
        )
        .map_err(js_error)?;
        Reflect::set(&capabilities, &"source".into(), &inspection).map_err(js_error)?;
        Ok(capabilities)
    }

    fn settings_from_command(rate: &str, fps: &str) -> Result<VideoSettings, MediaError> {
        let bitrate = match rate {
            "smaller" => VideoBitrate::Smaller,
            "recommended" => VideoBitrate::Recommended,
            "higher" => VideoBitrate::Higher,
            value => VideoBitrate::BitsPerSecond(
                value
                    .strip_prefix("bps=")
                    .ok_or_else(|| platform("invalid bitrate policy"))?
                    .parse()
                    .map_err(|_| platform("invalid bitrate"))?,
            ),
        };
        let frame_rate = fps.parse()?;
        Ok(VideoSettings {
            bitrate,
            frame_rate,
            ..Default::default()
        })
    }

    fn video_options(
        inspection: &JsValue,
        output: Size,
        settings: Option<VideoSettings>,
    ) -> Result<JsValue, MediaError> {
        let source = source_metadata(inspection)?;
        let options = js_sys::Object::new();
        let (brightness, contrast, saturation) = settings.unwrap_or_default().color.values();
        Reflect::set(&options, &"colorAdjustments".into(),
            &format!("brightness={brightness}, contrast={contrast}%, saturation={saturation}% (encoded RGB)").into())
            .map_err(js_error)?;
        for (name, profile) in [
            ("avc", OutputProfileId::Mp4H264Aac),
            ("hevc", OutputProfileId::Mp4H265Aac),
            ("vp8", OutputProfileId::WebmVp8Opus),
        ] {
            let rate = settings
                .map(|settings| source.resolve_bitrate(output, profile, settings))
                .transpose()?
                .unwrap_or(0);
            Reflect::set(&options, &name.into(), &JsValue::from_f64(f64::from(rate)))
                .map_err(js_error)?;
        }
        let (num, den) = match settings.map(|settings| settings.frame_rate) {
            Some(FrameRateSpec::Constant {
                numerator,
                denominator,
            }) => (numerator, denominator),
            _ => (0, 1),
        };
        for (name, value) in [
            ("fpsNumerator", f64::from(num)),
            ("fpsDenominator", f64::from(den)),
            (
                "frameRate",
                if num > 0 {
                    f64::from(num) / f64::from(den)
                } else {
                    source.frame_rate
                },
            ),
        ] {
            Reflect::set(&options, &name.into(), &JsValue::from_f64(value)).map_err(js_error)?;
        }
        Ok(options.into())
    }

    fn safe_integer(value: f64) -> Result<i64, MediaError> {
        if !value.is_finite() || value.fract() != 0.0 || value.abs() > 9_007_199_254_740_991.0 {
            return Err(MediaError::TimestampOverflow);
        }
        Ok(value as i64)
    }

    type FrameRateCallback = Closure<dyn FnMut(String, f64, f64, f64) -> Result<f64, JsValue>>;
    fn frame_rate_callback(
        settings: Option<VideoSettings>,
    ) -> Result<FrameRateCallback, MediaError> {
        let grid = match settings.map(|settings| settings.frame_rate) {
            Some(FrameRateSpec::Constant {
                numerator,
                denominator,
            }) => Some(FrameRateGrid::new(numerator, denominator)?),
            _ => None,
        };
        Ok(Closure::new(
            move |kind: String, value: f64, resolution: f64, origin: f64| {
                let result = (|| -> Result<i64, MediaError> {
                    let grid = grid.ok_or(MediaError::InvalidFrameRate)?;
                    let value = safe_integer(value)?;
                    match kind.as_str() {
                        "index" => grid.index(i128::from(value), 1, false),
                        "time" => grid.timestamp_us(value),
                        "end" => {
                            let resolution = safe_integer(resolution)?;
                            let origin = safe_integer(origin)?;
                            let scaled = i128::from(value)
                                .checked_mul(1_000_000)
                                .and_then(|end| {
                                    i128::from(origin)
                                        .checked_mul(i128::from(resolution))
                                        .and_then(|origin| end.checked_sub(origin))
                                })
                                .ok_or(MediaError::TimestampOverflow)?;
                            grid.index(scaled, i128::from(resolution), true)
                        }
                        _ => Err(platform("invalid frame-grid operation")),
                    }
                })()
                .and_then(|value| {
                    safe_integer(value as f64)?;
                    Ok(value as f64)
                });
                result.map_err(|error| JsValue::from_str(&error.to_string()))
            },
        ))
    }

    fn resize_command(resize: ResizeSpec) -> String {
        match resize {
            ResizeSpec::Original => "original".to_string(),
            ResizeSpec::Percent(percent) => format!("percent:{percent}"),
            ResizeSpec::Exact {
                width,
                height,
                preserve_aspect_ratio,
            } => format!("exact:{width}:{height}:{}", u8::from(preserve_aspect_ratio)),
        }
    }

    fn resize_from_command(command: &str) -> Result<ResizeSpec, MediaError> {
        let fields = command.split(':').collect::<Vec<_>>();
        match fields.as_slice() {
            ["original"] => Ok(ResizeSpec::Original),
            ["percent", percent] => Ok(ResizeSpec::Percent(
                percent
                    .parse()
                    .map_err(|_| platform("resize percentage is invalid"))?,
            )),
            ["exact", width, height, preserve] => Ok(ResizeSpec::Exact {
                width: width
                    .parse()
                    .map_err(|_| platform("exact resize width is invalid"))?,
                height: height
                    .parse()
                    .map_err(|_| platform("exact resize height is invalid"))?,
                preserve_aspect_ratio: match *preserve {
                    "1" => true,
                    "0" => false,
                    _ => return Err(platform("exact resize aspect-ratio flag is invalid")),
                },
            }),
            _ => Err(platform("unknown resize command")),
        }
    }

    fn execution_options_from_command(command: &str) -> Result<ExecutionOptions, MediaError> {
        let mut parts = command.split(':');
        let output_mode = match parts.next() {
            Some(value @ ("auto" | "memory")) => value,
            _ => return Err(platform("invalid output mode")),
        };
        let verify_output = match parts.next() {
            Some("0") => false,
            Some("1") => true,
            _ => return Err(platform("invalid verification mode")),
        };
        let failure_mode = match parts.next() {
            Some(value @ ("none" | "codec-once" | "device-loss-once")) => value,
            _ => return Err(platform("invalid failure injection mode")),
        };
        if parts.next().is_some() {
            return Err(platform("invalid execution options"));
        }
        Ok(ExecutionOptions {
            output_mode: output_mode.to_string(),
            verify_output,
            failure_mode: failure_mode.to_string(),
        })
    }

    fn begin_generation() -> u32 {
        BITMAP_COPIES.with(|value| value.set(0));
        BITMAP_INGRESS_REQUIRED.with(|value| value.set(false));
        PROCESSING_METRICS.with(|value| value.set(ProcessingMetrics::ZERO));
        GENERATION.with(|value| {
            let next = value.get().wrapping_add(1);
            value.set(next);
            next
        })
    }

    fn cancellation_callback(generation: u32) -> Closure<dyn FnMut() -> bool> {
        Closure::new(move || GENERATION.with(|value| value.get() != generation))
    }

    fn bitmap_ingress_callback() -> Closure<dyn FnMut() -> bool> {
        Closure::new(move || BITMAP_INGRESS_REQUIRED.with(Cell::get))
    }

    fn process_callback(
        gpu: Rc<GpuSession>,
        inject_device_loss: bool,
        color: ColorAdjustments,
        preview: Option<(u32, Function)>,
    ) -> Closure<dyn FnMut(JsValue, JsValue, f64, f64) -> Promise> {
        let processed = Rc::new(Cell::new(0_u32));
        Closure::new(
            move |value: JsValue, prepared_bitmap: JsValue, timestamp: f64, duration: f64| {
                let gpu = Rc::clone(&gpu);
                let processed = Rc::clone(&processed);
                let preview = preview.clone();
                future_to_promise(async move {
                    let prepared_bitmap = OwnedBitmap(prepared_bitmap);
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
                    if preview
                        .as_ref()
                        .is_some_and(|(generation, _)| GENERATION.with(Cell::get) != *generation)
                    {
                        frame.close();
                        return Err(JsValue::from_str(
                            "CANCELLED: preview stopped before GPU submission",
                        ));
                    }
                    let count = processed.get() + 1;
                    processed.set(count);
                    if inject_device_loss && count == if preview.is_some() { 2 } else { 5 } {
                        gpu.device.destroy();
                        gpu.device_lost.store(true, Ordering::Relaxed);
                        frame.close();
                        return Err(JsValue::from_str(if preview.is_some() {
                            "INJECTED: WebGPU preview device loss after 1 prepared frame"
                        } else {
                            "INJECTED: WebGPU device loss after 4 completed frames"
                        }));
                    }
                    let output = gpu
                        .process(
                            frame,
                            prepared_bitmap,
                            timestamp as i64,
                            duration as i64,
                            color,
                        )
                        .await?;
                    if let Some((generation, status)) = preview {
                        if count == 1 {
                            let _ = status
                                .call1(&JsValue::NULL, &"Preview first GPU frame prepared".into());
                        }
                        if GENERATION.with(Cell::get) != generation {
                            if let Some(frame) = output.dyn_ref::<VideoFrame>() {
                                frame.close();
                            }
                            return Err(JsValue::from_str(
                                "CANCELLED: preview stopped after GPU submission",
                            ));
                        }
                    }
                    Ok(output)
                })
            },
        )
    }

    async fn configured_gpu(
        input: Size,
        output: Size,
        rotation: Rotation,
        flip_horizontal: bool,
        align_for_encoding: bool,
    ) -> Result<Rc<GpuSession>, MediaError> {
        let existing = GPU_SESSION.with(|slot| slot.borrow().clone());
        let gpu = if let Some(existing) =
            existing.filter(|gpu| !gpu.device_lost.load(Ordering::Relaxed))
        {
            existing
        } else {
            let canvas = if let Some(canvas) = GPU_CANVAS.with(|slot| slot.borrow().clone()) {
                canvas
            } else {
                let document = web_sys::window()
                    .and_then(|w| w.document())
                    .ok_or_else(|| platform("document is unavailable"))?;
                let canvas = document
                    .get_element_by_id("export-canvas")
                    .ok_or_else(|| platform("export canvas was not mounted"))?
                    .dyn_into::<HtmlCanvasElement>()
                    .map_err(|_| platform("export element is not a canvas"))?;
                let canvas = ExportCanvas::Html(canvas);
                GPU_CANVAS.with(|slot| *slot.borrow_mut() = Some(canvas.clone()));
                canvas
            };
            let created = Rc::new(GpuSession::new(canvas, None).await?);
            GPU_SESSION.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&created)));
            created
        };
        gpu.configure(input, output, rotation, flip_horizontal, align_for_encoding)?;
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
        slots: Vec<InputTextureSlot>,
        next_slot: Cell<usize>,
        input_size: Size,
        output_size: Size,
        capture_size: Size,
        rotation: Rotation,
        flip_horizontal: bool,
        transform: wgpu::Buffer,
        device_generation: u32,
    }
    struct InputTextureSlot {
        input: wgpu::Texture,
        uses: Cell<u64>,
        pending: RefCell<Option<PendingSubmission>>,
    }
    struct PendingSubmission {
        completion: futures_channel::oneshot::Receiver<f64>,
        error: Pin<Box<dyn Future<Output = Option<wgpu::Error>>>>,
        _lease: ProcessingLease,
        _decoded: OwnedVideoFrame,
        _bitmap: OwnedBitmap,
    }
    struct GpuSession {
        device_lost: Arc<AtomicBool>,
        equalization: RefCell<Option<BrowserEqualization>>,
        device_generation: u32,
        texture_allocations: Cell<u64>,
        adapter_label: String,
        canvas: ExportCanvas,
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        max_texture_dimension_2d: u32,
        pipeline: ResizePipeline,
        configured: RefCell<Option<ConfiguredResources>>,
    }
    struct OwnedVideoFrame(VideoFrame);
    struct BrowserEqualization {
        stage: EqualizationStage,
        preview_color: Option<PreviewColorStage>,
        settings: Equalization,
        origin: Option<i64>,
        preview_key: Option<String>,
        preview_predecessors: u32,
    }
    #[derive(Clone, Copy)]
    struct ProcessingMetrics {
        equalization_frames: u64,
        equalization_bytes: u64,
        ingress_copies: u64,
        canvas_captures: u64,
        live_leases: u32,
        peak_leases: u32,
        texture_reuses: u64,
        cpu_submission_ms: f64,
        gpu_completion_latency_ms: f64,
        pool_wait_ms: f64,
        final_drain_wait_ms: f64,
    }
    impl ProcessingMetrics {
        const ZERO: Self = Self {
            equalization_frames: 0,
            equalization_bytes: 0,
            ingress_copies: 0,
            canvas_captures: 0,
            live_leases: 0,
            peak_leases: 0,
            texture_reuses: 0,
            cpu_submission_ms: 0.0,
            gpu_completion_latency_ms: 0.0,
            pool_wait_ms: 0.0,
            final_drain_wait_ms: 0.0,
        };
    }
    struct ProcessingLease;
    impl ProcessingLease {
        fn new() -> Result<Self, JsValue> {
            PROCESSING_METRICS.with(|metrics| {
                let mut value = metrics.get();
                if value.live_leases as usize >= GPU_POOL_SIZE {
                    return Err(JsValue::from_str(
                        "GPU input texture pool is full; unretired reuse was rejected",
                    ));
                }
                value.live_leases += 1;
                value.peak_leases = value.peak_leases.max(value.live_leases);
                metrics.set(value);
                Ok(Self)
            })
        }
    }
    impl Drop for ProcessingLease {
        fn drop(&mut self) {
            PROCESSING_METRICS.with(|metrics| {
                let mut value = metrics.get();
                value.live_leases = value.live_leases.saturating_sub(1);
                metrics.set(value);
            });
        }
    }
    #[derive(Clone)]
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

    fn startup_stage(callback: Option<&Function>, stage: &str) {
        if let Some(callback) = callback {
            let _ = callback.call1(&JsValue::NULL, &JsValue::from_str(stage));
        }
    }

    impl GpuSession {
        async fn new(canvas: ExportCanvas, stage: Option<&Function>) -> Result<Self, MediaError> {
            let device_generation = NEXT_DEVICE_GENERATION.with(|generation| {
                let next = generation.get().wrapping_add(1);
                generation.set(next);
                next
            });
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            descriptor.backends = wgpu::Backends::BROWSER_WEBGPU;
            let instance = wgpu::Instance::new(descriptor);
            let surface = instance
                .create_surface(canvas.surface_target())
                .map_err(|error| platform(format!("WebGPU canvas surface failed: {error}")))?;
            startup_stage(stage, "WebGPU adapter request");
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
            let compute_limits = wgpu::Limits::default().using_resolution(adapter.limits());
            let required_limits = if compute_limits.check_limits(&adapter.limits()) {
                compute_limits
            } else {
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits())
            };
            startup_stage(stage, "WebGPU device request");
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("browser video processing device"),
                    required_features: wgpu::Features::empty(),
                    required_limits,
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                    memory_hints: Default::default(),
                    trace: wgpu::Trace::Off,
                })
                .await
                .map_err(|error| platform(format!("WebGPU device request failed: {error}")))?;
            // Identity comes from this processing device, never a second adapter
            // request that could stall or describe a different selected adapter.
            let adapter_label = device
                .as_webgpu()
                .and_then(|device| describe_selected_device(device.as_ref()).ok())
                .filter(|label| !label.trim().is_empty())
                .map(|label| format!("{label} ({:?})", adapter.get_info().backend))
                .unwrap_or(fallback_label);
            startup_stage(stage, "shared pipeline creation");
            let max_texture_dimension_2d = device.limits().max_texture_dimension_2d;
            let capabilities = surface.get_capabilities(&adapter);
            let surface_format = capabilities
                .formats
                .iter()
                .copied()
                .find(|format| format.is_srgb())
                .or_else(|| capabilities.formats.first().copied())
                .ok_or_else(|| platform("canvas reported no texture formats"))?;
            let device_lost = Arc::new(AtomicBool::new(false));
            let loss_flag = Arc::clone(&device_lost);
            device.set_device_lost_callback(move |_, _| loss_flag.store(true, Ordering::Relaxed));
            let pipeline = ResizePipeline::new(&device, surface_format);
            Ok(Self {
                device_lost,
                device_generation,
                equalization: RefCell::new(None),
                texture_allocations: Cell::new(0),
                adapter_label,
                canvas,
                surface,
                device,
                queue,
                surface_format,
                max_texture_dimension_2d,
                pipeline,
                configured: RefCell::new(None),
            })
        }

        fn configure(
            &self,
            input_size: Size,
            output_size: Size,
            rotation: Rotation,
            flip_horizontal: bool,
            align_for_encoding: bool,
        ) -> Result<(), MediaError> {
            // GPU-backed RGB→YUV codec bridges can scale non-aligned backing
            // dimensions. Render only the exact visible viewport into padded
            // backing and crop at VideoFrame capture; never resize the content.
            // Preview stays exact-size. No second pass or CPU readback is added.
            let capture_size = if align_for_encoding {
                Size::new(
                    output_size
                        .width
                        .checked_add(15)
                        .ok_or_else(|| platform("capture width overflow"))?
                        & !15,
                    output_size
                        .height
                        .checked_add(15)
                        .ok_or_else(|| platform("capture height overflow"))?
                        & !15,
                )?
            } else {
                output_size
            };
            for (label, size) in [
                ("input", input_size),
                ("output", output_size),
                ("capture backing", capture_size),
            ] {
                if size.width > self.max_texture_dimension_2d
                    || size.height > self.max_texture_dimension_2d
                {
                    return Err(platform(format!(
                        "{label} dimensions {}×{} exceed the WebGPU device limit {}×{}",
                        size.width,
                        size.height,
                        self.max_texture_dimension_2d,
                        self.max_texture_dimension_2d,
                    )));
                }
            }
            if self.configured.borrow().as_ref().is_some_and(|c| {
                c.input_size == input_size
                    && c.output_size == output_size
                    && c.capture_size == capture_size
                    && c.rotation == rotation
                    && c.flip_horizontal == flip_horizontal
            }) {
                return Ok(());
            }
            if self.configured.borrow().as_ref().is_some_and(|configured| {
                configured
                    .slots
                    .iter()
                    .any(|slot| slot.pending.borrow().is_some())
            }) {
                return Err(platform(
                    "cannot reconfigure GPU textures while submissions are in flight",
                ));
            }
            // All previous submissions have drained before reconfiguration.
            *self.equalization.borrow_mut() = None;
            self.canvas.resize(capture_size);
            self.surface.configure(
                &self.device,
                &wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: self.surface_format,
                    color_space: wgpu::SurfaceColorSpace::Srgb,
                    width: capture_size.width,
                    height: capture_size.height,
                    present_mode: wgpu::PresentMode::Fifo,
                    alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                    view_formats: vec![],
                    desired_maximum_frame_latency: 2,
                },
            );
            let slots = (0..GPU_POOL_SIZE)
                .map(|_| InputTextureSlot {
                    input: self.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("decoded VideoFrame texture pool slot"),
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
                            | wgpu::TextureUsages::RENDER_ATTACHMENT
                            | if preview_diagnostics_enabled() {
                                wgpu::TextureUsages::COPY_SRC
                            } else {
                                wgpu::TextureUsages::empty()
                            },
                        view_formats: &[],
                    }),
                    uses: Cell::new(0),
                    pending: RefCell::new(None),
                })
                .collect();
            self.texture_allocations
                .set(self.texture_allocations.get() + GPU_POOL_SIZE as u64);
            let transform = self.pipeline.create_transform_buffer(
                &self.device,
                &self.queue,
                rotation,
                flip_horizontal,
            );
            *self.configured.borrow_mut() = Some(ConfiguredResources {
                slots,
                next_slot: Cell::new(0),
                input_size,
                output_size,
                capture_size,
                rotation,
                flip_horizontal,
                transform,
                device_generation: self.device_generation,
            });
            Ok(())
        }

        fn start_equalization(&self, settings: Equalization) -> Result<(), MediaError> {
            self.start_equalization_at(settings, None)
        }

        fn start_equalization_at(
            &self,
            settings: Equalization,
            selected: Option<Size>,
        ) -> Result<(), MediaError> {
            *self.equalization.borrow_mut() = None;
            if settings.active() {
                let configured = self.configured.borrow();
                let c = configured
                    .as_ref()
                    .ok_or_else(|| platform("GPU processor is not configured"))?;
                let stage = EqualizationStage::new(
                    &self.device,
                    &self.queue,
                    selected.unwrap_or(c.output_size),
                    c.rotation,
                    c.flip_horizontal,
                    u64::from(self.device_generation),
                )?;
                let preview_color =
                    if let Some(size) = selected.filter(|size| *size != c.output_size) {
                        Some(PreviewColorStage::new(
                            &self.device,
                            size,
                            stage.scratch_bytes(),
                        )?)
                    } else {
                        None
                    };
                PROCESSING_METRICS.with(|metrics| {
                    let mut m = metrics.get();
                    m.equalization_bytes = stage.scratch_bytes()
                        + preview_color.as_ref().map_or(0, PreviewColorStage::bytes);
                    metrics.set(m);
                });
                *self.equalization.borrow_mut() = Some(BrowserEqualization {
                    stage,
                    preview_color,
                    settings,
                    origin: None,
                    preview_key: None,
                    preview_predecessors: 0,
                });
            }
            Ok(())
        }

        fn prepare_preview_equalization(
            &self,
            key: &str,
            settings: Equalization,
            selected: Size,
        ) -> Result<bool, MediaError> {
            if let Some(effect) = self.equalization.borrow_mut().as_mut()
                && effect.preview_key.as_deref() == Some(key)
            {
                // Off/Before temporarily bypasses application without discarding
                // the bounded raw maps. Strength and sliders do not change stats.
                effect.settings = settings;
                return Ok(false);
            }
            self.start_equalization_at(settings, Some(selected))?;
            if let Some(effect) = self.equalization.borrow_mut().as_mut() {
                effect.preview_key = Some(key.to_owned());
            }
            Ok(settings.active())
        }

        async fn process(
            &self,
            decoded: VideoFrame,
            prepared_bitmap: OwnedBitmap,
            timestamp: i64,
            duration: i64,
            color: ColorAdjustments,
        ) -> Result<JsValue, JsValue> {
            let slot_index = {
                let configured = self.configured.borrow();
                let configured = configured
                    .as_ref()
                    .ok_or_else(|| JsValue::from_str("GPU processor is not configured"))?;
                if configured.device_generation != self.device_generation {
                    return Err(JsValue::from_str(
                        "GPU resource belongs to a different device generation",
                    ));
                }
                let index = configured.next_slot.get();
                configured
                    .next_slot
                    .set((index + 1) % configured.slots.len());
                index
            };
            let previous = {
                let configured = self.configured.borrow();
                configured.as_ref().unwrap().slots[slot_index]
                    .pending
                    .borrow_mut()
                    .take()
            };
            if let Some(previous) = previous {
                Self::retire_submission(previous, false).await?;
            }

            let lease = ProcessingLease::new()?;
            let submission_started = performance_now();
            let decoded = OwnedVideoFrame(decoded);
            let has_prepared_bitmap =
                !prepared_bitmap.0.is_null() && !prepared_bitmap.0.is_undefined();
            let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let ingress = (|| {
                let configured = self.configured.borrow();
                let configured = configured
                    .as_ref()
                    .ok_or_else(|| JsValue::from_str("GPU processor is not configured"))?;
                if configured.device_generation != self.device_generation {
                    return Err(JsValue::from_str(
                        "GPU resource belongs to a different device generation",
                    ));
                }
                let slot = &configured.slots[slot_index];
                let uses = slot.uses.get();
                if uses > 0 {
                    PROCESSING_METRICS.with(|metrics| {
                        let mut value = metrics.get();
                        value.texture_reuses += 1;
                        metrics.set(value);
                    });
                }
                slot.uses.set(uses + 1);
                PROCESSING_METRICS.with(|metrics| {
                    let mut value = metrics.get();
                    value.ingress_copies += 1;
                    metrics.set(value);
                });
                copy_decoded_frame(
                    self.queue
                        .as_webgpu()
                        .ok_or_else(|| JsValue::from_str("WebGPU queue unavailable"))?
                        .as_ref(),
                    self.device
                        .as_webgpu()
                        .ok_or_else(|| JsValue::from_str("WebGPU device unavailable"))?
                        .as_ref(),
                    configured.slots[slot_index]
                        .input
                        .as_webgpu()
                        .ok_or_else(|| JsValue::from_str("WebGPU texture unavailable"))?
                        .as_ref(),
                    &decoded.0,
                    &prepared_bitmap.0,
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
                BITMAP_INGRESS_REQUIRED.with(|value| value.set(true));
            }
            let bitmap = if has_prepared_bitmap {
                prepared_bitmap
            } else {
                OwnedBitmap(bitmap)
            };
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
                let source = configured.slots[slot_index]
                    .input
                    .create_view(&Default::default());
                let target = output.texture.create_view(&Default::default());
                let mut encoder =
                    self.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("video resize commands"),
                        });
                let mut effect = self.equalization.borrow_mut();
                let active_effect = effect.as_ref().is_some_and(|e| e.settings.active());
                let separate_display = effect.as_ref().is_some_and(|e| e.preview_color.is_some());
                let adjusted_transform = (!color.is_neutral() || active_effect || separate_display)
                    .then(|| {
                        self.pipeline.create_adjustment_buffer(
                            &self.device,
                            &self.queue,
                            if active_effect {
                                Rotation::Deg0
                            } else {
                                configured.rotation
                            },
                            !active_effect && configured.flip_horizontal,
                            color,
                        )
                    });
                let (rendered_source, preview_color) =
                    if let Some(effect) = effect.as_mut().filter(|e| e.settings.active()) {
                        let origin = *effect.origin.get_or_insert(timestamp);
                        let pts = timestamp
                            .checked_sub(origin)
                            .ok_or_else(|| JsValue::from_str("CLAHE timestamp overflow"))?;
                        let view = effect
                            .stage
                            .record(
                                &self.device,
                                &mut encoder,
                                &source,
                                pts,
                                effect.settings,
                                u64::from(self.device_generation),
                            )
                            .map_err(|e| JsValue::from_str(&e.to_string()))?;
                        PROCESSING_METRICS.with(|metrics| {
                            let mut m = metrics.get();
                            m.equalization_frames += 1;
                            metrics.set(m);
                        });
                        (view, effect.preview_color.as_ref())
                    } else {
                        (
                            &source,
                            effect.as_ref().and_then(|e| e.preview_color.as_ref()),
                        )
                    };
                let displayed_source = if let Some(stage) = preview_color {
                    stage.record(
                        &self.device,
                        &self.queue,
                        &mut encoder,
                        rendered_source,
                        (
                            if active_effect {
                                Rotation::Deg0
                            } else {
                                configured.rotation
                            },
                            !active_effect && configured.flip_horizontal,
                            color,
                        ),
                    )
                } else {
                    rendered_source
                };
                let display_transform = separate_display.then(|| {
                    self.pipeline.create_transform_buffer(
                        &self.device,
                        &self.queue,
                        Rotation::Deg0,
                        false,
                    )
                });
                self.pipeline.record_resize(
                    &self.device,
                    &mut encoder,
                    displayed_source,
                    display_transform
                        .as_ref()
                        .or(adjusted_transform.as_ref())
                        .unwrap_or(&configured.transform),
                    &target,
                    configured.output_size,
                );
                self.queue.submit([encoder.finish()]);
                self.queue.present(output);
                let init = VideoFrameInit::new();
                init.set_timestamp_f64(timestamp as f64);
                init.set_duration_f64(duration as f64);
                set_capture_geometry(
                    &init,
                    configured.output_size.width,
                    configured.output_size.height,
                );
                let frame = self.canvas.capture(&init);
                if frame.is_ok() {
                    PROCESSING_METRICS.with(|metrics| {
                        let mut value = metrics.get();
                        value.canvas_captures += 1;
                        metrics.set(value);
                    });
                }
                frame
            })();
            // Even capture/acquisition errors must retire submitted GPU work before
            // dropping decoded/bitmap guards or allowing the next job to reconfigure.
            let (sender, receiver) = futures_channel::oneshot::channel();
            let completion_wait_started = performance_now();
            PROCESSING_METRICS.with(|metrics| {
                let mut value = metrics.get();
                value.cpu_submission_ms += completion_wait_started - submission_started;
                metrics.set(value);
            });
            self.queue.on_submitted_work_done(move || {
                let _ = sender.send(performance_now() - completion_wait_started);
            });
            let pending = PendingSubmission {
                completion: receiver,
                error: Box::pin(scope.pop()),
                _lease: lease,
                _decoded: decoded,
                _bitmap: bitmap,
            };
            let replaced = {
                let configured = self.configured.borrow();
                configured.as_ref().unwrap().slots[slot_index]
                    .pending
                    .replace(Some(pending))
            };
            if let Some(replaced) = replaced {
                let _ = Self::retire_submission(replaced, false).await;
                if let Ok(frame) = encoded_input {
                    frame.close();
                }
                return Err(JsValue::from_str(
                    "GPU texture slot still held an unretired submission",
                ));
            }
            Ok(encoded_input?.into())
        }

        async fn retire_submission(
            pending: PendingSubmission,
            final_drain: bool,
        ) -> Result<(), JsValue> {
            let wait_started = performance_now();
            let completion = pending.completion.await;
            let validation_error = pending.error.await;
            let waited = performance_now() - wait_started;
            PROCESSING_METRICS.with(|metrics| {
                let mut value = metrics.get();
                if let Ok(completion_latency) = completion.as_ref() {
                    value.gpu_completion_latency_ms += *completion_latency;
                }
                if final_drain {
                    value.final_drain_wait_ms += waited;
                } else {
                    value.pool_wait_ms += waited;
                }
                metrics.set(value);
            });
            if let Some(error) = validation_error {
                return Err(JsValue::from_str(&format!(
                    "WebGPU validation failed: {error}"
                )));
            }
            completion.map_err(|_| {
                JsValue::from_str("GPU completion callback was dropped before signaling")
            })?;
            Ok(())
        }

        async fn drain_pending(&self) -> Result<(), JsValue> {
            let pending = {
                let configured = self.configured.borrow();
                configured
                    .as_ref()
                    .map(|configured| {
                        configured
                            .slots
                            .iter()
                            .filter_map(|slot| slot.pending.borrow_mut().take())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            };
            let mut first_error = None;
            for pending in pending {
                if let Err(error) = Self::retire_submission(pending, true).await
                    && first_error.is_none()
                {
                    first_error = Some(error);
                }
            }
            first_error.map_or(Ok(()), Err)
        }
    }

    async fn settle_gpu_job(
        gpu: &GpuSession,
        job_result: Result<JsValue, JsValue>,
    ) -> Result<JsValue, MediaError> {
        let drain_result = gpu.drain_pending().await;
        *gpu.equalization.borrow_mut() = None;
        match (job_result, drain_result) {
            (Ok(result), Ok(())) => Ok(result),
            (Err(job), Ok(())) => Err(js_error(job)),
            (Ok(_), Err(drain)) => Err(js_error(drain)),
            (Err(job), Err(drain)) => Err(platform(format!(
                "{}; GPU drain also failed: {}",
                js_error(job),
                js_error(drain)
            ))),
        }
    }

    fn gpu_telemetry() -> String {
        let metrics = PROCESSING_METRICS.with(Cell::get);
        let capture_geometry = GPU_SESSION
            .with(|slot| {
                slot.borrow().as_ref().and_then(|gpu| {
                    gpu.configured.borrow().as_ref().map(|configured| {
                        format!(
                            "{}×{} / {}×{}",
                            configured.output_size.width,
                            configured.output_size.height,
                            configured.capture_size.width,
                            configured.capture_size.height
                        )
                    })
                })
            })
            .unwrap_or_else(|| "not configured".to_string());
        let session = GPU_SESSION.with(|slot| {
            slot.borrow().as_ref().map(|gpu| {
                (
                    gpu.device_generation,
                    gpu.texture_allocations.get(),
                    gpu.configured
                        .borrow()
                        .as_ref()
                        .map_or(0, |configured| configured.slots.len()),
                )
            })
        });
        let (generation, allocations, pool_slots) = session.unwrap_or((0, 0, 0));
        let equalization = if metrics.equalization_bytes > 0 {
            format!(
                "\nCLAHE: {} recorded source frames; {} nominal compute passes plus geometry/final renders; {} scratch bytes; history reset/drained at job boundary.",
                metrics.equalization_frames,
                metrics.equalization_frames * 4,
                metrics.equalization_bytes
            )
        } else {
            String::new()
        };
        format!(
            "GPU telemetry: device generation {generation}; bounded input texture pool slots={pool_slots}, lifetime allocations={allocations}, job reuses={}; leases live/peak={}/{}; ingress copies={}; canvas captures={}; capture visible/backing={capture_geometry}; CPU submission/bridge={:.1} ms; cumulative submitted-work completion latency={:.1} ms; slot-reuse wait={:.1} ms; final-drain wait={:.1} ms (completion latencies can overlap and are not pure GPU execution; timestamp queries unavailable/not requested).{equalization}",
            metrics.texture_reuses,
            metrics.live_leases,
            metrics.peak_leases,
            metrics.ingress_copies,
            metrics.canvas_captures,
            metrics.cpu_submission_ms,
            metrics.gpu_completion_latency_ms,
            metrics.pool_wait_ms,
            metrics.final_drain_wait_ms,
        )
    }

    fn frame_geometry(value: &JsValue) -> Result<FrameGeometry, MediaError> {
        let coded = Size::new(
            u32_property(value, "codedWidth")?,
            u32_property(value, "codedHeight")?,
        )?;
        let visible = Rect::new(
            u32_property(value, "visibleX")?,
            u32_property(value, "visibleY")?,
            u32_property(value, "visibleWidth")?,
            u32_property(value, "visibleHeight")?,
        )?;
        let square_pixel = Size::new(
            u32_property(value, "width")?,
            u32_property(value, "height")?,
        )?;
        let rotation = Rotation::from_degrees(u32_property(value, "rotation")?)?;
        let geometry = FrameGeometry::new(
            coded,
            visible,
            square_pixel,
            rotation,
            bool_property(value, "flip")?,
        )?;
        let reported_display = Size::new(
            u32_property(value, "displayWidth")?,
            u32_property(value, "displayHeight")?,
        )?;
        if geometry.display_size() != reported_display {
            return Err(platform(format!(
                "browser display geometry {}x{} does not match normalized geometry {}x{}",
                reported_display.width,
                reported_display.height,
                geometry.display_size().width,
                geometry.display_size().height
            )));
        }
        Ok(geometry)
    }

    fn string_property(value: &JsValue, name: &str, missing: &str) -> Result<String, MediaError> {
        Reflect::get(value, &JsValue::from_str(name))
            .map_err(js_error)?
            .as_string()
            .ok_or_else(|| platform(missing))
    }
    fn optional_string_property(value: &JsValue, name: &str) -> Result<Option<String>, MediaError> {
        let property = Reflect::get(value, &JsValue::from_str(name)).map_err(js_error)?;
        if property.is_null() || property.is_undefined() {
            Ok(None)
        } else {
            property
                .as_string()
                .map(Some)
                .ok_or_else(|| platform(format!("invalid {name}")))
        }
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
    fn u64_property(value: &JsValue, name: &str) -> Result<u64, MediaError> {
        let number = number_property(value, name)?;
        if number >= 0.0 && number.fract() == 0.0 && number <= u64::MAX as f64 {
            Ok(number as u64)
        } else {
            Err(platform(format!("invalid {name}")))
        }
    }

    fn source_metadata(value: &JsValue) -> Result<SourceMetadata, MediaError> {
        Ok(SourceMetadata {
            video_bitrate_bps: Reflect::get(value, &"videoBitrate".into())
                .map_err(js_error)?
                .as_f64()
                .filter(|rate| rate.is_finite() && *rate > 0.0)
                .map(|rate| rate.round() as u64),
            variable_frame_rate: bool_property(value, "variableFrameRate")?,
            video_start_us: safe_integer(number_property(value, "videoStartUs")?)?,
            video_end_us: safe_integer(number_property(value, "videoEndUs")?)?,
            audio_start_us: Reflect::get(value, &"audioStartUs".into())
                .map_err(js_error)?
                .as_f64()
                .map(safe_integer)
                .transpose()?,
            audio_end_us: Reflect::get(value, &"audioEndUs".into())
                .map_err(js_error)?
                .as_f64()
                .map(safe_integer)
                .transpose()?,
            file_name: string_property(value, "name", "input metadata has no filename")?,
            file_size: u64_property(value, "size")?,
            display_size: Size::new(
                u32_property(value, "displayWidth")?,
                u32_property(value, "displayHeight")?,
            )?,
            coded_size: Size::new(
                u32_property(value, "codedWidth")?,
                u32_property(value, "codedHeight")?,
            )?,
            video_codec: string_property(value, "codec", "input metadata has no video codec")?,
            duration_seconds: number_property(value, "duration")?,
            frame_rate: number_property(value, "averagePacketRate")?,
            frame_count: u64_property(value, "packetCount")?,
            audio_codec: optional_string_property(value, "audioCodec")?,
            audio_channels: u32_property(value, "audioChannels")?,
            audio_sample_rate: u32_property(value, "audioSampleRate")?,
        })
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
    ConversionResult, OutputProfileCapabilities, SourceMetadata, cancel, clear_source_preview,
    convert_m3, mute_preview_playback, pause_preview_playback, preview_playback_error,
    preview_playback_playing, preview_playback_position, preview_playback_ready,
    preview_playback_source, preview_source, probe_output_profiles, release_preview_playback,
    run_m1, seek_preview_playback, selected_gpu, setup_ffmpeg_assets, setup_runtime,
    start_preview_playback,
};
#[cfg(not(target_arch = "wasm32"))]
pub fn cancel() {}
#[cfg(not(target_arch = "wasm32"))]
pub fn selected_gpu() -> Option<String> {
    None
}
