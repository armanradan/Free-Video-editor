//! Separate, on-demand preview decoding. Never taps conversion frame resources.
use crate::{CancellationToken, CancellationWatch, ChildGuard, NativeResult};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
};

pub const WIDTH: u32 = 640;
pub const HEIGHT: u32 = 360;
pub const FRAME_BYTES: usize = (WIDTH * HEIGHT * 4) as usize;
pub const FILTER: &str = "zscale=matrixin=709:transferin=709:primariesin=709:rangein=limited:matrix=gbr:transfer=iec61966-2-1:primaries=709:range=full,format=gbrp,scale=640:360:force_original_aspect_ratio=decrease:force_divisible_by=2:reset_sar=1,pad=640:360:(ow-iw)/2:(oh-ih)/2:color=black,format=rgba";

#[derive(Clone, Copy, Debug)]
pub struct PreviewInfo {
    pub duration_us: u64,
    pub has_audio: bool,
}

#[derive(Debug)]
pub struct PreviewFrame {
    pub rgba: Vec<u8>,
    /// Requested relative seek position, not a claim of exact decoded-frame PTS.
    pub requested_us: u64,
}

/// Probe bounded metadata and the supported display color policy, not pixels.
pub fn probe_info(path: &Path, cancel: &CancellationToken) -> NativeResult<PreviewInfo> {
    cancel.check()?;
    if !path.is_file() {
        return Err("preview input is not a file".into());
    }
    let mut probe = Command::new("ffprobe");
    probe
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=codec_type,color_space,color_transfer,color_primaries,color_range",
            "-of",
            "json",
        ])
        .arg(path)
        .stdout(Stdio::piped());
    let mut child = ChildGuard::spawn(&mut probe)?;
    let watch = CancellationWatch::new(cancel, &[&child]);
    let mut metadata = Vec::new();
    child
        .take_stdout()?
        .take(65537)
        .read_to_end(&mut metadata)?;
    cancel.check()?;
    if metadata.len() > 65536 {
        return Err("preview metadata exceeded its bound".into());
    }
    let exit = child.wait_cancellable(cancel)?;
    if !exit.success() {
        return Err(child.failure("preview probe", exit).into());
    }
    drop(watch);
    let metadata: serde_json::Value = serde_json::from_slice(&metadata)?;
    let video = metadata["streams"]
        .as_array()
        .and_then(|v| v.iter().find(|stream| stream["codec_type"] == "video"))
        .ok_or("preview input has no video")?;
    for (field, accepted) in [
        ("color_space", "bt709"),
        ("color_transfer", "bt709"),
        ("color_primaries", "bt709"),
        ("color_range", "tv"),
    ] {
        if video[field]
            .as_str()
            .is_some_and(|v| v != accepted && v != "unknown")
        {
            return Err("frame preview supports limited-range BT.709 SDR only; HDR/wide/full-range preview is not implemented".into());
        }
    }
    let duration = metadata["format"]["duration"]
        .as_str()
        .ok_or("preview duration is unavailable")?;
    let duration_us = u64::try_from(crate::parse_decimal_us(duration)?)?;
    if duration_us == 0 {
        return Err("preview duration must be positive".into());
    }
    let has_audio = metadata["streams"]
        .as_array()
        .is_some_and(|streams| streams.iter().any(|stream| stream["codec_type"] == "audio"));
    Ok(PreviewInfo {
        duration_us,
        has_audio,
    })
}

/// Decode one bounded SDR frame into a fixed letterboxed display surface.
/// CPU decode/color conversion and one presenter upload are explicit preview costs.
pub fn decode_frame(
    path: &Path,
    position_us: u64,
    cancel: &CancellationToken,
) -> NativeResult<PreviewFrame> {
    let _ = probe_info(path, cancel)?;
    cancel.check()?;
    let seek = format!("{}.{:06}", position_us / 1_000_000, position_us % 1_000_000);
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-ss",
            &seek,
            "-i",
        ])
        .arg(path)
        .args([
            "-map",
            "0:v:0",
            "-an",
            "-sn",
            "-dn",
            "-frames:v",
            "1",
            "-vf",
            FILTER,
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .stdout(Stdio::piped());
    let mut child = ChildGuard::spawn(&mut command)?;
    let _watch = CancellationWatch::new(cancel, &[&child]);
    let mut rgba = Vec::with_capacity(FRAME_BYTES);
    child
        .take_stdout()?
        .take((FRAME_BYTES + 1) as u64)
        .read_to_end(&mut rgba)?;
    cancel.check()?;
    let exit = child.wait_cancellable(cancel)?;
    if !exit.success() {
        return Err(child.failure("preview decode", exit).into());
    }
    if rgba.len() != FRAME_BYTES {
        return Err(
            "no complete preview frame at this position (possibly past end of video)".into(),
        );
    }
    Ok(PreviewFrame {
        rgba,
        requested_us: position_us,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_real_frame_seek_eof_and_cancellation() {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m35-vfr-offset.mp4");
        let cancel = CancellationToken::default();
        let first = decode_frame(&fixture, 0, &cancel).unwrap();
        let next = decode_frame(&fixture, 500_000, &cancel).unwrap();
        assert_eq!(first.rgba.len(), FRAME_BYTES);
        assert!(
            first
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 255)
        );
        assert_ne!(first.rgba, next.rgba);
        assert!(decode_frame(&fixture, 999_000_000, &cancel).is_err());
        cancel.cancel();
        assert!(decode_frame(&fixture, 0, &cancel).is_err());
        let hdr = fixture.with_file_name("m35-hdr-tagged.mp4");
        assert!(
            decode_frame(&hdr, 0, &CancellationToken::default())
                .unwrap_err()
                .to_string()
                .contains("SDR only")
        );
    }

    #[test]
    fn display_geometry_and_marker_colors_survive_preview_letterboxing() {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m35-geometry-color.mp4");
        let frame = decode_frame(&fixture, 0, &CancellationToken::default()).unwrap();
        let pixel = |x: usize, y: usize| {
            let offset = (y * WIDTH as usize + x) * 4;
            &frame.rgba[offset..offset + 3]
        };
        assert_eq!(pixel(20, 180), &[0, 0, 0]);
        assert_eq!(pixel(620, 180), &[0, 0, 0]);
        let (tl, tr, bl, br) = (
            pixel(258, 14),
            pixel(382, 14),
            pixel(258, 346),
            pixel(382, 346),
        );
        assert!(
            tl[0] > 150
                && u16::from(tl[0]) > u16::from(tl[1]) * 2
                && u16::from(tl[0]) > u16::from(tl[2]) * 2,
            "TL={tl:?}"
        );
        assert!(
            tr[2] > 150
                && u16::from(tr[2]) > u16::from(tr[0]) * 2
                && u16::from(tr[2]) > u16::from(tr[1]) * 2,
            "TR={tr:?}"
        );
        assert!(
            bl[1] > 70
                && u16::from(bl[1]) > u16::from(bl[0]) * 2
                && u16::from(bl[1]) > u16::from(bl[2]) * 2,
            "BL={bl:?}"
        );
        assert!(br[0] > 150 && br[1] > 150 && br[2] < 80, "BR={br:?}");
        let ten_bit = fixture.with_file_name("m4-10bit-sdr.mp4");
        assert_eq!(
            decode_frame(&ten_bit, 0, &CancellationToken::default())
                .unwrap()
                .rgba
                .len(),
            FRAME_BYTES
        );
    }
}
