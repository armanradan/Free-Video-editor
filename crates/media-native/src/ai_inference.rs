//! Concrete native ONNX/RTX dispatcher with shared bounded child ownership.
//! Helpers own their device/session; no foreign GPU handle crosses a wgpu ABI.
use crate::{CancellationToken, NativeResult, ai_assets::ModelCache, ai_runtime::RuntimePaths};
use media_core::{
    Size,
    ai_upscale::{AiModel, AiScale, AiSettings, pilot_tiles},
};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

const REQUEST: u32 = 0x41495131;
const RESPONSE: u32 = 0x41494f31;

struct StartingChild(Option<Child>);
impl Drop for StartingChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnnxHelperConfig {
    pub executable: PathBuf,
    pub runtime_directory: PathBuf,
    pub model_cache: PathBuf,
    pub dxgi_index: u32,
    pub exact_adapter_name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HelperIdentity {
    pub protocol: u32,
    pub pid: u32,
    pub runtime: String,
    pub model_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk_configuration: Option<String>,
    pub dxgi_index: u32,
    pub adapter_name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub luid_high: u32,
    pub luid_low: u32,
    pub cpu_fallback: bool,
    pub tile_interior: u32,
    pub halo: u32,
}

pub struct AiFrame {
    pub rgba: Vec<u8>,
    pub size: Size,
    pub device_generation: u64,
    pub host_to_gpu_bytes: u64,
    pub gpu_to_host_bytes: u64,
    pub sampled_process_local_peak: u64,
}

pub enum NativeAiProcessor {
    Off,
    Onnx(Box<NativeHelper>),
    Rtx(Box<NativeHelper>),
}
impl NativeAiProcessor {
    pub fn new(
        settings: AiSettings,
        config: Option<&OnnxHelperConfig>,
        source: Size,
        generation: u64,
        cancel: &CancellationToken,
    ) -> NativeResult<Self> {
        match settings {
            AiSettings::Off=>Ok(Self::Off),
            AiSettings::OnnxRuntime {scale,..}=>{
                pilot_tiles(source,128)?;
                Ok(Self::Onnx(Box::new(NativeHelper::start(config.ok_or("Explicit ONNX helper/runtime/model configuration is missing")?,
                    source,generation,scale,cancel)?)))
            }
            AiSettings::Burn {..}=>Err("Native Burn is unavailable: the assessed FP32 copy kernel failed its IR compilation gate".into()),
            AiSettings::RtxVideo {..}=>Err("RTX requires explicit helper configuration; ordinary preview and release integration remain pending".into()),
        }
    }
    pub fn new_rtx(
        settings: AiSettings,
        config: &super::rtx_inference::RtxHelperConfig,
        source: Size,
        generation: u64,
        cancel: &CancellationToken,
    ) -> NativeResult<Self> {
        let AiSettings::RtxVideo { scale, .. } = settings else {
            return Err("RTX helper requires explicit RTX settings".into());
        };
        Ok(Self::Rtx(Box::new(super::rtx_inference::start(
            config, source, generation, scale, cancel,
        )?)))
    }
    pub fn identity(&self) -> Option<&HelperIdentity> {
        match self {
            Self::Off => None,
            Self::Onnx(helper) | Self::Rtx(helper) => Some(&helper.identity),
        }
    }
    pub fn selected_scale(&self) -> Option<AiScale> {
        match self {
            Self::Off => None,
            Self::Onnx(helper) | Self::Rtx(helper) => Some(helper.scale),
        }
    }
    /// Portable model returns internal x4; RTX returns the selected native scale.
    pub fn process_frame(
        &mut self,
        rgba: Vec<u8>,
        generation: u64,
        cancel: &CancellationToken,
    ) -> NativeResult<AiFrame> {
        match self {
            Self::Off => Err("AI Off has no inference frame path".into()),
            Self::Onnx(helper) | Self::Rtx(helper) => helper.process(rgba, generation, cancel),
        }
    }
    pub fn close(&mut self) -> NativeResult<()> {
        match self {
            Self::Off => Ok(()),
            Self::Onnx(helper) | Self::Rtx(helper) => helper.close(),
        }
    }

