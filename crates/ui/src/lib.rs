#![forbid(unsafe_code)]

use dioxus::prelude::*;
use media_core::{
    CodecAcceleration, ColorAdjustments, FrameRateSpec, OutputProfileId, VideoBitrate,
};

#[component]
pub fn ColorControls(
    color: ColorAdjustments,
    disabled: bool,
    before: bool,
    on_change: EventHandler<ColorAdjustments>,
    on_compare: EventHandler<MouseEvent>,
) -> Element {
    let (brightness, contrast, saturation) = color.values();
    rsx! {
        details { class: "color-controls", open: true,
            summary { "Color adjustments" }
            for (id, name, value, min, max) in [
                ("brightness", "Brightness", brightness, -100, 100),
                ("contrast", "Contrast %", contrast as i16, 0, 200),
                ("saturation", "Saturation %", saturation as i16, 0, 200),
            ] {
                label { r#for: "color-{id}", "{name}: {value}" }
                input { id: "color-{id}", r#type: "range", min, max, step: "1", value, disabled,
                    oninput: move |event| {
                        if let Ok(value) = event.value().parse::<i16>() {
                            let candidate = match id {
                                "brightness" => ColorAdjustments::new(value, contrast, saturation),
                                "contrast" => ColorAdjustments::new(brightness, value as u16, saturation),
                                _ => ColorAdjustments::new(brightness, contrast, value as u16),
                            };
                            if let Ok(candidate) = candidate { on_change.call(candidate); }
                        }
                    }
                }
            }
            div { class: "color-actions",
                button { id: "color-reset", disabled, onclick: move |_| on_change.call(ColorAdjustments::default()), "Reset color" }
                button { id: "color-compare", disabled, onclick: move |event| on_compare.call(event),
                    if before { "Before — show After" } else { "After — show Before" }
                }
            }
            p { class: "note", "Before/After compares the source preview only; conversion uses the saved sliders. CLAHE is not implemented yet." }
        }
    }
}

const fn fps(numerator: u32, denominator: u32) -> FrameRateSpec {
    FrameRateSpec::Constant {
        numerator,
        denominator,
    }
}

pub const FRAME_RATE_PRESETS: &[(&str, &str, FrameRateSpec)] = &[
    ("original", "Original", FrameRateSpec::Original),
    ("15", "15", fps(15, 1)),
    ("24000/1001", "23.976", fps(24000, 1001)),
    ("24", "24", fps(24, 1)),
    ("25", "25", fps(25, 1)),
    ("30000/1001", "29.970", fps(30000, 1001)),
    ("30", "30", fps(30, 1)),
    ("50", "50", fps(50, 1)),
    ("60000/1001", "59.940", fps(60000, 1001)),
    ("60", "60", fps(60, 1)),
];

pub const BITRATE_PRESETS: &[(&str, &str, VideoBitrate)] = &[
    ("smaller", "Smaller", VideoBitrate::Smaller),
    ("recommended", "Recommended", VideoBitrate::Recommended),
    ("higher", "Higher", VideoBitrate::Higher),
];

pub fn frame_rate_label(spec: FrameRateSpec) -> String {
    if let Some((_, label, _)) = FRAME_RATE_PRESETS.iter().find(|(_, _, rate)| *rate == spec) {
        return (*label).into();
    }
    match spec {
        FrameRateSpec::Original => "Original".into(),
        FrameRateSpec::Constant {
            numerator,
            denominator: 1,
        } => format!("{numerator}"),
        FrameRateSpec::Constant {
            numerator,
            denominator,
        } => format!("{:.3}", f64::from(numerator) / f64::from(denominator)),
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn fps_preset_values_round_trip_through_shared_command_parser() {
        for &(value, label, rate) in FRAME_RATE_PRESETS {
            assert_eq!(value.parse::<FrameRateSpec>().unwrap(), rate);
            assert_eq!(frame_rate_label(rate), label);
        }
        assert_eq!(frame_rate_label("120".parse().unwrap()), "120");
        assert_eq!(frame_rate_label("7/2".parse().unwrap()), "3.500");
    }
}

const RESIZE_PRESETS: &[(&str, &str)] = &[
    ("original", "Original size"),
    ("percent-75", "75%"),
    ("percent-50", "50%"),
    ("percent-25", "25%"),
    ("hd-720p", "HD / 720p"),
    ("fhd-1080p", "Full HD / 1080p"),
    ("dci-2k", "2K width"),
    ("qhd-1440p", "QHD / 1440p"),
    ("uhd-2160p", "4K UHD / 2160p"),
];

// Blitz 0.7.10 does not present HTML select elements as interactive dropdowns.
// Keep the preset vocabulary shared, but use ordinary buttons on that renderer.
#[component]
pub fn ResizePresetButtons(
    running: bool,
    resize_mode: String,
    on_change: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "resize-options", role: "group", aria_label: "Resize preset",
            for &(value, label) in RESIZE_PRESETS {
                button {
                    key: "{value}",
                    class: "preset",
                    r#type: "button",
                    disabled: running.then_some("true"),
                    aria_pressed: resize_mode == value,
                    "data-selected": if resize_mode == value { "true" } else { "false" },
                    onclick: move |_| on_change.call(value.to_string()),
                    "{label}"
                }
            }
        }
    }
}

