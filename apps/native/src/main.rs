#![forbid(unsafe_code)]

use dioxus::prelude::*;
use futures_channel::{mpsc, oneshot};
use futures_util::StreamExt;
use media_core::{
    ColorAdjustments, FrameRateSpec, OutputProfileId, ResizeSpec, Size, VideoBitrate,
};
use media_native::{
    CancellationToken, JobStage, NativeJob, NativeSession, ProcessingRoute,
    SessionConversionReport, enumerate_adapters, enumerate_nvidia_gpus,
};
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};
use ui::{ResizePresetButtons, frame_rate_label};
mod preferences;
mod preview;
mod preview_gpu;
mod window;
use preferences::GpuPreferences;

enum JobUpdate {
    Stage(JobStage),
    Finished(Box<Result<SessionConversionReport, String>>),
}

fn stage_label(stage: JobStage) -> &'static str {
    match stage {
        JobStage::Inspecting => {
            "Inspecting input… Checking timeline (reuses an unchanged inspected file)."
        }
        JobStage::Preparing => "Preparing codecs and GPU…",
        JobStage::Converting => "Converting video and audio…",
        JobStage::Verifying => {
            "Verifying output… Decoding and checking every frame/timestamp on CPU."
        }
        JobStage::Publishing => "Publishing verified output…",
    }
}

struct AppState {
    session: Arc<NativeSession>,
    preferences: GpuPreferences,
    startup_status: String,
    preview: preview::PreviewService,
}

static STATE: OnceLock<AppState> = OnceLock::new();

fn app_state() -> &'static AppState {
    STATE.get_or_init(|| {
        let preferences = GpuPreferences::new();
        let (session, startup_status) = match preferences.load() {
            Ok(Some(key)) => match NativeSession::new(Some(&key)) {
                Ok(session) => (session, "Saved GPU preference restored. Choose source and output paths.".into()),
                Err(_) => (NativeSession::new(None).unwrap(), format!("Saved GPU {key} is unavailable. Using Automatic; saved preference retained.")),
            },
            Ok(None) => (NativeSession::new(None).unwrap(), "Ready. Enter source and output MP4 paths.".into()),
            Err(error) => (NativeSession::new(None).unwrap(), format!("GPU preference could not be read: {error}. Using Automatic.")),
        };
        AppState { session: Arc::new(session), preferences, startup_status, preview: preview::PreviewService::default() }
    })
}

fn persist_gpu(key: Option<&str>, message: &str) -> String {
    match app_state().preferences.save(key) {
        Ok(()) => format!("{message} Saved for the next launch."),
        Err(error) => format!("{message} Not saved: {error}. This choice is session-only."),
    }
}

fn resize_spec(value: &str) -> Option<ResizeSpec> {
    Some(match value {
        "original" => ResizeSpec::Original,
        "percent-75" => ResizeSpec::Percent(75),
        "percent-50" => ResizeSpec::Percent(50),
        "percent-25" => ResizeSpec::Percent(25),
        "hd-720p" => ResizeSpec::HD_720P,
        "fhd-1080p" => ResizeSpec::FULL_HD_1080P,
        "dci-2k" => ResizeSpec::DCI_2K,
        "qhd-1440p" => ResizeSpec::QHD_1440P,
        "uhd-2160p" => ResizeSpec::UHD_2160P,
        _ => return None,
    })
}

fn main() {
    use dioxus::native::{LogicalSize, WindowAttributes};
    let window = WindowAttributes::default()
        .with_title("Diaxus · Video Converter")
        .with_inner_size(LogicalSize::new(960.0, 760.0))
        .with_min_inner_size(LogicalSize::new(900.0, 740.0));
    window::launch(app, window);
    // Blitz exits its event loop on the last window's CloseRequested event.
    // Keep this process alive until FFmpeg children and partial outputs are cleaned.
    if let Some(state) = STATE.get() {
        state.preview.shutdown();
        state.session.shutdown();
    }
}

#[component]
fn Choice(
    label: String,
    hint: String,
    selected: bool,
    disabled: bool,
    on_select: EventHandler<MouseEvent>,
) -> Element {
    // Blitz treats a present disabled attribute as true, even disabled="false".
    // Omit the attribute entirely for enabled controls throughout this UI.
    rsx! {
        button {
            class: "choice", r#type: "button", disabled: disabled.then_some("true"),
            aria_pressed: selected,
            "data-selected": if selected { "true" } else { "false" },
            onclick: move |event| on_select.call(event),
            span { class: "choice-title", "{label}" }
            span { class: "choice-hint", "{hint}" }
        }
    }
}

#[cfg(test)]
mod bitrate_tests {
    use super::*;

    #[test]
    fn color_keyboard_changes_only_selected_channel_and_clamps() {
        let initial = ColorAdjustments::new(20, 50, 100).unwrap();
        let gray = color_from_key(initial, 2, window::ColorKey::Min);
        assert_eq!(gray.values(), (20, 50, 0));
        assert_eq!(color_from_key(gray, 2, window::ColorKey::Step(-1)), gray);
        let bright = color_from_key(gray, 0, window::ColorKey::Max);
        assert_eq!(bright.values(), (100, 50, 0));
        assert_eq!(color_from_key(bright, 0, window::ColorKey::Step(1)), bright);
        assert_eq!(
            color_from_key(bright, 1, window::ColorKey::Step(1)).values(),
            (100, 51, 0)
        );
        assert_eq!(color_from_key(initial, 3, window::ColorKey::Min), initial);
    }

