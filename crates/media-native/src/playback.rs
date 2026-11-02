//! Independent low-resolution player; conversion never uses these sampled frames.
use crate::{
    CancellationToken, CancellationWatch, ChildGuard, NativeResult,
    preview::{self, PreviewFrame},
};
use cpal::{
    FromSample, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::{
    collections::VecDeque,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const FPS: u64 = 15;
pub const AUDIO_BUFFER_MS: u64 = 250;

#[derive(Default)]
pub struct PlaybackControl {
    pub paused: AtomicBool,
    pub muted: AtomicBool,
    pub ready: AtomicBool,
    pub ended: AtomicBool,
    pub position_us: AtomicU64,
    pub duration_us: AtomicU64,
    pub audio_active: AtomicBool,
    pub frames_presented: AtomicU64,
    pub frames_dropped: AtomicU64,
    pub audio_samples: AtomicU64,
    pub audio_underruns: AtomicU64,
    pub audio_peak_samples: AtomicU64,
    pub audio_peak_amplitude: AtomicU64,
    pub error: Mutex<Option<String>>,
}

pub fn play(
    path: &Path,
    requested_us: u64,
    control: &Arc<PlaybackControl>,
    cancel: &CancellationToken,
    mut present: impl FnMut(PreviewFrame) -> NativeResult<()>,
) -> NativeResult<()> {
    let mut info = preview::probe_info(path, cancel)?;
    // Retain the same source-bound audio owner used by conversion until the
    // audio worker has stopped. Healthy AAC needs no intermediate.
    let prepared_audio = if info.has_aac {
        crate::audio_timing::prepare(path, cancel)?
    } else {
        None
    };
    if let Some(audio) = &prepared_audio {
        // A malformed leading PTS can inflate the original container duration.
        // The corrected audio packet endpoint and video track coverage define
        // the normalized preview endpoint, not that stale container header.
        info.duration_us = info
            .video_end_us
            .ok_or("Normalized preview requires known video coverage")?
            .max(audio.end_us);
    }
    let prepared_wma = if info.has_wma {
        // WMA playback uses exactly the inspected Windows PCM owner and video
        // endpoints, including the reported WMV terminal-duration policy.
        let (source, _) = crate::inspect_source(path, false, true, cancel)?;
        let video_end = source
            .video_start_us
            .checked_add(
                source
                    .video_duration_us
                    .ok_or("Missing WMA preview video duration")?,
            )
            .ok_or("WMA preview endpoint overflow")?;
        let audio = source
            .prepared_wma
            .ok_or("Missing Windows WMA audio plan")?;
        let endpoint = video_end
            .max(audio.coverage.end_us)
            .checked_sub(i128::from(info.format_start_us))
            .ok_or("WMA preview origin overflow")?;
        info.duration_us = u64::try_from(endpoint)?;
        Some(audio)
    } else {
        None
    };
    let start_us = requested_us.min(info.duration_us.saturating_sub(1_000_000 / FPS));
    control
        .duration_us
        .store(info.duration_us, Ordering::Release);
    control.position_us.store(start_us, Ordering::Release);
    let remaining = seconds(info.duration_us - start_us);
    let normalization = info
        .normalization
        .map(|(coded, normalized)| crate::display_geometry::normalization_filter(coded, normalized))
        .unwrap_or_default();
    let seek = seconds(start_us);
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-ss",
            &seek,
            "-i",
        ])
        .arg(path)
        .args([
            "-map",
            "0:v:0",
            "-an",
            "-sn",
            "-dn",
            "-vf",
            &format!(
                "{normalization}fps={FPS}:start_time=0,{},tpad=stop_mode=clone:stop_duration={remaining}",
                preview::display_filter(info.input_color)
            ),
            "-t",
            &remaining,
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .stdout(Stdio::piped());
    let mut video = ChildGuard::spawn(&mut command)?;
    let watch = CancellationWatch::new(cancel, &[&video]);
    let mut stdout = video.take_stdout()?;
    let audio = if info.has_audio && !control.muted.load(Ordering::Acquire) {
        Some(AudioOutput::open(
            prepared_wma
                .as_ref()
                .map(|audio| audio.path.clone())
                .or_else(|| prepared_audio.as_ref().map(|audio| audio.path.clone()))
                .unwrap_or_else(|| path.to_path_buf()),
            prepared_wma.as_ref().map(|_| info.format_start_us),
            start_us,
            info.duration_us - start_us,
            control.clone(),
            cancel.clone(),
        )?)
    } else {
        None
    };
    control
        .audio_active
        .store(audio.is_some(), Ordering::Release);
    let result = (|| {
        let mut index = 0_u64;
        let mut wall_us = 0_u64;
        let mut last_tick = Instant::now();
        loop {
            cancel.check()?;
            let mut rgba = vec![0; preview::FRAME_BYTES];
            let mut length = 0;
            while length < rgba.len() {
                let read = stdout.read(&mut rgba[length..])?;
                cancel.check()?;
                if read == 0 {
                    break;
                }
                length += read;
            }
            if length == 0 {
                break;
            }
            if length != rgba.len() {
                return Err("incomplete streaming preview frame".into());
            }
            let frame_us = start_us
                .saturating_add(index.saturating_mul(1_000_000) / FPS)
                .min(info.duration_us);
            if index == 0 {
                present(PreviewFrame {
                    rgba,
                    requested_us: frame_us,
                })?;
                control.frames_presented.fetch_add(1, Ordering::Relaxed);
                control.ready.store(true, Ordering::Release);
                last_tick = Instant::now();
            } else {
                loop {
                    cancel.check()?;
                    if let Some(error) = control
                        .error
                        .lock()
                        .map_err(|_| "audio error lock poisoned")?
                        .clone()
                    {
                        return Err(error.into());
                    }
                    let now = Instant::now();
                    let elapsed = now.duration_since(last_tick).as_micros() as u64;
                    last_tick = now;
                    let paused = control.paused.load(Ordering::Acquire);
                    if audio.is_none() && !paused {
                        wall_us = wall_us.saturating_add(elapsed);
                    }
                    let clock_us = if let Some(audio) = &audio {
                        audio.position_us()
                    } else {
                        wall_us
                    };
                    let position = start_us.saturating_add(clock_us).min(info.duration_us);
                    control.position_us.fetch_max(position, Ordering::AcqRel);
                    if !paused && position >= frame_us {
                        if position.saturating_sub(frame_us) <= 1_000_000 / FPS {
                            present(PreviewFrame {
                                rgba,
                                requested_us: frame_us,
                            })?;
                            control.frames_presented.fetch_add(1, Ordering::Relaxed);
                        } else {
                            control.frames_dropped.fetch_add(1, Ordering::Relaxed);
                        }
                        break;
                    }
                    thread::sleep(Duration::from_millis(4));
                }
            }
            index += 1;
        }
        if index == 0 {
            return Err("No preview video frame at this position".into());
        }
        // Keep the last frame while the remaining (possibly longer) audio drains.
        while control.position_us.load(Ordering::Acquire) < info.duration_us {
            cancel.check()?;
            let now = Instant::now();
            let elapsed = now.duration_since(last_tick).as_micros() as u64;
            last_tick = now;
            if !control.paused.load(Ordering::Acquire) {
                if audio.is_none() {
                    wall_us = wall_us.saturating_add(elapsed);
                }
                let position = start_us
                    .saturating_add(audio.as_ref().map_or(wall_us, AudioOutput::position_us))
                    .min(info.duration_us);
                control.position_us.fetch_max(position, Ordering::AcqRel);
            }
            if let Some(error) = control
                .error
                .lock()
                .map_err(|_| "audio error lock poisoned")?
                .clone()
            {
                return Err(error.into());
            }
            thread::sleep(Duration::from_millis(4));
        }
        let exit = video.wait_cancellable(cancel)?;
        if !exit.success() {
            return Err(video.failure("preview video", exit).into());
        }
        Ok(())
    })();
    // Reap both codec trees, including blocked PCM producer, on every exit.
    drop(watch);
    drop(video);
    drop(audio);
    result
}

fn seconds(us: u64) -> String {
    format!("{}.{:06}", us / 1_000_000, us % 1_000_000)
}

struct AudioShared {
    queue: Mutex<VecDeque<f32>>,
    clock_us: AtomicU64,
    consumed: AtomicU64,
    primed: AtomicBool,
    done: AtomicBool,
    duration_us: u64,
    tail: Mutex<Option<Instant>>,
}

struct AudioOutput {
    _stream: cpal::Stream,
    shared: Arc<AudioShared>,
    stop: CancellationToken,
    producer: Option<thread::JoinHandle<()>>,
}

impl AudioOutput {
    fn open(
        path: PathBuf,
        timeline_origin_us: Option<u64>,
        start_us: u64,
        duration_us: u64,
        control: Arc<PlaybackControl>,
        cancel: CancellationToken,
    ) -> NativeResult<Self> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No audio output device; choose Mute to play video only")?;
        let supported = device.default_output_config()?;
        let config: cpal::StreamConfig = supported.into();
        let rate = config.sample_rate;
        let channels = usize::from(config.channels);
        if rate == 0 || channels == 0 {
            return Err("invalid audio output configuration".into());
        }
        let capacity = rate as usize * channels * AUDIO_BUFFER_MS as usize / 1_000;
        let shared = Arc::new(AudioShared {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            clock_us: AtomicU64::new(0),
            consumed: AtomicU64::new(0),
            primed: AtomicBool::new(false),
            done: AtomicBool::new(false),
            duration_us,
            tail: Mutex::new(None),
        });
        let stream =
            match supported.sample_format() {
                cpal::SampleFormat::F32 => {
                    audio_stream::<f32>(&device, config, shared.clone(), control.clone())?
                }
                cpal::SampleFormat::I16 => {
                    audio_stream::<i16>(&device, config, shared.clone(), control.clone())?
                }
                cpal::SampleFormat::I32 => {
                    audio_stream::<i32>(&device, config, shared.clone(), control.clone())?
                }
                cpal::SampleFormat::U16 => {
                    audio_stream::<u16>(&device, config, shared.clone(), control.clone())?
                }
                _ => return Err(
                    "unsupported audio output sample format; choose Mute for video-only playback"
                        .into(),
                ),
            };
        let stop = CancellationToken::default();
        let producer_stop = stop.clone();
        let audio = shared.clone();
        let producer_control = control.clone();
        let producer = thread::spawn(move || {
            let result = produce_audio(
                &path,
                timeline_origin_us,
                start_us,
                duration_us,
                rate,
                channels,
                capacity,
                &audio,
                &producer_control,
                &cancel,
                &producer_stop,
            );
            audio.done.store(true, Ordering::Release);
            if let Err(error) = result
                && !cancel.is_cancelled()
                && !producer_stop.is_cancelled()
                && let Ok(mut slot) = producer_control.error.lock()
            {
                *slot = Some(error.to_string());
            }
        });
        let output = Self {
            _stream: stream,
            shared,
            stop,
            producer: Some(producer),
        };
        output._stream.play()?;
        Ok(output)
    }
    fn position_us(&self) -> u64 {
        self.shared.clock_us.load(Ordering::Acquire)
    }
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        self.stop.cancel();
        if let Some(producer) = self.producer.take() {
            let _ = producer.join();
        }
    }
}

