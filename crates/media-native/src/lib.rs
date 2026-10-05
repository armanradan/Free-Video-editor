#![forbid(unsafe_code)]

use media_core::{
    BitrateSource, FrameGeometry, FrameRateSpec, OutputProfileId, Rect, ResizeSpec, Rotation, Size,
    TrackCoverage, VideoBitrate, VideoCodec, VideoSettings, estimate_output_coverage,
};
use media_gpu::ResizePipeline;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub type NativeResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
mod frame_stream;
mod inspection;
#[cfg(not(target_arch = "wasm32"))]
pub mod playback;
pub mod preview;
use inspection::InspectionCache;

#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    fn check(&self) -> NativeResult<()> {
        if self.is_cancelled() {
            Err("native conversion cancelled".into())
        } else {
            Ok(())
        }
    }
}

struct ChildGuard {
    child: Arc<Mutex<Child>>,
    reaped: bool,
    diagnostics: Arc<Mutex<Vec<u8>>>,
    diagnostics_thread: Option<thread::JoinHandle<()>>,
}

fn terminate_child(child: &mut Child) {
    // PATH may resolve to a package-manager launcher rather than FFmpeg itself.
    // Kill only the live owned process and its descendants, never by image name.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if matches!(child.try_wait(), Ok(None)) {
            let _ = Command::new("taskkill")
                .args(["/PID", &child.id().to_string(), "/T", "/F"])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
    let _ = child.kill();
}

