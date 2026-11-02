//! Source-bound Windows WMA PCM and clock. FFmpeg consumes lossless PCM,
//! never its own WMA clock, after every payload and timestamp is verified.
use crate::{CancellationToken, CancellationWatch, ChildGuard, NativeResult};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, OnceLock, Weak},
};
const MAX_SCRATCH: u64 = 64 * 1024 * 1024;
const RESERVE: u64 = 32 * 1024 * 1024;
const MAX_BLOCK: u32 = 1024 * 1024;

#[derive(Clone, Debug, serde::Serialize)]
pub struct DecodedAudioCoverage {
    pub decoder: &'static str,
    pub start_us: i128,
    pub end_us: i128,
    pub frame_count: u64,
    pub sample_count: u64,
    pub sample_rate: u32,
    pub channels: u32,
    pub bits: u32,
    pub scratch_bytes: u64,
}
#[derive(Debug)]
pub(crate) struct PreparedAudio {
    directory: PathBuf,
    pub path: PathBuf,
    pub coverage: DecodedAudioCoverage,
}
impl Drop for PreparedAudio {
    fn drop(&mut self) {
        // Only paths in this owner's atomically reserved fresh directory.
        for name in ["audio.mka", "decoder.exe", "owner.txt"] {
            let _ = fs::remove_file(self.directory.join(name));
        }
        let _ = fs::remove_dir(&self.directory);
    }
}
pub(crate) fn prepare(path: &Path, cancel: &CancellationToken) -> NativeResult<Arc<PreparedAudio>> {
    struct Entry {
        stamp: crate::inspection::FileStamp,
        owner: Weak<PreparedAudio>,
    }
    static CACHE: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    cancel.check()?;
    let stamp = crate::inspection::FileStamp::read(path)?;
    {
        let lock = cache.lock().map_err(|_| "WMA cache poisoned")?;
        if let Some(owner) = lock
            .iter()
            .find(|e| e.stamp == stamp)
            .and_then(|e| e.owner.upgrade())
        {
            return Ok(owner);
        }
    }
    let owner = Arc::new(prepare_uncached(path, cancel)?);
    cancel.check()?;
    if crate::inspection::FileStamp::read(path)? != stamp {
        return Err("Source changed during Windows audio preparation; inspect again".into());
    }
    let mut lock = cache.lock().map_err(|_| "WMA cache poisoned")?;
    if let Some(existing) = lock
        .iter()
        .find(|e| e.stamp == stamp)
        .and_then(|e| e.owner.upgrade())
    {
        return Ok(existing);
    }
    lock.retain(|e| e.owner.strong_count() > 0);
    if lock.len() >= 32 {
        return Err("Too many live Windows audio sources (limit32)".into());
    }
    lock.push(Entry {
        stamp,
        owner: Arc::downgrade(&owner),
    });
    Ok(owner)
}
#[cfg(not(windows))]
fn prepare_uncached(_: &Path, _: &CancellationToken) -> NativeResult<PreparedAudio> {
    Err("WMA audio currently requires built-in Windows Media Foundation".into())
}
#[cfg(windows)]
fn prepare_uncached(path: &Path, cancel: &CancellationToken) -> NativeResult<PreparedAudio> {
    cancel.check()?;
    if crate::audio_timing::available_temp_space(cancel)? < MAX_SCRATCH + RESERVE {
        return Err("Windows WMA needs up to64MiB temporary storage plus32MiB free reserve".into());
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("diaxus-wma-{}-{now}", std::process::id()));
    fs::create_dir(&directory)?;
    let mut owned = PreparedAudio {
        path: directory.join("audio.mka"),
        directory,
        coverage: DecodedAudioCoverage {
            decoder: "Windows Media Foundation",
            start_us: 0,
            end_us: 0,
            frame_count: 0,
            sample_count: 0,
            sample_rate: 0,
            channels: 0,
            bits: 0,
            scratch_bytes: 0,
        },
    };
    fs::write(
        owned.directory.join("owner.txt"),
        b"Diaxus Windows WMA preparation v1\n",
    )?;
    let helper = include_bytes!(env!("DIAXUS_WMA_HELPER_BUILD_PATH"));
    let exe = owned.directory.join("decoder.exe");
    let mut executable = OpenOptions::new().write(true).create_new(true).open(&exe)?;
    executable.write_all(helper)?;
    drop(executable);
    let mut command = Command::new(&exe);
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000); // background helper, no console window
    command.arg(path).stdout(Stdio::piped());
    let mut child = ChildGuard::spawn(&mut command)?;
    let _watch = CancellationWatch::new(cancel, &[&child]);
    let mut input = BufReader::new(child.take_stdout()?);
    let mut output = LimitedWriter {
        file: OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&owned.path)?,
        written: helper.len() as u64 + 64,
    };
    let transcribed = transcribe(&mut input, &mut output, cancel);
    cancel.check()?;
    let (coverage, fingerprint) = transcribed.map_err(|error| {
        format!(
            "Windows WMA preparation: {error}; {}",
            child.diagnostic().trim()
        )
    })?;
    output.file.flush()?;
    owned.coverage = coverage;
    owned.coverage.scratch_bytes = output.written;
    drop(output);
    let status = child.wait_cancellable(cancel)?;
    if !status.success() || !child.diagnostic().trim().is_empty() {
        return Err(child.failure("Windows WMA decoder", status).into());
    }
    verify(&owned.path, fingerprint, cancel)?;
    cancel.check()?;
    Ok(owned)
}
struct LimitedWriter {
    file: File,
    written: u64,
}
impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .written
            .checked_add(bytes.len() as u64)
            .is_none_or(|n| n > MAX_SCRATCH)
        {
            return Err(std::io::Error::other(
                "Windows WMA exceeds64MiB scratch limit",
            ));
        }
        let n = self.file.write(bytes)?;
        self.written += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
