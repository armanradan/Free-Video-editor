#![forbid(unsafe_code)]

use dioxus::prelude::*;
use media_core::{CodecAcceleration, OutputProfileId};

#[component]
pub fn ConverterControls(
    running: bool,
    download_url: String,
    download_name: String,
    profile: OutputProfileId,
    acceleration: CodecAcceleration,
    resize_mode: String,
    exact_width: String,
    exact_height: String,
    preserve_aspect_ratio: bool,
    resolved_size: String,
    mp4_supported: bool,
    mp4_reason: String,
    on_file_change: EventHandler<FormEvent>,
    on_profile_change: EventHandler<FormEvent>,
    on_acceleration_change: EventHandler<FormEvent>,
    on_resize_mode_change: EventHandler<FormEvent>,
    on_exact_width_change: EventHandler<FormEvent>,
    on_exact_height_change: EventHandler<FormEvent>,
    on_aspect_ratio_change: EventHandler<FormEvent>,
    on_convert: EventHandler<MouseEvent>,
    on_cancel: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        div { class: "control-grid",
            label { r#for: "source-file", "Source MP4 (H.264)" }
            input {
                id: "source-file",
                r#type: "file",
                accept: ".mp4,video/mp4",
                disabled: running,
                onchange: move |event| on_file_change.call(event),
            }
            label { r#for: "resize-preset", "Resize" }
            select {
                id: "resize-preset",
                disabled: running,
                onchange: move |event| on_resize_mode_change.call(event),
                option { value: "original", selected: resize_mode == "original", "Original size" }
                option { value: "percent-75", selected: resize_mode == "percent-75", "75%" }
                option { value: "percent-50", selected: resize_mode == "percent-50", "50%" }
                option { value: "percent-25", selected: resize_mode == "percent-25", "25%" }
                option { value: "exact", selected: resize_mode == "exact", "Exact bounding size" }
            }
            if resize_mode == "exact" {
                label { r#for: "resize-width", "Maximum width" }
                input {
                    id: "resize-width",
                    r#type: "number",
                    min: "2",
                    step: "1",
                    value: exact_width,
                    disabled: running,
                    onchange: move |event| on_exact_width_change.call(event),
                }
                label { r#for: "resize-height", "Maximum height" }
                input {
                    id: "resize-height",
                    r#type: "number",
                    min: "2",
                    step: "1",
                    value: exact_height,
                    disabled: running,
                    onchange: move |event| on_exact_height_change.call(event),
                }
                label { r#for: "resize-aspect", "Geometry" }
                label { class: "inline-check",
                    input {
                        id: "resize-aspect",
                        r#type: "checkbox",
                        checked: preserve_aspect_ratio,
                        disabled: running,
                        onchange: move |event| on_aspect_ratio_change.call(event),
                    }
                    "Preserve aspect ratio"
                }
            }
            if !resolved_size.is_empty() {
                div {}
                p { id: "resolved-size", class: "note", "Resolved output: {resolved_size}" }
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
            label { r#for: "codec-acceleration", "Codec acceleration" }
            select {
                id: "codec-acceleration",
                disabled: running,
                onchange: move |event| on_acceleration_change.call(event),
                option {
                    value: CodecAcceleration::NoPreference.as_str(),
                    selected: acceleration == CodecAcceleration::NoPreference,
                    "Compatibility baseline (no preference)"
                }
                option {
                    value: CodecAcceleration::PreferHardware.as_str(),
                    selected: acceleration == CodecAcceleration::PreferHardware,
                    "Prefer hardware (capability-probed)"
                }
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