    #[test]
    fn custom_mbps_accepts_decimals_without_silently_replacing_invalid_values() {
        let policy = VideoBitrate::BitsPerSecond(4_000_000);
        assert_eq!(
            selected_bitrate(policy, "3.5").unwrap(),
            VideoBitrate::BitsPerSecond(3_500_000)
        );
        for value in ["", "NaN", "inf", "0.2", "121", "oops"] {
            assert!(selected_bitrate(policy, value).is_err());
        }
        assert_eq!(
            selected_bitrate(VideoBitrate::Recommended, "oops").unwrap(),
            VideoBitrate::Recommended
        );
    }
}

fn selected_bitrate(policy: VideoBitrate, custom: &str) -> Result<VideoBitrate, String> {
    if !matches!(policy, VideoBitrate::BitsPerSecond(_)) {
        return Ok(policy);
    }
    VideoBitrate::from_mbps(custom).map_err(|error| error.to_string())
}

fn app() -> Element {
    let preview_canvas =
        dioxus::native::use_wgpu(|| preview::Presenter::new(app_state().preview.frames.clone()));
    let adapters = use_hook(enumerate_adapters);
    let nvidia_gpus = use_hook(|| enumerate_nvidia_gpus().unwrap_or_default());
    let nvidia_available = !nvidia_gpus.is_empty();
    let mut dark_mode = use_signal(|| true);
    let mut choosing_gpu = use_signal(|| false);
    let mut choosing_fps = use_signal(|| false);
    let mut color_expanded = use_signal(|| false);
    let mut color = use_signal(ColorAdjustments::default);
    let mut before = use_signal(|| false);
    let mut preview_is_source = use_signal(|| true);
    let mut showing_preview = use_signal(|| false);
    let mut preview_path = use_signal(String::new);
    let mut pointer_released = use_signal(|| 0_u64);
    use_effect(move || {
        window::set_player_open(showing_preview() && !choosing_gpu() && !choosing_fps())
    });
    use_effect(move || {
        app_state()
            .preview
            .set_color(if before() || !preview_is_source() {
                ColorAdjustments::default()
            } else {
                color()
            });
    });
    use_drop(|| window::set_player_open(false));
    use_future(move || async move {
        loop {
            while let Some(key) = window::take_player_key() {
                if !showing_preview() {
                    continue;
                }
                if key == window::PlayerKey::Close {
                    app_state().preview.cancel();
                    showing_preview.set(false);
                    window::set_player_open(false);
                    continue;
                }
                let state = app_state().preview.state();
                if !state.ready {
                    continue;
                }
                let target = match key {
                    window::PlayerKey::Back => Some(state.position_us.saturating_sub(5_000_000)),
                    window::PlayerKey::Forward => Some(
                        state
                            .position_us
                            .saturating_add(5_000_000)
                            .min(state.duration_us.saturating_sub(1)),
                    ),
                    window::PlayerKey::Start => Some(0),
                    window::PlayerKey::End => Some(state.duration_us.saturating_sub(1)),
                    window::PlayerKey::Toggle => {
                        if state.ended || state.error.is_some() {
                            let _ = app_state().preview.start(
                                PathBuf::from(preview_path()),
                                0,
                                true,
                                state.muted,
                            );
                        } else {
                            app_state().preview.pause(!state.paused);
                        }
                        None
                    }
                    window::PlayerKey::Close => None,
                };
                if let Some(target) = target {
                    let _ = app_state().preview.start(
                        PathBuf::from(preview_path()),
                        target,
                        !state.paused,
                        state.muted,
                    );
                }
            }
            futures_timer::Delay::new(std::time::Duration::from_millis(50)).await;
        }
    });
    let mut verified_output = use_signal(String::new);
    let mut input = use_signal(String::new);
    use_effect(move || {
        if showing_preview() && preview_is_source() && preview_path() != input() {
            app_state().preview.cancel();
            showing_preview.set(false);
        }
    });
    let mut output = use_signal(String::new);
    let mut resize = use_signal(|| "original".to_string());
    let mut profile = use_signal(|| OutputProfileId::Mp4H264Aac);
    let mut bitrate = use_signal(VideoBitrate::default);
    let mut custom_bitrate = use_signal(|| "4".to_string());
    let mut frame_rate = use_signal(FrameRateSpec::default);
    let mut estimate_source = use_signal(|| None::<media_native::SourceInfo>);
    let mut route = use_signal(move || {
        if nvidia_available {
            ProcessingRoute::NvidiaFfmpeg
        } else {
            ProcessingRoute::DirectFfmpeg
        }
    });
    let mut selected_adapter =
        use_signal(|| app_state().session.status().adapter_key.unwrap_or_default());
    let mut job_cancel = use_signal(CancellationToken::default);
    let mut job_stage = use_signal(|| JobStage::Inspecting);
    let mut running = use_signal(|| false);
    let mut switching = use_signal(|| false);
    use_effect(move || {
        if running() || switching() || !color_expanded() {
            window::set_color_focus(None);
        }
    });
    use_drop(|| window::set_color_focus(None));
    use_future(move || async move {
        loop {
            while let Some((index, key)) = window::take_color_key() {
                if !color_expanded() || running() || switching() {
                    continue;
                }
                color.set(color_from_key(color(), index, key));
                before.set(false);
            }
            futures_timer::Delay::new(std::time::Duration::from_millis(50)).await;
        }
    });
    let mut status = use_signal(|| app_state().startup_status.clone());
    let mut source_metadata = use_signal(String::new);
    let mut gpu_limitation = use_signal(|| None::<String>);
    let mut active_gpu = use_signal(|| "No conversion yet".to_string());
    let session = app_state().session.clone();

    let pick_source = move |_| {
        spawn(async move {
            let (send, receive) = oneshot::channel();
            std::thread::spawn(move || {
                let chosen = rfd::FileDialog::new()
                    .add_filter("MP4 video", &["mp4"])
                    .pick_file();
                let _ = send.send(chosen);
            });
            if let Ok(Some(path)) = receive.await {
                input.set(path.display().to_string());
                source_metadata.set(String::new());
                estimate_source.set(None);
                gpu_limitation.set(None);
                status.set("Source selected. Inspect it before conversion.".into());
            }
        });
    };

    let pick_output = move |_| {
        spawn(async move {
            let (send, receive) = oneshot::channel();
            std::thread::spawn(move || {
                let chosen = rfd::FileDialog::new()
                    .add_filter("MP4 video", &["mp4"])
                    .set_file_name("converted.mp4")
                    .save_file();
                let _ = send.send(chosen);
            });
            if let Ok(Some(path)) = receive.await {
                output.set(path.display().to_string());
            }
        });
    };

    let inspect_session = session.clone();
    let inspect =
        move |_| {
            let source = PathBuf::from(input());
            if source.as_os_str().is_empty() {
                status.set("Enter a source path first.".into());
                return;
            }
            running.set(true);
            job_stage.set(JobStage::Inspecting);
            let cancel = CancellationToken::default();
            job_cancel.set(cancel.clone());
            status.set(stage_label(JobStage::Inspecting).into());
            let session = inspect_session.clone();
            let inspected_path = source.clone();
            spawn(async move {
                let (send, receive) = oneshot::channel();
                std::thread::spawn(move || {
                    let _ = send.send(
                        session
                            .inspect(&source, &cancel)
                            .map_err(|error| error.to_string()),
                    );
                });
                let result = receive.await;
                running.set(false);
                if input() != inspected_path.display().to_string() {
                    return;
                }
                match result {
                    Ok(Ok(info)) => {
                        estimate_source.set(Some(info.clone()));
                        gpu_limitation.set(info.shared_gpu_limitation());
                        source_metadata.set(format!(
                        "{} × {} (display {} × {}), {}, {}, {:.3} fps ({}), {} frames, audio: {}",
                        info.width,
                        info.height,
                        info.display_width,
                        info.display_height,
                        info.codec,
                        info.pixel_format,
                        f64::from(info.frame_rate_num) / f64::from(info.frame_rate_den),
                        if info.variable_frame_rate { "VFR average" } else { "CFR" },
                        info.frame_count,
                        info.audio_codec.as_deref().unwrap_or("none"),
                    ));
                        status.set("Source inspected. Choose settings and convert.".into());
                    }
                    Ok(Err(error)) => status.set(format!("Source inspection failed: {error}")),
                    Err(_) => status.set("Source inspection worker stopped.".into()),
                }
            });
        };

    let convert_session = session.clone();
    let convert = move |_| {
        let source = PathBuf::from(input());
        let target = PathBuf::from(output());
        if source.as_os_str().is_empty() || target.as_os_str().is_empty() {
            status.set("Enter both source and output paths.".into());
            return;
        }
        if source == target {
            status.set("Source and output paths must differ.".into());
            return;
        }
        let Some(size) = resize_spec(&resize()) else {
            status.set("Unsupported resize preset.".into());
            return;
        };
        let selected_profile = profile();
        let selected_bitrate = match selected_bitrate(bitrate(), &custom_bitrate()) {
            Ok(value) => value,
            Err(error) => {
                status.set(error);
                return;
            }
        };
        let selected_fps = frame_rate();
        let selected_route = route();
        let selected_color = color();
        if selected_route == ProcessingRoute::NvidiaFfmpeg && !selected_color.is_neutral() {
            status.set("Color adjustments need CPU FFmpeg or shared GPU. Select that route explicitly; direct NVIDIA will not fall back to CPU.".into());
            return;
        }
        if selected_route.uses_wgpu()
            && let Some(reason) = gpu_limitation()
        {
            status.set(reason);
            return;
        }
        if selected_route.uses_wgpu() && selected_profile == OutputProfileId::Mp4H265Main10Aac {
            status.set("Main 10 is supported only by direct FFmpeg.".into());
            return;
        }
        running.set(true);
        let cancel = CancellationToken::default();
        job_cancel.set(cancel.clone());
        job_stage.set(JobStage::Inspecting);
        status.set(stage_label(JobStage::Inspecting).into());
        let session = convert_session.clone();
        spawn(async move {
            // A fixed five stages plus one result, never per-frame UI events.
            let (send, mut receive) = mpsc::unbounded();
            let output_label = target.display().to_string();
            std::thread::spawn(move || {
                let result = session
                    .convert_job(
                        NativeJob {
                            input: &source,
                            output: &target,
                            resize: size,
                            profile: selected_profile,
                            route: selected_route,
                            bitrate: Some(selected_bitrate),
                            frame_rate: selected_fps,
                            color: selected_color,
                        },
                        &cancel,
                        |stage| {
                            let _ = send.unbounded_send(JobUpdate::Stage(stage));
                        },
                    )
                    .map_err(|error| error.to_string());
                let _ = send.unbounded_send(JobUpdate::Finished(Box::new(result)));
            });
            let mut finished = None;
            while let Some(update) = receive.next().await {
                match update {
                    JobUpdate::Stage(stage) => {
                        job_stage.set(stage);
                        if !job_cancel().is_cancelled() {
                            status.set(stage_label(stage).into());
                        }
                    }
                    JobUpdate::Finished(result) => {
                        finished = Some(*result);
                        break;
                    }
                }
            }
            match finished {
                Some(Ok(report)) => {
                    verified_output.set(output_label.clone());
                    let result = report.conversion;
                    active_gpu.set(result.adapter.as_ref().map_or_else(
                        || {
                            result.hardware_gpu.as_ref().map_or_else(
                                || "Direct FFmpeg · CPU".to_string(),
                                |gpu| format!("{} · CUDA / {}", gpu.name, result.video_encoder),
                            )
                        },
                        |adapter| {
                            format!(
                                "{} ({}) · {}",
                                adapter.name, adapter.backend, result.video_encoder
                            )
                        },
                    ));
                    status.set(format!(
                        "Complete: {} input → {} output frames, {} × {}, {} bytes. Total {} ms: inspection/setup {} ms, conversion {} ms, verification {} ms. Inspection {}. Saved to {}",
                        result.frames_processed, result.output_frame_count,
                        result.output_width,
                        result.output_height,
                        result.output_bytes,
                        result.total_ms, result.preflight_ms, result.elapsed_ms, result.verification_ms,
                        if result.inspection_reused { "reused" } else { "scanned" },
                        output_label,
                    ));
                }
                Some(Err(error)) => status.set(format!("Conversion failed: {error}")),
                None => status.set("Conversion worker stopped.".into()),
            }
            running.set(false);
        });
    };

    let busy = running() || switching();
    let estimate = match (estimate_source(), resize_spec(&resize())) {
        (Some(source), Some(size)) => match selected_bitrate(bitrate(), &custom_bitrate())
            .map_err(|error| error.into())
            .and_then(|policy| {
                source.output_estimate_with_rate(size, profile(), policy, frame_rate())
            }) {
            Ok((bps, Some(bytes))) => format!(
                "{:.2} Mbps video · Estimated ~{:.1} MB",
                f64::from(bps) / 1_000_000.0,
                bytes as f64 / 1_000_000.0
            ),
            Ok((bps, None)) => format!(
                "{:.2} Mbps video · Size unavailable: unknown duration",
                f64::from(bps) / 1_000_000.0
            ),
            Err(error) => format!("Estimate unavailable: {error}"),
        },
        _ => "Inspect source for bitrate and size estimate.".to_string(),
    };
    let fps_label = frame_rate_label(frame_rate());
    let bitrate_label = |policy: VideoBitrate, name: &str| -> String {
        let rate = estimate_source().and_then(|source| {
            let size = resize_spec(&resize())?
                .output_size(Size::new(source.display_width, source.display_height).ok()?)
                .ok()?;
            source
                .resolve_bitrate(size, profile(), policy, frame_rate())
                .ok()
        });
        rate.map_or_else(
            || name.to_string(),
            |rate| format!("{name} · {:.2} Mbps", f64::from(rate) / 1_000_000.0),
        )
    };
    let gpu_label = adapters
        .iter()
        .find(|adapter| adapter.key == selected_adapter())
        .map(|adapter| format!("{} · {}", adapter.name, adapter.backend))
        .unwrap_or_else(|| "Automatic".to_string());
    rsx! {
        style { {include_str!("../assets/native.css")} }
        div { class: "native-root", "data-theme": if dark_mode() { "dark" } else { "light" },
        onmousedown: move |_| { window::set_player_open(false); window::set_color_focus(None); },
        onmouseup: move |_| pointer_released.set(pointer_released().wrapping_add(1)),
        main { class: "app-shell",
            header { class: "app-header",
                div {
                    h1 { "Diaxus Video Converter" }
                }
                div { class: "header-actions",
                    span { class: "badge", "Native" }
                    button { onclick: move |_| dark_mode.set(!dark_mode()), aria_pressed: dark_mode(),
                        if dark_mode() { "Light mode" } else { "Dark mode" }
                    }
                }
            }
            section { class: "panel files",
                div { class: "file-field",
                label { r#for: "native-input", "Source video" }
                div { class: "path-row",
                    input { id: "native-input", r#type: "text", value: input,
                        placeholder: "Select an MP4 or enter its path", disabled: busy.then_some("true"),
                        oninput: move |event| { input.set(event.value()); source_metadata.set(String::new()); estimate_source.set(None); gpu_limitation.set(None); } }
                    button { disabled: busy.then_some("true"), onclick: pick_source, "Browse…" }
                    button { disabled: (busy || input().trim().is_empty()).then_some("true"), onclick: inspect, "Inspect" }
                    button { disabled: (busy || input().trim().is_empty()).then_some("true"),
                        onclick: move |_| {
                            preview_is_source.set(true); preview_path.set(input()); showing_preview.set(true);
                        }, "Preview" }
                }
                }
                if !source_metadata().is_empty() {
                    p { class: "metadata", "{source_metadata}" }
                }
                div { class: "file-field",
                label { r#for: "native-output", "Output" }
                div { class: "path-row",
                    input { id: "native-output", r#type: "text", value: output,
                        placeholder: "Choose a new .mp4 file", title: "Existing files are never overwritten", disabled: busy.then_some("true"),
                        oninput: move |event| output.set(event.value()) }
                    button { disabled: busy.then_some("true"), onclick: pick_output, "Browse…" }
                }
                }
            }
            div { class: "settings-grid",
                section { class: "panel",
                    h2 { "Output size" }
                    ResizePresetButtons {
                        running: busy, resize_mode: resize(),
                        on_change: move |value| resize.set(value),
                    }
                    div { class: "fps-summary",
                        button { class: "preset", disabled: busy.then_some("true"), onclick: move |_| choosing_fps.set(true), "FPS: {fps_label}…" }
                        span { class: "note", if frame_rate() == FrameRateSpec::Original { "No upscaling" } else { "CFR · frames resampled" } }
                    }
                    p { class: "note", title: "Approximate size including audio when present and 2% muxing allowance. Variable bitrate can differ substantially from this estimate.", "{estimate}" }
                }
                section { class: "panel",
                    h2 { "Format · MP4 / AAC · SDR" }
                    div { class: "format-options",
                        button { class: "preset", "data-selected": profile() == OutputProfileId::Mp4H264Aac,
                            disabled: busy.then_some("true"), onclick: move |_| profile.set(OutputProfileId::Mp4H264Aac), "H.264 · 8-bit" }
                        button { class: "preset", "data-selected": profile() == OutputProfileId::Mp4H265Main10Aac,
                            title: "H.265 Main 10 · Direct FFmpeg only",
                            disabled: (busy || route().uses_wgpu()).then_some("true"), onclick: move |_| profile.set(OutputProfileId::Mp4H265Main10Aac), "H.265 · 10-bit" }
                    }
                    div { class: "bitrate-options", role: "group", aria_label: "Video bitrate",
                        for &(_, name, policy) in ui::BITRATE_PRESETS {
                            button { class: "preset", "data-selected": bitrate() == policy,
                                disabled: (busy || (estimate_source().is_none() && policy != VideoBitrate::Recommended)).then_some("true"),
                                onclick: move |_| bitrate.set(policy), "{bitrate_label(policy, name)}" }
                        }
                    }
                    div { class: "bitrate-custom",
                        button { class: "preset", "data-selected": matches!(bitrate(), VideoBitrate::BitsPerSecond(_)), disabled: busy.then_some("true"), onclick: move |_| bitrate.set(VideoBitrate::BitsPerSecond(4_000_000)), "Custom" }
                        input { r#type: "text", aria_label: "Custom video bitrate in Mbps", value: custom_bitrate,
                            disabled: busy.then_some("true"), oninput: move |event| { custom_bitrate.set(event.value()); bitrate.set(VideoBitrate::BitsPerSecond(4_000_000)); } }
                        span { class: "note", "Mbps · lower = smaller files" }
                    }
                }
            }
            if choosing_fps() {
                div { class: "picker-backdrop",
                    section { class: "panel fps-picker", role: "dialog", aria_modal: "true", aria_label: "Output frame rate",
                        div { class: "picker-heading", h2 { "Output frame rate" } button { onclick: move |_| choosing_fps.set(false), "Done" } }
                        div { class: "fps-options",
                            for &(_, _, value) in ui::FRAME_RATE_PRESETS {
                                button { "data-selected": frame_rate() == value, onclick: move |_| { frame_rate.set(value); choosing_fps.set(false); }, "{frame_rate_label(value)}" }
                            }
                        }
                        p { class: "note", "Original preserves source timing, including VFR. Fixed FPS duplicates/drops frames; no motion interpolation or speed change. Audio timing is preserved." }
                    }
                }
            }
            div { class: "workspace-row",
            div { class: "conversion-column",
            section { class: "panel processing-panel",
                h2 { "Processing" }
                div { class: "route-options",
                    Choice { label: "Direct FFmpeg · NVIDIA".to_string(), hint: "CUDA decode / resize · NVENC video · AAC on CPU".to_string(),
                        selected: route() == ProcessingRoute::NvidiaFfmpeg, disabled: busy || !nvidia_available,
                        on_select: move |_| route.set(ProcessingRoute::NvidiaFfmpeg) }
                    Choice { label: "Direct FFmpeg · CPU".to_string(), hint: "Software codecs and resizing".to_string(),
                        selected: route() == ProcessingRoute::DirectFfmpeg, disabled: busy,
                        on_select: move |_| route.set(ProcessingRoute::DirectFfmpeg) }
                    Choice { label: "Shared GPU · NVIDIA".to_string(), hint: "NVDEC / wgpu / NVENC · CPU-staged bridge · 8-bit".to_string(),
                        selected: route() == ProcessingRoute::SharedWgpuNvidia,
                        disabled: busy || !nvidia_available || profile() == OutputProfileId::Mp4H265Main10Aac || gpu_limitation().is_some(),
                        on_select: move |_| route.set(ProcessingRoute::SharedWgpuNvidia) }
                    Choice { label: "Shared GPU · software codecs".to_string(), hint: "wgpu resize · CPU decode / encode · 8-bit".to_string(),
                        selected: route() == ProcessingRoute::SharedWgpu,
                        disabled: busy || profile() == OutputProfileId::Mp4H265Main10Aac || gpu_limitation().is_some(),
                        on_select: move |_| route.set(ProcessingRoute::SharedWgpu) }
                }
                if let Some(reason) = gpu_limitation() {
                    p { class: "note", "{reason}" }
                }
                if route() != ProcessingRoute::DirectFfmpeg {
                    div { class: "gpu-summary",
                        span { title: "GPU: {gpu_label}", "GPU: {gpu_label}" }
                        button { disabled: busy.then_some("true"), onclick: move |_| choosing_gpu.set(true), "Change GPU…" }
                    }
                    if choosing_gpu() {
                    div { class: "picker-backdrop",
                    section { class: "panel gpu-picker", role: "dialog", aria_modal: "true", aria_label: "Choose processing GPU",
                        div { class: "picker-heading",
                            h2 { "Processing GPU" }
                            button { onclick: move |_| choosing_gpu.set(false), "Done" }
                        }
                    div { class: "gpu-options", role: "group", aria_label: "Processing GPU",
                        Choice { label: "Automatic".to_string(), hint: if route().uses_nvidia() { "Use the uniquely identified NVIDIA GPU".to_string() } else { "Let wgpu choose an adapter".to_string() },
                            selected: selected_adapter().is_empty(), disabled: busy,
                            on_select: {
                                let adapter_session = session.clone();
                                move |_| {
                                    switching.set(true);
                                    match adapter_session.switch_adapter(None) {
                                        Ok(_) => { selected_adapter.set(String::new()); choosing_gpu.set(false); status.set(persist_gpu(None, "GPU preference set to Automatic.")); }
                                        Err(error) => status.set(format!("GPU selection failed: {error}")),
                                    }
                                    switching.set(false);
                                }
                            }
                        }
                        for adapter in adapters.iter().filter(|adapter| !route().uses_nvidia() || adapter.vendor == 0x10de) {
                            Choice { key: "{adapter.key}", label: adapter.name.clone(), hint: adapter.backend.clone(),
                                selected: selected_adapter() == adapter.key, disabled: busy,
                                on_select: {
                                    let adapter_session = session.clone();
                                    let key = adapter.key.clone();
                                    move |_| {
                                        switching.set(true);
                                        match adapter_session.switch_adapter(Some(&key)) {
                                            Ok(_) => { selected_adapter.set(key.clone()); choosing_gpu.set(false); status.set(persist_gpu(Some(&key), "GPU preference updated.")); }
                                            Err(error) => status.set(format!("GPU selection failed: {error}")),
                                        }
                                        switching.set(false);
                                    }
                                }
                            }
                        }
                    }
                    }
                    }
                    }
                } else {
                    p { class: "note", "Direct FFmpeg does not use the processing GPU preference." }
                }
            }
            section { class: "panel color-panel", aria_label: "Color adjustments",
                button { class: "color-toggle", aria_label: "Expand or collapse color adjustments", aria_expanded: color_expanded(), aria_controls: "native-color-controls",
                    onclick: move |_| {
                        let opening = !color_expanded(); color_expanded.set(opening); window::set_color_focus(None);
                        if opening {
                            app_state().preview.pause(true);
                            if !input().trim().is_empty() { preview_is_source.set(true); preview_path.set(input()); showing_preview.set(true); }
                        }
                    },
                    span { if color_expanded() { "- Color adjustments" } else { "+ Color adjustments" } }
                    span { class: "note", if color().is_neutral() { "Neutral" } else if route() == ProcessingRoute::NvidiaFfmpeg { "Select CPU / Shared GPU" } else { "Adjusted" } }
                }
                if color_expanded() {
                    div { id: "native-color-controls",
                        for (index, name, min, max) in [(0, "Brightness", -100, 100), (1, "Contrast %", 0, 200), (2, "Saturation %", 0, 200)] {
                            NativeColorSlider { index, name, min, max, color, before, pointer_released, disabled: busy }
                        }
                        div { class: "color-actions",
                            button { disabled: busy.then_some("true"), aria_label: "Reset brightness contrast and saturation", onclick: move |_| { color.set(ColorAdjustments::default()); before.set(false); }, "Reset color" }
                            button { disabled: (busy || !showing_preview() || !preview_is_source()).then_some("true"), aria_pressed: before(), aria_label: "Compare original source with adjusted preview",
                                onclick: move |_| { app_state().preview.pause(true); before.set(!before()); }, if before() { "Before · show After" } else { "After · show Before" } }
                            span { class: "note", title: "8-bit display approximation, not exact selected-output geometry. Before affects preview only; saved settings are exported. Output preview bypasses adjustments.", "Preview only comparison" }
                        }
                    }
                }
            }
            section { class: "panel job-panel",
                div { class: "actions",
                    button { class: "primary", disabled: (busy || input().trim().is_empty() || output().trim().is_empty()).then_some("true"),
                        onclick: convert, if running() {
                            match job_stage() {
                                JobStage::Inspecting => "Inspecting…", JobStage::Preparing => "Preparing…", JobStage::Converting => "Converting…", JobStage::Verifying => "Verifying…", JobStage::Publishing => "Publishing…"
                            }
                        } else { "Convert video" } }
                    button { disabled: (!running()).then_some("true"), onclick: move |_| {
                        job_cancel().cancel();
                        session.cancel_active();
                        status.set("Cancelling…".into());
                    }, "Cancel" }
                    button { disabled: (busy || verified_output().is_empty()).then_some("true"),
                        onclick: move |_| {
                            preview_is_source.set(false); preview_path.set(verified_output()); showing_preview.set(true);
                        }, "Preview last output" }
                }
                p { id: "native-status", class: "job-status", role: "status", aria_live: "polite", "{status}" }
                p { class: "note", title: "Last conversion: {active_gpu}", "Last conversion: {active_gpu}" }
            }
            }
            if showing_preview() {
                VideoPlayer { key: "{preview_path}", path: preview_path(), canvas: preview_canvas.to_string(), pointer_released,
                    on_close: move |_| { showing_preview.set(false); window::set_player_open(false); } }
            } else {
                section { class: "panel preview-panel",
                    h2 { "Video preview" }
                    div { class: "preview-empty", "Select a source and click Preview, or preview your converted output." }
                    p { class: "note", "Independent player · Original FPS keeps every frame" }
                }
            }
            }
            footer { "No automatic CPU fallback · Shared GPU uses CPU staging · Preview is independent" }
        }
        }
    }
}

#[component]
fn NativeColorSlider(
    index: usize,
    name: &'static str,
    min: i16,
    max: i16,
    mut color: Signal<ColorAdjustments>,
    mut before: Signal<bool>,
    pointer_released: Signal<u64>,
    disabled: bool,
) -> Element {
    let mut dragging = use_signal(|| false);
    use_effect(move || {
        let _ = pointer_released();
        dragging.set(false);
    });
    let (b, c, s) = color().values();
    let value = [b, c as i16, s as i16][index];
    let percent = f64::from(value - min) * 100.0 / f64::from(max - min);
    let mut change = move |value: i16| {
        let (b, c, s) = color().values();
        let next = match index {
            0 => ColorAdjustments::new(value, c, s),
            1 => ColorAdjustments::new(b, value as u16, s),
            _ => ColorAdjustments::new(b, c, value as u16),
        };
        if let Ok(next) = next {
            color.set(next);
            before.set(false);
        }
    };
    let mut pointer = move |x: f64| {
        let value =
            (f64::from(min) + window::color_fraction(x) * f64::from(max - min)).round() as i16;
        change(value);
    };
    rsx! {
        div { class: "color-row",
        label { r#for: "native-color-{index}", "{name}: {value}" }
        button { id: "native-color-{index}", class: "native-color-track", role: "slider", disabled: disabled.then_some("true"), aria_label: name,
            aria_valuemin: min, aria_valuemax: max, aria_valuenow: value, aria_valuetext: "{name}: {value}", aria_disabled: disabled,
            onfocus: move |_| { if !disabled { window::set_color_focus(Some(index)); window::set_player_open(false); } },
            onblur: move |_| window::set_color_focus(None),
            onmousedown: move |event| { event.stop_propagation(); if !disabled { window::set_color_focus(Some(index)); window::set_player_open(false); dragging.set(true);pointer(event.client_coordinates().x); } },
            onmousemove: move |event| { if !disabled && dragging() { pointer(event.client_coordinates().x); } },
            onmouseup: move |_| dragging.set(false),
            div { class: "timeline-track",
                div { class: "timeline-fill", style: "width: {percent}%" }
                div { class: "timeline-thumb", style: "left: {percent}%" }
            }
        }
        }
    }
}

fn color_from_key(
    color: ColorAdjustments,
    index: usize,
    key: window::ColorKey,
) -> ColorAdjustments {
    let (b, c, s) = color.values();
    let (value, min, max) = match index {
        0 => (b, -100, 100),
        1 => (c as i16, 0, 200),
        2 => (s as i16, 0, 200),
        _ => return color,
    };
    let next = match key {
        window::ColorKey::Step(step) => value.saturating_add(step).clamp(min, max),
        window::ColorKey::Min => min,
        window::ColorKey::Max => max,
    };
    match index {
        0 => ColorAdjustments::new(next, c, s),
        1 => ColorAdjustments::new(b, next as u16, s),
        _ => ColorAdjustments::new(b, c, next as u16),
    }
    .unwrap_or(color)
}

#[component]
fn VideoPlayer(
    path: String,
    canvas: String,
    pointer_released: Signal<u64>,
    on_close: EventHandler,
) -> Element {
    let mut player = use_signal(preview::PlayerState::default);
    let mut muted = use_signal(|| false);
    let mut dragging = use_signal(|| false);
    let mut drag_position = use_signal(|| 0_u64);
    let mut resume_after_seek = use_signal(|| false);
    let mut command_error = use_signal(|| None::<String>);
    let initial_path = path.clone();
    use_effect(move || {
        if let Err(error) = app_state()
            .preview
            .start(PathBuf::from(&initial_path), 0, false, false)
        {
            command_error.set(Some(error));
        }
    });
    use_drop(|| app_state().preview.cancel());
    use_future(move || async move {
        loop {
            let next = app_state().preview.state();
            if *player.peek() != next {
                player.set(next);
            }
            futures_timer::Delay::new(std::time::Duration::from_millis(50)).await;
        }
    });
    let state = player();
    let position = if dragging() {
        drag_position()
    } else {
        state.position_us
    };
    let percent = if state.duration_us == 0 {
        0.0
    } else {
        position as f64 / state.duration_us as f64 * 100.0
    };
    let mut restart = {
        let path = path.clone();
        move |position, playing, mute| {
            command_error.set(
                app_state()
                    .preview
                    .start(PathBuf::from(&path), position, playing, mute)
                    .err(),
            );
            player.set(app_state().preview.state());
        }
    };
    let mut toggle_play = {
        let mut restart = restart.clone();
        move || {
            let state = app_state().preview.state();
            if state.ended || state.error.is_some() {
                restart(0, true, muted());
            } else {
                app_state().preview.pause(!state.paused);
                player.set(app_state().preview.state());
            }
        }
    };
    let mut finish_seek = {
        let mut restart = restart.clone();
        move || {
            if dragging() {
                dragging.set(false);
                restart(drag_position(), resume_after_seek(), muted());
            }
        }
    };
    let release_path = path.clone();
    use_effect(move || {
        let _ = pointer_released();
        // Do not subscribe to dragging here: only a release ends a drag. A
        // release over another main-window control must also finish the seek.
        if *dragging.peek() {
            let position = *drag_position.peek();
            let playing = *resume_after_seek.peek();
            let mute = *muted.peek();
            dragging.set(false);
            command_error.set(
                app_state()
                    .preview
                    .start(PathBuf::from(&release_path), position, playing, mute)
                    .err(),
            );
            player.set(app_state().preview.state());
        }
    });
    let error = command_error().or(state.error.clone());
    rsx! {
            section { class: "panel preview-panel", aria_label: "Video preview",
                onmousedown: move |event| { event.stop_propagation(); window::set_player_open(true); },
                onmouseup: move |_| finish_seek(),
                div { class: "picker-heading",
                    h2 { "Video preview" }
                    button { onclick: move |_| { app_state().preview.cancel(); on_close.call(()); }, "Close" }
                }
                p { class: "preview-path", "{path}" }
                div { class: "preview-viewport",
                    canvas { class: "preview-canvas", "src": "{canvas}", width: "640", height: "360" }
                }
                div { class: "player-timeline", role: "slider", tabindex: "0", aria_label: "Playback position",
                    aria_valuemin: "0", aria_valuemax: state.duration_us / 1_000,
                    aria_valuenow: position / 1_000, aria_valuetext: preview::format_time(position),
                    aria_disabled: !state.ready,
                    onmousedown: move |event| {
                        if player().ready {
                            resume_after_seek.set(!player().paused); dragging.set(true); app_state().preview.pause(true);
                            drag_position.set(preview::seek_from_fraction(window::timeline_fraction(event.client_coordinates().x), player().duration_us));
                        }
                    },
                    onmousemove: move |event| {
                        if dragging() { drag_position.set(preview::seek_from_fraction(window::timeline_fraction(event.client_coordinates().x), player().duration_us)); }
                    },
                    div { class: "timeline-track",
                        div { class: "timeline-fill", style: "width: {percent}%" }
                        div { class: "timeline-thumb", style: "left: {percent}%" }
                    }
                }
                div { class: "preview-controls",
                    button { class: "primary", disabled: (!state.ready).then_some("true"), onclick: move |_| toggle_play(),
                        if !state.ready { "Loading…" } else if state.paused { "Play" } else { "Pause" }
                    }
                    span { class: "player-time", "{preview::format_time(position)} / {preview::format_time(state.duration_us)}" }
                    button { disabled: (!state.ready).then_some("true"), aria_pressed: muted(), onclick: move |_| {
                        muted.set(!muted()); restart(player().position_us, !player().paused, muted());
                    }, if muted() { "Unmute" } else { "Mute" } }
                }
                p { class: "note", if !state.ready { "Preparing preview…" } else if state.ended { "Ended" } else if state.audio_active { "Audio enabled" } else { "Video only" } }
                if let Some(error) = error { p { class: "job-status", role: "alert", "Preview failed: {error}" } }
                p { class: "note", "Drag to seek · ←/→ 5 s · Space play/pause" }
            }
    }
}
