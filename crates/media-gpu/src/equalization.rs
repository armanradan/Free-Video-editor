//! GPU-resident CLAHE engine. Platform preview/codec wiring is a separate slice.
use media_core::{
    MediaError, Size,
    equalization::{BINS, Equalization, MappingHistory},
};
use wgpu::util::DeviceExt;

pub const SHADER: &str = include_str!("equalization.wgsl");
const MAX_BYTES: u64 = 128 * 1024 * 1024;

/// Selected-resolution slider output, before an independent display resize.
/// RGBA8 matches the supported browser export boundary, not Main 10 precision.
pub struct PreviewColorStage {
    pipeline: crate::ResizePipeline,
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: Size,
    bytes: u64,
}
impl PreviewColorStage {
    pub fn new(
        device: &wgpu::Device,
        size: Size,
        preceding_bytes: u64,
    ) -> Result<Self, MediaError> {
        let bytes = u64::from(size.width)
            .checked_mul(u64::from(size.height))
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| MediaError::Platform("CLAHE preview byte-size overflow".into()))?;
        if preceding_bytes
            .checked_add(bytes)
            .is_none_or(|n| n > 192 * 1024 * 1024)
            || size.width == 0
            || size.height == 0
            || size.width > device.limits().max_texture_dimension_2d
            || size.height > device.limits().max_texture_dimension_2d
        {
            return Err(MediaError::Platform(
                "CLAHE preview scratch exceeds device/192 MiB budget".into(),
            ));
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("selected-resolution adjusted preview before display scaling"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Ok(Self {
            pipeline: crate::ResizePipeline::new(device, wgpu::TextureFormat::Rgba8Unorm),
            _texture: texture,
            view,
            size,
            bytes,
        })
    }
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
    pub fn record<'a>(
        &'a self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::TextureView,
        policy: (media_core::Rotation, bool, media_core::ColorAdjustments),
    ) -> &'a wgpu::TextureView {
        let transform = self
            .pipeline
            .create_adjustment_buffer(device, queue, policy.0, policy.1, policy.2);
        self.pipeline
            .record_resize(device, encoder, source, &transform, &self.view, self.size);
        &self.view
    }
}

/// Shared neutral geometry stage used by browser and native conversion. The
/// caller consumes its encoded output through the existing manual-slider pass.
pub struct EqualizationStage {
    pipeline: crate::ResizePipeline,
    geometry: wgpu::Texture,
    neutral: wgpu::Buffer,
    processor: ClaheProcessor,
    size: Size,
    scratch_bytes: u64,
}
impl EqualizationStage {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: Size,
        rotation: media_core::Rotation,
        flip: bool,
        generation: u64,
    ) -> Result<Self, MediaError> {
        let processor = ClaheProcessor::new(device, size, generation)?;
        let scratch_bytes = processor
            .storage_bytes()
            .checked_add(u64::from(size.width) * u64::from(size.height) * 8)
            .ok_or_else(|| MediaError::Platform("CLAHE geometry scratch overflow".into()))?;
        if scratch_bytes > 192 * 1024 * 1024 {
            return Err(MediaError::Platform(
                "CLAHE engine plus geometry scratch exceeds 192 MiB budget".into(),
            ));
        }
        let pipeline = crate::ResizePipeline::new(device, wgpu::TextureFormat::Rgba16Float);
        let neutral = pipeline.create_transform_buffer(device, queue, rotation, flip);
        let geometry = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("CLAHE neutral resized encoded RGB"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        Ok(Self {
            pipeline,
            geometry,
            neutral,
            processor,
            size,
            scratch_bytes,
        })
    }
    pub fn scratch_bytes(&self) -> u64 {
        self.scratch_bytes
    }
    #[allow(clippy::too_many_arguments)]
    pub fn record<'a>(
        &'a mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::TextureView,
        pts: i64,
        settings: Equalization,
        generation: u64,
    ) -> Result<&'a wgpu::TextureView, MediaError> {
        self.pipeline.record_resize(
            device,
            encoder,
            source,
            &self.neutral,
            &self.geometry.create_view(&Default::default()),
            self.size,
        );
        self.processor
            .record(
                device,
                encoder,
                &self.geometry.create_view(&Default::default()),
                pts,
                settings,
                generation,
            )?
            .ok_or_else(|| MediaError::Platform("enabled CLAHE unexpectedly bypassed".into()))
    }
}

