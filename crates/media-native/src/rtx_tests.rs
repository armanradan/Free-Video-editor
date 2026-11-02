//! Opt-in actual RTX GPU/image/export/lifecycle gates; no synthetic runtime mocks.
use super::*;
use media_core::ai_upscale::{AiScale, RtxConfiguration};
use std::sync::{Arc, Mutex};

fn config(diagnostic: bool) -> rtx_inference::RtxHelperConfig {
    let path = |name| PathBuf::from(std::env::var_os(name).expect(name));
    rtx_inference::RtxHelperConfig {
        executable: path(if diagnostic {
            "DIAXUS_RTX_DIAGNOSTIC_HELPER"
        } else {
            "DIAXUS_RTX_HELPER"
        }),
        sdk_root: path("DIAXUS_RTX_SDK"),
        cache_parent: path("DIAXUS_RTX_CACHE"),
        dxgi_index: 1,
        exact_adapter_name: "NVIDIA GeForce RTX 3050 Laptop GPU".into(),
        expected_luid_high: std::env::var("DIAXUS_RTX_LUID_HIGH")
            .unwrap()
            .parse()
            .unwrap(),
        expected_luid_low: std::env::var("DIAXUS_RTX_LUID_LOW")
            .unwrap()
            .parse()
            .unwrap(),
    }
}
fn settings(scale: AiScale) -> AiSettings {
    AiSettings::RtxVideo {
        configuration: RtxConfiguration::Sdk11SdrV1,
        scale,
    }
}
fn adapter(helper: &rtx_inference::RtxHelperConfig) -> AdapterDescriptor {
    enumerate_adapters()
        .into_iter()
        .find(|a| a.name == helper.exact_adapter_name && a.backend == "Vulkan")
        .unwrap()
}
fn directory(label: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../tmp/ai-upscale/{label}-{}",
        std::process::id()
    ));
    fs::create_dir(&path).unwrap();
    path
}

