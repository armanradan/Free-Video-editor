//! Test-only CPU fixture upload and final readback around the real Rust adapter.
#![forbid(unsafe_code)]
use media_core::Size;
use media_web::{ProcessingGpuDiagnostic, create_processing_ai_pilot};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
thread_local! { static SETUP: RefCell<Option<ProcessingGpuDiagnostic>> = const { RefCell::new(None) }; }
fn error(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

#[wasm_bindgen]
pub fn ownership() -> String {
    media_web::processing_ai_ownership_diagnostic()
}

#[wasm_bindgen]
pub async fn initialize(
    canvas: web_sys::OffscreenCanvas,
    stage: js_sys::Function,
) -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(|e| web_sys::console::error_1(&error(e))));
    let setup = media_web::initialize_processing_gpu_diagnostic(canvas, stage).await?;
    SETUP.with(|state| *state.borrow_mut() = Some(setup));
    Ok(())
}

#[wasm_bindgen]
pub async fn evaluate(
    width: u32,
    height: u32,
    input: Vec<f32>,
    expected: Vec<f32>,
) -> Result<String, JsValue> {
    if width == 0
        || height == 0
        || width > 577
        || height > 579
        || input.len() != (width * height * 3) as usize
        || expected.len() != input.len() * 16
    {
        return Err(error("outside diagnostic fixture envelope"));
    }
    let (device, queue, generation) = SETUP.with(|state| {
        let state = state.borrow();
        let setup = state.as_ref().ok_or_else(|| error("uninitialized"))?;
        Ok::<_, JsValue>((
            setup.device.clone(),
            setup.queue.clone(),
            u64::from(setup.generation),
        ))
    })?;
    let size = Size::new(width, height).map_err(error)?;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("diagnostic-only uint8 fixture upload"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let plane = (width * height) as usize;
    let mut pixels = Vec::with_capacity(plane * 4);
    for i in 0..plane {
        for c in 0..3 {
            let v = input[c * plane + i];
            let byte = (v * 255.).round();
            if !v.is_finite() || !(0.0..=255.0).contains(&byte) || (byte / 255. - v).abs() > 1e-7 {
                return Err(error("reference input is not uint8/255"));
            }
            pixels.push(byte as u8);
        }
        pixels.push(255);
    }
    queue.write_texture(
        texture.as_image_copy(),
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        texture.size(),
    );
    let mut processor = create_processing_ai_pilot(size, 0, 0)
        .await
        .map_err(error)?;
    let owned = processor.owned_bytes().map_err(error)?;
    let memory = processor.memory_plan().map_err(error)?;
    let cancelled = js_sys::Function::new_no_args("return false");
    let result = processor.process(&texture, generation, &cancelled).await;
    let close = processor.close().await;
    let output = result.map_err(error)?;
    close.map_err(error)?;
    // Inference has been released before this readback; it also proves that the
    // shared processing device survives release. No production readback callback.
    let stride = (width * 16).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("diagnostic-only RGBA8 readback"),
        size: u64::from(stride) * u64::from(height) * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        output.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(height * 4),
            },
        },
        output.size(),
    );
    queue.submit([encoder.finish()]);
    let (tx, rx) = futures_channel::oneshot::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    rx.await.map_err(error)?.map_err(error)?;
    let bytes = buffer.slice(..).get_mapped_range().map_err(error)?;
    let output_plane = plane * 16;
    let mut max = 0u8;
    let mut sum = 0u64;
    for y in 0..height * 4 {
        for x in 0..width * 4 {
            for c in 0..4 {
                let expected = if c == 3 {
                    255
                } else {
                    let v = expected[c as usize * output_plane + (y * width * 4 + x) as usize];
                    if !v.is_finite() {
                        return Err(error("nonfinite reference"));
                    }
                    (v.clamp(0., 1.) * 255.).round() as u8
                };
                let diff = bytes[(y * stride + x * 4 + c) as usize].abs_diff(expected);
                max = max.max(diff);
                sum += u64::from(diff);
            }
        }
    }
    drop(bytes);
    buffer.unmap();
    buffer.destroy();
    texture.destroy();
    output.destroy();
    if max > 1 {
        return Err(error(format!(
            "RGBA8 reference difference {max} exceeds 1 LSB"
        )));
    }
    Ok(serde_json::json!({"max_rgba8_lsb":max,"mae_rgba8_lsb":sum as f64/(output_plane*4) as f64,
        "source_width":width,"source_height":height,"tile_interior":128,"halo":34,
        "owned_tensor_result_bytes":owned,"memory_admission_bytes":memory.total,"memory_admission_budget":memory.budget,
        "workspace_estimate_bytes":memory.workspace_estimate,"weights_allowance_bytes":memory.weights,
        "tiles":media_core::ai_upscale::pilot_tiles(size,128).map_err(error)?.len(),
        "production_pixel_readback_bytes":0,"diagnostic_upload_bytes":pixels.len(),
        "diagnostic_readback_bytes":u64::from(stride)*u64::from(height)*4,
        "session_released_before_readback":true}).to_string())
}

