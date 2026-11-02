//! Bounded AAC packet evidence. Classification never rewrites a source.
use crate::{CancellationToken, CancellationWatch, ChildGuard, NativeResult};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{
    fs,
    sync::{Arc, Mutex, OnceLock, Weak},
};

const MAX_PACKET_LINE: u64 = 4096;
const MAX_PACKETS: u64 = 2_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Decision {
    Unchanged,
    LeadingPts { before: i64, after: i64 },
}

#[derive(Clone, Copy, Debug)]
struct Packet {
    pts: i64,
    dts: i64,
    duration: i64,
    side_data: bool,
}

impl Packet {
    fn parse(line: &str) -> NativeResult<Self> {
        let mut pts = None;
        let mut dts = None;
        let mut duration = None;
        let mut side_data = false;
        for field in line.split('|').filter(|field| !field.is_empty()) {
            let (name, value) = field.split_once('=').ok_or("invalid AAC packet metadata")?;
            match name {
                "pts" => pts = Some(value.parse()?),
                "dts" => dts = Some(value.parse()?),
                "duration" => duration = Some(value.parse()?),
                name if name.starts_with("side_datum/") => side_data = true,
                "size" | "data_hash" => (),
                _ => return Err("unexpected AAC packet metadata".into()),
            }
        }
        let packet = Self {
            pts: pts.ok_or("AAC packet is missing PTS")?,
            dts: dts.ok_or("AAC packet is missing DTS")?,
            duration: duration.ok_or("AAC packet is missing duration")?,
            side_data,
        };
        if packet.duration <= 0 {
            return Err("AAC packet duration must be positive".into());
        }
        packet
            .dts
            .checked_add(packet.duration)
            .ok_or("AAC DTS overflow")?;
        packet
            .pts
            .checked_add(packet.duration)
            .ok_or("AAC PTS overflow")?;
        Ok(packet)
    }
}

#[derive(Default)]
struct Classifier {
    first: Option<Packet>,
    previous: Option<Packet>,
    count: u64,
    leading_fault: bool,
    all_later_pts_equal_dts: bool,
    side_data: bool,
    leading_decode_contiguous: bool,
}

impl Classifier {
    fn push(&mut self, packet: Packet) -> NativeResult<()> {
        if self.count >= MAX_PACKETS {
            return Err("AAC timing inspection exceeds 2000000 packets".into());
        }
        if let Some(previous) = self.previous {
            if packet.dts < previous.dts + previous.duration {
                return Err("AAC decode timestamps overlap or go backward".into());
            }
            if self.count == 1 {
                self.leading_fault = packet.pts <= previous.pts;
                self.leading_decode_contiguous = packet.dts == previous.dts + previous.duration;
                self.all_later_pts_equal_dts = packet.pts == packet.dts;
            } else {
                self.all_later_pts_equal_dts &= packet.pts == packet.dts;
                if packet.pts <= previous.pts {
                    return Err("Unsupported midstream AAC presentation timestamp fault".into());
                }
            }
        } else {
            self.first = Some(packet);
        }
        self.side_data |= packet.side_data;
        self.previous = Some(packet);
        self.count += 1;
        Ok(())
    }

    fn finish(&self) -> NativeResult<Decision> {
        let first = self.first.ok_or("AAC track has no packets")?;
        if !self.leading_fault {
            return Ok(Decision::Unchanged);
        }
        // Deliberately narrow: no negative priming, packet side data, or later
        // composition offsets can authorize replacing the first presentation.
        if self.count < 3
            || first.dts != 0
            || first.pts <= 0
            || !self.all_later_pts_equal_dts
            || self.side_data
            || !self.leading_decode_contiguous
        {
            return Err(
                "Ambiguous leading AAC timestamp fault; no automatic reconstruction".into(),
            );
        }
        Ok(Decision::LeadingPts {
            before: first.pts,
            after: first.dts,
        })
    }
}

struct Evidence {
    decision: Decision,
    fingerprint: [u8; 32],
    normalized_fingerprint: [u8; 32],
    payload_bytes: u64,
    end_us: u64,
}

