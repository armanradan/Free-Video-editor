use media_core::{ColorAdjustments, Rotation, Size, VideoSettings};
use media_gpu::ResizePipeline;

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
#[ignore = "requires a real native wgpu adapter; diagnostic readback is test-only"]
fn gpu_color_pixels_match_reference_on_unorm_and_srgb_targets() {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    println!("Color reference adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let source_bytes: [u8; 16] = [
        20, 80, 220, 255, 128, 128, 128, 255, 255, 0, 0, 255, 0, 255, 64, 255,
    ];
    let extent = wgpu::Extent3d {
        width: 4,
        height: 1,
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
            bytes_per_row: Some(16),
            rows_per_image: Some(1),
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
            ColorAdjustments::new(-100, 200, 200).unwrap(),
            ColorAdjustments::new(100, 200, 0).unwrap(),
            ColorAdjustments::new(0, 0, 100).unwrap(),
        ] {
            let transform =
                pipeline.create_adjustment_buffer(&device, &queue, Rotation::Deg0, false, color);
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("diagnostic pixels"),
                size: 256,
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
                Size::new(4, 1).unwrap(),
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
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
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
                .zip(bytes[..16].as_chunks::<4>().0.iter())
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
