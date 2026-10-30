//! Explicit local configuration shared by ordinary preview and job snapshots.
//! Reading it neither executes a helper nor grants backend acceptance.
use crate::{
    NativeResult,
    ai_preview::{PreviewHelper, PreviewPolicy},
};
use media_core::{
    ResizeSpec, Size,
    ai_upscale::{AiCapability, AiSettings},
};
use serde::Deserialize;
use std::{io::Read, path::Path};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeAiConfiguration {
    pub helper: PreviewHelper,
    pub processing_adapter: String,
}

impl NativeAiConfiguration {
    pub fn read(path: &Path) -> NativeResult<Self> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(65537)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 65536 {
            return Err("AI configuration exceeds 64 KiB".into());
        }
        let config: Self = serde_json::from_slice(&bytes)?;
        let (name, paths): (&str, Vec<&Path>) = match &config.helper {
            PreviewHelper::Onnx(c) => (
                &c.exact_adapter_name,
                vec![&c.executable, &c.runtime_directory, &c.model_cache],
            ),
            PreviewHelper::Rtx(c) => (
                &c.exact_adapter_name,
                vec![&c.executable, &c.sdk_root, &c.cache_parent],
            ),
        };
        if name.is_empty()
            || config.processing_adapter.is_empty()
            || paths.iter().any(|p| !p.is_absolute())
        {
            return Err(
                "AI configuration requires absolute asset paths and explicit adapter identities"
                    .into(),
            );
        }
        // Do not start a runtime, inspect SDK terms or guess/replace a device.
        if !crate::enumerate_adapters()
            .iter()
            .any(|a| a.key == config.processing_adapter && a.name == name && a.backend == "Vulkan")
        {
            return Err("Configured AI Vulkan adapter is unavailable; no fallback".into());
        }
        Ok(config)
    }

    pub fn validate_selection(
        &self,
        ai: AiSettings,
        capability: &AiCapability,
    ) -> NativeResult<()> {
        match capability {
            AiCapability::Validated { settings, .. } if *settings == ai => {}
            AiCapability::PilotOnly { reason } | AiCapability::Unavailable { reason } => {
                return Err(reason.clone().into());
            }
            _ => {
                return Err(
                    "AI acceptance does not match the requested backend/model/scale".into(),
                );
            }
        }
        if !matches!(
            (&self.helper, ai),
            (PreviewHelper::Onnx(_), AiSettings::OnnxRuntime { .. })
                | (PreviewHelper::Rtx(_), AiSettings::RtxVideo { .. })
        ) {
            return Err("Native AI backend/configuration mismatch; no fallback".into());
        }
        Ok(())
    }

    pub fn preview_policy(
        &self,
        ai: AiSettings,
        source: Size,
        capability: &AiCapability,
    ) -> NativeResult<(PreviewPolicy, Size)> {
        self.validate_selection(ai, capability)?;
        let selected = ai.output_size(source, ResizeSpec::Original, capability)?;
        Ok((
            PreviewPolicy {
                ai,
                helper: self.helper.clone(),
                processing_adapter: self.processing_adapter.clone(),
                source_end: None,
            },
            selected,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CancellationToken, NativeJob, NativeSession, ProcessingRoute,
        ai_inference::OnnxHelperConfig,
    };
    use media_core::{
        ColorAdjustments, FrameRateSpec, OutputProfileId,
        ai_upscale::{AiGeometryLimits, AiModel, AiScale},
        equalization::Equalization,
    };

    #[test]
    #[ignore = "explicit local configuration and actual Vulkan adapter enumeration; no SDK execution"]
    fn actual_local_configuration_import_keeps_acceptance_separate() {
        let path = std::path::PathBuf::from(
            std::env::var_os("DIAXUS_TEST_AI_CONFIGURATION").expect("explicit configuration path"),
        );
        let config = NativeAiConfiguration::read(&path).unwrap();
        let settings = match &config.helper {
            PreviewHelper::Onnx(_) => ai(),
            PreviewHelper::Rtx(_) => AiSettings::RtxVideo {
                configuration: media_core::ai_upscale::RtxConfiguration::Sdk11SdrV1,
                scale: AiScale::Two,
            },
        };
        let pending = AiCapability::PilotOnly {
            reason: "ordinary acceptance pending".into(),
        };
        assert!(
            config
                .preview_policy(settings, Size::new(64, 36).unwrap(), &pending)
                .unwrap_err()
                .to_string()
                .contains("ordinary acceptance pending")
        );
        println!(
            "Local configuration imported for {}; runtime not executed and pending capability rejected",
            config.processing_adapter
        );
    }

    fn config() -> NativeAiConfiguration {
        NativeAiConfiguration {
            helper: PreviewHelper::Onnx(OnnxHelperConfig {
                executable: "missing-helper.exe".into(),
                runtime_directory: "missing-runtime".into(),
                model_cache: "missing-model".into(),
                dxgi_index: 0,
                exact_adapter_name: "unselected GPU".into(),
            }),
            processing_adapter: "unselected GPU".into(),
        }
    }
    fn ai() -> AiSettings {
        AiSettings::OnnxRuntime {
            model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
            scale: AiScale::Two,
        }
    }
    fn cap(settings: AiSettings) -> AiCapability {
        AiCapability::Validated {
            settings,
            limits: AiGeometryLimits {
                max_source: Size::new(64, 36).unwrap(),
                max_output: Size::new(128, 72).unwrap(),
                max_output_pixels: 128 * 72,
            },
        }
    }

    #[test]
    fn configuration_read_rejects_oversize_unknown_and_relative_assets_without_execution() {
        let path = std::env::temp_dir().join(format!(
            "diaxus-invalid-ai-config-{}.json",
            std::process::id()
        ));
        for (bytes, reason) in [
            (vec![b' ';65537], "64 KiB"),
            (br#"{"processing_adapter":"GPU","helper":{"backend":"cloud","configuration":{}}}"#.to_vec(), "unknown variant"),
            (br#"{"processing_adapter":"GPU","helper":{"backend":"onnx","configuration":{"executable":"missing.exe","runtime_directory":"missing","model_cache":"missing","dxgi_index":0,"exact_adapter_name":"GPU"}},"acceptance":true}"#.to_vec(), "unknown field"),
            (br#"{"processing_adapter":"GPU","helper":{"backend":"onnx","configuration":{"executable":"missing.exe","runtime_directory":"missing","model_cache":"missing","dxgi_index":0,"exact_adapter_name":"GPU"}}}"#.to_vec(), "absolute asset paths"),
        ] {
            std::fs::write(&path, bytes).unwrap();
            let error = NativeAiConfiguration::read(&path).unwrap_err().to_string();
            assert!(error.contains(reason),"{error}");
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn configured_jobs_reject_pending_mismatch_and_unselected_gpu_before_input_or_output() {
        let session = NativeSession::new(None).unwrap();
        let input = Path::new("missing-source.mp4");
        let output =
            std::env::temp_dir().join(format!("diaxus-config-reject-{}.mp4", std::process::id()));
        assert!(!output.exists());
        let config = config();
        let job = NativeJob {
            input,
            output: &output,
            resize: ResizeSpec::Original,
            profile: OutputProfileId::Mp4H264Aac,
            route: ProcessingRoute::SharedWgpu,
            bitrate: None,
            frame_rate: FrameRateSpec::Original,
            color: ColorAdjustments::default(),
            equalization: Equalization::default(),
            ai: ai(),
        };
        for (capability, reason) in [
            (
                AiCapability::PilotOnly {
                    reason: "release pending".into(),
                },
                "release pending",
            ),
            (
                cap(AiSettings::RtxVideo {
                    configuration: media_core::ai_upscale::RtxConfiguration::Sdk11SdrV1,
                    scale: AiScale::Two,
                }),
                "acceptance does not match",
            ),
            (cap(ai()), "processing GPU explicitly"),
        ] {
            let error = session
                .convert_configured_job(
                    job,
                    &config,
                    &capability,
                    &CancellationToken::default(),
                    |_| panic!("rejected job started"),
                )
                .err()
                .unwrap()
                .to_string();
            assert!(error.contains(reason), "{error}");
            assert!(!output.exists());
            assert!(!session.status().active);
        }
        assert!(
            config
                .preview_policy(ai(), Size::new(65, 36).unwrap(), &cap(ai()))
                .is_err()
        );
        let rtx = AiSettings::RtxVideo {
            configuration: media_core::ai_upscale::RtxConfiguration::Sdk11SdrV1,
            scale: AiScale::Two,
        };
        assert!(
            config
                .validate_selection(rtx, &cap(rtx))
                .unwrap_err()
                .to_string()
                .contains("backend/configuration mismatch")
        );
        session.shutdown();
    }

    #[test]
    fn off_ignores_invalid_ai_assets_and_keeps_ordinary_conversion() {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ai-upscale-vfr.mp4");
        let output =
            std::env::temp_dir().join(format!("diaxus-config-off-{}.mp4", std::process::id()));
        assert!(!output.exists());
        let session = NativeSession::new(None).unwrap();
        let job = NativeJob {
            input: &source,
            output: &output,
            resize: ResizeSpec::Original,
            profile: OutputProfileId::Mp4H264Aac,
            route: ProcessingRoute::DirectFfmpeg,
            bitrate: None,
            frame_rate: FrameRateSpec::Original,
            color: ColorAdjustments::default(),
            equalization: Equalization::default(),
            ai: AiSettings::Off,
        };
        let result = session
            .convert_configured_job(
                job,
                &config(),
                &AiCapability::Unavailable {
                    reason: "not installed".into(),
                },
                &CancellationToken::default(),
                |_| {},
            )
            .unwrap();
        assert_eq!(result.conversion.ai.source_frames, 0);
        assert_eq!(result.conversion.frames_processed, 4);
        assert!(output.exists());
        std::fs::remove_file(output).unwrap();
        session.shutdown();
    }

    #[test]
    fn diagnostic_reference_never_weakens_ordinary_off_or_route_policy() {
        let session = NativeSession::new(None).unwrap();
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ai-upscale-vfr.mp4");
        let output = std::env::temp_dir().join(format!(
            "diaxus-reference-reject-{}.mp4",
            std::process::id()
        ));
        assert!(!output.exists());
        let mut job = NativeJob {
            input: &source,
            output: &output,
            resize: ResizeSpec::Exact {
                width: 128,
                height: 72,
                preserve_aspect_ratio: true,
            },
            profile: OutputProfileId::Mp4H264Aac,
            route: ProcessingRoute::SharedWgpu,
            bitrate: None,
            frame_rate: FrameRateSpec::Original,
            color: ColorAdjustments::default(),
            equalization: Equalization::default(),
            ai: AiSettings::Off,
        };
        let error = session
            .convert_job(job, &CancellationToken::default(), |_| {})
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("would upscale"), "{error}");
        assert!(!output.exists());
        job.resize = ResizeSpec::Original;
        for route in [
            ProcessingRoute::DirectFfmpeg,
            ProcessingRoute::NvidiaFfmpeg,
            ProcessingRoute::SharedWgpuNvidia,
        ] {
            job.route = route;
            let error = session
                .convert_bilinear_pilot(job, AiScale::Two, &CancellationToken::default(), |_| {
                    panic!("bad reference route started")
                })
                .err()
                .unwrap()
                .to_string();
            assert!(error.contains("AI Off and explicit Shared GPU"), "{error}");
            assert!(!output.exists());
        }
        job.route = ProcessingRoute::SharedWgpu;
        job.ai = ai();
        assert!(
            session
                .convert_bilinear_pilot(
                    job,
                    AiScale::Four,
                    &CancellationToken::default(),
                    |_| panic!("reference started AI")
                )
                .is_err()
        );
        assert!(!session.status().active);
        session.shutdown();
    }
}
