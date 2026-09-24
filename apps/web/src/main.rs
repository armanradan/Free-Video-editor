use dioxus::prelude::*;
use media_core::{CodecAcceleration, OutputProfileId, ResizeSpec};
use ui::{ConverterControls, JobStatus};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

const MAIN_CSS: Asset = asset!("/assets/main.css");
const M1_SCRIPT: Asset = asset!("/assets/m1.js");
const MEDIA_PIPELINE_SCRIPT: Asset = asset!("/assets/m2.js");

fn main() {
    // The dedicated media worker initializes this same bundle, but never mounts UI.
    if !js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("document"))
        .unwrap_or(JsValue::UNDEFINED)
        .is_undefined()
    {
        dioxus::launch(App);
    }
}

#[component]
fn App() -> Element {
    use_effect(|| {
        media_web::setup_runtime(&M1_SCRIPT.to_string(), &MEDIA_PIPELINE_SCRIPT.to_string())
    });
    let mut status = use_signal(|| "Ready. Select an MP4 with H.264 or H.265 video.".to_string());
    let mut running = use_signal(|| false);
    let selected_gpu =
        use_signal(|| "Selected GPU: determined when processing starts.".to_string());
    let mut download_url = use_signal(String::new);
    let mut download_name = use_signal(String::new);
    let mut profile = use_signal(|| OutputProfileId::PREFERRED);
    let mut acceleration = use_signal(CodecAcceleration::default);
    let mut resize_mode = use_signal(|| "percent-50".to_string());
    let mut exact_width = use_signal(|| "320".to_string());
    let mut exact_height = use_signal(|| "180".to_string());
    let mut preserve_aspect_ratio = use_signal(|| true);
    let resolved_size = use_signal(String::new);
    let mut source_metadata = use_signal(String::new);
    let mut has_source = use_signal(|| false);
    let mp4_supported = use_signal(|| false);
    let mp4_reason = use_signal(|| "Select a source file to probe this profile.".to_string());
    let hevc_supported = use_signal(|| false);
    let hevc_reason = use_signal(|| "Select a source file to probe this profile.".to_string());
    let mut probe_generation = use_signal(|| 0_u64);
    let probe_signals = ProbeSignals {
        generation: probe_generation,
        running,
        mp4_supported,
        mp4_reason,
        hevc_supported,
        hevc_reason,
        profile,
        resolved_size,
        source_metadata,
        status,
    };

    let convert = move |_| {
        let selected_resize = match requested_resize(
            &resize_mode(),
            &exact_width(),
            &exact_height(),
            preserve_aspect_ratio(),
        ) {
            Ok(resize) => resize,
            Err(error) => {
                status.set(display_error(error));
                return;
            }
        };
        running.set(true);
        download_url.set(String::new());
        status.set("Inspecting MP4 container and exact video codec configuration…".to_string());
        let selected_profile = profile();
        let selected_acceleration = acceleration();
        spawn(async move {
            let callback = status_callback(status);
            let result = media_web::convert_m3(
                "source-file",
                "export-canvas",
                selected_profile,
                selected_acceleration,
                selected_resize,
                callback
                    .as_ref()
                    .unchecked_ref::<js_sys::Function>()
                    .clone(),
            )
            .await;
            update_gpu(selected_gpu);
            match result {
                Ok(result) => {
                    download_url.set(result.download_url);
                    download_name.set(result.file_name);
                    status.set(format!(
                        "{}\nOutput: {} bytes, {} frames, {:.3} seconds.",
                        result.summary,
                        result.output_bytes,
                        result.frame_count,
                        result.duration_seconds
                    ));
                }
                Err(error) => status.set(display_error(error.to_string())),
            }
            running.set(false);
        });
    };
    let profile_changed = move |event: FormEvent| {
        profile.set(match event.value().as_str() {
            "webm-vp8-video-only" => OutputProfileId::WebmVp8VideoOnly,
            "mp4-h264-aac" if mp4_supported() => OutputProfileId::Mp4H264Aac,
            "mp4-h265-aac" if hevc_supported() => OutputProfileId::Mp4H265Aac,
            _ => OutputProfileId::WebmVp8Opus,
        });
    };
    let acceleration_changed = move |event: FormEvent| {
        acceleration.set(match event.value().as_str() {
            "prefer-hardware" => CodecAcceleration::PreferHardware,
            _ => CodecAcceleration::NoPreference,
        });
    };
    let file_changed = move |_| {
        has_source.set(true);
        source_metadata.set(String::new());
        let resize = match requested_resize(
            &resize_mode(),
            &exact_width(),
            &exact_height(),
            preserve_aspect_ratio(),
        ) {
            Ok(resize) => resize,
            Err(error) => {
                status.set(display_error(error));
                return;
            }
        };
        let generation = probe_generation().wrapping_add(1);
        probe_generation.set(generation);
        spawn(probe_resize(resize, generation, probe_signals));
    };
    let resize_mode_changed = move |event: FormEvent| {
        resize_mode.set(event.value());
        reprobe_if_ready(
            has_source(),
            &resize_mode(),
            &exact_width(),
            &exact_height(),
            preserve_aspect_ratio(),
            probe_signals,
        );
    };
    let exact_width_changed = move |event: FormEvent| {
        exact_width.set(event.value());
        reprobe_if_ready(
            has_source(),
            &resize_mode(),
            &exact_width(),
            &exact_height(),
            preserve_aspect_ratio(),
            probe_signals,
        );
    };
    let exact_height_changed = move |event: FormEvent| {
        exact_height.set(event.value());
        reprobe_if_ready(
            has_source(),
            &resize_mode(),
            &exact_width(),
            &exact_height(),
            preserve_aspect_ratio(),
            probe_signals,
        );
    };
    let aspect_ratio_changed = move |event: FormEvent| {
        preserve_aspect_ratio.set(event.checked());
        reprobe_if_ready(
            has_source(),
            &resize_mode(),
            &exact_width(),
            &exact_height(),
            preserve_aspect_ratio(),
            probe_signals,
        );
    };
    let cancel = move |_| {
        media_web::cancel();
        status.set(
            "Cancellation requested; releasing decoder, encoder, frames, and muxer…".to_string(),
        );
    };
    let run_m1 = move |_| {
        running.set(true);
        status.set("Running the deterministic M1 regression probe…".to_string());
        spawn(async move {
            let callback = status_callback(status);
            let result = media_web::run_m1(
                "export-canvas",
                callback
                    .as_ref()
                    .unchecked_ref::<js_sys::Function>()
                    .clone(),
            )
            .await;
            update_gpu(selected_gpu);
            match result {
                Ok(summary) => status.set(summary),
                Err(error) => status.set(display_error(error.to_string())),
            }
            running.set(false);
        });
    };

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        document::Script { src: M1_SCRIPT }
        document::Script { src: MEDIA_PIPELINE_SCRIPT }
        main { class: "shell",
            p { class: "eyebrow", "MILESTONE M3.6 — CONFIGURABLE OUTPUT" }
            h1 { "Browser video converter" }
            p { class: "lede", "MP4/H.264 or H.265 input → WebCodecs decode → configurable wgpu resize → capability-checked WebM/VP8/Opus, MP4/H.264/AAC, or MP4/H.265/AAC output. A video-only WebM profile remains available." }
            ConverterControls {
                running: running(),
                download_url: download_url(),
                download_name: download_name(),
                profile: profile(),
                acceleration: acceleration(),
                resize_mode: resize_mode(),
                exact_width: exact_width(),
                exact_height: exact_height(),
                preserve_aspect_ratio: preserve_aspect_ratio(),
                resolved_size: resolved_size(),
                source_metadata: source_metadata(),
                mp4_supported: mp4_supported(),
                mp4_reason: mp4_reason(),
                hevc_supported: hevc_supported(),
                hevc_reason: hevc_reason(),
                on_file_change: file_changed,
                on_profile_change: profile_changed,
                on_acceleration_change: acceleration_changed,
                on_resize_mode_change: resize_mode_changed,
                on_exact_width_change: exact_width_changed,
                on_exact_height_change: exact_height_changed,
                on_aspect_ratio_change: aspect_ratio_changed,
                on_convert: convert,
                on_cancel: cancel,
            }
            section { class: "preview-panel",
                div { h2 { "GPU output" } p { "The canvas shows the frame submitted to the encoder." } }
                p { id: "execution-context", class: "note", "Execution: worker capabilities will be checked before processing." }
                div { id: "worker-preview" }
                canvas { id: "export-canvas", width: "160", height: "90", aria_label: "wgpu output" }
            }
            JobStatus { status: status(), selected_gpu: selected_gpu() }
            p { class: "note", "BT.709/sRGB SDR input is normalized through the browser color pipeline. Crop, pixel aspect ratio, rotation, and flip are baked into square-pixel output; HDR and mid-stream geometry changes are rejected. Input is read through a bounded cache and output streams to origin-private file storage when available; the status reports any capped memory fallback." }
            details { class: "regression",
                summary { "M1 deterministic regression probe" }
                p { "Runs the original embedded 30-frame VP8 correctness fixture." }
                button { disabled: running(), onclick: run_m1, "Run M1 probe" }
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ProbeSignals {
    generation: Signal<u64>,
    running: Signal<bool>,
    mp4_supported: Signal<bool>,
    mp4_reason: Signal<String>,
    hevc_supported: Signal<bool>,
    hevc_reason: Signal<String>,
    profile: Signal<OutputProfileId>,
    resolved_size: Signal<String>,
    source_metadata: Signal<String>,
    status: Signal<String>,
}

fn requested_resize(
    mode: &str,
    exact_width: &str,
    exact_height: &str,
    preserve_aspect_ratio: bool,
) -> Result<ResizeSpec, String> {
    match mode {
        "original" => Ok(ResizeSpec::Original),
        "percent-75" => Ok(ResizeSpec::Percent(75)),
        "percent-50" => Ok(ResizeSpec::Percent(50)),
        "percent-25" => Ok(ResizeSpec::Percent(25)),
        "hd-720p" => Ok(ResizeSpec::HD_720P),
        "fhd-1080p" => Ok(ResizeSpec::FULL_HD_1080P),
        "dci-2k" => Ok(ResizeSpec::DCI_2K),
        "qhd-1440p" => Ok(ResizeSpec::QHD_1440P),
        "uhd-2160p" => Ok(ResizeSpec::UHD_2160P),
        "exact" => {
            let width = exact_width
                .trim()
                .parse::<u32>()
                .map_err(|_| "Exact output width must be a positive integer.".to_string())?;
            let height = exact_height
                .trim()
                .parse::<u32>()
                .map_err(|_| "Exact output height must be a positive integer.".to_string())?;
            Ok(ResizeSpec::Exact {
                width,
                height,
                preserve_aspect_ratio,
            })
        }
        _ => Err("Unknown resize selection.".to_string()),
    }
}

fn reprobe_if_ready(
    has_source: bool,
    mode: &str,
    exact_width: &str,
    exact_height: &str,
    preserve_aspect_ratio: bool,
    mut signals: ProbeSignals,
) {
    if !has_source || (signals.running)() {
        return;
    }
    let resize = match requested_resize(mode, exact_width, exact_height, preserve_aspect_ratio) {
        Ok(resize) => resize,
        Err(error) => {
            signals.mp4_supported.set(false);
            signals.hevc_supported.set(false);
            signals.resolved_size.set(String::new());
            signals.status.set(display_error(error));
            return;
        }
    };
    let generation = (signals.generation)().wrapping_add(1);
    signals.generation.set(generation);
    spawn(probe_resize(resize, generation, signals));
}

async fn probe_resize(resize: ResizeSpec, generation: u64, mut signals: ProbeSignals) {
    signals.mp4_supported.set(false);
    signals.hevc_supported.set(false);
    signals
        .mp4_reason
        .set("Checking the exact H.264/AAC encoder configuration…".to_string());
    signals
        .hevc_reason
        .set("Checking the exact H.265/AAC encoder configuration…".to_string());
    signals.resolved_size.set(String::new());
    signals
        .status
        .set("Inspecting input and probing output profiles for the selected size…".to_string());
    let result = media_web::probe_output_profiles("source-file", resize).await;
    if (signals.generation)() != generation || (signals.running)() {
        return;
    }
    match result {
        Ok(capabilities) => {
            signals.mp4_supported.set(capabilities.mp4_supported);
            signals.mp4_reason.set(capabilities.mp4_reason.clone());
            signals.hevc_supported.set(capabilities.hevc_supported);
            signals.hevc_reason.set(capabilities.hevc_reason.clone());
            signals.resolved_size.set(format!(
                "{}×{} (codec-safe)",
                capabilities.output_size.width, capabilities.output_size.height
            ));
            signals
                .source_metadata
                .set(format_source_metadata(&capabilities.source));
            if !capabilities.mp4_supported && (signals.profile)() == OutputProfileId::Mp4H264Aac {
                signals.profile.set(OutputProfileId::PREFERRED);
            }
            if !capabilities.hevc_supported && (signals.profile)() == OutputProfileId::Mp4H265Aac {
                signals.profile.set(OutputProfileId::PREFERRED);
            }
            let mut available = vec!["WebM/VP8/Opus"];
            if capabilities.mp4_supported {
                available.push("MP4/H.264/AAC");
            }
            if capabilities.hevc_supported {
                available.push("MP4/H.265/AAC");
            }
            signals.status.set(format!(
                "Ready. Output resolves to {}×{}. Supported profiles: {}.",
                capabilities.output_size.width,
                capabilities.output_size.height,
                available.join(", ")
            ));
        }
        Err(error) => {
            signals
                .mp4_reason
                .set("Input or resize inspection failed.".to_string());
            signals
                .hevc_reason
                .set("Input or resize inspection failed.".to_string());
            signals.resolved_size.set(String::new());
            signals.status.set(display_error(error.to_string()));
        }
    }
}

fn format_source_metadata(source: &media_web::SourceMetadata) -> String {
    let video_codec =
        if source.video_codec.starts_with("hvc1") || source.video_codec.starts_with("hev1") {
            "H.265/HEVC"
        } else if source.video_codec.starts_with("avc1") || source.video_codec.starts_with("avc3") {
            "H.264/AVC"
        } else {
            "Unknown video codec"
        };
    let audio = match &source.audio_codec {
        Some(codec) => format!(
            "{} · {} channel{} · {:.1} kHz",
            codec.to_uppercase(),
            source.audio_channels,
            if source.audio_channels == 1 { "" } else { "s" },
            f64::from(source.audio_sample_rate) / 1_000.0
        ),
        None => "No audio track".to_string(),
    };
    format!(
        "{} · display {}×{} · coded {}×{}\nVideo: {} ({}) · {:.3} fps · {} frames · {:.3} s\nAudio: {} · file size {}",
        source.file_name,
        source.display_size.width,
        source.display_size.height,
        source.coded_size.width,
        source.coded_size.height,
        video_codec,
        source.video_codec,
        source.frame_rate,
        source.frame_count,
        source.duration_seconds,
        audio,
        format_bytes(source.file_size)
    )
}

fn format_bytes(bytes: u64) -> String {
    const MIB: f64 = 1_048_576.0;
    const KIB: f64 = 1_024.0;
    if bytes >= 1_048_576 {
        format!("{:.1} MiB", bytes as f64 / MIB)
    } else if bytes >= 1_024 {
        format!("{:.1} KiB", bytes as f64 / KIB)
    } else {
        format!("{bytes} bytes")
    }
}

fn status_callback(mut status: Signal<String>) -> Closure<dyn FnMut(JsValue)> {
    Closure::new(move |value: JsValue| {
        if let Some(message) = value.as_string() {
            status.set(message);
        }
    })
}
fn update_gpu(mut selected_gpu: Signal<String>) {
    if let Some(adapter) = media_web::selected_gpu() {
        selected_gpu.set(format!("Selected GPU: {adapter}"));
    }
}
fn display_error(message: String) -> String {
    if let Some((_, details)) = message.split_once("CANCELLED:") {
        format!("CANCELLED:{details}")
    } else {
        format!("FAILED: {message}")
    }
}