fn scan(path: &Path, ignore_editlist: bool, cancel: &CancellationToken) -> NativeResult<Evidence> {
    cancel.check()?;
    let mut command = Command::new("ffprobe");
    command.args(["-v", "error"]);
    if ignore_editlist {
        command.args(["-ignore_editlist", "1"]);
    }
    command.args([
        "-select_streams", "a:0", "-show_packets", "-show_streams", "-show_entries",
        "packet=pts,dts,duration,size,data_hash:packet_side_data=side_data_type,skip_samples,discard_padding:stream=time_base:stream_disposition=:stream_tags=",
        "-show_data_hash", "sha256",
        "-of", "compact=p=0:nk=0",
    ]).arg(path).stdout(Stdio::piped());
    let mut child = ChildGuard::spawn(&mut command)?;
    let _watch = CancellationWatch::new(cancel, &[&child]);
    let mut reader = BufReader::new(child.take_stdout()?);
    let mut classifier = Classifier::default();
    let mut digest = Sha256::new();
    let mut normalized_digest = Sha256::new();
    let mut payload_bytes = 0u64;
    let mut time_bases = 0;
    let mut tick_scale = None;
    loop {
        cancel.check()?;
        let mut line = Vec::new();
        let length = reader
            .by_ref()
            .take(MAX_PACKET_LINE + 1)
            .read_until(b'\n', &mut line)?;
        if length == 0 {
            break;
        }
        if length as u64 > MAX_PACKET_LINE {
            return Err("AAC packet metadata exceeds 4096 bytes".into());
        }
        let text = std::str::from_utf8(&line)?.trim();
        if text.is_empty() {
            continue;
        }
        digest.update(text.as_bytes());
        digest.update(b"\n");
        if let Some(time_base) = text.strip_prefix("time_base=") {
            let (num, den) = time_base.split_once('/').ok_or("AAC time base missing")?;
            if num.parse::<u64>()? == 0 || den.parse::<u64>()? == 0 {
                return Err("AAC time base must be positive".into());
            }
            tick_scale = Some((num.parse::<i128>()?, den.parse::<i128>()?));
            normalized_digest.update(text.as_bytes());
            normalized_digest.update(b"\n");
            time_bases += 1;
            continue;
        }
        let packet = Packet::parse(text)?;
        let normalized = if classifier.count == 0 {
            text.replacen(
                &format!("pts={}|", packet.pts),
                &format!("pts={}|", packet.dts),
                1,
            )
        } else {
            text.to_owned()
        };
        normalized_digest.update(normalized.as_bytes());
        normalized_digest.update(b"\n");
        let size = text
            .split('|')
            .find_map(|s| s.strip_prefix("size="))
            .ok_or("AAC packet size missing")?
            .parse::<u64>()?;
        if size == 0 {
            return Err("AAC packet payload is empty".into());
        }
        payload_bytes = payload_bytes
            .checked_add(size)
            .ok_or("AAC payload size overflow")?;
        classifier.push(packet)?;
    }
    if !child.wait_cancellable(cancel)?.success() {
        return Err("AAC packet timeline inspection failed".into());
    }
    if time_bases != 1 {
        return Err("AAC inspection requires one exact audio time base".into());
    }
    let last = classifier.previous.ok_or("AAC track has no packets")?;
    let (num, den) = tick_scale.ok_or("AAC time base missing")?;
    let end_us = i128::from(last.pts)
        .checked_add(i128::from(last.duration))
        .and_then(|end| end.checked_mul(num))
        .and_then(|end| end.checked_mul(1_000_000))
        .ok_or("AAC endpoint overflow")?
        / den;
    Ok(Evidence {
        decision: classifier.finish()?,
        fingerprint: digest.finalize().into(),
        normalized_fingerprint: normalized_digest.finalize().into(),
        payload_bytes,
        end_us: u64::try_from(end_us)?,
    })
}

#[cfg(test)]
fn inspect(path: &Path, cancel: &CancellationToken) -> NativeResult<Decision> {
    Ok(inspect_evidence(path, cancel)?.decision)
}

fn inspect_evidence(path: &Path, cancel: &CancellationToken) -> NativeResult<Evidence> {
    let evidence = scan(path, false, cancel)?;
    if matches!(evidence.decision, Decision::LeadingPts { .. }) {
        // A correction cannot discard a meaningful MP4 edit list. Compare the
        // complete demuxed packet timeline with edit-list handling disabled.
        let raw = scan(path, true, cancel)?;
        if evidence.decision != raw.decision || evidence.fingerprint != raw.fingerprint {
            return Err("AAC edit-list timing conflicts with leading PTS correction".into());
        }
    }
    Ok(evidence)
}

