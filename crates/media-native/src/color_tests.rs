use media_core::{ColorAdjustments, Rotation, Size, VideoSettings};
use media_gpu::ResizePipeline;

#[test]
fn main10_fixture_adjusted_preencoder_retains_full_rgb16_precision() {
    use std::process::Command;
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/m4-10bit-sdr.mp4");
    let color = ColorAdjustments::new(10, 80, 60).unwrap();
    // Exercise the actual direct route's geometry/RGB16/adjustment prefix.
    // Stop before the YUV420/codec boundary, whose errors are separate.
    let full = crate::color::direct_filter(color, Size::new(160, 96).unwrap(), 10);
    let prefix = full.split(crate::color::FROM_SRGB).next().unwrap();
    let decode = |filter: &str| {
        let output = Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&fixture)
            .args([
                "-an",
                "-frames:v",
                "1",
                "-vf",
                filter,
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb48le",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.len(), 160 * 96 * 6);
        output.stdout
    };
    let original = decode(&format!("{},format=gbrp16le", crate::color::TO_SRGB));
    let adjusted = decode(&format!("{prefix}format=rgb48le"));
    let mut maximum = 0;
    let mut quantized_maximum = 0;
    let mut precision_samples = 0;
    for (input, output) in original
        .as_chunks::<6>()
        .0
        .iter()
        .zip(adjusted.as_chunks::<6>().0)
    {
        let rgb: [u16; 3] =
            std::array::from_fn(|i| u16::from_le_bytes([input[i * 2], input[i * 2 + 1]]));
        let reference = color.reference_rgb(rgb.map(|v| f32::from(v) / 65535.0));
        let quantized = color.reference_rgb(rgb.map(|v| (f32::from(v) / 257.0).round() / 255.0));
        for (index, actual) in output.as_chunks::<2>().0.iter().enumerate() {
            let actual = i32::from(u16::from_le_bytes(*actual));
            maximum = maximum.max((actual - (reference[index] * 65535.0).round() as i32).abs());
            quantized_maximum =
                quantized_maximum.max((actual - (quantized[index] * 65535.0).round() as i32).abs());
            precision_samples += usize::from(actual % 257 != 0);
        }
    }
    assert!(maximum <= 2, "RGB16 pre-encoder error: {maximum}");
    assert!(
        quantized_maximum > 32,
        "fixture cannot distinguish an 8-bit intermediate"
    );
    assert!(
        precision_samples > 1000,
        "insufficient sub-8-bit adjusted samples"
    );
    eprintln!(
        "Main 10 RGB16 diagnostic: max={maximum}/65535; 8-bit-intermediate comparison max={quantized_maximum}/65535; sub-8-bit samples={precision_samples}"
    );
}

#[test]
fn actual_ffmpeg_rgb16_formula_matches_reference_without_8bit_quantization() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    // Encoded-RGB diagnostic bypasses transfer/matrix conversion to isolate math.
    let input: [[u16; 3]; 4] = [
        [5140, 20560, 56540],
        [32768; 3],
        [65535, 0, 0],
        [10001, 30003, 60007],
    ];
    let bytes: Vec<_> = input
        .iter()
        .flatten()
        .flat_map(|channel| channel.to_le_bytes())
        .collect();
    for color in [
        ColorAdjustments::default(),
        ColorAdjustments::new(10, 150, 50).unwrap(),
        ColorAdjustments::new(0, 100, 0).unwrap(),
        ColorAdjustments::new(-100, 200, 200).unwrap(),
        ColorAdjustments::new(100, 200, 0).unwrap(),
        ColorAdjustments::new(0, 0, 100).unwrap(),
    ] {
        let filter = format!(
            "format=gbrp16le,{},format=rgb48le",
            crate::color::expression_filter(color)
        );
        let mut child = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "rawvideo",
                "-pixel_format",
                "rgb48le",
                "-video_size",
                "4x1",
                "-i",
                "pipe:0",
                "-frames:v",
                "1",
                "-vf",
                &filter,
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb48le",
                "pipe:1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.len(), bytes.len());
        for (input, pixel) in input.iter().zip(output.stdout.as_chunks::<6>().0.iter()) {
            let reference = color.reference_rgb(input.map(|v| f32::from(v) / 65535.0));
            for (actual, expected) in pixel.as_chunks::<2>().0.iter().zip(reference) {
                let actual = u16::from_le_bytes(*actual);
                assert!(
                    (i32::from(actual) - (expected * 65535.0).round() as i32).abs() <= 2,
                    "{color:?}: {actual} vs {expected}"
                );
            }
        }
    }
}

