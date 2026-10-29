//! Source-only video composition pilot. Ordinary commands cannot select it.
use crate::ai_inference::BrowserAiProcessor;
use media_core::{
    MediaError, Rotation, Size,
    ai_upscale::{AiMemoryPlan, AiScale},
};
use media_gpu::ResizePipeline;

pub struct AiProcessedFrame {
    pub texture: wgpu::Texture,
    pub neutral: wgpu::Buffer,
    internal_x4: Option<wgpu::Texture>,
}
impl Drop for AiProcessedFrame {
    fn drop(&mut self) {
        self.texture.destroy();
        if let Some(texture) = self.internal_x4.take() {
            texture.destroy();
        }
    }
}

pub struct BrowserAiVideo {
    processor: BrowserAiProcessor,
    normalized: wgpu::Texture,
    pipeline: ResizePipeline,
    orientation: wgpu::Buffer,
    neutral: wgpu::Buffer,
    normalized_size: Size,
    output_size: Size,
    scale: AiScale,
    pub frames: u64,
    pub memory: AiMemoryPlan,
    pub job_generation: u32,
}

impl BrowserAiVideo {
    /// One cached AI frame and one selected-size adjusted preview image. At x2,
    /// the cached frame also retains its internal x4 texture until consumers drain.
    pub fn preview_retained_bytes(output: Size, scale: AiScale) -> Result<u64, MediaError> {
        u64::from(output.width)
            .checked_mul(u64::from(output.height))
            .and_then(|pixels| pixels.checked_mul(if scale == AiScale::Two { 24 } else { 8 }))
            .ok_or_else(|| MediaError::AiUnavailable("AI preview retention overflow".into()))
    }
    /// Caller preflights this reservation before allocating conversion surfaces.
    /// Retention covers four pending processed frames, input pool and two surfaces;
    /// the internal x4 result is additionally retained for x2 post-downscale.
    pub fn reservations(
        source: Size,
        output: Size,
        scale: AiScale,
        clahe: bool,
        extra_retained_bytes: u64,
    ) -> Result<(u64, u64), MediaError> {
        media_core::ai_upscale::pilot_tiles(source, 128)?;
        if output
            != Size::new(
                source.width * scale.factor(),
                source.height * scale.factor(),
            )?
        {
            return Err(MediaError::AiUnavailable(
                "AI video output does not match selected scale".into(),
            ));
        }
        let downstream = if clahe {
            media_gpu::equalization::EqualizationStage::estimated_scratch_bytes(output)?
        } else {
            0
        };
        let source_bytes = u64::from(source.width) * u64::from(source.height) * 4;
        let output_bytes = u64::from(output.width) * u64::from(output.height) * 4;
        let retained = 320
            + source_bytes * 4
            + output_bytes * 6
            + if scale == AiScale::Two {
                source_bytes * 16 * 4
            } else {
                0
            };
        let retained = retained
            .checked_add(extra_retained_bytes)
            .ok_or_else(|| MediaError::AiUnavailable("AI preview retention overflow".into()))?;
        AiMemoryPlan::new(source, 8 << 20, 128 << 20, downstream, retained, 384 << 20)?;
        Ok((downstream, retained))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: Size,
        output: Size,
        rotation: Rotation,
        flip: bool,
        generation: u64,
        scale: AiScale,
        clahe: bool,
        job_generation: u32,
        extra_retained_bytes: u64,
    ) -> Result<Self, MediaError> {
        let (downstream, retained) =
            Self::reservations(source, output, scale, clahe, extra_retained_bytes)?;
        let processor = BrowserAiProcessor::new(
            device.clone(),
            queue.clone(),
            source,
            generation,
            downstream,
            retained,
        )
        .await?;
        let memory = processor.memory_plan();
        let normalized = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("AI normalized oriented source"),
            size: wgpu::Extent3d {
                width: source.width,
                height: source.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let pipeline = ResizePipeline::new(device, wgpu::TextureFormat::Rgba8Unorm);
        let orientation = pipeline.create_transform_buffer(device, queue, rotation, flip);
        let neutral = pipeline.create_transform_buffer(device, queue, Rotation::Deg0, false);
        Ok(Self {
            processor,
            normalized,
            pipeline,
            orientation,
            neutral,
            normalized_size: source,
            output_size: output,
            scale,
            frames: 0,
            memory,
            job_generation,
        })
    }

    pub async fn process(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &wgpu::Texture,
        generation: u64,
        cancelled: &js_sys::Function,
    ) -> Result<AiProcessedFrame, MediaError> {
        let mut encoder = device.create_command_encoder(&Default::default());
        self.pipeline.record_resize(
            device,
            &mut encoder,
            &source.create_view(&Default::default()),
            &self.orientation,
            &self.normalized.create_view(&Default::default()),
            self.normalized_size,
        );
        queue.submit([encoder.finish()]);
        let x4 = self
            .processor
            .process(&self.normalized, generation, cancelled)
            .await?;
        self.frames += 1;
        if self.scale == AiScale::Four {
            return Ok(AiProcessedFrame {
                texture: x4,
                neutral: self.neutral.clone(),
                internal_x4: None,
            });
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("AI x2 shared post-downscale result"),
            size: wgpu::Extent3d {
                width: self.output_size.width,
                height: self.output_size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        self.pipeline.record_resize(
            device,
            &mut encoder,
            &x4.create_view(&Default::default()),
            &self.neutral,
            &texture.create_view(&Default::default()),
            self.output_size,
        );
        queue.submit([encoder.finish()]);
        Ok(AiProcessedFrame {
            texture,
            neutral: self.neutral.clone(),
            internal_x4: Some(x4),
        })
    }

    pub async fn close(&mut self) -> Result<(), MediaError> {
        self.processor.close().await
    }
}
impl Drop for BrowserAiVideo {
    fn drop(&mut self) {
        self.normalized.destroy();
        self.orientation.destroy();
        self.neutral.destroy();
    }
}
