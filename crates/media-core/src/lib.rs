#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;

pub const INPUT_SIZE: Size = Size::new_unchecked(320, 180);
pub const OUTPUT_SIZE: Size = Size::new_unchecked(160, 90);
pub const FRAME_COUNT: u32 = 30;
pub const FRAME_DURATION_US: i64 = 33_333;
pub const MAX_BROWSER_INPUT_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputProfileId {
    WebmVp8Opus,
    WebmVp8VideoOnly,
    Mp4H264Aac,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodecAcceleration {
    #[default]
    NoPreference,
    PreferHardware,
}

impl CodecAcceleration {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoPreference => "no-preference",
            Self::PreferHardware => "prefer-hardware",
        }
    }
}

impl OutputProfileId {
    pub const PREFERRED: Self = Self::WebmVp8Opus;

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WebmVp8Opus => "webm-vp8-opus",
            Self::WebmVp8VideoOnly => "webm-vp8-video-only",
            Self::Mp4H264Aac => "mp4-h264-aac",
        }
    }

    pub const fn audio_policy(self) -> AudioPolicy {
        match self {
            Self::WebmVp8Opus => AudioPolicy::Transcode(AudioCodec::Opus),
            Self::WebmVp8VideoOnly => AudioPolicy::Omit,
            Self::Mp4H264Aac => AudioPolicy::Transcode(AudioCodec::Aac),
        }
    }

    pub const fn container(self) -> ContainerFormat {
        match self {
            Self::WebmVp8Opus | Self::WebmVp8VideoOnly => ContainerFormat::WebM,
            Self::Mp4H264Aac => ContainerFormat::Mp4,
        }
    }

    pub const fn video_codec(self) -> VideoCodec {
        match self {
            Self::WebmVp8Opus | Self::WebmVp8VideoOnly => VideoCodec::Vp8,
            Self::Mp4H264Aac => VideoCodec::H264,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContainerFormat {
    WebM,
    Mp4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum VideoCodec {
    Vp8,
    H264,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AudioCodec {
    Opus,
    Aac,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AudioPolicy {
    Omit,
    Transcode(AudioCodec),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Capability {
    Supported,
    Unsupported { reason: String },
}

impl Capability {
    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self::Unsupported {
            reason: reason.into(),
        }
    }
}

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

    #[test]
    fn preferred_profile_preserves_audio_by_transcoding_to_opus() {
        assert_eq!(OutputProfileId::PREFERRED, OutputProfileId::WebmVp8Opus);
        assert_eq!(
            OutputProfileId::PREFERRED.audio_policy(),
            AudioPolicy::Transcode(AudioCodec::Opus)
        );
        assert_eq!(
            OutputProfileId::WebmVp8VideoOnly.audio_policy(),
            AudioPolicy::Omit
        );
        assert_eq!(
            OutputProfileId::Mp4H264Aac.audio_policy(),
            AudioPolicy::Transcode(AudioCodec::Aac)
        );
        assert_eq!(
            OutputProfileId::Mp4H264Aac.container(),
            ContainerFormat::Mp4
        );
        assert_eq!(OutputProfileId::Mp4H264Aac.video_codec(), VideoCodec::H264);
    }

    #[test]
    fn unsupported_capability_retains_the_exact_reason() {
        assert_eq!(
            Capability::unsupported("Opus encoder unavailable"),
            Capability::Unsupported {
                reason: "Opus encoder unavailable".to_string()
            }
        );
    }

    #[test]
    fn codec_acceleration_defaults_to_compatibility_baseline() {
        assert_eq!(
            CodecAcceleration::default(),
            CodecAcceleration::NoPreference
        );
        assert_eq!(CodecAcceleration::NoPreference.as_str(), "no-preference");
        assert_eq!(
            CodecAcceleration::PreferHardware.as_str(),
            "prefer-hardware"
        );
    }
}