#[test]
fn color_settings_deserialization_validates_and_defaults_legacy_payloads() {
    let legacy: VideoSettings =
        serde_json::from_str(r#"{"bitrate":"Recommended","frame_rate":"Original"}"#).unwrap();
    assert!(legacy.color.is_neutral());
    for color in [
        r#"{"brightness":101,"contrast":100,"saturation":100}"#,
        r#"{"brightness":0,"contrast":201,"saturation":100}"#,
        r#"{"brightness":0,"contrast":100,"saturation":-1}"#,
    ] {
        assert!(serde_json::from_str::<ColorAdjustments>(color).is_err());
    }
    let value = ColorAdjustments::new(-20, 120, 0).unwrap();
    assert_eq!(
        serde_json::from_str::<ColorAdjustments>(&serde_json::to_string(&value).unwrap()).unwrap(),
        value
    );
}

#[test]
fn decoded_preview_and_cpu_preencoder_pixels_share_the_color_contract() {
    use std::process::Command;
    let input =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m2-h264-aac.mp4");
    for position in [0, 500_000] {
        let original =
            crate::preview::decode_frame(&input, position, &crate::CancellationToken::default())
                .unwrap();
        for color in [
            ColorAdjustments::default(),
            ColorAdjustments::new(10, 150, 50).unwrap(),
            ColorAdjustments::new(-15, 70, 130).unwrap(),
            ColorAdjustments::new(0, 100, 0).unwrap(),
        ] {
            // Match position/geometry exactly. RGB16 is the direct route's
            // pre-encoder stage, before YUV subsampling or lossy compression.
            let filter = format!(
                "{},format=gbrp16le,{},format=rgba",
                crate::color::TO_SRGB,
                crate::color::expression_filter(color)
            );
            let output = Command::new("ffmpeg")
                .args([
                    "-v",
                    "error",
                    "-ss",
                    &format!("{:.6}", position as f64 / 1_000_000.0),
                    "-i",
                ])
                .arg(&input)
                .args([
                    "-an",
                    "-frames:v",
                    "1",
                    "-vf",
                    &filter,
                    "-f",
                    "rawvideo",
                    "pipe:1",
                ])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout.len(), original.rgba.len());
            let mut max_error = 0;
            let mut sum = 0_u64;
            for (source, actual) in original
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .zip(output.stdout.as_chunks::<4>().0)
            {
                let expected = color.reference_rgb([
                    source[0] as f32 / 255.0,
                    source[1] as f32 / 255.0,
                    source[2] as f32 / 255.0,
                ]);
                for (actual, expected) in actual[..3].iter().zip(expected) {
                    let error =
                        (i32::from(*actual) - (expected * 255.0).round() as i32).unsigned_abs();
                    max_error = max_error.max(error);
                    sum += u64::from(error);
                }
                assert_eq!(actual[3], 255);
            }
            println!(
                "Decoded CPU/preview position={position} color={color:?} max={max_error} MAE={:.4}",
                sum as f64 / (640.0 * 360.0 * 3.0)
            );
            assert!(
                max_error <= 2,
                "preencoder {color:?} at {position}: max {max_error}"
            );
        }
    }
}

