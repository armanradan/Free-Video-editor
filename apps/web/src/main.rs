use dioxus::prelude::*;
use media_core::{
    CodecAcceleration, ColorAdjustments, FrameRateSpec, OutputProfileId, ResizeSpec, Size,
    VideoBitrate,
};
use media_web::{SourceMetadata, VideoSettings};
use ui::{ColorControls, ConverterControls, JobStatus};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

const MAIN_CSS: Asset = asset!("/assets/main.css");
const M1_SCRIPT: Asset = asset!("/assets/m1.js");
const MEDIA_PIPELINE_SCRIPT: Asset = asset!("/assets/m2.js");
const FFMPEG_CORE_SCRIPT: Asset = asset!("/node_modules/@ffmpeg/core/dist/esm/ffmpeg-core.js");
const FFMPEG_CORE_WASM: Asset = asset!("/node_modules/@ffmpeg/core/dist/esm/ffmpeg-core.wasm");
const FFMPEG_WORKER_SCRIPT: Asset = asset!("/assets/ffmpeg-worker.js");

#[wasm_bindgen::prelude::wasm_bindgen(
    inline_js = "export function previewDelay() { return new Promise(resolve => setTimeout(resolve, 34)); } export function ffmpegSpikeSelected() { return new URL(location.href).searchParams.get('backend') === 'ffmpeg-wasm'; } export function backendHref(backend) { const url = new URL(location.href); if (backend === 'webcodecs') url.searchParams.delete('backend'); else url.searchParams.set('backend', 'ffmpeg-wasm'); return url.href; }"
)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = previewDelay)]
    fn preview_delay() -> js_sys::Promise;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = ffmpegSpikeSelected)]
    fn ffmpeg_spike_selected() -> bool;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = backendHref)]
    fn backend_href(backend: &str) -> String;
}

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
    let ffmpeg_spike = ffmpeg_spike_selected();
    let webcodecs_href = backend_href("webcodecs");
    let ffmpeg_href = backend_href("ffmpeg-wasm");
    use_effect(|| {
        media_web::setup_runtime(&M1_SCRIPT.to_string(), &MEDIA_PIPELINE_SCRIPT.to_string());
        media_web::setup_ffmpeg_assets(
            &FFMPEG_CORE_SCRIPT.to_string(),
            &FFMPEG_CORE_WASM.to_string(),
            &FFMPEG_WORKER_SCRIPT.to_string(),
        );
    });
    let mut color = use_signal(ColorAdjustments::default);
    let mut before = use_signal(|| false);
    let mut preview_seconds = use_signal(|| 0.0_f64);
    let mut source_key = use_signal(|| 0_u64);
    let mut preview_revision = use_signal(|| 0_u64);
    let mut preview_busy = use_signal(|| false);
    let mut preview_playing = use_signal(|| false);
    let mut playback_busy = use_signal(|| false);
    let mut preview_muted = use_signal(|| false);
    let mut playback_token = use_signal(|| 0_u64);
    let mut playback_error = use_signal(String::new);
    let mut preview_status =
        use_signal(|| "Select an input for paused source preview.".to_string());
    let mut status = use_signal(|| "Ready. Select an MP4 with H.264 or H.265 video.".to_string());
    let mut running = use_signal(|| false);
    let selected_gpu =
        use_signal(|| "Selected GPU: determined when processing starts.".to_string());
    let mut download_url = use_signal(String::new);
    let mut download_name = use_signal(String::new);
    let mut profile = use_signal(|| {
        if ffmpeg_spike {
            OutputProfileId::Mp4H264Aac
        } else {
            OutputProfileId::PREFERRED
        }
    });
    let mut acceleration = use_signal(CodecAcceleration::default);
    let mut resize_mode = use_signal(|| "original".to_string());
    let mut exact_width = use_signal(|| "320".to_string());
    let mut exact_height = use_signal(|| "180".to_string());
    let mut preserve_aspect_ratio = use_signal(|| true);
    let mut bitrate_mode = use_signal(|| "recommended".to_string());
    let mut custom_bitrate = use_signal(|| "4".to_string());
    let mut frame_rate = use_signal(|| "original".to_string());
    let mut inspected_source = use_signal(|| None::<SourceMetadata>);
    let output_size = use_signal(|| None::<Size>);
    let resolved_size = use_signal(String::new);
    let mut source_metadata = use_signal(String::new);
    let mut has_source = use_signal(|| false);
    let mut profile_ready = use_signal(|| false);
    let mp4_supported = use_signal(|| false);
    let mp4_reason = use_signal(String::new);
    let hevc_supported = use_signal(|| false);
    let hevc_reason = use_signal(String::new);
    let mut probe_generation = use_signal(|| 0_u64);
    let probe_signals = ProbeSignals {
        ffmpeg_spike,
        generation: probe_generation,
        running,
        profile_ready,
        mp4_supported,
        mp4_reason,
        hevc_supported,
        hevc_reason,
        profile,
        resolved_size,
        source_metadata,
        status,
        bitrate_mode,
        custom_bitrate,
        frame_rate,
        inspected_source,
        output_size,
    };

    // One producer task and one latest desired state, never a task/command per event.
    use_effect(move || {
        let _desired = (
            color(),
            before(),
            preview_seconds(),
            source_key(),
            resize_mode(),
            exact_width(),
            exact_height(),
            preserve_aspect_ratio(),
        );
        let next_revision = preview_revision.peek().wrapping_add(1);
        preview_revision.set(next_revision);
        if running() || !profile_ready() || !has_source() {
            media_web::pause_preview_playback();
            if *preview_playing.peek() {
                preview_playing.set(false);
            }
            return;
        }
        if preview_playing() || playback_busy() {
            return;
        }
        if *preview_busy.peek() {
            return;
        }
        preview_busy.set(true);
        spawn(async move {
            loop {
                let _ = wasm_bindgen_futures::JsFuture::from(preview_delay()).await;
                if *running.peek() || !*profile_ready.peek() || *preview_playing.peek() {
                    break;
                }
                let revision = *preview_revision.peek();
                let resize = requested_resize(
                    &resize_mode.peek(),
                    &exact_width.peek(),
                    &exact_height.peek(),
                    *preserve_aspect_ratio.peek(),
                );
                if let Ok(resize) = resize {
                    let selected_color = if *before.peek() {
                        ColorAdjustments::default()
                    } else {
                        *color.peek()
                    };
                    let result = media_web::preview_source(
                        *source_key.peek(),
                        *preview_seconds.peek(),
                        resize,
                        selected_color,
                    )
                    .await;
                    if revision == *preview_revision.peek() && !*running.peek() {
                        update_gpu(selected_gpu);
                        preview_status.set(match result {
                            Ok(summary) => summary,
                            Err(error) => format!("Source preview unavailable: {error}"),
                        });
                    }
                }
                if revision == *preview_revision.peek() || *running.peek() {
                    break;
                }
            }
            preview_busy.set(false);
        });
    });

    use_drop(move || {
        media_web::release_preview_playback();
    });

    let mut toggle_preview = move || {
        if running() || !profile_ready() || !has_source() {
            return;
        }
        let next_token = playback_token.peek().wrapping_add(1);
        playback_token.set(next_token);
        if preview_playing() {
            media_web::pause_preview_playback();
            preview_seconds.set(media_web::preview_playback_position());
            preview_playing.set(false);
            return;
        }
        let Some(source) = inspected_source() else {
            return;
        };
        let token = *playback_token.peek();
        playback_error.set(String::new());
        let start = source.video_start_us.max(0) as f64 / 1_000_000.0;
        let duration = (source.video_end_us as f64 / 1_000_000.0 - start).max(0.001);
        // Invoke play inside the gesture, rather than after an async codec probe.
        let play =
            media_web::start_preview_playback(preview_seconds(), start, duration, preview_muted());
        preview_playing.set(true);
        spawn(async move {
            let result = match play {
                Ok(play) => wasm_bindgen_futures::JsFuture::from(play)
                    .await
                    .map(|_| ())
                    .map_err(|error| format!("{error:?}")),
                Err(error) => Err(error.to_string()),
            };
            if token != *playback_token.peek() {
                return;
            }
            if let Err(error) = result {
                playback_error.set(format!("Preview playback unavailable: {error}"));
                preview_playing.set(false);
                return;
            }
            while *playback_busy.peek() && token == *playback_token.peek() {
                let _ = wasm_bindgen_futures::JsFuture::from(preview_delay()).await;
            }
            if token != *playback_token.peek() || !*preview_playing.peek() || *running.peek() {
                return;
            }
            playback_busy.set(true);
            let key = *source_key.peek();
            let mut last_position = -1.0_f64;
            let mut last_revision = 0;
            while *preview_playing.peek() && !*running.peek() && key == *source_key.peek() {
                let _ = wasm_bindgen_futures::JsFuture::from(preview_delay()).await;
                if !*preview_playing.peek() || *running.peek() || key != *source_key.peek() {
                    break;
                }
                if *preview_busy.peek() {
                    continue;
                }
                let error = media_web::preview_playback_error();
                if !error.is_empty() {
                    playback_error.set(error);
                    break;
                }
                if !media_web::preview_playback_playing() {
                    preview_seconds.set(media_web::preview_playback_position());
                    break;
                }
                if !media_web::preview_playback_ready() {
                    continue;
                }
                let seconds = media_web::preview_playback_position();
                let revision = *preview_revision.peek();
                if seconds == last_position && revision == last_revision {
                    continue;
                }
                let resize = requested_resize(
                    &resize_mode.peek(),
                    &exact_width.peek(),
                    &exact_height.peek(),
                    *preserve_aspect_ratio.peek(),
                );
                let selected_color = if *before.peek() {
                    ColorAdjustments::default()
                } else {
                    *color.peek()
                };
                let result = match resize {
                    Ok(resize) => {
                        media_web::preview_playback_source(key, seconds, resize, selected_color)
                            .await
                    }
                    Err(error) => {
                        playback_error.set(error);
                        break;
                    }
                };
                if key != *source_key.peek() || *running.peek() {
                    break;
                }
                match result {
                    Ok(summary) => {
                        if revision == *preview_revision.peek() {
                            preview_seconds.set(seconds);
                            preview_status.set(summary);
                            update_gpu(selected_gpu);
                        }
                    }
                    Err(error) => {
                        playback_error.set(format!("Preview playback failed: {error}"));
                        break;
                    }
                }
                last_position = seconds;
                last_revision = *preview_revision.peek();
            }
            if key == *source_key.peek() {
                media_web::pause_preview_playback();
                preview_playing.set(false);
            }
            playback_busy.set(false);
            // Retire decoder lookahead on pause/EOF; keep one original image.
            // The ordinary paused-preview producer performs this serialized job.
            let revision = preview_revision.peek().wrapping_add(1);
            preview_revision.set(revision);
        });
    };

    let convert = move |_| {
        if !profile_ready() {
            status.set("Select an input and wait for its output profile checks.".to_string());
            return;
        }
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
        let mut settings = match video_settings(&bitrate_mode(), &custom_bitrate(), &frame_rate()) {
            Ok(settings) => settings,
            Err(error) => {
                status.set(display_error(error));
                return;
            }
        };
        settings.color = color();
        preview_playing.set(false);
        media_web::pause_preview_playback();
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
                settings,
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
        playback_error.set(String::new());
        let next_token = playback_token.peek().wrapping_add(1);
        playback_token.set(next_token);
        preview_playing.set(false);
        media_web::release_preview_playback();
        let next_source_key = source_key.peek().wrapping_add(1);
        source_key.set(next_source_key);
        preview_seconds.set(0.0);
        preview_status.set("Loading this input's source preview…".into());
        spawn(async {
            let _ = media_web::clear_source_preview().await;
        });
        has_source.set(true);
        profile_ready.set(false);
        source_metadata.set(String::new());
        inspected_source.set(None);
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
    let bitrate_changed = move |event: FormEvent| {
        bitrate_mode.set(event.value());
        reprobe_if_ready(
            has_source(),
            &resize_mode(),
            &exact_width(),
            &exact_height(),
            preserve_aspect_ratio(),
            probe_signals,
        );
    };
    let custom_bitrate_changed = move |event: FormEvent| {
        custom_bitrate.set(event.value());
        reprobe_if_ready(
            has_source(),
            &resize_mode(),
            &exact_width(),
            &exact_height(),
            preserve_aspect_ratio(),
            probe_signals,
        );
    };
    let frame_rate_changed = move |event: FormEvent| {
        frame_rate.set(event.value());
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
        preview_playing.set(false);
        media_web::pause_preview_playback();
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

    let settings = video_settings(&bitrate_mode(), &custom_bitrate(), &frame_rate());
    let bitrate_options = ui::BITRATE_PRESETS
        .iter()
        .copied()
        .map(|(value, label, policy)| {
            let rate = inspected_source()
                .zip(output_size())
                .and_then(|(source, size)| {
                    settings.as_ref().ok().and_then(|settings| {
                        source
                            .resolve_bitrate(
                                size,
                                profile(),
                                VideoSettings {
                                    bitrate: policy,
                                    ..*settings
                                },
                            )
                            .ok()
                    })
                });
            (
                value.to_string(),
                rate.map_or_else(
                    || label.to_string(),
                    |rate| format!("{label} · {:.2} Mbps", f64::from(rate) / 1_000_000.0),
                ),
            )
        })
        .chain(std::iter::once((
            "custom".to_string(),
            "Custom…".to_string(),
        )))
        .collect::<Vec<_>>();
    let output_estimate = match (inspected_source(), output_size(), settings) {
        (_, _, Err(error)) => error,
        (Some(source), Some(size), Ok(settings)) => {
            match source.estimate(size, profile(), settings) {
                Ok((bps, Some(bytes))) => format!(
                    "{:.2} Mbps video · Estimated ~{:.1} MB (VBR may differ)",
                    f64::from(bps) / 1_000_000.0,
                    bytes as f64 / 1_000_000.0
                ),
                Ok((_, None)) => "Estimated size unavailable: unknown duration.".into(),
                Err(error) => error.to_string(),
            }
        }
        _ => "Select a source for adaptive bitrate and estimated output size.".into(),
    };
    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        document::Script { src: M1_SCRIPT, r#type: "module" }
        document::Script { src: MEDIA_PIPELINE_SCRIPT, r#type: "module" }
        main { class: "shell",
            header { class: "page-header",
                div {
                    p { class: "eyebrow", "BROWSER VIDEO CONVERTER" }
                    h1 { "Browser video converter" }
                    p { class: "lede", "Choose a video, output size, and format. The resize runs on your GPU." }
                }
                nav { class: "backend-switch", aria_label: "Encoder backend",
                    if ffmpeg_spike {
                        if running() {
                            span { class: "backend-option is-disabled", "WebCodecs" }
                        } else {
                            a { id: "backend-webcodecs", class: "backend-option", href: webcodecs_href, "WebCodecs" }
                        }
                        span { class: "backend-option is-active", "FFmpeg WASM" }
                    } else {
                        span { class: "backend-option is-active", "WebCodecs" }
                        if running() {
                            span { class: "backend-option is-disabled", "FFmpeg WASM" }
                        } else {
                            a { id: "backend-ffmpeg", class: "backend-option", href: ffmpeg_href, "FFmpeg WASM" }
                        }
                    }
                }
            }
            div { class: "workspace",
                section { class: "controls-panel", aria_label: "Conversion settings",
                    ConverterControls {
                        ffmpeg_spike,
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
                        bitrate_mode: bitrate_mode(), custom_bitrate: custom_bitrate(), bitrate_options,
                        frame_rate: frame_rate(), output_estimate,
                        on_bitrate_change: bitrate_changed, on_custom_bitrate_change: custom_bitrate_changed, on_frame_rate_change: frame_rate_changed,
                        source_metadata: source_metadata(),
                        has_source: has_source(),
                        profile_ready: profile_ready(),
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
                    ColorControls { color: color(), disabled: running() || !has_source(), before: before(),
                        on_change: move |value| { color.set(value); before.set(false); },
                        on_compare: move |_| { let value = !before(); before.set(value); },
                    }
                }
                section { class: "monitor-panel", aria_label: "Preview and progress",
                    div { class: "preview-panel", style: if !running() && (preview_status().starts_with("Loading") || preview_status().starts_with("Source preview unavailable")) { "visibility:hidden" } else { "visibility:visible" },
                        h2 { "GPU preview" }
                        div { class: "preview-viewport", role: "button", tabindex: "0",
                            aria_label: if preview_playing() { "Pause source preview" } else { "Play source preview" },
                            onclick: move |_| toggle_preview(),
                            onkeydown: move |event| { if event.key() == Key::Enter || event.key() == Key::Character(" ".into()) { event.prevent_default(); toggle_preview(); } },
                            div { id: "worker-preview" }
                            canvas { id: "export-canvas", width: "160", height: "90", aria_label: "wgpu output" }
                        }
                    }
                    JobStatus { status: status(), selected_gpu: selected_gpu() }
                    p { id: "source-preview-status", class: "note", "{preview_status}" }
                    if !playback_error().is_empty() {
                        p { id: "source-preview-error", class: "note", role: "alert", "{playback_error}" }
                    }
                    if let Some(source) = inspected_source() {
                        div { class: "preview-playback-controls",
                        div { class: "preview-transport",
                            button { id: "source-preview-play", disabled: running() || !profile_ready(), onclick: move |_| toggle_preview(),
                                if preview_playing() { "Pause" } else { "Play" }
                            }
                            span { class: "note", "{preview_seconds():.2} / {source.duration_seconds:.2} s" }
                            label { class: "inline-check note",
                                input { id: "source-preview-mute", r#type: "checkbox", checked: preview_muted(), oninput: move |event| { preview_muted.set(event.checked()); media_web::mute_preview_playback(event.checked()); } }
                                "Mute"
                            }
                        }
                        label { r#for: "source-preview-position", class: "note", "Source preview position: {preview_seconds():.2} s" }
                        input { id: "source-preview-position", r#type: "range", min: "0", max: "{source.duration_seconds.max(0.001)}", step: "0.01", value: "{preview_seconds()}", disabled: running(),
                            oninput: move |event| { if let Ok(value) = event.value().parse::<f64>() && value.is_finite() && value >= 0.0 { media_web::seek_preview_playback(value); preview_seconds.set(value); } }
                        }
                        }
                    }
                    p { id: "execution-context", class: "note", "Execution: checking worker support." }
                }
            }
            details { class: "technical-notes",
                summary { "Processing and compatibility notes" }
                p { class: "note", "BT.709/sRGB SDR input is normalized through the browser color pipeline. Crop, pixel aspect ratio, rotation, and flip are baked into square-pixel output; HDR and mid-stream geometry changes are rejected. Input uses a bounded cache and output streams to origin-private file storage when available; the status reports any capped memory fallback." }
            }
            details { class: "regression technical-notes",
                summary { "M1 deterministic regression probe" }
                p { "Runs the original embedded 30-frame VP8 correctness fixture." }
                button { disabled: running(), onclick: run_m1, "Run M1 probe" }
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ProbeSignals {
    bitrate_mode: Signal<String>,
    custom_bitrate: Signal<String>,
    frame_rate: Signal<String>,
    inspected_source: Signal<Option<SourceMetadata>>,
    output_size: Signal<Option<Size>>,
    ffmpeg_spike: bool,
    generation: Signal<u64>,
    running: Signal<bool>,
    profile_ready: Signal<bool>,
    mp4_supported: Signal<bool>,
    mp4_reason: Signal<String>,
    hevc_supported: Signal<bool>,
    hevc_reason: Signal<String>,
    profile: Signal<OutputProfileId>,
    resolved_size: Signal<String>,
    source_metadata: Signal<String>,
    status: Signal<String>,
}

fn video_settings(mode: &str, custom: &str, fps: &str) -> Result<VideoSettings, String> {
    let bitrate = match mode {
        "smaller" => VideoBitrate::Smaller,
        "recommended" => VideoBitrate::Recommended,
        "higher" => VideoBitrate::Higher,
        "custom" => VideoBitrate::from_mbps(custom).map_err(|error| error.to_string())?,
        _ => return Err("Unknown bitrate policy.".into()),
    };
    let frame_rate = fps
        .parse::<FrameRateSpec>()
        .map_err(|error| error.to_string())?;
    Ok(VideoSettings {
        bitrate,
        frame_rate,
        ..Default::default()
    })
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
            signals.profile_ready.set(false);
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
    if (signals.generation)() != generation {
        return;
    }
    signals.profile_ready.set(false);
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
    let settings = match video_settings(
        &(signals.bitrate_mode)(),
        &(signals.custom_bitrate)(),
        &(signals.frame_rate)(),
    ) {
        Ok(settings) => settings,
        Err(error) => {
            signals.status.set(display_error(error));
            return;
        }
    };
    let result = media_web::probe_output_profiles("source-file", resize, settings).await;
    if (signals.generation)() != generation || (signals.running)() {
        return;
    }
    match result {
        Ok(capabilities) => {
            signals
                .inspected_source
                .set(Some(capabilities.source.clone()));
            signals.output_size.set(Some(capabilities.output_size));
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
            if signals.ffmpeg_spike && !capabilities.mp4_supported {
                signals.status.set(format!(
                    "FFmpeg WASM spike unavailable for this input at {}×{}: {} Switch to WebCodecs to try its available profiles.",
                    capabilities.output_size.width,
                    capabilities.output_size.height,
                    capabilities.mp4_reason
                ));
                return;
            }
            if !capabilities.mp4_supported && (signals.profile)() == OutputProfileId::Mp4H264Aac {
                signals.profile.set(OutputProfileId::PREFERRED);
            }
            if !capabilities.hevc_supported && (signals.profile)() == OutputProfileId::Mp4H265Aac {
                signals.profile.set(OutputProfileId::PREFERRED);
            }
            signals.profile_ready.set(true);
            let mut available = if signals.ffmpeg_spike {
                vec![]
            } else {
                vec!["WebM/VP8/Opus"]
            };
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
        "{} · display {}×{} · coded {}×{}\nVideo: {} ({}) · {:.3} fps ({}) · {} frames · {:.3} s\nAudio: {} · file size {}",
        source.file_name,
        source.display_size.width,
        source.display_size.height,
        source.coded_size.width,
        source.coded_size.height,
        video_codec,
        source.video_codec,
        source.frame_rate,
        if source.variable_frame_rate {
            "VFR average"
        } else {
            "CFR"
        },
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