pub struct ClaheProcessor {
    size: Size,
    grid: [u32; 2],
    device_generation: u64,
    history: MappingHistory,
    layout: wgpu::BindGroupLayout,
    stages: [wgpu::ComputePipeline; 4],
    hist: wgpu::Buffer,
    maps: wgpu::Buffer,
    epochs: wgpu::Buffer,
    _output: wgpu::Texture,
    view: wgpu::TextureView,
    storage_bytes: u64,
}
impl ClaheProcessor {
    pub fn new(
        device: &wgpu::Device,
        size: Size,
        device_generation: u64,
    ) -> Result<Self, MediaError> {
        let limits = device.limits();
        #[cfg(feature = "renderer-abi-26")]
        let max_storage_binding = u64::from(limits.max_storage_buffer_binding_size);
        #[cfg(not(feature = "renderer-abi-26"))]
        let max_storage_binding = limits.max_storage_buffer_binding_size;
        let grid = [
            (size.width / 64).clamp(1, 8),
            (size.height / 64).clamp(1, 8),
        ];
        let tiles = u64::from(grid[0] * grid[1]);
        let histogram_bytes = 4 * (tiles + 1) * u64::from(BINS) * 4;
        let map_bytes = 4 * tiles * u64::from(BINS) * 4;
        let storage_bytes = u64::from(size.width)
            .checked_mul(u64::from(size.height))
            .and_then(|n| n.checked_mul(8))
            .and_then(|n| n.checked_add(histogram_bytes + map_bytes + 16))
            .ok_or_else(|| MediaError::Platform("CLAHE scratch size overflow".into()))?;
        if size.width == 0
            || size.height == 0
            || size.width > limits.max_texture_dimension_2d
            || size.height > limits.max_texture_dimension_2d
            || storage_bytes > MAX_BYTES
            || histogram_bytes > max_storage_binding
            || map_bytes > max_storage_binding
            || histogram_bytes > limits.max_buffer_size
            || map_bytes > limits.max_buffer_size
            || limits.max_storage_buffers_per_shader_stage < 3
            || limits.max_storage_textures_per_shader_stage < 1
            || limits.max_compute_invocations_per_workgroup < 64
            || limits.max_compute_workgroup_size_x < 8
            || limits.max_compute_workgroup_size_y < 8
            || size.width.div_ceil(8) > limits.max_compute_workgroups_per_dimension
            || size.height.div_ceil(8) > limits.max_compute_workgroups_per_dimension
        {
            return Err(MediaError::Platform(
                "CLAHE exceeds this device's compute/storage limits or 128 MiB scratch budget"
                    .into(),
            ));
        }
        let buffer = |label, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let hist = buffer("CLAHE four-slot histograms", histogram_bytes);
        let maps = buffer("CLAHE four-slot mappings", map_bytes);
        let epochs = buffer("CLAHE scene epochs", 16);
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("CLAHE encoded RGB16 output"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = output.create_view(&Default::default());
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty,
            count: None,
        };
        let storage = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("CLAHE resources"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    1,
                    wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                ),
                entry(
                    2,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(64),
                    },
                ),
                entry(3, storage),
                entry(4, storage),
                entry(5, storage),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("CLAHE pipelines"),
            #[cfg(not(feature = "renderer-abi-26"))]
            bind_group_layouts: &[Some(&layout)],
            #[cfg(feature = "renderer-abi-26")]
            bind_group_layouts: &[&layout],
            #[cfg(not(feature = "renderer-abi-26"))]
            immediate_size: 0,
            #[cfg(feature = "renderer-abi-26")]
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("CLAHE v1"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let stages = ["histogram", "scene_cut", "mapping", "apply"].map(|name| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(name),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(name),
                compilation_options: Default::default(),
                cache: None,
            })
        });
        Ok(Self {
            size,
            grid,
            device_generation,
            history: MappingHistory::default(),
            layout,
            stages,
            hist,
            maps,
            epochs,
            _output: output,
            view,
            storage_bytes,
        })
    }
    pub fn storage_bytes(&self) -> u64 {
        self.storage_bytes
    }
    pub fn reset(&mut self) {
        self.history.reset();
    }

    /// Source is already geometry-normalized/resized *encoded* RGB, not an sRGB
    /// attachment decoded to linear values. Caller's one device/queue must submit
    /// each recorded frame in order and consume the returned view in that same
    /// command encoder before recording another frame. Reset if recording is
    /// abandoned, on source/seek/settings/generation changes and cancellation.
    /// No readback; returned None means original source bypasses all four passes.
    #[allow(clippy::too_many_arguments)]
    pub fn record<'a>(
        &'a mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::TextureView,
        pts_us: i64,
        settings: Equalization,
        device_generation: u64,
    ) -> Result<Option<&'a wgpu::TextureView>, MediaError> {
        if device_generation != self.device_generation {
            return Err(MediaError::Platform(
                "CLAHE device generation mismatch".into(),
            ));
        }
        if !settings.active() {
            self.reset();
            return Ok(None);
        }
        let plan = self.history.advance(pts_us)?;
        let mut bytes = Vec::with_capacity(64);
        for v in [
            self.size.width,
            self.size.height,
            self.grid[0],
            self.grid[1],
            plan.slot,
            plan.previous,
            u32::from(plan.reset),
            0,
        ] {
            bytes.extend(v.to_le_bytes());
        }
        for v in plan
            .weights
            .into_iter()
            .chain([settings.blend(), 0.35, 0., 0.])
        {
            bytes.extend(v.to_le_bytes());
        }
        // Immutable per-submission uniforms: never overwrite an in-flight plan.
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CLAHE frame plan"),
            contents: &bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CLAHE frame"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.hist.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.maps.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.epochs.as_entire_binding(),
                },
            ],
        });
        if !plan.reuse {
            let stride = u64::from((self.grid[0] * self.grid[1] + 1) * BINS) * 4;
            encoder.clear_buffer(&self.hist, u64::from(plan.slot) * stride, Some(stride));
        }
        for (index, stage) in self.stages.iter().enumerate() {
            if plan.reuse && index != 3 {
                continue;
            }
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("CLAHE bounded pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(stage);
            pass.set_bind_group(0, &bind, &[]);
            match index {
                0 | 3 => pass.dispatch_workgroups(
                    self.size.width.div_ceil(8),
                    self.size.height.div_ceil(8),
                    1,
                ),
                1 => pass.dispatch_workgroups(1, 1, 1),
                _ => pass.dispatch_workgroups(self.grid[0] * self.grid[1], 1, 1),
            }
        }
        Ok(Some(&self.view))
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "equalization_tests.rs"]
mod tests;
