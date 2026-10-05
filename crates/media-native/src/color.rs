//! Explicit BT.709 limited SDR ↔ full-range encoded sRGB boundaries.
use media_core::{ColorAdjustments, Size};

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
    let format = if depth == 10 {
        "yuv420p10le"
    } else {
        "yuv420p"
    };
    format!(
        "{TO_SRGB},format=gbrp16le,scale={}:{}:flags=bilinear,setsar=1,{},{FROM_SRGB},format={format}",
        output.width,
        output.height,
        expression_filter(color)
    )
}