impl ChildGuard {
    fn spawn(command: &mut Command) -> NativeResult<Self> {
        command.stderr(Stdio::piped());
        let mut child = command.spawn()?;
        let diagnostics = Arc::new(Mutex::new(Vec::new()));
        let target = diagnostics.clone();
        let mut stderr = child.stderr.take().ok_or("child stderr was unavailable")?;
        // Drain concurrently, retaining only an 8 KiB diagnostic tail. A pipe
        // must never fill and stall a codec while the conversion waits for it.
        let diagnostics_thread = thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            while let Ok(length) = stderr.read(&mut buffer) {
                if length == 0 {
                    break;
                }
                if let Ok(mut tail) = target.lock() {
                    tail.extend_from_slice(&buffer[..length]);
                    let excess = tail.len().saturating_sub(8192);
                    tail.drain(..excess);
                }
            }
        });
        Ok(Self {
            child: Arc::new(Mutex::new(child)),
            reaped: false,
            diagnostics,
            diagnostics_thread: Some(diagnostics_thread),
        })
    }

    fn failure(&self, label: &str, status: std::process::ExitStatus) -> String {
        format!("{label} exited with {status}: {}", self.diagnostic().trim())
    }

    fn diagnostic(&self) -> String {
        self.diagnostics
            .lock()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default()
    }

    fn take_stdout(&self) -> NativeResult<std::process::ChildStdout> {
        self.child
            .lock()
            .map_err(|_| "decoder process lock was poisoned")?
            .stdout
            .take()
            .ok_or_else(|| "decoder stdout was unavailable".into())
    }

    fn take_stdin(&self) -> NativeResult<std::process::ChildStdin> {
        self.child
            .lock()
            .map_err(|_| "encoder process lock was poisoned")?
            .stdin
            .take()
            .ok_or_else(|| "encoder stdin was unavailable".into())
    }

    fn wait_cancellable(
        &mut self,
        cancel: &CancellationToken,
    ) -> NativeResult<std::process::ExitStatus> {
        loop {
            cancel.check()?;
            if let Some(status) = self
                .child
                .lock()
                .map_err(|_| "child process lock was poisoned")?
                .try_wait()?
            {
                self.reaped = true;
                if let Some(thread) = self.diagnostics_thread.take() {
                    let _ = thread.join();
                }
                return Ok(status);
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !self.reaped
            && let Ok(mut child) = self.child.lock()
        {
            terminate_child(&mut child);
            let _ = child.wait();
        }
    }
}

struct CancellationWatch {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl CancellationWatch {
    fn new(cancel: &CancellationToken, children: &[&ChildGuard]) -> Self {
        Self::new_with_failure(cancel, children, None)
    }

    fn new_with_failure(
        cancel: &CancellationToken,
        children: &[&ChildGuard],
        failure: Option<Arc<AtomicBool>>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_in_thread = stop.clone();
        let token = cancel.clone();
        let children: Vec<_> = children.iter().map(|guard| guard.child.clone()).collect();
        let thread = thread::spawn(move || {
            while !stop_in_thread.load(Ordering::Acquire) {
                if token.is_cancelled()
                    || failure
                        .as_ref()
                        .is_some_and(|signal| signal.load(Ordering::Acquire))
                {
                    for child in children {
                        if let Ok(mut child) = child.lock() {
                            terminate_child(&mut child);
                        }
                    }
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for CancellationWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessingRoute {
    DirectFfmpeg,
    NvidiaFfmpeg,
    SharedWgpu,
    SharedWgpuNvidia,
}

impl ProcessingRoute {
    pub fn uses_wgpu(self) -> bool {
        matches!(self, Self::SharedWgpu | Self::SharedWgpuNvidia)
    }
    pub fn uses_nvidia(self) -> bool {
        matches!(self, Self::NvidiaFfmpeg | Self::SharedWgpuNvidia)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum JobStage {
    Inspecting,
    Preparing,
    Converting,
    Verifying,
    Publishing,
}

pub struct NativeJob<'a> {
    pub input: &'a Path,
    pub output: &'a Path,
    pub resize: ResizeSpec,
    pub profile: OutputProfileId,
    pub route: ProcessingRoute,
    /// None retains legacy CRF/CQ20 for existing harnesses.
    pub bitrate: Option<VideoBitrate>,
    pub frame_rate: FrameRateSpec,
}

#[derive(Debug, Deserialize)]
struct Probe {
    streams: Vec<ProbeStream>,
}

#[derive(Debug, Deserialize)]
struct ProbeStream {
    bit_rate: Option<String>,
    codec_type: String,
    codec_name: String,
    codec_tag_string: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    sample_aspect_ratio: Option<String>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    time_base: Option<String>,
    start_time: Option<String>,
    duration: Option<String>,
    duration_ts: Option<i64>,
    nb_frames: Option<String>,
    pix_fmt: Option<String>,
    profile: Option<String>,
    color_range: Option<String>,
    color_space: Option<String>,
    color_transfer: Option<String>,
    color_primaries: Option<String>,
    side_data_list: Option<Vec<SideData>>,
}

#[derive(Debug, Deserialize)]
struct SideData {
    side_data_type: Option<String>,
    rotation: Option<i32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceInfo {
    pub video_bitrate_bps: Option<u64>,
    pub codec: String,
    pub codec_tag: Option<String>,
    pub codec_profile: Option<String>,
    pub pixel_format: String,
    pub width: u32,
    pub height: u32,
    pub display_width: u32,
    pub display_height: u32,
    pub sample_aspect_ratio: String,
    pub rotation_degrees: i32,
    pub has_display_matrix: bool,
    pub color_range: Option<String>,
    pub color_space: Option<String>,
    pub color_transfer: Option<String>,
    pub color_primaries: Option<String>,
    pub frame_rate_num: u32,
    pub frame_rate_den: u32,
    pub frame_count: u64,
    pub audio_codec: Option<String>,
    pub video_start_us: i128,
    pub audio_start_us: Option<i128>,
    pub video_duration_us: Option<i128>,
    /// Exact stream duration for CFR endpoint planning, not rounded decimal seconds.
    pub video_duration_ticks: Option<i64>,
    pub video_time_base: Option<String>,
    pub audio_duration_us: Option<i128>,
    pub variable_frame_rate: bool,
}

impl SourceInfo {
    pub fn resolve_bitrate(
        &self,
        size: Size,
        profile: OutputProfileId,
        bitrate: VideoBitrate,
        frame_rate: FrameRateSpec,
    ) -> NativeResult<u32> {
        Ok(VideoSettings {
            bitrate,
            frame_rate,
        }
        .resolve_bitrate(
            size,
            profile.video_codec(),
            BitrateSource {
                size: Size::new(self.display_width, self.display_height)?,
                fps_num: self.frame_rate_num,
                fps_den: self.frame_rate_den,
                codec: if self.codec == "hevc" {
                    VideoCodec::H265
                } else {
                    VideoCodec::H264
                },
                bitrate_bps: self.video_bitrate_bps,
            },
        )?)
    }

    pub fn output_estimate(
        &self,
        resize: ResizeSpec,
        profile: OutputProfileId,
        bitrate: VideoBitrate,
    ) -> NativeResult<(u32, Option<u64>)> {
        self.output_estimate_with_rate(resize, profile, bitrate, FrameRateSpec::Original)
    }

    pub fn output_estimate_with_rate(
        &self,
        resize: ResizeSpec,
        profile: OutputProfileId,
        bitrate: VideoBitrate,
        frame_rate: FrameRateSpec,
    ) -> NativeResult<(u32, Option<u64>)> {
        let size = resize.output_size(Size::new(self.display_width, self.display_height)?)?;
        let bps = self.resolve_bitrate(size, profile, bitrate, frame_rate)?;
        let video_end = self
            .video_duration_us
            .and_then(|duration| self.video_start_us.checked_add(duration));
        let audio_end = self
            .audio_start_us
            .zip(self.audio_duration_us)
            .and_then(|(start, duration)| start.checked_add(duration));
        Ok((
            bps,
            estimate_output_coverage(
                bps,
                TrackCoverage {
                    start_us: self.video_start_us,
                    end_us: video_end,
                },
                self.audio_codec.as_ref().map(|_| {
                    (
                        TrackCoverage {
                            start_us: self.audio_start_us.unwrap_or(self.video_start_us),
                            end_us: audio_end,
                        },
                        192_000,
                    )
                }),
            ),
        ))
    }

    /// Known shared-RGBA bridge restrictions after direct-source inspection.
    /// An absent reason does not replace the GPU route's full preflight.
    pub fn shared_gpu_limitation(&self) -> Option<String> {
        if self.pixel_format != "yuv420p" {
            Some("Shared GPU currently requires 8-bit yuv420p input. Use Direct FFmpeg for this source.".into())
        } else if self.sample_aspect_ratio != "1:1" || self.has_display_matrix {
            Some("Shared GPU currently requires square pixels and no display transform. Use Direct FFmpeg for this source.".into())
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AdapterDescriptor {
    pub key: String,
    pub name: String,
    pub vendor: u32,
    pub device: u32,
    pub device_type: String,
    pub backend: String,
    pub driver: String,
}

impl AdapterDescriptor {
    fn from_info(info: &wgpu::AdapterInfo) -> Self {
        let name = info.name.clone();
        let backend = format!("{:?}", info.backend);
        let device_type = format!("{:?}", info.device_type);
        Self {
            key: format!(
                "{}:{:04x}:{:04x}:{}",
                backend, info.vendor, info.device, name
            ),
            name,
            vendor: info.vendor,
            device: info.device,
            device_type,
            backend,
            driver: info.driver.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ConversionReport {
    pub output_frame_count: u64,
    pub frame_rate: FrameRateSpec,
    pub video_bitrate_bps: Option<u32>,
    pub route: String,
    pub profile: String,
    pub input: SourceInfo,
    pub output_width: u32,
    pub output_height: u32,
    pub frames_processed: u64,
    pub output_bytes: u64,
    pub elapsed_ms: u128,
    pub verification_ms: u128,
    pub adapter: Option<AdapterDescriptor>,
    pub explicit_cpu_to_gpu_bytes: u64,
    pub explicit_gpu_to_cpu_bytes: u64,
    pub adapter_fallback: Option<String>,
    pub hardware_gpu: Option<NvidiaGpu>,
    pub video_encoder: String,
    pub inspection_reused: bool,
    pub preflight_ms: u128,
    pub total_ms: u128,
    pub codec_gpu_to_cpu_bytes: u64,
    pub codec_cpu_to_gpu_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NvidiaGpu {
    pub name: String,
    pub uuid: String,
    pub vendor: u32,
    pub device: u32,
}

/// Discovery alone is not a driver/codec capability proof; conversion must succeed.
pub fn enumerate_nvidia_gpus() -> NativeResult<Vec<NvidiaGpu>> {
    let output = run_capture(
        "nvidia-smi",
        &[
            OsStr::new("--query-gpu=name,uuid,pci.device_id"),
            OsStr::new("--format=csv,noheader,nounits"),
        ],
    )?;
    parse_nvidia_gpus(std::str::from_utf8(&output)?)
}

fn parse_nvidia_gpus(text: &str) -> NativeResult<Vec<NvidiaGpu>> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let fields: Vec<_> = line.split(',').map(str::trim).collect();
            if fields.len() != 3 || !fields[1].starts_with("GPU-") {
                return Err("invalid NVIDIA discovery response".into());
            }
            let pci = u32::from_str_radix(fields[2].trim_start_matches("0x"), 16)?;
            Ok(NvidiaGpu {
                name: fields[0].into(),
                uuid: fields[1].into(),
                vendor: pci & 0xffff,
                device: pci >> 16,
            })
        })
        .collect()
}

fn select_nvidia_gpu(key: Option<&str>) -> NativeResult<NvidiaGpu> {
    let mut gpus = enumerate_nvidia_gpus()?;
    if let Some(key) = key {
        let descriptor = enumerate_adapters()
            .into_iter()
            .find(|adapter| adapter.key == key)
            .ok_or("selected GPU is unavailable; select an available NVIDIA adapter")?;
        gpus.retain(|gpu| {
            gpu.vendor == descriptor.vendor
                && gpu.device == descriptor.device
                && gpu.name == descriptor.name
        });
    }
    // wgpu currently has no PCI bus/UUID identity. Do not guess among identical boards.
    if gpus.len() != 1 {
        return Err("NVIDIA route requires one uniquely identifiable NVIDIA GPU; select its adapter or use software FFmpeg".into());
    }
    Ok(gpus.remove(0))
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionStatus {
    pub device_generation: u64,
    pub adapter_key: Option<String>,
    pub active: bool,
    pub switching: bool,
}

#[derive(Debug, Serialize)]
pub struct SessionConversionReport {
    pub device_generation: u64,
    pub conversion: ConversionReport,
}

struct SessionState {
    device_generation: u64,
    adapter_key: Option<String>,
    active: Option<CancellationToken>,
    switching: bool,
    closed: bool,
}

/// A headless native execution context. GPU resources are created per job;
/// switching waits for the old job to finish cleanup before a new one starts.
pub struct NativeSession {
    state: Mutex<SessionState>,
    idle: Condvar,
    inspection: InspectionCache,
}

struct ActiveSessionJob<'a> {
    session: &'a NativeSession,
}

impl Drop for ActiveSessionJob<'_> {
    fn drop(&mut self) {
        let mut state = self.session.state.lock().unwrap_or_else(|e| e.into_inner());
        state.active = None;
        self.session.idle.notify_all();
    }
}

impl NativeSession {
    pub fn new(adapter_key: Option<&str>) -> NativeResult<Self> {
        if let Some(key) = adapter_key {
            Self::validate_adapter(key)?;
        }
        Ok(Self {
            state: Mutex::new(SessionState {
                device_generation: 1,
                adapter_key: adapter_key.map(str::to_owned),
                active: None,
                switching: false,
                closed: false,
            }),
            idle: Condvar::new(),
            inspection: InspectionCache::default(),
        })
    }

    fn validate_adapter(key: &str) -> NativeResult<()> {
        if enumerate_adapters()
            .iter()
            .any(|adapter| adapter.key == key)
        {
            Ok(())
        } else {
            Err(format!("native GPU adapter is unavailable: {key}").into())
        }
    }

    pub fn status(&self) -> SessionStatus {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        SessionStatus {
            device_generation: state.device_generation,
            adapter_key: state.adapter_key.clone(),
            active: state.active.is_some(),
            switching: state.switching,
        }
    }

    pub fn cancel_active(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(token) = &state.active {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Permanently reject new jobs and wait for active codec/output cleanup.
    /// Call after the native window event loop returns, not on its UI thread.
    pub fn shutdown(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.closed = true;
        if let Some(token) = &state.active {
            token.cancel();
        }
        while state.active.is_some() || state.switching {
            state = self.idle.wait(state).unwrap_or_else(|e| e.into_inner());
        }
    }

    pub fn switch_adapter(&self, adapter_key: Option<&str>) -> NativeResult<u64> {
        if let Some(key) = adapter_key {
            Self::validate_adapter(key)?;
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed {
            return Err("native session is closed".into());
        }
        if state.switching {
            return Err("native GPU adapter switch is already in progress".into());
        }
        if state.adapter_key.as_deref() == adapter_key {
            return Ok(state.device_generation);
        }
        let next_generation = state
            .device_generation
            .checked_add(1)
            .ok_or("native device generation overflow")?;
        state.switching = true;
        if let Some(token) = &state.active {
            token.cancel();
        }
        while state.active.is_some() {
            state = self.idle.wait(state).unwrap_or_else(|e| e.into_inner());
        }
        if state.closed {
            state.switching = false;
            self.idle.notify_all();
            return Err("native session is closed".into());
        }
        state.adapter_key = adapter_key.map(str::to_owned);
        state.device_generation = next_generation;
        state.switching = false;
        self.idle.notify_all();
        Ok(next_generation)
    }

    pub fn convert(
        &self,
        input: &Path,
        output: &Path,
        resize: ResizeSpec,
        profile: OutputProfileId,
        route: ProcessingRoute,
    ) -> NativeResult<SessionConversionReport> {
        self.convert_with_cancellation(
            input,
            output,
            resize,
            profile,
            route,
            &CancellationToken::default(),
        )
    }

    /// Accept a UI-owned token so Cancel also covers a not-yet-started worker.
    pub fn convert_with_cancellation(
        &self,
        input: &Path,
        output: &Path,
        resize: ResizeSpec,
        profile: OutputProfileId,
        route: ProcessingRoute,
        cancel: &CancellationToken,
    ) -> NativeResult<SessionConversionReport> {
        self.convert_job(
            NativeJob {
                input,
                output,
                resize,
                profile,
                route,
                bitrate: None,
                frame_rate: FrameRateSpec::Original,
            },
            cancel,
            |_| {},
        )
    }

    fn begin_job(
        &self,
        cancel: &CancellationToken,
    ) -> NativeResult<(u64, Option<String>, CancellationToken)> {
        let (generation, adapter_key, token) = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.closed {
                return Err("native session is closed".into());
            }
            if state.switching || state.active.is_some() {
                return Err("native session is busy".into());
            }
            cancel.check()?;
            let token = cancel.clone();
            state.active = Some(token.clone());
            (state.device_generation, state.adapter_key.clone(), token)
        };
        Ok((generation, adapter_key, token))
    }

    /// Inspection participates in cancellation, serialization and window shutdown.
    pub fn inspect(&self, input: &Path, cancel: &CancellationToken) -> NativeResult<SourceInfo> {
        self.begin_job(cancel)?;
        let _active_job = ActiveSessionJob { session: self };
        let (value, _) = self.inspection.load(input, cancel)?;
        Ok(value.source.clone())
    }

    pub fn convert_job(
        &self,
        job: NativeJob<'_>,
        cancel: &CancellationToken,
        on_stage: impl Fn(JobStage),
    ) -> NativeResult<SessionConversionReport> {
        let NativeJob {
            input,
            output,
            resize,
            profile,
            route,
            bitrate,
            frame_rate,
        } = job;
        let (generation, adapter_key, token) = self.begin_job(cancel)?;
        let _active_job = ActiveSessionJob { session: self };
        let conversion = convert_with_control_inner(
            input,
            output,
            resize,
            profile,
            route,
            NativeRunOptions {
                bitrate,
                frame_rate,
                adapter_key: adapter_key.as_deref(),
                inspection: Some(&self.inspection),
                on_stage: Some(&on_stage),
                ..Default::default()
            },
            &token,
        )?;
        Ok(SessionConversionReport {
            device_generation: generation,
            conversion,
        })
    }
}

fn run_capture(program: &str, args: &[&OsStr]) -> NativeResult<Vec<u8>> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output.stdout)
}

pub fn probe_source(path: &Path) -> NativeResult<SourceInfo> {
    inspect_source(path, true, true, &CancellationToken::default()).map(|(source, _)| source)
}

/// Inspect a source using the direct route's timing and display-geometry policy.
/// Conversion still performs profile- and route-specific preflight before output.
pub fn probe_source_direct(path: &Path) -> NativeResult<SourceInfo> {
    inspect_source(path, false, true, &CancellationToken::default()).map(|(source, _)| source)
}

fn inspect_source(
    path: &Path,
    require_plain_geometry: bool,
    allow_10bit: bool,
    cancel: &CancellationToken,
) -> NativeResult<(SourceInfo, Vec<i128>)> {
    cancel.check()?;
    if !path.is_file() {
        return Err(format!("input is not a file: {}", path.display()).into());
    }
    let output = run_capture(
        "ffprobe",
        &[
            OsStr::new("-v"),
            OsStr::new("error"),
            OsStr::new("-show_streams"),
            OsStr::new("-of"),
            OsStr::new("json"),
            path.as_os_str(),
        ],
    )?;
    cancel.check()?;
    let parsed: Probe = serde_json::from_slice(&output)?;
    let video_tracks: Vec<_> = parsed
        .streams
        .iter()
        .filter(|s| s.codec_type == "video")
        .collect();
    if video_tracks.len() != 1 {
        return Err("native M4 harness requires exactly one video track".into());
    }
    let video = video_tracks[0];
    let audio_tracks: Vec<_> = parsed
        .streams
        .iter()
        .filter(|s| s.codec_type == "audio")
        .collect();
    if audio_tracks.len() > 1 {
        return Err("native M4 harness does not yet support multiple audio tracks".into());
    }
    let width = video.width.ok_or("ffprobe omitted video width")?;
    let height = video.height.ok_or("ffprobe omitted video height")?;
    let coded = Size::new(width, height)?;
    let sample_aspect_ratio = video.sample_aspect_ratio.as_deref().unwrap_or("1:1");
    let (sar_num, sar_den) = if sample_aspect_ratio == "N/A" {
        (1, 1)
    } else {
        let ratio = sample_aspect_ratio.replace(':', "/");
        parse_rational(&ratio)?
    };
    let rotation_degrees = video
        .side_data_list
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find_map(|side| side.rotation)
        .unwrap_or(0);
    let has_display_matrix = video
        .side_data_list
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|side| side.side_data_type.as_deref() == Some("Display Matrix"));
    if require_plain_geometry && (sar_num, sar_den) != (1, 1) {
        return Err("native M4 harness currently requires square-pixel input".into());
    }
    if require_plain_geometry && has_display_matrix {
        return Err(
            "native M4 shared-wgpu route does not yet implement source display matrices".into(),
        );
    }
    let angle = rotation_degrees.unsigned_abs() % 360;
    let rotation = Rotation::from_degrees(angle)?;
    let square_width = u32::try_from(
        i128::from(width)
            .checked_mul(sar_num)
            .and_then(|value| value.checked_add(sar_den / 2))
            .ok_or("sample aspect ratio overflow")?
            / sar_den,
    )?;
    let geometry = FrameGeometry::new(
        coded,
        Rect::new(0, 0, width, height)?,
        Size::new(square_width, height)?,
        rotation,
        false,
    )?;
    let display = geometry.display_size();
    validate_native_sdr(video, allow_10bit)?;
    let video_start_us = video
        .start_time
        .as_deref()
        .map(parse_decimal_us)
        .transpose()?
        .unwrap_or(0);
    let audio_start_us = audio_tracks
        .first()
        .and_then(|stream| stream.start_time.as_deref())
        .map(parse_decimal_us)
        .transpose()?;
    let video_duration_us = video
        .duration
        .as_deref()
        .map(parse_decimal_us)
        .transpose()?;
    let audio_duration_us = audio_tracks
        .first()
        .and_then(|stream| stream.duration.as_deref())
        .map(parse_decimal_us)
        .transpose()?;
    if video_start_us < 0 || audio_start_us.is_some_and(|start| start < 0) {
        return Err("native M4 harness does not yet support negative stream origins".into());
    }
    if !audio_tracks.is_empty() && audio_start_us.is_none() {
        return Err("direct native timeline requires a known audio start time".into());
    }
    let rate = video
        .avg_frame_rate
        .as_deref()
        .ok_or("ffprobe omitted frame rate")?;
    let (num, den) = rate.split_once('/').ok_or("invalid rational frame rate")?;
    let (num, den) = (num.parse::<u32>()?, den.parse::<u32>()?);
    if num == 0 || den == 0 {
        return Err("invalid zero frame rate".into());
    }
    let frame_count = video
        .nb_frames
        .as_deref()
        .ok_or("native M4 harness requires a known frame count")?
        .parse::<u64>()?;
    if frame_count == 0 {
        return Err("video has no frames".into());
    }
    let timeline = scan_video_timestamps(
        path,
        FrameScanSpec {
            time_base: video
                .time_base
                .as_deref()
                .ok_or("ffprobe omitted video time base")?,
            frame_count,
            size: require_plain_geometry.then_some(coded),
        },
        cancel,
    )?;
    let period_num = i128::from(den)
        .checked_mul(1_000_000)
        .ok_or("frame period overflow")?;
    let period_den = i128::from(num);
    let mut variable_frame_rate = video.r_frame_rate.as_deref() != Some(rate);
    for (index, pts) in timeline.iter().enumerate() {
        let offset = i128::try_from(index)?
            .checked_mul(period_num)
            .and_then(|v| v.checked_add(period_den / 2))
            .ok_or("frame period overflow")?
            / period_den;
        let expected = timeline[0]
            .checked_add(offset)
            .ok_or("frame timestamp overflow")?;
        variable_frame_rate |= pts
            .checked_sub(expected)
            .ok_or("frame timestamp overflow")?
            .unsigned_abs()
            > 150;
    }
    Ok((
        SourceInfo {
            video_bitrate_bps: video
                .bit_rate
                .as_deref()
                .and_then(|value| value.parse().ok())
                .filter(|rate| *rate > 0),
            codec: video.codec_name.clone(),
            codec_tag: video.codec_tag_string.clone(),
            codec_profile: video.profile.clone(),
            pixel_format: video
                .pix_fmt
                .clone()
                .ok_or("ffprobe omitted pixel format")?,
            width,
            height,
            display_width: display.width,
            display_height: display.height,
            sample_aspect_ratio: sample_aspect_ratio.to_owned(),
            rotation_degrees,
            has_display_matrix,
            color_range: video.color_range.clone(),
            color_space: video.color_space.clone(),
            color_transfer: video.color_transfer.clone(),
            color_primaries: video.color_primaries.clone(),
            frame_rate_num: num,
            frame_rate_den: den,
            frame_count,
            audio_codec: audio_tracks.first().map(|v| v.codec_name.clone()),
            video_start_us,
            audio_start_us,
            video_duration_us,
            video_duration_ticks: video.duration_ts,
            video_time_base: video.time_base.clone(),
            audio_duration_us,
            variable_frame_rate,
        },
        timeline,
    ))
}

fn validate_native_sdr(video: &ProbeStream, allow_10bit: bool) -> NativeResult<()> {
    for (label, value, accepted) in [
        ("range", video.color_range.as_deref(), "tv"),
        ("matrix", video.color_space.as_deref(), "bt709"),
        ("transfer", video.color_transfer.as_deref(), "bt709"),
        ("primaries", video.color_primaries.as_deref(), "bt709"),
    ] {
        if value.is_some_and(|tag| tag != accepted && tag != "unknown") {
            return Err(format!(
                "native M4 supports only BT.709 limited-range SDR; unsupported {label}: {}",
                value.unwrap_or_default()
            )
            .into());
        }
    }
    if !matches!(video.pix_fmt.as_deref(), Some("yuv420p" | "nv12"))
        && !(allow_10bit && matches!(video.pix_fmt.as_deref(), Some("yuv420p10le" | "p010le")))
    {
        return Err(format!(
            "native M4 supports only opaque 8-bit YUV 4:2:0{} input; unsupported pixel format: {}",
            if allow_10bit {
                " or direct-route 10-bit YUV 4:2:0"
            } else {
                ""
            },
            video.pix_fmt.as_deref().unwrap_or("missing")
        )
        .into());
    }
    Ok(())
}

fn parse_rational(value: &str) -> NativeResult<(i128, i128)> {
    let (num, den) = value.split_once('/').ok_or("invalid rational value")?;
    let (num, den) = (num.parse::<i128>()?, den.parse::<i128>()?);
    if num <= 0 || den <= 0 {
        return Err("nonpositive rational value".into());
    }
    Ok((num, den))
}

fn parse_decimal_us(value: &str) -> NativeResult<i128> {
    let (negative, digits) = match value.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, value),
    };
    let (whole, fractional) = digits.split_once('.').unwrap_or((digits, ""));
    let scale = 10_i128
        .checked_pow(u32::try_from(fractional.len())?)
        .ok_or("timestamp scale overflow")?;
    let whole = whole.parse::<i128>()?;
    let fraction = if fractional.is_empty() {
        0
    } else {
        fractional.parse::<i128>()?
    };
    let scaled = whole
        .checked_mul(scale)
        .and_then(|v| v.checked_add(fraction))
        .ok_or("timestamp overflow")?;
    let microseconds = scaled.checked_mul(1_000_000).ok_or("timestamp overflow")?;
    let microseconds = microseconds
        .checked_add(scale / 2)
        .ok_or("timestamp overflow")?
        / scale;
    Ok(if negative {
        microseconds.checked_neg().ok_or("timestamp overflow")?
    } else {
        microseconds
    })
}

struct FrameScanSpec<'a> {
    time_base: &'a str,
    frame_count: u64,
    size: Option<Size>,
}

fn scan_video_timestamps(
    path: &Path,
    spec: FrameScanSpec<'_>,
    cancel: &CancellationToken,
) -> NativeResult<Vec<i128>> {
    let FrameScanSpec {
        time_base,
        frame_count,
        size,
    } = spec;
    let (time_num, time_den) = parse_rational(time_base)?;
    let mut command = Command::new("ffprobe");
    command
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "frame=best_effort_timestamp,width,height",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .stdout(Stdio::piped());
    let mut child = ChildGuard::spawn(&mut command)?;
    let _watch = CancellationWatch::new(cancel, &[&child]);
    let mut timeline = Vec::new();
    let mut decoded_size = size;
    let result = (|| -> NativeResult<()> {
        let stdout = child.take_stdout()?;
        for line in BufReader::new(stdout).lines() {
            cancel.check()?;
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let mut fields = line.split(',');
            let pts = fields
                .next()
                .ok_or("missing frame timestamp")?
                .parse::<i128>()?;
            if pts < 0 {
                return Err(
                    "native M4 harness does not yet support negative frame timestamps".into(),
                );
            }
            let width = fields.next().ok_or("missing frame width")?.parse::<u32>()?;
            let height = fields
                .next()
                .ok_or("missing frame height")?
                .parse::<u32>()?;
            let current_size = Size::new(width, height)?;
            if decoded_size.is_some_and(|expected| expected != current_size) {
                return Err(format!(
                    "mid-stream video geometry changed at frame {}",
                    timeline.len()
                )
                .into());
            }
            decoded_size = Some(current_size);
            let micros = pts
                .checked_mul(time_num)
                .and_then(|v| v.checked_mul(1_000_000))
                .ok_or("frame timestamp overflow")?;
            let micros = micros
                .checked_add(time_den / 2)
                .ok_or("frame timestamp overflow")?
                / time_den;
            if timeline.last().is_some_and(|previous| micros <= *previous) {
                return Err("video frame timestamps are not strictly increasing".into());
            }
            timeline.push(micros);
            if u64::try_from(timeline.len())? > frame_count {
                return Err("ffprobe decoded more frames than reported".into());
            }
        }
        if u64::try_from(timeline.len())? != frame_count {
            return Err(format!(
                "ffprobe decoded {} frames, expected {frame_count}",
                timeline.len()
            )
            .into());
        }
        Ok(())
    })();
    cancel.check()?;
    result?;
    if !child.wait_cancellable(cancel)?.success() {
        return Err("ffprobe frame timeline inspection failed".into());
    }
    Ok(timeline)
}

#[derive(Clone, Copy, Debug)]
struct CfrPlan {
    numerator: u32,
    denominator: u32,
    first: i64,
    end: i64,
}

impl CfrPlan {
    fn for_source(
        source: &SourceInfo,
        timeline: &[i128],
        spec: FrameRateSpec,
    ) -> NativeResult<Option<Self>> {
        let FrameRateSpec::Constant { .. } = spec else {
            return Ok(None);
        };
        let (numerator, denominator) =
            spec.resolve(source.frame_rate_num, source.frame_rate_den)?;
        let origin = source
            .audio_start_us
            .map_or(source.video_start_us, |audio| {
                audio.min(source.video_start_us)
            });
        let start = timeline
            .first()
            .copied()
            .ok_or("empty video timeline")?
            .checked_sub(origin)
            .ok_or("CFR origin overflow")?;
        source
            .video_duration_us
            .ok_or("frame-rate conversion requires known video duration")?;
        let ticks = source
            .video_duration_ticks
            .ok_or("frame-rate conversion requires exact video duration ticks")?;
        let (time_num, time_den) = parse_rational(
            source
                .video_time_base
                .as_deref()
                .ok_or("frame-rate conversion requires video time base")?,
        )?;
        if ticks <= 0 || time_num <= 0 || time_den <= 0 {
            return Err("invalid exact video duration".into());
        }
        let end = i128::from(ticks)
            .checked_mul(time_num)
            .and_then(|value| value.checked_mul(1_000_000))
            .and_then(|value| {
                source
                    .video_start_us
                    .checked_sub(origin)
                    .and_then(|offset| offset.checked_mul(time_den))
                    .and_then(|offset| value.checked_add(offset))
            })
            .ok_or("CFR duration overflow")?;
        if start < 0 || end <= start.checked_mul(time_den).ok_or("CFR start overflow")? {
            return Err("invalid video coverage for frame-rate conversion".into());
        }
        let grid = media_core::FrameRateGrid::new(numerator, denominator)?;
        let plan = Self {
            numerator,
            denominator,
            first: grid.index(start, 1, false)?,
            end: grid.index(end, time_den, true)?,
        };
        if plan.end <= plan.first {
            return Err("frame-rate conversion would produce no frames".into());
        }
        Ok(Some(plan))
    }

    fn count(self) -> u64 {
        (self.end - self.first) as u64
    }

    fn timestamp_us(self, index: i64) -> NativeResult<i128> {
        Ok(i128::from(
            media_core::FrameRateGrid::new(self.numerator, self.denominator)?
                .timestamp_us(index)?,
        ))
    }

    fn duration_us(self) -> NativeResult<i128> {
        self.timestamp_us(self.end - self.first)
    }

    fn verify(self, timeline: &[i128]) -> NativeResult<()> {
        if u64::try_from(timeline.len())? != self.count() {
            return Err("CFR output frame count differs from planned coverage".into());
        }
        for (index, &pts) in timeline.iter().enumerate() {
            let expected = self.timestamp_us(
                self.first
                    .checked_add(i64::try_from(index)?)
                    .ok_or("CFR index overflow")?,
            )?;
            if pts
                .checked_sub(expected)
                .ok_or("CFR verification overflow")?
                .unsigned_abs()
                > 1_000
            {
                return Err(format!("CFR output frame {index} timestamp mismatch: expected {expected}µs, got {pts}µs").into());
            }
        }
        Ok(())
    }
}

fn append_fps_filter(filter: &mut String, plan: Option<CfrPlan>) -> NativeResult<()> {
    if let Some(plan) = plan {
        if !filter.is_empty() {
            filter.push(',');
        }
        let start = plan.timestamp_us(plan.first)?;
        // Force the planned first grid point; trim only a possible extra tail
        // caused by the bridge's microsecond-rounded EOF. Never pad missing EOF.
        filter.push_str(&format!(
            "fps=fps={}/{}:start_time={}.{:06}:round=near:eof_action=pass,trim=end_frame={}",
            plan.numerator,
            plan.denominator,
            start / 1_000_000,
            start % 1_000_000,
            plan.count()
        ));
    }
    Ok(())
}

fn verify_output_timeline(
    input: &SourceInfo,
    input_timeline: &[i128],
    output: &SourceInfo,
    output_timeline: &[i128],
) -> NativeResult<()> {
    if input_timeline.len() != output_timeline.len() {
        return Err("native output changed the video frame count".into());
    }
    let origin = input.audio_start_us.map_or(input.video_start_us, |audio| {
        audio.min(input.video_start_us)
    });
    for (index, (&before, &after)) in input_timeline.iter().zip(output_timeline).enumerate() {
        let expected = before
            .checked_sub(origin)
            .ok_or("timeline origin overflow")?;
        if after
            .checked_sub(expected)
            .ok_or("timeline difference overflow")?
            .unsigned_abs()
            > 1_000
        {
            return Err(format!(
                "native output changed frame {index} timing: expected {expected}µs, got {after}µs"
            )
            .into());
        }
    }
    verify_track_coverage(input, output, input.video_duration_us, origin)
}

fn verify_track_coverage(
    input: &SourceInfo,
    output: &SourceInfo,
    expected_video_duration: Option<i128>,
    origin: i128,
) -> NativeResult<()> {
    if input.audio_codec.is_some() {
        let before = input
            .audio_start_us
            .ok_or("input audio start was unavailable")?;
        let after = output
            .audio_start_us
            .ok_or("output audio start was unavailable")?;
        let expected = before.checked_sub(origin).ok_or("audio origin overflow")?;
        if after
            .checked_sub(expected)
            .ok_or("audio start difference overflow")?
            .unsigned_abs()
            > 25_000
        {
            return Err(format!(
                "native output changed audio start: expected {expected}µs, got {after}µs"
            )
            .into());
        }
    }
    if let Some(before) = expected_video_duration {
        let after = output
            .video_duration_us
            .ok_or("output video duration was unavailable")?;
        if after
            .checked_sub(before)
            .ok_or("video duration difference overflow")?
            .unsigned_abs()
            > 1_000
        {
            return Err("native output changed video duration by more than 1 ms".into());
        }
    }
    if let Some(before) = input.audio_duration_us {
        let after = output
            .audio_duration_us
            .ok_or("output audio duration was unavailable")?;
        if after
            .checked_sub(before)
            .ok_or("audio duration difference overflow")?
            .unsigned_abs()
            > 25_000
        {
            return Err("native output changed audio duration by more than one AAC packet".into());
        }
    }
    Ok(())
}

fn partial_path(output: &Path) -> NativeResult<PathBuf> {
    if output.exists() {
        return Err(format!("refusing to overwrite output: {}", output.display()).into());
    }
    let name = output
        .file_stem()
        .ok_or("output needs a filename")?
        .to_string_lossy();
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    Ok(parent.join(format!("{name}.{}.partial.mp4", std::process::id())))
}

struct PartialOutput {
    path: PathBuf,
    committed: bool,
}

impl Drop for PartialOutput {
    fn drop(&mut self) {
        if !self.committed {
            for attempt in 0..100 {
                match fs::remove_file(&self.path) {
                    Ok(()) => return,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
                    Err(error) => {
                        if attempt == 99 {
                            eprintln!(
                                "could not remove partial output {}: {error}",
                                self.path.display()
                            );
                        } else {
                            thread::sleep(Duration::from_millis(20));
                        }
                    }
                }
            }
        }
    }
}

fn create_partial(output: &Path) -> NativeResult<PartialOutput> {
    let partial = partial_path(output)?;
    if partial.exists() {
        return Err(format!(
            "refusing to overwrite existing partial output: {}",
            partial.display()
        )
        .into());
    }
    Ok(PartialOutput {
        path: partial,
        committed: false,
    })
}

fn finish_output(partial: &mut PartialOutput, output: &Path) -> NativeResult<u64> {
    let size = fs::metadata(&partial.path)?.len();
    if size == 0 || output.exists() {
        return Err("empty output or output path became occupied".into());
    }
    fs::rename(&partial.path, output)?;
    partial.committed = true;
    Ok(size)
}

pub fn enumerate_adapters() -> Vec<AdapterDescriptor> {
    let instance = wgpu::Instance::default();
    pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
        .iter()
        .map(|adapter| AdapterDescriptor::from_info(&adapter.get_info()))
        .collect()
}

pub fn save_adapter_preference(path: &Path, key: &str) -> NativeResult<AdapterDescriptor> {
    let adapter = enumerate_adapters()
        .into_iter()
        .find(|adapter| adapter.key == key)
        .ok_or("cannot save an unavailable GPU adapter")?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("pending.json");
    fs::write(&temporary, serde_json::to_vec_pretty(&adapter)?)?;
    match fs::rename(&temporary, path) {
        Ok(()) => {}
        // Some Windows encrypted/redirected application-data directories reject
        // even a same-parent rename with ERROR_NOT_SAME_DEVICE. Copy only this
        // tiny preference file; this fallback is not an atomic replacement.
        Err(error)
            if error.kind() == std::io::ErrorKind::CrossesDevices
                || (cfg!(windows) && error.raw_os_error() == Some(17)) =>
        {
            fs::copy(&temporary, path)?;
            fs::remove_file(&temporary)?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(adapter)
}

pub fn load_adapter_preference(path: &Path) -> NativeResult<Option<AdapterDescriptor>> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice(&fs::read(path)?)?))
}

fn select_adapter(
    instance: &wgpu::Instance,
    requested: Option<&str>,
) -> NativeResult<(wgpu::Adapter, AdapterDescriptor, Option<String>)> {
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    let candidate = requested.and_then(|key| {
        adapters
            .iter()
            .position(|adapter| AdapterDescriptor::from_info(&adapter.get_info()).key == key)
    });
    let fallback = requested.filter(|_| candidate.is_none()).map(|key| {
        format!("preferred adapter {key:?} was unavailable; selected a compatible fallback")
    });
    let index = candidate
        .or_else(|| {
            adapters
                .iter()
                .position(|a| a.get_info().device_type == wgpu::DeviceType::DiscreteGpu)
        })
        .or_else(|| {
            adapters
                .iter()
                .position(|a| a.get_info().device_type == wgpu::DeviceType::IntegratedGpu)
        })
        .or_else(|| (!adapters.is_empty()).then_some(0))
        .ok_or("wgpu found no native adapters")?;
    let adapter = adapters
        .into_iter()
        .nth(index)
        .ok_or("selected adapter disappeared")?;
    let descriptor = AdapterDescriptor::from_info(&adapter.get_info());
    Ok((adapter, descriptor, fallback))
}

fn ffmpeg_base() -> Command {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-n"]);
    cmd
}

fn configure_video_bitrate(cmd: &mut Command, bitrate: Option<u32>, hardware: bool) {
    if let Some(bitrate) = bitrate {
        // Average VBR target. Do not also impose CRF/CQ (quality mode).
        cmd.arg("-b:v").arg(bitrate.to_string());
    } else if hardware {
        cmd.args(["-cq", "20", "-b:v", "0"]);
    } else {
        cmd.args(["-crf", "20"]);
    }
}

pub fn convert(
    input: &Path,
    output: &Path,
    resize: ResizeSpec,
    profile: OutputProfileId,
    route: ProcessingRoute,
    adapter_key: Option<&str>,
) -> NativeResult<ConversionReport> {
    convert_with_control(
        input,
        output,
        resize,
        profile,
        route,
        adapter_key,
        &CancellationToken::default(),
    )
}

pub fn convert_with_control(
    input: &Path,
    output: &Path,
    resize: ResizeSpec,
    profile: OutputProfileId,
    route: ProcessingRoute,
    adapter_key: Option<&str>,
    cancel: &CancellationToken,
) -> NativeResult<ConversionReport> {
    convert_with_control_inner(
        input,
        output,
        resize,
        profile,
        route,
        NativeRunOptions {
            adapter_key,
            inject_device_loss_after_frames: None,
            ..Default::default()
        },
        cancel,
    )
}

#[derive(Clone, Copy, Default)]
struct NativeRunOptions<'a> {
    frame_rate: FrameRateSpec,
    cfr_plan: Option<CfrPlan>,
    bitrate: Option<VideoBitrate>,
    video_bitrate_bps: Option<u32>,
    adapter_key: Option<&'a str>,
    inject_device_loss_after_frames: Option<u64>,
    inspection: Option<&'a InspectionCache>,
    on_stage: Option<&'a dyn Fn(JobStage)>,
    hardware_gpu: Option<&'a NvidiaGpu>,
}

impl NativeRunOptions<'_> {
    fn stage(self, stage: JobStage) {
        if let Some(callback) = self.on_stage {
            callback(stage);
        }
    }
}

fn convert_with_control_inner(
    input: &Path,
    output: &Path,
    resize: ResizeSpec,
    profile: OutputProfileId,
    route: ProcessingRoute,
    options: NativeRunOptions<'_>,
    cancel: &CancellationToken,
) -> NativeResult<ConversionReport> {
    let total_started = Instant::now();
    cancel.check()?;
    if !matches!(
        profile,
        OutputProfileId::Mp4H264Aac | OutputProfileId::Mp4H265Main10Aac
    ) {
        return Err(format!(
            "native M4 harness does not yet implement profile {}",
            profile.as_str()
        )
        .into());
    }
    if route.uses_wgpu() && profile == OutputProfileId::Mp4H265Main10Aac {
        return Err(
            "shared-wgpu route cannot preserve 10-bit precision through its RGBA8 bridge".into(),
        );
    }
    let allow_10bit = profile == OutputProfileId::Mp4H265Main10Aac;
    options.stage(JobStage::Inspecting);
    let local_cache = InspectionCache::default();
    let (inspected, inspection_reused) = options
        .inspection
        .unwrap_or(&local_cache)
        .load(input, cancel)?;
    let source = inspected.source.clone();
    let input_timeline = &inspected.timeline;
    if !allow_10bit && matches!(source.pixel_format.as_str(), "yuv420p10le" | "p010le") {
        return Err(format!(
            "native 8-bit output has unsupported pixel format: {}",
            source.pixel_format
        )
        .into());
    }
    options.stage(JobStage::Preparing);
    if route.uses_wgpu()
        && let Some(reason) = source.shared_gpu_limitation()
    {
        return Err(reason.into());
    }
    let hardware_gpu = if route.uses_nvidia() {
        if source.sample_aspect_ratio != "1:1" || source.has_display_matrix {
            return Err("NVIDIA CUDA route currently requires square pixels and no display transform; use software FFmpeg".into());
        }
        Some(select_nvidia_gpu(options.adapter_key)?)
    } else {
        None
    };
    let hardware_adapter = if route == ProcessingRoute::SharedWgpuNvidia {
        let device = hardware_gpu.as_ref().ok_or("NVIDIA identity unavailable")?;
        Some(
            enumerate_adapters()
                .into_iter()
                .find(|adapter| {
                    adapter.vendor == device.vendor
                        && adapter.device == device.device
                        && adapter.name == device.name
                        && options.adapter_key.is_none_or(|key| key == adapter.key)
                })
                .ok_or("no matching wgpu adapter for the NVIDIA codec GPU")?,
        )
    } else {
        None
    };
    cancel.check()?;
    let input_size = if !route.uses_wgpu() {
        Size::new(source.display_width, source.display_height)?
    } else {
        Size::new(source.width, source.height)?
    };
    let output_size = resize.output_size(input_size)?;
    let cfr_plan = CfrPlan::for_source(&source, input_timeline, options.frame_rate)?;
    let video_bitrate_bps = options
        .bitrate
        .map(|bitrate| source.resolve_bitrate(output_size, profile, bitrate, options.frame_rate))
        .transpose()?;
    let mut partial = create_partial(output)?;
    let preflight_ms = total_started.elapsed().as_millis();
    options.stage(JobStage::Converting);
    let started = Instant::now();
    let result = match route {
        ProcessingRoute::DirectFfmpeg => convert_direct(
            input,
            &partial.path,
            output_size,
            &source,
            profile,
            NativeRunOptions {
                cfr_plan,
                video_bitrate_bps,
                ..options
            },
            cancel,
        )
        .map(|()| (None, 0, 0, None)),
        ProcessingRoute::NvidiaFfmpeg => convert_direct(
            input,
            &partial.path,
            output_size,
            &source,
            profile,
            NativeRunOptions {
                cfr_plan,
                video_bitrate_bps,
                hardware_gpu: hardware_gpu.as_ref(),
                ..options
            },
            cancel,
        )
        .map(|()| (None, 0, 0, None)),
        ProcessingRoute::SharedWgpu | ProcessingRoute::SharedWgpuNvidia => convert_gpu(
            input,
            &partial.path,
            output_size,
            &source,
            input_timeline,
            NativeRunOptions {
                cfr_plan,
                video_bitrate_bps,
                adapter_key: hardware_adapter
                    .as_ref()
                    .map(|adapter| adapter.key.as_str())
                    .or(options.adapter_key),
                hardware_gpu: hardware_gpu.as_ref(),
                ..options
            },
            cancel,
        ),
    };
    let (adapter, uploaded, downloaded, fallback) = result?;
    let conversion_elapsed = started.elapsed().as_millis();
    let verification_started = Instant::now();
    options.stage(JobStage::Verifying);
    let (verified, output_timeline) = inspect_source(&partial.path, false, allow_10bit, cancel)?;
    cancel.check()?;
    if verified.width != output_size.width
        || verified.height != output_size.height
        || verified.frame_count != cfr_plan.map_or(source.frame_count, |plan| plan.count())
        || verified.audio_codec.is_some() != source.audio_codec.is_some()
        || verified.codec != if allow_10bit { "hevc" } else { "h264" }
        || verified.pixel_format
            != if allow_10bit {
                "yuv420p10le"
            } else {
                "yuv420p"
            }
        || (allow_10bit
            && (verified.codec_profile.as_deref() != Some("Main 10")
                || verified.codec_tag.as_deref() != Some("hvc1")))
        || verified.sample_aspect_ratio != "1:1"
        || verified.rotation_degrees != 0
        || verified.has_display_matrix
        || verified
            .audio_codec
            .as_deref()
            .is_some_and(|codec| codec != "aac")
    {
        return Err(
            "native output stream metadata does not match expected size/frame/audio counts".into(),
        );
    }
    if let Some(plan) = cfr_plan {
        plan.verify(&output_timeline)?;
        verify_track_coverage(
            &source,
            &verified,
            Some(plan.duration_us()?),
            source
                .audio_start_us
                .map_or(source.video_start_us, |audio| {
                    audio.min(source.video_start_us)
                }),
        )?;
    } else {
        verify_output_timeline(&source, input_timeline, &verified, &output_timeline)?;
    }
    for (label, input_tag, output_tag) in [
        ("range", &source.color_range, &verified.color_range),
        ("space", &source.color_space, &verified.color_space),
        ("transfer", &source.color_transfer, &verified.color_transfer),
        (
            "primaries",
            &source.color_primaries,
            &verified.color_primaries,
        ),
    ] {
        if input_tag.as_deref().is_some_and(|tag| tag != "unknown") && input_tag != output_tag {
            return Err(format!(
                "native output color {label} changed from {input_tag:?} to {output_tag:?}"
            )
            .into());
        }
    }
    inspected.check_unchanged(input)?;
    let verification_ms = verification_started.elapsed().as_millis();
    options.stage(JobStage::Publishing);
    let size = finish_output(&mut partial, output)?;
    Ok(ConversionReport {
        frame_rate: options.frame_rate,
        output_frame_count: verified.frame_count,
        video_bitrate_bps,
        route: match route {
            ProcessingRoute::DirectFfmpeg => "direct-ffmpeg",
            ProcessingRoute::NvidiaFfmpeg => "nvidia-ffmpeg",
            ProcessingRoute::SharedWgpu => "shared-wgpu",
            ProcessingRoute::SharedWgpuNvidia => "shared-wgpu-nvidia",
        }
        .into(),
        profile: profile.as_str().into(),
        input: source.clone(),
        output_width: output_size.width,
        output_height: output_size.height,
        frames_processed: source.frame_count,
        output_bytes: size,
        elapsed_ms: conversion_elapsed,
        verification_ms,
        adapter,
        explicit_cpu_to_gpu_bytes: uploaded,
        explicit_gpu_to_cpu_bytes: downloaded,
        adapter_fallback: fallback,
        video_encoder: match (hardware_gpu.is_some(), allow_10bit) {
            (true, true) => "hevc_nvenc",
            (true, false) => "h264_nvenc",
            (false, true) => "libx265",
            (false, false) => "libx264",
        }
        .into(),
        hardware_gpu,
        inspection_reused,
        preflight_ms,
        total_ms: total_started.elapsed().as_millis(),
        codec_gpu_to_cpu_bytes: if route == ProcessingRoute::SharedWgpuNvidia {
            u64::from(source.width) * u64::from(source.height) * 3 / 2 * source.frame_count
        } else {
            0
        },
        codec_cpu_to_gpu_bytes: if route == ProcessingRoute::SharedWgpuNvidia {
            u64::from(output_size.width) * u64::from(output_size.height) * 3 / 2
                * source.frame_count
        } else {
            0
        },
    })
}

fn convert_direct(
    input: &Path,
    partial: &Path,
    output: Size,
    source: &SourceInfo,
    profile: OutputProfileId,
    options: NativeRunOptions<'_>,
    cancel: &CancellationToken,
) -> NativeResult<()> {
    cancel.check()?;
    let hardware_gpu = options.hardware_gpu;
    let mut cmd = ffmpeg_base();
    cmd.args(["-copyts", "-start_at_zero"]);
    if let Some(gpu) = hardware_gpu {
        // CUDA ordinal 0 is scoped to this UUID, not a wgpu enumeration index.
        cmd.env("CUDA_VISIBLE_DEVICES", &gpu.uuid).args([
            "-hwaccel",
            "cuda",
            "-hwaccel_device",
            "0",
            "-hwaccel_output_format",
            "cuda",
        ]);
    }
    cmd.arg("-i").arg(input).args(["-map", "0:v:0"]);
    if source.audio_codec.is_some() {
        cmd.args(["-map", "0:a:0"]);
    }
    let mut filter = if hardware_gpu.is_some() {
        format!(
            "scale_cuda={}:{}:format={}:interp_algo=bilinear,setsar=1",
            output.width,
            output.height,
            if profile.video_bit_depth() == 10 {
                "p010le"
            } else {
                "yuv420p"
            }
        )
    } else {
        format!(
            "scale={}:{}:flags=bilinear,format={},setsar=1",
            output.width,
            output.height,
            if profile.video_bit_depth() == 10 {
                "yuv420p10le"
            } else {
                "yuv420p"
            }
        )
    };
    append_fps_filter(&mut filter, options.cfr_plan)?;
    cmd.args([
        "-vf",
        &filter,
        "-fps_mode",
        "passthrough",
        "-enc_time_base:v",
        "1:90000",
        "-c:v",
        if hardware_gpu.is_some() && profile.video_bit_depth() == 10 {
            "hevc_nvenc"
        } else if hardware_gpu.is_some() {
            "h264_nvenc"
        } else if profile.video_bit_depth() == 10 {
            "libx265"
        } else {
            "libx264"
        },
        "-preset",
        if hardware_gpu.is_some() {
            "p4"
        } else {
            "veryfast"
        },
    ]);
    if hardware_gpu.is_some() {
        cmd.args(["-rc", "vbr", "-gpu", "0"]);
        if profile.video_bit_depth() == 10 {
            cmd.args(["-profile:v", "main10"]);
        }
    } else {
        cmd.args([
            "-pix_fmt",
            if profile.video_bit_depth() == 10 {
                "yuv420p10le"
            } else {
                "yuv420p"
            },
        ]);
    }
    configure_video_bitrate(&mut cmd, options.video_bitrate_bps, hardware_gpu.is_some());
    if profile.video_bit_depth() == 10 {
        cmd.args(["-tag:v", "hvc1"]);
        let mut x265_color = vec!["log-level=error".to_owned()];
        for (key, value) in [
            (
                "range",
                source
                    .color_range
                    .as_deref()
                    .filter(|tag| *tag != "unknown")
                    .map(|_| "limited"),
            ),
            ("colormatrix", source.color_space.as_deref()),
            ("transfer", source.color_transfer.as_deref()),
            ("colorprim", source.color_primaries.as_deref()),
        ] {
            if let Some(value) = value.filter(|value| *value != "unknown") {
                x265_color.push(format!("{key}={value}"));
            }
        }
        if hardware_gpu.is_none() {
            cmd.arg("-x265-params").arg(x265_color.join(":"));
        }
    }
    if source.audio_codec.is_some() {
        cmd.args(["-c:a", "aac", "-b:a", "192k", "-ar", "48000"]);
    }
    cmd.args(["-movflags", "+faststart", "-f", "mp4"])
        .arg(partial);
    let mut child = ChildGuard::spawn(&mut cmd)?;
    let status = child.wait_cancellable(cancel)?;
    if !status.success() {
        return Err(child
            .failure(
                if hardware_gpu.is_some() {
                    "NVIDIA FFmpeg (no CPU fallback; choose software FFmpeg if unsupported)"
                } else {
                    "direct FFmpeg"
                },
                status,
            )
            .into());
    }
    Ok(())
}

fn convert_gpu(
    input: &Path,
    partial: &Path,
    output: Size,
    source: &SourceInfo,
    timeline: &[i128],
    options: NativeRunOptions<'_>,
    cancel: &CancellationToken,
) -> NativeResult<(Option<AdapterDescriptor>, u64, u64, Option<String>)> {
    cancel.check()?;
    let mut gpu = GpuProcessor::new(
        Size::new(source.width, source.height)?,
        output,
        options.adapter_key,
    )?;
    cancel.check()?;
    let mut decoder = ffmpeg_base();
    if let Some(device) = options.hardware_gpu {
        if gpu.adapter.vendor != device.vendor
            || gpu.adapter.device != device.device
            || gpu.adapter.name != device.name
        {
            return Err("wgpu/codec GPU identity mismatch; no hardware fallback".into());
        }
        decoder.env("CUDA_VISIBLE_DEVICES", &device.uuid).args([
            "-hwaccel",
            "cuda",
            "-hwaccel_device",
            "0",
            "-hwaccel_output_format",
            "cuda",
        ]);
    }
    decoder.arg("-i").arg(input);
    if options.hardware_gpu.is_some() {
        // Explicit NVDEC download; CPU converts NV12 to RGBA for the wgpu bridge.
        decoder.args(["-vf", "hwdownload,format=nv12,format=rgba"]);
    }
    decoder
        .args([
            "-map", "0:v:0", "-f", "rawvideo", "-pix_fmt", "rgba", "-vsync", "0", "pipe:1",
        ])
        .stdout(Stdio::piped());
    let mut decoder = ChildGuard::spawn(&mut decoder)?;
    let mut encoder = ffmpeg_base();
    if let Some(device) = options.hardware_gpu {
        encoder.env("CUDA_VISIBLE_DEVICES", &device.uuid).args([
            "-init_hw_device",
            "cuda=codec:0",
            "-filter_hw_device",
            "codec",
        ]);
    }
    let origin = source
        .audio_start_us
        .map_or(source.video_start_us, |audio| {
            audio.min(source.video_start_us)
        });
    encoder.args(["-copyts", "-f", "matroska", "-i", "pipe:0"]);
    if source.audio_codec.is_some() {
        encoder
            .arg("-itsoffset")
            .arg(format!("-{}.{:06}", origin / 1_000_000, origin % 1_000_000))
            .arg("-i")
            .arg(input);
    }
    encoder.args(["-map", "0:v:0"]);
    if source.audio_codec.is_some() {
        encoder.args(["-map", "1:a:0"]);
    }
    if options.hardware_gpu.is_some() {
        // Explicit NV12 upload after the shared RGBA8 readback/CPU conversion.
        // The Matroska bridge contains full-range RGBA, not source-range YUV.
        // Output tags/setparams alone do not select swscale's conversion matrix:
        // an automatic RGBA -> NV12 conversion can otherwise use BT.601 while
        // NVENC advertises BT.709. Select the pixel conversion before upload.
        let mut filter = "scale=in_range=full:out_range=limited".to_string();
        if source.color_space.as_deref() == Some("bt709") {
            filter.push_str(":out_color_matrix=bt709");
        }
        filter.push_str(",format=nv12,hwupload_cuda");
        append_fps_filter(&mut filter, options.cfr_plan)?;
        let colors: Vec<_> = [
            ("range", &source.color_range),
            ("colorspace", &source.color_space),
            ("color_trc", &source.color_transfer),
            ("color_primaries", &source.color_primaries),
        ]
        .into_iter()
        .filter_map(|(name, value)| {
            value
                .as_deref()
                .filter(|value| *value != "unknown")
                .map(|value| format!("{name}={}", if value == "tv" { "limited" } else { value }))
        })
        .collect();
        if !colors.is_empty() {
            filter.push_str(&format!(",setparams={}", colors.join(":")));
        }
        encoder.args([
            "-vf",
            &filter,
            "-c:v",
            "h264_nvenc",
            "-gpu",
            "0",
            "-preset",
            "p4",
            "-rc",
            "vbr",
        ]);
    } else {
        let mut filter = String::new();
        append_fps_filter(&mut filter, options.cfr_plan)?;
        if !filter.is_empty() {
            encoder.args(["-vf", &filter]);
        }
        encoder.args([
            "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p",
        ]);
    }
    configure_video_bitrate(
        &mut encoder,
        options.video_bitrate_bps,
        options.hardware_gpu.is_some(),
    );
    encoder.args(["-fps_mode", "passthrough", "-enc_time_base:v", "1:90000"]);
    for (flag, value) in [
        ("-color_range", &source.color_range),
        ("-colorspace", &source.color_space),
        ("-color_trc", &source.color_transfer),
        ("-color_primaries", &source.color_primaries),
    ] {
        if let Some(value) = value.as_deref().filter(|value| *value != "unknown") {
            encoder.arg(flag).arg(value);
        }
    }
    let x264_color: Vec<_> = [
        ("range", &source.color_range),
        ("colormatrix", &source.color_space),
        ("transfer", &source.color_transfer),
        ("colorprim", &source.color_primaries),
    ]
    .into_iter()
    .filter_map(|(key, value)| {
        value
            .as_deref()
            .filter(|value| *value != "unknown")
            .map(|value| format!("{key}={value}"))
    })
    .collect();
    if options.hardware_gpu.is_none() && !x264_color.is_empty() {
        encoder.arg("-x264-params").arg(x264_color.join(":"));
    }
    if source.audio_codec.is_some() {
        encoder.args(["-c:a", "aac", "-b:a", "192k", "-ar", "48000"]);
    }
    encoder
        .args(["-movflags", "+faststart", "-f", "mp4"])
        .arg(partial)
        .stdin(Stdio::piped());
    let mut encoder = ChildGuard::spawn(&mut encoder)?;
    // The main thread may block inside a pipe read or write. A separate watcher
    // terminates both processes when cancellation is requested, releasing the pipes.
    let _cancellation_watch = CancellationWatch::new_with_failure(
        cancel,
        &[&decoder, &encoder],
        Some(gpu.device_lost_signal.clone()),
    );
    let frame_len = usize::try_from(u64::from(source.width) * u64::from(source.height) * 4)?;
    let mut frame = vec![0_u8; frame_len];
    let result = (|| -> NativeResult<u64> {
        let mut reader = decoder.take_stdout()?;
        let mut writer = encoder.take_stdin()?;
        frame_stream::start(&mut writer, output)?;
        let mut count = 0_u64;
        loop {
            cancel.check()?;
            gpu.check_device_lost()?;
            let mut filled = 0;
            while filled < frame.len() {
                cancel.check()?;
                gpu.check_device_lost()?;
                let read = match reader.read(&mut frame[filled..]) {
                    Ok(read) => read,
                    Err(error) => {
                        cancel.check()?;
                        gpu.check_device_lost()?;
                        return Err(error.into());
                    }
                };
                if read == 0 {
                    break;
                }
                filled += read;
            }
            if filled == 0 {
                gpu.check_device_lost()?;
                break;
            }
            if filled != frame.len() {
                return Err("decoder ended with a partial RGBA frame".into());
            }
            let index = usize::try_from(count)?;
            let pts = *timeline
                .get(index)
                .ok_or("decoder produced more frames than the inspected timeline")?;
            let end = timeline
                .get(index + 1)
                .copied()
                .or_else(|| {
                    source
                        .video_duration_us
                        .and_then(|duration| source.video_start_us.checked_add(duration))
                })
                .ok_or("last frame duration is unavailable")?;
            frame_stream::frame_header(
                &mut writer,
                pts.checked_sub(origin).ok_or("frame origin overflow")?,
                end.checked_sub(pts)
                    .filter(|value| *value > 0)
                    .ok_or("invalid frame duration")?,
                u64::from(output.width) * u64::from(output.height) * 4,
            )?;
            if let Err(error) = gpu.process(&frame, &mut writer, cancel) {
                cancel.check()?;
                return Err(error);
            }
            count += 1;
            if options.inject_device_loss_after_frames == Some(count) {
                gpu.device.destroy();
            }
            if count > source.frame_count {
                return Err("decoder produced more frames than the probed source".into());
            }
        }
        drop(writer);
        gpu.check_device_lost()?;
        if count != source.frame_count {
            return Err(format!(
                "decoder produced {count} frames, expected {}",
                source.frame_count
            )
            .into());
        }
        Ok(count)
    })();
    cancel.check()?;
    if let Err(error) = result {
        return Err(format!("shared pipeline failed: {error}; decoder: {}; encoder: {}. No implicit CPU codec fallback.", decoder.diagnostic().trim(), encoder.diagnostic().trim()).into());
    }
    let decode_status = decoder.wait_cancellable(cancel)?;
    let encode_status = encoder.wait_cancellable(cancel)?;
    cancel.check()?;
    if !decode_status.success() || !encode_status.success() {
        return Err(format!(
            "native pipeline failed: decoder={decode_status}, encoder={encode_status}"
        )
        .into());
    }
    Ok((
        Some(gpu.adapter),
        gpu.uploaded,
        gpu.downloaded,
        gpu.fallback,
    ))
}

struct GpuProcessor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: ResizePipeline,
    source: wgpu::Texture,
    target: wgpu::Texture,
    transform: wgpu::Buffer,
    readback: wgpu::Buffer,
    input_size: Size,
    output_size: Size,
    padded_bytes_per_row: u32,
    adapter: AdapterDescriptor,
    fallback: Option<String>,
    uploaded: u64,
    downloaded: u64,
    device_lost: Arc<Mutex<Option<String>>>,
    device_lost_signal: Arc<AtomicBool>,
}

impl GpuProcessor {
    fn new(input: Size, output: Size, requested: Option<&str>) -> NativeResult<Self> {
        let instance = wgpu::Instance::default();
        let (adapter, descriptor, fallback) = select_adapter(&instance, requested)?;
        let max_dimension = adapter.limits().max_texture_dimension_2d;
        if [input.width, input.height, output.width, output.height]
            .into_iter()
            .any(|v| v > max_dimension)
        {
            return Err(
                format!("selected GPU maximum texture dimension is {max_dimension}").into(),
            );
        }
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        let device_lost = Arc::new(Mutex::new(None));
        let device_lost_signal = Arc::new(AtomicBool::new(false));
        let callback_state = device_lost.clone();
        let callback_signal = device_lost_signal.clone();
        device.set_device_lost_callback(move |reason, message| {
            if let Ok(mut state) = callback_state.lock() {
                *state = Some(format!("native GPU device lost ({reason:?}): {message}"));
            }
            callback_signal.store(true, Ordering::Release);
        });
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let make_texture = |size: Size, usage: wgpu::TextureUsages| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("native resize frame"),
                size: wgpu::Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let source = make_texture(
            input,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let target = make_texture(
            output,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let pipeline = ResizePipeline::new(&device, format);
        let transform = pipeline.create_transform_buffer(&device, &queue, Rotation::Deg0, false);
        let padded_bytes_per_row = output
            .width
            .checked_mul(4)
            .and_then(|v| v.checked_add(255))
            .map(|v| v & !255)
            .ok_or("output row byte count overflow")?;
        let readback_bytes = u64::from(padded_bytes_per_row) * u64::from(output.height);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("native FFmpeg RGBA readback"),
            size: readback_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Ok(Self {
            device,
            queue,
            pipeline,
            source,
            target,
            transform,
            readback,
            input_size: input,
            output_size: output,
            padded_bytes_per_row,
            adapter: descriptor,
            fallback,
            uploaded: 0,
            downloaded: 0,
            device_lost,
            device_lost_signal,
        })
    }

    fn check_device_lost(&self) -> NativeResult<()> {
        if let Some(message) = self
            .device_lost
            .lock()
            .map_err(|_| "GPU device-loss state was poisoned")?
            .as_ref()
        {
            return Err(message.clone().into());
        }
        Ok(())
    }

    fn process(
        &mut self,
        input: &[u8],
        output: &mut impl Write,
        cancel: &CancellationToken,
    ) -> NativeResult<()> {
        cancel.check()?;
        self.check_device_lost()?;
        if let Err(error) = self.device.poll(wgpu::PollType::Poll) {
            self.check_device_lost()?;
            return Err(error.into());
        }
        self.check_device_lost()?;
        let input_len = u64::from(self.input_size.width) * u64::from(self.input_size.height) * 4;
        if u64::try_from(input.len())? != input_len {
            return Err("invalid input frame byte count".into());
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.source,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            input,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.input_size.width * 4),
                rows_per_image: Some(self.input_size.height),
            },
            wgpu::Extent3d {
                width: self.input_size.width,
                height: self.input_size.height,
                depth_or_array_layers: 1,
            },
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("native resize"),
            });
        self.pipeline.record_resize(
            &self.device,
            &mut encoder,
            &self.source.create_view(&Default::default()),
            &self.transform,
            &self.target.create_view(&Default::default()),
            self.output_size,
        );
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_bytes_per_row),
                    rows_per_image: Some(self.output_size.height),
                },
            },
            wgpu::Extent3d {
                width: self.output_size.width,
                height: self.output_size.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let slice = self.readback.slice(..);
        let (sender, receiver) = mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let wait_started = Instant::now();
        loop {
            cancel.check()?;
            self.check_device_lost()?;
            match receiver.try_recv() {
                Ok(result) => {
                    if let Err(error) = result {
                        self.check_device_lost()?;
                        return Err(error.into());
                    }
                    break;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("GPU map callback disconnected".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
            if wait_started.elapsed() > Duration::from_secs(30) {
                return Err("GPU readback timed out".into());
            }
            match self.device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_millis(100)),
            }) {
                Ok(_) | Err(wgpu::PollError::Timeout) => {}
                Err(error) => {
                    self.check_device_lost()?;
                    return Err(error.into());
                }
            }
        }
        cancel.check()?;
        self.check_device_lost()?;
        let write_result = {
            let mapped = slice.get_mapped_range()?;
            let row_bytes = usize::try_from(self.output_size.width * 4)?;
            mapped
                .chunks_exact(self.padded_bytes_per_row as usize)
                .take(self.output_size.height as usize)
                .try_for_each(|row| output.write_all(&row[..row_bytes]))
        };
        self.readback.unmap();
        self.check_device_lost()?;
        write_result?;
        self.uploaded = self
            .uploaded
            .checked_add(input_len)
            .ok_or("upload byte counter overflow")?;
        self.downloaded = self
            .downloaded
            .checked_add(u64::from(self.output_size.width) * u64::from(self.output_size.height) * 4)
            .ok_or("readback byte counter overflow")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_core::estimate_output_bytes;

    #[test]
    fn cfr_plan_checks_full_grid_coverage_and_fractional_rates() {
        let source = probe_source(&fixture("m2-h264-aac.mp4")).unwrap();
        for (numerator, denominator, count) in [(15, 1, 30), (60, 1, 120), (30_000, 1001, 60)] {
            let plan = CfrPlan::for_source(
                &source,
                &[0],
                FrameRateSpec::Constant {
                    numerator,
                    denominator,
                },
            )
            .unwrap()
            .unwrap();
            assert_eq!(plan.count(), count);
            let mut timeline: Vec<_> = (plan.first..plan.end)
                .map(|index| plan.timestamp_us(index).unwrap())
                .collect();
            plan.verify(&timeline).unwrap();
            timeline[1] += 2_000;
            assert!(plan.verify(&timeline).is_err());
            assert!(plan.verify(&timeline[..timeline.len() - 1]).is_err());
        }
        assert!(
            CfrPlan::for_source(&source, &[0], FrameRateSpec::Original)
                .unwrap()
                .is_none()
        );
        let mut unknown = source;
        unknown.video_duration_us = None;
        assert!(
            CfrPlan::for_source(
                &unknown,
                &[0],
                FrameRateSpec::Constant {
                    numerator: 25,
                    denominator: 1
                }
            )
            .is_err()
        );
    }

    #[test]
    fn cfr_plan_retains_relative_audio_video_offset_for_vfr() {
        let source = probe_source(&fixture("m35-vfr-offset.mp4")).unwrap();
        let plan = CfrPlan::for_source(
            &source,
            &[source.video_start_us],
            FrameRateSpec::Constant {
                numerator: 60,
                denominator: 1,
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(plan.first, 1);
        assert_eq!(plan.end, 120);
        assert_eq!(plan.count(), 119);
    }

    #[test]
    fn cfr_endpoint_does_not_ceil_a_rounded_two_frame_duration_to_three() {
        let mut source = probe_source(&fixture("m2-h264-aac.mp4")).unwrap();
        source.video_duration_us = Some(66_667);
        source.video_duration_ticks = Some(2);
        source.video_time_base = Some("1/30".into());
        let spec = FrameRateSpec::Constant {
            numerator: 30,
            denominator: 1,
        };
        let plan = CfrPlan::for_source(&source, &[0], spec).unwrap().unwrap();
        assert_eq!(plan.count(), 2);
        let mut filter = String::new();
        append_fps_filter(&mut filter, Some(plan)).unwrap();
        assert!(filter.contains("trim=end_frame=2"));
        source.video_duration_ticks = None;
        assert!(CfrPlan::for_source(&source, &[0], spec).is_err());
    }

    #[test]
    fn session_reuses_inspection_and_reports_ordered_stages_and_total_time() {
        let session = NativeSession::new(None).unwrap();
        let source = fixture("m35-vfr-offset.mp4");
        let token = CancellationToken::default();
        let directory = std::env::temp_dir().join(format!("diaxus-stage-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        session.inspect(&source, &token).unwrap();
        let stages = std::cell::RefCell::new(Vec::new());
        let output = directory.join("output.mp4");
        let report = session
            .convert_job(
                NativeJob {
                    input: &source,
                    output: &output,
                    resize: ResizeSpec::Percent(50),
                    profile: OutputProfileId::Mp4H264Aac,
                    route: ProcessingRoute::DirectFfmpeg,
                    bitrate: None,
                    frame_rate: FrameRateSpec::Original,
                },
                &token,
                |stage| stages.borrow_mut().push(stage),
            )
            .unwrap()
            .conversion;
        assert!(report.inspection_reused);
        assert_eq!(
            *stages.borrow(),
            [
                JobStage::Inspecting,
                JobStage::Preparing,
                JobStage::Converting,
                JobStage::Verifying,
                JobStage::Publishing
            ]
        );
        assert!(
            report.total_ms >= report.preflight_ms + report.elapsed_ms + report.verification_ms
        );
        assert_eq!(report.frames_processed, 36);
        assert!(!session.status().active);
        session.shutdown();
        assert!(session.inspect(&source, &token).is_err());
        fs::remove_file(output).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn shutdown_waits_for_cleanup_and_rejects_delayed_jobs() {
        let session = Arc::new(NativeSession::new(None).unwrap());
        let token = CancellationToken::default();
        session.state.lock().unwrap().active = Some(token.clone());
        let worker_session = session.clone();
        let (cleaned, cleanup) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let worker = thread::spawn(move || {
            let guard = ActiveSessionJob {
                session: &worker_session,
            };
            while !token.is_cancelled() {
                thread::yield_now();
            }
            cleaned.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(2)).unwrap();
            drop(guard);
        });
        let closing = session.clone();
        let (done, finished) = mpsc::channel();
        let closer = thread::spawn(move || {
            closing.shutdown();
            done.send(()).unwrap();
        });
        cleanup.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(
            finished.try_recv().is_err(),
            "shutdown returned before resource cleanup"
        );
        release.send(()).unwrap();
        finished.recv_timeout(Duration::from_secs(2)).unwrap();
        worker.join().unwrap();
        closer.join().unwrap();
        assert!(!session.status().active);
        let error = session
            .convert(
                Path::new("missing-input.mp4"),
                Path::new("must-not-create.mp4"),
                ResizeSpec::Original,
                OutputProfileId::Mp4H264Aac,
                ProcessingRoute::DirectFfmpeg,
            )
            .unwrap_err()
            .to_string();
        assert_eq!(error, "native session is closed");
        assert!(session.switch_adapter(None).is_err());
        session.shutdown(); // Idempotent, including an already idle session.
    }

    #[test]
    fn cancel_before_worker_start_does_not_open_input_or_output() {
        let session = NativeSession::new(None).unwrap();
        let token = CancellationToken::default();
        token.cancel();
        let error = session
            .convert_with_cancellation(
                Path::new("missing-input.mp4"),
                Path::new("must-not-create.mp4"),
                ResizeSpec::Original,
                OutputProfileId::Mp4H264Aac,
                ProcessingRoute::DirectFfmpeg,
                &token,
            )
            .unwrap_err()
            .to_string();
        assert_eq!(error, "native conversion cancelled");
        assert!(!session.status().active);
    }

    #[cfg(windows)]
    #[test]
    fn cancellation_reaps_launcher_descendants_holding_a_pipe() {
        let mut command = Command::new("cmd.exe");
        command
            .args([
                "/D",
                "/C",
                "powershell.exe",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Write-Output ready; Start-Sleep -Seconds 5",
            ])
            .stdout(Stdio::piped());
        let child = ChildGuard::spawn(&mut command).unwrap();
        let mut output = BufReader::new(child.take_stdout().unwrap());
        let mut ready = String::new();
        output.read_line(&mut ready).unwrap();
        assert_eq!(ready.trim(), "ready");
        let token = CancellationToken::default();
        let _watch = CancellationWatch::new(&token, &[&child]);
        let started = Instant::now();
        token.cancel();
        let mut byte = [0];
        assert_eq!(output.read(&mut byte).unwrap(), 0);
        drop(child);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "launcher descendant kept the pipe open"
        );
    }

    #[cfg(windows)]
    #[test]
    fn cancellation_interrupts_blocked_child_pipes() {
        for blocked_read in [true, false] {
            let mut command = Command::new("powershell.exe");
            command.args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 5",
            ]);
            if blocked_read {
                command.stdout(Stdio::piped());
            } else {
                command.stdin(Stdio::piped());
            }
            let mut child = ChildGuard::spawn(&mut command).unwrap();
            let token = CancellationToken::default();
            let _watch = CancellationWatch::new(&token, &[&child]);
            let signal = token.clone();
            let timer = thread::spawn(move || {
                thread::sleep(Duration::from_millis(200));
                signal.cancel();
            });
            let started = Instant::now();
            if blocked_read {
                let mut byte = [0_u8];
                let _ = child.take_stdout().unwrap().read(&mut byte);
            } else {
                let bytes = vec![0_u8; 8 * 1024 * 1024];
                let _ = child.take_stdin().unwrap().write_all(&bytes);
            }
            timer.join().unwrap();
            assert!(token.is_cancelled());
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "blocked pipe did not close promptly"
            );
            assert!(child.wait_cancellable(&token).is_err());
        }
    }

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    #[test]
    fn inspected_sources_explain_known_gpu_restrictions() {
        let cfr = probe_source_direct(&fixture("m2-h264-aac.mp4")).unwrap();
        assert!(cfr.shared_gpu_limitation().is_none());
        let vfr = probe_source_direct(&fixture("m35-vfr-offset.mp4")).unwrap();
        assert!(vfr.shared_gpu_limitation().is_none());
        let transformed = probe_source_direct(&fixture("m35-geometry-color.mp4")).unwrap();
        assert!(
            transformed
                .shared_gpu_limitation()
                .unwrap()
                .contains("display transform")
        );
        let main10 = probe_source_direct(&fixture("m4-10bit-sdr.mp4")).unwrap();
        assert!(main10.shared_gpu_limitation().unwrap().contains("8-bit"));
    }

    #[test]
    fn size_estimate_accounts_for_offsets_audio_tail_and_unknown_duration() {
        let mut source = probe_source_direct(&fixture("m2-h264-aac.mp4")).unwrap();
        source.video_start_us = 1_000_000;
        source.video_duration_us = Some(2_000_000);
        source.audio_start_us = Some(0);
        source.audio_duration_us = Some(4_000_000);
        let (_, estimate) = source
            .output_estimate(
                ResizeSpec::Original,
                OutputProfileId::Mp4H264Aac,
                VideoBitrate::BitsPerSecond(1_000_000),
            )
            .unwrap();
        assert_eq!(
            estimate,
            estimate_output_bytes(1_000_000, 192_000, 4_000_000)
        );
        source.audio_duration_us = None;
        assert!(
            source
                .output_estimate(
                    ResizeSpec::Original,
                    OutputProfileId::Mp4H264Aac,
                    VideoBitrate::Recommended
                )
                .unwrap()
                .1
                .is_none()
        );
        source.audio_codec = None;
        source.audio_start_us = None;
        assert_eq!(
            source
                .output_estimate(
                    ResizeSpec::Original,
                    OutputProfileId::Mp4H264Aac,
                    VideoBitrate::BitsPerSecond(1_000_000)
                )
                .unwrap()
                .1,
            estimate_output_bytes(1_000_000, 0, 2_000_000)
        );
    }

    #[test]
    fn bitrate_commands_do_not_mix_target_rate_with_quality_mode() {
        for hardware in [false, true] {
            let mut command = ffmpeg_base();
            configure_video_bitrate(&mut command, Some(2_000_000), hardware);
            let args: Vec<_> = command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
            assert!(args.windows(2).any(|pair| pair == ["-b:v", "2000000"]));
            assert!(!args.iter().any(|arg| arg == "-crf" || arg == "-cq"));
            let mut legacy = ffmpeg_base();
            configure_video_bitrate(&mut legacy, None, hardware);
            assert!(
                legacy
                    .get_args()
                    .any(|arg| arg == if hardware { "-cq" } else { "-crf" })
            );
        }
    }

    #[test]
    #[ignore = "requires FFmpeg, FFprobe and a native wgpu adapter"]
    fn cancel_active_jobs_and_retry_in_same_process() {
        let directory = std::env::temp_dir().join(format!(
            "media-native-lifecycle-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let input = directory.join("long-input.mp4");
        let status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:size=640x360:rate=30",
                "-t",
                "80",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&input)
            .status()
            .unwrap();
        assert!(status.success());
        let source = probe_source(&input).unwrap();
        assert_eq!(source.frame_count, 2400);
        for route in [ProcessingRoute::DirectFfmpeg, ProcessingRoute::SharedWgpu] {
            let name = if route == ProcessingRoute::DirectFfmpeg {
                "direct"
            } else {
                "wgpu"
            };
            let cancelled_output = directory.join(format!("{name}-cancelled.mp4"));
            let partial = partial_path(&cancelled_output).unwrap();
            let token = CancellationToken::default();
            let watcher_token = token.clone();
            let watcher = thread::spawn(move || {
                let start = Instant::now();
                while !partial.exists() && start.elapsed() < Duration::from_secs(10) {
                    thread::sleep(Duration::from_millis(5));
                }
                assert!(partial.exists(), "FFmpeg never opened the partial output");
                watcher_token.cancel();
            });
            let result = convert_with_control(
                &input,
                &cancelled_output,
                ResizeSpec::Percent(50),
                OutputProfileId::Mp4H264Aac,
                route,
                None,
                &token,
            );
            watcher.join().unwrap();
            assert!(result.unwrap_err().to_string().contains("cancelled"));
            assert!(!cancelled_output.exists());
            assert!(!partial_path(&cancelled_output).unwrap().exists());

            // FFmpeg cannot open this output directory. Both routes must reap
            // their children and leave the process usable for the next job.
            let failed_output = directory
                .join("missing-parent")
                .join(format!("{name}-failed.mp4"));
            let failure = convert_with_control(
                &fixture("m2-h264-aac.mp4"),
                &failed_output,
                ResizeSpec::Percent(50),
                OutputProfileId::Mp4H264Aac,
                route,
                None,
                &CancellationToken::default(),
            );
            assert!(failure.is_err());
            assert!(!failed_output.exists());

            let retry_output = directory.join(format!("{name}-retry.mp4"));
            let report = convert_with_control(
                &fixture("m2-h264-aac.mp4"),
                &retry_output,
                ResizeSpec::Percent(50),
                OutputProfileId::Mp4H264Aac,
                route,
                None,
                &CancellationToken::default(),
            )
            .unwrap();
            assert_eq!(report.frames_processed, 60);
            assert_eq!(probe_source(&retry_output).unwrap().frame_count, 60);
        }

        let adapters = enumerate_adapters();
        let discrete = adapters
            .iter()
            .find(|adapter| adapter.device_type == "DiscreteGpu");
        let integrated = adapters
            .iter()
            .find(|adapter| adapter.device_type == "IntegratedGpu");
        if let (Some(discrete), Some(integrated)) = (discrete, integrated) {
            let session = Arc::new(NativeSession::new(Some(&discrete.key)).unwrap());
            assert_eq!(session.status().device_generation, 1);
            assert_eq!(session.switch_adapter(Some(&discrete.key)).unwrap(), 1);
            assert!(
                session
                    .switch_adapter(Some("missing-native-adapter"))
                    .is_err()
            );
            assert_eq!(session.status().device_generation, 1);
            let session_job = session.clone();
            let job_input = input.clone();
            let interrupted_output = directory.join("switch-interrupted.mp4");
            let job_output = interrupted_output.clone();
            let job = thread::spawn(move || {
                session_job.convert(
                    &job_input,
                    &job_output,
                    ResizeSpec::Percent(50),
                    OutputProfileId::Mp4H264Aac,
                    ProcessingRoute::SharedWgpu,
                )
            });
            let partial = partial_path(&interrupted_output).unwrap();
            let started = Instant::now();
            while !partial.exists() && started.elapsed() < Duration::from_secs(10) {
                thread::sleep(Duration::from_millis(5));
            }
            assert!(
                partial.exists(),
                "switch test did not start an active GPU job"
            );
            assert!(session.status().active);
            let overlapping_output = directory.join("overlapping-job.mp4");
            assert!(
                session
                    .convert(
                        &fixture("m2-h264-aac.mp4"),
                        &overlapping_output,
                        ResizeSpec::Percent(50),
                        OutputProfileId::Mp4H264Aac,
                        ProcessingRoute::SharedWgpu,
                    )
                    .unwrap_err()
                    .to_string()
                    .contains("busy")
            );
            assert!(!overlapping_output.exists());
            assert_eq!(session.switch_adapter(Some(&integrated.key)).unwrap(), 2);
            assert!(
                job.join()
                    .unwrap()
                    .unwrap_err()
                    .to_string()
                    .contains("cancelled")
            );
            assert!(!interrupted_output.exists());
            assert!(!partial.exists());
            let status = session.status();
            assert_eq!(status.device_generation, 2);
            assert_eq!(status.adapter_key.as_deref(), Some(integrated.key.as_str()));
            assert!(!status.active && !status.switching);
            let switched_output = directory.join("switch-retry.mp4");
            let switched = session
                .convert(
                    &fixture("m2-h264-aac.mp4"),
                    &switched_output,
                    ResizeSpec::Percent(50),
                    OutputProfileId::Mp4H264Aac,
                    ProcessingRoute::SharedWgpu,
                )
                .unwrap();
            assert_eq!(switched.device_generation, 2);
            assert_eq!(switched.conversion.adapter.unwrap().key, integrated.key);
            assert_eq!(probe_source(&switched_output).unwrap().frame_count, 60);
        } else {
            eprintln!(
                "cross-GPU session switch skipped: discrete and integrated adapters are both required"
            );
        }
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn probes_real_cfr_video_and_audio() {
        let input = probe_source(&fixture("m2-h264-aac.mp4")).unwrap();
        assert_eq!(
            (
                input.width,
                input.height,
                input.frame_rate_num,
                input.frame_rate_den
            ),
            (640, 360, 30, 1)
        );
        assert_eq!(input.frame_count, 60);
        assert_eq!(input.audio_codec.as_deref(), Some("aac"));
    }

    #[test]
    fn probes_real_hevc_input_for_avc_output_route() {
        let input = probe_source(&fixture("m36-h265-aac.mp4")).unwrap();
        assert_eq!(input.codec, "hevc");
        assert_eq!(
            (input.width, input.height, input.frame_count),
            (320, 180, 30)
        );
        assert_eq!(input.audio_codec.as_deref(), Some("aac"));
    }

    #[test]
    fn shared_probe_accepts_real_vfr_and_nonzero_origin() {
        let info = probe_source(&fixture("m35-vfr-offset.mp4")).unwrap();
        assert!(info.variable_frame_rate);
        assert_eq!(info.video_start_us, 1_250_000);
        assert_eq!(info.frame_count, 36);
    }

    #[test]
    fn nvidia_identity_is_not_a_wgpu_enumeration_index() {
        let gpus = parse_nvidia_gpus("NVIDIA RTX, GPU-example, 0x25A210DE\n").unwrap();
        assert_eq!(gpus[0].vendor, 0x10de);
        assert_eq!(gpus[0].device, 0x25a2);
        assert_eq!(gpus[0].uuid, "GPU-example");
        assert!(parse_nvidia_gpus("invalid").is_err());
    }

    #[test]
    fn ui_direct_probe_accepts_supported_vfr_and_display_geometry() {
        let vfr = probe_source_direct(&fixture("m35-vfr-offset.mp4")).unwrap();
        assert!(vfr.variable_frame_rate);
        assert_eq!(vfr.video_start_us, 1_250_000);
        let transformed = probe_source_direct(&fixture("m35-geometry-color.mp4")).unwrap();
        assert_eq!(
            (transformed.display_width, transformed.display_height),
            (180, 421)
        );
        assert!(transformed.has_display_matrix);
    }

    #[test]
    fn direct_route_inspects_vfr_and_detects_timeline_or_geometry_corruption() {
        let (source, timeline) = inspect_source(
            &fixture("m35-vfr-offset.mp4"),
            false,
            false,
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(source.frame_count, 36);
        assert!(source.variable_frame_rate);
        assert_eq!(source.video_start_us, 1_250_000);
        assert_eq!(source.audio_start_us, Some(1_228_000));
        assert_eq!(timeline[0], 1_250_000);
        let mut output = source.clone();
        output.video_start_us = 22_000;
        output.audio_start_us = Some(0);
        let mut normalized: Vec<_> = timeline.iter().map(|pts| pts - 1_228_000).collect();
        verify_output_timeline(&source, &timeline, &output, &normalized).unwrap();
        normalized[5] += 2_000;
        assert!(
            verify_output_timeline(&source, &timeline, &output, &normalized)
                .unwrap_err()
                .to_string()
                .contains("frame 5")
        );

        let error = inspect_source(
            &fixture("m35-resolution-change.mp4"),
            false,
            false,
            &CancellationToken::default(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("geometry changed"), "{error}");
    }

    #[test]
    fn gpu_rejects_sample_aspect_and_rotation_but_direct_resolves_display_size() {
        let error = probe_source(&fixture("m35-geometry-color.mp4"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("square-pixel"), "{error}");
        let (source, timeline) = inspect_source(
            &fixture("m35-geometry-color.mp4"),
            false,
            false,
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!((source.width, source.height), (316, 180));
        assert_eq!((source.display_width, source.display_height), (180, 421));
        assert_eq!(source.rotation_degrees, -90);
        assert_eq!(source.sample_aspect_ratio, "4:3");
        assert_eq!(timeline.len(), 24);
    }

    #[test]
    fn rejects_hdr_and_wide_color_before_native_conversion() {
        for strict in [false, true] {
            let error = inspect_source(
                &fixture("m35-hdr-tagged.mp4"),
                strict,
                false,
                &CancellationToken::default(),
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains("unsupported matrix: bt2020nc"), "{error}");
        }
    }

    #[test]
    fn probes_true_10bit_sdr_without_treating_it_as_an_8bit_gpu_source() {
        let input = fixture("m4-10bit-sdr.mp4");
        let source = probe_source(&input).unwrap();
        assert_eq!(source.codec_profile.as_deref(), Some("Main 10"));
        assert_eq!(source.pixel_format, "yuv420p10le");
        assert_eq!(source.frame_count, 24);
        let error = inspect_source(&input, true, false, &CancellationToken::default())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("unsupported pixel format: yuv420p10le"),
            "{error}"
        );
    }

    #[test]
    #[ignore = "requires FFmpeg, FFprobe and a native wgpu adapter"]
    fn injected_device_loss_cleans_job_and_allows_fresh_gpu_job() {
        let directory = std::env::temp_dir().join(format!(
            "media-native-device-loss-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let failed_output = directory.join("lost.mp4");
        let source = fixture("m2-h264-aac.mp4");
        let failed = convert_with_control_inner(
            &source,
            &failed_output,
            ResizeSpec::Percent(50),
            OutputProfileId::Mp4H264Aac,
            ProcessingRoute::SharedWgpu,
            NativeRunOptions {
                adapter_key: None,
                inject_device_loss_after_frames: Some(1),
                ..Default::default()
            },
            &CancellationToken::default(),
        )
        .unwrap_err()
        .to_string();
        assert!(failed.contains("GPU device lost (Destroyed)"), "{failed}");
        assert!(!failed_output.exists());
        assert!(!partial_path(&failed_output).unwrap().exists());

        let retry_output = directory.join("retry.mp4");
        let retry = convert_with_control(
            &source,
            &retry_output,
            ResizeSpec::Percent(50),
            OutputProfileId::Mp4H264Aac,
            ProcessingRoute::SharedWgpu,
            None,
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(retry.frames_processed, 60);
        assert_eq!(probe_source(&retry_output).unwrap().frame_count, 60);
    }
}
