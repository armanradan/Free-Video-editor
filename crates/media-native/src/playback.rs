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
    let info = preview::probe_info(path, cancel)?;
    let start_us = requested_us.min(info.duration_us.saturating_sub(1_000_000 / FPS));
    control
        .duration_us
        .store(info.duration_us, Ordering::Release);
    control.position_us.store(start_us, Ordering::Release);
    let remaining = seconds(info.duration_us - start_us);
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
                "fps={FPS}:start_time=0,{},tpad=stop_mode=clone:stop_duration={remaining}",
                preview::FILTER
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
            path.to_path_buf(),
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

#[allow(clippy::too_many_arguments)]
fn produce_audio(
    path: &Path,
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
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-ss",
            &seconds(start_us),
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
            &format!("aresample={rate}:async=1:first_pts=0,apad"),
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
        let control = Arc::new(PlaybackControl::default());
        control.muted.store(true, Ordering::Release);
        control.paused.store(true, Ordering::Release);
        let cancel = CancellationToken::default();
        let c = control.clone();
        let token = cancel.clone();
        let frames = Arc::new(Mutex::new(Vec::new()));
        let observed = frames.clone();
        let worker = thread::spawn(move || {
            play(&fixture(), 500_000, &c, &token, |frame| {
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
        assert!(frames.len() >= 15);
        assert!(frames.windows(2).all(|pair| pair[0] < pair[1]));

        let control = Arc::new(PlaybackControl::default());
        control.muted.store(true, Ordering::Release);
        control.paused.store(true, Ordering::Release);
        let c = control.clone();
        let token = cancel.clone();
        let worker = thread::spawn(move || {
            play(&fixture(), u64::MAX, &c, &token, |_| Ok(())).map_err(|e| e.to_string())
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
