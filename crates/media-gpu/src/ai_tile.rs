//! Shared GPU-only FP32 model boundary. Construction is explicit and allocates
//! nothing for AI Off. The platform owns serialization, inference and completion;
//! it must not reuse these two buffers until inference/unpacking completes.
use media_core::{
    MediaError, Size,
    ai_upscale::{AiTile, pilot_tiles},
};
use wgpu::util::DeviceExt;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "ai_tile_tests.rs"]
mod tests;

pub struct AiTilePipeline {
    pack: wgpu::ComputePipeline,
    unpack: wgpu::ComputePipeline,
    pack_layout: wgpu::BindGroupLayout,
    unpack_layout: wgpu::BindGroupLayout,
    input: wgpu::Buffer,
    output: wgpu::Buffer,
    source_size: Size,
    generation: u64,
}

impl AiTilePipeline {
    // Integration pilot: one 128-square interior with a clipped 34-pixel halo.
    pub const MAX_TILE: u32 = 196;
    pub const INPUT_BYTES: u64 = 196 * 196 * 3 * 4;
    pub const OUTPUT_BYTES: u64 = Self::INPUT_BYTES * 16;

    pub fn new(
        device: &wgpu::Device,
        source_size: Size,
        generation: u64,
    ) -> Result<Self, MediaError> {
        pilot_tiles(source_size, 128)?;
        let limits = device.limits();
        #[cfg(feature = "renderer-abi-26")]
        let binding_limit = u64::from(limits.max_storage_buffer_binding_size);
        #[cfg(not(feature = "renderer-abi-26"))]
        let binding_limit = limits.max_storage_buffer_binding_size;
        if binding_limit < Self::OUTPUT_BYTES
            || limits.max_buffer_size < Self::OUTPUT_BYTES
            || limits.max_texture_dimension_2d < source_size.width.max(source_size.height) * 4
        {
            return Err(MediaError::AiUnavailable(
                "AI tile exceeds this device's limits".into(),
            ));
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("AI RGB NCHW tile boundary"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ai_tile.wgsl").into()),
        });
        let storage = |read_only| wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let uniform = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(32),
        };
        let layout = |label, types: [wgpu::BindingType; 3]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries: &types
                    .into_iter()
                    .enumerate()
                    .map(|(binding, ty)| wgpu::BindGroupLayoutEntry {
                        binding: binding as u32,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty,
                        count: None,
                    })
                    .collect::<Vec<_>>(),
            })
        };
        let pack_layout = layout(
            "AI pack layout",
            [
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                storage(false),
                uniform,
            ],
        );
        let unpack_layout = layout(
            "AI unpack layout",
            [
                storage(true),
                wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                uniform,
            ],
        );
        let make_pipeline = |entry, layouts: &[&wgpu::BindGroupLayout]| {
            #[cfg(not(feature = "renderer-abi-26"))]
            let layouts = layouts.iter().map(|v| Some(*v)).collect::<Vec<_>>();
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(entry),
                #[cfg(not(feature = "renderer-abi-26"))]
                bind_group_layouts: &layouts,
                #[cfg(feature = "renderer-abi-26")]
                bind_group_layouts: layouts,
                #[cfg(not(feature = "renderer-abi-26"))]
                immediate_size: 0,
                #[cfg(feature = "renderer-abi-26")]
                push_constant_ranges: &[],
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let pack = make_pipeline("pack", &[&pack_layout]);
        // WGSL unpack uses group 1 so its bindings cannot alias the pack group.
        let empty = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("AI unused group"),
            entries: &[],
        });
        let unpack = make_pipeline("unpack", &[&empty, &unpack_layout]);
        let buffer = |label, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Ok(Self {
            pack,
            unpack,
            pack_layout,
            unpack_layout,
            input: buffer("AI single input tile", Self::INPUT_BYTES),
            output: buffer("AI single output tile", Self::OUTPUT_BYTES),
            source_size,
            generation,
        })
    }

    pub fn input(&self) -> &wgpu::Buffer {
        &self.input
    }
    pub fn output(&self) -> &wgpu::Buffer {
        &self.output
    }

    fn plan(
        &self,
        device: &wgpu::Device,
        tile: AiTile,
        generation: u64,
    ) -> Result<wgpu::Buffer, MediaError> {
        // Accept only plans generated by the bounded policy; reject malformed
        // public tile values before subtraction or GPU submission.
        if generation != self.generation || !pilot_tiles(self.source_size, 128)?.contains(&tile) {
            return Err(MediaError::AiUnavailable(
                "AI tile plan/device generation mismatch".into(),
            ));
        }
        let values = [
            tile.x0,
            tile.y0,
            tile.x1 - tile.x0,
            tile.y1 - tile.y0,
            tile.left - tile.x0,
            tile.top - tile.y0,
            tile.right - tile.left,
            tile.bottom - tile.top,
        ];
        let bytes = values
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        Ok(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("AI immutable tile plan"),
                contents: &bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            }),
        )
    }

    pub fn record_pack(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Texture,
        tile: AiTile,
        generation: u64,
    ) -> Result<wgpu::Buffer, MediaError> {
        if source.format() != wgpu::TextureFormat::Rgba8Unorm
            || source.width() != self.source_size.width
            || source.height() != self.source_size.height
        {
            return Err(MediaError::AiUnavailable(
                "AI requires normalized RGBA8 unorm source geometry".into(),
            ));
        }
        let plan = self.plan(device, tile, generation)?;
        let view = source.create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("AI pack tile"),
            layout: &self.pack_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.input.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: plan.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&self.pack);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(
            (tile.x1 - tile.x0).div_ceil(8),
            (tile.y1 - tile.y0).div_ceil(8),
            1,
        );
        drop(pass);
        Ok(plan)
    }

    pub fn record_unpack(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
        tile: AiTile,
        generation: u64,
    ) -> Result<wgpu::Buffer, MediaError> {
        if target.format() != wgpu::TextureFormat::Rgba8Unorm
            || target.width() != self.source_size.width * 4
            || target.height() != self.source_size.height * 4
        {
            return Err(MediaError::AiUnavailable(
                "AI requires RGBA8 unorm x4 target geometry".into(),
            ));
        }
        let plan = self.plan(device, tile, generation)?;
        let view = target.create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("AI unpack tile"),
            layout: &self.unpack_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.output.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: plan.as_entire_binding(),
                },
            ],
        });
        let empty = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("AI unused group"),
            layout: &self.unpack.get_bind_group_layout(0),
            entries: &[],
        });
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&self.unpack);
        pass.set_bind_group(0, &empty, &[]);
        pass.set_bind_group(1, &bind, &[]);
        pass.dispatch_workgroups(
            ((tile.right - tile.left) * 4).div_ceil(8),
            ((tile.bottom - tile.top) * 4).div_ceil(8),
            1,
        );
        drop(pass);
        Ok(plan)
    }
}
