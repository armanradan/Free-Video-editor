//! Exact, bounded paused-preview ingress. No display resizing or pixel history cache.
use super::probe_metadata;
use crate::{CancellationToken, CancellationWatch, ChildGuard, NativeResult, color};
use media_core::{Size, TimeBase};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
};

const MAX_FRAMES: usize = 65; // Up to 64 predecessors plus the selected image.
const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

pub struct HistoryFrame<'a> {
    pub rgba: &'a [u8],
    pub size: Size,
    /// Exact decoder ticks; never the requested seek position.
    pub source_pts: i64,
    pub time_base: TimeBase,
    /// Rounded integer microseconds relative to the video stream origin.
    pub relative_us: i64,
    pub selected: bool,
}

#[derive(Debug)]
pub struct HistorySummary {
    pub selected_us: i64,
    pub predecessors: usize,
    pub frame_bytes: usize,
}

/// Streams the 100 ms seek preroll followed by the first image at/after the
/// requested relative position. The callback must finish consuming/uploading
/// its borrowed pixels before returning. One CPU pixel buffer is reused.
/// Geometry is deliberately restricted to the native shared-conversion policy.
pub fn stream_history(
    path: &Path,
    position_us: u64,
    cancel: &CancellationToken,
    mut consume: impl FnMut(HistoryFrame<'_>) -> NativeResult<()>,
) -> NativeResult<HistorySummary> {
    let metadata = probe_metadata(path, cancel)?;
    let video = metadata["streams"]
        .as_array()
        .and_then(|streams| {
            streams
                .iter()
                .find(|stream| stream["codec_type"] == "video")
        })
        .ok_or("preview input has no video")?;
    if video["sample_aspect_ratio"]
        .as_str()
        .is_some_and(|v| v != "1:1" && v != "N/A")
        || video["side_data_list"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
    {
        return Err(
            "history preview currently requires square pixels without display rotation".into(),
        );
    }
    let size = Size::new(
        u32::try_from(video["width"].as_u64().ok_or("missing preview width")?)?,
        u32::try_from(video["height"].as_u64().ok_or("missing preview height")?)?,
    )?;
    let frame_bytes = usize::try_from(
        u64::from(size.width)
            .checked_mul(u64::from(size.height))
            .and_then(|bytes| bytes.checked_mul(4))
            .ok_or("preview frame size overflow")?,
    )?;
    if frame_bytes > MAX_FRAME_BYTES {
        return Err("history preview frame exceeds 64 MiB budget".into());
    }
    let (num, den) = video["time_base"]
        .as_str()
        .ok_or("missing preview time base")?
        .split_once('/')
        .ok_or("invalid preview time base")?;
    let time_base = TimeBase::new(num.parse()?, den.parse()?)?;
    let origin = video["start_pts"]
        .as_i64()
        .ok_or("missing preview video origin")?;
    let requested = i64::try_from(position_us)?;
    let window_start = requested.saturating_sub(100_000).max(0);
    if origin < 0 {
        return Err("negative preview origin is unsupported".into());
    }
    // FFmpeg input -ss is relative to the container origin. Accurate seek can
    // therefore leave earlier video frames when audio starts first. Trim using
    // exact video ticks, not container-relative seconds, after decode.
    let seek_us = window_start;
    let tick_scale = i128::from(time_base.numerator) * 1_000_000;
    let scaled = i128::from(window_start) * i128::from(time_base.denominator);
    let first_tick = i64::try_from(
        i128::from(origin)
            + (scaled - i128::from(time_base.denominator) / 2 + tick_scale - 1).max(0) / tick_scale,
    )?;
    let seek = format!("{}.{:06}", seek_us / 1_000_000, seek_us % 1_000_000);
    // showinfo observes the very same output frames as the raw pipe. Integer
    // ticks/time base and geometry are checked; decimal pts_time is never used.
    let filter = format!(
        "trim=start_pts={first_tick},{},format=gbrp,format=rgba,trim=end_frame={MAX_FRAMES},showinfo",
        color::TO_SRGB
    );
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "info",
            "-nostdin",
            "-copyts",
            "-ss",
            &seek,
            "-noautorotate",
            "-i",
        ])
        .arg(path)
        .args([
            "-map",
            "0:v:0",
            "-an",
            "-sn",
            "-dn",
            "-vf",
            &filter,
            "-frames:v",
            "65",
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .stdout(Stdio::piped());
    // Metadata only: nonblocking stderr drain with a strict fixed queue bound.
    let (sender, receiver) = mpsc::sync_channel(MAX_FRAMES + 1);
    let mut lines = Vec::with_capacity(4096);
    let mut declared_base = None;
    let mut child = ChildGuard::spawn_observed(&mut command, move |chunk| {
        for &byte in chunk {
            if byte == b'\n' {
                let line = String::from_utf8_lossy(&lines);
                if line.contains("Parsed_showinfo_") {
                    if let Some(base) = line.split("config in time_base: ").nth(1) {
                        declared_base = base.split(',').next().map(str::to_owned);
                    } else if line.contains(" n:") {
                        let parsed = parse_frame(&line, declared_base.as_deref(), time_base, size);
                        // trim bounds output to 65; the extra queue slot permits
                        // one error without waiting on the pixel reader.
                        let _ = sender.try_send(parsed.map_err(|e| e.to_string()));
                    }
                }
                lines.clear();
            } else if lines.len() < 4096 {
                lines.push(byte);
            }
        }
    })?;
    let _watch = CancellationWatch::new(cancel, &[&child]);
    let mut stdout = child.take_stdout()?;
    let mut rgba = vec![0; frame_bytes];
    let mut previous = None;
    for index in 0..MAX_FRAMES {
        cancel.check()?;
        if let Err(error) = stdout.read_exact(&mut rgba) {
            cancel.check()?;
            let exit = child.wait_cancellable(cancel)?;
            return Err(format!(
                "history preview ended before selected frame: {error}; {}",
                child.failure("decoder", exit)
            )
            .into());
        }
        cancel.check()?;
        let (ordinal, pts) = loop {
            match receiver.recv_timeout(std::time::Duration::from_millis(20)) {
                Ok(result) => {
                    break result
                        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => cancel.check()?,
                Err(_) => {
                    return Err("preview pixels arrived without exact timestamp metadata".into());
                }
            }
        };
        if ordinal != index || previous.is_some_and(|v| pts <= v) {
            return Err("preview frame ordering/timestamps are invalid".into());
        }
        previous = Some(pts);
        let relative_us = time_base.ticks_to_microseconds(
            pts.checked_sub(origin)
                .ok_or("preview timestamp overflow")?,
        )?;
        if relative_us < window_start {
            return Err("preview decoder emitted a frame before the seek window".into());
        }
        let selected = relative_us >= requested;
        if index == MAX_FRAMES - 1 && !selected {
            return Err("history preview exceeded 64 predecessor limit".into());
        }
        consume(HistoryFrame {
            rgba: &rgba,
            size,
            source_pts: pts,
            time_base,
            relative_us,
            selected,
        })?;
        cancel.check()?;
        if selected {
            // ChildGuard kills/reaps any decode-ahead; no full-clip decode/drain.
            return Ok(HistorySummary {
                selected_us: relative_us,
                predecessors: index,
                frame_bytes,
            });
        }
    }
    Err("history preview exceeded 64 predecessor limit".into())
}

fn parse_frame(
    line: &str,
    base: Option<&str>,
    expected: TimeBase,
    size: Size,
) -> NativeResult<(usize, i64)> {
    if base != Some(format!("{}/{}", expected.numerator, expected.denominator).as_str()) {
        return Err("preview decoder time base changed".into());
    }
    let field = |name: &str| -> NativeResult<&str> {
        line.split(name)
            .nth(1)
            .and_then(|v| v.split_whitespace().next())
            .ok_or_else(|| format!("missing showinfo {name}").into())
    };
    if field(" s:")? != format!("{}x{}", size.width, size.height) || field(" fmt:")? != "rgba" {
        return Err("preview decoder geometry/pixel format changed".into());
    }
    Ok((field(" n:")?.parse()?, field(" pts:")?.parse()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vfr_seek_pixels_and_exact_pts_match_independent_full_decode() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m35-vfr-offset.mp4");
        let probe = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "frame=best_effort_timestamp",
                "-of",
                "csv=p=0",
            ])
            .arg(&path)
            .output()
            .unwrap();
        assert!(probe.status.success());
        let pts: Vec<i64> = String::from_utf8(probe.stdout)
            .unwrap()
            .lines()
            .filter_map(|line| line.split(',').next()?.parse().ok())
            .collect();
        assert_eq!(pts.len(), 36);
        let filter = format!("{},format=gbrp,format=rgba", color::TO_SRGB);
        let decoded = Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&path)
            .args([
                "-map",
                "0:v:0",
                "-an",
                "-vf",
                &filter,
                "-fps_mode",
                "passthrough",
                "-f",
                "rawvideo",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(decoded.status.success());
        let bytes = 320 * 180 * 4;
        assert_eq!(decoded.stdout.len(), pts.len() * bytes);
        let base = TimeBase::new(1, 15360).unwrap();
        let relative: Vec<_> = pts
            .iter()
            .map(|p| base.ticks_to_microseconds(p - pts[0]).unwrap())
            .collect();
        for requested in [0, 1, 400_000, 500_000, 777_777, 1_900_000] {
            let selected = relative.iter().position(|p| *p >= requested).unwrap();
            let start = relative
                .iter()
                .position(|p| *p >= (requested - 100_000).max(0))
                .unwrap();
            let mut seen = Vec::new();
            let summary = stream_history(
                &path,
                requested as u64,
                &CancellationToken::default(),
                |frame| {
                    let index = start + seen.len();
                    assert_eq!(frame.source_pts, pts[index]);
                    assert_eq!(frame.relative_us, relative[index]);
                    assert_eq!(
                        frame.rgba,
                        &decoded.stdout[index * bytes..(index + 1) * bytes]
                    );
                    assert_eq!(frame.selected, index == selected);
                    seen.push(frame.source_pts);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(summary.predecessors, selected - start);
            assert_eq!(summary.selected_us, relative[selected]);
            assert_eq!(summary.frame_bytes, bytes);
        }
    }

    #[test]
    fn history_cancellation_callback_error_eof_and_geometry_rejection() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m35-vfr-offset.mp4");
        let cancel = CancellationToken::default();
        let error = stream_history(&path, 500_000, &cancel, |_| {
            cancel.cancel();
            Ok(())
        })
        .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert!(
            stream_history(&path, 0, &CancellationToken::default(), |_| Err(
                "callback failed".into()
            ))
            .unwrap_err()
            .to_string()
            .contains("callback failed")
        );
        assert!(
            stream_history(
                &path,
                999_000_000,
                &CancellationToken::default(),
                |_| Ok(())
            )
            .is_err()
        );
        assert!(
            stream_history(
                &path.with_file_name("m35-geometry-color.mp4"),
                0,
                &CancellationToken::default(),
                |_| Ok(())
            )
            .unwrap_err()
            .to_string()
            .contains("square pixels")
        );
        // An aborted stream must not poison the next explicit request.
        assert_eq!(
            stream_history(&path, 0, &CancellationToken::default(), |_| Ok(()))
                .unwrap()
                .selected_us,
            0
        );
    }

    #[test]
    fn malformed_or_changed_decoder_metadata_is_rejected() {
        let line = "[Parsed_showinfo_4] n: 3 pts: 26880 pts_time:1.75 fmt:rgba s:320x180";
        let base = TimeBase::new(1, 15360).unwrap();
        let size = Size::new(320, 180).unwrap();
        assert_eq!(
            parse_frame(line, Some("1/15360"), base, size).unwrap(),
            (3, 26880)
        );
        assert!(parse_frame(line, Some("1/30"), base, size).is_err());
        assert!(parse_frame(line, Some("1/15360"), base, Size::new(640, 360).unwrap()).is_err());
        assert!(parse_frame(&line.replace("26880", "NOPTS"), Some("1/15360"), base, size).is_err());
    }

    #[test]
    fn high_rate_source_hits_predecessor_bound_without_retaining_pixels() {
        let path = std::env::temp_dir().join(format!(
            "diaxus-preview-history-{}-{}.mp4",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        struct Temporary(std::path::PathBuf);
        impl Drop for Temporary {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _temporary = Temporary(path.clone());
        // Tiny generated CC0 solid-color fixture; never adds user media to Git.
        let status = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=gray:s=32x32:r=1000",
                "-frames:v",
                "200",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
                "-color_primaries",
                "bt709",
                "-color_trc",
                "bt709",
                "-colorspace",
                "bt709",
                "-color_range",
                "tv",
            ])
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        let mut count = 0;
        let mut address = None;
        let error = stream_history(&path, 150_000, &CancellationToken::default(), |frame| {
            assert!(!frame.selected);
            if let Some(pointer) = address {
                assert_eq!(frame.rgba.as_ptr(), pointer);
            }
            address = Some(frame.rgba.as_ptr());
            count += 1;
            Ok(())
        })
        .unwrap_err();
        assert_eq!(count, 64);
        assert!(
            error.to_string().contains("64 predecessor limit"),
            "{error}"
        );
    }
}
