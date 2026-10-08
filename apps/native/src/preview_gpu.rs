//! Renderer-ABI adapter only: the shader and uniform serialization are shared.
use media_core::{ColorAdjustments, Rotation};
use wgpu_blitz as w;

pub struct ColorPass {
    pub source: w::Texture,
    pipeline: w::RenderPipeline,
    layout: w::BindGroupLayout,
    sampler: w::Sampler,
}

impl ColorPass {
    pub fn new(device: &w::Device) -> Self {
        let source = device.create_texture(&w::TextureDescriptor {
            label: Some("cached original preview image"),
            size: w::Extent3d {
                width: 640,
                height: 360,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: w::TextureDimension::D2,
            format: w::TextureFormat::Rgba8Unorm,
            usage: w::TextureUsages::COPY_DST | w::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let layout = device.create_bind_group_layout(&w::BindGroupLayoutDescriptor {
            label: Some("shared color presenter bindings"),
            entries: &[
                w::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: w::ShaderStages::FRAGMENT,
                    ty: w::BindingType::Texture {
                        sample_type: w::TextureSampleType::Float { filterable: true },
                        view_dimension: w::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                w::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: w::ShaderStages::FRAGMENT,
                    ty: w::BindingType::Sampler(w::SamplerBindingType::Filtering),
                    count: None,
                },
                w::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: w::ShaderStages::FRAGMENT,
                    ty: w::BindingType::Buffer {
                        ty: w::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: w::BufferSize::new(32),
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&w::PipelineLayoutDescriptor {
            label: Some("shared color presenter layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(w::ShaderModuleDescriptor {
            label: Some("shared resize/color WGSL on Blitz device"),
            source: w::ShaderSource::Wgsl(media_gpu::RESIZE_SHADER.into()),
        });
        let pipeline = device.create_render_pipeline(&w::RenderPipelineDescriptor {
            label: Some("shared preview color pass"),
            layout: Some(&pipeline_layout),
            vertex: w::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(w::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(w::ColorTargetState {
                    format: w::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: w::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&w::SamplerDescriptor {
            mag_filter: w::FilterMode::Linear,
            min_filter: w::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            source,
            pipeline,
            layout,
            sampler,
        }
    }

    pub fn render(
        &self,
        device: &w::Device,
        queue: &w::Queue,
        target: &w::Texture,
        color: ColorAdjustments,
    ) {
        let uniform = device.create_buffer(&w::BufferDescriptor {
            label: Some("immutable preview color snapshot"),
            size: 32,
            usage: w::BufferUsages::UNIFORM | w::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &uniform,
            0,
            &media_gpu::adjustment_uniform_bytes(Rotation::Deg0, false, color, false),
        );
        let source = self.source.create_view(&Default::default());
        let target = target.create_view(&Default::default());
        let binding = device.create_bind_group(&w::BindGroupDescriptor {
            label: Some("preview color snapshot"),
            layout: &self.layout,
            entries: &[
                w::BindGroupEntry {
                    binding: 0,
                    resource: w::BindingResource::TextureView(&source),
                },
                w::BindGroupEntry {
                    binding: 1,
                    resource: w::BindingResource::Sampler(&self.sampler),
                },
                w::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&w::RenderPassDescriptor {
                label: Some("shared native preview adjustment"),
                color_attachments: &[Some(w::RenderPassColorAttachment {
                    view: &target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: w::Operations {
                        load: w::LoadOp::Clear(w::Color::BLACK),
                        store: w::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &binding, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "real renderer-ABI GPU diagnostic; no UI automation"]
    fn actual_blitz_abi_shader_matches_reference() {
        let instance = w::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        println!("Blitz ABI color adapter: {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let pass = ColorPass::new(&device);
        let input = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/m2-h264-aac.mp4");
        let source = media_native::preview::decode_frame(
            &input,
            0,
            &media_native::CancellationToken::default(),
        )
        .unwrap()
        .rgba;
        queue.write_texture(
            w::TexelCopyTextureInfo {
                texture: &pass.source,
                mip_level: 0,
                origin: w::Origin3d::ZERO,
                aspect: w::TextureAspect::All,
            },
            &source,
            w::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(640 * 4),
                rows_per_image: Some(360),
            },
            w::Extent3d {
                width: 640,
                height: 360,
                depth_or_array_layers: 1,
            },
        );
        let target = device.create_texture(&w::TextureDescriptor {
            label: None,
            size: w::Extent3d {
                width: 640,
                height: 360,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: w::TextureDimension::D2,
            format: w::TextureFormat::Rgba8Unorm,
            usage: w::TextureUsages::RENDER_ATTACHMENT | w::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        for color in [
            ColorAdjustments::default(),
            ColorAdjustments::new(10, 150, 0).unwrap(),
            ColorAdjustments::new(10, 150, 50).unwrap(),
            ColorAdjustments::new(-15, 70, 130).unwrap(),
            ColorAdjustments::new(-100, 200, 200).unwrap(),
        ] {
            pass.render(&device, &queue, &target, color);
            let buffer = device.create_buffer(&w::BufferDescriptor {
                label: None,
                size: (640 * 360 * 4) as u64,
                usage: w::BufferUsages::COPY_DST | w::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_buffer(
                w::TexelCopyTextureInfo {
                    texture: &target,
                    mip_level: 0,
                    origin: w::Origin3d::ZERO,
                    aspect: w::TextureAspect::All,
                },
                w::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: w::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(640 * 4),
                        rows_per_image: Some(360),
                    },
                },
                w::Extent3d {
                    width: 640,
                    height: 360,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit([encoder.finish()]);
            let (send, receive) = std::sync::mpsc::channel();
            buffer.slice(..).map_async(w::MapMode::Read, move |result| {
                send.send(result).unwrap();
            });
            device.poll(w::PollType::Wait).unwrap();
            receive.recv().unwrap().unwrap();
            let bytes = buffer.slice(..).get_mapped_range();
            let mut max_error = 0;
            for (input, pixel) in source
                .as_chunks::<4>()
                .0
                .iter()
                .zip(bytes.as_chunks::<4>().0)
            {
                for (actual, expected) in pixel[..3].iter().zip(color.reference_rgb([
                    input[0] as f32 / 255.0,
                    input[1] as f32 / 255.0,
                    input[2] as f32 / 255.0,
                ])) {
                    let error =
                        (i32::from(*actual) - (expected * 255.0).round() as i32).unsigned_abs();
                    max_error = max_error.max(error);
                }
                assert_eq!(pixel[3], 255);
            }
            println!("Actual decoded-frame Blitz color {color:?}: max={max_error}");
            assert!(max_error <= 2, "{color:?}: {max_error}");
            drop(bytes);
            buffer.unmap();
        }
    }
}