#[test]
#[ignore = "actual licensed RTX SDK and NVIDIA GPU; normalized MPEG preview versus chronological conversion pixels"]
fn actual_legacy_sar_preview_matches_normalized_conversion_at_both_scales() {
    use crate::ai_preview::{AiPreview, PreviewHelper, PreviewPolicy};
    let helper = config(false);
    let adapter = adapter(&helper);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/native-mpeg1.mpg");
    let token = CancellationToken::default();
    let source = probe_source_direct(&path).unwrap();
    let input = Size::new(384, 288).unwrap();
    assert_eq!((source.width, source.height), (352, 288));
    let full = Command::new("ffmpeg").args(["-v", "error", "-i"]).arg(&path)
        .args(["-an", "-vf", "scale=384:288:flags=bilinear,setsar=1,zscale=matrixin=smpte170m:transferin=709:primariesin=709:rangein=limited:matrix=gbr:transfer=iec61966-2-1:primaries=709:range=full,format=gbrp,format=rgba", "-frames:v", "3", "-fps_mode", "passthrough", "-f", "rawvideo", "pipe:1"]).output().unwrap();
    assert!(
        full.status.success(),
        "{}",
        String::from_utf8_lossy(&full.stderr)
    );
    let bytes = (input.width * input.height * 4) as usize;
    assert_eq!(full.stdout.len(), bytes * 3);
    let mut cases = vec![];
    for scale in [AiScale::Two, AiScale::Four] {
        let ai = settings(scale);
        let selected = ai
            .requested_output_size(input, ResizeSpec::Original)
            .unwrap();
        let policy = PreviewPolicy {
            ai,
            helper: PreviewHelper::Rtx(helper.clone()),
            processing_adapter: adapter.key.clone(),
            source_end: source.preview_end.clone(),
        };
        let (mut preview, summary) = AiPreview::prepare(&path, 80000, &policy, 11, &token).unwrap();
        assert_eq!((summary.selected_us, summary.predecessors), (80000, 2));
        assert_eq!(preview.selected_size(), selected);
        let image = preview
            .render(
                Equalization::default(),
                ColorAdjustments::default(),
                11,
                &token,
            )
            .unwrap();
        let mut gpu = GpuProcessor::new(selected, selected, Some(&adapter.key)).unwrap();
        gpu.prepare_ai_postscale(true);
        gpu.prepare_preview_display(image.size).unwrap();
        let mut inference = policy.helper.processor(ai, input, 11, &token).unwrap();
        let mut expected = vec![];
        for (frame, pts) in full.stdout.chunks_exact(bytes).zip([0, 40000, 80000]) {
            let enhanced = inference.process_frame(frame.to_vec(), 11, &token).unwrap();
            expected.clear();
            gpu.process(&enhanced.rgba, pts, &mut expected, &token)
                .unwrap();
        }
        inference.close().unwrap();
        assert_eq!(image.rgba.len(), expected.len());
        let maximum = image
            .rgba
            .iter()
            .zip(&expected)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(
            maximum <= 1,
            "Normalized {scale:?} preview differs: {maximum}LSB"
        );
        cases.push(serde_json::json!({"scale":scale.factor(),"normalized_input":[384,288],"selected":[selected.width,selected.height],"selected_us":summary.selected_us,"predecessors":summary.predecessors,"display":[image.size.width,image.size.height],"max_rgba_error_lsb":maximum,"transfers":image.transfers}));
    }
    let output = PathBuf::from(
        std::env::var_os("DIAXUS_TEST_LEGACY_PREVIEW_REPORT").expect("explicit report destination"),
    );
    fs::write(output, serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","scope":"Actual RTX2x/4x normalized MPEG history preview versus independent chronological full-source decode and conversion processor; readbacks are diagnostic only","cases":cases})).unwrap()).unwrap();
}

#[test]
#[ignore = "actual NVIDIA shared-wgpu bilinear pixels versus independent scalar sampling; no helper"]
fn actual_native_bilinear_reference_pixels() {
    let adapter = enumerate_adapters()
        .into_iter()
        .find(|a| a.name == "NVIDIA GeForce RTX 3050 Laptop GPU" && a.backend == "Vulkan")
        .unwrap();
    let input = Size::new(7, 5).unwrap();
    let mut rgba = Vec::new();
    for y in 0..input.height {
        for x in 0..input.width {
            rgba.extend_from_slice(&[
                (x * 37 + y * 11) as u8,
                (x * 13 + y * 41) as u8,
                (x * 29 + y * 17) as u8,
                255,
            ]);
        }
    }
    let mut cases = vec![];
    for factor in [2, 4] {
        let output = Size::new(input.width * factor, input.height * factor).unwrap();
        let mut gpu = GpuProcessor::new(input, output, Some(&adapter.key)).unwrap();
        let mut actual = Vec::new();
        gpu.process(&rgba, 0, &mut actual, &CancellationToken::default())
            .unwrap();
        let mut maximum = 0;
        for y in 0..output.height {
            for x in 0..output.width {
                let sx = (x as f64 + 0.5) / f64::from(factor) - 0.5;
                let sy = (y as f64 + 0.5) / f64::from(factor) - 0.5;
                let (left, top) = (sx.floor(), sy.floor());
                let (fx, fy) = (sx - left, sy - top);
                for channel in 0..4 {
                    let sample = |dx: f64, dy: f64| {
                        let xx = (left + dx).clamp(0.0, f64::from(input.width - 1)) as usize;
                        let yy = (top + dy).clamp(0.0, f64::from(input.height - 1)) as usize;
                        f64::from(rgba[(yy * input.width as usize + xx) * 4 + channel])
                    };
                    let expected = ((1.0 - fy)
                        * ((1.0 - fx) * sample(0.0, 0.0) + fx * sample(1.0, 0.0))
                        + fy * ((1.0 - fx) * sample(0.0, 1.0) + fx * sample(1.0, 1.0)))
                    .round() as u8;
                    maximum = maximum.max(
                        expected.abs_diff(actual[((y * output.width + x) * 4) as usize + channel]),
                    );
                }
            }
        }
        assert!(maximum <= 2, "bilinear scalar gate: {maximum}");
        cases.push(serde_json::json!({"scale":factor,"max_lsb":maximum,"upload_bytes":gpu.uploaded,"readback_bytes":gpu.downloaded}));
    }
    let output = directory("native-bilinear-reference");
    fs::write(output.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","gate_max_lsb":2,"adapter":adapter,"cases":cases})).unwrap()).unwrap();
    println!("Bilinear reference evidence: {}", output.display());
}

#[test]
#[ignore = "actual RTX SDK/helper + native GPU, independent scalar manual transform after selected-size CLAHE"]
fn actual_rtx_effect_order_before_fps_at_selected_geometry() {
    let helper = config(false);
    let adapter = adapter(&helper);
    let cancel = CancellationToken::default();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let input = root.join("fixtures/ai-upscale-vfr.mp4");
    let decoded = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&input)
        .args([
            "-an",
            "-vf",
            &format!("{},format=gbrp,format=rgba", color::TO_SRGB),
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(decoded.status.success());
    assert_eq!(decoded.stdout.len(), 64 * 36 * 4 * 4);
    let output = directory("rtx-effect-order");
    let mut cases = vec![];
    for scale in [AiScale::Two, AiScale::Four] {
        let mut strength_deltas = vec![];
        for strength in [0, 50, 100] {
            let selected = Size::new(64 * scale.factor(), 36 * scale.factor()).unwrap();
            let equalization = Equalization::new(strength != 0, strength).unwrap();
            let color = ColorAdjustments::new(10, 115, 80).unwrap();
            let mut clahe_only = GpuProcessor::new(selected, selected, Some(&adapter.key)).unwrap();
            let mut combined = GpuProcessor::new(selected, selected, Some(&adapter.key)).unwrap();
            for gpu in [&mut clahe_only, &mut combined] {
                if equalization.active() {
                    gpu.equalization = Some(
                        NativeEqualization::new(&gpu.device, &gpu.queue, selected, equalization, 7)
                            .unwrap(),
                    );
                }
            }
            combined.transform = combined.pipeline.create_adjustment_buffer(
                &combined.device,
                &combined.queue,
                Rotation::Deg0,
                false,
                color,
            );
            let mut inference = ai_inference::NativeAiProcessor::new_rtx(
                settings(scale),
                &helper,
                Size::new(64, 36).unwrap(),
                7,
                &cancel,
            )
            .unwrap();
            let mut maximum = 0;
            let mut clahe_delta = 0;
            for (index, frame) in decoded
                .stdout
                .as_chunks::<{ 64 * 36 * 4 }>()
                .0
                .iter()
                .enumerate()
            {
                let enhanced = inference.process_frame(frame.to_vec(), 7, &cancel).unwrap();
                assert_eq!(enhanced.size, selected);
                let pts = [0, 66667, 100000, 233333][index];
                let (mut plain, mut actual) = (Vec::new(), Vec::new());
                clahe_only
                    .process(&enhanced.rgba, pts, &mut plain, &cancel)
                    .unwrap();
                combined
                    .process(&enhanced.rgba, pts, &mut actual, &cancel)
                    .unwrap();
                clahe_delta = clahe_delta.max(
                    plain
                        .iter()
                        .zip(&enhanced.rgba)
                        .map(|(a, b)| a.abs_diff(*b))
                        .max()
                        .unwrap(),
                );
                for (pixel, adjusted) in plain
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(actual.as_chunks::<4>().0)
                {
                    let expected = color
                        .reference_rgb([pixel[0], pixel[1], pixel[2]].map(|v| f32::from(v) / 255.));
                    for channel in 0..3 {
                        maximum = maximum.max(
                            (i32::from(adjusted[channel])
                                - (expected[channel] * 255.).round() as i32)
                                .abs(),
                        );
                    }
                    assert_eq!(adjusted[3], 255);
                }
            }
            inference.close().unwrap();
            assert!(
                maximum <= 2,
                "RTX -> selected-size CLAHE -> scalar manual reference: {maximum} LSB"
            );
            strength_deltas.push(clahe_delta);
            cases.push(serde_json::json!({"scale":scale.factor(),"strength":strength,"source_frames":4,"max_lsb":maximum,"pinned_max_lsb":2,"clahe_max_source_delta":clahe_delta}));
            println!(
                "RTX {}x strength{strength} -> manual scalar reference: max {maximum} LSB; CLAHE/source delta {clahe_delta}",
                scale.factor()
            );
        }
        assert!(
            strength_deltas[0] <= 1
                && strength_deltas[1] > 1
                && strength_deltas[2] > strength_deltas[1],
            "CLAHE was bypassed or strength ignored: {strength_deltas:?}"
        );
    }
    fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","cases":cases})).unwrap(),
    )
    .unwrap();
    println!("Effect-order evidence: {}", output.display());
}

#[test]
#[ignore = "actual RTX SDK, NVDEC/NVENC, nonzero VFR timestamp and transfer regression"]
fn actual_rtx_nvidia_preserves_vfr_offset_and_rejects_small_output_before_helper() {
    let helper = config(false);
    let adapter = adapter(&helper);
    let output = directory("rtx-nvidia-timing");
    let tiny = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ai-upscale-vfr.mp4");
    let fixture = output.join("vfr-96x80.mp4");
    let result = Command::new("ffmpeg")
        .args(["-v", "error", "-n", "-copyts", "-i"])
        .arg(&tiny)
        .args([
            "-vf",
            "scale=96:80:flags=neighbor,setsar=1",
            "-fps_mode",
            "passthrough",
            "-enc_time_base:v",
            "1:90000",
            "-c:v",
            "libx264",
            "-c:a",
            "copy",
        ])
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let cancel = CancellationToken::default();
    let mut reports = vec![];
    for scale in [AiScale::Two, AiScale::Four] {
        let path = output.join(format!("rtx-{}x.mp4", scale.factor()));
        let report = convert_with_control_inner(
            &fixture,
            &path,
            ResizeSpec::Original,
            OutputProfileId::Mp4H264Aac,
            ProcessingRoute::SharedWgpuNvidia,
            NativeRunOptions {
                ai: settings(scale),
                rtx_helper: Some(&helper),
                adapter_key: Some(&adapter.key),
                device_generation: 7,
                ..Default::default()
            },
            &cancel,
        )
        .unwrap();
        assert_eq!(report.video_encoder, "h264_nvenc");
        assert_eq!(report.ai.source_frames, 4);
        assert_eq!(report.output_frame_count, 4);
        assert!(report.hardware_gpu.is_some() && report.adapter_fallback.is_none());
        assert_eq!(report.codec_gpu_to_cpu_bytes, 96 * 80 * 3 / 2 * 4);
        assert_eq!(
            report.codec_cpu_to_gpu_bytes,
            96 * 80 * u64::from(scale.factor()).pow(2) * 3 / 2 * 4
        );
        assert_eq!(probe_source(&path).unwrap().video_start_us, 22000);
        reports.push(report);
    }
    let mut never_started = helper.clone();
    never_started.executable = "never-start-this-helper.exe".into();
    let path = output.join("small-rejected.mp4");
    let error = convert_with_control_inner(
        &tiny,
        &path,
        ResizeSpec::Original,
        OutputProfileId::Mp4H264Aac,
        ProcessingRoute::SharedWgpuNvidia,
        NativeRunOptions {
            ai: settings(AiScale::Two),
            rtx_helper: Some(&never_started),
            adapter_key: Some(&adapter.key),
            device_generation: 8,
            ..Default::default()
        },
        &cancel,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("192x144"), "{error}");
    assert!(!path.exists() && !partial_path(&path).unwrap().exists());
    fs::write(output.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","cases":reports,"small_output_rejected_before_helper":error})).unwrap()).unwrap();
    println!("NVIDIA timing evidence: {}", output.display());
}

#[test]
#[ignore = "actual diagnostic RTX SDK helper deliberately stalls/crashes; native FFmpeg/GPU failure cleanup and retry"]
fn actual_rtx_pipe_cancel_timeout_crash_and_fresh_video_retry() {
    let helper = config(true);
    let healthy = config(false);
    let adapter = adapter(&helper);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let hardware = std::env::var("DIAXUS_TEST_RTX_NVIDIA_CODECS").as_deref() == Ok("1");
    let route = if hardware {
        ProcessingRoute::SharedWgpuNvidia
    } else {
        ProcessingRoute::SharedWgpu
    };
    let output = directory(if hardware {
        "rtx-nvidia-lifecycle"
    } else {
        "rtx-active-lifecycle"
    });
    // Source payload exceeds anonymous-pipe buffering. Derive a short
    // local CC0 fixture, retaining source timestamps/audio, not a raw sequence.
    let fixture = output.join("pipe-fixture.mp4");
    let command = ["-v", "error", "-n", "-copyts", "-i"];
    let result = Command::new("ffmpeg")
        .args(command)
        .arg(root.join("fixtures/ai-upscale-vfr.mp4"))
        .args([
            "-map",
            "0:v:0",
            "-map",
            "0:a:0",
            "-vf",
            if std::env::var("DIAXUS_TEST_RTX_LARGE_LIFECYCLE").as_deref() == Ok("1") {
                "scale=640:360:flags=neighbor"
            } else {
                "scale=192:108:flags=neighbor"
            },
            "-fps_mode",
            "passthrough",
            "-c:v",
            "libx264",
            "-c:a",
            "copy",
        ])
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut cases = vec![];
    for (label, phase, action, cancel, scale) in [
        (
            "cancel-write",
            "before-payload",
            "stall",
            true,
            AiScale::Two,
        ),
        (
            "cancel-read",
            "before-response",
            "stall",
            true,
            AiScale::Four,
        ),
        (
            "timeout-write",
            "before-payload",
            "stall",
            false,
            AiScale::Four,
        ),
        (
            "timeout-read",
            "before-response",
            "stall",
            false,
            AiScale::Two,
        ),
        ("crash", "before-response", "crash", false, AiScale::Four),
    ] {
        let audit = Arc::new(Mutex::new(serde_json::json!({})));
        let destination = output.join(format!("{label}.mp4"));
        let started = Instant::now();
        let scope = rtx_inference::test_fault_scope(rtx_inference::TestFault {
            phase,
            action,
            cancel,
            audit: Arc::clone(&audit),
        });
        let error = convert_with_control_inner(
            &fixture,
            &destination,
            ResizeSpec::Original,
            OutputProfileId::Mp4H264Aac,
            route,
            NativeRunOptions {
                ai: settings(scale),
                rtx_helper: Some(&helper),
                adapter_key: Some(&adapter.key),
                device_generation: 7,
                equalization: Equalization::new(true, 50).unwrap(),
                color: ColorAdjustments::new(10, 115, 80).unwrap(),
                ..Default::default()
            },
            &CancellationToken::default(),
        )
        .unwrap_err()
        .to_string();
        drop(scope);
        assert!(
            error.contains(if cancel {
                "cancelled"
            } else if action == "crash" {
                "RTX helper"
            } else {
                "timed out"
            }),
            "{error}"
        );
        assert!(!destination.exists());
        assert!(!partial_path(&destination).unwrap().exists());
        let mut evidence = audit.lock().unwrap().clone();
        assert_eq!(evidence["reaped"], true);
        assert!(evidence["pid"].as_u64().unwrap() > 0);
        let pid = evidence["pid"].as_u64().unwrap();
        let tasklist = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .unwrap();
        assert!(tasklist.status.success());
        assert!(
            !String::from_utf8_lossy(&tasklist.stdout)
                .lines()
                .any(|line| line.split(',').nth(1) == Some(format!("\"{pid}\"").as_str())),
            "Owned RTX helper PID is still running"
        );
        evidence["os_pid_absent"] = true.into();
        assert!(
            evidence["stderr"]
                .as_str()
                .unwrap()
                .contains("\"stage\":\"test-barrier\""),
            "{evidence}"
        );
        if action != "crash" {
            assert_eq!(
                evidence["observed_io_stage"],
                if phase == "before-payload" {
                    "write-payload"
                } else {
                    "read-response-header"
                }
            );
        }
        let failed_seconds = started.elapsed().as_secs_f64();
        let retry = output.join(format!("{label}-retry.mp4"));
        let report = convert_with_control_inner(
            &fixture,
            &retry,
            ResizeSpec::Original,
            OutputProfileId::Mp4H264Aac,
            route,
            NativeRunOptions {
                ai: settings(scale),
                rtx_helper: Some(&healthy),
                adapter_key: Some(&adapter.key),
                device_generation: 8,
                equalization: Equalization::new(true, 50).unwrap(),
                color: ColorAdjustments::new(10, 115, 80).unwrap(),
                ..Default::default()
            },
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(report.ai.source_frames, 4);
        assert_eq!(report.output_frame_count, 4);
        cases.push(serde_json::json!({"label":label,"error":error,"failed_seconds":failed_seconds,"output_unpublished":true,"audit":evidence,"retry":report}));
        fs::write(
            output.join("report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({"status":"running","cases":cases}))
                .unwrap(),
        )
        .unwrap();
        println!(
            "RTX {label}: observed pipe stage, killed/reaped, no partial, healthy generation8 retry passed"
        );
    }
    for label in ["device-loss", "budget"] {
        let destination = output.join(format!("{label}.mp4"));
        let options = NativeRunOptions {
            ai: settings(AiScale::Four),
            rtx_helper: Some(&healthy),
            adapter_key: Some(&adapter.key),
            device_generation: 9,
            equalization: Equalization::new(true, 50).unwrap(),
            color: ColorAdjustments::new(10, 115, 80).unwrap(),
            ..Default::default()
        };
        let error = convert_with_control_inner(
            &fixture,
            &destination,
            ResizeSpec::Original,
            OutputProfileId::Mp4H264Aac,
            route,
            NativeRunOptions {
                inject_device_loss_after_frames: (label == "device-loss").then_some(1),
                ai_test_budget_bytes: (label == "budget").then_some(1),
                ..options
            },
            &CancellationToken::default(),
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains(if label == "device-loss" {
                "GPU device lost"
            } else {
                "RTX memory admission"
            }),
            "{error}"
        );
        assert!(!destination.exists() && !partial_path(&destination).unwrap().exists());
        let retry = output.join(format!("{label}-retry.mp4"));
        let report = convert_with_control_inner(
            &fixture,
            &retry,
            ResizeSpec::Original,
            OutputProfileId::Mp4H264Aac,
            route,
            NativeRunOptions {
                device_generation: 10,
                ..options
            },
            &CancellationToken::default(),
        )
        .unwrap();
        cases.push(serde_json::json!({"label":label,"error":error,"output_unpublished":true,"retry":report}));
        println!("RTX {label}: no partial, healthy generation10 retry with effects passed");
    }
    fs::write(output.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","fixture_provenance":"CC0 derivative of checked-in ai-upscale-vfr.mp4, FFmpeg neighbor scale, copyts, audio copy","fixture":probe_source(&fixture).unwrap(),"route":format!("{route:?}"),"cases":cases})).unwrap()).unwrap();
    println!("Lifecycle evidence: {}", output.display());
}
