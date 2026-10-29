use media_core::equalization::Equalization;
use media_core::{ColorAdjustments, FrameRateSpec, OutputProfileId, ResizeSpec, VideoBitrate};
use media_native::{
    CancellationToken, NativeJob, NativeSession, ProcessingRoute, convert_with_control,
    enumerate_adapters, load_adapter_preference, save_adapter_preference,
};
use std::path::PathBuf;

fn usage() -> &'static str {
    // CLAHE is CLI-only until history-aware preview and controls are integrated.
    "native-convert list-gpus\n\
     native-convert save-gpu --adapter-key KEY --preference FILE\n\
     native-convert import-ai-model --model FILE --cache DIRECTORY\n\
     native-convert ai-model-status --cache DIRECTORY\n\
     native-convert remove-ai-model --cache DIRECTORY\n\
     native-convert ai-runtime-status [--ort-directory DIRECTORY] [--rtx-sdk DIRECTORY]\n\
     native-convert ai-frame-pilot --helper FILE --ort-directory DIRECTORY --cache DIRECTORY --dxgi-index N --adapter-name NAME --width W --height H --rgba FILE --output FILE\n\
     native-convert convert --input FILE --output FILE [--route direct|nvidia|wgpu|wgpu-nvidia] [--profile mp4-h264-aac|mp4-h265-main10-aac] [--resize original|75|50|25|720p|1080p|2k|1440p|4k] [--bitrate smaller|recommended|higher|MBPS] [--fps original|NUM[/DEN]] [--adapter-key KEY | --preference FILE] [--cancel-after-ms N]\n\
     Color: --brightness -100..100 (default 0), --contrast 0..200 (default 100), --saturation 0..200 (default 100). Adjusted direct CPU jobs use 16-bit encoded RGB; shared-wgpu uses RGBA8. Direct NVIDIA non-neutral adjustments are rejected, not CPU-fallback.\n\
     Experimental CLAHE: --equalization-strength 0..100, default off. CLI/shared-wgpu routes only; direct routes reject enabled CLAHE. History-aware UI preview is not connected yet. Zero strength bypasses equalization.\n\
     CLI defaults to software direct FFmpeg. NVIDIA uses CUDA decode/resize and NVENC video. wgpu uses software codecs around GPU resize. wgpu-nvidia uses NVDEC/NVENC around the shared wgpu resize, with explicit CPU-staged pixel transfers. NVIDIA routes never silently fall back to CPU. Audio and full inspection/verification still use CPU. Session inspection can be reused for unchanged files; standalone CLI runs start with an empty cache. All routes preserve tested VFR/non-negative A/V origins and require one video and at most one audio track. NVIDIA and wgpu require square pixels and no display transform. Main 10 is available on direct/NVIDIA only. BT.709 limited SDR or unspecified tags only; HDR, wide color, negative origins and multiple audio tracks are unsupported. Output must not already exist."
}

fn argument(args: &[String], key: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == key)
        .map(|pair| pair[1].clone())
}

fn required(args: &[String], key: &str) -> Result<String, String> {
    argument(args, key).ok_or_else(|| format!("missing {key}"))
}

fn resize(value: &str) -> Result<ResizeSpec, String> {
    match value {
        "original" => Ok(ResizeSpec::Original),
        "75" => Ok(ResizeSpec::Percent(75)),
        "50" => Ok(ResizeSpec::Percent(50)),
        "25" => Ok(ResizeSpec::Percent(25)),
        "720p" => Ok(ResizeSpec::HD_720P),
        "1080p" => Ok(ResizeSpec::FULL_HD_1080P),
        "2k" => Ok(ResizeSpec::DCI_2K),
        "1440p" => Ok(ResizeSpec::QHD_1440P),
        "4k" => Ok(ResizeSpec::UHD_2160P),
        _ => Err(format!("unknown resize preset {value}")),
    }
}

fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        #[cfg(feature = "ai-upscale-pilot")]
        Some("ai-video-pilot" | "rtx-video-pilot" | "bilinear-video-pilot") => {
            use media_core::ai_upscale::{AiModel, AiScale, AiSettings};
            use media_native::ai_inference::OnnxHelperConfig;
            let rtx = args[0] == "rtx-video-pilot";
            let bilinear = args[0] == "bilinear-video-pilot";
            let config = if rtx || bilinear {
                None
            } else {
                Some(OnnxHelperConfig {
                    executable: required(&args, "--helper")?.into(),
                    runtime_directory: required(&args, "--ort-directory")?.into(),
                    model_cache: required(&args, "--cache")?.into(),
                    dxgi_index: required(&args, "--dxgi-index")?.parse()?,
                    exact_adapter_name: required(&args, "--adapter-name")?,
                })
            };
            let rtx_config = if rtx {
                Some(media_native::rtx_inference::RtxHelperConfig {
                    executable: required(&args, "--helper")?.into(),
                    sdk_root: required(&args, "--rtx-sdk")?.into(),
                    cache_parent: required(&args, "--cache")?.into(),
                    dxgi_index: required(&args, "--dxgi-index")?.parse()?,
                    exact_adapter_name: required(&args, "--adapter-name")?,
                    expected_luid_high: required(&args, "--luid-high")?.parse()?,
                    expected_luid_low: required(&args, "--luid-low")?.parse()?,
                })
            } else {
                None
            };
            let input = PathBuf::from(required(&args, "--input")?);
            let output = PathBuf::from(required(&args, "--output")?);
            let scale = match required(&args, "--scale")?.as_str() {
                "2x" => AiScale::Two,
                "4x" => AiScale::Four,
                _ => return Err("Pilot scale must be 2x or 4x".into()),
            };
            let cancel = CancellationToken::default();
            let signal = cancel.clone();
            ctrlc::set_handler(move || signal.cancel())?;
            if let Some(delay) = argument(&args, "--cancel-after-ms") {
                let delay = delay.parse::<u64>()?;
                let signal = cancel.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                    signal.cancel();
                });
            }
            let adapter = required(&args, "--adapter-key")?;
            let route = match argument(&args, "--route").as_deref().unwrap_or("wgpu") {
                "wgpu" => ProcessingRoute::SharedWgpu,
                "wgpu-nvidia" => ProcessingRoute::SharedWgpuNvidia,
                _ => return Err("AI pilot route must be wgpu or wgpu-nvidia".into()),
            };
            let session = NativeSession::new(Some(&adapter))?;
            let job = NativeJob {
                input: &input,
                output: &output,
                resize: ResizeSpec::Original,
                profile: OutputProfileId::Mp4H264Aac,
                route,
                bitrate: Some(VideoBitrate::Recommended),
                frame_rate: argument(&args, "--fps")
                    .as_deref()
                    .unwrap_or("original")
                    .parse()?,
                color: ColorAdjustments::new(
                    argument(&args, "--brightness")
                        .as_deref()
                        .unwrap_or("0")
                        .parse()?,
                    argument(&args, "--contrast")
                        .as_deref()
                        .unwrap_or("100")
                        .parse()?,
                    argument(&args, "--saturation")
                        .as_deref()
                        .unwrap_or("100")
                        .parse()?,
                )?,
                equalization: match argument(&args, "--equalization-strength") {
                    Some(value) => Equalization::new(true, value.parse()?)?,
                    None => Equalization::default(),
                },
                ai: if bilinear {
                    AiSettings::Off
                } else if rtx {
                    AiSettings::RtxVideo {
                        configuration: media_core::ai_upscale::RtxConfiguration::Sdk11SdrV1,
                        scale,
                    }
                } else {
                    AiSettings::OnnxRuntime {
                        model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
                        scale,
                    }
                },
            };
            let report = if bilinear {
                session.convert_bilinear_pilot(job, scale, &cancel, |_| {})?
            } else if let Some(config) = rtx_config.as_ref() {
                session.convert_rtx_pilot(job, config, &cancel, |_| {})?
            } else {
                session.convert_ai_pilot(
                    job,
                    config.as_ref().ok_or("Missing ONNX configuration")?,
                    &cancel,
                    |_| {},
                )?
            };
            println!("{}", serde_json::to_string_pretty(&report.conversion)?);
        }
        Some("ai-frame-pilot") => {
            use media_core::ai_upscale::{AiModel, AiScale, AiSettings, pilot_tiles};
            use media_native::ai_inference::{NativeAiProcessor, OnnxHelperConfig};
            use std::io::{Read, Write};
            let size = media_core::Size::new(
                required(&args, "--width")?.parse()?,
                required(&args, "--height")?.parse()?,
            )?;
            pilot_tiles(size, 128)?;
            let output = PathBuf::from(required(&args, "--output")?);
            if output.exists() {
                return Err("Pilot output already exists".into());
            }
            let mut pixels = Vec::new();
            std::fs::File::open(required(&args, "--rgba")?)?
                .take(u64::from(size.width) * u64::from(size.height) * 4 + 1)
                .read_to_end(&mut pixels)?;
            let config = OnnxHelperConfig {
                executable: required(&args, "--helper")?.into(),
                runtime_directory: required(&args, "--ort-directory")?.into(),
                model_cache: required(&args, "--cache")?.into(),
                dxgi_index: required(&args, "--dxgi-index")?.parse()?,
                exact_adapter_name: required(&args, "--adapter-name")?,
            };
            let cancel = CancellationToken::default();
            let mut processor = NativeAiProcessor::new(
                AiSettings::OnnxRuntime {
                    model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
                    scale: AiScale::Four,
                },
                Some(&config),
                size,
                1,
                &cancel,
            )?;
            let identity = processor.identity().cloned();
            let frame = processor.process_frame(pixels, 1, &cancel)?;
            processor.close()?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)?;
            if let Err(error) = file.write_all(&frame.rgba).and_then(|()| file.sync_all()) {
                drop(file);
                let _ = std::fs::remove_file(&output);
                return Err(error.into());
            }
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"status":"frame-pilot-not-release-accepted","identity":identity,
                "output_width":frame.size.width,"output_height":frame.size.height,"output_bytes":frame.rgba.len(),
                "host_to_gpu_bytes":frame.host_to_gpu_bytes,"gpu_to_host_bytes":frame.gpu_to_host_bytes,
                "sampled_process_local_peak":frame.sampled_process_local_peak,"session_closed":true})
                )?
            );
        }
        Some("ai-runtime-status") => {
            let mut paths = media_native::ai_runtime::RuntimePaths::from_environment();
            if let Some(path) = argument(&args, "--ort-directory") {
                paths.onnx_directory = Some(path.into());
            }
            if let Some(path) = argument(&args, "--rtx-sdk") {
                paths.rtx_sdk = Some(path.into());
            }
            println!("{}", serde_json::to_string_pretty(&paths.inspect())?);
        }
        Some("import-ai-model") => {
            println!("{}", media_core::ai_upscale::ASSET_MANIFEST_JSON);
            let cache = media_native::ai_assets::ModelCache::new(required(&args, "--cache")?);
            let model = PathBuf::from(required(&args, "--model")?);
            let path = cache.import_onnx(&model, &CancellationToken::default())?;
            println!(
                "Verified AI model cache: {} (inference remains gated)",
                path.display()
            );
        }
        Some("ai-model-status") => {
            let cache = media_native::ai_assets::ModelCache::new(required(&args, "--cache")?);
            let bytes = cache.read_onnx(&CancellationToken::default())?;
            println!(
                "Verified cached model: {} bytes; inference remains gated",
                bytes.len()
            );
        }
        Some("remove-ai-model") => {
            let cache = media_native::ai_assets::ModelCache::new(required(&args, "--cache")?);
            println!("Removed cached AI model: {}", cache.remove_onnx()?);
        }
        Some("list-gpus") => {
            println!("{}", serde_json::to_string_pretty(&enumerate_adapters())?);
        }
        Some("save-gpu") => {
            let path = PathBuf::from(required(&args, "--preference")?);
            let saved = save_adapter_preference(&path, &required(&args, "--adapter-key")?)?;
            println!("{}", serde_json::to_string_pretty(&saved)?);
        }
        Some("convert") => {
            if args.iter().any(|arg| arg == "--equalization-strength")
                && argument(&args, "--equalization-strength").is_none()
            {
                return Err("missing value for --equalization-strength".into());
            }
            let input = PathBuf::from(required(&args, "--input")?);
            let output = PathBuf::from(required(&args, "--output")?);
            let route = match argument(&args, "--route").as_deref().unwrap_or("direct") {
                "direct" => ProcessingRoute::DirectFfmpeg,
                "nvidia" => ProcessingRoute::NvidiaFfmpeg,
                "wgpu" => ProcessingRoute::SharedWgpu,
                "wgpu-nvidia" => ProcessingRoute::SharedWgpuNvidia,
                other => return Err(format!("unknown route {other}").into()),
            };
            let resize = resize(&argument(&args, "--resize").unwrap_or_else(|| "original".into()))?;
            let profile = match argument(&args, "--profile")
                .as_deref()
                .unwrap_or("mp4-h264-aac")
            {
                "mp4-h264-aac" => OutputProfileId::Mp4H264Aac,
                "mp4-h265-main10-aac" => OutputProfileId::Mp4H265Main10Aac,
                other => {
                    return Err(format!(
                        "native M4 harness does not yet implement profile {other}"
                    )
                    .into());
                }
            };
            let adapter_key = match argument(&args, "--adapter-key") {
                Some(key) => Some(key),
                None => match argument(&args, "--preference") {
                    Some(path) => {
                        load_adapter_preference(&PathBuf::from(path))?.map(|adapter| adapter.key)
                    }
                    None => None,
                },
            };
            let cancel = CancellationToken::default();
            let signal_token = cancel.clone();
            ctrlc::set_handler(move || signal_token.cancel())?;
            if let Some(delay) = argument(&args, "--cancel-after-ms") {
                let delay = delay.parse::<u64>()?;
                let timer_token = cancel.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                    timer_token.cancel();
                });
            }
            let frame_rate = argument(&args, "--fps")
                .as_deref()
                .unwrap_or("original")
                .parse::<FrameRateSpec>()?;
            let report = if argument(&args, "--bitrate").is_some()
                || frame_rate != FrameRateSpec::Original
                || [
                    "--brightness",
                    "--contrast",
                    "--saturation",
                    "--equalization-strength",
                ]
                .iter()
                .any(|key| argument(&args, key).is_some())
            {
                let bitrate = match argument(&args, "--bitrate").as_deref() {
                    None => None,
                    Some("recommended") => Some(VideoBitrate::Recommended),
                    Some("smaller") => Some(VideoBitrate::Smaller),
                    Some("higher") => Some(VideoBitrate::Higher),
                    Some(value) => Some(VideoBitrate::from_mbps(value)?),
                };
                let session = NativeSession::new(adapter_key.as_deref())?;
                session
                    .convert_job(
                        NativeJob {
                            input: &input,
                            output: &output,
                            resize,
                            profile,
                            route,
                            bitrate,
                            frame_rate,
                            ai: Default::default(),
                            equalization: match argument(&args, "--equalization-strength") {
                                Some(value) => Equalization::new(true, value.parse()?)?,
                                None => Equalization::default(),
                            },
                            color: ColorAdjustments::new(
                                argument(&args, "--brightness")
                                    .unwrap_or_else(|| "0".into())
                                    .parse()?,
                                argument(&args, "--contrast")
                                    .unwrap_or_else(|| "100".into())
                                    .parse()?,
                                argument(&args, "--saturation")
                                    .unwrap_or_else(|| "100".into())
                                    .parse()?,
                            )?,
                        },
                        &cancel,
                        |_| {},
                    )?
                    .conversion
            } else {
                convert_with_control(
                    &input,
                    &output,
                    resize,
                    profile,
                    route,
                    adapter_key.as_deref(),
                    &cancel,
                )?
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        _ => return Err(usage().into()),
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}\n\n{}", usage());
        std::process::exit(1);
    }
}