const MAX_SCRATCH_BYTES: u64 = 64 * 1024 * 1024;
const CONTAINER_ALLOWANCE: u64 = 4 * 1024 * 1024;
const FREE_RESERVE: u64 = 32 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct PreparedAudio {
    directory: PathBuf,
    pub path: PathBuf,
    pub decision: Decision,
    pub end_us: u64,
}

impl Drop for PreparedAudio {
    fn drop(&mut self) {
        // The directory was atomically created by this owner, never supplied by
        // a caller. Do not traverse or delete any pre-existing session directory.
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(self.directory.join("owner.txt"));
        let _ = fs::remove_dir(&self.directory);
    }
}

fn admit_scratch(payload: u64, free: u64) -> NativeResult<u64> {
    let needed = payload
        .checked_add(CONTAINER_ALLOWANCE)
        .ok_or("AAC scratch size overflow")?;
    if needed > MAX_SCRATCH_BYTES {
        return Err("Audio timestamp normalization exceeds its 64 MiB scratch limit".into());
    }
    if free < needed + FREE_RESERVE {
        return Err("Insufficient temporary disk space for audio timestamp normalization".into());
    }
    Ok(needed)
}

#[cfg(windows)]
pub(crate) fn available_temp_space(cancel: &CancellationToken) -> NativeResult<u64> {
    // Fixed script: no file paths or user text interpolated into shell code.
    let mut command = Command::new("powershell.exe");
    command.args(["-NoProfile", "-NonInteractive", "-Command",
        "[IO.DriveInfo]::new([IO.Path]::GetPathRoot([IO.Path]::GetTempPath())).AvailableFreeSpace"])
        .stdout(Stdio::piped());
    let mut child = ChildGuard::spawn(&mut command)?;
    let _watch = CancellationWatch::new(cancel, &[&child]);
    let mut output = String::new();
    child.take_stdout()?.take(128).read_to_string(&mut output)?;
    if !child.wait_cancellable(cancel)?.success() {
        return Err("Could not determine temporary audio disk space".into());
    }
    Ok(output.trim().parse()?)
}

#[cfg(not(windows))]
pub(crate) fn available_temp_space(_cancel: &CancellationToken) -> NativeResult<u64> {
    Err("Automatic AAC preparation disk admission is currently evaluated on Windows only".into())
}

pub(crate) fn prepare(
    path: &Path,
    cancel: &CancellationToken,
) -> NativeResult<Option<Arc<PreparedAudio>>> {
    struct Entry {
        stamp: crate::inspection::FileStamp,
        prepared: Option<Weak<PreparedAudio>>,
    }
    // Bounded metadata and weak owners only. Another source must not evict a
    // live owner and cause preview/conversion to prepare different copies.
    static CACHE: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let stamp = crate::inspection::FileStamp::read(path)?;
    cancel.check()?;
    {
        let guard = cache.lock().map_err(|_| "AAC preparation cache poisoned")?;
        if let Some(entry) = guard.iter().find(|entry| entry.stamp == stamp) {
            match &entry.prepared {
                None => return Ok(None),
                Some(weak) => {
                    if let Some(owned) = weak.upgrade() {
                        return Ok(Some(owned));
                    }
                }
            }
        }
    }
    let prepared = prepare_uncached(path, cancel)?;
    if stamp != crate::inspection::FileStamp::read(path)? {
        return Err("Source changed during audio inspection; inspect it again".into());
    }
    let mut guard = cache.lock().map_err(|_| "AAC preparation cache poisoned")?;
    // Concurrent readers may have prepared the same source. Publish one owner
    // and release this duplicate through its normal cleanup path.
    if let Some(entry) = guard.iter().find(|entry| entry.stamp == stamp)
        && let Some(existing) = entry.prepared.as_ref().and_then(Weak::upgrade)
    {
        return Ok(Some(existing));
    }
    guard.retain(|entry| {
        entry
            .prepared
            .as_ref()
            .is_some_and(|weak| weak.strong_count() > 0)
    });
    if guard.len() >= 32 {
        return Err("Too many live prepared audio sources (limit32)".into());
    }
    guard.push(Entry {
        stamp,
        prepared: prepared.as_ref().map(Arc::downgrade),
    });
    Ok(prepared)
}

