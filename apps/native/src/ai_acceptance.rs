//! The evaluated native RTX release envelope. Import/setup cannot expand it.
use media_core::{
    Size,
    ai_upscale::{AiCapability, AiGeometryLimits, AiScale},
};
use media_native::{
    ProcessingRoute, SourceInfo, ai_configuration::NativeAiConfiguration,
    ai_preflight::SetupPreflight, ai_preview::PreviewHelper,
};

/// Export is independent of preview capability. Never select a codec route on
/// the user's behalf; reuse this policy in both rendering and command handling.
pub fn export_route_reason(
    route: ProcessingRoute,
    ai: media_core::ai_upscale::AiSettings,
    source: Option<&SourceInfo>,
) -> Option<String> {
    use media_core::ai_upscale::AiSettings;
    if ai == AiSettings::Off || route == ProcessingRoute::SharedWgpu {
        return None;
    }
    if route != ProcessingRoute::SharedWgpuNvidia {
        return Some("AI export requires a Shared GPU route. Select software codecs or NVIDIA codecs explicitly; source preview remains available.".into());
    }
    if !matches!(ai, AiSettings::RtxVideo { .. }) {
        return Some(
            "Native ONNX with NVIDIA codecs remains unvalidated; no automatic fallback.".into(),
        );
    }
    let result = (|| -> Result<(), String> {
        let source = source
            .ok_or("Inspect a supported source before selecting hardware-codec AI export.")?;
        let size =
            Size::new(source.display_width, source.display_height).map_err(|e| e.to_string())?;
        let output = ai
            .requested_output_size(size, media_core::ResizeSpec::Original)
            .map_err(|e| e.to_string())?;
        media_native::rtx_inference::validate_nvidia_export(&source.codec, output)
            .map_err(|e| e.to_string())
    })();
    result.err()
}