#[component]
pub fn ResizePresetSelect(
    running: bool,
    resize_mode: String,
    show_exact: bool,
    on_change: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        label { r#for: "resize-preset", "Resize" }
        select {
            id: "resize-preset",
            disabled: running,
            onchange: move |event| on_change.call(event),
            for &(value, label) in RESIZE_PRESETS {
                option { key: "{value}", value, selected: resize_mode == value, "{label}" }
            }
            if show_exact {
                option { value: "exact", selected: resize_mode == "exact", "Exact bounding size" }
            }
        }
    }
}

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
    bitrate_mode: String,
    custom_bitrate: String,
    bitrate_options: Vec<(String, String)>,
    frame_rate: String,
    output_estimate: String,
    on_bitrate_change: EventHandler<FormEvent>,
    on_custom_bitrate_change: EventHandler<FormEvent>,
    on_frame_rate_change: EventHandler<FormEvent>,
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
            ResizePresetSelect {
                running,
                resize_mode: resize_mode.clone(),
                show_exact: true,
                on_change: on_resize_mode_change,
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
                p { class: "profile-note note", "FFmpeg WASM software encoder: live raw-frame streaming uses shared memory and explicit GPU readbacks." }
            }
            label { r#for: "output-fps", "Output FPS" }
            select { id: "output-fps", value: frame_rate.clone(), disabled: running,
                onchange: move |event| on_frame_rate_change.call(event),
                for &(value, label, _) in FRAME_RATE_PRESETS {
                    option { value, selected: frame_rate == value,
                        if value == "original" { "Original — preserve source timing" } else { "{label}" }
                    }
                }
            }
            label { r#for: "video-bitrate", "Video bitrate" }
            select { id: "video-bitrate", value: bitrate_mode.clone(), disabled: running,
                onchange: move |event| on_bitrate_change.call(event),
                for (value, label) in bitrate_options {
                    option { value: value.clone(), selected: bitrate_mode == value, "{label}" }
                }
            }
            if bitrate_mode == "custom" {
                label { r#for: "custom-bitrate", "Custom Mbps" }
                input { id: "custom-bitrate", r#type: "number", min: "0.25", max: "120", step: "0.01", value: custom_bitrate, disabled: running,
                    oninput: move |event| on_custom_bitrate_change.call(event) }
            }
            p { id: "output-estimate", class: "profile-note note", "{output_estimate}" }
            if frame_rate != "original" {
                p { class: "profile-note note", "Fixed FPS duplicates/drops frames; playback speed and audio timing are unchanged. No motion interpolation." }
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
