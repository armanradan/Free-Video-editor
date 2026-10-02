use media_native::{NativeResult, load_adapter_preference, save_adapter_preference};
use std::path::PathBuf;

pub struct GpuPreferences {
    path: Option<PathBuf>,
}

impl GpuPreferences {
    pub fn new() -> Self {
        let path = std::env::var_os("DIAXUS_GPU_PREFERENCE")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                if cfg!(windows) {
                    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
                } else {
                    std::env::var_os("XDG_CONFIG_HOME")
                        .map(PathBuf::from)
                        .or_else(|| {
                            std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config"))
                        })
                }
                .filter(|path| !path.as_os_str().is_empty())
                .map(|directory| directory.join("Diaxus/gpu-preference.json"))
            });
        Self { path }
    }

    pub fn load(&self) -> NativeResult<Option<String>> {
        match &self.path {
            Some(path) => Ok(load_adapter_preference(path)?.map(|adapter| adapter.key)),
            None => Ok(None),
        }
    }

    pub fn save(&self, key: Option<&str>) -> NativeResult<()> {
        let path = self
            .path
            .as_ref()
            .ok_or("application configuration directory unavailable")?;
        if let Some(key) = key {
            save_adapter_preference(path, key)?;
        } else {
            // Automatic clears only this application's exact preference file.
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_corrupt_and_reset_preferences() {
        let directory =
            std::env::temp_dir().join(format!("diaxus-preferences-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("gpu.json");
        let store = GpuPreferences {
            path: Some(path.clone()),
        };
        store.save(None).unwrap();
        assert!(store.load().unwrap().is_none());
        std::fs::write(&path, "not JSON").unwrap();
        assert!(store.load().is_err());
        store.save(None).unwrap();
        assert!(store.load().unwrap().is_none());
        std::fs::remove_dir(directory).unwrap();
    }
}
