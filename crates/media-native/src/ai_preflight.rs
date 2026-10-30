//! Deliberate local setup tests. These results never grant release capability.
use crate::{
    CancellationToken, NativeResult, ai_configuration::NativeAiConfiguration,
    ai_inference::HelperIdentity, ai_preview::PreviewHelper, ai_runtime::RuntimePaths,
};
use media_core::{
    Size,
    ai_upscale::{AiModel, AiScale, AiSettings, RtxConfiguration},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{io::Read, path::PathBuf};

const RTX_TERMS_SHA256: &str = "5addfcf7c78891a12f1dcd6b5f86c2512351ab71b3872f446ba10e1b6da54464";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SdkTerms {
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SetupInspection {
    pub issues: Vec<String>,
    pub terms: Option<SdkTerms>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SetupFrame {
    pub scale: u32,
    pub pattern: u32,
    pub output: Size,
    pub sha256: String,
    pub host_to_gpu_bytes: u64,
    pub gpu_to_host_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SetupPreflight {
    pub identities: Vec<HelperIdentity>,
    pub frames: Vec<SetupFrame>,
    pub processing_adapter: String,
    pub sdk_terms: Option<SdkTerms>,
    pub helper_sha256: String,
    pub release_accepted: bool,
}

impl NativeAiConfiguration {
    pub fn helper_sha256(&self) -> NativeResult<String> {
        let executable = match &self.helper {
            PreviewHelper::Onnx(c) => &c.executable,
            PreviewHelper::Rtx(c) => &c.executable,
        };
        executable_sha256(executable)
    }
    /// Read files/PE metadata only. Does not execute the configured helper.
    pub fn inspect_setup(&self) -> SetupInspection {
        let (paths, executable) = match &self.helper {
            PreviewHelper::Onnx(c) => (
                RuntimePaths {
                    onnx_directory: Some(c.runtime_directory.clone()),
                    rtx_sdk: None,
                },
                &c.executable,
            ),
            PreviewHelper::Rtx(c) => (
                RuntimePaths {
                    onnx_directory: None,
                    rtx_sdk: Some(c.sdk_root.clone()),
                },
                &c.executable,
            ),
        };
        let report = paths.inspect();
        let components = match &self.helper {
            PreviewHelper::Onnx(_) => report.onnx,
            PreviewHelper::Rtx(_) => report.rtx,
        };
        let mut issues: Vec<_> = components
            .into_iter()
            .filter(|c| c.status != "present-unvalidated")
            .map(|c| {
                format!(
                    "{}: {}",
                    c.path
                        .as_ref()
                        .map_or_else(String::new, |p| p.display().to_string()),
                    c.reason
                )
            })
            .collect();
        if !executable.is_absolute() || !executable.is_file() {
            issues.push(
                "Configured helper must be an existing absolute local executable path".into(),
            );
        }
        let terms = match self.read_sdk_terms() {
            Ok(terms) => terms,
            Err(error) => {
                issues.push(error.to_string());
                None
            }
        };
        SetupInspection { issues, terms }
    }

    fn read_sdk_terms(&self) -> NativeResult<Option<SdkTerms>> {
        let PreviewHelper::Rtx(c) = &self.helper else {
            return Ok(None);
        };
        let path = c.sdk_root.join("NVIDIA_RTX_Video_SDK_License.pdf");
        let mut bytes = Vec::new();
        std::fs::File::open(&path)?
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("SDK terms exceed the 8 MiB inspection bound".into());
        }
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        if sha256 != RTX_TERMS_SHA256 {
            return Err("SDK terms differ from the assessed SDK 1.1 package; review the package before execution".into());
        }
        Ok(Some(SdkTerms { path, sha256 }))
    }

    /// Caller supplies an explicit current GPU and, for RTX, the terms hash
    /// acknowledged for this invocation. No consent is read from configuration.
    pub fn test_setup(
        &self,
        selected_adapter: &str,
        acknowledged_terms: Option<&str>,
        cancel: &CancellationToken,
    ) -> NativeResult<SetupPreflight> {
        cancel.check()?;
        if self.processing_adapter != selected_adapter
            || !crate::enumerate_adapters().iter().any(|a| {
                let name = match &self.helper {
                    PreviewHelper::Onnx(c) => &c.exact_adapter_name,
                    PreviewHelper::Rtx(c) => &c.exact_adapter_name,
                };
                a.key == selected_adapter && a.name == *name && a.backend == "Vulkan"
            })
        {
            return Err(
                "Select the configuration's exact processing GPU explicitly; no fallback".into(),
            );
        }
        let inspection = self.inspect_setup();
        if !inspection.issues.is_empty() {
            return Err(inspection.issues.join(". ").into());
        }
        if let Some(terms) = &inspection.terms
            && acknowledged_terms != Some(terms.sha256.as_str())
        {
            return Err("Explicit acknowledgement of the inspected SDK terms is required before local RTX execution".into());
        }
        let source = Size::new(16, 12)?;
        let mut report = SetupPreflight {
            identities: Vec::new(),
            frames: Vec::new(),
            processing_adapter: selected_adapter.into(),
            sdk_terms: inspection.terms,
            helper_sha256: self.helper_sha256()?,
            release_accepted: false,
        };
        for (index, scale) in [AiScale::Two, AiScale::Four].into_iter().enumerate() {
            cancel.check()?;
            let settings = match &self.helper {
                PreviewHelper::Onnx(_) => AiSettings::OnnxRuntime {
                    model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
                    scale,
                },
                PreviewHelper::Rtx(_) => AiSettings::RtxVideo {
                    configuration: RtxConfiguration::Sdk11SdrV1,
                    scale,
                },
            };
            let generation = index as u64 + 1;
            let mut processor = self
                .helper
                .processor(settings, source, generation, cancel)?;
            report.identities.push(
                processor
                    .identity()
                    .ok_or("Setup helper returned no identity")?
                    .clone(),
            );
            let internal_scale = match self.helper {
                PreviewHelper::Onnx(_) => 4,
                PreviewHelper::Rtx(_) => scale.factor(),
            };
            let expected = Size::new(
                source.width * internal_scale,
                source.height * internal_scale,
            )?;
            for pattern in 0..2 {
                let mut rgba = Vec::with_capacity((source.width * source.height * 4) as usize);
                for y in 0..source.height {
                    for x in 0..source.width {
                        rgba.extend_from_slice(&[
                            ((17 * x + 13 * y + pattern * 79) % 256) as u8,
                            (((x / 3 + y / 5 + pattern) % 2) * 255) as u8,
                            ((7 * x * y + 31 + pattern * 101) % 256) as u8,
                            255,
                        ]);
                    }
                }
                let frame = processor.process_frame(rgba, generation, cancel)?;
                if frame.size != expected
                    || frame.device_generation != generation
                    || frame.rgba.len() != (expected.width * expected.height * 4) as usize
                    || frame.rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255)
                    || !frame
                        .rgba
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|p| p[..3].iter().any(|&v| v != 0))
                {
                    return Err("Setup output failed size/generation/opaque/nonblack checks".into());
                }
                report.frames.push(SetupFrame {
                    scale: scale.factor(),
                    pattern,
                    output: frame.size,
                    sha256: format!("{:x}", Sha256::digest(&frame.rgba)),
                    host_to_gpu_bytes: frame.host_to_gpu_bytes,
                    gpu_to_host_bytes: frame.gpu_to_host_bytes,
                });
            }
            if report.frames[report.frames.len() - 1].sha256
                == report.frames[report.frames.len() - 2].sha256
            {
                return Err("Distinct setup inputs produced identical outputs".into());
            }
            processor.close()?;
            cancel.check()?;
            if self.helper_sha256()? != report.helper_sha256 {
                return Err("Configured helper changed during setup testing; inspect again".into());
            }
        }
        Ok(report)
    }
}

pub(crate) fn executable_sha256(executable: &std::path::Path) -> NativeResult<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(executable)?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Configured helper exceeds the 64 MiB inspection bound".into());
    }
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_inspection_rejects_changed_terms_and_missing_assets_without_execution() {
        let root = std::env::temp_dir().join(format!("diaxus-setup-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let terms_path = root.join("NVIDIA_RTX_Video_SDK_License.pdf");
        std::fs::write(&terms_path, b"changed terms").unwrap();
        let config = NativeAiConfiguration {
            processing_adapter: "explicit GPU".into(),
            helper: PreviewHelper::Rtx(crate::rtx_inference::RtxHelperConfig {
                executable: root.join("must-not-execute.exe"),
                sdk_root: root.clone(),
                cache_parent: root.clone(),
                dxgi_index: 0,
                exact_adapter_name: "GPU".into(),
                expected_luid_high: 0,
                expected_luid_low: 0,
            }),
        };
        let inspected = config.inspect_setup();
        assert!(inspected.terms.is_none());
        assert!(inspected.issues.iter().any(|i| i.contains("terms differ")));
        assert!(inspected.issues.iter().any(|i| i.contains("helper")));
        assert!(
            config
                .test_setup("another GPU", None, &CancellationToken::default())
                .unwrap_err()
                .to_string()
                .contains("exact processing GPU")
        );
        let cancelled = CancellationToken::default();
        cancelled.cancel();
        assert!(config.test_setup("explicit GPU", None, &cancelled).is_err());
        std::fs::File::create(&terms_path)
            .unwrap()
            .set_len(8 * 1024 * 1024 + 1)
            .unwrap();
        assert!(
            config
                .read_sdk_terms()
                .unwrap_err()
                .to_string()
                .contains("inspection bound")
        );
        std::fs::remove_file(terms_path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    #[ignore = "explicit local DirectML helper/runtime/model and selected Vulkan GPU"]
    fn actual_onnx_setup_test_keeps_internal_scale_and_release_gates_explicit() {
        let helper = crate::ai_inference::tests::actual_config();
        let adapter = crate::enumerate_adapters()
            .into_iter()
            .find(|a| a.name == helper.exact_adapter_name && a.backend == "Vulkan")
            .unwrap();
        let config = NativeAiConfiguration {
            helper: helper.into(),
            processing_adapter: adapter.key,
        };
        let result = config
            .test_setup(
                &config.processing_adapter,
                None,
                &CancellationToken::default(),
            )
            .unwrap();
        assert!(result.sdk_terms.is_none());
        assert!(!result.release_accepted);
        assert_eq!(result.frames.len(), 4);
        // Portable model computes internal x4 for either requested scale.
        assert!(
            result
                .frames
                .iter()
                .all(|f| f.output == Size::new(64, 48).unwrap())
        );
        let destination = PathBuf::from(std::env::var_os("DIAXUS_TEST_SETUP_REPORT").unwrap());
        std::fs::write(destination, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    }

    #[test]
    #[ignore = "explicit local RTX SDK execution; requires acknowledged terms and configured GPU"]
    fn actual_rtx_setup_test_checks_consent_cancel_identity_output_and_cleanup() {
        let path = PathBuf::from(std::env::var_os("DIAXUS_TEST_AI_CONFIGURATION").unwrap());
        let config = NativeAiConfiguration::read(&path).unwrap();
        assert!(matches!(config.helper, PreviewHelper::Rtx(_)));
        let adapter = config.processing_adapter.clone();
        let inspection = config.inspect_setup();
        assert!(inspection.issues.is_empty(), "{:?}", inspection.issues);
        let terms = inspection.terms.unwrap();
        for consent in [None, Some("outdated terms")] {
            let error = config
                .test_setup(&adapter, consent, &CancellationToken::default())
                .unwrap_err();
            assert!(error.to_string().contains("acknowledgement"), "{error}");
        }
        assert!(
            config
                .test_setup(
                    "another GPU",
                    Some(&terms.sha256),
                    &CancellationToken::default()
                )
                .unwrap_err()
                .to_string()
                .contains("exact processing GPU")
        );
        let cancelled = CancellationToken::default();
        cancelled.cancel();
        assert!(
            config
                .test_setup(&adapter, Some(&terms.sha256), &cancelled)
                .is_err()
        );
        let result = config
            .test_setup(&adapter, Some(&terms.sha256), &CancellationToken::default())
            .unwrap();
        assert!(!result.release_accepted);
        assert_eq!(result.frames.len(), 4);
        let destination = PathBuf::from(std::env::var_os("DIAXUS_TEST_SETUP_REPORT").unwrap());
        std::fs::write(destination, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
        let pending = media_core::ai_upscale::AiCapability::PilotOnly {
            reason: "ordinary window acceptance pending".into(),
        };
        let ai = AiSettings::RtxVideo {
            configuration: RtxConfiguration::Sdk11SdrV1,
            scale: AiScale::Two,
        };
        assert!(config.validate_selection(ai, &pending).is_err());
        let mut changed = config;
        if let PreviewHelper::Rtx(c) = &mut changed.helper {
            c.sdk_root = std::env::temp_dir().join("missing-diaxus-sdk");
        }
        assert!(!changed.inspect_setup().issues.is_empty());
        assert!(
            changed
                .test_setup(&adapter, Some(&terms.sha256), &CancellationToken::default())
                .is_err()
        );
    }
}
