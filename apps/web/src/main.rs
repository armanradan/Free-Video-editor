use dioxus::prelude::*;
use ui::{ConverterControls, JobStatus};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

const MAIN_CSS: Asset = asset!("/assets/main.css");
const M1_SCRIPT: Asset = asset!("/assets/m1.js");
const M2_SCRIPT: Asset = asset!("/assets/m2.js");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut status = use_signal(|| {
        "Ready. Select an MP4 with H.264 video; audio is intentionally omitted in M2.".to_string()
    });
    let mut running = use_signal(|| false);
    let selected_gpu =
        use_signal(|| "Selected GPU: determined when processing starts.".to_string());
    let mut download_url = use_signal(String::new);
    let mut download_name = use_signal(String::new);

    let convert = move |_| {
        running.set(true);
        download_url.set(String::new());
        status.set("Inspecting MP4 container and exact H.264 configuration…".to_string());
        spawn(async move {
            let callback = status_callback(status);
            let result = media_web::convert_m2(
                "source-file",
                "export-canvas",
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
        document::Script { src: M2_SCRIPT }
        main { class: "shell",
            p { class: "eyebrow", "MILESTONE M2" }
            h1 { "Browser video converter" }
            p { class: "lede", "MP4/H.264 → WebCodecs decode → wgpu half-size resize → WebCodecs VP8 → WebM. Video only; every audio track is omitted." }
            ConverterControls { running: running(), download_url: download_url(), download_name: download_name(), on_convert: convert, on_cancel: cancel }
            section { class: "preview-panel",
                div { h2 { "GPU output" } p { "The canvas shows the frame submitted to the encoder." } }
                canvas { id: "export-canvas", width: "160", height: "90", aria_label: "wgpu output" }
            }
            JobStatus { status: status(), selected_gpu: selected_gpu() }
            p { class: "note", "Input is capped at 256 MiB and the finalized WebM is held in memory. Conversion performs no explicit CPU pixel readback; browser-internal copies and codec hardware execution remain unknown." }
            details { class: "regression",
                summary { "M1 deterministic regression probe" }
                p { "Runs the original embedded 30-frame VP8 correctness fixture." }
                button { disabled: running(), onclick: run_m1, "Run M1 probe" }
            }
        }
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
