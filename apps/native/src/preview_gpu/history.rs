//! Paused renderer-device CLAHE adapter. Not run from a UI paint callback:
//! decoding/preparation is blocking and must live in the preview worker.
use media_core::{ColorAdjustments, Rotation, Size, equalization::Equalization};
use media_gpu_blitz::{
    ResizePipeline,
    equalization::{EqualizationStage, PreviewColorStage},
};
use media_native::{
    CancellationToken, NativeResult,
    preview::{HistoryFrame, HistorySummary, stream_history},
};
use std::path::Path;
use wgpu_blitz as w;

/// Owns one current original and finite GPU statistics on one renderer device.
/// Recreate on source/seek/geometry/device changes; rerender parameters in place.
pub struct HistoryPass {
    device: w::Device,
    queue: w::Queue,
    source: w::Texture,
    input: Size,
    equalization: EqualizationStage,
    color: PreviewColorStage,
    display: ResizePipeline,
    generation: u64,
    selected_us: Option<i64>,
    uploaded: u64,
    lost: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl HistoryPass {
    /// Prepares an atomic new cache. Failure/cancel never publishes a partial
    /// history, and submitted work is drained before its owner is discarded.
    pub fn prepare(
        device: &w::Device,
        queue: &w::Queue,
        path: &Path,
        position_us: u64,
        selected: Size,
        generation: u64,
        cancel: &CancellationToken,
    ) -> NativeResult<(Self, HistorySummary)> {
        Self::prepare_checked(
            device,
            queue,
            path,
            position_us,
            selected,
            generation,
            (
                cancel,
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            ),
        )
    }

    pub(crate) fn prepare_checked(
        device: &w::Device,
        queue: &w::Queue,
        path: &Path,
        position_us: u64,
        selected: Size,
        generation: u64,
        health: (
            &CancellationToken,
            std::sync::Arc<std::sync::atomic::AtomicBool>,
        ),
    ) -> NativeResult<(Self, HistorySummary)> {
        let (cancel, lost) = health;
        let mut pass = None;
        let result = stream_history(path, position_us, cancel, |frame| {
            if lost.load(std::sync::atomic::Ordering::Acquire) {
                return Err("renderer device lost; restart renderer".into());
            }
            if pass.is_none() {
                let mut created = Self::new(device, queue, frame.size, selected, generation)?;
                created.lost = lost.clone();
                pass = Some(created);
            }
            pass.as_mut().ok_or("missing preview stage")?.ingest(frame)
        });
        if let Some(pass) = &pass {
            pass.drain()?;
        }
        if cancel.is_cancelled() {
            return Err("native preview cancelled".into());
        }
        let summary = result?;
        Ok((
            pass.ok_or("preview did not decode a source frame")?,
            summary,
        ))
    }

    fn new(
        device: &w::Device,
        queue: &w::Queue,
        input: Size,
        selected: Size,
        generation: u64,
    ) -> NativeResult<Self> {
        if input.width > device.limits().max_texture_dimension_2d
            || input.height > device.limits().max_texture_dimension_2d
        {
            return Err("native preview source exceeds renderer texture limit".into());
        }
        if selected.width > input.width || selected.height > input.height {
            return Err("native preview selected geometry cannot upscale the source".into());
        }
        let equalization =
            EqualizationStage::new(device, queue, selected, Rotation::Deg0, false, generation)?;
        let color = PreviewColorStage::new(device, selected, equalization.scratch_bytes())?;
        let source = device.create_texture(&w::TextureDescriptor {
            label: Some("exact original paused preview"),
            size: w::Extent3d {
                width: input.width,
                height: input.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: w::TextureDimension::D2,
            format: w::TextureFormat::Rgba8Unorm,
            usage: w::TextureUsages::COPY_DST | w::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        Ok(Self {
            device: device.clone(),
            queue: queue.clone(),
            source,
            input,
            equalization,
            color,
            display: ResizePipeline::new(device, w::TextureFormat::Rgba8Unorm),
            generation,
            selected_us: None,
            uploaded: 0,
            lost: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    fn ingest(&mut self, frame: HistoryFrame<'_>) -> NativeResult<()> {
        if frame.size != self.input || self.selected_us.is_some() {
            return Err(
                "native preview ingress changed geometry or continued after selection".into(),
            );
        }
        self.queue.write_texture(
            w::TexelCopyTextureInfo {
                texture: &self.source,
                mip_level: 0,
                origin: w::Origin3d::ZERO,
                aspect: w::TextureAspect::All,
            },
            frame.rgba,
            w::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.input.width * 4),
                rows_per_image: Some(self.input.height),
            },
            self.source.size(),
        );
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.equalization.record(
            &self.device,
            &mut encoder,
            &self.source.create_view(&Default::default()),
            frame.relative_us,
            Equalization::new(true, 50)?,
            self.generation,
        )?;
        self.queue.submit([encoder.finish()]);
        // Bound preview submissions as well as CPU frames; never run this wait
        // on the UI thread. This is correctness-first paused preview, not FPS.
        self.drain()?;
        self.uploaded += 1;
        if frame.selected {
            self.selected_us = Some(frame.relative_us);
        }
        Ok(())
    }

    pub fn uploaded_frames(&self) -> u64 {
        self.uploaded
    }
    pub fn scratch_bytes(&self) -> u64 {
        self.equalization.scratch_bytes() + self.color.bytes()
    }

    /// Strength/sliders and temporary Before bypass reuse original/statistics.
    /// The caller must use a target from this renderer device and generation.
    pub fn render(
        &mut self,
        target: &w::Texture,
        settings: Equalization,
        sliders: ColorAdjustments,
        generation: u64,
    ) -> NativeResult<()> {
        if generation != self.generation {
            return Err("native preview device generation mismatch".into());
        }
        if target.format() != w::TextureFormat::Rgba8Unorm
            || target.width() > 640
            || target.height() > 360
        {
            return Err("native CLAHE preview requires a bounded RGBA8 display target".into());
        }
        let pts = self
            .selected_us
            .ok_or("native preview has no selected frame")?;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let original = self.source.create_view(&Default::default());
        let image = if settings.active() {
            self.equalization.record(
                &self.device,
                &mut encoder,
                &original,
                pts,
                settings,
                self.generation,
            )?
        } else {
            &original
        }; // Do not reset maps for temporary off/zero Before.
        let adjusted = self.color.record(
            &self.device,
            &self.queue,
            &mut encoder,
            image,
            (Rotation::Deg0, false, sliders),
        );
        let neutral =
            self.display
                .create_transform_buffer(&self.device, &self.queue, Rotation::Deg0, false);
        self.display.record_resize(
            &self.device,
            &mut encoder,
            adjusted,
            &neutral,
            &target.create_view(&Default::default()),
            Size::new(target.width(), target.height())?,
        );
        self.queue.submit([encoder.finish()]);
        self.drain()
    }

    fn drain(&self) -> NativeResult<()> {
        if self.lost.load(std::sync::atomic::Ordering::Acquire) {
            return Err("renderer device lost; restart renderer".into());
        }
        let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let callback = finished.clone();
        self.queue.on_submitted_work_done(move || {
            callback.store(true, std::sync::atomic::Ordering::Release);
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !finished.load(std::sync::atomic::Ordering::Acquire) {
            if self.lost.load(std::sync::atomic::Ordering::Acquire) {
                return Err("renderer device lost; restart renderer".into());
            }
            self.device.poll(w::PollType::Poll)?;
            if std::time::Instant::now() >= deadline {
                return Err("native preview GPU completion exceeded 30 second deadline".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::process::Command;

    pub(crate) fn make_target(device: &w::Device, size: Size) -> w::Texture {
        device.create_texture(&w::TextureDescriptor {
            label: Some("test-only native preview raster"),
            size: w::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: w::TextureDimension::D2,
            format: w::TextureFormat::Rgba8Unorm,
            usage: w::TextureUsages::RENDER_ATTACHMENT
                | w::TextureUsages::COPY_SRC
                | w::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    pub(crate) fn readback(device: &w::Device, queue: &w::Queue, texture: &w::Texture) -> Vec<u8> {
        let stride = (texture.width() * 4).div_ceil(256) * 256;
        let buffer = device.create_buffer(&w::BufferDescriptor {
            label: Some("test-only preview readback"),
            size: u64::from(stride) * u64::from(texture.height()),
            usage: w::BufferUsages::MAP_READ | w::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            w::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: w::Origin3d::ZERO,
                aspect: w::TextureAspect::All,
            },
            w::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: w::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(texture.height()),
                },
            },
            texture.size(),
        );
        queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(w::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device.poll(w::PollType::Wait).unwrap();
        receiver.recv().unwrap().unwrap();
        let mapped = buffer.slice(..).get_mapped_range();
        let pixels = mapped
            .chunks(stride as usize)
            .flat_map(|row| row[..texture.width() as usize * 4].iter().copied())
            .collect();
        drop(mapped);
        buffer.unmap();
        pixels
    }

    /// Full chronological processing-wgpu-30 export stage, independently of
    /// seek/preroll/cache and renderer-wgpu-26. Diagnostic readbacks only.
    fn conversion_reference(
        frames: &[u8],
        pts: &[i64],
        selected: Size,
        policy: Equalization,
        sliders: ColorAdjustments,
    ) -> Vec<Vec<u8>> {
        use wgpu as p;
        let instance = p::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        println!("Conversion parity adapter: {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let texture = |size: Size, usage| {
            device.create_texture(&p::TextureDescriptor {
                label: Some("test-only conversion parity"),
                size: p::Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: p::TextureDimension::D2,
                format: p::TextureFormat::Rgba8Unorm,
                usage,
                view_formats: &[],
            })
        };
        let input = texture(
            Size::new(320, 180).unwrap(),
            p::TextureUsages::COPY_DST | p::TextureUsages::TEXTURE_BINDING,
        );
        let output = texture(
            selected,
            p::TextureUsages::RENDER_ATTACHMENT | p::TextureUsages::COPY_SRC,
        );
        let mut stage = media_gpu::equalization::EqualizationStage::new(
            &device,
            &queue,
            selected,
            Rotation::Deg0,
            false,
            1,
        )
        .unwrap();
        let pipeline = media_gpu::ResizePipeline::new(&device, p::TextureFormat::Rgba8Unorm);
        let transform =
            pipeline.create_adjustment_buffer(&device, &queue, Rotation::Deg0, false, sliders);
        let stride = (selected.width * 4).div_ceil(256) * 256;
        let buffer = device.create_buffer(&p::BufferDescriptor {
            label: Some("test-only pre-encoder parity readback"),
            size: u64::from(stride) * u64::from(selected.height),
            usage: p::BufferUsages::MAP_READ | p::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut results = Vec::new();
        for (index, (rgba, pts)) in frames
            .as_chunks::<{ 320 * 180 * 4 }>()
            .0
            .iter()
            .zip(pts)
            .enumerate()
        {
            queue.write_texture(
                p::TexelCopyTextureInfo {
                    texture: &input,
                    mip_level: 0,
                    origin: p::Origin3d::ZERO,
                    aspect: p::TextureAspect::All,
                },
                rgba,
                p::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(320 * 4),
                    rows_per_image: Some(180),
                },
                input.size(),
            );
            let mut encoder = device.create_command_encoder(&Default::default());
            let equalized = stage
                .record(
                    &device,
                    &mut encoder,
                    &input.create_view(&Default::default()),
                    *pts,
                    policy,
                    1,
                )
                .unwrap();
            pipeline.record_resize(
                &device,
                &mut encoder,
                equalized,
                &transform,
                &output.create_view(&Default::default()),
                selected,
            );
            let capture = [0, 7, 14, 24, 35].contains(&index);
            if capture {
                encoder.copy_texture_to_buffer(
                    p::TexelCopyTextureInfo {
                        texture: &output,
                        mip_level: 0,
                        origin: p::Origin3d::ZERO,
                        aspect: p::TextureAspect::All,
                    },
                    p::TexelCopyBufferInfo {
                        buffer: &buffer,
                        layout: p::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(stride),
                            rows_per_image: Some(selected.height),
                        },
                    },
                    output.size(),
                );
            }
            queue.submit([encoder.finish()]);
            if capture {
                let (sender, receiver) = std::sync::mpsc::channel();
                buffer.slice(..).map_async(p::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
                device
                    .poll(p::PollType::Wait {
                        submission_index: None,
                        timeout: Some(std::time::Duration::from_secs(30)),
                    })
                    .unwrap();
                receiver.recv().unwrap().unwrap();
                let mapped = buffer.slice(..).get_mapped_range().unwrap();
                results.push(
                    mapped
                        .chunks(stride as usize)
                        .flat_map(|row| row[..selected.width as usize * 4].iter().copied())
                        .collect(),
                );
                drop(mapped);
                buffer.unmap();
            }
        }
        results
    }

    #[test]
    #[ignore = "real FFmpeg decode plus processing/renderer GPU parity; no GUI"]
    fn exact_preroll_renderer_matches_full_conversion_and_reuses_maps() {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m35-vfr-offset.mp4");
        let probe = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "frame=best_effort_timestamp",
                "-of",
                "csv=p=0",
            ])
            .arg(&fixture)
            .output()
            .unwrap();
        assert!(probe.status.success());
        let ticks: Vec<i64> = String::from_utf8(probe.stdout)
            .unwrap()
            .lines()
            .filter_map(|line| line.split(',').next()?.parse().ok())
            .collect();
        let base = media_core::TimeBase::new(1, 15360).unwrap();
        let pts: Vec<_> = ticks
            .iter()
            .map(|p| base.ticks_to_microseconds(p - ticks[0]).unwrap())
            .collect();
        let filter = format!("{},format=gbrp,format=rgba", media_native::color::TO_SRGB);
        let decoded = Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&fixture)
            .args([
                "-map",
                "0:v:0",
                "-an",
                "-vf",
                &filter,
                "-fps_mode",
                "passthrough",
                "-f",
                "rawvideo",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(decoded.status.success());
        assert_eq!(pts.len(), 36);
        assert_eq!(decoded.stdout.len(), 36 * 320 * 180 * 4);
        let instance = w::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        println!("Renderer parity adapter: {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        for size in [Size::new(160, 90).unwrap(), Size::new(320, 180).unwrap()] {
            let target = make_target(&device, size);
            let policies = [
                (
                    Equalization::new(true, 25).unwrap(),
                    ColorAdjustments::default(),
                ),
                (
                    Equalization::new(true, 50).unwrap(),
                    ColorAdjustments::new(10, 80, 60).unwrap(),
                ),
                (
                    Equalization::new(true, 80).unwrap(),
                    ColorAdjustments::new(-10, 120, 140).unwrap(),
                ),
            ];
            let reference: Vec<_> = policies
                .iter()
                .map(|(equalization, color)| {
                    conversion_reference(&decoded.stdout, &pts, size, *equalization, *color)
                })
                .collect();
            for (sample, index) in [0, 7, 14, 24, 35].into_iter().enumerate() {
                let (mut pass, summary) = HistoryPass::prepare(
                    &device,
                    &queue,
                    &fixture,
                    pts[index] as u64,
                    size,
                    7,
                    &CancellationToken::default(),
                )
                .unwrap();
                assert_eq!(summary.selected_us, pts[index]);
                let uploads = pass.uploaded_frames();
                assert_eq!(uploads, summary.predecessors as u64 + 1);
                for (policy_index, (settings, sliders)) in policies.into_iter().enumerate() {
                    pass.render(&target, settings, sliders, 7).unwrap();
                    let actual = readback(&device, &queue, &target);
                    let expected = &reference[policy_index][sample];
                    let error = actual
                        .iter()
                        .zip(expected)
                        .map(|(a, b)| a.abs_diff(*b))
                        .max()
                        .unwrap();
                    println!(
                        "Native exact parity {size:?} index={index} policy={policy_index} max={error}; uploads={uploads}; scratch={}",
                        pass.scratch_bytes()
                    );
                    assert!(error <= 2, "renderer vs full conversion: {error}");
                    pass.render(
                        &target,
                        Equalization::default(),
                        ColorAdjustments::default(),
                        7,
                    )
                    .unwrap();
                    let before = readback(&device, &queue, &target);
                    pass.render(
                        &target,
                        Equalization::new(true, 0).unwrap(),
                        ColorAdjustments::default(),
                        7,
                    )
                    .unwrap();
                    assert_eq!(before, readback(&device, &queue, &target));
                    pass.render(&target, settings, sliders, 7).unwrap();
                    assert_eq!(actual, readback(&device, &queue, &target));
                    assert_eq!(uploads, pass.uploaded_frames());
                    if size.width == 320 {
                        let thumbnail = make_target(&device, Size::new(160, 90).unwrap());
                        pass.render(&thumbnail, settings, sliders, 7).unwrap();
                        let pixels = readback(&device, &queue, &thumbnail);
                        // Independent half-size bilinear oracle, AFTER the
                        // full-selected-resolution sliders and RGBA8 clipping.
                        let mut maximum = 0;
                        for y in 0..90_usize {
                            for x in 0..160_usize {
                                for channel in 0..4 {
                                    let sum: u32 = [0, 1]
                                        .into_iter()
                                        .flat_map(|dy| {
                                            [0, 1].into_iter().map(move |dx| {
                                                u32::from(
                                                    expected[((2 * y + dy) * 320 + 2 * x + dx) * 4
                                                        + channel],
                                                )
                                            })
                                        })
                                        .sum();
                                    let reference = ((sum + 2) / 4) as u8;
                                    maximum = maximum.max(
                                        pixels[(y * 160 + x) * 4 + channel].abs_diff(reference),
                                    );
                                }
                            }
                        }
                        println!(
                            "Selected-size to display parity index={index} policy={policy_index} max={maximum}"
                        );
                        assert!(maximum <= 2);
                        assert_eq!(uploads, pass.uploaded_frames());
                    }
                }
                assert!(
                    pass.render(&target, policies[0].0, policies[0].1, 8)
                        .is_err()
                );
            }
        }
        let cancel = CancellationToken::default();
        cancel.cancel();
        assert!(
            HistoryPass::prepare(
                &device,
                &queue,
                &fixture,
                500_000,
                Size::new(160, 90).unwrap(),
                7,
                &cancel
            )
            .is_err()
        );
    }
}