fn u32le(reader: &mut impl Read) -> NativeResult<u32> {
    let mut b = [0; 4];
    reader.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}
fn i64le(reader: &mut impl Read) -> NativeResult<i64> {
    let mut b = [0; 8];
    reader.read_exact(&mut b)?;
    Ok(i64::from_le_bytes(b))
}
fn hash_record(hash: &mut Sha256, pts: i64, duration: i64, size: u32, payload: &[u8]) {
    hash.update(pts.to_le_bytes());
    hash.update(duration.to_le_bytes());
    hash.update(size.to_le_bytes());
    hash.update(payload);
}
fn transcribe(
    input: &mut impl Read,
    output: &mut impl Write,
    cancel: &CancellationToken,
) -> NativeResult<(DecodedAudioCoverage, [u8; 32])> {
    let mut magic = [0; 8];
    input.read_exact(&mut magic)?;
    if &magic != b"DXWMA001" {
        return Err("Unsupported Windows WMA protocol".into());
    }
    let rate = u32le(input)?;
    let channels = u32le(input)?;
    let bits = u32le(input)?;
    let align = u32le(input)?;
    if !(8000..=192000).contains(&rate)
        || !(1..=2).contains(&channels)
        || ![16, 24, 32].contains(&bits)
        || align != channels * (bits / 8)
    {
        return Err("Unsupported Windows WMA PCM format".into());
    }
    crate::frame_stream::audio_start(output, rate, channels, bits)?;
    let mut coverage = DecodedAudioCoverage {
        decoder: "Windows Media Foundation",
        start_us: 0,
        end_us: 0,
        frame_count: 0,
        sample_count: 0,
        sample_rate: rate,
        channels,
        bits,
        scratch_bytes: 0,
    };
    let mut hash = Sha256::new();
    let mut previous = None;
    let mut end = None;
    let mut eof = false;
    for _ in 0..2_000_000 {
        cancel.check()?;
        let pts = i64le(input)?;
        let duration = i64le(input)?;
        let size = u32le(input)?;
        let flags = u32le(input)?;
        if flags == 1 && pts == 0 && duration == 0 && size == 0 {
            eof = true;
            break;
        }
        if flags == 2 && size == 0 && duration == 0 && pts >= 0 {
            continue;
        }
        if flags != 0
            || size == 0
            || size > MAX_BLOCK
            || size % align != 0
            || pts < 0
            || duration <= 0
            || previous.is_some_and(|p| pts <= p)
        {
            return Err("Invalid Windows WMA block/timing or unsupported negative origin".into());
        }
        let samples = u64::from(size / align);
        let sample_ticks = i64::try_from(samples * 10_000_000 / u64::from(rate))?;
        if duration.abs_diff(sample_ticks) > 1 {
            return Err("Windows WMA duration disagrees with PCM length".into());
        }
        // Retain timestamps and samples. Sub-millisecond ASF quantization can
        // overlap adjacent buffers slightly; never flatten gaps into a clock.
        if end.is_some_and(|e: i64| e.saturating_sub(pts) > 20_000) {
            return Err("Windows WMA chunks overlap beyond2ms source quantization".into());
        }
        let next = pts
            .checked_add(duration)
            .ok_or("Windows audio endpoint overflow")?;
        let mut pcm = vec![0; size as usize];
        input.read_exact(&mut pcm)?;
        crate::frame_stream::frame_header(
            output,
            i128::from(pts),
            i128::from(duration),
            u64::from(size),
        )?;
        output.write_all(&pcm)?;
        hash_record(&mut hash, pts, duration, size, &Sha256::digest(&pcm));
        if coverage.frame_count == 0 {
            coverage.start_us = i128::from(pts) / 10;
        }
        coverage.end_us = (i128::from(next) + 5) / 10;
        coverage.frame_count += 1;
        coverage.sample_count = coverage
            .sample_count
            .checked_add(samples)
            .ok_or("WMA sample count overflow")?;
        previous = Some(pts);
        end = Some(next);
    }
    if !eof || coverage.frame_count == 0 {
        return Err("Windows WMA missing complete EOF or samples".into());
    }
    let mut extra = [0];
    if input.read(&mut extra)? != 0 {
        return Err("Trailing Windows WMA protocol bytes".into());
    }
    Ok((coverage, hash.finalize().into()))
}
/// Independent lossless demux checks exact PCM and exact 100ns PTS. PCM length
/// independently constrains each sample duration in the protocol parser.
fn verify(path: &Path, expected: [u8; 32], cancel: &CancellationToken) -> NativeResult<()> {
    let mut command = Command::new("ffprobe");
    command
        .args([
            "-v",
            "error",
            "-show_packets",
            "-show_streams",
            "-show_entries",
            "packet=pts,duration,size,data_hash:stream=time_base:stream_tags=:stream_disposition=",
            "-show_data_hash",
            "sha256",
            "-of",
            "compact=p=0:nk=0",
        ])
        .arg(path)
        .stdout(Stdio::piped());
    let mut child = ChildGuard::spawn(&mut command)?;
    let _watch = CancellationWatch::new(cancel, &[&child]);
    let mut input = BufReader::new(child.take_stdout()?);
    let mut hash = Sha256::new();
    let mut clock = false;
    for count in 0..=2_000_000 {
        cancel.check()?;
        let mut line = Vec::new();
        if input.by_ref().take(4097).read_until(b'\n', &mut line)? == 0 {
            break;
        }
        if line.len() > 4096 || count == 2_000_000 {
            return Err("WMA verification metadata limit".into());
        }
        let line = std::str::from_utf8(&line)?.trim();
        if line.is_empty() {
            continue;
        }
        if line == "time_base=1/10000000" && !clock {
            clock = true;
            continue;
        }
        let mut pts = None;
        let mut duration = None;
        let mut size = None;
        let mut digest = None;
        for field in line.split('|') {
            let (key, value) = field
                .split_once('=')
                .ok_or("Invalid WMA verification record")?;
            match key {
                "pts" => pts = Some(value.parse::<i64>()?),
                "duration" => duration = Some(value.parse::<i64>()?),
                "size" => size = Some(value.parse::<u32>()?),
                "data_hash" => {
                    digest = Some(
                        value
                            .strip_prefix("SHA256:")
                            .ok_or("WMA PCM hash algorithm")?,
                    )
                }
                _ => return Err("Unexpected WMA verification field".into()),
            }
        }
        let digest = digest.ok_or("Missing WMA PCM hash")?;
        if digest.len() != 64 || !digest.is_ascii() {
            return Err("Invalid WMA PCM hash".into());
        }
        let bytes = (0..32)
            .map(|n| u8::from_str_radix(&digest[n * 2..n * 2 + 2], 16))
            .collect::<Result<Vec<_>, _>>()?;
        hash_record(
            &mut hash,
            pts.ok_or("Missing WMA PCM PTS")?,
            duration.ok_or("Missing WMA PCM duration")?,
            size.ok_or("Missing WMA PCM size")?,
            &bytes,
        );
    }
    let status = child.wait_cancellable(cancel)?;
    if !status.success() || !child.diagnostic().trim().is_empty() {
        return Err(child.failure("WMA PCM verification", status).into());
    }
    let actual: [u8; 32] = hash.finalize().into();
    if !clock || actual != expected {
        return Err("Prepared WMA PCM payload/timestamps differ from Windows decoder".into());
    }
    Ok(())
}
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn preview_cancel_and_conversion_share_owner_across_source_replacement() {
        use crate::{NativeRunOptions, ProcessingRoute, convert_with_control_inner};
        use media_core::{OutputProfileId, ResizeSpec};
        use std::sync::atomic::Ordering;
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/native-codecs");
        let directory =
            std::env::temp_dir().join(format!("diaxus-wma-handoff-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let input = directory.join("input.wmv");
        fs::copy(root.join("wmv2-wmav2.wmv"), &input).unwrap();
        let token = CancellationToken::default();
        let cache = crate::inspection::InspectionCache::default();
        let (source, _) = cache.load(&input, &token).unwrap();
        let prepared = source.source.prepared_wma.as_ref().unwrap().clone();
        let playback_owner = prepare(&input, &token).unwrap();
        assert!(Arc::ptr_eq(&prepared, &playback_owner));
        let control = Arc::new(crate::playback::PlaybackControl::default());
        control.muted.store(true, Ordering::Release);
        let preview_cancel = CancellationToken::default();
        let result = crate::playback::play(&input, 0, &control, &preview_cancel, |_| {
            preview_cancel.cancel();
            Ok(())
        });
        assert!(result.unwrap_err().to_string().contains("cancelled"));
        assert!(prepared.path.is_file());
        let output = directory.join("output.mp4");
        let report = convert_with_control_inner(
            &input,
            &output,
            ResizeSpec::Original,
            OutputProfileId::Mp4H264Aac,
            ProcessingRoute::DirectFfmpeg,
            NativeRunOptions {
                inspection: Some(&cache),
                ..Default::default()
            },
            &token,
        )
        .unwrap();
        assert!(Arc::ptr_eq(
            report.input.prepared_wma.as_ref().unwrap(),
            &prepared
        ));
        fs::copy(root.join("wmv1-wmav1.wmv"), &input).unwrap();
        assert!(source.check_unchanged(&input).is_err());
        let (replacement, reused) = cache.load(&input, &token).unwrap();
        assert!(!reused);
        assert!(!Arc::ptr_eq(
            replacement.source.prepared_wma.as_ref().unwrap(),
            &prepared
        ));
        let old_directory = prepared.directory.clone();
        drop(source);
        drop(playback_owner);
        drop(prepared);
        cache.clear();
        assert!(old_directory.is_dir());
        drop(report);
        assert!(!old_directory.exists());
        let new_directory = replacement
            .source
            .prepared_wma
            .as_ref()
            .unwrap()
            .directory
            .clone();
        drop(replacement);
        assert!(!new_directory.exists());
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
        fs::remove_dir(directory).unwrap();
    }
    #[test]
    #[ignore = "explicit full local WMA source: cancel active preparation, fail helper and retry"]
    fn actual_active_preparation_cancel_failure_cleanup_and_retry() {
        use std::{
            sync::atomic::{AtomicBool, Ordering},
            thread,
            time::{Duration, Instant},
        };
        let source = PathBuf::from(
            std::env::var_os("DIAXUS_TEST_WMA_SOURCE").expect("absolute full WMA source"),
        );
        let report =
            PathBuf::from(std::env::var_os("DIAXUS_TEST_WMA_REPORT").expect("new absolute report"));
        let prefix = format!("diaxus-wma-{}-", std::process::id());
        let owned_directories = || {
            fs::read_dir(std::env::temp_dir())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
                .map(|entry| entry.path())
                .collect::<Vec<_>>()
        };
        assert!(owned_directories().is_empty());
        let stamp = crate::inspection::FileStamp::read(&source).unwrap();
        let cancel = CancellationToken::default();
        let signal = cancel.clone();
        let seen = Arc::new(AtomicBool::new(false));
        let worker_seen = seen.clone();
        let done = Arc::new(AtomicBool::new(false));
        let worker_done = done.clone();
        let worker_prefix = prefix.clone();
        let watcher = thread::spawn(move || {
            let started = Instant::now();
            while !worker_done.load(Ordering::Acquire)
                && started.elapsed() < Duration::from_secs(30)
            {
                for entry in fs::read_dir(std::env::temp_dir())
                    .unwrap()
                    .filter_map(Result::ok)
                {
                    if entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(&worker_prefix)
                        && fs::metadata(entry.path().join("audio.mka"))
                            .is_ok_and(|m| m.len() > 1024 * 1024)
                    {
                        worker_seen.store(true, Ordering::Release);
                        signal.cancel();
                        return;
                    }
                }
                thread::sleep(Duration::from_millis(1));
            }
        });
        let result = prepare(&source, &cancel);
        done.store(true, Ordering::Release);
        watcher.join().unwrap();
        assert!(
            seen.load(Ordering::Acquire),
            "Did not observe active PCM preparation"
        );
        assert!(result.unwrap_err().to_string().contains("cancelled"));
        assert!(owned_directories().is_empty());
        let bad =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/native-aac/reference.mp4");
        assert!(prepare(&bad, &CancellationToken::default()).is_err());
        assert!(owned_directories().is_empty());
        let audio = prepare(&source, &CancellationToken::default()).unwrap();
        let coverage = audio.coverage.clone();
        assert_eq!(stamp, crate::inspection::FileStamp::read(&source).unwrap());
        drop(audio);
        assert!(owned_directories().is_empty());
        #[cfg(windows)]
        {
            let output=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",
                "@(Get-CimInstance Win32_Process -Filter \"Name='decoder.exe'\" | Where-Object { $_.CommandLine.Contains($env:DIAXUS_WMA_OWNED_PREFIX) }).Count"])
                .env("DIAXUS_WMA_OWNED_PREFIX",&prefix).output().unwrap();
            assert!(output.status.success());
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0");
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(report)
            .unwrap();
        serde_json::to_writer_pretty(&mut file,&serde_json::json!({"status":"passed", "cancelled_after_pcm_exceeded_one_mib":true,
            "cancel_and_helper_failure_scratch_clean":true,"fresh_verified_retry":coverage,"retired_scratch_directories":0,
            "owned_decoder_processes_remaining":0,"source_stamp_unchanged":true})).unwrap();
    }
    #[test]
    #[ignore = "explicit local WMA source and redacted evidence destination"]
    fn actual_windows_wma_source_preparation() {
        let source = PathBuf::from(std::env::var_os("DIAXUS_TEST_WMA_SOURCE").expect("source"));
        let report = PathBuf::from(std::env::var_os("DIAXUS_TEST_WMA_REPORT").expect("report"));
        let token = CancellationToken::default();
        let before = crate::inspection::FileStamp::read(&source).unwrap();
        let started = std::time::Instant::now();
        let audio = prepare(&source, &token).unwrap();
        let coverage = audio.coverage.clone();
        let elapsed = started.elapsed().as_millis();
        let directory = audio.directory.clone();
        assert_eq!(before, crate::inspection::FileStamp::read(&source).unwrap());
        drop(audio);
        assert!(!directory.exists());
        let value = serde_json::json!({"status":"passed", "coverage":coverage, "preparation_ms":elapsed,
            "exact_pcm_and_100ns_pts_verified":true, "source_stamp_unchanged":true, "scratch_released":true});
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(report)
            .unwrap();
        serde_json::to_writer_pretty(&mut file, &value).unwrap();
    }
    fn protocol(records: &[(i64, i64, u32, u32)]) -> Vec<u8> {
        let mut bytes = b"DXWMA001".to_vec();
        for n in [48000_u32, 2, 16, 4] {
            bytes.extend(n.to_le_bytes());
        }
        for &(pts, duration, size, flags) in records {
            bytes.extend(pts.to_le_bytes());
            bytes.extend(duration.to_le_bytes());
            bytes.extend(size.to_le_bytes());
            bytes.extend(flags.to_le_bytes());
            if flags == 0 && size <= 4096 {
                bytes.resize(bytes.len() + size as usize, 0x2a);
            }
        }
        bytes
    }
    #[test]
    fn protocol_retains_gaps_and_rejects_corrupt_truncated_or_unbounded_records() {
        let token = CancellationToken::default();
        let good = protocol(&[
            (10_000_000, 10000, 192, 0),
            (20_000_000, 10000, 192, 0),
            (0, 0, 0, 1),
        ]);
        let (coverage, _) = transcribe(&mut good.as_slice(), &mut Vec::new(), &token).unwrap();
        assert_eq!(coverage.start_us, 1_000_000);
        assert_eq!(coverage.end_us, 2_001_000);
        assert_eq!(coverage.sample_count, 96);
        for records in [
            vec![(0, 10000, MAX_BLOCK + 4, 0)],
            vec![(-1, 10000, 192, 0)],
            vec![(0, 10000, 191, 0)],
            vec![(0, 1, 192, 0)],
            vec![(0, 10000, 192, 0), (0, 10000, 192, 0)],
            vec![(0, 10000, 192, 0)],
            vec![(0, 0, 0, 1)],
            vec![(0, 0, 0, 9)],
        ] {
            assert!(
                transcribe(&mut protocol(&records).as_slice(), &mut Vec::new(), &token).is_err()
            );
        }
        let mut truncated = good.clone();
        truncated.truncate(50);
        assert!(transcribe(&mut truncated.as_slice(), &mut Vec::new(), &token).is_err());
        let mut trailing = good.clone();
        trailing.push(1);
        assert!(transcribe(&mut trailing.as_slice(), &mut Vec::new(), &token).is_err());
        token.cancel();
        assert!(transcribe(&mut good.as_slice(), &mut Vec::new(), &token).is_err());
    }
    #[test]
    fn scratch_writer_enforces_ceiling_before_write() {
        let path = std::env::temp_dir().join(format!("diaxus-wma-bound-{}", std::process::id()));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let mut writer = LimitedWriter {
            file,
            written: MAX_SCRATCH - 1,
        };
        assert!(writer.write_all(&[1, 2]).is_err());
        assert_eq!(fs::metadata(&path).unwrap().len(), 0);
        drop(writer);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn changed_source_does_not_reuse_an_old_audio_owner() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/native-codecs");
        let path =
            std::env::temp_dir().join(format!("diaxus-wma-replace-{}.wmv", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(&fs::read(root.join("wmv1-wmav1.wmv")).unwrap())
            .unwrap();
        drop(file);
        let token = CancellationToken::default();
        let first = prepare(&path, &token).unwrap();
        fs::copy(root.join("wmv2-wmav2.wmv"), &path).unwrap();
        let second = prepare(&path, &token).unwrap();
        assert!(!Arc::ptr_eq(&first, &second));
        assert_ne!(first.coverage.sample_count, second.coverage.sample_count);
        drop(first);
        drop(second);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn windows_wma_preserves_origin_and_retires_scratch() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/native-codecs");
        for name in ["wmv1-wmav1.wmv", "wmv2-wmav2.wmv"] {
            let source = std::env::temp_dir()
                .join(format!("diaxus-wma-owner-{}-{name}", std::process::id()));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&source)
                .unwrap();
            file.write_all(&fs::read(root.join(name)).unwrap()).unwrap();
            drop(file);
            let token = CancellationToken::default();
            let audio = prepare(&source, &token).unwrap();
            assert_eq!(audio.coverage.start_us, 1_250_000);
            let frames = if name == "wmv1-wmav1.wmv" { 24 } else { 25 };
            assert_eq!(audio.coverage.sample_count, frames * 2048);
            assert_eq!(audio.coverage.frame_count, frames);
            let same = prepare(&source, &token).unwrap();
            assert!(Arc::ptr_eq(&same, &audio));
            let directory = audio.directory.clone();
            drop(audio);
            assert!(directory.is_dir());
            drop(same);
            assert!(!directory.exists());
            token.cancel();
            assert!(prepare(&root.join(name), &token).is_err());
            fs::remove_file(source).unwrap();
        }
    }
}
