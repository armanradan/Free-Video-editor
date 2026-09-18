use dioxus::prelude::*;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

const MAIN_CSS: Asset = asset!("/assets/main.css");
const M1_SCRIPT: Asset = asset!("/assets/m1.js");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut status =
        use_signal(|| "Ready. The deterministic 30-frame VP8 fixture is embedded.".to_string());
    let mut running = use_signal(|| false);
    let mut selected_gpu =
        use_signal(|| "Selected GPU: determined when the probe starts.".to_string());

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        document::Script { src: M1_SCRIPT }
        main { class: "shell",
            h1 { "M1 WebCodecs → wgpu → WebCodecs" }
            p { class: "lede", "A fixed VP8 fixture is decoded, resized from 320×180 to 160×90 by shared WGSL, encoded, and independently decoded for verification." }
            div { class: "actions",
                button {
                    id: "run",
                    disabled: running(),
                    onclick: move |_| {
                        running.set(true);
                        status.set("Starting capability probes…".to_string());
                        spawn(async move {
                            let callback = Closure::<dyn FnMut(JsValue)>::new(move |value: JsValue| {
                                if let Some(message) = value.as_string() {
                                    status.set(message);
                                }
                            });
                            let result = media_web::run("export-canvas", callback.as_ref().unchecked_ref::<js_sys::Function>().clone()).await;
                            if let Some(adapter) = media_web::selected_gpu() {
                                selected_gpu.set(format!("Selected GPU: {adapter}"));
                            }
                            match result {
                                Ok(summary) => status.set(summary),
                                Err(error) => {
                                    let message = error.to_string();
                                    if let Some((_, details)) = message.split_once("CANCELLED:") {
                                        status.set(format!("CANCELLED:{details}"));
                                    } else {
                                        status.set(format!("FAILED: {message}"));
                                    }
                                }
                            }
                            running.set(false);
                        });
                    },
                    "Run M1 probe"
                }
                button {
                    id: "cancel",
                    disabled: !running(),
                    onclick: move |_| {
                        media_web::cancel();
                        status.set("Cancellation requested; draining and closing owned frames…".to_string());
                    },
                    "Cancel"
                }
            }
            p { id: "selected-gpu", class: "note", "{selected_gpu}" }
            canvas { id: "export-canvas", width: "160", height: "90", aria_label: "wgpu output" }
            pre { id: "status", role: "status", aria_live: "polite", "{status}" }
            p { class: "note", "Conversion uses no explicit CPU pixel readback. Verification uses VideoFrame.copyTo and reports it separately." }
        }
    }
}
