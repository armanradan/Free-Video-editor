//! Independent diagnostic oracle and actual-adapter checks. Never a CPU fallback.
use super::*;

fn luma(rgb: [f32; 3]) -> f32 {
    rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722
}

fn reference_maps(pixels: &[[f32; 4]], size: Size, grid: [u32; 2]) -> Vec<Vec<f32>> {
    let mut counts = vec![vec![0_u32; 1024]; (grid[0] * grid[1]) as usize];
    for (index, pixel) in pixels.iter().enumerate() {
        let x = index as u32 % size.width;
        let y = index as u32 / size.width;
        let tile = y * grid[1] / size.height * grid[0] + x * grid[0] / size.width;
        let bin = (luma([pixel[0], pixel[1], pixel[2]]) * 1023.0 + 0.5).floor() as usize;
        counts[tile as usize][bin] += 1;
    }
    counts
        .into_iter()
        .map(|histogram| {
            let total: u32 = histogram.iter().sum();
            if histogram.iter().filter(|&&n| n != 0).count() <= 1 {
                return (0..1024).map(|n| n as f32 / 1023.0).collect();
            }
            let ceiling = (4 * total).div_ceil(1024).max(1);
            let clipped: Vec<_> = histogram.iter().map(|&n| n.min(ceiling)).collect();
            let overflow = total - clipped.iter().sum::<u32>();
            let redistributed: Vec<_> = clipped
                .iter()
                .enumerate()
                .map(|(index, &n)| {
                    n + overflow / 1024 + u32::from((index as u32) < overflow % 1024)
                })
                .collect();
            assert_eq!(redistributed.iter().sum::<u32>(), total);
            let mut cumulative = 0;
            redistributed
                .into_iter()
                .map(|n| {
                    cumulative += n;
                    (cumulative as f32 - n as f32 / 2.0) / total as f32
                })
                .collect()
        })
        .collect()
}

fn reference_apply(
    pixels: &[[f32; 4]],
    size: Size,
    grid: [u32; 2],
    maps: &[Vec<f32>],
    strength: f32,
) -> Vec<[f32; 4]> {
    pixels
        .iter()
        .enumerate()
        .map(|(index, pixel)| {
            let rgb = [pixel[0], pixel[1], pixel[2]];
            let value = luma(rgb);
            let bin = value * 1023.0;
            let low = bin.floor() as usize;
            let lookup = |x: u32, y: u32| {
                let map = &maps[(y * grid[0] + x) as usize];
                map[low] + (map[(low + 1).min(1023)] - map[low]) * bin.fract()
            };
            let x = ((index as u32 % size.width) as f32 + 0.5) * grid[0] as f32 / size.width as f32
                - 0.5;
            let y = ((index as u32 / size.width) as f32 + 0.5) * grid[1] as f32
                / size.height as f32
                - 0.5;
            let x = x.clamp(0.0, (grid[0] - 1) as f32);
            let y = y.clamp(0.0, (grid[1] - 1) as f32);
            let left = x.floor() as u32;
            let top = y.floor() as u32;
            let right = (left + 1).min(grid[0] - 1);
            let bottom = (top + 1).min(grid[1] - 1);
            let upper = lookup(left, top) * (1.0 - x.fract()) + lookup(right, top) * x.fract();
            let lower =
                lookup(left, bottom) * (1.0 - x.fract()) + lookup(right, bottom) * x.fract();
            let mapped = upper * (1.0 - y.fract()) + lower * y.fract();
            let delta = (mapped - value) * strength;
            [
                (rgb[0] + delta).clamp(0.0, 1.0),
                (rgb[1] + delta).clamp(0.0, 1.0),
                (rgb[2] + delta).clamp(0.0, 1.0),
                1.0,
            ]
        })
        .collect()
}

// Positive normalized half-float decoder, including subnormal values.
fn half(bits: u16) -> f32 {
    assert_eq!(bits & 0x8000, 0);
    let exponent = (bits >> 10) & 31;
    let mantissa = bits & 1023;
    assert!(exponent < 31);
    if exponent == 0 {
        f32::from(mantissa) * 2_f32.powi(-24)
    } else {
        (1.0 + f32::from(mantissa) / 1024.0) * 2_f32.powi(i32::from(exponent) - 15)
    }
}

