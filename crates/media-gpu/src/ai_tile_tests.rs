//! Explicit GPU boundary diagnostic; CPU uploads/readbacks are test-only.
use super::*;

fn read(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Vec<u8> {
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    #[cfg(not(feature = "renderer-abi-26"))]
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        })
        .unwrap();
    #[cfg(feature = "renderer-abi-26")]
    device.poll(wgpu::PollType::Wait).unwrap();
    rx.recv().unwrap().unwrap();
    #[cfg(not(feature = "renderer-abi-26"))]
    let data = buffer.slice(..).get_mapped_range().unwrap().to_vec();
    #[cfg(feature = "renderer-abi-26")]
    let data = buffer.slice(..).get_mapped_range().to_vec();
    buffer.unmap();
    data
}

#[test]
#[ignore = "requires an actual GPU; explicit diagnostic readbacks"]
fn actual_gpu_pack_crop_quantize_and_generation() {
    #[cfg(not(feature = "renderer-abi-26"))]
    let descriptor = {
        let mut d = wgpu::InstanceDescriptor::new_without_display_handle();
        d.backends = wgpu::Backends::VULKAN;
        d
    };
    #[cfg(feature = "renderer-abi-26")]
    let descriptor = wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    };
    #[cfg(not(feature = "renderer-abi-26"))]
    let instance = wgpu::Instance::new(descriptor);
    #[cfg(feature = "renderer-abi-26")]
    let instance = wgpu::Instance::new(&descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .unwrap();
    println!("AI tile diagnostic adapter: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let size = Size::new(257, 259).unwrap();
    let stage = AiTilePipeline::new(&device, size, 17).unwrap();
    let texture = |size: Size, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("AI test-only texture"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage,
            view_formats: &[],
        })
    };
    let source = texture(
        size,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    let output_size = Size::new(size.width * 4, size.height * 4).unwrap();
    let target = texture(
        output_size,
        wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
    );
    let rgb = |x: u32, y: u32, c: u32| ((x * 13 + y * 29 + c * 71) % 256) as u8;
    let pixels = (0..size.height)
        .flat_map(|y| {
            (0..size.width).flat_map(move |x| [rgb(x, y, 0), rgb(x, y, 1), rgb(x, y, 2), 255])
        })
        .collect::<Vec<_>>();
    queue.write_texture(
        source.as_image_copy(),
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size.width * 4),
            rows_per_image: Some(size.height),
        },
        source.size(),
    );
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("AI test-only input readback"),
        size: AiTilePipeline::INPUT_BYTES,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let tiles = pilot_tiles(size, 128).unwrap();
    let mut invalid_encoder = device.create_command_encoder(&Default::default());
    assert!(
        stage
            .record_pack(&device, &mut invalid_encoder, &source, tiles[0], 18)
            .is_err()
    );
    let mut malformed = tiles[0];
    malformed.x1 = u32::MAX;
    assert!(
        stage
            .record_pack(&device, &mut invalid_encoder, &source, malformed, 17)
            .is_err()
    );
    // Reverse tile order exposes stale uniforms and incorrect destination offsets.
    for tile in tiles.iter().rev().copied() {
        let w = tile.x1 - tile.x0;
        let h = tile.y1 - tile.y0;
        let mut encoder = device.create_command_encoder(&Default::default());
        stage
            .record_pack(&device, &mut encoder, &source, tile, 17)
            .unwrap();
        encoder.copy_buffer_to_buffer(stage.input(), 0, &staging, 0, u64::from(w * h * 12));
        queue.submit([encoder.finish()]);
        let packed = read(&device, &staging);
        for c in 0..3 {
            for y in 0..h {
                for x in 0..w {
                    let offset = ((c * h + y) * w + x) as usize * 4;
                    let actual = f32::from_le_bytes(packed[offset..offset + 4].try_into().unwrap());
                    let expected = f32::from(rgb(x + tile.x0, y + tile.y0, c)) / 255.;
                    assert!(
                        (actual - expected).abs() < 1e-7,
                        "pack {x},{y},{c}: {actual}/{expected}"
                    );
                }
            }
        }
        // Synthetic raw model values deliberately exceed [0,1]. This verifies
        // channel order, clipped halo crop and final unorm quantization separately
        // from inference, without duplicating the packing shader as an oracle.
        let values = (0..3)
            .flat_map(|c| {
                (0..h * 4).flat_map(move |y| {
                    (0..w * 4).map(move |x| {
                        (f32::from(rgb(x + tile.x0 * 4, y + tile.y0 * 4, c)) - 32.) / 191.
                    })
                })
            })
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        queue.write_buffer(stage.output(), 0, &values);
        let mut encoder = device.create_command_encoder(&Default::default());
        stage
            .record_unpack(&device, &mut encoder, &target, tile, 17)
            .unwrap();
        queue.submit([encoder.finish()]);
    }
    let stride = (output_size.width * 4).div_ceil(256) * 256;
    let pixels = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("AI test-only texture readback"),
        size: u64::from(stride) * u64::from(output_size.height),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &pixels,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(output_size.height),
            },
        },
        target.size(),
    );
    queue.submit([encoder.finish()]);
    let actual = read(&device, &pixels);
    for y in 0..output_size.height {
        for x in 0..output_size.width {
            for c in 0..4 {
                let expected = if c == 3 {
                    255
                } else {
                    (((f32::from(rgb(x, y, c)) - 32.) / 191.).clamp(0., 1.) * 255.).round() as u8
                };
                let value = actual[(y * stride + x * 4 + c) as usize];
                assert!(
                    value.abs_diff(expected) <= 1,
                    "unpack {x},{y},{c}: {value}/{expected}"
                );
            }
        }
    }
    println!(
        "AI tile diagnostic passed: 9 reverse tiles; exact FP32 pack, <=1 LSB crop/quantization; generation and malformed plans rejected"
    );
}