pub fn choices(
    config: Option<&NativeAiConfiguration>,
    setup: Option<&SetupPreflight>,
    adapter: &str,
    source: Option<&SourceInfo>,
) -> ui::AiChoices {
    let result = (|| -> Result<(), String> {
        let config = config
            .ok_or("Import a local RTX configuration, inspect it, and test its runtime first.")?;
        let PreviewHelper::Rtx(rtx) = &config.helper else {
            return Err(
                "Native ONNX ordinary-window acceptance remains pending; no RTX fallback.".into(),
            );
        };
        let setup=setup.ok_or("Inspect this configuration, acknowledge the supplied SDK terms, and complete Test runtime first.")?;
        if !cfg!(target_os = "windows")
            || config.processing_adapter != adapter
            || setup.processing_adapter != adapter
        {
            return Err(
                "Select the configuration's exact evaluated Vulkan processing GPU explicitly."
                    .into(),
            );
        }
        if setup.helper_sha256 != media_native::rtx_inference::EVALUATED_HELPER_SHA256
            || setup.identities.len() != 2
            || setup.frames.len() != 4
            || setup.sdk_terms.as_ref().is_none_or(|t| {
                t.sha256 != "5addfcf7c78891a12f1dcd6b5f86c2512351ab71b3872f446ba10e1b6da54464"
            })
            || setup.identities.iter().any(|id| {
                id.runtime != "RTX Video SDK 1.1.0"
                    || id.sdk_configuration.as_deref() != Some("sdk-1.1-sdr-v1/high3")
                    || id.vendor_id != 0x10de
                    || id.device_id != 0x25a2
                    || id.adapter_name != "NVIDIA GeForce RTX 3050 Laptop GPU"
                    || id.cpu_fallback
                    || id.dxgi_index != rtx.dxgi_index
                    || id.luid_high != rtx.expected_luid_high
                    || id.luid_low != rtx.expected_luid_low
            })
        {
            return Err("This helper/runtime/device has no matching evaluated native RTX release envelope. No fallback.".into());
        }
        let source = source.ok_or("Inspect a source before selecting RTX AI.")?;
        if source.preview_end.is_none() {
            return Err("Exact final video timestamp/file identity is unavailable; inspect this source before AI preview.".into());
        }
        if let Some(reason) = source.shared_gpu_limitation() {
            return Err(reason);
        }
        media_native::rtx_inference::validate_source(
            Size::new(source.width, source.height).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })();
    match result {
        Err(reason) => {
            let pending = AiCapability::Unavailable { reason };
            ui::AiChoices {
                two: pending.clone(),
                four: pending,
            }
        }
        Ok(()) => {
            let capability = |scale: AiScale| {
                let maximum = match scale {
                    AiScale::Two => Size::new(1280, 720).unwrap(),
                    AiScale::Four => Size::new(640, 360).unwrap(),
                };
                let source = source.unwrap();
                if source.width > maximum.width || source.height > maximum.height {
                    return AiCapability::Unavailable {
                        reason: format!(
                            "Evaluated RTX {}× input limit is {}×{}; reduce scale or source size. The 384 MiB staging and CLAHE scratch limits remain enforced.",
                            scale.factor(),
                            maximum.width,
                            maximum.height
                        ),
                    };
                }
                AiCapability::Validated {
                    settings: media_core::ai_upscale::AiSettings::RtxVideo {
                        configuration: media_core::ai_upscale::RtxConfiguration::Sdk11SdrV1,
                        scale,
                    },
                    limits: AiGeometryLimits {
                        max_source: maximum,
                        max_output: Size::new(2560, 1440).unwrap(),
                        max_output_pixels: 2560 * 1440,
                    },
                }
            };
            ui::AiChoices {
                two: capability(AiScale::Two),
                four: capability(AiScale::Four),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_core::ai_upscale::{AiSettings, RtxConfiguration};
    use media_native::{
        CancellationToken, NativeSession,
        ai_inference::HelperIdentity,
        ai_preflight::{SdkTerms, SetupFrame},
        rtx_inference::RtxHelperConfig,
    };

    #[test]
    fn rtx_export_routes_preserve_preview_and_reject_unvalidated_codec_geometry() {
        let fixture =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m2-h264-aac.mp4");
        let mut source = media_native::probe_source(&fixture).unwrap();
        let ai = AiSettings::RtxVideo {
            configuration: RtxConfiguration::Sdk11SdrV1,
            scale: AiScale::Two,
        };
        for route in [
            ProcessingRoute::SharedWgpu,
            ProcessingRoute::SharedWgpuNvidia,
        ] {
            assert!(export_route_reason(route, ai, Some(&source)).is_none());
        }
        for route in [ProcessingRoute::DirectFfmpeg, ProcessingRoute::NvidiaFfmpeg] {
            assert!(export_route_reason(route, ai, Some(&source)).is_some());
            assert!(export_route_reason(route, AiSettings::Off, Some(&source)).is_none());
        }
        source.display_width = 64;
        source.display_height = 36;
        assert!(
            export_route_reason(ProcessingRoute::SharedWgpuNvidia, ai, Some(&source))
                .unwrap()
                .contains("192x144")
        );
        assert!(export_route_reason(ProcessingRoute::SharedWgpu, ai, Some(&source)).is_none());
        source.display_width = 192;
        source.display_height = 108;
        source.codec = "vp9".into();
        assert!(
            export_route_reason(ProcessingRoute::SharedWgpuNvidia, ai, Some(&source))
                .unwrap()
                .contains("H.264/HEVC")
        );
        let onnx = AiSettings::OnnxRuntime {
            model: media_core::ai_upscale::AiModel::RealEsrGeneralX4v3StrongFp32V1,
            scale: AiScale::Two,
        };
        assert!(
            export_route_reason(ProcessingRoute::SharedWgpuNvidia, onnx, Some(&source))
                .unwrap()
                .contains("unvalidated")
        );
    }

    #[test]
    fn source_device_helper_and_session_readiness_are_independent_gates() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/ai-upscale-cut.mp4");
        let session = NativeSession::new(None).unwrap();
        let source = session
            .inspect(&fixture, &CancellationToken::default())
            .unwrap();
        session.shutdown();
        // Test-only completed setup metadata; no runtime or product acceptance
        // is imported through this fixture or through NativeAiConfiguration.
        let adapter = "explicit evaluated Vulkan adapter";
        let config = NativeAiConfiguration {
            processing_adapter: adapter.into(),
            helper: PreviewHelper::Rtx(RtxHelperConfig {
                executable: "synthetic.exe".into(),
                sdk_root: "synthetic-sdk".into(),
                cache_parent: "synthetic-cache".into(),
                dxgi_index: 1,
                exact_adapter_name: "NVIDIA GeForce RTX 3050 Laptop GPU".into(),
                expected_luid_high: 0,
                expected_luid_low: 7,
            }),
        };
        let identity = HelperIdentity {
            protocol: 1,
            pid: 0,
            runtime: "RTX Video SDK 1.1.0".into(),
            model_sha256: String::new(),
            sdk_configuration: Some("sdk-1.1-sdr-v1/high3".into()),
            dxgi_index: 1,
            adapter_name: "NVIDIA GeForce RTX 3050 Laptop GPU".into(),
            vendor_id: 0x10de,
            device_id: 0x25a2,
            luid_high: 0,
            luid_low: 7,
            cpu_fallback: false,
            tile_interior: 0,
            halo: 0,
        };
        let report = SetupPreflight {
            identities: vec![identity.clone(), identity],
            frames: (0..4)
                .map(|pattern| SetupFrame {
                    scale: 2,
                    pattern,
                    output: Size::new(32, 24).unwrap(),
                    sha256: String::new(),
                    host_to_gpu_bytes: 0,
                    gpu_to_host_bytes: 0,
                })
                .collect(),
            processing_adapter: adapter.into(),
            sdk_terms: Some(SdkTerms {
                path: "synthetic-sdk/NVIDIA_RTX_Video_SDK_License.pdf".into(),
                sha256: "5addfcf7c78891a12f1dcd6b5f86c2512351ab71b3872f446ba10e1b6da54464".into(),
            }),
            helper_sha256: media_native::rtx_inference::EVALUATED_HELPER_SHA256.into(),
            release_accepted: false,
        };
        let two = AiSettings::RtxVideo {
            configuration: RtxConfiguration::Sdk11SdrV1,
            scale: AiScale::Two,
        };
        let four = AiSettings::RtxVideo {
            configuration: RtxConfiguration::Sdk11SdrV1,
            scale: AiScale::Four,
        };
        for (cfg, setup, gpu, src) in [
            (None, None, adapter, None),
            (Some(&config), None, adapter, Some(&source)),
            (Some(&config), Some(&report), "wrong GPU", Some(&source)),
            (Some(&config), Some(&report), adapter, None),
        ] {
            let choices = choices(cfg, setup, gpu, src);
            assert!(choices.select(two).is_err());
            assert_eq!(choices.select(AiSettings::Off).unwrap(), AiSettings::Off);
        }
        let accepted = choices(Some(&config), Some(&report), adapter, Some(&source));
        assert_eq!(accepted.select(two).unwrap(), two);
        assert_eq!(accepted.select(four).unwrap(), four);
        assert_eq!(
            four.output_size(
                Size::new(192, 108).unwrap(),
                media_core::ResizeSpec::Original,
                &accepted.four
            )
            .unwrap(),
            Size::new(768, 432).unwrap()
        );
        for invalid in ["helper", "GPU", "terms"] {
            let mut changed = report.clone();
            match invalid {
                "helper" => changed.helper_sha256 = "different build".into(),
                "GPU" => changed.identities[0].luid_low = 8,
                _ => changed.sdk_terms = None,
            };
            assert!(
                choices(Some(&config), Some(&changed), adapter, Some(&source))
                    .select(two)
                    .is_err()
            );
        }
        let mut changed = source.clone();
        changed.width = 1280;
        changed.height = 720;
        let large = choices(Some(&config), Some(&report), adapter, Some(&changed));
        assert!(large.select(two).is_ok());
        assert!(large.select(four).is_err());
        changed.width = 1281;
        assert!(
            choices(Some(&config), Some(&report), adapter, Some(&changed))
                .select(two)
                .is_err()
        );
        changed = source;
        changed.pixel_format = "yuv420p10le".into();
        assert!(
            choices(Some(&config), Some(&report), adapter, Some(&changed))
                .select(two)
                .is_err()
        );
    }
}