struct Harness {
    device: wgpu::Device,
    queue: wgpu::Queue,
    source: wgpu::Texture,
    processor: ClaheProcessor,
    size: Size,
}
impl Harness {
    fn new(backends: wgpu::Backends, power_preference: wgpu::PowerPreference) -> Self {
        #[cfg(not(feature = "renderer-abi-26"))]
        let descriptor = {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            descriptor.backends = backends;
            descriptor
        };
        #[cfg(feature = "renderer-abi-26")]
        let descriptor = wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        };
        #[cfg(not(feature = "renderer-abi-26"))]
        let instance = wgpu::Instance::new(descriptor);
        #[cfg(feature = "renderer-abi-26")]
        let instance = wgpu::Instance::new(&descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference,
            ..Default::default()
        }))
        .unwrap();
        println!("CLAHE diagnostic adapter: {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let size = Size::new(130, 129).unwrap(); // uneven tile geometry and padded readback
        let source = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test-only encoded float input"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let processor = ClaheProcessor::new(&device, size, 7).unwrap();
        println!(
            "CLAHE diagnostic scratch bytes: {}",
            processor.storage_bytes()
        );
        Self {
            device,
            queue,
            source,
            processor,
            size,
        }
    }
    fn run(&mut self, input: &[[f32; 4]], pts: i64, strength: u16) -> Vec<[f32; 4]> {
        self.run_adjusted(input, pts, strength, None)
    }
    fn run_adjusted(
        &mut self,
        input: &[[f32; 4]],
        pts: i64,
        strength: u16,
        color: Option<media_core::ColorAdjustments>,
    ) -> Vec<[f32; 4]> {
        let extent = wgpu::Extent3d {
            width: self.size.width,
            height: self.size.height,
            depth_or_array_layers: 1,
        };
        let data: Vec<_> = input
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.source,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.size.width * 16),
                rows_per_image: Some(self.size.height),
            },
            extent,
        );
        let stride = (self.size.width * 8).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test-only CLAHE pixel readback"),
            size: u64::from(stride * self.size.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.processor
            .record(
                &self.device,
                &mut encoder,
                &self.source.create_view(&Default::default()),
                pts,
                Equalization::new(true, strength).unwrap(),
                7,
            )
            .unwrap();
        let adjusted = color.map(|color| {
            let target = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("test-only downstream sliders"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let pipeline =
                crate::ResizePipeline::new(&self.device, wgpu::TextureFormat::Rgba16Float);
            let uniform = pipeline.create_adjustment_buffer(
                &self.device,
                &self.queue,
                media_core::Rotation::Deg0,
                false,
                color,
            );
            pipeline.record_resize(
                &self.device,
                &mut encoder,
                &self.processor.view,
                &uniform,
                &target.create_view(&Default::default()),
                self.size,
            );
            target
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: adjusted.as_ref().unwrap_or(&self.processor._output),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(self.size.height),
                },
            },
            extent,
        );
        self.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                sender.send(result).unwrap()
            });
        #[cfg(not(feature = "renderer-abi-26"))]
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(30)),
            })
            .unwrap();
        #[cfg(feature = "renderer-abi-26")]
        self.device.poll(wgpu::PollType::Wait).unwrap();
        receiver.recv().unwrap().unwrap();
        #[cfg(not(feature = "renderer-abi-26"))]
        let bytes = buffer.slice(..).get_mapped_range().unwrap();
        #[cfg(feature = "renderer-abi-26")]
        let bytes = buffer.slice(..).get_mapped_range();
        bytes
            .chunks_exact(stride as usize)
            .flat_map(|row| {
                row[..(self.size.width * 8) as usize]
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|pixel| {
                        std::array::from_fn(|i| {
                            half(u16::from_le_bytes([pixel[i * 2], pixel[i * 2 + 1]]))
                        })
                    })
            })
            .collect()
    }
}

fn difference(actual: &[[f32; 4]], expected: &[[f32; 4]]) -> f32 {
    actual
        .iter()
        .zip(expected)
        .flat_map(|(a, b)| a.iter().zip(b).map(|(a, b)| (a - b).abs()))
        .fold(0.0, f32::max)
}

#[test]
#[ignore = "requires a real wgpu compute adapter; all readbacks are diagnostic only"]
fn actual_gpu_clahe_reference_identity_history_cut_and_precision() {
    verify(Harness::new(
        wgpu::Backends::VULKAN,
        wgpu::PowerPreference::HighPerformance,
    ));
}

