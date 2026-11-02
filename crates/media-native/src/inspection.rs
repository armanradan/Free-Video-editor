//! Session-local cache of checked metadata/timestamps, never pixel frames.
use crate::{CancellationToken, NativeResult, SourceInfo, inspect_source};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileStamp {
    path: PathBuf,
    length: u64,
    modified: SystemTime,
    created: Option<SystemTime>,
}

impl FileStamp {
    pub(crate) fn read(path: &Path) -> NativeResult<Self> {
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
    pub fn clear(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).take();
    }
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
    #[ignore = "explicit full local MPEG source: active inspection cancellation and fresh cache retry"]
    fn actual_cancelled_inspection_does_not_publish_cache_or_leave_children() {
        use std::{
            process::Command,
            thread,
            time::{Duration, Instant},
        };
        let source = PathBuf::from(
            std::env::var_os("DIAXUS_TEST_LEGACY_VIDEO_SOURCE").expect("absolute full source"),
        );
        let report = PathBuf::from(
            std::env::var_os("DIAXUS_TEST_LEGACY_VIDEO_REPORT").expect("new absolute report"),
        );
        let stamp = FileStamp::read(&source).unwrap();
        let cache = InspectionCache::default();
        let cancel = CancellationToken::default();
        let signal = cancel.clone();
        let timer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            signal.cancel();
        });
        let started = Instant::now();
        let error = cache
            .load(&source, &cancel)
            .err()
            .expect("cancelled inspection")
            .to_string();
        timer.join().unwrap();
        assert!(error.contains("cancelled"), "{error}");
        let elapsed = started.elapsed().as_millis();
        assert!(elapsed < 2000);
        assert!(cache.0.lock().unwrap().is_none());
        let (retry, reused) = cache.load(&source, &CancellationToken::default()).unwrap();
        assert!(!reused);
        assert_eq!(retry.source.frame_count, 6941);
        assert_eq!(stamp, FileStamp::read(&source).unwrap());
        cache.clear();
        drop(retry);
        #[cfg(windows)]
        {
            let output=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",
                "@(Get-CimInstance Win32_Process -Filter \"Name='ffprobe.exe'\" | Where-Object { $_.CommandLine.Contains($env:DIAXUS_TEST_LEGACY_VIDEO_SOURCE) }).Count"])
                .env("DIAXUS_TEST_LEGACY_VIDEO_SOURCE",&source).output().unwrap();
            assert!(output.status.success());
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0");
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(report)
            .unwrap();
        serde_json::to_writer_pretty(&mut file,&serde_json::json!({"status":"passed","cancel_delay_ms":100,
            "cancelled_inspection_return_ms":elapsed,"cancelled_cache_not_published":true,"fresh_retry_frame_count":6941,
            "retry_reused_cancelled_cache":false,"source_stamp_unchanged":true,"matching_ffprobe_children_remaining":0})).unwrap();
    }
    #[test]
    #[cfg(windows)]
    fn prepared_audio_is_shared_invalidated_and_released_after_consumers() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/native-aac");
        let directory =
            std::env::temp_dir().join(format!("diaxus-cache-audio-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let input = directory.join("input.mp4");
        fs::copy(root.join("leading-pts.mp4"), &input).unwrap();
        let token = CancellationToken::default();
        let cache = InspectionCache::default();
        let (first, reused) = cache.load(&input, &token).unwrap();
        assert!(!reused);
        let prepared = first.source.audio_input(&input).to_owned();
        assert_ne!(prepared, input);
        let (second, reused) = cache.load(&input, &token).unwrap();
        assert!(reused && Arc::ptr_eq(&first, &second));
        // Playback obtains the same owner through the bounded weak metadata cache.
        let playback = crate::audio_timing::prepare(&input, &token)
            .unwrap()
            .unwrap();
        assert_eq!(playback.path, prepared);
        fs::copy(root.join("reference.mp4"), &input).unwrap();
        assert!(first.check_unchanged(&input).is_err());
        let (replacement, reused) = cache.load(&input, &token).unwrap();
        assert!(!reused && replacement.source.audio_normalization.is_none());
        assert_eq!(replacement.source.audio_input(&input), input);
        drop(first);
        drop(second);
        cache.clear();
        assert!(prepared.exists());
        drop(playback);
        assert!(!prepared.exists());
        fs::remove_file(input).unwrap();
        fs::remove_dir(directory).unwrap();
    }
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
        let end = first.source.preview_end.as_ref().unwrap();
        end.check_source(&input).unwrap();
        assert_eq!(
            i128::from(end.position_us),
            first.timeline.last().unwrap() - first.source.video_start_us
        );
        use std::io::Write;
        fs::OpenOptions::new()
            .append(true)
            .open(&input)
            .unwrap()
            .write_all(b"changed")
            .unwrap();
        assert!(first.check_unchanged(&input).is_err());
        assert!(end.check_source(&input).is_err());
        let (third, reused) = cache.load(&input, &token).unwrap();
        assert!(!reused);
        assert!(!Arc::ptr_eq(&first, &third));
        third
            .source
            .preview_end
            .as_ref()
            .unwrap()
            .check_source(&input)
            .unwrap();
        token.cancel();
        assert!(cache.load(&input, &token).is_err());
        fs::remove_file(&input).unwrap();
        fs::remove_dir(&directory).unwrap();
    }
}
