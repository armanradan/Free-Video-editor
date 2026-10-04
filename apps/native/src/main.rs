#![forbid(unsafe_code)]

use dioxus::prelude::*;
use futures_channel::{mpsc, oneshot};
use futures_util::StreamExt;
use media_core::{OutputProfileId, ResizeSpec};
use media_native::{
    CancellationToken, JobStage, NativeJob, NativeSession, ProcessingRoute,
    SessionConversionReport, enumerate_adapters, enumerate_nvidia_gpus,
};
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};
use ui::ResizePresetButtons;
mod preferences;
mod preview;
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

fn app() -> Element {
    let preview_canvas =
        dioxus::native::use_wgpu(|| preview::Presenter::new(app_state().preview.frames.clone()));
    let adapters = use_hook(enumerate_adapters);
    let nvidia_gpus = use_hook(|| enumerate_nvidia_gpus().unwrap_or_default());
    let nvidia_available = !nvidia_gpus.is_empty();
    let mut dark_mode = use_signal(|| true);
    let mut choosing_gpu = use_signal(|| false);
    let mut showing_preview = use_signal(|| false);
    let mut preview_path = use_signal(String::new);
    let mut pointer_released = use_signal(|| 0_u64);
    use_effect(move || window::set_player_open(showing_preview() && !choosing_gpu()));
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
    let mut output = use_signal(String::new);
    let mut resize = use_signal(|| "original".to_string());
    let mut profile = use_signal(|| OutputProfileId::Mp4H264Aac);
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
    let inspect = move |_| {
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
                    gpu_limitation.set(info.shared_gpu_limitation());
                    source_metadata.set(format!(
                        "{} × {} (display {} × {}), {}, {}, {} frames, audio: {}",
                        info.width,
                        info.height,
                        info.display_width,
                        info.display_height,
                        info.codec,
                        info.pixel_format,
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
        let selected_route = route();
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
                        "Complete: {} frames, {} × {}, {} bytes. Total {} ms: inspection/setup {} ms, conversion {} ms, verification {} ms. Inspection {}. Saved to {}",
                        result.frames_processed,
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
    let gpu_label = adapters
        .iter()
        .find(|adapter| adapter.key == selected_adapter())
        .map(|adapter| format!("{} · {}", adapter.name, adapter.backend))
        .unwrap_or_else(|| "Automatic".to_string());
    rsx! {
        style { {include_str!("../assets/native.css")} }
        div { class: "native-root", "data-theme": if dark_mode() { "dark" } else { "light" },
        onmousedown: move |_| window::set_player_open(false),
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
                        oninput: move |event| { input.set(event.value()); source_metadata.set(String::new()); gpu_limitation.set(None); } }
                    button { disabled: busy.then_some("true"), onclick: pick_source, "Browse…" }
                    button { disabled: (busy || input().trim().is_empty()).then_some("true"), onclick: inspect, "Inspect" }
                    button { disabled: (busy || input().trim().is_empty()).then_some("true"),
                        onclick: move |_| {
                            preview_path.set(input()); showing_preview.set(true);
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
                        placeholder: "Choose a new .mp4 file", disabled: busy.then_some("true"),
                        oninput: move |event| output.set(event.value()) }
                    button { disabled: busy.then_some("true"), onclick: pick_output, "Browse…" }
                }
                }
                p { class: "note", "Existing files are never overwritten." }
            }
            div { class: "settings-grid",
                section { class: "panel",
                    h2 { "Output size" }
                    ResizePresetButtons {
                        running: busy, resize_mode: resize(),
                        on_change: move |value| resize.set(value),
                    }
                    p { class: "note", "Keeps aspect ratio · No upscaling" }
                }
                section { class: "panel",
                    h2 { "Format" }
                    div { class: "choice-list",
                        Choice { label: "MP4 · H.264".to_string(), hint: "8-bit SDR · AAC audio".to_string(),
                            selected: profile() == OutputProfileId::Mp4H264Aac, disabled: busy,
                            on_select: move |_| profile.set(OutputProfileId::Mp4H264Aac) }
                        Choice { label: "MP4 · H.265 Main 10".to_string(), hint: "10-bit SDR · AAC audio · Direct FFmpeg only".to_string(),
                            selected: profile() == OutputProfileId::Mp4H265Main10Aac,
                            disabled: busy || route().uses_wgpu(),
                            on_select: move |_| profile.set(OutputProfileId::Mp4H265Main10Aac) }
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
                    Choice { label: "Shared GPU · software codecs".to_string(), hint: "wgpu resize · CPU decode / encode · 8-bit · Preserves VFR".to_string(),
                        selected: route() == ProcessingRoute::SharedWgpu,
                        disabled: busy || profile() == OutputProfileId::Mp4H265Main10Aac || gpu_limitation().is_some(),
                        on_select: move |_| route.set(ProcessingRoute::SharedWgpu) }
                }
                if let Some(reason) = gpu_limitation() {
                    p { class: "note", "{reason}" }
                } else if source_metadata().is_empty() {
                    p { class: "note", "Inspect once to reuse input checks. Shared GPU stages pixels through CPU memory." }
                }
                if route() != ProcessingRoute::DirectFfmpeg {
                    div { class: "gpu-summary",
                        span { "GPU: {gpu_label}" }
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
                            preview_path.set(verified_output()); showing_preview.set(true);
                        }, "Preview last output" }
                }
                p { id: "native-status", class: "job-status", role: "status", aria_live: "polite", "{status}" }
                p { class: "note", "Last conversion: {active_gpu}" }
            }
            }
            if showing_preview() {
                VideoPlayer { key: "{preview_path}", path: preview_path(), canvas: preview_canvas.to_string(), pointer_released,
                    on_close: move |_| { showing_preview.set(false); window::set_player_open(false); } }
            } else {
                section { class: "panel preview-panel",
                    h2 { "Video preview" }
                    div { class: "preview-empty", "Select a source and click Preview, or preview your converted output." }
                    p { class: "note", "Independent player · Conversion keeps every frame" }
                }
            }
            }
            footer { "Hardware failures never silently fall back to CPU · Preview playback is independent of conversion" }
        }
        }
    }
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