#[test]
#[ignore = "real FFmpeg/wgpu cancellation and device destruction with color enabled"]
fn adjusted_jobs_cancel_lose_device_and_retry_without_partial_output() {
    use crate::{
        CancellationToken, NativeRunOptions, OutputProfileId, ProcessingRoute,
        convert_with_control_inner, partial_path, probe_source,
    };
    use media_core::ResizeSpec;
    use std::{
        fs,
        process::Command,
        thread,
        time::{Duration, Instant},
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let directory = root.join(format!("tmp/color-lifecycle-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let source = root.join("fixtures/m2-h264-aac.mp4");
    let long = directory.join("input.mp4");
    assert!(
        Command::new("ffmpeg")
            .args(["-v", "error", "-n", "-stream_loop", "39", "-i"])
            .arg(&source)
            .args(["-c", "copy"])
            .arg(&long)
            .status()
            .unwrap()
            .success()
    );
    let color = ColorAdjustments::new(10, 150, 50).unwrap();
    let nvidia = crate::enumerate_adapters()
        .into_iter()
        .find(|adapter| adapter.vendor == 0x10de);
    let mut routes = vec![ProcessingRoute::DirectFfmpeg, ProcessingRoute::SharedWgpu];
    if let Some(adapter) = &nvidia {
        println!("Adjusted staged NVIDIA lifecycle adapter: {adapter:?}");
        routes.push(ProcessingRoute::SharedWgpuNvidia);
    } else {
        println!("Staged NVIDIA lifecycle UNTESTED: no NVIDIA adapter");
    }
    for route in routes {
        let cancelled = directory.join(format!("{route:?}-cancelled.mp4"));
        let partial = partial_path(&cancelled).unwrap();
        let token = CancellationToken::default();
        let signal = token.clone();
        let watch = thread::spawn(move || {
            let start = Instant::now();
            while !partial.exists() && start.elapsed() < Duration::from_secs(30) {
                thread::sleep(Duration::from_millis(5));
            }
            let opened = partial.exists();
            signal.cancel();
            opened
        });
        let result = convert_with_control_inner(
            &long,
            &cancelled,
            ResizeSpec::Percent(50),
            OutputProfileId::Mp4H264Aac,
            route,
            NativeRunOptions {
                color,
                adapter_key: nvidia.as_ref().map(|adapter| adapter.key.as_str()),
                ..Default::default()
            },
            &token,
        );
        assert!(watch.join().unwrap(), "no active partial output observed");
        assert!(result.unwrap_err().to_string().contains("cancelled"));
        assert!(!cancelled.exists());
        assert!(!partial_path(&cancelled).unwrap().exists());
        let retry = directory.join(format!("{route:?}-retry.mp4"));
        let report = convert_with_control_inner(
            &source,
            &retry,
            ResizeSpec::Percent(50),
            OutputProfileId::Mp4H264Aac,
            route,
            NativeRunOptions {
                color,
                adapter_key: nvidia.as_ref().map(|adapter| adapter.key.as_str()),
                ..Default::default()
            },
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(report.color, color);
        assert_eq!(report.frames_processed, 60);
        assert_eq!(probe_source(&retry).unwrap().frame_count, 60);
        println!("Adjusted {route:?}: active cancel/cleanup/retry PASS");
    }
    let lost = directory.join("lost.mp4");
    let failure = convert_with_control_inner(
        &source,
        &lost,
        ResizeSpec::Percent(50),
        OutputProfileId::Mp4H264Aac,
        ProcessingRoute::SharedWgpu,
        NativeRunOptions {
            color,
            inject_device_loss_after_frames: Some(2),
            ..Default::default()
        },
        &CancellationToken::default(),
    )
    .unwrap_err()
    .to_string();
    assert!(failure.contains("GPU device lost (Destroyed)"), "{failure}");
    assert!(!lost.exists());
    assert!(!partial_path(&lost).unwrap().exists());
    let retry = directory.join("loss-retry.mp4");
    let report = convert_with_control_inner(
        &source,
        &retry,
        ResizeSpec::Percent(50),
        OutputProfileId::Mp4H264Aac,
        ProcessingRoute::SharedWgpu,
        NativeRunOptions {
            color,
            ..Default::default()
        },
        &CancellationToken::default(),
    )
    .unwrap();
    assert_eq!(report.frames_processed, 60);
    println!(
        "Adjusted device destruction/cleanup/fresh-device retry PASS; evidence {}",
        directory.display()
    );
}

#[test]
#[ignore = "requires a real native wgpu adapter; diagnostic readback is test-only"]
fn gpu_color_pixels_match_reference_on_unorm_and_srgb_targets() {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    println!("Color reference adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    // Non-power-of-two raster dimensions plus sharp boundaries reproduce the
    // browser's extreme-control sampler error; four texels alone missed it.
    let palette = [
        [95, 11, 0, 255],
        [244, 135, 41, 255],
        [96, 17, 0, 255],
        [245, 138, 39, 255],
        [20, 80, 220, 255],
        [128, 128, 128, 255],
        [255, 0, 0, 255],
        [0, 255, 64, 255],
    ];
    let source_bytes: Vec<u8> = (0..640 * 360)
        .flat_map(|pixel| palette[pixel % palette.len()])
        .collect();
    let extent = wgpu::Extent3d {
        width: 640,
        height: 360,
        depth_or_array_layers: 1,
    };
    let texture = |format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("color reference test"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let source = texture(
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &source,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &source_bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(640 * 4),
            rows_per_image: Some(360),
        },
        extent,
    );
    for format in [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ] {
        let target = texture(
            format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let pipeline = ResizePipeline::new(&device, format);
        for color in [
            ColorAdjustments::default(),
            ColorAdjustments::new(0, 100, 0).unwrap(),
            ColorAdjustments::new(10, 150, 50).unwrap(),
            ColorAdjustments::new(-20, 200, 200).unwrap(),
            ColorAdjustments::new(-100, 200, 200).unwrap(),
            ColorAdjustments::new(100, 200, 0).unwrap(),
            ColorAdjustments::new(0, 0, 100).unwrap(),
        ] {
            let transform =
                pipeline.create_adjustment_buffer(&device, &queue, Rotation::Deg0, false, color);
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("diagnostic pixels"),
                size: 640 * 360 * 4,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            pipeline.record_resize(
                &device,
                &mut encoder,
                &source.create_view(&Default::default()),
                &transform,
                &target.create_view(&Default::default()),
                Size::new(640, 360).unwrap(),
            );
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &target,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(640 * 4),
                        rows_per_image: Some(360),
                    },
                },
                extent,
            );
            queue.submit([encoder.finish()]);
            let (sender, receiver) = std::sync::mpsc::channel();
            readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |value| {
                    sender.send(value).unwrap();
                });
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(std::time::Duration::from_secs(30)),
                })
                .unwrap();
            receiver.recv().unwrap().unwrap();
            let bytes = readback.slice(..).get_mapped_range().unwrap();
            for (input, actual) in source_bytes
                .as_chunks::<4>()
                .0
                .iter()
                .zip(bytes[..source_bytes.len()].as_chunks::<4>().0.iter())
            {
                let expected = color.reference_rgb([
                    f32::from(input[0]) / 255.0,
                    f32::from(input[1]) / 255.0,
                    f32::from(input[2]) / 255.0,
                ]);
                for (channel, expected) in actual[..3].iter().zip(expected) {
                    assert!(
                        (i32::from(*channel) - (expected * 255.0).round() as i32).abs() <= 2,
                        "{format:?} {color:?}: {actual:?} vs {expected}"
                    );
                }
                assert_eq!(actual[3], 255);
            }
            drop(bytes);
            readback.unmap();
        }
    }
}
