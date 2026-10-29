use super::*;
use media_core::ai_upscale::{AiModel, AiScale};

#[test]
fn rtx_incompatible_routes_and_main10_reject_before_source_io() {
    let missing = Path::new("missing-rtx-input.mp4");
    let helper = crate::rtx_inference::RtxHelperConfig {
        executable: "missing-helper.exe".into(),
        sdk_root: "missing-sdk".into(),
        cache_parent: "missing-cache".into(),
        dxgi_index: 1,
        exact_adapter_name: "GPU".into(),
        expected_luid_high: 0,
        expected_luid_low: 1,
    };
    for (color, equalization, profile, route, reason) in [
        (
            ColorAdjustments::new(10, 100, 100).unwrap(),
            Equalization::default(),
            OutputProfileId::Mp4H264Aac,
            ProcessingRoute::DirectFfmpeg,
            "explicitly configured shared-wgpu",
        ),
        (
            ColorAdjustments::default(),
            Equalization::new(true, 50).unwrap(),
            OutputProfileId::Mp4H264Aac,
            ProcessingRoute::NvidiaFfmpeg,
            "explicitly configured shared-wgpu",
        ),
        (
            ColorAdjustments::default(),
            Equalization::default(),
            OutputProfileId::Mp4H265Main10Aac,
            ProcessingRoute::SharedWgpu,
            "cannot preserve 10-bit precision",
        ),
    ] {
        let error = convert_with_control_inner(
            missing,
            missing,
            ResizeSpec::Original,
            profile,
            route,
            NativeRunOptions {
                ai: AiSettings::RtxVideo {
                    configuration: media_core::ai_upscale::RtxConfiguration::Sdk11SdrV1,
                    scale: AiScale::Two,
                },
                rtx_helper: Some(&helper),
                color,
                equalization,
                ..Default::default()
            },
            &CancellationToken::default(),
        )
        .unwrap_err();
        assert!(error.to_string().contains(reason), "{error}");
    }
}

#[test]
fn ai_pilot_direct_routes_reject_before_inspection() {
    let missing = Path::new("missing-ai-input.mp4");
    for route in [ProcessingRoute::DirectFfmpeg, ProcessingRoute::NvidiaFfmpeg] {
        let error = convert_with_control_inner(
            missing,
            missing,
            ResizeSpec::Original,
            OutputProfileId::Mp4H264Aac,
            route,
            NativeRunOptions {
                ai: AiSettings::OnnxRuntime {
                    model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
                    scale: AiScale::Four,
                },
                ..Default::default()
            },
            &CancellationToken::default(),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("explicitly configured shared-wgpu")
        );
    }
}

#[test]
fn ai_packet_endpoint_preserves_b_frame_presentation_tail() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ai-upscale-vfr.mp4");
    let source = probe_source(&path).unwrap();
    assert_eq!(source.video_duration_ticks, Some(3072));
    assert_eq!(
        scan_presentation_duration(&path, &source, &CancellationToken::default()).unwrap(),
        4096
    );
}

#[test]
fn ai_nvidia_codec_route_reports_failed_timing_gate_before_inspection() {
    let missing = Path::new("missing-ai-input.mp4");
    let error = convert_with_control_inner(
        missing,
        missing,
        ResizeSpec::Original,
        OutputProfileId::Mp4H264Aac,
        ProcessingRoute::SharedWgpuNvidia,
        NativeRunOptions {
            ai: AiSettings::OnnxRuntime {
                model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
                scale: AiScale::Two,
            },
            ..Default::default()
        },
        &CancellationToken::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("shifted the first video PTS"));
}

