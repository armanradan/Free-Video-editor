//! Independent native decoding/encoding policy. Discovery never grants runtime
//! capability: selected-device probes finish before a partial export is created.
use crate::{
    CancellationToken, CancellationWatch, ChildGuard, NativeResult, NvidiaGpu, SourceInfo,
};
use media_core::{OutputProfileId, Size};
use std::{path::Path, process::Command};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Decoder {
    #[default]
    Software,
    Nvdec,
}
impl Decoder {
    pub fn label(self) -> &'static str {
        match self {
            Self::Software => "software",
            Self::Nvdec => "NVDEC",
        }
    }
}
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct DecodePlan {
    pub decoder: Decoder,
    pub reason: &'static str,
}

pub(crate) fn choose(source: &SourceInfo, nvidia: bool) -> DecodePlan {
    if !nvidia {
        DecodePlan {
            decoder: Decoder::Software,
            reason: "Software codec route selected",
        }
    } else if (matches!(source.codec.as_str(), "h264" | "hevc")
        && matches!(source.pixel_format.as_str(), "yuv420p" | "nv12"))
        || (source.codec == "hevc"
            && matches!(source.pixel_format.as_str(), "yuv420p10le" | "p010le"))
    {
        DecodePlan {
            decoder: Decoder::Nvdec,
            reason: "H.264/HEVC source configuration requires a successful selected-device NVDEC probe",
        }
    } else {
        DecodePlan {
            decoder: Decoder::Software,
            reason: "This source has no evaluated NVDEC path; software decoding is planned independently of NVENC",
        }
    }
}

