//! Renderer-ABI adapter: one shared processor source, no cross-device handles.
use media_core::{ColorAdjustments, Rotation, Size};
use wgpu_blitz as w;

mod history;
pub use history::HistoryPass;
#[cfg(test)]
pub(crate) use history::tests::readback as diagnostic_readback;
pub(crate) mod worker;

/// Final display-only letterboxing. Histogram/slider stages never see bars.
pub(crate) fn copy_history_display(
    device: &w::Device,
    queue: &w::Queue,
    source: &w::Texture,
    target: &w::Texture,
) {
    let mut encoder = device.create_command_encoder(&Default::default());
    let view = target.create_view(&Default::default());
    {
        let _clear = encoder.begin_render_pass(&w::RenderPassDescriptor {
            label: Some("display-only black preview bars"),
            color_attachments: &[Some(w::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: w::Operations {
                    load: w::LoadOp::Clear(w::Color::BLACK),
                    store: w::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }
    let copy = |texture, origin| w::TexelCopyTextureInfo {
        texture,
        mip_level: 0,
        origin,
        aspect: w::TextureAspect::All,
    };
    encoder.copy_texture_to_texture(
        copy(source, w::Origin3d::ZERO),
        copy(
            target,
            w::Origin3d {
                x: (target.width() - source.width()) / 2,
                y: (target.height() - source.height()) / 2,
                z: 0,
            },
        ),
        source.size(),
    );
    queue.submit([encoder.finish()]);
}

pub struct ColorPass {
    pub source: w::Texture,
    pipeline: media_gpu_blitz::ResizePipeline,
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
        Self {
            source,
            pipeline: media_gpu_blitz::ResizePipeline::new(device, w::TextureFormat::Rgba8Unorm),
        }
    }
    pub fn render(
        &self,
        device: &w::Device,
        queue: &w::Queue,
        target: &w::Texture,
        color: ColorAdjustments,
    ) {
        let uniform =
            self.pipeline
                .create_adjustment_buffer(device, queue, Rotation::Deg0, false, color);
        let mut encoder = device.create_command_encoder(&Default::default());
        self.pipeline.record_resize(
            device,
            &mut encoder,
            &self.source.create_view(&Default::default()),
            &uniform,
            &target.create_view(&Default::default()),
            Size::new(640, 360).expect("fixed preview size"),
        );
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