#[test]
#[ignore = "requires a real DX12 compute adapter; all readbacks are diagnostic only"]
fn actual_gpu_clahe_dx12_reference_identity_history_cut_and_precision() {
    verify(Harness::new(
        wgpu::Backends::DX12,
        wgpu::PowerPreference::LowPower,
    ));
}

fn verify(mut h: Harness) {
    let count = (h.size.width * h.size.height) as usize;
    let grid = h.processor.grid;
    for gray in [0.0, 0.001, 0.5, 1.0] {
        h.processor.reset();
        let input = vec![[gray, gray, gray, 1.0]; count];
        let actual = h.run(&input, 0, 100);
        assert!(difference(&actual, &input) <= 0.0005, "flat {gray}");
    }
    let image = |phase: usize| -> Vec<[f32; 4]> {
        (0..count)
            .map(|index| {
                let noise = ((index * 17 + phase * 3) % 7) as f32 * 0.0004;
                let y = 0.25 + ((index * 13) % 1024) as f32 / 4096.0 + noise;
                [y + 0.035, y, y - 0.025, 1.0]
            })
            .collect()
    };
    let first = image(0);
    let maps = reference_maps(&first, h.size, grid);
    assert!(maps.iter().all(|map| map.windows(2).all(|w| w[1] >= w[0])));
    h.processor.reset();
    let actual = h.run(&first, 0, 100);
    let expected = reference_apply(&first, h.size, grid, &maps, 1.0);
    let error = difference(&actual, &expected);
    assert!(error <= 0.0007, "first frame oracle max error {error}");
    println!("CLAHE first-frame float reference max error={error}");
    let half_strength = h.run(&first, 0, 50); // cached statistics, no history advance
    let expected = reference_apply(&first, h.size, grid, &maps, 0.5);
    assert!(difference(&half_strength, &expected) <= 0.0007);
    let mut raw_maps = vec![maps];
    let mut max_temporal_error = 0.0_f32;
    let timestamps = [
        0_i64, 9_000, 33_000, 54_000, 67_000, 80_000, 94_000, 110_000, 140_000,
    ];
    for frame in 1..9 {
        let input = image(frame);
        raw_maps.push(reference_maps(&input, h.size, grid));
        let mut average = vec![vec![0.0; 1024]; (grid[0] * grid[1]) as usize];
        let mut weight_sum = 0.0;
        for (prior, raw_map) in raw_maps
            .iter()
            .enumerate()
            .take(frame + 1)
            .skip(frame.saturating_sub(3))
        {
            let age = timestamps[frame] - timestamps[prior];
            let weight = (100_000 - age) as f32 / 100_000.0;
            weight_sum += weight;
            for (tile, map) in average.iter_mut().zip(raw_map) {
                for (a, b) in tile.iter_mut().zip(map) {
                    *a += b * weight;
                }
            }
        }
        for tile in &mut average {
            for value in tile {
                *value /= weight_sum;
            }
        }
        let actual = h.run(&input, timestamps[frame], 100);
        let expected = reference_apply(&input, h.size, grid, &average, 1.0);
        max_temporal_error = max_temporal_error.max(difference(&actual, &expected));
    }
    assert!(
        max_temporal_error <= 0.0007,
        "history oracle {max_temporal_error}"
    );
    println!("CLAHE VFR 4-slot ring wraps: temporal reference max error={max_temporal_error}");
    let mut combined_error = 0.0_f32;
    for color in [
        media_core::ColorAdjustments::new(10, 150, 50).unwrap(),
        media_core::ColorAdjustments::new(-20, 200, 200).unwrap(),
        media_core::ColorAdjustments::new(0, 0, 0).unwrap(),
        media_core::ColorAdjustments::new(-100, 200, 200).unwrap(),
    ] {
        h.processor.reset();
        let combined = h.run_adjusted(&first, 0, 100, Some(color));
        let expected: Vec<_> = reference_apply(&first, h.size, grid, &raw_maps[0], 1.0)
            .into_iter()
            .map(|p| {
                let rgb = color.reference_rgb([p[0], p[1], p[2]]);
                [rgb[0], rgb[1], rgb[2], 1.0]
            })
            .collect();
        combined_error = combined_error.max(difference(&combined, &expected));
    }
    assert!(
        combined_error <= 2.0 / 255.0,
        "downstream slider order {combined_error}"
    );
    println!("CLAHE → existing slider shader combined max error={combined_error}");
    // Rebuild the preceding temporal sequence before the scene-cut check.
    h.processor.reset();
    for frame in 0..9 {
        h.run(&image(frame), frame as i64 * 17_000, 100);
    }
    let cut: Vec<_> = first
        .iter()
        .map(|p| [p[0] + 0.4, p[1] + 0.4, p[2] + 0.4, 1.0])
        .collect();
    let after_cut = h.run(&cut, 160_000, 100);
    h.processor.reset();
    let isolated = h.run(&cut, 160_000, 100);
    assert_eq!(
        after_cut, isolated,
        "scene cut must not blend previous scene"
    );
    h.processor.reset();
    let repeated = h.run(&first, 0, 100);
    assert_eq!(actual, repeated, "explicit reset must be deterministic");
    let levels: Vec<_> = (0..count)
        .map(|n| {
            let v = 0.4 + (n % 16) as f32 / 1023.0;
            [v, v, v, 1.0]
        })
        .collect();
    h.processor.reset();
    let precise = h.run(&levels, 0, 1);
    let distinct: std::collections::BTreeSet<_> =
        precise.iter().take(16).map(|p| p[0].to_bits()).collect();
    assert_eq!(
        distinct.len(),
        16,
        "10-bit-spaced inputs must not collapse to 8-bit steps"
    );
    // Measure only a fixed patch: histogram noise varies elsewhere, so the
    // measurement isolates mapping flicker from source-pixel variation.
    let noisy = |phase: usize| {
        let mut pixels = image(phase);
        for pixel in &mut pixels {
            for channel in &mut pixel[..3] {
                *channel += (phase % 2) as f32 * 0.004;
            }
        }
        pixels[..100].fill([0.42, 0.42, 0.42, 1.0]);
        pixels
    };
    let mut stabilized = Vec::new();
    let mut isolated = Vec::new();
    for reset_each_frame in [false, true] {
        h.processor.reset();
        for frame in 0..16 {
            if reset_each_frame {
                h.processor.reset();
            }
            let output = h.run(&noisy(frame), frame as i64 * 17_000, 100);
            let mean = output[..100].iter().map(|p| p[0]).sum::<f32>() / 100.0;
            if reset_each_frame {
                isolated.push(mean);
            } else {
                stabilized.push(mean);
            }
        }
    }
    let metric = |values: &[f32]| {
        values[4..]
            .windows(2)
            .map(|p| (p[1] - p[0]).abs())
            .sum::<f32>()
            / 11.0
    };
    let stable_flicker = metric(&stabilized);
    let raw_flicker = metric(&isolated);
    assert!(
        raw_flicker > 0.0001,
        "fixture must exhibit measurable mapping flicker"
    );
    assert!(
        stable_flicker < raw_flicker * 0.75,
        "stabilized {stable_flicker}, isolated {raw_flicker}"
    );
    println!(
        "CLAHE stationary-patch mean absolute temporal delta: stabilized={stable_flicker}, isolated={raw_flicker}, ratio={}",
        stable_flicker / raw_flicker
    );
    let mut encoder = h.device.create_command_encoder(&Default::default());
    let view = h.source.create_view(&Default::default());
    for settings in [Equalization::default(), Equalization::new(true, 0).unwrap()] {
        assert!(
            h.processor
                .record(&h.device, &mut encoder, &view, 0, settings, 7)
                .unwrap()
                .is_none()
        );
    }
    assert!(
        h.processor
            .record(
                &h.device,
                &mut encoder,
                &view,
                0,
                Equalization::new(true, 50).unwrap(),
                8
            )
            .is_err()
    );
    assert!(
        ClaheProcessor::new(
            &h.device,
            Size {
                width: u32::MAX,
                height: u32::MAX
            },
            7
        )
        .is_err()
    );
    assert!(ClaheProcessor::new(&h.device, Size::new(8192, 8192).unwrap(), 7).is_err());
    println!(
        "CLAHE flat/zero/off identity, cache, reset/cut, sub-8-bit diagnostic, generation and scratch rejection PASS"
    );
}