fn prepare_uncached(
    path: &Path,
    cancel: &CancellationToken,
) -> NativeResult<Option<Arc<PreparedAudio>>> {
    let before = fs::metadata(path)?;
    let evidence = inspect_evidence(path, cancel)?;
    if evidence.decision == Decision::Unchanged {
        return Ok(None);
    }
    let limit = admit_scratch(evidence.payload_bytes, available_temp_space(cancel)?)?;
    let mut owned = reserve_audio(evidence.decision)?;
    owned.end_us = evidence.end_us;
    copy_audio(path, &owned, limit, cancel)?;
    let verified = scan(&owned.path, false, cancel)?;
    if verified.decision != Decision::Unchanged
        || verified.fingerprint != evidence.normalized_fingerprint
    {
        return Err(
            "Prepared AAC packets differ beyond the authorized first PTS correction".into(),
        );
    }
    let after = fs::metadata(path)?;
    if before.len() != after.len()
        || before.modified()? != after.modified()?
        || before.created().ok() != after.created().ok()
    {
        return Err("Source changed during audio preparation; inspect it again".into());
    }
    cancel.check()?;
    Ok(Some(Arc::new(owned)))
}

fn reserve_audio(decision: Decision) -> NativeResult<PreparedAudio> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let parent = std::env::temp_dir();
    let directory = parent.join(format!("diaxus-audio-{}-{now}", std::process::id()));
    fs::create_dir(&directory)?;
    let owned = PreparedAudio {
        path: directory.join("audio.m4a"),
        directory,
        decision,
        end_us: 0,
    };
    fs::write(
        owned.directory.join("owner.txt"),
        "Diaxus audio timestamp preparation v1\n",
    )?;
    Ok(owned)
}

fn copy_audio(
    path: &Path,
    owned: &PreparedAudio,
    limit: u64,
    cancel: &CancellationToken,
) -> NativeResult<()> {
    cancel.check()?;
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-nostdin",
            "-n",
            "-copyts",
            "-i",
        ])
        .arg(path)
        .args([
            "-map",
            "0:a:0",
            "-c:a",
            "copy",
            "-bsf:a",
            "setts=pts=if(eq(N\\,0)\\,DTS\\,PTS)",
            "-fs",
            &limit.to_string(),
        ])
        .arg(&owned.path);
    let mut child = ChildGuard::spawn(&mut command)?;
    let _watch = CancellationWatch::new(cancel, &[&child]);
    let status = child.wait_cancellable(cancel)?;
    // The installed Windows launcher can report exit 0 for an FFmpeg error.
    // This command uses -v error, so any retained diagnostic is a failure too.
    if !status.success() || !child.diagnostic().trim().is_empty() {
        return Err(child.failure("Audio timestamp preparation", status).into());
    }
    if fs::metadata(&owned.path)?.len() > limit {
        return Err("Audio timestamp preparation exceeded its scratch reservation".into());
    }
    Ok(())
}