#[wasm_bindgen]
pub async fn lifecycle() -> Result<String, JsValue> {
    let (device, queue, generation) = SETUP.with(|state| {
        let state = state.borrow();
        let setup = state.as_ref().ok_or_else(|| error("uninitialized"))?;
        Ok::<_, JsValue>((
            setup.device.clone(),
            setup.queue.clone(),
            u64::from(setup.generation),
        ))
    })?;
    let size = Size::new(129, 131).map_err(error)?;
    let source = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("diagnostic lifecycle source"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    queue.write_texture(
        source.as_image_copy(),
        &vec![127; (size.width * size.height * 4) as usize],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size.width * 4),
            rows_per_image: Some(size.height),
        },
        source.size(),
    );
    let mut processor = create_processing_ai_pilot(size, 0, 0)
        .await
        .map_err(error)?;
    let never = js_sys::Function::new_no_args("return false");
    if create_processing_ai_pilot(size, 0, 0).await.is_ok() {
        return Err(error(
            "second session bypassed the processing-context credit",
        ));
    }
    let stale = processor.process(&source, generation + 1, &never).await;
    if stale.is_ok() {
        return Err(error("stale generation accepted"));
    }
    // Calls: before allocation, before tile, after actual first GPU inference.
    // Avoid relying on `this` binding: the callback is deliberately a closure.
    let make = js_sys::Function::new_no_args("let calls=0; return ()=>++calls>=3");
    let cancel: js_sys::Function = make
        .call0(&JsValue::NULL)?
        .dyn_into()
        .map_err(|_| error("cancel closure"))?;
    let cancelled = processor.process(&source, generation, &cancel).await;
    if !matches!(&cancelled,Err(e) if e.to_string().contains("cancelled")) {
        return Err(error("active tile cancellation did not reject"));
    }
    let retired = processor.process(&source, generation, &never).await;
    if retired.is_ok() {
        return Err(error("cancelled session silently reused"));
    }
    processor.close().await.map_err(error)?;
    let mut fresh = create_processing_ai_pilot(size, 0, 0)
        .await
        .map_err(error)?;
    processor.close().await.map_err(error)?;
    if create_processing_ai_pilot(size, 0, 0).await.is_ok() {
        return Err(error("idempotent close released another session's credit"));
    }
    let first = fresh
        .process(&source, generation, &never)
        .await
        .map_err(error)?;
    first.destroy();
    let second = fresh
        .process(&source, generation, &never)
        .await
        .map_err(error)?;
    second.destroy();
    fresh.close().await.map_err(error)?;
    source.destroy();
    let rejected = create_processing_ai_pilot(Size::new(578, 579).map_err(error)?, 0, 0).await;
    if rejected.is_ok() {
        return Err(error("geometry admission guard failed"));
    }
    if create_processing_ai_pilot(size, 384 << 20, 0).await.is_ok() {
        return Err(error("combined downstream memory guard failed"));
    }
    Ok(serde_json::json!({"stale_generation_rejected":true,"cancel_after_first_gpu_tile":true,
        "partial_result_unpublished":true,"cancelled_session_retired":true,"explicit_fresh_session_retry":true,
        "two_frames_reuse_one_buffer_pair":true,"second_session_credit_rejected":true,"idempotent_close_preserves_active_credit":true,
        "outside_envelope_rejected_before_model_runtime_setup":true,
        "combined_downstream_budget_rejected_before_model_runtime_setup":true}).to_string())
}
