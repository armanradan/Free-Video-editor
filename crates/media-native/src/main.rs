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
