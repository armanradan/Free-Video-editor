#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;

pub const INPUT_SIZE: Size = Size::new_unchecked(320, 180);
pub const OUTPUT_SIZE: Size = Size::new_unchecked(160, 90);
pub const FRAME_COUNT: u32 = 30;
pub const FRAME_DURATION_US: i64 = 33_333;
pub const MAX_BROWSER_INPUT_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub const fn new_unchecked(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn new(width: u32, height: u32) -> Result<Self, MediaError> {
        if width == 0 || height == 0 {
            Err(MediaError::InvalidSize { width, height })
        } else {
            Ok(Self { width, height })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ResizePreset {
    Half,
}

impl ResizePreset {
    pub fn output_size(self, input: Size) -> Result<Size, MediaError> {
        match self {
            Self::Half => {
                let even_half = |value: u32| ((value / 2).max(2)) & !1;
                Size::new(even_half(input.width), even_half(input.height))
            }
        }
    }
}

pub fn validate_browser_input_size(bytes: u64) -> Result<(), MediaError> {
    if bytes == 0 {
        Err(MediaError::EmptyInput)
    } else if bytes > MAX_BROWSER_INPUT_BYTES {
        Err(MediaError::InputTooLarge {
            actual: bytes,
            maximum: MAX_BROWSER_INPUT_BYTES,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TimeBase {
    pub numerator: u32,
    pub denominator: u32,
}

impl TimeBase {
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, MediaError> {
        if numerator == 0 || denominator == 0 {
            Err(MediaError::InvalidTimeBase)
        } else {
            Ok(Self {
                numerator,
                denominator,
            })
        }
    }

    pub fn ticks_to_microseconds(self, ticks: i64) -> Result<i64, MediaError> {
        let scaled = i128::from(ticks)
            .checked_mul(i128::from(self.numerator))
            .and_then(|v| v.checked_mul(1_000_000))
            .ok_or(MediaError::TimestampOverflow)?;
        let denominator = i128::from(self.denominator);
        let rounded = if scaled >= 0 {
            (scaled + denominator / 2) / denominator
        } else {
            (scaled - denominator / 2) / denominator
        };
        i64::try_from(rounded).map_err(|_| MediaError::TimestampOverflow)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MediaError {
    InvalidSize { width: u32, height: u32 },
    InvalidTimeBase,
    TimestampOverflow,
    EmptyInput,
    InputTooLarge { actual: u64, maximum: u64 },
    Platform(String),
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize { width, height } => write!(f, "invalid frame size {width}x{height}"),
            Self::InvalidTimeBase => f.write_str("time-base terms must both be non-zero"),
            Self::TimestampOverflow => f.write_str("timestamp conversion overflowed"),
            Self::EmptyInput => f.write_str("the selected file is empty"),
            Self::InputTooLarge { actual, maximum } => write!(
                f,
                "selected file is {actual} bytes; M2 allows at most {maximum} bytes"
            ),
            Self::Platform(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for MediaError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_ntsc_ticks_with_nearest_rounding() {
        let base = TimeBase::new(1, 30_000).unwrap();
        assert_eq!(base.ticks_to_microseconds(1_001).unwrap(), 33_367);
        assert_eq!(base.ticks_to_microseconds(-1_001).unwrap(), -33_367);
    }

    #[test]
    fn rejects_overflow() {
        let base = TimeBase::new(u32::MAX, 1).unwrap();
        assert_eq!(
            base.ticks_to_microseconds(i64::MAX),
            Err(MediaError::TimestampOverflow)
        );
    }

    #[test]
    fn half_resize_produces_even_codec_dimensions() {
        assert_eq!(
            ResizePreset::Half
                .output_size(Size::new(1_921, 1_081).unwrap())
                .unwrap(),
            Size::new(960, 540).unwrap()
        );
        assert_eq!(
            ResizePreset::Half
                .output_size(Size::new(1, 1).unwrap())
                .unwrap(),
            Size::new(2, 2).unwrap()
        );
    }

    #[test]
    fn browser_input_limit_is_explicit() {
        assert!(validate_browser_input_size(MAX_BROWSER_INPUT_BYTES).is_ok());
        assert_eq!(
            validate_browser_input_size(MAX_BROWSER_INPUT_BYTES + 1),
            Err(MediaError::InputTooLarge {
                actual: MAX_BROWSER_INPUT_BYTES + 1,
                maximum: MAX_BROWSER_INPUT_BYTES,
            })
        );
    }
}
