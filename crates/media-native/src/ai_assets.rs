//! Explicit local import for one allowlisted model. Constructing the cache does
//! no IO; no runtime, model, toolchain or driver is installed by these functions.
use crate::{CancellationToken, NativeResult};
use media_core::ai_upscale::{ASSET_MANIFEST_JSON, AiModel};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static IMPORT_SERIAL: AtomicU64 = AtomicU64::new(0);

pub struct ModelCache {
    directory: PathBuf,
}

struct PendingFile(PathBuf);
impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

impl ModelCache {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    pub fn model_path(&self) -> PathBuf {
        self.directory.join(format!(
            "{}.onnx",
            AiModel::RealEsrGeneralX4v3StrongFp32V1.onnx_sha256()
        ))
    }

    pub fn read_onnx(&self, cancel: &CancellationToken) -> NativeResult<Vec<u8>> {
        read_verified(&self.model_path(), cancel)
    }

    /// The caller shows bytes/source/license before invoking this explicit action.
    /// Existing invalid cache entries require an explicit remove action.
    pub fn import_onnx(&self, source: &Path, cancel: &CancellationToken) -> NativeResult<PathBuf> {
        let bytes = read_verified(source, cancel)?;
        let target = self.model_path();
        if target.exists() {
            self.read_onnx(cancel)?;
            return Ok(target);
        }
        cancel.check()?;
        fs::create_dir_all(&self.directory)?;
        let pending_path = self.directory.join(format!(
            ".model-{}-{}.partial",
            std::process::id(),
            IMPORT_SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending_path)?;
        let pending = PendingFile(pending_path);
        for chunk in bytes.chunks(65536) {
            cancel.check()?;
            file.write_all(chunk)?;
        }
        file.sync_all()?;
        drop(file);
        cancel.check()?;
        if let Err(error) = fs::rename(&pending.0, &target) {
            // A competing valid importer may already have published this exact hash.
            if target.exists() {
                self.read_onnx(cancel)?;
            } else {
                return Err(error.into());
            }
        }
        Ok(target)
    }

    pub fn remove_onnx(&self) -> NativeResult<bool> {
        match fs::remove_file(self.model_path()) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

fn read_verified(path: &Path, cancel: &CancellationToken) -> NativeResult<Vec<u8>> {
    cancel.check()?;
    let manifest: serde_json::Value = serde_json::from_str(ASSET_MANIFEST_JSON)?;
    let expected_bytes = manifest["model"]["bytes"]
        .as_u64()
        .ok_or("incompatible AI model manifest")?;
    if expected_bytes > 8 * 1024 * 1024 {
        return Err("AI model exceeds the allowlisted acquisition limit".into());
    }
    let mut file = File::open(path)?;
    if file.metadata()?.len() != expected_bytes {
        return Err("AI model size mismatch; explicitly import the validated model.onnx".into());
    }
    let mut bytes = Vec::with_capacity(expected_bytes as usize);
    let mut hasher = Sha256::new();
    let mut chunk = [0; 65536];
    loop {
        cancel.check()?;
        let count = file.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        if bytes.len() + count > expected_bytes as usize {
            return Err("AI model changed size during import".into());
        }
        hasher.update(&chunk[..count]);
        bytes.extend_from_slice(&chunk[..count]);
    }
    let hash = format!("{:x}", hasher.finalize());
    if bytes.len() != expected_bytes as usize
        || hash != AiModel::RealEsrGeneralX4v3StrongFp32V1.onnx_sha256()
    {
        return Err(
            "AI model checksum mismatch; remove the invalid cache and import the validated export"
                .into(),
        );
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_import_performs_no_filesystem_work() {
        let path =
            std::env::temp_dir().join(format!("diaxus-ai-cache-cancel-{}", std::process::id()));
        assert!(!path.exists());
        let cache = ModelCache::new(&path);
        let cancelled = CancellationToken::default();
        cancelled.cancel();
        assert!(
            cache
                .import_onnx(Path::new("missing-model.onnx"), &cancelled)
                .is_err()
        );
        assert!(!path.exists());
    }

    #[test]
    fn cache_construction_and_missing_read_create_nothing() {
        let path =
            std::env::temp_dir().join(format!("diaxus-ai-cache-missing-{}", std::process::id()));
        assert!(!path.exists());
        let cache = ModelCache::new(&path);
        assert!(cache.read_onnx(&CancellationToken::default()).is_err());
        assert!(!cache.remove_onnx().unwrap());
        assert!(!path.exists());
    }
}