#[test]
#[ignore = "actual configured DirectML helper and NVIDIA wgpu, cancellation/device-loss/retry and pixel reference"]
fn actual_ai_video_pixels_cancel_device_loss_and_retry() {
    let config = crate::ai_inference::tests::actual_config();
    let adapter = enumerate_adapters()
        .into_iter()
        .find(|a| a.name == config.exact_adapter_name && a.backend == "Vulkan")
        .unwrap();
    let token = CancellationToken::default();
    let settings = AiSettings::OnnxRuntime {
        model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
        scale: AiScale::Four,
    };
    let mut inference = ai_inference::NativeAiProcessor::new(
        settings,
        Some(&config),
        Size::new(3, 5).unwrap(),
        12,
        &token,
    )
    .unwrap();
    let input = (0..5)
        .flat_map(|y| {
            (0..3).flat_map(move |x| [(x * 17 + y * 13) as u8, 0, (x * y * 7 + 31) as u8, 255])
        })
        .collect();
    let frame = inference.process_frame(input, 12, &token).unwrap();
    inference.close().unwrap();
    // Independent scalar reference from checked-in unclipped CPU tensors.
    let bytes = include_bytes!("../../../fixtures/ai-upscale/3x5-fp32.npy");
    let offset = 10 + usize::from(u16::from_le_bytes(bytes[8..10].try_into().unwrap()));
    let values = bytes[offset..]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|v| f32::from_le_bytes(*v))
        .collect::<Vec<_>>();
    let color = ColorAdjustments::new(10, 115, 80).unwrap();
    for divisor in [1, 2] {
        let selected = Size::new(12 / divisor, 20 / divisor).unwrap();
        let mut gpu = GpuProcessor::new(frame.size, selected, Some(&adapter.key)).unwrap();
        gpu.prepare_ai_postscale(true);
        gpu.transform = gpu.pipeline.create_adjustment_buffer(
            &gpu.device,
            &gpu.queue,
            Rotation::Deg0,
            false,
            color,
        );
        let mut actual = Vec::new();
        gpu.process(&frame.rgba, 0, &mut actual, &token).unwrap();
        let mut maximum = 0;
        for y in 0..selected.height {
            for x in 0..selected.width {
                let mut rgb = [0.; 3];
                for (channel, value) in rgb.iter_mut().enumerate() {
                    for dy in 0..divisor {
                        for dx in 0..divisor {
                            let index = ((y * divisor + dy) * 12 + x * divisor + dx) as usize;
                            *value += (values[channel * 240 + index].clamp(0., 1.) * 255.).round();
                        }
                    }
                    *value = (*value / (divisor * divisor) as f32).round() / 255.;
                }
                let expected = color.reference_rgb(rgb);
                let index = ((y * selected.width + x) * 4) as usize;
                for (channel, value) in expected.into_iter().enumerate() {
                    maximum = maximum.max(
                        (i32::from(actual[index + channel]) - (value * 255.).round() as i32).abs(),
                    );
                }
                assert_eq!(actual[index + 3], 255);
            }
        }
        assert!(maximum <= 2, "x4/{divisor}: {maximum} LSB");
        println!("native AI selected postscale x4/{divisor} + manual controls: max {maximum} LSB");
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let directory = root.join(format!(
        "tmp/ai-upscale/native-ai-lifecycle-{}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let input = root.join("fixtures/ai-upscale-vfr.mp4");
    let options = NativeRunOptions {
        ai: settings,
        ai_helper: Some(&config),
        adapter_key: Some(&adapter.key),
        equalization: Equalization::new(true, 50).unwrap(),
        color,
        device_generation: 13,
        ..Default::default()
    };
    for name in ["cancel", "loss", "helper-crash", "budget"] {
        let output = directory.join(format!("{name}.mp4"));
        let token = CancellationToken::default();
        let error = convert_with_control_inner(
            &input,
            &output,
            ResizeSpec::Original,
            OutputProfileId::Mp4H264Aac,
            ProcessingRoute::SharedWgpu,
            NativeRunOptions {
                inject_cancel_after_frames: (name == "cancel").then_some(1),
                inject_device_loss_after_frames: (name == "loss").then_some(1),
                inject_ai_helper_crash_after_frames: (name == "helper-crash").then_some(1),
                ai_test_budget_bytes: (name == "budget").then_some(1),
                ..options
            },
            &token,
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains(match name {
                "loss" => "GPU device lost",
                "cancel" => "cancelled",
                "helper-crash" => "ONNX helper",
                "budget" => "AI memory admission",
                _ => unreachable!(),
            }),
            "{error}"
        );
        assert!(!output.exists());
        assert!(!partial_path(&output).unwrap().exists());
        println!("native AI {name}: no published or partial file; {error}");
    }
    let retry = directory.join("retry.mp4");
    let report = convert_with_control_inner(
        &input,
        &retry,
        ResizeSpec::Original,
        OutputProfileId::Mp4H264Aac,
        ProcessingRoute::SharedWgpu,
        NativeRunOptions {
            device_generation: 14,
            ..options
        },
        &CancellationToken::default(),
    )
    .unwrap();
    assert_eq!(report.ai.source_frames, 4);
    assert_eq!(report.output_frame_count, 4);
    println!(
        "native AI video cancel, actual device destruction and helper crash after one processed frame, and preallocation budget rejection: unpublished partials removed; fresh generation 14 retry passed: {}",
        directory.display()
    );
}
