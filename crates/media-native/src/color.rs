//! Explicit BT.709 limited SDR ↔ full-range encoded sRGB boundaries.
use media_core::{ColorAdjustments, Size};

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct InputColor {
    pub matrix: &'static str,
    pub transfer: &'static str,
    pub primaries: &'static str,
    pub assumed: bool,
}

impl InputColor {
    pub const BT709: Self = Self {
        matrix: "bt709",
        transfer: "bt709",
        primaries: "bt709",
        assumed: false,
    };

    pub(crate) fn from_source(
        codec: &str,
        matrix: Option<&str>,
        transfer: Option<&str>,
        primaries: Option<&str>,
    ) -> Self {
        let legacy = matches!(
            codec,
            "mpeg1video" | "mpeg2video" | "wmv1" | "wmv2" | "wmv3" | "vc1"
        );
        let known = |value| matches!(value, Some("bt709" | "smpte170m" | "bt470bg" | "gamma28"));
        Self {
            matrix: match matrix {
                Some("smpte170m") => "smpte170m",
                Some("bt470bg") => "bt470bg",
                Some("bt709") => "bt709",
                _ if legacy => "smpte170m",
                _ => "bt709",
            },
            transfer: match transfer {
                Some("smpte170m") => "smpte170m",
                Some("gamma28") => "bt470bg",
                _ => "bt709",
            },
            primaries: match primaries {
                Some("smpte170m") => "smpte170m",
                Some("bt470bg") => "bt470bg",
                _ => "bt709",
            },
            assumed: !known(matrix) || !known(transfer) || !known(primaries),
        }
    }

    pub(crate) fn from_metadata(video: &serde_json::Value) -> Self {
        Self::from_source(
            video["codec_name"].as_str().unwrap_or(""),
            video["color_space"].as_str(),
            video["color_transfer"].as_str(),
            video["color_primaries"].as_str(),
        )
    }

    pub fn to_srgb(self) -> String {
        format!(
            "zscale=matrixin={}:transferin={}:primariesin={}:rangein=limited:matrix=gbr:transfer=iec61966-2-1:primaries=709:range=full",
            self.matrix, self.transfer, self.primaries
        )
    }
}

pub fn check_filters() -> crate::NativeResult<()> {
    static CHECK: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
    CHECK.get_or_init(|| {
        let result=std::process::Command::new("ffmpeg").args(["-hide_banner","-filters"]).output().map_err(|error| error.to_string())?;
        let names=String::from_utf8_lossy(&result.stdout);
        if result.status.success() && ["zscale","geq"].iter().all(|name| names.lines().any(|line| line.split_whitespace().nth(1)==Some(*name))) { Ok(()) }
        else { Err("Color adjustments require FFmpeg zscale and geq filters; neutral conversion remains available.".into()) }
    }).clone().map_err(Into::into)
}

pub const TO_SRGB: &str = "zscale=matrixin=709:transferin=709:primariesin=709:rangein=limited:matrix=gbr:transfer=iec61966-2-1:primaries=709:range=full";
pub const FROM_SRGB: &str = "zscale=matrixin=gbr:transferin=iec61966-2-1:primariesin=709:rangein=full:matrix=709:transfer=709:primaries=709:range=limited";

pub fn expression_filter(color: ColorAdjustments) -> String {
    let (b, c, s) = color.values();
    let (b, c, s) = (
        f64::from(b) / 100.0,
        f64::from(c) / 100.0,
        f64::from(s) / 100.0,
    );
    let gray = "(0.2126*r(X,Y)+0.7152*g(X,Y)+0.0722*b(X,Y))";
    let expressions = ["r", "g", "b"].map(|channel| {
        format!("{channel}='clip(({gray}+{s}*({channel}(X,Y)-{gray})-32767.5)*{c}+32767.5+{b}*65535,0,65535)'")
    });
    format!("geq={}", expressions.join(":"))
}

pub fn direct_filter(color: ColorAdjustments, output: Size, depth: u8) -> String {
    direct_filter_with_input(color, output, depth, InputColor::BT709)
}

pub(crate) fn direct_filter_with_input(
    color: ColorAdjustments,
    output: Size,
    depth: u8,
    input: InputColor,
) -> String {
    let format = if depth == 10 {
        "yuv420p10le"
    } else {
        "yuv420p"
    };
    format!(
        "{},format=gbrp16le,scale={}:{}:flags=bilinear,setsar=1,{},{FROM_SRGB},format={format}",
        input.to_srgb(),
        output.width,
        output.height,
        expression_filter(color)
    )
}