    /// The RTX helper's own DXGI samples, available after successful close.
    /// This is neither whole-app GPU usage nor a strict transient peak bound.
    pub(crate) fn closed_sampled_local_peak(&self) -> Option<u64> {
        let Self::Rtx(helper) = self else {
            return None;
        };
        if !helper.retired {
            return None;
        }
        let log = helper.stderr.lock().unwrap_or_else(|p| p.into_inner());
        String::from_utf8_lossy(&log)
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .rfind(|item| item["stage"] == "closed")
            .and_then(|item| item["sampled_local_peak"].as_u64())
    }

    #[cfg(test)]
    pub(crate) fn crash_owned_child_for_test(&mut self) -> NativeResult<()> {
        match self {
            Self::Onnx(helper) | Self::Rtx(helper) => {
                helper
                    .child
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .as_mut()
                    .ok_or("No owned helper to crash")?
                    .kill()?;
                Ok(())
            }
            Self::Off => Err("No AI helper while Off".into()),
        }
    }
}

pub(super) enum Request {
    Frame(Vec<u8>, Size),
    Stop,
}
pub(super) enum Reply {
    Ready(HelperIdentity),
    Frame(AiFrame),
    Stopped,
}

pub struct NativeHelper {
    pub(super) identity: HelperIdentity,
    scale: AiScale,
    source: Size,
    generation: u64,
    child: Arc<Mutex<Option<Child>>>,
    tx: Option<mpsc::SyncSender<Request>>,
    rx: mpsc::Receiver<NativeResult<Reply>>,
    worker: Option<thread::JoinHandle<()>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    stderr_worker: Option<thread::JoinHandle<()>>,
    retired: bool,
    label: &'static str,
    pub(super) frame_timeout: Duration,
}

fn kill_reap(child: &Arc<Mutex<Option<Child>>>) {
    if let Some(mut process) = child.lock().unwrap_or_else(|p| p.into_inner()).take() {
        let _ = process.kill();
        let _ = process.wait();
    }
}
fn read_u32(reader: &mut impl Read) -> NativeResult<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}
fn read_u64(reader: &mut impl Read) -> NativeResult<u64> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}
fn header(writer: &mut impl Write, op: u32, size: Size, length: u32) -> NativeResult<()> {
    for value in [REQUEST, 2, op, size.width, size.height, length] {
        writer.write_all(&value.to_le_bytes())?;
    }
    Ok(())
}

impl NativeHelper {
    fn start(
        config: &OnnxHelperConfig,
        source: Size,
        generation: u64,
        scale: AiScale,
        cancel: &CancellationToken,
    ) -> NativeResult<Self> {
        cancel.check()?;
        if config.dxgi_index >= 32 || config.exact_adapter_name.is_empty() {
            return Err("Explicit DXGI ordinal/name is invalid".into());
        }
        let report = RuntimePaths {
            onnx_directory: Some(config.runtime_directory.clone()),
            rtx_sdk: None,
        }
        .inspect();
        if report
            .onnx
            .iter()
            .any(|c| c.status != "present-unvalidated")
        {
            return Err(format!(
                "ONNX runtime discovery rejected: {}",
                serde_json::to_string(&report)?
            )
            .into());
        }
        let cache = ModelCache::new(&config.model_cache);
        let _verified = cache.read_onnx(cancel)?;
        let mut command = Command::new(&config.executable);
        command
            .arg(&config.runtime_directory)
            .arg(cache.model_path())
            .arg(config.dxgi_index.to_string())
            .arg(&config.exact_adapter_name)
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let owner = Self::launch(command, source, generation, scale, false, cancel)?;
        let identity = &owner.identity;
        if identity.protocol != 2
            || owner
                .child
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
                .is_none_or(|child| child.id() != identity.pid)
            || identity.runtime != "1.23.0"
            || identity.model_sha256 != AiModel::RealEsrGeneralX4v3StrongFp32V1.onnx_sha256()
            || identity.dxgi_index != config.dxgi_index
            || identity.adapter_name != config.exact_adapter_name
            || identity.cpu_fallback
            || identity.tile_interior != 128
            || identity.halo != 34
        {
            return Err("Helper did not retain exact runtime/model/device/credit identity".into());
        }
        Ok(owner)
    }

