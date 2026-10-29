//! Isolated same-device browser feasibility probe; never a selectable backend.
#[allow(dead_code)]
mod model {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tmp/ai-upscale/burn-generated/model.rs"
    ));
}
use burn::tensor::{
    Device, DeviceConfig, FloatDType, Tensor, TensorData,
    wgpu::{WgpuSetup, wgpu},
};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

struct State {
    model: model::Model,
    device: Device,
}
thread_local! {static STATE: RefCell<Option<State>> = const { RefCell::new(None) };}

#[wasm_bindgen]
pub async fn initialize(
    canvas: web_sys::OffscreenCanvas,
    stage: js_sys::Function,
    weights: Vec<u8>,
) -> Result<String, JsValue> {
    console_error_panic_hook();
    if format!("{:x}", Sha256::digest(&weights))
        != "0e5f20302257c42a8082a4b6bc067e514d38073e22659d6dbd6fd6f347a97784"
    {
        return Err(JsValue::from_str("Generated weights hash mismatch"));
    }
    let setup = media_web::initialize_processing_gpu_diagnostic(canvas, stage).await?;
    let info = setup.adapter.get_info();
    let generation = setup.generation;
    let mut device = Device::wgpu_options()
        .setup(WgpuSetup {
            instance: setup.instance,
            adapter: setup.adapter,
            device: setup.device,
            queue: setup.queue,
            backend: wgpu::Backend::BrowserWebGpu,
        })
        .tasks_max(1)
        .init()
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    device
        .configure(DeviceConfig::default().float_dtype(FloatDType::F32))
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let model = model::Model::from_bytes(burn::tensor::Bytes::from_bytes_vec(weights), &device);
    STATE.with(|state| *state.borrow_mut() = Some(State { model, device }));
    Ok(
        serde_json::json!({"adapter":format!("{info:?}"), "generation":generation,
        "precision":"explicit F32", "device_source":"actual media-web GpuSession handles"})
        .to_string(),
    )
}

fn console_error_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        // A diagnostic message, never a replacement device/loss callback.
        web_sys::console::error_1(&JsValue::from_str(&info.to_string()));
    }));
}

#[wasm_bindgen]
pub async fn evaluate(
    width: usize,
    height: usize,
    input: Vec<f32>,
    expected: Vec<f32>,
) -> Result<String, JsValue> {
    if width == 0
        || height == 0
        || width > 257
        || height > 259
        || input.len() != 3 * width * height
        || expected.len() != 3 * width * height * 16
    {
        return Err(JsValue::from_str(
            "diagnostic input dimensions outside reference set",
        ));
    }
    let output = STATE.with(|state| {
        let state = state.borrow();
        let state = state
            .as_ref()
            .ok_or_else(|| JsValue::from_str("uninitialized Burn session"))?;
        let input =
            Tensor::<4>::from_data(TensorData::new(input, [1, 3, height, width]), &state.device);
        if input.device() != state.device {
            return Err(JsValue::from_str("input device changed"));
        }
        let output = state.model.forward(input);
        if output.device() != state.device {
            return Err(JsValue::from_str("output device changed"));
        }
        Ok(output)
    })?;
    if output.dims() != [1, 3, height * 4, width * 4] {
        return Err(JsValue::from_str("wrong output geometry"));
    }
    let data = output
        .into_data_async()
        .await
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let actual = data
        .as_slice::<f32>()
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    if actual.len() != expected.len() {
        return Err(JsValue::from_str("wrong reference length"));
    }
    let mut max = 0_f32;
    let mut sum = 0_f64;
    for (a, b) in actual.iter().zip(expected) {
        let error = (a - b).abs();
        if !error.is_finite() {
            return Err(JsValue::from_str("nonfinite output"));
        }
        max = max.max(error);
        sum += error as f64;
    }
    let mae = sum / actual.len() as f64;
    if max > 0.01 || mae > 0.001 {
        return Err(JsValue::from_str(&format!(
            "reference error max={max} mae={mae}"
        )));
    }
    Ok(
        serde_json::json!({"max_abs":max,"mae":mae,"same_device_input_output":true,
        "diagnostic_readback_bytes":actual.len()*4})
        .to_string(),
    )
}

#[wasm_bindgen]
pub fn release() {
    STATE.with(|state| *state.borrow_mut() = None);
}