fn audio_stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    shared: Arc<AudioShared>,
    control: Arc<PlaybackControl>,
) -> NativeResult<cpal::Stream> {
    let channels = usize::from(config.channels);
    let rate = u64::from(config.sample_rate);
    let error_control = control.clone();
    Ok(device.build_output_stream(
        config,
        move |output: &mut [T], info: &cpal::OutputCallbackInfo| {
            output.fill(T::from_sample(0.0));
            if !control.ready.load(Ordering::Acquire)
                || control.paused.load(Ordering::Acquire)
                || !shared.primed.load(Ordering::Acquire)
            {
                return;
            }
            // Audio callback never waits for the decoder or for a contested mutex.
            let Ok(mut queue) = shared.queue.try_lock() else {
                control.audio_underruns.fetch_add(1, Ordering::Relaxed);
                return;
            };
            let available = queue.len().min(output.len()) / channels * channels;
            for sample in &mut output[..available] {
                let value = queue.pop_front().unwrap_or(0.0);
                let peak = (value.abs().min(1.0) * 1_000_000.0) as u64;
                control
                    .audio_peak_amplitude
                    .fetch_max(peak, Ordering::Relaxed);
                if !control.muted.load(Ordering::Acquire) {
                    *sample = T::from_sample(value);
                }
            }
            if available < output.len() && !shared.done.load(Ordering::Acquire) {
                control.audio_underruns.fetch_add(1, Ordering::Relaxed);
            }
            let consumed = shared
                .consumed
                .fetch_add((available / channels) as u64, Ordering::AcqRel)
                + (available / channels) as u64;
            control.audio_samples.store(consumed, Ordering::Release);
            let latency = info
                .timestamp()
                .playback
                .saturating_duration_since(info.timestamp().callback)
                .as_micros() as u64;
            let played_us = consumed.saturating_mul(1_000_000) / rate;
            // Wait for the last submitted samples to reach the device before EOF.
            // Snap to duration to account for sub-sample rounding in FFmpeg's -t.
            let mut time = played_us.saturating_sub(latency);
            if shared.done.load(Ordering::Acquire)
                && queue.is_empty()
                && let Ok(mut tail) = shared.tail.try_lock()
            {
                let deadline =
                    tail.get_or_insert_with(|| Instant::now() + Duration::from_micros(latency));
                if Instant::now() >= *deadline {
                    time = shared.duration_us;
                }
            }
            shared.clock_us.fetch_max(time, Ordering::AcqRel);
        },
        move |error| {
            if let Ok(mut slot) = error_control.error.lock() {
                *slot = Some(format!("Audio output failed: {error}"));
            }
        },
        None,
    )?)
}

