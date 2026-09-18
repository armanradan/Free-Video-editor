#![forbid(unsafe_code)]

use dioxus::prelude::*;
use media_core::OutputProfileId;

#[component]
pub fn ConverterControls(
    running: bool,
    download_url: String,
    download_name: String,
    profile: OutputProfileId,
    mp4_supported: bool,
    mp4_reason: String,
    on_file_change: EventHandler<FormEvent>,
    on_profile_change: EventHandler<FormEvent>,
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
                onchange: move |event| on_file_change.call(event),
            }
            label { r#for: "resize-preset", "Resize" }
            select { id: "resize-preset", disabled: running,
                option { value: "half", "50% width and height" }
            }
            label { r#for: "output-profile", "Output profile" }
            select {
                id: "output-profile",
                disabled: running,
                onchange: move |event| on_profile_change.call(event),
                option {
                    value: OutputProfileId::WebmVp8Opus.as_str(),
                    selected: profile == OutputProfileId::WebmVp8Opus,
                    "WebM — VP8 + Opus (preserve audio)"
                }
                option {
                    value: OutputProfileId::WebmVp8VideoOnly.as_str(),
                    selected: profile == OutputProfileId::WebmVp8VideoOnly,
                    "WebM — VP8 video only"
                }
                option {
                    value: OutputProfileId::Mp4H264Aac.as_str(),
                    selected: profile == OutputProfileId::Mp4H264Aac,
                    disabled: !mp4_supported,
                    if mp4_supported { "MP4 — H.264 + AAC" } else { "MP4 — H.264 + AAC (unavailable)" }
                }
            }
            if !mp4_supported && !mp4_reason.is_empty() {
                p { class: "note", "MP4 unavailable: {mp4_reason}" }
            }
        }
        div { class: "actions",
            button { id: "convert", disabled: running, onclick: move |event| on_convert.call(event), "Convert" }
            button { id: "cancel", disabled: !running, onclick: move |event| on_cancel.call(event), "Cancel" }
            if !download_url.is_empty() {
                a { id: "download", class: "button-link", href: download_url, download: download_name, "Download output" }
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
