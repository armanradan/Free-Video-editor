use media_core::{OutputProfileId, ResizeSpec};
use media_native::{
    CancellationToken, ProcessingRoute, convert_with_control, enumerate_adapters,
    load_adapter_preference, save_adapter_preference,
};
use std::path::PathBuf;

fn usage() -> &'static str {
    "native-convert list-gpus\n\
     native-convert save-gpu --adapter-key KEY --preference FILE\n\
     native-convert convert --input FILE --output FILE --route direct|wgpu [--profile mp4-h264-aac] [--resize original|75|50|25|720p|1080p|2k|1440p|4k] [--adapter-key KEY | --preference FILE] [--cancel-after-ms N]\n\
     The output must not already exist. Direct FFmpeg accepts 8-bit, square-pixel, fixed-size, unrotated MP4 input with VFR and non-negative A/V origins; shared wgpu currently requires zero-origin CFR. Both accept one video and at most one audio track."
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
            let input = PathBuf::from(required(&args, "--input")?);
            let output = PathBuf::from(required(&args, "--output")?);
            let route = match required(&args, "--route")?.as_str() {
                "direct" => ProcessingRoute::DirectFfmpeg,
                "wgpu" => ProcessingRoute::SharedWgpu,
                other => return Err(format!("unknown route {other}").into()),
            };
            let resize = resize(&argument(&args, "--resize").unwrap_or_else(|| "original".into()))?;
            let profile = match argument(&args, "--profile")
                .as_deref()
                .unwrap_or("mp4-h264-aac")
            {
                "mp4-h264-aac" => OutputProfileId::Mp4H264Aac,
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
            let report = convert_with_control(
                &input,
                &output,
                resize,
                profile,
                route,
                adapter_key.as_deref(),
                &cancel,
            )?;
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