fn audio_decode_command(
    path: &Path,
    start_us: u64,
    duration_us: u64,
    rate: u32,
    channels: usize,
) -> Command {
    // AAC overlap state needs decoded predecessors. An audio-only MP4 can seek
    // much closer to the request than a video-keyframe-indexed MP4; discard a
    // bounded 100 ms preroll after decoding so both have settled sample state.
    let preroll_us = start_us.min(100_000);
    let input_seek_us = start_us - preroll_us;
    let filter = format!(
        "atrim=start={},asetpts=PTS-{}/TB,aresample={rate}:async=1:first_pts=0,apad",
        seconds(preroll_us),
        seconds(preroll_us)
    );
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-ss",
            &seconds(input_seek_us),
            "-i",
        ])
        .arg(path)
        .args([
            "-map",
            "0:a:0",
            "-vn",
            "-sn",
            "-dn",
            "-af",
            &filter,
            "-t",
            &seconds(duration_us),
            "-ac",
            &channels.to_string(),
            "-ar",
            &rate.to_string(),
            "-f",
            "f32le",
            "pipe:1",
        ])
        .stdout(Stdio::piped());
    command
}

fn windows_audio_decode_command(
    path: &Path,
    origin: u64,
    start: u64,
    duration: u64,
    rate: u32,
    channels: usize,
) -> NativeResult<Command> {
    let absolute = origin
        .checked_add(start)
        .ok_or("Preview WMA seek overflow")?;
    let mut command = Command::new("ffmpeg");
    // The bounded PCM intermediate has no seek index. Demux linearly (at most
    // 64MiB) and trim by absolute PTS, avoiding demux seek-origin rewriting.
    // first_pts=0 pads a genuine initial offset instead of losing that silence.
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-copyts",
            "-i",
        ])
        .arg(path)
        .args([
            "-map",
            "0:a:0",
            "-vn",
            "-sn",
            "-dn",
            "-af",
            &format!(
                "atrim=start={},asetpts=PTS-{}/TB,aresample={rate}:async=1:first_pts=0,apad",
                seconds(absolute),
                seconds(absolute)
            ),
            "-t",
            &seconds(duration),
            "-ac",
            &channels.to_string(),
            "-ar",
            &rate.to_string(),
            "-f",
            "f32le",
            "pipe:1",
        ])
        .stdout(Stdio::piped());
    Ok(command)
}