pub(crate) fn preflight(
    plan: DecodePlan,
    input: &Path,
    source: &SourceInfo,
    output: Size,
    profile: OutputProfileId,
    gpu: &NvidiaGpu,
    cancel: &CancellationToken,
) -> NativeResult<()> {
    if plan.decoder == Decoder::Nvdec {
        let mut command = crate::ffmpeg_base();
        command
            .env("CUDA_VISIBLE_DEVICES", &gpu.uuid)
            .args([
                "-hwaccel",
                "cuda",
                "-hwaccel_device",
                "0",
                "-hwaccel_output_format",
                "cuda",
                "-i",
            ])
            .arg(input)
            .args([
                "-map",
                "0:v:0",
                "-an",
                "-sn",
                "-dn",
                "-frames:v",
                "1",
                "-vf",
                if matches!(source.pixel_format.as_str(), "yuv420p10le" | "p010le") {
                    "hwdownload,format=p010le"
                } else {
                    "hwdownload,format=nv12"
                },
                "-f",
                "null",
                "-",
            ]);
        run(
            &mut command,
            "Selected-device NVDEC preflight failed; no decoder retry",
            cancel,
        )?;
    }
    let mut command = crate::ffmpeg_base();
    command
        .env("CUDA_VISIBLE_DEVICES", &gpu.uuid)
        .args([
            "-init_hw_device",
            "cuda=codec:0",
            "-filter_hw_device",
            "codec",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg(format!(
            "color=black:size={}x{}:rate=25",
            output.width, output.height
        ))
        .args([
            "-an",
            "-frames:v",
            "1",
            "-vf",
            if profile.video_bit_depth() == 10 {
                "format=p010le,hwupload_cuda"
            } else {
                "format=nv12,hwupload_cuda"
            },
            "-c:v",
            if profile.video_bit_depth() == 10 {
                "hevc_nvenc"
            } else {
                "h264_nvenc"
            },
            "-gpu",
            "0",
            "-preset",
            "p4",
            "-bf",
            "0",
            "-f",
            "null",
            "-",
        ]);
    run(
        &mut command,
        "Selected-device NVENC preflight failed; no software encoder substitution",
        cancel,
    )
}
fn run(command: &mut Command, label: &str, cancel: &CancellationToken) -> NativeResult<()> {
    cancel.check()?;
    let mut child = ChildGuard::spawn(command)?;
    let _watch = CancellationWatch::new(cancel, &[&child]);
    let status = child.wait_cancellable(cancel)?;
    if !status.success() || !child.diagnostic().trim().is_empty() {
        return Err(child.failure(label, status).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "real selected NVIDIA GPU: decoder/encoder failure, active hybrid cancel and retry"]
    fn actual_hybrid_preflight_cancel_cleanup_and_retry() {
        use crate::{NativeRunOptions, ProcessingRoute, convert_with_control_inner, partial_path};
        use media_core::ResizeSpec;
        use std::{
            fs,
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
            thread,
            time::{Duration, Instant},
        };
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let directory = std::path::PathBuf::from(
            std::env::var_os("DIAXUS_TEST_HYBRID_DIR").expect("new absolute evidence directory"),
        );
        fs::create_dir(&directory).unwrap();
        let input = directory.join("input.mpg");
        fs::copy(root.join("fixtures/native-mpeg1.mpg"), &input).unwrap();
        let key = std::env::var("DIAXUS_TEST_HYBRID_ADAPTER").expect("selected adapter");
        let gpu = crate::select_nvidia_gpu(Some(&key)).unwrap();
        let token = CancellationToken::default();
        let h264 = crate::probe_source_direct(&root.join("fixtures/m2-h264-aac.mp4")).unwrap();
        let mut absent = gpu.clone();
        absent.uuid = "GPU-00000000-0000-0000-0000-000000000000".into();
        let error = preflight(
            choose(&h264, true),
            &root.join("fixtures/m2-h264-aac.mp4"),
            &h264,
            Size::new(192, 144).unwrap(),
            OutputProfileId::Mp4H264Aac,
            &absent,
            &token,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("NVDEC preflight"), "{error}");
        let mut cases = Vec::new();
        for (label, route) in [
            ("direct", ProcessingRoute::NvidiaFfmpeg),
            ("shared", ProcessingRoute::SharedWgpuNvidia),
        ] {
            let small = directory.join(format!("{label}-small.mp4"));
            let options = NativeRunOptions {
                adapter_key: Some(&key),
                ..Default::default()
            };
            let error = convert_with_control_inner(
                &input,
                &small,
                ResizeSpec::Percent(25),
                OutputProfileId::Mp4H264Aac,
                route,
                options,
                &token,
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains("NVENC preflight"), "{error}");
            assert!(!small.exists() && !partial_path(&small).unwrap().exists());
            let output = directory.join(format!("{label}-cancel.mp4"));
            let partial = partial_path(&output).unwrap();
            let signal = CancellationToken::default();
            let worker_signal = signal.clone();
            let seen = Arc::new(AtomicBool::new(false));
            let worker_seen = seen.clone();
            let done = Arc::new(AtomicBool::new(false));
            let worker_done = done.clone();
            let watcher = thread::spawn(move || {
                let started = Instant::now();
                while !worker_done.load(Ordering::Acquire)
                    && started.elapsed() < Duration::from_secs(30)
                {
                    if fs::metadata(&partial).is_ok_and(|m| m.len() > 0) {
                        worker_seen.store(true, Ordering::Release);
                        worker_signal.cancel();
                        return;
                    }
                    thread::sleep(Duration::from_millis(1));
                }
            });
            let result = convert_with_control_inner(
                &input,
                &output,
                ResizeSpec::Original,
                OutputProfileId::Mp4H264Aac,
                route,
                options,
                &signal,
            );
            done.store(true, Ordering::Release);
            watcher.join().unwrap();
            assert!(
                seen.load(Ordering::Acquire),
                "Did not observe active hybrid output"
            );
            assert!(result.unwrap_err().to_string().contains("cancelled"));
            assert!(!output.exists() && !partial_path(&output).unwrap().exists());
            let retry = directory.join(format!("{label}-retry.mp4"));
            let report = convert_with_control_inner(
                &input,
                &retry,
                ResizeSpec::Original,
                OutputProfileId::Mp4H264Aac,
                route,
                options,
                &token,
            )
            .unwrap();
            assert_eq!(report.video_decode.decoder, Decoder::Software);
            assert_eq!(report.video_encoder, "h264_nvenc");
            assert_eq!(report.codec_gpu_to_cpu_bytes, 0);
            assert_eq!(report.codec_cpu_to_gpu_bytes, 384 * 288 * 3 / 2 * 50);
            let output_info = crate::probe_source_direct(&retry).unwrap();
            assert_eq!(
                output_info
                    .encoder_metadata
                    .as_deref()
                    .unwrap()
                    .split_whitespace()
                    .last(),
                Some("h264_nvenc")
            );
            cases.push(serde_json::json!({"route":label,"nvenc_preflight_failure_no_partial":true,"active_cancel_no_partial":true,"fresh_retry":report}));
        }
        #[cfg(windows)]
        {
            // Only inspect FFmpeg processes referring to this unique test-owned
            // directory. No user path is interpolated into shell source.
            let output=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",
                "@(Get-CimInstance Win32_Process -Filter \"Name='ffmpeg.exe'\" | Where-Object { $_.CommandLine.Contains($env:DIAXUS_TEST_HYBRID_DIR) }).Count"])
                .env("DIAXUS_TEST_HYBRID_DIR",&directory).output().unwrap();
            assert!(output.status.success());
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0");
        }
        fs::write(directory.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","missing_device_nvdec_failed_without_retry":true,"owned_ffmpeg_children_remaining":0,"cases":cases})).unwrap()).unwrap();
    }
    #[test]
    fn legacy_decoder_choice_does_not_forbid_nvidia_encoding() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut source = crate::probe_source_direct(&root.join("m2-h264-aac.mp4")).unwrap();
        for codec in [
            "h264",
            "hevc",
            "mpeg1video",
            "mpeg2video",
            "wmv1",
            "wmv2",
            "wmv3",
            "vc1",
        ] {
            source.codec = codec.into();
            assert_eq!(choose(&source, false).decoder, Decoder::Software);
            assert_eq!(
                choose(&source, true).decoder,
                if matches!(codec, "h264" | "hevc") {
                    Decoder::Nvdec
                } else {
                    Decoder::Software
                }
            );
        }
        source.codec = "h264".into();
        source.pixel_format = "yuv420p10le".into();
        assert_eq!(choose(&source, true).decoder, Decoder::Software);
        source.codec = "hevc".into();
        assert_eq!(choose(&source, true).decoder, Decoder::Nvdec);
    }
}
