//! Native codec ingress policy: square pixels at unchanged coded height.
use crate::NativeResult;
use media_core::Size;

pub(crate) fn has_display_transform(video: &serde_json::Value) -> bool {
    video["side_data_list"].as_array().is_some_and(|sides| {
        sides.iter().any(|side| {
            side["side_data_type"] == "Display Matrix" || side["rotation"].as_i64().is_some()
        })
    })
}

pub(crate) fn metadata_size(video: &serde_json::Value) -> NativeResult<Size> {
    let coded = Size::new(
        u32::try_from(video["width"].as_u64().ok_or("Missing coded width")?)?,
        u32::try_from(video["height"].as_u64().ok_or("Missing coded height")?)?,
    )?;
    let ratio = video["sample_aspect_ratio"].as_str().unwrap_or("1:1");
    let (num, den) = if ratio == "N/A" {
        (1, 1)
    } else {
        let (num, den) = ratio.split_once(':').ok_or("Invalid sample aspect ratio")?;
        (num.parse()?, den.parse()?)
    };
    square_pixel_size(coded, num, den)
}

/// Explicit CPU decoder ingress normalization, before shared GPU processing.
pub(crate) fn normalization_filter(coded: Size, normalized: Size) -> String {
    if coded == normalized {
        String::new()
    } else {
        format!(
            "scale={}:{}:flags=bilinear,setsar=1,",
            normalized.width, normalized.height
        )
    }
}

pub(crate) fn square_pixel_size(
    coded: Size,
    numerator: i128,
    denominator: i128,
) -> NativeResult<Size> {
    if numerator <= 0 || denominator <= 0 {
        return Err("Sample aspect ratio must be positive".into());
    }
    if numerator == denominator {
        return Ok(coded);
    }
    // Round display width to a multiple of two; half ties choose the larger
    // even width. Do not round once to an integer and then round again.
    let divisor = denominator
        .checked_mul(2)
        .ok_or("Display geometry overflow")?;
    let units = i128::from(coded.width)
        .checked_mul(numerator)
        .and_then(|width| width.checked_add(denominator))
        .ok_or("Display geometry overflow")?
        / divisor;
    let width = units.checked_mul(2).ok_or("Display geometry overflow")?;
    Ok(Size::new(u32::try_from(width)?, coded.height)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rational_display_width_rounds_once_to_even_and_preserves_square_pixels() {
        assert_eq!(
            square_pixel_size(Size::new(352, 288).unwrap(), 178, 163).unwrap(),
            Size::new(384, 288).unwrap()
        );
        assert_eq!(
            square_pixel_size(Size::new(3, 5).unwrap(), 2, 2).unwrap(),
            Size::new(3, 5).unwrap()
        );
        for (num, den, expected) in [(3, 2, 4), (6, 5, 2), (9, 5, 4)] {
            assert_eq!(
                square_pixel_size(Size::new(2, 8).unwrap(), num, den)
                    .unwrap()
                    .width,
                expected
            );
        }
        let coded = Size::new(2, 8).unwrap();
        for (num, den) in [
            (0, 1),
            (1, 0),
            (-1, 2),
            (1, -2),
            (i128::MAX, 1),
            (1, i128::MAX),
            (1, 10),
            (i128::from(u32::MAX), 1),
        ] {
            assert!(square_pixel_size(coded, num, den).is_err());
        }
    }
}