#[allow(clippy::too_many_arguments)]
fn produce_audio(
    path: &Path,
    timeline_origin_us: Option<u64>,
    start_us: u64,
    duration_us: u64,
    rate: u32,
    channels: usize,
    capacity: usize,
    shared: &AudioShared,
    control: &PlaybackControl,
    cancel: &CancellationToken,
    stop: &CancellationToken,
) -> NativeResult<()> {
    let mut command = if let Some(origin) = timeline_origin_us {
        windows_audio_decode_command(path, origin, start_us, duration_us, rate, channels)?
    } else {
        audio_decode_command(path, start_us, duration_us, rate, channels)
    };
    let mut child = ChildGuard::spawn(&mut command)?;
    let watch_cancel = CancellationWatch::new(cancel, &[&child]);
    let watch_stop = CancellationWatch::new(stop, &[&child]);
    let mut stdout = child.take_stdout()?;
    let chunk_samples = (rate as usize / 100).max(1) * channels;
    let mut bytes = vec![0; chunk_samples * 4];
    loop {
        cancel.check()?;
        stop.check()?;
        let mut length = 0;
        while length < bytes.len() {
            let count = stdout.read(&mut bytes[length..])?;
            if count == 0 {
                break;
            }
            length += count;
        }
        if length == 0 {
            break;
        }
        if length % (channels * 4) != 0 {
            return Err("incomplete PCM preview sample frame".into());
        }
        let samples = length / 4;
        loop {
            cancel.check()?;
            stop.check()?;
            {
                let mut queue = shared
                    .queue
                    .lock()
                    .map_err(|_| "audio queue lock poisoned")?;
                if queue.len() + samples <= capacity {
                    queue.extend(
                        bytes[..length]
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .map(|bytes| f32::from_le_bytes(*bytes)),
                    );
                    control
                        .audio_peak_samples
                        .fetch_max(queue.len() as u64, Ordering::Relaxed);
                    if queue.len() >= rate as usize * channels / 10 {
                        shared.primed.store(true, Ordering::Release);
                    }
                    break;
                }
            }
            thread::sleep(Duration::from_millis(3));
        }
    }
    shared.primed.store(true, Ordering::Release);
    let exit = child.wait_cancellable(cancel)?;
    drop(watch_stop);
    drop(watch_cancel);
    if !exit.success() {
        return Err(child.failure("preview audio", exit).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn windows_pcm_seek_preserves_offset_gap_and_tail() {
        use std::io::Write;
        let path =
            std::env::temp_dir().join(format!("diaxus-wma-seek-clock-{}.mka", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        crate::frame_stream::audio_start(&mut file, 48000, 2, 16).unwrap();
        // Known PCM markers:100ms initial silence,50ms sound,150ms real gap,
        // then50ms sound. Independent sample-array expectations below detect
        // lost source origin, missing gap, incorrect forward/back seeks or EOF.
        for index in 0..10_i64 {
            let relative_us = 100_000 + index * 10_000 + if index >= 5 { 150_000 } else { 0 };
            crate::frame_stream::frame_header(
                &mut file,
                i128::from((1_250_000 + relative_us) * 10),
                100_000,
                1920,
            )
            .unwrap();
            let value = ((index + 1) * 1000) as i16;
            for _ in 0..960 {
                file.write_all(&value.to_le_bytes()).unwrap();
            }
        }
        drop(file);
        for position in [0, 120_000, 300_000, 100_000, 340_000] {
            let actual =
                windows_audio_decode_command(&path, 1_250_000, position, 100_000, 48000, 2)
                    .unwrap()
                    .output()
                    .unwrap();
            assert!(
                actual.status.success() && actual.stderr.is_empty(),
                "{}",
                String::from_utf8_lossy(&actual.stderr)
            );
            let mut expected = Vec::new();
            for frame in 0..4800_u64 {
                let sample = position * 48 / 1000 + frame;
                let index = if (4800..7200).contains(&sample) {
                    Some((sample - 4800) / 480)
                } else if (14400..16800).contains(&sample) {
                    Some(5 + (sample - 14400) / 480)
                } else {
                    None
                };
                let value = index.map_or(0.0, |n| ((n + 1) * 1000) as f32 / 32768.0);
                for _ in 0..2 {
                    expected.extend(value.to_le_bytes());
                }
            }
            assert!(
                actual.stdout == expected,
                "Windows PCM seek lost offset/gap/tail at{position}; lengths={}/{}",
                actual.stdout.len(),
                expected.len()
            );
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    #[cfg(windows)]
    fn normalized_audio_matches_reference_at_initial_seek_back_and_tail() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/native-aac");
        let token = CancellationToken::default();
        let prepared = crate::audio_timing::prepare(&root.join("leading-pts.mp4"), &token)
            .unwrap()
            .unwrap();
        for (position, duration) in [
            (0, 200_000),
            (700_000, 200_000),
            (100_000, 200_000),
            (1_200_000, 223_220),
        ] {
            let expected =
                audio_decode_command(&root.join("reference.m4a"), position, duration, 48000, 2)
                    .output()
                    .unwrap();
            let actual = audio_decode_command(&prepared.path, position, duration, 48000, 2)
                .output()
                .unwrap();
            assert!(expected.status.success() && expected.stderr.is_empty());
            assert!(actual.status.success() && actual.stderr.is_empty());
            assert!(!actual.stdout.is_empty());
            assert!(
                actual.stdout == expected.stdout,
                "PCM differs at seek {position}"
            );
            assert!(actual.stdout.len() as u64 <= duration * 48000 * 2 * 4 / 1_000_000 + 8);
        }
    }

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m35-vfr-offset.mp4")
    }

    fn wait_until(mut condition: impl FnMut() -> bool) {
        let started = Instant::now();
        while !condition() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "preview timed out"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn continuous_preview_pause_seek_end_and_cancel() {
        continuous_preview_case(fixture(), 15);
    }

    #[test]
    #[cfg(windows)]
    fn normalized_source_preview_pause_seek_end_and_cancel() {
        continuous_preview_case(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/native-aac/leading-pts.mp4"),
            8,
        );
    }

    fn continuous_preview_case(input: PathBuf, minimum_frames: usize) {
        let control = Arc::new(PlaybackControl::default());
        control.muted.store(true, Ordering::Release);
        control.paused.store(true, Ordering::Release);
        let cancel = CancellationToken::default();
        let c = control.clone();
        let token = cancel.clone();
        let frames = Arc::new(Mutex::new(Vec::new()));
        let observed = frames.clone();
        let first_input = input.clone();
        let worker = thread::spawn(move || {
            play(&first_input, 500_000, &c, &token, |frame| {
                assert_eq!(frame.rgba.len(), preview::FRAME_BYTES);
                observed.lock().unwrap().push(frame.requested_us);
                Ok(())
            })
            .map_err(|error| error.to_string())
        });
        wait_until(|| control.ready.load(Ordering::Acquire) || worker.is_finished());
        assert!(control.ready.load(Ordering::Acquire));
        thread::sleep(Duration::from_millis(120));
        assert_eq!(control.position_us.load(Ordering::Acquire), 500_000);
        assert_eq!(&*frames.lock().unwrap(), &[500_000]);
        control.paused.store(false, Ordering::Release);
        wait_until(|| control.position_us.load(Ordering::Acquire) >= 800_000);
        control.paused.store(true, Ordering::Release);
        thread::sleep(Duration::from_millis(30));
        let position = control.position_us.load(Ordering::Acquire);
        thread::sleep(Duration::from_millis(100));
        assert_eq!(control.position_us.load(Ordering::Acquire), position);
        control.paused.store(false, Ordering::Release);
        wait_until(|| worker.is_finished());
        worker.join().unwrap().unwrap();
        assert_eq!(
            control.position_us.load(Ordering::Acquire),
            control.duration_us.load(Ordering::Acquire)
        );
        let frames = frames.lock().unwrap();
        assert!(frames.len() >= minimum_frames);
        assert!(frames.windows(2).all(|pair| pair[0] < pair[1]));

        let control = Arc::new(PlaybackControl::default());
        control.muted.store(true, Ordering::Release);
        control.paused.store(true, Ordering::Release);
        let c = control.clone();
        let token = cancel.clone();
        let worker = thread::spawn(move || {
            play(&input, u64::MAX, &c, &token, |_| Ok(())).map_err(|e| e.to_string())
        });
        wait_until(|| control.ready.load(Ordering::Acquire) || worker.is_finished());
        assert!(control.ready.load(Ordering::Acquire));
        cancel.cancel();
        wait_until(|| worker.is_finished());
        assert!(worker.join().unwrap().is_err());
    }

    #[test]
    #[ignore = "requires an actual audio output device; run explicitly for runtime evidence"]
    fn audio_device_playback_is_bounded_and_reaches_eof() {
        let control = Arc::new(PlaybackControl::default());
        let started = Instant::now();
        let cancel = CancellationToken::default();
        let token = cancel.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let done = finished.clone();
        let watchdog = thread::spawn(move || {
            let started = Instant::now();
            while !done.load(Ordering::Acquire) {
                if started.elapsed() > Duration::from_secs(10) {
                    token.cancel();
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
        });
        let result = play(&fixture(), 0, &control, &cancel, |_| Ok(()));
        finished.store(true, Ordering::Release);
        watchdog.join().unwrap();
        result.unwrap();
        assert!(control.audio_active.load(Ordering::Acquire));
        assert!(control.audio_samples.load(Ordering::Acquire) > 0);
        assert!(control.audio_peak_amplitude.load(Ordering::Acquire) > 0);
        assert_eq!(
            control.position_us.load(Ordering::Acquire),
            control.duration_us.load(Ordering::Acquire)
        );
        assert!(started.elapsed() >= Duration::from_millis(1_900));
        let config = cpal::default_host()
            .default_output_device()
            .unwrap()
            .default_output_config()
            .unwrap();
        let capacity =
            u64::from(config.sample_rate()) * u64::from(config.channels()) * AUDIO_BUFFER_MS
                / 1_000;
        assert!(control.audio_peak_samples.load(Ordering::Acquire) <= capacity);
        eprintln!(
            "Audio runtime: elapsed={:?}, duration_us={}, rate={}, channels={}, consumed={}, peak_queue={}/{}, peak_amplitude={}, underruns={}, displayed={}, dropped={}",
            started.elapsed(),
            control.duration_us.load(Ordering::Acquire),
            config.sample_rate(),
            config.channels(),
            control.audio_samples.load(Ordering::Acquire),
            control.audio_peak_samples.load(Ordering::Acquire),
            capacity,
            control.audio_peak_amplitude.load(Ordering::Acquire),
            control.audio_underruns.load(Ordering::Acquire),
            control.frames_presented.load(Ordering::Acquire),
            control.frames_dropped.load(Ordering::Acquire)
        );
    }
}