/// MPEG-PS duration headers can stop before the last audio packets. Preserve
/// their actual presentation coverage, including valid timestamp gaps.
pub(crate) fn packet_duration_us(
    path: &Path,
    time_base: &str,
    cancel: &CancellationToken,
) -> NativeResult<i128> {
    let (num, den) = crate::parse_rational(time_base)?;
    if num <= 0 || den <= 0 {
        return Err("Audio time base must be positive".into());
    }
    let mut command = Command::new("ffprobe");
    command
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_packets",
            "-show_entries",
            "packet=pts,duration:packet_side_data=",
            "-of",
            "compact=p=0:nk=0",
        ])
        .arg(path)
        .stdout(Stdio::piped());
    let mut child = ChildGuard::spawn(&mut command)?;
    let _watch = CancellationWatch::new(cancel, &[&child]);
    let mut reader = BufReader::new(child.take_stdout()?);
    let mut first = None;
    let mut previous = None;
    let mut end = None;
    let mut packets = 0u64;
    loop {
        cancel.check()?;
        let mut bytes = Vec::new();
        let length = reader
            .by_ref()
            .take(MAX_PACKET_LINE + 1)
            .read_until(b'\n', &mut bytes)?;
        if length == 0 {
            break;
        }
        if length as u64 > MAX_PACKET_LINE {
            return Err("Audio coverage metadata exceeds its line bound".into());
        }
        let line = std::str::from_utf8(&bytes)?.trim();
        if line.is_empty() {
            continue;
        }
        packets += 1;
        if packets > MAX_PACKETS {
            return Err("Audio coverage exceeds its packet bound".into());
        }
        let mut pts = None;
        let mut duration = None;
        for field in line.split('|') {
            let (key, value) = field
                .split_once('=')
                .ok_or("Malformed audio coverage metadata")?;
            match key {
                "pts" => pts = Some(value.parse::<i128>()?),
                "duration" => duration = Some(value.parse::<i128>()?),
                _ => return Err("Unexpected audio coverage metadata".into()),
            }
        }
        let pts = pts.ok_or("Missing audio presentation timestamp")?;
        let duration = duration
            .filter(|d| *d > 0)
            .ok_or("Missing positive audio packet duration")?;
        if previous.is_some_and(|previous| pts <= previous) {
            return Err("Audio presentation timestamps are not increasing".into());
        }
        first.get_or_insert(pts);
        previous = Some(pts);
        end = Some(
            pts.checked_add(duration)
                .ok_or("Audio packet endpoint overflow")?,
        );
    }
    let status = child.wait_cancellable(cancel)?;
    if !status.success() || !child.diagnostic().trim().is_empty() {
        return Err(child.failure("Audio packet coverage", status).into());
    }
    let ticks = end
        .ok_or("Audio has no packets")?
        .checked_sub(first.ok_or("Audio has no origin")?)
        .ok_or("Audio duration overflow")?;
    Ok(ticks
        .checked_mul(num)
        .and_then(|n| n.checked_mul(1_000_000))
        .and_then(|n| n.checked_add(den / 2))
        .ok_or("Audio duration overflow")?
        / den)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "explicit private original AAC source; bounded production preparation only, no SDK"]
    fn actual_original_preparation_cost_and_identity() {
        let source = PathBuf::from(
            std::env::var_os("DIAXUS_TEST_LEGACY_AAC_SOURCE").expect("explicit source"),
        );
        let report = PathBuf::from(
            std::env::var_os("DIAXUS_TEST_LEGACY_AAC_REPORT").expect("explicit report"),
        );
        let hash = |path: &Path| {
            let mut file = fs::File::open(path).unwrap();
            let mut digest = Sha256::new();
            let mut bytes = [0u8; 8192];
            loop {
                let n = file.read(&mut bytes).unwrap();
                if n == 0 {
                    break;
                }
                digest.update(&bytes[..n]);
            }
            digest.finalize()
        };
        let before = hash(&source);
        let started = std::time::Instant::now();
        let prepared = prepare(&source, &CancellationToken::default())
            .unwrap()
            .unwrap();
        let elapsed = started.elapsed().as_millis();
        assert_eq!(
            prepared.decision,
            Decision::LeadingPts {
                before: 3600,
                after: 0
            }
        );
        let bytes = fs::metadata(&prepared.path).unwrap().len();
        let end_us = prepared.end_us;
        let path = prepared.path.clone();
        assert_eq!(before, hash(&source));
        drop(prepared);
        assert!(!path.exists());
        fs::write(report, serde_json::to_vec_pretty(&serde_json::json!({
            "status":"passed", "source":"private original; path excluded", "production_prepare_ms":elapsed,
            "audio_scratch_bytes":bytes, "normalized_audio_endpoint_us":end_us,
            "original_unchanged":true, "scratch_released_after_drop":true,
            "scope":"Isolated preparation timing, not normal-window phase telemetry or a throughput benchmark"
        })).unwrap()).unwrap();
    }

    fn classify(lines: &[&str]) -> NativeResult<Decision> {
        let mut classifier = Classifier::default();
        for line in lines {
            classifier.push(Packet::parse(line)?)?;
        }
        classifier.finish()
    }

    #[test]
    fn distinguishes_single_leading_fault_from_ambiguous_timing() {
        assert_eq!(
            classify(&[
                "pts=3600|dts=0|duration=1024",
                "pts=1024|dts=1024|duration=1024",
                "pts=2048|dts=2048|duration=1024",
            ])
            .unwrap(),
            Decision::LeadingPts {
                before: 3600,
                after: 0
            }
        );
        for lines in [
            vec![
                "pts=3600|dts=0|duration=1024",
                "pts=1024|dts=1024|duration=1024",
            ],
            vec![
                "pts=0|dts=0|duration=1024",
                "pts=1024|dts=1024|duration=1024",
                "pts=1024|dts=2048|duration=1024",
            ],
            vec![
                "pts=0|dts=0|duration=1024",
                "pts=1024|dts=512|duration=1024",
            ],
            vec![
                "pts=3600|dts=-1024|duration=1024",
                "pts=0|dts=0|duration=1024",
                "pts=1024|dts=1024|duration=1024",
            ],
            vec!["pts=9223372036854775807|dts=0|duration=1024"],
            vec!["pts=N/A|dts=0|duration=1024"],
            vec!["pts=0|dts=0|duration=0"],
        ] {
            assert!(classify(&lines).is_err(), "{lines:?}");
        }
    }

    #[test]
    fn real_fixtures_keep_offsets_priming_and_gap() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/native-aac");
        let cancel = CancellationToken::default();
        for name in [
            "reference",
            "negative-priming",
            "positive-offset",
            "intentional-gap",
        ] {
            assert_eq!(
                inspect(&root.join(format!("{name}.mp4")), &cancel).unwrap(),
                Decision::Unchanged
            );
        }
        assert_eq!(
            inspect(&root.join("leading-pts.mp4"), &cancel).unwrap(),
            Decision::LeadingPts {
                before: 3600,
                after: 0
            }
        );
        cancel.cancel();
        assert!(inspect(&root.join("reference.mp4"), &cancel).is_err());
    }

    #[test]
    #[cfg(windows)]
    fn preparation_is_packet_verified_owned_and_bounded() {
        use std::io::Write;
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/native-aac");
        let source =
            std::env::temp_dir().join(format!("diaxus-aac-owned-test-{}.mp4", std::process::id()));
        let mut copy = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&source)
            .unwrap();
        copy.write_all(&fs::read(root.join("leading-pts.mp4")).unwrap())
            .unwrap();
        drop(copy);
        let hash = Sha256::digest(fs::read(&source).unwrap());
        let token = CancellationToken::default();
        for name in [
            "reference",
            "negative-priming",
            "positive-offset",
            "intentional-gap",
        ] {
            assert!(
                prepare(&root.join(format!("{name}.mp4")), &token)
                    .unwrap()
                    .is_none()
            );
        }
        let owned = prepare(&source, &token).unwrap().unwrap();
        assert_eq!(
            owned.decision,
            Decision::LeadingPts {
                before: 3600,
                after: 0
            }
        );
        assert_eq!(
            scan(&owned.path, false, &token).unwrap().fingerprint,
            scan(&root.join("reference.mp4"), false, &token)
                .unwrap()
                .fingerprint
        );
        let directory = owned.directory.clone();
        let retained = owned.clone();
        drop(owned);
        assert!(directory.is_dir());
        drop(retained);
        assert!(!directory.exists());
        assert_eq!(hash, Sha256::digest(fs::read(&source).unwrap()));
        fs::remove_file(&source).unwrap();
        assert!(admit_scratch(MAX_SCRATCH_BYTES, u64::MAX).is_err());
        assert!(admit_scratch(u64::MAX, u64::MAX).is_err());
        assert!(admit_scratch(1, 1).is_err());
        token.cancel();
        assert!(prepare(&source, &token).is_err());
    }

    #[test]
    #[cfg(windows)]
    fn failed_copy_and_cancel_release_only_owned_scratch() {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/native-aac/leading-pts.mp4");
        let owner = reserve_audio(Decision::LeadingPts {
            before: 3600,
            after: 0,
        })
        .unwrap();
        let directory = owner.directory.clone();
        // FFmpeg -n must fail on this already-created owned destination. The
        // original remains readable and cleanup never touches a user path.
        fs::write(&owner.path, b"existing owned partial").unwrap();
        assert!(
            copy_audio(
                &source,
                &owner,
                MAX_SCRATCH_BYTES,
                &CancellationToken::default()
            )
            .is_err()
        );
        drop(owner);
        assert!(!directory.exists());
        let owner = reserve_audio(Decision::LeadingPts {
            before: 3600,
            after: 0,
        })
        .unwrap();
        let directory = owner.directory.clone();
        let cancel = CancellationToken::default();
        cancel.cancel();
        assert!(copy_audio(&source, &owner, MAX_SCRATCH_BYTES, &cancel).is_err());
        drop(owner);
        assert!(!directory.exists());
        assert!(source.is_file());
    }
}
