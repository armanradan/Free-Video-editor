#![forbid(unsafe_code)]

use media_core::{ColorAdjustments, Rotation, Size};

pub const RESIZE_SHADER: &str = include_str!("resize.wgsl");
pub mod equalization;

pub struct ResizePipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    target_is_srgb: bool,
}

impl ResizePipeline {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("M1 bilinear resize shader"),
            source: wgpu::ShaderSource::Wgsl(RESIZE_SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("M1 resize bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(32),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("M1 resize pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("M1 resize pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("M1 bilinear sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipeline,
            layout,
            sampler,
            target_is_srgb: target_format.is_srgb(),
        }
    }

    pub fn record_resize(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::TextureView,
        transform: &wgpu::Buffer,
        target: &wgpu::TextureView,
        output_size: Size,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("M1 resize bind group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: transform.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("M1 resize pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_viewport(
            0.0,
            0.0,
            output_size.width as f32,
            output_size.height as f32,
            0.0,
            1.0,
        );
        pass.draw(0..3, 0..1);
    }

    pub fn create_transform_buffer(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rotation: Rotation,
        flip_horizontal: bool,
    ) -> wgpu::Buffer {
        self.create_adjustment_buffer(
            device,
            queue,
            rotation,
            flip_horizontal,
            ColorAdjustments::default(),
        )
    }

    pub fn create_adjustment_buffer(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rotation: Rotation,
        flip_horizontal: bool,
        color: ColorAdjustments,
    ) -> wgpu::Buffer {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame orientation uniform"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &buffer,
            0,
            &adjustment_uniform_bytes(rotation, flip_horizontal, color, self.target_is_srgb),
        );
        buffer
    }
}

/// Renderer adapters may use a different wgpu ABI, but share this shader contract.
pub fn adjustment_uniform_bytes(
    rotation: Rotation,
    flip_horizontal: bool,
    color: ColorAdjustments,
    target_is_srgb: bool,
) -> [u8; 32] {
    let values = [
        rotation as u32,
        u32::from(flip_horizontal),
        u32::from(target_is_srgb),
        0_u32,
    ];
    let mut bytes = [0_u8; 32];
    for (index, value) in values.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_ne_bytes());
    }
    for (index, value) in color
        .parameters()
        .into_iter()
        .chain([if color.is_neutral() { 0.0 } else { 1.0 }])
        .enumerate()
    {
        bytes[16 + index * 4..20 + index * 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}
