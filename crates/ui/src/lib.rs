#![forbid(unsafe_code)]

use dioxus::prelude::*;

#[component]
pub fn ConverterControls(
    running: bool,
    download_url: String,
    download_name: String,
    on_convert: EventHandler<MouseEvent>,
    on_cancel: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        div { class: "control-grid",
            label { r#for: "source-file", "Source MP4 (H.264, up to 256 MiB)" }
            input {
                id: "source-file",
                r#type: "file",
                accept: ".mp4,video/mp4",
                disabled: running,
            }
            label { r#for: "resize-preset", "Resize" }
            select { id: "resize-preset", disabled: running,
                option { value: "half", "50% width and height" }
            }
        }
        div { class: "actions",
            button { id: "convert", disabled: running, onclick: move |event| on_convert.call(event), "Convert" }
            button { id: "cancel", disabled: !running, onclick: move |event| on_cancel.call(event), "Cancel" }
            if !download_url.is_empty() {
                a { id: "download", class: "button-link", href: download_url, download: download_name, "Download WebM" }
            }
        }
    }
}

#[component]
pub fn JobStatus(status: String, selected_gpu: String) -> Element {
    rsx! {
        p { id: "selected-gpu", class: "note", "{selected_gpu}" }
        pre { id: "status", role: "status", aria_live: "polite", "{status}" }
    }
}
