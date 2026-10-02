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

pub(crate) fn start(writer: &mut impl Write, size: Size) -> NativeResult<()> {
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
