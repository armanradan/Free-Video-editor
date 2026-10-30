//! Local setup choices, never persisted runtime results or release capability.
use media_native::{
    NativeResult, ai_configuration::NativeAiConfiguration, ai_preflight::SetupInspection,
};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedSetup {
    pub configuration: NativeAiConfiguration,
    pub terms_sha256: Option<String>,
    // Permission to recheck this exact helper when opening AI setup. A fresh
    // in-memory test result is still required each session; JSON grants none.
    pub approved_helper_sha256: Option<String>,
}

impl SavedSetup {
    pub fn can_resume(
        &self,
        config: &NativeAiConfiguration,
        inspection: &SetupInspection,
        helper_hash: &str,
    ) -> bool {
        self.configuration == *config
            && inspection.issues.is_empty()
            && self.terms_sha256.as_deref() == inspection.terms.as_ref().map(|t| t.sha256.as_str())
            && self.approved_helper_sha256.as_deref() == Some(helper_hash)
    }
}

pub struct AiPreferences {
    path: Option<PathBuf>,
}
impl AiPreferences {
    /// Deployment supplies component locations as an application asset, never
    /// consent or a runtime result. No SDK/model binary is bundled by this code.
    pub fn load_initial(&self) -> NativeResult<Option<SavedSetup>> {
        if let Some(saved) = self.load()? {
            return Ok(Some(saved));
        }
        let path = std::env::current_exe()?
            .parent()
            .ok_or("Application directory unavailable")?
            .join("assets/native-rtx.json");
        if !path.is_file() {
            return Ok(None);
        }
        Ok(Some(SavedSetup {
            configuration: NativeAiConfiguration::read(&path)?,
            terms_sha256: None,
            approved_helper_sha256: None,
        }))
    }
    pub fn new() -> Self {
        let directory = if cfg!(windows) {
            std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
        };
        Self {
            path: directory
                .filter(|p| !p.as_os_str().is_empty())
                .map(|p| p.join("Diaxus/ai-setup.json")),
        }
    }
    pub fn load(&self) -> NativeResult<Option<SavedSetup>> {
        let Some(path) = &self.path else {
            return Ok(None);
        };
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let mut bytes = Vec::new();
        file.take(65537).read_to_end(&mut bytes)?;
        if bytes.len() > 65536 {
            return Err("Saved AI setup exceeds 64 KiB; import configuration again".into());
        }
        Ok(Some(serde_json::from_slice(&bytes)?))
    }
    pub fn save(&self, setup: Option<&SavedSetup>) -> NativeResult<()> {
        let path = self
            .path
            .as_ref()
            .ok_or("Application configuration directory unavailable")?;
        let Some(setup) = setup else {
            return match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.into()),
            };
        };
        let bytes = serde_json::to_vec_pretty(setup)?;
        if bytes.len() > 65536 {
            return Err("AI setup exceeds 64 KiB".into());
        }
        std::fs::create_dir_all(path.parent().ok_or("AI preference directory unavailable")?)?;
        // Same-directory rename returned ERROR_NOT_SAME_DEVICE in the actual
        // encrypted Windows app-data folder. Write this small preference file
        // directly; an interrupted/truncated write fails the strict bounded
        // reader and can never restore consent or a runtime result.
        let mut file = std::fs::File::create(path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_setup_is_bounded_and_requires_exact_current_approval() {
        let directory =
            std::env::temp_dir().join(format!("diaxus-ai-prefs-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let store = AiPreferences {
            path: Some(directory.join("ai.json")),
        };
        store.save(None).unwrap();
        assert!(store.load().unwrap().is_none());
        let config = NativeAiConfiguration {
            processing_adapter: "explicit-gpu".into(),
            helper: media_native::ai_preview::PreviewHelper::Rtx(
                media_native::rtx_inference::RtxHelperConfig {
                    executable: directory.join("helper.exe"),
                    sdk_root: directory.join("sdk"),
                    cache_parent: directory.join("cache"),
                    dxgi_index: 1,
                    exact_adapter_name: "RTX".into(),
                    expected_luid_high: 0,
                    expected_luid_low: 7,
                },
            ),
        };
        let mut saved = SavedSetup {
            configuration: config.clone(),
            terms_sha256: Some("terms-a".into()),
            approved_helper_sha256: Some("helper-a".into()),
        };
        store.save(Some(&saved)).unwrap();
        let inspection = SetupInspection {
            issues: vec![],
            terms: Some(media_native::ai_preflight::SdkTerms {
                path: directory.join("terms.pdf"),
                sha256: "terms-a".into(),
            }),
        };
        assert!(
            store
                .load()
                .unwrap()
                .unwrap()
                .can_resume(&config, &inspection, "helper-a")
        );
        let mut changed = config.clone();
        changed.processing_adapter = "other-gpu".into();
        assert!(!saved.can_resume(&changed, &inspection, "helper-a"));
        assert!(!saved.can_resume(&config, &inspection, "helper-b"));
        let mut changed_terms = inspection.clone();
        changed_terms.terms.as_mut().unwrap().sha256 = "terms-b".into();
        assert!(!saved.can_resume(&config, &changed_terms, "helper-a"));
        saved.terms_sha256 = None;
        saved.approved_helper_sha256 = None;
        store.save(Some(&saved)).unwrap(); // Revoke must overwrite old approval.
        assert!(
            !store
                .load()
                .unwrap()
                .unwrap()
                .can_resume(&config, &inspection, "helper-a")
        );
        std::fs::write(store.path.as_ref().unwrap(), vec![b' '; 65537]).unwrap();
        assert!(store.load().is_err());
        std::fs::write(store.path.as_ref().unwrap(), b"invalid").unwrap();
        assert!(store.load().is_err());
        store.save(None).unwrap();
        assert!(store.load().unwrap().is_none());
        std::fs::remove_dir(directory).unwrap();
    }
}
