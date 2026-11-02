//! Counted host-staged RTX protocol; no SDK ABI or foreign texture enters Rust.
use crate::{
    CancellationToken, NativeResult,
    ai_inference::{AiFrame, HelperIdentity, NativeHelper, Reply, Request},
};
use media_core::{Size, ai_upscale::AiScale};
use serde::Deserialize;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
};

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct TestFault {
    pub phase: &'static str,
    pub action: &'static str,
    pub cancel: bool,
    pub audit: std::sync::Arc<std::sync::Mutex<serde_json::Value>>,
}
#[cfg(test)]
thread_local! {
// The initializer is already const. Rust1.98 Clippy reports the macro expansion.
#[allow(clippy::missing_const_for_thread_local, reason="literal const initializer is already present")]
static TEST_FAULT: std::cell::RefCell<Option<TestFault>> = const { std::cell::RefCell::new(None) }; }
#[cfg(test)]
pub(crate) struct TestFaultScope(Option<TestFault>);
#[cfg(test)]
impl Drop for TestFaultScope {
    fn drop(&mut self) {
        TEST_FAULT.with(|slot| {
            slot.replace(self.0.take());
        });
    }
}
#[cfg(test)]
pub(crate) fn test_fault_scope(fault: TestFault) -> TestFaultScope {
    TestFaultScope(TEST_FAULT.with(|slot| slot.replace(Some(fault))))
}
#[cfg(test)]
pub(super) fn current_test_fault() -> Option<TestFault> {
    TEST_FAULT.with(|slot| slot.borrow().clone())
}
#[cfg(test)]
pub(super) fn test_cancel_at_barrier(
    cancel: &CancellationToken,
    log: &std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
) {
    TEST_FAULT.with(|slot| {
        if let Some(fault) = slot.borrow().as_ref() {
            let text = log.lock().unwrap_or_else(|p| p.into_inner());
            if String::from_utf8_lossy(&text).contains("\"stage\":\"test-barrier\"") {
                let mut audit = fault.audit.lock().unwrap_or_else(|p| p.into_inner());
                if audit["observed_io_stage"].is_null() {
                    audit["observed_io_stage"] = audit["io_stage"].clone();
                }
                if fault.cancel {
                    cancel.cancel();
                }
            }
        }
    });
}
#[cfg(test)]
pub(super) fn test_retirement(
    log: &std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    pid: u32,
    reaped: bool,
) {
    TEST_FAULT.with(|slot| {
        if let Some(fault) = slot.borrow().as_ref() {
            let mut audit = fault.audit.lock().unwrap_or_else(|p| p.into_inner());
            audit["pid"] = pid.into();
            audit["reaped"] = reaped.into();
            audit["stderr"] =
                String::from_utf8_lossy(&log.lock().unwrap_or_else(|p| p.into_inner()))
                    .into_owned()
                    .into();
        }
    });
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct RtxHelperConfig {
    pub executable: PathBuf,
    pub sdk_root: PathBuf,
    /// Existing parent for a fresh, helper-owned NGX cache directory per run.
    pub cache_parent: PathBuf,
    pub dxgi_index: u32,
    pub exact_adapter_name: String,
    pub expected_luid_high: u32,
    pub expected_luid_low: u32,
}

/// Source-built helper evaluated with SDK1.1 High3. No binary is distributed.
pub const EVALUATED_HELPER_SHA256: &str =
    "e9188624f8a4605f4429f83f8c74463a22db9bfd374a36f590a23f37cf5cac95";

pub const MAX_SOURCE_WIDTH: u32 = 1280;
pub const MAX_SOURCE_HEIGHT: u32 = 720;

fn reserve_cache_path(parent: &Path, tag: &str) -> NativeResult<PathBuf> {
    let parent = parent.canonicalize()?;
    for attempt in 0..32 {
        let reservation = parent.join(format!("rtx-{tag}-{attempt}"));
        match std::fs::create_dir(&reservation) {
            Ok(()) => return Ok(reservation.join("ngx")),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err("Cannot reserve a fresh RTX cache directory; existing caches were preserved".into())
}

/// Conservative evaluated codec envelope, not a universal NVENC minimum.
/// Preview and the software-codec route remain independent of this export gate.
pub fn validate_nvidia_export(source_codec: &str, output: Size) -> NativeResult<()> {
    if !matches!(
        source_codec,
        "h264" | "hevc" | "mpeg1video" | "mpeg2video" | "wmv1" | "wmv2" | "wmv3" | "vc1"
    ) {
        return Err("Evaluated RTX NVIDIA input families are H.264/HEVC, MPEG-1/2 and WMV1/2/3/VC-1. Unsupported input family; no automatic route change.".into());
    }
    if output.width < 192 || output.height < 144 {
        return Err("Evaluated RTX with NVIDIA codecs requires output at least 192x144. Select Shared GPU software codecs for smaller output; no automatic route change.".into());
    }
    Ok(())
}

pub fn validate_source(source: Size) -> NativeResult<()> {
    if source.width == 0
        || source.height == 0
        || source.width > MAX_SOURCE_WIDTH
        || source.height > MAX_SOURCE_HEIGHT
    {
        return Err(
            "RTX helper supports opaque SDR input at most 1280x720; scale/effects admission is checked separately; no resize or backend fallback"
                .into(),
        );
    }
    Ok(())
}

/// Whole-frame RTX staging ledger. Portable model tile geometry/weights are
/// unrelated to this adapter. Includes helper CPU/D3D input, output/readback,
/// shared upload texture and caller-owned downstream/retained frames. Opaque
/// driver/model workspace is an estimate, separately sampled by the helper.
pub fn memory_admission(
    source: Size,
    scale: AiScale,
    downstream: u64,
    retained: u64,
    budget: u64,
) -> NativeResult<u64> {
    validate_source(source)?;
    let pixels = u64::from(source.width) * u64::from(source.height);
    let enlarged = pixels * u64::from(scale.factor()).pow(2);
    let total = [
        pixels * 8,
        enlarged * 16,
        8 << 20,
        128 << 20,
        downstream,
        retained,
    ]
    .into_iter()
    .try_fold(0u64, |sum, n| sum.checked_add(n))
    .ok_or("RTX memory admission overflow")?;
    if total > budget {
        return Err(format!("RTX memory admission requires {total} bytes, budget is {budget}; reduce scale or source geometry").into());
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_inference::NativeAiProcessor;
    use media_core::ai_upscale::{AiSettings, RtxConfiguration};
    use sha2::{Digest, Sha256};

    #[test]
    fn cache_reservation_never_reuses_or_modifies_a_previous_run() {
        let parent =
            std::env::temp_dir().join(format!("diaxus-rtx-cache-test-{}", std::process::id()));
        std::fs::create_dir(&parent).unwrap();
        let old = parent.join("rtx-reused-pid-0");
        std::fs::create_dir(&old).unwrap();
        std::fs::write(old.join("keep"), b"previous cache").unwrap();
        let first = reserve_cache_path(&parent, "reused-pid").unwrap();
        let second = reserve_cache_path(&parent, "reused-pid").unwrap();
        assert_ne!(first, second);
        assert!(first.parent().unwrap().is_dir() && second.parent().unwrap().is_dir());
        assert!(!first.exists() && !second.exists()); // SDK owns creation of this child.
        assert_eq!(std::fs::read(old.join("keep")).unwrap(), b"previous cache");
        std::fs::remove_dir(first.parent().unwrap()).unwrap();
        std::fs::remove_dir(second.parent().unwrap()).unwrap();
        std::fs::remove_file(old.join("keep")).unwrap();
        std::fs::remove_dir(old).unwrap();
        std::fs::remove_dir(parent).unwrap();
    }

    #[test]
    fn whole_frame_admission_preserves_scale_scratch_and_overflow_limits() {
        for (source, scale) in [
            (Size::new(640, 360).unwrap(), AiScale::Four),
            (Size::new(1280, 720).unwrap(), AiScale::Two),
        ] {
            let output = Size::new(
                source.width * scale.factor(),
                source.height * scale.factor(),
            )
            .unwrap();
            let rgba = u64::from(output.width) * u64::from(output.height) * 4;
            let scratch =
                media_gpu::equalization::EqualizationStage::estimated_scratch_bytes(output)
                    .unwrap();
            let total = memory_admission(
                source,
                scale,
                rgba * 3 + scratch,
                rgba + u64::from(source.width) * u64::from(source.height) * 4,
                384 << 20,
            )
            .unwrap();
            assert!(total > scratch + rgba * 4);
            assert!(
                memory_admission(
                    source,
                    scale,
                    rgba * 3 + scratch,
                    rgba + u64::from(source.width) * u64::from(source.height) * 4,
                    total - 1
                )
                .is_err()
            );
            assert!(memory_admission(source, scale, u64::MAX, 1, u64::MAX).is_err());
        }
        assert!(
            media_gpu::equalization::EqualizationStage::estimated_scratch_bytes(
                Size::new(5120, 2880).unwrap()
            )
            .is_err()
        );
        assert!(
            memory_admission(
                Size::new(1280, 720).unwrap(),
                AiScale::Four,
                3 * 5120 * 2880 * 4,
                5120 * 2880 * 4,
                384 << 20
            )
            .is_err()
        );
    }

    #[test]
    fn oversized_handshake_is_rejected_without_reading_its_payload() {
        let mut output = Vec::new();
        write_header(
            &mut output,
            [
                RESPONSE,
                1,
                0,
                0,
                0,
                MAX_SOURCE_WIDTH,
                MAX_SOURCE_HEIGHT,
                0,
                1,
                u32::MAX,
            ],
        )
        .unwrap();
        let (_, requests) = mpsc::sync_channel(0);
        let (replies, _) = mpsc::sync_channel(1);
        let error = run_protocol(
            &mut Vec::new(),
            &mut output.as_slice(),
            requests,
            &replies,
            Size::new(64, 36).unwrap(),
            1,
            AiScale::Four,
        )
        .unwrap_err();
        assert!(error.to_string().contains("bounded RTX handshake"));
    }

    #[test]
    fn output_length_is_checked_against_the_selected_geometry_before_allocation() {
        let identity = serde_json::json!({"protocol":1,"sdk":"1.1.0","pid":1,"dxgi_index":1,
            "adapter_name":"GPU","vendor_id":4318,"device_id":1,"luid_high":0,"luid_low":1,
            "color":"opaque-rgba8-full-range-bt709-srgb","quality":3,"max_width":1280,"max_height":720,"in_flight":1});
        let payload = serde_json::to_vec(&identity).unwrap();
        let mut output = Vec::new();
        write_header(
            &mut output,
            [
                RESPONSE,
                1,
                0,
                0,
                0,
                MAX_SOURCE_WIDTH,
                MAX_SOURCE_HEIGHT,
                0,
                1,
                payload.len() as u32,
            ],
        )
        .unwrap();
        output.extend_from_slice(&payload);
        write_header(&mut output, [RESPONSE, 1, 1, 1, 7, 128, 72, 2, 1, u32::MAX]).unwrap();
        let (sender, requests) = mpsc::sync_channel(1);
        sender
            .send(Request::Frame(
                vec![255; 64 * 36 * 4],
                Size::new(64, 36).unwrap(),
            ))
            .unwrap();
        let (replies, _receiver) = mpsc::sync_channel(1);
        let error = run_protocol(
            &mut Vec::new(),
            &mut output.as_slice(),
            requests,
            &replies,
            Size::new(64, 36).unwrap(),
            7,
            AiScale::Two,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("geometry/color/id/generation/length mismatch")
        );
    }

    #[test]
    #[ignore = "actual explicitly configured NVIDIA RTX helper/SDK and pinned sample hashes"]
    fn actual_rtx_frames_match_sample_hashes_at_both_native_scales() {
        let path = |name| PathBuf::from(std::env::var_os(name).expect(name));
        let config = RtxHelperConfig {
            executable: path("DIAXUS_RTX_HELPER"),
            sdk_root: path("DIAXUS_RTX_SDK"),
            cache_parent: path("DIAXUS_RTX_CACHE"),
            dxgi_index: 1,
            exact_adapter_name: "NVIDIA GeForce RTX 3050 Laptop GPU".into(),
            expected_luid_high: std::env::var("DIAXUS_RTX_LUID_HIGH")
                .unwrap()
                .parse()
                .unwrap(),
            expected_luid_low: std::env::var("DIAXUS_RTX_LUID_LOW")
                .unwrap()
                .parse()
                .unwrap(),
        };
        let report: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/evidence/ai-upscale-20261029/rtx-helper/report.json"
        ))
        .unwrap();
        let cancel = CancellationToken::default();
        for scale in [AiScale::Two, AiScale::Four] {
            let settings = AiSettings::RtxVideo {
                configuration: RtxConfiguration::Sdk11SdrV1,
                scale,
            };
            let mut processor = NativeAiProcessor::new_rtx(
                settings,
                &config,
                Size::new(192, 108).unwrap(),
                7,
                &cancel,
            )
            .unwrap();
            for variant in 0..2 {
                let pixels: Vec<u8> = (0..108)
                    .flat_map(|y| {
                        (0..192).flat_map(move |x| {
                            [
                                ((x * if variant == 1 { 7 } else { 3 } + y * 5) % 256) as u8,
                                (((x / 5 + y / 7 + variant) % 2) * 180 + 30) as u8,
                                ((y * 9 + x * 2 + variant * 103) % 256) as u8,
                                255,
                            ]
                        })
                    })
                    .collect();
                assert!(processor.process_frame(pixels.clone(), 6, &cancel).is_err());
                let first = processor.process_frame(pixels.clone(), 7, &cancel).unwrap();
                let repeat = processor.process_frame(pixels, 7, &cancel).unwrap();
                assert_eq!(first.rgba, repeat.rgba);
                assert_eq!(
                    first.size,
                    Size::new(192 * scale.factor(), 108 * scale.factor()).unwrap()
                );
                assert_eq!(first.host_to_gpu_bytes, 192 * 108 * 4);
                assert_eq!(first.gpu_to_host_bytes, first.rgba.len() as u64);
                let expected = report["cases"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| {
                        row["width"] == 192
                            && row["height"] == 108
                            && row["pattern"] == variant
                            && row["scale"] == scale.factor()
                    })
                    .unwrap();
                assert_eq!(
                    format!("{:x}", Sha256::digest(&first.rgba)),
                    expected["output_sha256"].as_str().unwrap()
                );
                let adapter = crate::enumerate_adapters()
                    .into_iter()
                    .find(|a| {
                        a.name == config.exact_adapter_name
                            && a.backend == "Vulkan"
                            && a.vendor == 0x10de
                    })
                    .unwrap();
                let mut gpu =
                    crate::GpuProcessor::new(first.size, first.size, Some(&adapter.key)).unwrap();
                gpu.prepare_ai_postscale(true);
                let mut preencoder = Vec::new();
                gpu.process(&first.rgba, 0, &mut preencoder, &cancel)
                    .unwrap();
                assert_eq!(preencoder.len(), first.rgba.len());
                let maximum = preencoder
                    .iter()
                    .zip(&first.rgba)
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap();
                assert!(
                    maximum <= 1,
                    "Neutral native RTX bridge exceeds pinned 1 LSB tolerance: {maximum}"
                );
                println!(
                    "RTX {}x pattern {variant}: exact SDK sample hash, exact repeat, counted transfers, native preencoder max {maximum} LSB",
                    scale.factor()
                );
            }
            processor.close().unwrap();
        }
    }
}

pub(super) fn start(
    config: &RtxHelperConfig,
    source: Size,
    generation: u64,
    scale: AiScale,
    cancel: &CancellationToken,
) -> NativeResult<NativeHelper> {
    cancel.check()?;
    validate_source(source)?;
    if generation == 0
        || u32::try_from(generation).is_err()
        || config.dxgi_index > 15
        || config.exact_adapter_name.is_empty()
    {
        return Err("RTX requires a nonzero u32 generation and explicit DXGI identity".into());
    }
    let sdk = config.sdk_root.canonicalize()?;
    if !sdk.join("bin/Windows/x64/rel/nvngx_vsr.dll").is_file() {
        return Err(
            "RTX SDK runtime is missing; install separately, no automatic provisioning".into(),
        );
    }
    if !config.cache_parent.is_dir() {
        return Err("RTX cache parent must already exist".into());
    }
    #[cfg(test)]
    let require_assessed = current_test_fault().is_none();
    #[cfg(not(test))]
    let require_assessed = true;
    if require_assessed
        && crate::ai_preflight::executable_sha256(&config.executable)? != EVALUATED_HELPER_SHA256
    {
        return Err("RTX helper differs from the evaluated source-built binary; a new build requires fresh acceptance evidence. No fallback".into());
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let cache = reserve_cache_path(
        &config.cache_parent,
        &format!("{}-{generation}-{nonce}", std::process::id()),
    )?;
    let mut command = Command::new(&config.executable);
    command
        .arg(sdk)
        .arg(config.dxgi_index.to_string())
        .arg(&config.exact_adapter_name)
        .arg(cache)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(test)]
    let fault = TEST_FAULT.with(|slot| slot.borrow().clone());
    #[cfg(test)]
    if let Some(fault) = fault.as_ref() {
        command
            .env("DIAXUS_RTX_TEST_PHASE", fault.phase)
            .env("DIAXUS_RTX_TEST_ACTION", fault.action);
    }
    #[allow(unused_mut)]
    let mut owner = NativeHelper::launch(command, source, generation, scale, true, cancel)?;
    #[cfg(test)]
    if fault.is_some() {
        owner.frame_timeout = std::time::Duration::from_secs(3);
    }
    let identity = &owner.identity;
    if identity.runtime != "RTX Video SDK 1.1.0"
        || identity.sdk_configuration.as_deref() != Some("sdk-1.1-sdr-v1/high3")
        || identity.dxgi_index != config.dxgi_index
        || identity.adapter_name != config.exact_adapter_name
        || identity.vendor_id != 0x10de
        || identity.luid_high != config.expected_luid_high
        || identity.luid_low != config.expected_luid_low
    {
        return Err(format!(
            "RTX helper did not retain the explicit SDK/quality/NVIDIA/LUID identity: expected DXGI {} / {} / LUID {}:{}, got runtime {} / configuration {:?} / DXGI {} / {} / vendor {:04x} / LUID {}:{}. Re-inspect the current device identity; no fallback",
            config.dxgi_index, config.exact_adapter_name, config.expected_luid_high, config.expected_luid_low,
            identity.runtime, identity.sdk_configuration, identity.dxgi_index, identity.adapter_name,
            identity.vendor_id, identity.luid_high, identity.luid_low,
        ).into());
    }
    Ok(owner)
}

const REQUEST: u32 = 0x31585452;
const RESPONSE: u32 = 0x31595452;
fn write_header(writer: &mut impl Write, header: [u32; 10]) -> NativeResult<()> {
    for field in header {
        writer.write_all(&field.to_le_bytes())?;
    }
    Ok(())
}
fn read_header(reader: &mut impl Read) -> NativeResult<[u32; 10]> {
    let mut result = [0; 10];
    for field in &mut result {
        let mut bytes = [0; 4];
        reader.read_exact(&mut bytes)?;
        *field = u32::from_le_bytes(bytes);
    }
    if result[0] != RESPONSE || result[1] != 1 {
        return Err("Invalid RTX response magic/version".into());
    }
    Ok(result)
}
#[derive(Deserialize)]
struct Ready {
    protocol: u32,
    sdk: String,
    pid: u32,
    dxgi_index: u32,
    adapter_name: String,
    vendor_id: u32,
    device_id: u32,
    luid_high: u32,
    luid_low: u32,
    color: String,
    quality: u32,
    max_width: u32,
    max_height: u32,
    in_flight: u32,
}

pub(super) fn run_protocol(
    input: &mut impl Write,
    output: &mut impl Read,
    requests: mpsc::Receiver<Request>,
    replies: &mpsc::SyncSender<NativeResult<Reply>>,
    source: Size,
    generation: u64,
    scale: AiScale,
) -> NativeResult<()> {
    let header = read_header(output)?;
    if header[..9]
        != [
            RESPONSE,
            1,
            0,
            0,
            0,
            MAX_SOURCE_WIDTH,
            MAX_SOURCE_HEIGHT,
            0,
            1,
        ]
        || header[9] == 0
        || header[9] > 8192
    {
        return Err("Invalid bounded RTX handshake".into());
    }
    let mut payload = vec![0; header[9] as usize];
    output.read_exact(&mut payload)?;
    let ready: Ready = serde_json::from_slice(&payload)?;
    if ready.protocol != 1
        || ready.sdk != "1.1.0"
        || ready.quality != 3
        || ready.in_flight != 1
        || ready.max_width != MAX_SOURCE_WIDTH
        || ready.max_height != MAX_SOURCE_HEIGHT
        || ready.color != "opaque-rgba8-full-range-bt709-srgb"
    {
        return Err("RTX handshake changed SDK/quality/color/geometry/credit policy".into());
    }
    replies
        .send(Ok(Reply::Ready(HelperIdentity {
            protocol: 1,
            pid: ready.pid,
            runtime: "RTX Video SDK 1.1.0".into(),
            model_sha256: String::new(),
            sdk_configuration: Some("sdk-1.1-sdr-v1/high3".into()),
            dxgi_index: ready.dxgi_index,
            adapter_name: ready.adapter_name,
            vendor_id: ready.vendor_id,
            device_id: ready.device_id,
            luid_high: ready.luid_high,
            luid_low: ready.luid_low,
            cpu_fallback: false,
            tile_interior: 0,
            halo: 0,
        })))
        .map_err(|_| "RTX owner disappeared")?;
    let generation = u32::try_from(generation)?;
    #[cfg(test)]
    let test_fault = TEST_FAULT.with(|slot| slot.borrow().clone());
    let mut request_id = 0u32;
    while let Ok(request) = requests.recv() {
        match request {
            Request::Stop => {
                write_header(input, [REQUEST, 1, 2, 0, 0, 0, 0, 0, 0, 0])?;
                input.flush()?;
                if read_header(output)? != [RESPONSE, 1, 2, 0, 0, 0, 0, 0, 0, 0] {
                    return Err("Invalid RTX shutdown acknowledgement".into());
                }
                break;
            }
            Request::Frame(bytes, size) => {
                if size != source {
                    return Err("RTX source identity changed".into());
                }
                request_id = request_id
                    .checked_add(1)
                    .ok_or("RTX request id exhausted")?;
                let factor = scale.factor();
                let width = source
                    .width
                    .checked_mul(factor)
                    .ok_or("RTX width overflow")?;
                let height = source
                    .height
                    .checked_mul(factor)
                    .ok_or("RTX height overflow")?;
                let length = width
                    .checked_mul(height)
                    .and_then(|v| v.checked_mul(4))
                    .ok_or("RTX output length overflow")?;
                write_header(
                    input,
                    [
                        REQUEST,
                        1,
                        1,
                        request_id,
                        generation,
                        source.width,
                        source.height,
                        factor,
                        1,
                        u32::try_from(bytes.len())?,
                    ],
                )?;
                #[cfg(test)]
                if let Some(fault) = test_fault.as_ref() {
                    fault.audit.lock().unwrap_or_else(|p| p.into_inner())["io_stage"] =
                        "write-payload".into();
                }
                input.write_all(&bytes)?;
                input.flush()?;
                #[cfg(test)]
                if let Some(fault) = test_fault.as_ref() {
                    fault.audit.lock().unwrap_or_else(|p| p.into_inner())["io_stage"] =
                        "read-response-header".into();
                }
                if read_header(output)?
                    != [
                        RESPONSE, 1, 1, request_id, generation, width, height, factor, 1, length,
                    ]
                {
                    return Err("RTX response geometry/color/id/generation/length mismatch".into());
                }
                // Allocate only the expected, already bounded selected-scale output.
                let mut rgba = vec![0; length as usize];
                output.read_exact(&mut rgba)?;
                if rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 255) {
                    return Err("RTX returned nonopaque pixels".into());
                }
                replies
                    .send(Ok(Reply::Frame(AiFrame {
                        rgba,
                        size: Size::new(width, height)?,
                        device_generation: u64::from(generation),
                        host_to_gpu_bytes: bytes.len() as u64,
                        gpu_to_host_bytes: u64::from(length),
                        sampled_process_local_peak: 0,
                    })))
                    .map_err(|_| "RTX owner disappeared")?;
            }
        }
    }
    Ok(())
}
