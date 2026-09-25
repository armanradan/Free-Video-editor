#![forbid(unsafe_code)]

use dioxus::prelude::*;
use futures_channel::oneshot;
use media_core::{OutputProfileId, ResizeSpec};
use media_native::{NativeSession, ProcessingRoute, enumerate_adapters, probe_source_direct};
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};
use ui::ResizePresetSelect;

static SESSION: OnceLock<Arc<NativeSession>> = OnceLock::new();

fn session() -> Arc<NativeSession> {
    SESSION
        .get_or_init(|| Arc::new(NativeSession::new(None).expect("default native session")))
        .clone()
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
    dioxus::launch(app);
}

fn app() -> Element {
    let adapters = use_hook(enumerate_adapters);
    let mut input = use_signal(String::new);
    let mut output = use_signal(String::new);
    let mut resize = use_signal(|| "original".to_string());
    let mut profile = use_signal(|| OutputProfileId::Mp4H264Aac);
    let mut route = use_signal(|| ProcessingRoute::DirectFfmpeg);
    let mut selected_adapter = use_signal(String::new);
    let mut running = use_signal(|| false);
    let mut switching = use_signal(|| false);
    let mut status = use_signal(|| "Ready. Enter source and output MP4 paths.".to_string());
    let mut source_metadata = use_signal(String::new);
    let mut active_gpu = use_signal(|| "No processing GPU selected".to_string());
    let session = session();

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

    let inspect = move |_| {
        let source = PathBuf::from(input());
        if source.as_os_str().is_empty() {
            status.set("Enter a source path first.".into());
            return;
        }
        status.set("Inspecting source…".into());
        let inspected_path = source.clone();
        spawn(async move {
            let (send, receive) = oneshot::channel();
            std::thread::spawn(move || {
                let _ = send.send(probe_source_direct(&source).map_err(|error| error.to_string()));
            });
            let result = receive.await;
            if input() != inspected_path.display().to_string() {
                return;
            }
            match result {
                Ok(Ok(info)) => {
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
        if selected_route == ProcessingRoute::SharedWgpu
            && selected_profile == OutputProfileId::Mp4H265Main10Aac
        {
            status.set("Main 10 is supported only by direct FFmpeg.".into());
            return;
        }
        running.set(true);
        status.set("Converting… Cancel remains available.".into());
        let session = convert_session.clone();
        spawn(async move {
            let (send, receive) = oneshot::channel();
            let output_label = target.display().to_string();
            std::thread::spawn(move || {
                let result = session
                    .convert(&source, &target, size, selected_profile, selected_route)
                    .map_err(|error| error.to_string());
                let _ = send.send(result);
            });
            match receive.await {
                Ok(Ok(report)) => {
                    let result = report.conversion;
                    active_gpu.set(result.adapter.as_ref().map_or_else(
                        || "Direct FFmpeg (no processing GPU)".to_string(),
                        |adapter| format!("{} ({})", adapter.name, adapter.backend),
                    ));
                    status.set(format!(
                        "Complete: {} frames, {} × {}, {} bytes, {} ms. Saved to {}",
                        result.frames_processed,
                        result.output_width,
                        result.output_height,
                        result.output_bytes,
                        result.elapsed_ms,
                        output_label,
                    ));
                }
                Ok(Err(error)) => status.set(format!("Conversion failed: {error}")),
                Err(_) => status.set("Conversion worker stopped.".into()),
            }
            running.set(false);
        });
    };

    let adapter_session = session.clone();
    let preferred_gpu = adapters
        .iter()
        .find(|adapter| adapter.key == selected_adapter())
        .map_or_else(
            || "Automatic (used only by the wgpu route)".to_string(),
            |adapter| format!("{} ({})", adapter.name, adapter.backend),
        );
    rsx! {
        div { style: "padding: 24px; max-width: 760px; font-family: sans-serif;",
            h1 { "Diaxus Video Converter" }
            p { "Native Dioxus/Blitz preview — FFmpeg conversion with an optional shared-wgpu route." }
            div {
                label { r#for: "native-input", "Source MP4 path" }
                input { id: "native-input", value: input, disabled: running() || switching(),
                    oninput: move |event| { input.set(event.value()); source_metadata.set(String::new()); } }
                button { disabled: running() || switching(), onclick: pick_source, "Browse…" }
                button { disabled: running() || switching(), onclick: inspect, "Inspect" }
            }
            if !source_metadata().is_empty() { p { "{source_metadata}" } }
            div {
                label { r#for: "native-output", "Output MP4 path (must not exist)" }
                input { id: "native-output", value: output, disabled: running() || switching(),
                    oninput: move |event| output.set(event.value()) }
                button { disabled: running() || switching(), onclick: pick_output, "Browse…" }
            }
            ResizePresetSelect {
                running: running() || switching(),
                resize_mode: resize(),
                show_exact: false,
                on_change: move |event: FormEvent| resize.set(event.value()),
            }
            div {
                label { r#for: "native-profile", "Output profile" }
                select { id: "native-profile", disabled: running() || switching(),
                    onchange: move |event| profile.set(match event.value().as_str() {
                        "mp4-h265-main10-aac" => OutputProfileId::Mp4H265Main10Aac,
                        _ => OutputProfileId::Mp4H264Aac,
                    }),
                    option { value: "mp4-h264-aac", selected: profile() == OutputProfileId::Mp4H264Aac,
                        "MP4 — H.264 + AAC (8-bit)" }
                    option { value: "mp4-h265-main10-aac", selected: profile() == OutputProfileId::Mp4H265Main10Aac,
                        "MP4 — H.265 Main 10 + AAC (SDR)" }
                }
            }
            div {
                label { r#for: "native-route", "Processing route" }
                select { id: "native-route", disabled: running() || switching(),
                    onchange: move |event| route.set(if event.value() == "wgpu" {
                        ProcessingRoute::SharedWgpu
                    } else { ProcessingRoute::DirectFfmpeg }),
                    option { value: "direct", selected: route() == ProcessingRoute::DirectFfmpeg,
                        "Direct FFmpeg (recommended)" }
                    option { value: "wgpu", selected: route() == ProcessingRoute::SharedWgpu,
                        "Shared wgpu (8-bit CFR comparison)" }
                }
            }
            div {
                label { r#for: "native-gpu", "Preferred processing GPU" }
                select { id: "native-gpu", disabled: running() || switching(),
                    onchange: move |event| {
                        let key = event.value();
                        switching.set(true);
                        match adapter_session.switch_adapter(if key.is_empty() { None } else { Some(&key) }) {
                            Ok(generation) => {
                                selected_adapter.set(key.clone());
                                status.set(format!("GPU preference set (generation {generation})."));
                            }
                            Err(error) => status.set(format!("GPU selection failed: {error}")),
                        }
                        switching.set(false);
                    },
                    option { value: "", selected: selected_adapter().is_empty(), "Automatic" }
                    for adapter in adapters.iter() {
                        option { value: "{adapter.key}", selected: selected_adapter() == adapter.key,
                            "{adapter.name} ({adapter.backend})" }
                    }
                }
            }
            p { "Preferred GPU: {preferred_gpu}" }
            p { "Active processing GPU: {active_gpu}" }
            button { disabled: running() || switching(), onclick: convert, "Convert" }
            button { disabled: !running(), onclick: move |_| {
                if session.cancel_active() { status.set("Cancelling…".into()); }
            }, "Cancel" }
            pre { role: "status", "{status}" }
            p { "Preview rendering and hardware codec-surface interoperability are not enabled yet." }
        }
    }
}
