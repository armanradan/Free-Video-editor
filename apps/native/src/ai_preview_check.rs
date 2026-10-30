//! Explicit ordinary-window diagnostic, never a product capability grant.
use media_core::{
    ResizeSpec, Size,
    ai_upscale::{AiModel, AiScale, AiSettings, RtxConfiguration},
};
use media_native::{ai_inference::OnnxHelperConfig, ai_preview::PreviewPolicy};
use std::path::PathBuf;

pub fn requested() -> bool {
    std::env::args().any(|arg| arg == "--ai-preview-check")
}

pub fn source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ai-upscale-cut.mp4")
}

pub fn policy() -> Result<(PreviewPolicy, Size), String> {
    let variable = |name| {
        std::env::var(name)
            .map_err(|_| format!("Set {name} for the explicit AI preview diagnostic"))
    };
    let scale = match variable("DIAXUS_AI_PREVIEW_SCALE")?.as_str() {
        "2" => AiScale::Two,
        "4" => AiScale::Four,
        _ => return Err("DIAXUS_AI_PREVIEW_SCALE must be 2 or 4".into()),
    };
    let backend = match std::env::var("DIAXUS_AI_PREVIEW_BACKEND") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => "onnx".into(),
        Err(e) => return Err(e.to_string()),
    };
    let rtx = match backend.as_str() {
        "onnx" => false,
        "rtx" => true,
        _ => return Err("DIAXUS_AI_PREVIEW_BACKEND must be onnx or rtx; no fallback".into()),
    };
    let ai = if rtx {
        AiSettings::RtxVideo {
            configuration: RtxConfiguration::Sdk11SdrV1,
            scale,
        }
    } else {
        AiSettings::OnnxRuntime {
            model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
            scale,
        }
    };
    let exact_adapter_name = variable("DIAXUS_AI_ADAPTER")?;
    let processing_adapter = media_native::enumerate_adapters()
        .into_iter()
        .find(|adapter| adapter.name == exact_adapter_name && adapter.backend == "Vulkan")
        .ok_or("The explicitly selected AI Vulkan adapter is unavailable; no fallback")?
        .key;
    let helper = if rtx {
        media_native::ai_preview::PreviewHelper::Rtx(media_native::rtx_inference::RtxHelperConfig {
            executable: variable("DIAXUS_RTX_HELPER")?.into(),
            sdk_root: variable("DIAXUS_RTX_SDK")?.into(),
            cache_parent: variable("DIAXUS_RTX_CACHE")?.into(),
            dxgi_index: variable("DIAXUS_AI_DXGI_INDEX")?
                .parse()
                .map_err(|_| "Invalid DXGI index")?,
            exact_adapter_name: exact_adapter_name.clone(),
            expected_luid_high: variable("DIAXUS_RTX_LUID_HIGH")?
                .parse()
                .map_err(|_| "Invalid LUID high")?,
            expected_luid_low: variable("DIAXUS_RTX_LUID_LOW")?
                .parse()
                .map_err(|_| "Invalid LUID low")?,
        })
    } else {
        OnnxHelperConfig {
            executable: variable("DIAXUS_AI_HELPER")?.into(),
            runtime_directory: variable("DIAXUS_ORT_DIRECTORY")?.into(),
            model_cache: variable("DIAXUS_AI_MODEL_CACHE")?.into(),
            dxgi_index: variable("DIAXUS_AI_DXGI_INDEX")?
                .parse()
                .map_err(|_| "DIAXUS_AI_DXGI_INDEX must be an integer")?,
            exact_adapter_name,
        }
        .into()
    };
    let selected = ai
        .requested_output_size(
            Size::new(64, 36).map_err(|e| e.to_string())?,
            ResizeSpec::Original,
        )
        .map_err(|e| e.to_string())?;
    Ok((
        PreviewPolicy {
            ai,
            helper,
            processing_adapter,
            source_end: None,
        },
        selected,
    ))
}
