//! Private, sequential Matroska RGBA bridge to FFmpeg, not an output container.
//! One finite cluster per frame, microsecond ticks, no seeking or pixel buffering.
//! See https://www.matroska.org/technical/elements.html and codec_specs.html.
use crate::NativeResult;
use media_core::Size;
use std::io::Write;

fn size_bytes(size: u64) -> NativeResult<Vec<u8>> {
    for length in 1..=8 {
        let limit = (1_u64 << (7 * length)) - 1;
        // The all-ones payload is reserved for unknown size.
        if size < limit {
            let encoded = size | (1_u64 << (7 * length));
            return Ok(encoded.to_be_bytes()[8 - length..].to_vec());
        }
    }
    Err("Matroska element size overflow".into())
}

fn header(writer: &mut impl Write, id: &[u8], length: u64) -> NativeResult<()> {
    writer.write_all(id)?;
    writer.write_all(&size_bytes(length)?)?;
    Ok(())
}

fn element(writer: &mut impl Write, id: &[u8], data: &[u8]) -> NativeResult<()> {
    header(writer, id, u64::try_from(data.len())?)?;
    writer.write_all(data)?;
    Ok(())
}

fn integer(writer: &mut impl Write, id: &[u8], value: u64) -> NativeResult<()> {
    let bytes = value.to_be_bytes();
    let start = bytes.iter().position(|byte| *byte != 0).unwrap_or(7);
    element(writer, id, &bytes[start..])
}

pub(crate) fn start(
    writer: &mut impl Write,
    size: Size,
    nominal_rate: (u32, u32),
) -> NativeResult<()> {
    let (numerator, denominator) = nominal_rate;
    if numerator == 0 || denominator == 0 {
        return Err("RGBA bridge requires a nonzero inspected nominal frame rate".into());
    }
    let nominal_duration =
        (u64::from(denominator) * 1_000_000_000 + u64::from(numerator) / 2) / u64::from(numerator);
    if nominal_duration == 0 {
        return Err("RGBA bridge nominal frame duration is below one nanosecond".into());
    }
    let mut ebml = Vec::new();
    for (id, value) in [
        (&[0x42, 0x86][..], 1),
        (&[0x42, 0xf7], 1),
        (&[0x42, 0xf2], 4),
        (&[0x42, 0xf3], 8),
        (&[0x42, 0x87], 4),
        (&[0x42, 0x85], 2),
    ] {
        integer(&mut ebml, id, value)?;
    }
    element(&mut ebml, &[0x42, 0x82], b"matroska")?;
    element(writer, &[0x1a, 0x45, 0xdf, 0xa3], &ebml)?;
    // Streaming Segment: its size is intentionally unknown until EOF.
    writer.write_all(&[
        0x18, 0x53, 0x80, 0x67, 0x01, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ])?;
    let mut info = Vec::new();
    integer(&mut info, &[0x2a, 0xd7, 0xb1], 1_000)?;
    element(&mut info, &[0x4d, 0x80], b"Diaxus")?;
    element(&mut info, &[0x57, 0x41], b"Diaxus")?;
    element(writer, &[0x15, 0x49, 0xa9, 0x66], &info)?;
    let mut video = Vec::new();
    integer(&mut video, &[0xb0], u64::from(size.width))?;
    integer(&mut video, &[0xba], u64::from(size.height))?;
    element(&mut video, &[0x2e, 0xb5, 0x24], b"RGBA")?;
    let mut track = Vec::new();
    integer(&mut track, &[0xd7], 1)?;
    integer(&mut track, &[0x73, 0xc5], 1)?;
    integer(&mut track, &[0x83], 1)?;
    integer(&mut track, &[0x9c], 0)?; // no lacing
    // Rate-control hint only. Each cluster still carries its exact PTS and
    // BlockDuration. Without this FFmpeg infers 1,000,000fps from the timebase
    // and x264 average-bitrate mode can encode structured input as gray frames.
    integer(&mut track, &[0x23, 0xe3, 0x83], nominal_duration)?;
    element(&mut track, &[0x86], b"V_UNCOMPRESSED")?;
    element(&mut track, &[0xe0], &video)?;
    let mut tracks = Vec::new();
    element(&mut tracks, &[0xae], &track)?;
    element(writer, &[0x16, 0x54, 0xae, 0x6b], &tracks)
}

