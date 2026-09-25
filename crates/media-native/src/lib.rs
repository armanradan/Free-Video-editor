#![forbid(unsafe_code)]

use media_core::{OutputProfileId, ResizeSpec, Rotation, Size};
use media_gpu::ResizePipeline;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub type NativeResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

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
    child: Child,
    reaped: bool,
}

impl ChildGuard {
    fn spawn(command: &mut Command) -> NativeResult<Self> {
        Ok(Self {
            child: command.spawn()?,
            reaped: false,
        })
    }

    fn wait_cancellable(
        &mut self,
        cancel: &CancellationToken,
    ) -> NativeResult<std::process::ExitStatus> {
        loop {
            cancel.check()?;
            if let Some(status) = self.child.try_wait()? {
                self.reaped = true;
                return Ok(status);
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessingRoute {
    DirectFfmpeg,
    SharedWgpu,
}

#[derive(Debug, Deserialize)]
struct Probe {
    streams: Vec<ProbeStream>,
}

#[derive(Debug, Deserialize)]
struct ProbeStream {
    codec_type: String,
    codec_name: String,
    width: Option<u32>,
    height: Option<u32>,
    sample_aspect_ratio: Option<String>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    time_base: Option<String>,
    start_time: Option<String>,
    nb_frames: Option<String>,
    pix_fmt: Option<String>,
    color_transfer: Option<String>,
    side_data_list: Option<Vec<SideData>>,
}

#[derive(Debug, Deserialize)]
struct SideData {
    rotation: Option<i32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceInfo {
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate_num: u32,
    pub frame_rate_den: u32,
    pub frame_count: u64,
    pub audio_codec: Option<String>,
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
    let _ = Size::new(width, height)?;
    if video
        .sample_aspect_ratio
        .as_deref()
        .is_some_and(|v| v != "1:1")
    {
        return Err("native M4 harness currently requires square-pixel input".into());
    }
    if video
        .side_data_list
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|v| v.rotation.unwrap_or(0) != 0)
    {
        return Err("native M4 harness does not yet implement source rotation".into());
    }
    if video
        .color_transfer
        .as_deref()
        .is_some_and(|v| matches!(v, "smpte2084" | "arib-std-b67"))
    {
        return Err("native M4 harness does not yet implement HDR transfer".into());
    }
    if video
        .pix_fmt
        .as_deref()
        .is_some_and(|v| v.contains("10") || v.contains("12"))
    {
        return Err("native M4 harness currently accepts 8-bit video only".into());
    }
    if video
        .start_time
        .as_deref()
        .is_some_and(|v| v.parse::<f64>().is_ok_and(|t| t.abs() > 0.000_001))
    {
        return Err("native M4 harness currently requires zero-origin video".into());
    }
    let rate = video
        .avg_frame_rate
        .as_deref()
        .ok_or("ffprobe omitted frame rate")?;
    if video.r_frame_rate.as_deref() != Some(rate) {
        return Err(
            "native M4 harness currently requires matching nominal and average frame rates".into(),
        );
    }
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
    validate_cfr_timestamps(
        path,
        video
            .time_base
            .as_deref()
            .ok_or("ffprobe omitted video time base")?,
        num,
        den,
        frame_count,
    )?;
    Ok(SourceInfo {
        codec: video.codec_name.clone(),
        width,
        height,
        frame_rate_num: num,
        frame_rate_den: den,
        frame_count,
        audio_codec: audio_tracks.first().map(|v| v.codec_name.clone()),
    })
}

fn parse_rational(value: &str) -> NativeResult<(i128, i128)> {
    let (num, den) = value.split_once('/').ok_or("invalid rational value")?;
    let (num, den) = (num.parse::<i128>()?, den.parse::<i128>()?);
    if num <= 0 || den <= 0 {
        return Err("nonpositive rational value".into());
    }
    Ok((num, den))
}

fn validate_cfr_timestamps(
    path: &Path,
    time_base: &str,
    rate_num: u32,
    rate_den: u32,
    frame_count: u64,
) -> NativeResult<()> {
    let (time_num, time_den) = parse_rational(time_base)?;
    let denominator = i128::from(rate_num)
        .checked_mul(time_num)
        .ok_or("frame-time denominator overflow")?;
    let numerator = i128::from(rate_den)
        .checked_mul(time_den)
        .ok_or("frame-time numerator overflow")?;
    let mut child = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "frame=best_effort_timestamp",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .spawn()?;
    let mut count = 0_u64;
    let result = (|| -> NativeResult<()> {
        let stdout = child.stdout.take().ok_or("ffprobe stdout unavailable")?;
        for line in BufReader::new(stdout).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let pts = line
                .split(',')
                .next()
                .ok_or("missing frame timestamp")?
                .parse::<i128>()?;
            let expected = i128::from(count)
                .checked_mul(numerator)
                .ok_or("frame-time calculation overflow")?;
            let expected = (expected + denominator / 2) / denominator;
            if (pts - expected).abs() > 1 {
                return Err(format!("video is not zero-origin CFR at frame {count}: PTS={pts}, expected {expected}±1 tick").into());
            }
            count += 1;
            if count > frame_count {
                return Err("ffprobe decoded more frames than reported".into());
            }
        }
        if count != frame_count {
            return Err(format!("ffprobe decoded {count} frames, expected {frame_count}").into());
        }
        Ok(())
    })();
    if result.is_err() {
        terminate_child(&mut child);
        return result;
    }
    if !child.wait()?.success() {
        return Err("ffprobe frame timeline inspection failed".into());
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
    fs::rename(&temporary, path)?;
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

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
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
    cancel.check()?;
    let source = probe_source(input)?;
    cancel.check()?;
    if profile != OutputProfileId::Mp4H264Aac {
        return Err(format!(
            "native M4 harness does not yet implement profile {}",
            profile.as_str()
        )
        .into());
    }
    let output_size = resize.output_size(Size::new(source.width, source.height)?)?;
    let mut partial = create_partial(output)?;
    let started = Instant::now();
    let result = match route {
        ProcessingRoute::DirectFfmpeg => {
            convert_direct(input, &partial.path, output_size, &source, cancel)
                .map(|()| (None, 0, 0, None))
        }
        ProcessingRoute::SharedWgpu => convert_gpu(
            input,
            &partial.path,
            output_size,
            &source,
            adapter_key,
            cancel,
        ),
    };
    let (adapter, uploaded, downloaded, fallback) = result?;
    let conversion_elapsed = started.elapsed().as_millis();
    let verification_started = Instant::now();
    let verified = probe_source(&partial.path)?;
    cancel.check()?;
    if verified.width != output_size.width
        || verified.height != output_size.height
        || verified.frame_count != source.frame_count
        || verified.audio_codec.is_some() != source.audio_codec.is_some()
        || verified.codec != "h264"
        || verified
            .audio_codec
            .as_deref()
            .is_some_and(|codec| codec != "aac")
    {
        return Err(
            "native output stream metadata does not match expected size/frame/audio counts".into(),
        );
    }
    let size = finish_output(&mut partial, output)?;
    Ok(ConversionReport {
        route: match route {
            ProcessingRoute::DirectFfmpeg => "direct-ffmpeg",
            ProcessingRoute::SharedWgpu => "shared-wgpu",
        }
        .into(),
        profile: profile.as_str().into(),
        input: source.clone(),
        output_width: output_size.width,
        output_height: output_size.height,
        frames_processed: source.frame_count,
        output_bytes: size,
        elapsed_ms: conversion_elapsed,
        verification_ms: verification_started.elapsed().as_millis(),
        adapter,
        explicit_cpu_to_gpu_bytes: uploaded,
        explicit_gpu_to_cpu_bytes: downloaded,
        adapter_fallback: fallback,
    })
}

fn convert_direct(
    input: &Path,
    partial: &Path,
    output: Size,
    source: &SourceInfo,
    cancel: &CancellationToken,
) -> NativeResult<()> {
    cancel.check()?;
    let mut cmd = ffmpeg_base();
    cmd.arg("-i").arg(input).args(["-map", "0:v:0"]);
    if source.audio_codec.is_some() {
        cmd.args(["-map", "0:a:0"]);
    }
    cmd.args([
        "-vf",
        &format!(
            "scale={}:{}:flags=bilinear,setsar=1",
            output.width, output.height
        ),
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-crf",
        "20",
        "-pix_fmt",
        "yuv420p",
    ]);
    if source.audio_codec.is_some() {
        cmd.args(["-c:a", "aac", "-b:a", "192k", "-ar", "48000"]);
    }
    cmd.args(["-movflags", "+faststart", "-f", "mp4"])
        .arg(partial);
    let mut child = ChildGuard::spawn(&mut cmd)?;
    let status = child.wait_cancellable(cancel)?;
    if !status.success() {
        return Err(format!("direct FFmpeg exited with {status}").into());
    }
    Ok(())
}

fn convert_gpu(
    input: &Path,
    partial: &Path,
    output: Size,
    source: &SourceInfo,
    adapter_key: Option<&str>,
    cancel: &CancellationToken,
) -> NativeResult<(Option<AdapterDescriptor>, u64, u64, Option<String>)> {
    cancel.check()?;
    let mut gpu = GpuProcessor::new(Size::new(source.width, source.height)?, output, adapter_key)?;
    cancel.check()?;
    let mut decoder = ffmpeg_base();
    decoder
        .arg("-i")
        .arg(input)
        .args([
            "-map", "0:v:0", "-f", "rawvideo", "-pix_fmt", "rgba", "-vsync", "0", "pipe:1",
        ])
        .stdout(Stdio::piped());
    let mut decoder = ChildGuard::spawn(&mut decoder)?;
    let mut encoder = ffmpeg_base();
    encoder.args([
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-video_size",
        &format!("{}x{}", output.width, output.height),
        "-framerate",
        &format!("{}/{}", source.frame_rate_num, source.frame_rate_den),
        "-i",
        "pipe:0",
    ]);
    if source.audio_codec.is_some() {
        encoder.arg("-i").arg(input);
    }
    encoder.args(["-map", "0:v:0"]);
    if source.audio_codec.is_some() {
        encoder.args(["-map", "1:a:0"]);
    }
    encoder.args([
        "-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p",
    ]);
    if source.audio_codec.is_some() {
        encoder.args(["-c:a", "aac", "-b:a", "192k", "-ar", "48000"]);
    }
    encoder
        .args(["-movflags", "+faststart", "-f", "mp4"])
        .arg(partial)
        .stdin(Stdio::piped());
    let mut encoder = ChildGuard::spawn(&mut encoder)?;
    let frame_len = usize::try_from(u64::from(source.width) * u64::from(source.height) * 4)?;
    let mut frame = vec![0_u8; frame_len];
    let result = (|| -> NativeResult<u64> {
        let mut reader = decoder
            .child
            .stdout
            .take()
            .ok_or("decoder stdout was unavailable")?;
        let mut writer = encoder
            .child
            .stdin
            .take()
            .ok_or("encoder stdin was unavailable")?;
        let mut count = 0_u64;
        loop {
            cancel.check()?;
            let mut filled = 0;
            while filled < frame.len() {
                cancel.check()?;
                let read = reader.read(&mut frame[filled..])?;
                if read == 0 {
                    break;
                }
                filled += read;
            }
            if filled == 0 {
                break;
            }
            if filled != frame.len() {
                return Err("decoder ended with a partial RGBA frame".into());
            }
            gpu.process(&frame, &mut writer, cancel)?;
            count += 1;
            if count > source.frame_count {
                return Err("decoder produced more frames than the probed source".into());
            }
        }
        drop(writer);
        if count != source.frame_count {
            return Err(format!(
                "decoder produced {count} frames, expected {}",
                source.frame_count
            )
            .into());
        }
        Ok(count)
    })();
    result?;
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
        })
    }

    fn process(
        &mut self,
        input: &[u8],
        output: &mut impl Write,
        cancel: &CancellationToken,
    ) -> NativeResult<()> {
        cancel.check()?;
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
            match receiver.try_recv() {
                Ok(result) => {
                    result?;
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
                Err(error) => return Err(error.into()),
            }
        }
        cancel.check()?;
        {
            let mapped = slice.get_mapped_range()?;
            let row_bytes = usize::try_from(self.output_size.width * 4)?;
            for row in mapped
                .chunks_exact(self.padded_bytes_per_row as usize)
                .take(self.output_size.height as usize)
            {
                output.write_all(&row[..row_bytes])?;
            }
        }
        self.readback.unmap();
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

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
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
    fn rejects_real_vfr_and_nonzero_origin() {
        let error = probe_source(&fixture("m35-vfr-offset.mp4"))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("zero-origin") || error.contains("frame rates") || error.contains("CFR"),
            "{error}"
        );
    }

    #[test]
    fn rejects_unimplemented_sample_aspect_and_rotation() {
        let error = probe_source(&fixture("m35-geometry-color.mp4"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("square-pixel"), "{error}");
    }
}
