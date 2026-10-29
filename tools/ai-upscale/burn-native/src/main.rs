//! Actual GPU diagnostic, with an independently owned Vulkan device and counted staging.
#[allow(dead_code)]
mod model {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tmp/ai-upscale/burn-generated/model.rs"
    ));
}
use burn::tensor::{Device, DeviceKind, Tensor, TensorData, wgpu::WgpuBackend};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, time::Instant};

fn npy(path: &Path) -> Vec<f32> {
    let bytes = fs::read(path).unwrap();
    assert_eq!(&bytes[..6], b"\x93NUMPY");
    assert_eq!(bytes[6], 1);
    let header_len = u16::from_le_bytes(bytes[8..10].try_into().unwrap()) as usize;
    let header = std::str::from_utf8(&bytes[10..10 + header_len]).unwrap();
    assert!(header.contains("'<f4'") && header.contains("'fortran_order': False"));
    let (values, remainder) = bytes[10 + header_len..].as_chunks::<4>();
    assert!(remainder.is_empty(), "incomplete FP32 tensor element");
    values
        .iter()
        .map(|value| f32::from_le_bytes(*value))
        .collect()
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(
        args.len(),
        4,
        "expected reference-dir, generated-weights, exact-adapter-name, evidence-json"
    );
    let reference = Path::new(&args[0]);
    let bytes = fs::read(reference.join("model.onnx")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "1793a6e7fdf15a53eed213ba269ea768b5373858ae433d56ee8e1b0424377cc5"
    );
    let mut report = json!({"status": "running", "cases": [], "transfers": "CPU FP32 input upload and GPU FP32 output diagnostic readback per case; independent Vulkan device"});
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (mut device, setup) = Device::wgpu_options()
            .device_kind(DeviceKind::DiscreteGpu(0))
            .graphics_api(WgpuBackend::Vulkan)
            .tasks_max(1)
            .init_with_setup()
            .expect("explicit Vulkan GPU initialization failed");
        let info = setup.adapter.get_info();
        assert_eq!(info.name, args[2], "requested adapter not selected");
        assert_eq!(info.vendor, 0x10de);
        device
            .configure(
                burn::tensor::DeviceConfig::default().float_dtype(burn::tensor::FloatDType::F32),
            )
            .expect("explicit FP32 device configuration failed");
        report["precision"] = json!("explicit FloatDType::F32 before model/tensor creation");
        report["adapter"] = json!({"name": info.name, "vendor": info.vendor,
            "device": info.device, "backend": format!("{:?}", info.backend), "driver": info.driver,
            "driver_info": info.driver_info});
        let started = Instant::now();
        let model = model::Model::from_file(&args[1], &device);
        report["load_seconds"] = json!(started.elapsed().as_secs_f64());
        for (width, height) in [(1, 1), (3, 5), (17, 19), (33, 31), (257, 259)] {
            let stem = format!("{width}x{height}");
            let input = npy(&reference.join(format!("{stem}-input.npy")));
            let expected = npy(&reference.join(format!("{stem}-fp32.npy")));
            let input_bytes = input.len() * 4;
            let started = Instant::now();
            let tensor =
                Tensor::<4>::from_data(TensorData::new(input, [1, 3, height, width]), &device);
            let output = model.forward(tensor);
            assert_eq!(output.dims(), [1, 3, height * 4, width * 4]);
            let data = output.into_data();
            let actual = data.as_slice::<f32>().unwrap();
            assert_eq!(actual.len(), expected.len());
            let mut max = 0_f32;
            let mut sum = 0_f64;
            for (&a, &b) in actual.iter().zip(&expected) {
                let difference = (a - b).abs();
                assert!(difference.is_finite());
                max = max.max(difference);
                sum += difference as f64;
            }
            let mae = sum / actual.len() as f64;
            report["cases"].as_array_mut().unwrap().push(json!({"input": stem,
                "max_abs": max, "mae": mae, "seconds_including_transfers": started.elapsed().as_secs_f64(),
                "host_input_bytes": input_bytes, "host_output_bytes": actual.len() * 4}));
            assert!(max <= 0.01 && mae <= 0.001, "reference tolerance exceeded");
        }
        drop(model);
        drop(device);
        drop(setup);
        report["cleanup"] = json!(
            "Model, tensor and setup owners dropped; process-local runtime cache retires at process exit; driver reclamation unmeasured"
        );
    }));
    report["status"] = json!(if outcome.is_ok() {
        "numeric-gpu-probe-passed-not-product-accepted"
    } else {
        "failed"
    });
    if let Err(error) = &outcome {
        let message = error
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| error.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "non-string panic".into());
        report["error"] = json!(message);
    }
    fs::write(&args[3], serde_json::to_string_pretty(&report).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    if outcome.is_err() {
        std::process::exit(1);
    }
}