/// Writes only framing; the GPU readback writes exactly `pixel_bytes` next.
pub(crate) fn frame_header(
    writer: &mut impl Write,
    pts_us: i128,
    duration_us: i128,
    pixel_bytes: u64,
) -> NativeResult<()> {
    let mut timestamp = Vec::new();
    integer(&mut timestamp, &[0xe7], u64::try_from(pts_us)?)?;
    let mut duration = Vec::new();
    integer(&mut duration, &[0x9b], u64::try_from(duration_us)?)?;
    let block_size = pixel_bytes.checked_add(4).ok_or("frame size overflow")?;
    let group_size = u64::try_from(duration.len())?
        + 1
        + u64::try_from(size_bytes(block_size)?.len())?
        + block_size;
    let cluster_size = u64::try_from(timestamp.len())?
        + 1
        + u64::try_from(size_bytes(group_size)?.len())?
        + group_size;
    header(writer, &[0x1f, 0x43, 0xb6, 0x75], cluster_size)?;
    writer.write_all(&timestamp)?;
    header(writer, &[0xa0], group_size)?;
    writer.write_all(&duration)?;
    header(writer, &[0xa1], block_size)?;
    writer.write_all(&[0x81, 0, 0, 0])?; // track 1, relative PTS 0, no lacing
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn average_bitrate_encoder_keeps_first_image_and_irregular_timestamps() {
        use std::process::{Command, Stdio};
        fn pipe(mut command: Command, bytes: Vec<u8>) -> std::process::Output {
            let mut child = command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let mut input = child.stdin.take().unwrap();
            let writer = std::thread::spawn(move || input.write_all(&bytes));
            let result = child.wait_with_output().unwrap();
            writer.join().unwrap().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            result
        }
        let size = Size::new(1280, 536).unwrap();
        let pixels: Vec<u8> = (0..size.height)
            .flat_map(|y| {
                (0..size.width).flat_map(move |x| {
                    [(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255]
                })
            })
            .collect();
        let mut stream = vec![];
        start(&mut stream, size, (12, 1)).unwrap();
        for (pts, duration) in [(0, 66667), (66667, 166666), (233333, 100000)] {
            frame_header(&mut stream, pts, duration, pixels.len() as u64).unwrap();
            stream.extend_from_slice(&pixels);
        }
        let mut encoder = Command::new("ffmpeg");
        encoder.args([
            "-v",
            "error",
            "-copyts",
            "-f",
            "matroska",
            "-i",
            "pipe:0",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-pix_fmt",
            "yuv420p",
            "-b:v",
            "4000000",
            "-fps_mode",
            "passthrough",
            "-enc_time_base:v",
            "1:90000",
            "-movflags",
            "frag_keyframe+empty_moov",
            "-f",
            "mp4",
            "pipe:1",
        ]);
        let encoded = pipe(encoder, stream).stdout;
        let mut probe = Command::new("ffprobe");
        probe.args([
            "-v",
            "error",
            "-show_frames",
            "-show_entries",
            "frame=pts_time",
            "-of",
            "json",
            "pipe:0",
        ]);
        let timeline: serde_json::Value =
            serde_json::from_slice(&pipe(probe, encoded.clone()).stdout).unwrap();
        let frames = timeline["frames"].as_array().unwrap();
        assert_eq!(frames.len(), 3);
        // Fragmented MP4 may shift presentation origin by its B-frame delay;
        // compare relative PTS, preserving both unequal source intervals.
        let pts: Vec<f64> = frames
            .iter()
            .map(|f| f["pts_time"].as_str().unwrap().parse().unwrap())
            .collect();
        for (actual, expected) in pts.iter().zip([0.0, 0.066667, 0.233333]) {
            assert!((actual - pts[0] - expected).abs() < 0.000012);
        }
        let mut decoder = Command::new("ffmpeg");
        decoder.args([
            "-v",
            "error",
            "-i",
            "pipe:0",
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ]);
        let first = pipe(decoder, encoded).stdout;
        assert_eq!(first.len(), pixels.len());
        let red: Vec<u8> = first.as_chunks::<4>().0.iter().map(|p| p[0]).collect();
        assert!(
            red.iter().max().unwrap() - red.iter().min().unwrap() > 128,
            "Average-bitrate encode replaced structured first frame with gray"
        );
    }
    #[test]
    fn larger_rgba_blocks_decode_exactly_before_encoding() {
        use std::process::{Command, Stdio};
        let size = Size::new(1280, 536).unwrap();
        let pixels: Vec<u8> = (0..size.height)
            .flat_map(|y| {
                (0..size.width).flat_map(move |x| {
                    [(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255]
                })
            })
            .collect();
        let mut child = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "matroska",
                "-i",
                "pipe:0",
                "-map",
                "0:v:0",
                "-fps_mode",
                "passthrough",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "pipe:1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let sent = pixels.clone();
        let writer = std::thread::spawn(move || {
            start(&mut stdin, size, (12, 1)).unwrap();
            for i in 0..3 {
                frame_header(&mut stdin, i * 83333, 83333, sent.len() as u64).unwrap();
                stdin.write_all(&sent).unwrap();
            }
        });
        let decoded = child.wait_with_output().unwrap();
        writer.join().unwrap();
        assert!(
            decoded.status.success(),
            "{}",
            String::from_utf8_lossy(&decoded.stderr)
        );
        assert_eq!(decoded.stdout.len(), pixels.len() * 3);
        for frame in decoded.stdout.chunks_exact(pixels.len()) {
            assert_eq!(frame, pixels);
        }
    }
    #[test]
    fn ebml_sizes_exclude_reserved_unknown_values() {
        assert_eq!(size_bytes(126).unwrap(), [0xfe]);
        assert_eq!(size_bytes(127).unwrap(), [0x40, 0x7f]);
        assert_eq!(size_bytes(16382).unwrap(), [0x7f, 0xfe]);
        assert_eq!(size_bytes(16383).unwrap(), [0x20, 0x3f, 0xff]);
        assert!(size_bytes(u64::MAX).is_err());
    }
    #[test]
    fn rejects_negative_timing() {
        assert!(frame_header(&mut Vec::new(), -1, 1, 16).is_err());
        assert!(frame_header(&mut Vec::new(), 0, -1, 16).is_err());
    }
}
