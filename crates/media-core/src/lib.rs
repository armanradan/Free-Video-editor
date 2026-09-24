#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;

pub const INPUT_SIZE: Size = Size::new_unchecked(320, 180);
pub const OUTPUT_SIZE: Size = Size::new_unchecked(160, 90);
pub const FRAME_COUNT: u32 = 30;
pub const FRAME_DURATION_US: i64 = 33_333;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputProfileId {
    WebmVp8Opus,
    WebmVp8VideoOnly,
    Mp4H264Aac,
    Mp4H265Aac,
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
            Self::Mp4H265Aac => "mp4-h265-aac",
        }
    }

    pub const fn audio_policy(self) -> AudioPolicy {
        match self {
            Self::WebmVp8Opus => AudioPolicy::Transcode(AudioCodec::Opus),
            Self::WebmVp8VideoOnly => AudioPolicy::Omit,
            Self::Mp4H264Aac | Self::Mp4H265Aac => AudioPolicy::Transcode(AudioCodec::Aac),
        }
    }

    pub const fn container(self) -> ContainerFormat {
        match self {
            Self::WebmVp8Opus | Self::WebmVp8VideoOnly => ContainerFormat::WebM,
            Self::Mp4H264Aac | Self::Mp4H265Aac => ContainerFormat::Mp4,
        }
    }

    pub const fn video_codec(self) -> VideoCodec {
        match self {
            Self::WebmVp8Opus | Self::WebmVp8VideoOnly => VideoCodec::Vp8,
            Self::Mp4H264Aac => VideoCodec::H264,
            Self::Mp4H265Aac => VideoCodec::H265,
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
    H265,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Result<Self, MediaError> {
        Size::new(width, height)?;
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    pub fn fits_within(self, coded: Size) -> bool {
        self.x
            .checked_add(self.width)
            .is_some_and(|right| right <= coded.width)
            && self
                .y
                .checked_add(self.height)
                .is_some_and(|bottom| bottom <= coded.height)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u32)]
pub enum Rotation {
    #[default]
    Deg0 = 0,
    Deg90 = 1,
    Deg180 = 2,
    Deg270 = 3,
}

impl Rotation {
    pub fn from_degrees(value: u32) -> Result<Self, MediaError> {
        match value {
            0 => Ok(Self::Deg0),
            90 => Ok(Self::Deg90),
            180 => Ok(Self::Deg180),
            270 => Ok(Self::Deg270),
            _ => Err(MediaError::InvalidRotation(value)),
        }
    }

    pub const fn degrees(self) -> u32 {
        match self {
            Self::Deg0 => 0,
            Self::Deg90 => 90,
            Self::Deg180 => 180,
            Self::Deg270 => 270,
        }
    }

    pub const fn swaps_axes(self) -> bool {
        matches!(self, Self::Deg90 | Self::Deg270)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrameGeometry {
    pub coded: Size,
    pub visible: Rect,
    /// Browser-normalized square-pixel size before rotation and flip.
    pub square_pixel: Size,
    pub rotation: Rotation,
    pub flip_horizontal: bool,
}

impl FrameGeometry {
    pub fn new(
        coded: Size,
        visible: Rect,
        square_pixel: Size,
        rotation: Rotation,
        flip_horizontal: bool,
    ) -> Result<Self, MediaError> {
        if !visible.fits_within(coded) {
            return Err(MediaError::VisibleRectOutsideCoded { visible, coded });
        }
        Ok(Self {
            coded,
            visible,
            square_pixel,
            rotation,
            flip_horizontal,
        })
    }

    pub const fn display_size(self) -> Size {
        if self.rotation.swaps_axes() {
            Size::new_unchecked(self.square_pixel.height, self.square_pixel.width)
        } else {
            self.square_pixel
        }
    }
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
pub enum ResizeSpec {
    Original,
    Percent(u16),
    Exact {
        width: u32,
        height: u32,
        preserve_aspect_ratio: bool,
    },
}

impl ResizeSpec {
    pub const DEFAULT: Self = Self::Percent(50);
    pub const HD_720P: Self = Self::Exact {
        width: 1_280,
        height: 720,
        preserve_aspect_ratio: true,
    };
    pub const FULL_HD_1080P: Self = Self::Exact {
        width: 1_920,
        height: 1_080,
        preserve_aspect_ratio: true,
    };
    pub const DCI_2K: Self = Self::Exact {
        width: 2_048,
        height: 2_160,
        preserve_aspect_ratio: true,
    };
    pub const QHD_1440P: Self = Self::Exact {
        width: 2_560,
        height: 1_440,
        preserve_aspect_ratio: true,
    };
    pub const UHD_2160P: Self = Self::Exact {
        width: 3_840,
        height: 2_160,
        preserve_aspect_ratio: true,
    };

    pub fn output_size(self, input: Size) -> Result<Size, MediaError> {
        let requested = match self {
            Self::Original => input,
            Self::Percent(percent) => {
                if percent == 0 || percent > 100 {
                    return Err(MediaError::InvalidResizePercent(percent));
                }
                Size::new_unchecked(
                    scale_dimension(input.width, percent)?,
                    scale_dimension(input.height, percent)?,
                )
            }
            Self::Exact {
                width,
                height,
                preserve_aspect_ratio,
            } => {
                let bounds = Size::new(width, height)?;
                let resolved = if preserve_aspect_ratio {
                    fit_within(input, bounds)?
                } else {
                    bounds
                };
                if resolved.width > input.width || resolved.height > input.height {
                    return Err(MediaError::ResizeWouldUpscale {
                        input,
                        requested: resolved,
                    });
                }
                resolved
            }
        };
        codec_size(requested)
    }
}

fn scale_dimension(value: u32, percent: u16) -> Result<u32, MediaError> {
    u32::try_from(
        u64::from(value)
            .checked_mul(u64::from(percent))
            .ok_or(MediaError::ResizeOverflow)?
            / 100,
    )
    .map_err(|_| MediaError::ResizeOverflow)
}

fn fit_within(input: Size, bounds: Size) -> Result<Size, MediaError> {
    let width_limited = u64::from(bounds.width)
        .checked_mul(u64::from(input.height))
        .ok_or(MediaError::ResizeOverflow)?
        <= u64::from(bounds.height)
            .checked_mul(u64::from(input.width))
            .ok_or(MediaError::ResizeOverflow)?;
    if width_limited {
        Size::new(
            bounds.width,
            u32::try_from(
                u64::from(input.height)
                    .checked_mul(u64::from(bounds.width))
                    .ok_or(MediaError::ResizeOverflow)?
                    / u64::from(input.width),
            )
            .map_err(|_| MediaError::ResizeOverflow)?,
        )
    } else {
        Size::new(
            u32::try_from(
                u64::from(input.width)
                    .checked_mul(u64::from(bounds.height))
                    .ok_or(MediaError::ResizeOverflow)?
                    / u64::from(input.height),
            )
            .map_err(|_| MediaError::ResizeOverflow)?,
            bounds.height,
        )
    }
}

fn codec_size(value: Size) -> Result<Size, MediaError> {
    let width = value.width & !1;
    let height = value.height & !1;
    if width < 2 || height < 2 {
        Err(MediaError::ResizeTooSmall {
            width: value.width,
            height: value.height,
        })
    } else {
        Size::new(width, height)
    }
}

pub fn validate_input_size(bytes: u64) -> Result<(), MediaError> {
    if bytes == 0 {
        Err(MediaError::EmptyInput)
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
    InvalidRotation(u32),
    VisibleRectOutsideCoded { visible: Rect, coded: Size },
    InvalidResizePercent(u16),
    ResizeWouldUpscale { input: Size, requested: Size },
    ResizeTooSmall { width: u32, height: u32 },
    ResizeOverflow,
    TimestampOverflow,
    EmptyInput,
    Platform(String),
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize { width, height } => write!(f, "invalid frame size {width}x{height}"),
            Self::InvalidTimeBase => f.write_str("time-base terms must both be non-zero"),
            Self::InvalidRotation(value) => write!(f, "invalid frame rotation {value} degrees"),
            Self::VisibleRectOutsideCoded { visible, coded } => write!(
                f,
                "visible rectangle {}x{}+{},{} exceeds coded frame {}x{}",
                visible.width, visible.height, visible.x, visible.y, coded.width, coded.height
            ),
            Self::InvalidResizePercent(percent) => {
                write!(
                    f,
                    "resize percentage must be between 1 and 100, got {percent}"
                )
            }
            Self::ResizeWouldUpscale { input, requested } => write!(
                f,
                "requested output {}x{} would upscale the oriented source {}x{}",
                requested.width, requested.height, input.width, input.height
            ),
            Self::ResizeTooSmall { width, height } => write!(
                f,
                "requested output {width}x{height} is smaller than the minimum codec size 2x2"
            ),
            Self::ResizeOverflow => f.write_str("resize calculation overflowed"),
            Self::TimestampOverflow => f.write_str("timestamp conversion overflowed"),
            Self::EmptyInput => f.write_str("the selected file is empty"),
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
    fn percentage_resize_produces_even_codec_dimensions() {
        assert_eq!(
            ResizeSpec::Percent(50)
                .output_size(Size::new(1_921, 1_081).unwrap())
                .unwrap(),
            Size::new(960, 540).unwrap()
        );
        assert!(matches!(
            ResizeSpec::Percent(50).output_size(Size::new(1, 1).unwrap()),
            Err(MediaError::ResizeTooSmall { .. })
        ));
    }

    #[test]
    fn original_and_percentage_presets_never_upscale() {
        let input = Size::new(1_921, 1_081).unwrap();
        assert_eq!(
            ResizeSpec::Original.output_size(input).unwrap(),
            Size::new(1_920, 1_080).unwrap()
        );
        assert_eq!(
            ResizeSpec::Percent(75).output_size(input).unwrap(),
            Size::new(1_440, 810).unwrap()
        );
        assert!(matches!(
            ResizeSpec::Percent(101).output_size(input),
            Err(MediaError::InvalidResizePercent(101))
        ));
    }

    #[test]
    fn exact_locked_size_fits_and_preserves_display_aspect() {
        let input = Size::new(1_920, 1_080).unwrap();
        assert_eq!(
            ResizeSpec::Exact {
                width: 1_000,
                height: 1_000,
                preserve_aspect_ratio: true,
            }
            .output_size(input)
            .unwrap(),
            Size::new(1_000, 562).unwrap()
        );
        assert_eq!(
            ResizeSpec::Exact {
                width: 1_000,
                height: 700,
                preserve_aspect_ratio: false,
            }
            .output_size(input)
            .unwrap(),
            Size::new(1_000, 700).unwrap()
        );
    }

    #[test]
    fn exact_size_rejects_implicit_upscale() {
        let input = Size::new(640, 360).unwrap();
        assert!(matches!(
            ResizeSpec::Exact {
                width: 800,
                height: 500,
                preserve_aspect_ratio: true,
            }
            .output_size(input),
            Err(MediaError::ResizeWouldUpscale { .. })
        ));
    }

    #[test]
    fn named_resolution_presets_resolve_deterministically_for_uhd_input() {
        let input = Size::new(3_840, 2_160).unwrap();
        assert_eq!(
            ResizeSpec::HD_720P.output_size(input).unwrap(),
            Size::new(1_280, 720).unwrap()
        );
        assert_eq!(
            ResizeSpec::FULL_HD_1080P.output_size(input).unwrap(),
            Size::new(1_920, 1_080).unwrap()
        );
        assert_eq!(
            ResizeSpec::QHD_1440P.output_size(input).unwrap(),
            Size::new(2_560, 1_440).unwrap()
        );
        assert_eq!(
            ResizeSpec::DCI_2K.output_size(input).unwrap(),
            Size::new(2_048, 1_152).unwrap()
        );
        assert_eq!(ResizeSpec::UHD_2160P.output_size(input).unwrap(), input);
    }

    #[test]
    fn geometry_applies_crop_aspect_then_orientation_before_resize() {
        let geometry = FrameGeometry::new(
            Size::new(336, 192).unwrap(),
            Rect::new(8, 6, 320, 180).unwrap(),
            Size::new(426, 180).unwrap(),
            Rotation::Deg90,
            true,
        )
        .unwrap();
        assert_eq!(geometry.display_size(), Size::new(180, 426).unwrap());
        assert_eq!(
            ResizeSpec::Percent(50)
                .output_size(geometry.display_size())
                .unwrap(),
            Size::new(90, 212).unwrap()
        );
    }

    #[test]
    fn geometry_rejects_visible_rect_outside_coded_frame() {
        let coded = Size::new(320, 180).unwrap();
        let visible = Rect::new(4, 0, 320, 180).unwrap();
        assert!(matches!(
            FrameGeometry::new(coded, visible, coded, Rotation::Deg0, false),
            Err(MediaError::VisibleRectOutsideCoded { .. })
        ));
    }

    #[test]
    fn input_must_not_be_empty_but_has_no_policy_size_cap() {
        assert!(validate_input_size(u64::MAX).is_ok());
        assert_eq!(validate_input_size(0), Err(MediaError::EmptyInput));
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
        assert_eq!(
            OutputProfileId::Mp4H265Aac.audio_policy(),
            AudioPolicy::Transcode(AudioCodec::Aac)
        );
        assert_eq!(
            OutputProfileId::Mp4H265Aac.container(),
            ContainerFormat::Mp4
        );
        assert_eq!(OutputProfileId::Mp4H265Aac.video_codec(), VideoCodec::H265);
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
