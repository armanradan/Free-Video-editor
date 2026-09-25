#![forbid(unsafe_code)]

use dioxus::prelude::*;
use media_core::{CodecAcceleration, OutputProfileId};

#[component]
pub fn ConverterControls(
    ffmpeg_spike: bool,
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
    source_metadata: String,
    has_source: bool,
    profile_ready: bool,
    mp4_supported: bool,
    mp4_reason: String,
    hevc_supported: bool,
    hevc_reason: String,
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
            label { r#for: "source-file", "Source MP4 (H.264 or H.265/HEVC)" }
            input {
                id: "source-file",
                r#type: "file",
                accept: ".mp4,video/mp4",
                disabled: running,
                onchange: move |event| on_file_change.call(event),
            }
            if !source_metadata.is_empty() {
                div { class: "source-metadata",
                    strong { "Selected input" }
                    p { id: "source-metadata", "{source_metadata}" }
                }
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
                option { value: "hd-720p", selected: resize_mode == "hd-720p", "HD / 720p (up to 1280×720)" }
                option { value: "fhd-1080p", selected: resize_mode == "fhd-1080p", "Full HD / 1080p (up to 1920×1080)" }
                option { value: "dci-2k", selected: resize_mode == "dci-2k", "2K width (2048 px, aspect preserved)" }
                option { value: "qhd-1440p", selected: resize_mode == "qhd-1440p", "QHD / 1440p (up to 2560×1440)" }
                option { value: "uhd-2160p", selected: resize_mode == "uhd-2160p", "4K UHD / 2160p (up to 3840×2160)" }
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
                p { id: "resolved-size", class: "resolved-size note", "Output: {resolved_size}" }
            }
            label { r#for: "output-profile", "Output profile" }
            select {
                id: "output-profile",
                disabled: running || !profile_ready,
                onchange: move |event| on_profile_change.call(event),
                option {
                    value: OutputProfileId::WebmVp8Opus.as_str(),
                    selected: profile == OutputProfileId::WebmVp8Opus,
                    disabled: ffmpeg_spike,
                    "WebM — VP8 + Opus (preserve audio)"
                }
                option {
                    value: OutputProfileId::WebmVp8VideoOnly.as_str(),
                    selected: profile == OutputProfileId::WebmVp8VideoOnly,
                    disabled: ffmpeg_spike,
                    "WebM — VP8 video only"
                }
                option {
                    value: OutputProfileId::Mp4H264Aac.as_str(),
                    selected: profile == OutputProfileId::Mp4H264Aac,
                    disabled: !mp4_supported,
                    if mp4_supported { "MP4 — H.264 + AAC" } else { "MP4 — H.264 + AAC (unavailable)" }
                }
                option {
                    value: OutputProfileId::Mp4H265Aac.as_str(),
                    selected: profile == OutputProfileId::Mp4H265Aac,
                    disabled: ffmpeg_spike || !hevc_supported,
                    if hevc_supported { "MP4 — H.265/HEVC + AAC" } else { "MP4 — H.265/HEVC + AAC (unavailable)" }
                }
            }
            if ffmpeg_spike {
                p { class: "profile-note note", "FFmpeg WASM software encoder: raw frames use temporary disk space and explicit GPU readbacks." }
            }
            if !has_source {
                p { class: "profile-note note", "Select an input to check compatible formats." }
            }
            if has_source && !profile_ready && mp4_reason.starts_with("Checking") {
                p { class: "profile-note note", "Checking output compatibility…" }
            }
            if has_source && !mp4_supported && !mp4_reason.is_empty() && !mp4_reason.starts_with("Checking") {
                p { class: "profile-note note", "H.264 MP4 unavailable: {mp4_reason}" }
            }
            if profile_ready && !hevc_supported && !hevc_reason.is_empty() {
                p { class: "profile-note note", "H.265/HEVC MP4 unavailable: {hevc_reason}" }
            }
            details { class: "advanced-settings",
                summary { "Advanced codec settings" }
                div { class: "advanced-field",
                    label { r#for: "codec-acceleration", "Acceleration" }
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
            }
        }
        div { class: "actions",
            button { id: "convert", disabled: running || !profile_ready, onclick: move |event| on_convert.call(event), "Convert" }
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