    pub(super) fn launch(
        mut command: Command,
        source: Size,
        generation: u64,
        scale: AiScale,
        rtx: bool,
        cancel: &CancellationToken,
    ) -> NativeResult<Self> {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut starting = StartingChild(Some(command.spawn().map_err(|error| {
            format!(
                "Cannot start the configured native AI helper; verify its executable path: {error}"
            )
        })?));
        let process = starting.0.as_mut().ok_or("Helper child missing")?;
        let mut input = process.stdin.take().ok_or("Helper stdin was not created")?;
        let output = process
            .stdout
            .take()
            .ok_or("Helper stdout was not created")?;
        let mut errors = process
            .stderr
            .take()
            .ok_or("Helper stderr was not created")?;
        let child = Arc::new(Mutex::new(starting.0.take()));
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&stderr);
        let stderr_worker = thread::spawn(move || {
            let mut block = [0u8; 4096];
            while let Ok(count) = errors.read(&mut block) {
                if count == 0 {
                    break;
                }
                let mut bytes = captured.lock().unwrap_or_else(|p| p.into_inner());
                let retain = count.min(65536usize.saturating_sub(bytes.len()));
                bytes.extend_from_slice(&block[..retain]);
            }
        });
        let (tx, requests) = mpsc::sync_channel(0);
        let (replies, rx) = mpsc::sync_channel(1);
        #[cfg(test)]
        let test_fault = super::rtx_inference::current_test_fault();
        let worker = thread::spawn(move || {
            #[cfg(test)]
            let _test_scope = test_fault.map(super::rtx_inference::test_fault_scope);
            let mut output = BufReader::new(output);
            let outcome = (|| -> NativeResult<()> {
                if rtx {
                    return super::rtx_inference::run_protocol(
                        &mut input,
                        &mut output,
                        requests,
                        &replies,
                        source,
                        generation,
                        scale,
                    );
                }
                let mut line = Vec::new();
                output.by_ref().take(8193).read_until(b'\n', &mut line)?;
                if line.len() > 8192 || !line.ends_with(b"\n") {
                    return Err("Invalid/oversized helper handshake".into());
                }
                let identity = serde_json::from_slice(&line)?;
                replies
                    .send(Ok(Reply::Ready(identity)))
                    .map_err(|_| "Helper owner disappeared")?;
                while let Ok(request) = requests.recv() {
                    match request {
                        Request::Stop => {
                            header(&mut input, 2, Size::new_unchecked(0, 0), 0)?;
                            input.flush()?;
                            break;
                        }
                        Request::Frame(bytes, size) => {
                            // Shared core owns every halo/interior coordinate.
                            // This explicit native CPU-staged route packs/assembles
                            // those tiles; the helper only owns tensor inference.
                            let mut rgba = vec![
                                0u8;
                                (u64::from(size.width) * u64::from(size.height) * 16 * 4)
                                    as usize
                            ];
                            let (mut up, mut down, mut peak) = (0u64, 0u64, 0u64);
                            for tile in pilot_tiles(size, 128)? {
                                let (w, h) = (tile.x1 - tile.x0, tile.y1 - tile.y0);
                                let mut packed = Vec::with_capacity((w * h * 3 * 4) as usize);
                                for channel in 0..3 {
                                    for y in tile.y0..tile.y1 {
                                        for x in tile.x0..tile.x1 {
                                            let value = f32::from(
                                                bytes
                                                    [((y * size.width + x) * 4 + channel) as usize],
                                            ) / 255.;
                                            packed.extend_from_slice(&value.to_le_bytes());
                                        }
                                    }
                                }
                                header(
                                    &mut input,
                                    1,
                                    Size::new(w, h)?,
                                    u32::try_from(packed.len())?,
                                )?;
                                input.write_all(&packed)?;
                                input.flush()?;
                                if read_u32(&mut output)? != RESPONSE
                                    || read_u32(&mut output)? != 2
                                    || read_u32(&mut output)? != 0
                                {
                                    return Err("Invalid helper response identity/status".into());
                                }
                                let length = read_u32(&mut output)? as usize;
                                if length != packed.len() * 16 {
                                    return Err("Wrong bounded FP32 tile output byte size".into());
                                }
                                let tile_up = read_u64(&mut output)?;
                                let tile_down = read_u64(&mut output)?;
                                peak = peak.max(read_u64(&mut output)?);
                                if tile_up != packed.len() as u64 || tile_down != length as u64 {
                                    return Err("Helper transfer payload accounting disagrees with the tile plan".into());
                                }
                                up += tile_up;
                                down += tile_down;
                                let mut values = vec![0u8; length];
                                output.read_exact(&mut values)?;
                                let plane = (w * h * 16) as usize;
                                for y in 0..(tile.bottom - tile.top) * 4 {
                                    for x in 0..(tile.right - tile.left) * 4 {
                                        let src = (((tile.top - tile.y0) * 4 + y) * (w * 4)
                                            + (tile.left - tile.x0) * 4
                                            + x)
                                            as usize;
                                        let dst = (((tile.top * 4 + y) * (size.width * 4)
                                            + tile.left * 4
                                            + x)
                                            * 4)
                                            as usize;
                                        for channel in 0..3 {
                                            let offset = (channel * plane + src) * 4;
                                            let value = f32::from_le_bytes(
                                                values[offset..offset + 4].try_into()?,
                                            );
                                            if !value.is_finite() {
                                                return Err("Nonfinite FP32 helper output".into());
                                            }
                                            rgba[dst + channel] =
                                                (value.clamp(0., 1.) * 255.).round() as u8;
                                        }
                                        rgba[dst + 3] = 255;
                                    }
                                }
                            }
                            replies
                                .send(Ok(Reply::Frame(AiFrame {
                                    rgba,
                                    size: Size::new(size.width * 4, size.height * 4)?,
                                    device_generation: generation,
                                    host_to_gpu_bytes: up,
                                    gpu_to_host_bytes: down,
                                    sampled_process_local_peak: peak,
                                })))
                                .map_err(|_| "Helper owner disappeared")?;
                        }
                    }
                }
                Ok(())
            })();
            match outcome {
                Ok(()) => {
                    let _ = replies.try_send(Ok(Reply::Stopped));
                }
                Err(error) => {
                    let _ = replies.try_send(Err(error));
                }
            }
        });
        let mut owner = Self {
            identity: HelperIdentity {
                protocol: 0,
                pid: 0,
                runtime: String::new(),
                model_sha256: String::new(),
                sdk_configuration: None,
                dxgi_index: 0,
                adapter_name: String::new(),
                vendor_id: 0,
                device_id: 0,
                luid_high: 0,
                luid_low: 0,
                cpu_fallback: true,
                tile_interior: 0,
                halo: 0,
            },
            scale,
            source,
            generation,
            child,
            tx: Some(tx),
            rx,
            worker: Some(worker),
            stderr,
            stderr_worker: Some(stderr_worker),
            retired: false,
            label: if rtx { "RTX" } else { "ONNX" },
            frame_timeout: Duration::from_secs(60),
        };
        let ready = owner.receive(cancel, Duration::from_secs(60))?;
        let Reply::Ready(identity) = ready else {
            return Err("Helper skipped its startup handshake".into());
        };
        if owner
            .child
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .is_none_or(|child| child.id() != identity.pid)
        {
            return Err("Helper startup PID did not match its owned child".into());
        }
        owner.identity = identity;
        Ok(owner)
    }

    fn receive(&mut self, cancel: &CancellationToken, timeout: Duration) -> NativeResult<Reply> {
        let deadline = Instant::now() + timeout;
        loop {
            #[cfg(test)]
            if self.label == "RTX" {
                super::rtx_inference::test_cancel_at_barrier(cancel, &self.stderr);
            }
            if let Err(error) = cancel.check() {
                self.retire();
                return Err(error);
            }
            if Instant::now() >= deadline {
                self.retire();
                return Err(format!(
                    "{} helper timed out; explicit fresh helper required",
                    self.label
                )
                .into());
            }
            match self.rx.recv_timeout(Duration::from_millis(20)) {
                Ok(Ok(reply)) => {
                    if let Err(error) = cancel.check() {
                        self.retire();
                        return Err(error);
                    }
                    return Ok(reply);
                }
                Ok(Err(error)) => {
                    self.retire();
                    self.join();
                    let log = self.stderr.lock().unwrap_or_else(|p| p.into_inner());
                    return Err(format!(
                        "{} helper failed: {error}; {}",
                        self.label,
                        String::from_utf8_lossy(&log)
                    )
                    .into());
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.retire();
                    return Err(format!(
                        "{} helper disconnected; explicit fresh helper required",
                        self.label
                    )
                    .into());
                }
            }
        }
    }

    fn process(
        &mut self,
        rgba: Vec<u8>,
        generation: u64,
        cancel: &CancellationToken,
    ) -> NativeResult<AiFrame> {
        cancel.check()?;
        if self.retired || generation != self.generation {
            return Err(format!(
                "{} helper generation is retired/mismatched; explicit fresh helper required",
                self.label
            )
            .into());
        }
        if rgba.len() as u64 != u64::from(self.source.width) * u64::from(self.source.height) * 4 {
            return Err("Wrong normalized 8-bit RGBA frame length".into());
        }
        if rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 255) {
            return Err("Native AI requires opaque 8-bit SDR RGBA input".into());
        }
        self.tx
            .as_ref()
            .ok_or("Native AI helper closed")?
            .send(Request::Frame(rgba, self.source))
            .map_err(|_| "Native AI helper command channel closed")?;
        match self.receive(cancel, self.frame_timeout)? {
            Reply::Frame(frame) => Ok(frame),
            Reply::Ready(_) | Reply::Stopped => {
                self.retire();
                Err("Unexpected helper handshake or shutdown during frame processing".into())
            }
        }
    }

    fn retire(&mut self) {
        self.retired = true;
        kill_reap(&self.child);
        self.tx.take();
    }
    fn join(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if let Some(worker) = self.stderr_worker.take() {
            let _ = worker.join();
        }
        #[cfg(test)]
        if self.label == "RTX" {
            super::rtx_inference::test_retirement(
                &self.stderr,
                self.identity.pid,
                self.child
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .is_none(),
            );
        }
    }
    fn close(&mut self) -> NativeResult<()> {
        let was_retired = self.retired;
        let outcome = (|| -> NativeResult<()> {
            if !self.retired {
                if let Some(tx) = self.tx.take() {
                    let _ = tx.send(Request::Stop);
                }
                let deadline = Instant::now() + Duration::from_secs(2);
                loop {
                    let status = self
                        .child
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .as_mut()
                        .map(|c| c.try_wait())
                        .transpose()?;
                    if let Some(status) = status.flatten() {
                        if !status.success() {
                            return Err(
                                format!("{} helper exited with {status}", self.label).into()
                            );
                        }
                        break;
                    }
                    if Instant::now() >= deadline {
                        return Err(format!(
                            "{} helper shutdown timed out; owned child will be killed/reaped",
                            self.label
                        )
                        .into());
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
            Ok(())
        })();
        self.retire();
        self.join();
        outcome?;
        if !was_retired {
            match self.rx.try_recv() {
                Ok(Ok(Reply::Stopped)) => {}
                Ok(Err(error)) => return Err(error),
                _ => return Err("Helper exited without successful protocol shutdown".into()),
            }
        }
        Ok(())
    }
}
impl Drop for NativeHelper {
    fn drop(&mut self) {
        self.retire();
        self.join();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn actual_config() -> OnnxHelperConfig {
        OnnxHelperConfig {
            executable: std::env::var_os("DIAXUS_AI_HELPER")
                .expect("explicit test helper path")
                .into(),
            runtime_directory: std::env::var_os("DIAXUS_ORT_DIRECTORY")
                .expect("explicit test runtime directory")
                .into(),
            model_cache: std::env::var_os("DIAXUS_AI_MODEL_CACHE")
                .expect("explicit test model cache")
                .into(),
            dxgi_index: 1,
            exact_adapter_name: "NVIDIA GeForce RTX 3050 Laptop GPU".into(),
        }
    }

    #[test]
    #[ignore = "requires explicitly configured actual DirectML helper/runtime/model and NVIDIA DXGI ordinal 1"]
    fn actual_helper_reuses_frames_matches_reference_and_retires_cancelled_child() {
        let config = actual_config();
        let cancel = CancellationToken::default();
        let settings = AiSettings::OnnxRuntime {
            model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
            scale: AiScale::Four,
        };
        let mut processor = NativeAiProcessor::new(
            settings,
            Some(&config),
            Size::new(3, 5).unwrap(),
            7,
            &cancel,
        )
        .unwrap();
        let mut pixels = Vec::new();
        for y in 0..5 {
            for x in 0..3 {
                pixels.extend_from_slice(&[
                    (x * 17 + y * 13) as u8,
                    0,
                    (x * y * 7 + 31) as u8,
                    255,
                ]);
            }
        }
        assert!(processor.process_frame(pixels.clone(), 6, &cancel).is_err());
        let first = processor.process_frame(pixels.clone(), 7, &cancel).unwrap();
        let second = processor.process_frame(pixels, 7, &cancel).unwrap();
        assert_eq!(first.rgba, second.rgba);
        assert_eq!(
            (first.host_to_gpu_bytes, first.gpu_to_host_bytes),
            (180, 2880)
        );
        let bytes = include_bytes!("../../../fixtures/ai-upscale/3x5-fp32.npy");
        let offset = 10 + usize::from(u16::from_le_bytes(bytes[8..10].try_into().unwrap()));
        let reference = bytes[offset..]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| f32::from_le_bytes(*v))
            .collect::<Vec<_>>();
        for index in 0..240 {
            for channel in 0..3 {
                let expected =
                    (reference[channel * 240 + index].clamp(0., 1.) * 255.).round() as u8;
                assert!(first.rgba[index * 4 + channel].abs_diff(expected) <= 1);
            }
            assert_eq!(first.rgba[index * 4 + 3], 255);
        }
        println!(
            "native AI identity: {}",
            serde_json::to_string(processor.identity().unwrap()).unwrap()
        );
        processor.close().unwrap();
        processor.close().unwrap();
        let mut active = NativeAiProcessor::new(
            settings,
            Some(&config),
            Size::new(577, 579).unwrap(),
            8,
            &cancel,
        )
        .unwrap();
        let token = CancellationToken::default();
        let stop = token.clone();
        let trigger = thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            stop.cancel();
        });
        let started = Instant::now();
        let result = active.process_frame(vec![255; 577 * 579 * 4], 8, &token);
        trigger.join().unwrap();
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(
            active
                .process_frame(vec![255; 577 * 579 * 4], 8, &cancel)
                .is_err()
        );
        active.close().unwrap();
        let mut fresh = NativeAiProcessor::new(
            settings,
            Some(&config),
            Size::new(3, 5).unwrap(),
            9,
            &cancel,
        )
        .unwrap();
        fresh
            .process_frame(vec![255; 3 * 5 * 4], 9, &cancel)
            .unwrap();
        fresh.close().unwrap();
        let mut crashed = NativeAiProcessor::new(
            settings,
            Some(&config),
            Size::new(3, 5).unwrap(),
            10,
            &cancel,
        )
        .unwrap();
        let NativeAiProcessor::Onnx(helper) = &mut crashed else {
            unreachable!()
        };
        helper
            .child
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .kill()
            .unwrap();
        assert!(
            crashed
                .process_frame(vec![255; 3 * 5 * 4], 10, &cancel)
                .is_err()
        );
        crashed.close().unwrap();
        println!(
            "native AI actual helper: repeated-frame/pixel/transfer/generation gates, active-request cancel+reap, retired rejection, fresh-child retry and helper crash cleanup passed"
        );
    }
    #[test]
    fn off_and_failed_candidates_never_spawn_or_read_assets() {
        let absent = OnnxHelperConfig {
            executable: "not-present.exe".into(),
            runtime_directory: "not-present".into(),
            model_cache: "not-present".into(),
            dxgi_index: 1,
            exact_adapter_name: "GPU".into(),
        };
        let size = Size::new(64, 36).unwrap();
        let cancel = CancellationToken::default();
        let mut off =
            NativeAiProcessor::new(AiSettings::Off, Some(&absent), size, 1, &cancel).unwrap();
        assert!(off.identity().is_none());
        off.close().unwrap();
        assert!(
            NativeAiProcessor::new(
                AiSettings::Burn {
                    model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
                    scale: AiScale::Four
                },
                None,
                size,
                1,
                &cancel
            )
            .is_err()
        );
        assert!(
            NativeAiProcessor::new(
                AiSettings::OnnxRuntime {
                    model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
                    scale: AiScale::Four
                },
                Some(&absent),
                Size::new(578, 579).unwrap(),
                1,
                &cancel
            )
            .is_err()
        );
    }
}
