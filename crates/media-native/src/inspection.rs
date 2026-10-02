//! Session-local cache of checked metadata/timestamps, never pixel frames.
use crate::{CancellationToken, NativeResult, SourceInfo, inspect_source};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileStamp {
    path: PathBuf,
    length: u64,
    modified: SystemTime,
    created: Option<SystemTime>,
}

impl FileStamp {
    fn read(path: &Path) -> NativeResult<Self> {
        let path = fs::canonicalize(path)?;
        let metadata = fs::metadata(&path)?;
        if !metadata.is_file() {
            return Err("input is not a file".into());
        }
        Ok(Self {
            path,
            length: metadata.len(),
            modified: metadata.modified()?,
            created: metadata.created().ok(),
        })
    }
}

pub(crate) struct InspectedSource {
    stamp: FileStamp,
    pub source: SourceInfo,
    pub timeline: Vec<i128>,
}

impl InspectedSource {
    pub fn check_unchanged(&self, path: &Path) -> NativeResult<()> {
        if self.stamp != FileStamp::read(path)? {
            return Err("source changed during inspection/conversion; inspect it again".into());
        }
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct InspectionCache(Mutex<Option<Arc<InspectedSource>>>);

impl InspectionCache {
    pub fn load(
        &self,
        path: &Path,
        cancel: &CancellationToken,
    ) -> NativeResult<(Arc<InspectedSource>, bool)> {
        cancel.check()?;
        let stamp = FileStamp::read(path)?;
        {
            let cached = self
                .0
                .lock()
                .map_err(|_| "inspection cache lock was poisoned")?;
            if let Some(value) = cached.as_ref().filter(|value| value.stamp == stamp) {
                return Ok((value.clone(), true));
            }
        }
        // Inspect with broad SDR depth policy; each job still gates its profile.
        let (source, timeline) = inspect_source(path, false, true, cancel)?;
        cancel.check()?;
        let value = Arc::new(InspectedSource {
            stamp,
            source,
            timeline,
        });
        value.check_unchanged(path)?;
        *self
            .0
            .lock()
            .map_err(|_| "inspection cache lock was poisoned")? = Some(value.clone());
        Ok((value, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reuse_and_invalidation_do_not_retain_pixels() {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m35-vfr-offset.mp4");
        let directory = std::env::temp_dir().join(format!("diaxus-cache-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let input = directory.join("input.mp4");
        fs::copy(source, &input).unwrap();
        let cache = InspectionCache::default();
        let token = CancellationToken::default();
        let (first, reused) = cache.load(&input, &token).unwrap();
        assert!(!reused);
        let (second, reused) = cache.load(&input, &token).unwrap();
        assert!(reused);
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(second.timeline.len(), 36);
        use std::io::Write;
        fs::OpenOptions::new()
            .append(true)
            .open(&input)
            .unwrap()
            .write_all(b"changed")
            .unwrap();
        assert!(first.check_unchanged(&input).is_err());
        let (third, reused) = cache.load(&input, &token).unwrap();
        assert!(!reused);
        assert!(!Arc::ptr_eq(&first, &third));
        token.cancel();
        assert!(cache.load(&input, &token).is_err());
        fs::remove_file(&input).unwrap();
        fs::remove_dir(&directory).unwrap();
    }
}
