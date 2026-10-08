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
    Mp4H265Main10Aac,
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
            Self::Mp4H265Main10Aac => "mp4-h265-main10-aac",
        }
    }

    pub const fn audio_policy(self) -> AudioPolicy {
        match self {
            Self::WebmVp8Opus => AudioPolicy::Transcode(AudioCodec::Opus),
            Self::WebmVp8VideoOnly => AudioPolicy::Omit,
            Self::Mp4H264Aac | Self::Mp4H265Aac | Self::Mp4H265Main10Aac => {
                AudioPolicy::Transcode(AudioCodec::Aac)
            }
        }
    }

    pub const fn container(self) -> ContainerFormat {
        match self {
            Self::WebmVp8Opus | Self::WebmVp8VideoOnly => ContainerFormat::WebM,
            Self::Mp4H264Aac | Self::Mp4H265Aac | Self::Mp4H265Main10Aac => ContainerFormat::Mp4,
        }
    }

    pub const fn video_codec(self) -> VideoCodec {
        match self {
            Self::WebmVp8Opus | Self::WebmVp8VideoOnly => VideoCodec::Vp8,
            Self::Mp4H264Aac => VideoCodec::H264,
            Self::Mp4H265Aac | Self::Mp4H265Main10Aac => VideoCodec::H265,
        }
    }

    pub const fn video_bit_depth(self) -> u8 {
        match self {
            Self::Mp4H265Main10Aac => 10,
            _ => 8,
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

/// Average video bitrate policy, independent of encoder/platform APIs.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum VideoBitrate {
    Smaller,
    #[default]
    Recommended,
    Higher,
    BitsPerSecond(u32),
}

impl VideoBitrate {
    pub fn from_mbps(value: &str) -> Result<Self, MediaError> {
        let mbps = value
            .trim()
            .parse::<f64>()
            .map_err(|_| MediaError::InvalidBitrateInput)?;
        if !mbps.is_finite() || !(0.25..=120.0).contains(&mbps) {
            return Err(MediaError::InvalidBitrateInput);
        }
        Ok(Self::BitsPerSecond((mbps * 1_000_000.0).round() as u32))
    }

    pub fn resolve(
        self,
        size: Size,
        fps_num: u32,
        fps_den: u32,
        codec: VideoCodec,
    ) -> Result<u32, MediaError> {
        Size::new(size.width, size.height)?;
        if fps_num == 0 || fps_den == 0 {
            return Err(MediaError::InvalidTimeBase);
        }
        let bitrate = match self {
            Self::BitsPerSecond(value) => value,
            Self::Smaller | Self::Recommended | Self::Higher => {
                // A starting heuristic, not a perceptual-quality guarantee:
                // H.264/VP8 0.10 bits/pixel/frame, HEVC 0.07.
                let factor = if codec == VideoCodec::H265 {
                    7_u128
                } else {
                    10
                };
                let numerator =
                    u128::from(size.width) * u128::from(size.height) * u128::from(fps_num) * factor;
                let denominator = u128::from(fps_den) * 100;
                let base = numerator.div_ceil(denominator);
                let (num, den) = match self {
                    Self::Smaller => (3, 5),
                    Self::Higher => (3, 2),
                    _ => (1, 1),
                };
                (base * num / den).clamp(250_000, 120_000_000) as u32
            }
        };
        if !(250_000..=120_000_000).contains(&bitrate) {
            return Err(MediaError::InvalidBitrate(bitrate));
        }
        Ok(bitrate)
    }

    /// Adapt to the source video rate when known; never use container/audio bitrate.
    pub fn resolve_with_source(
        self,
        size: Size,
        fps_num: u32,
        fps_den: u32,
        codec: VideoCodec,
        source: BitrateSource,
    ) -> Result<u32, MediaError> {
        if matches!(self, Self::BitsPerSecond(_)) {
            return self.resolve(size, fps_num, fps_den, codec);
        }
        let heuristic = Self::Recommended.resolve(size, fps_num, fps_den, codec)?;
        Size::new(source.size.width, source.size.height)?;
        if source.fps_num == 0 || source.fps_den == 0 {
            return Err(MediaError::InvalidTimeBase);
        }
        let factor = |codec| {
            if codec == VideoCodec::H265 {
                7_u128
            } else {
                10
            }
        };
        let scaled_source = source
            .bitrate_bps
            .filter(|rate| *rate > 0)
            .and_then(|rate| {
                // Downscaling does not reduce detail/codec overhead in direct
                // proportion to pixel count. Scale by the geometric mean of
                // width/height ratios (sqrt of area ratio), retaining source
                // complexity instead of capping it at the fallback heuristic.
                // Fixed-point integer arithmetic keeps both hosts identical.
                let area_scale_ppm =
                    (u128::from(size.width) * u128::from(size.height) * 1_000_000_000_000
                        / (u128::from(source.size.width) * u128::from(source.size.height)))
                    .isqrt();
                let numerator = u128::from(rate)
                    .checked_mul(area_scale_ppm)?
                    .checked_mul(u128::from(fps_num))?
                    .checked_mul(u128::from(source.fps_den))?
                    .checked_mul(factor(codec))?;
                let denominator = 1_000_000_u128
                    .checked_mul(u128::from(source.fps_num))?
                    .checked_mul(u128::from(fps_den))?
                    .checked_mul(factor(source.codec))?;
                Some(numerator.div_ceil(denominator).clamp(250_000, 120_000_000) as u32)
            });
        let base = scaled_source.unwrap_or(heuristic);
        let (num, den) = match self {
            Self::Smaller => (3_u64, 5),
            Self::Higher => (3, 2),
            _ => (1, 1),
        };
        Ok((u64::from(base) * num / den).clamp(250_000, 120_000_000) as u32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BitrateSource {
    pub size: Size,
    pub fps_num: u32,
    pub fps_den: u32,
    pub codec: VideoCodec,
    pub bitrate_bps: Option<u64>,
}

/// Original retains every PTS (including VFR). Constant explicitly resamples.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum FrameRateSpec {
    #[default]
    Original,
    Constant {
        numerator: u32,
        denominator: u32,
    },
}

impl std::str::FromStr for FrameRateSpec {
    type Err = MediaError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        if value == "original" {
            return Ok(Self::Original);
        }
        let (num, den) = value.split_once('/').unwrap_or((value, "1"));
        let rate = Self::Constant {
            numerator: num.parse().map_err(|_| MediaError::InvalidFrameRate)?,
            denominator: den.parse().map_err(|_| MediaError::InvalidFrameRate)?,
        };
        rate.resolve(1, 1)?;
        Ok(rate)
    }
}

/// Validated display-oriented SDR controls in integer UI units.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ColorValues")]
pub struct ColorAdjustments {
    brightness: i16,
    contrast: u16,
    saturation: u16,
}

#[derive(Deserialize)]
struct ColorValues {
    brightness: i16,
    contrast: u16,
    saturation: u16,
}

impl TryFrom<ColorValues> for ColorAdjustments {
    type Error = MediaError;
    fn try_from(value: ColorValues) -> Result<Self, Self::Error> {
        Self::new(value.brightness, value.contrast, value.saturation)
    }
}

impl Default for ColorAdjustments {
    fn default() -> Self {
        Self {
            brightness: 0,
            contrast: 100,
            saturation: 100,
        }
    }
}

impl ColorAdjustments {
    pub fn new(brightness: i16, contrast: u16, saturation: u16) -> Result<Self, MediaError> {
        if !(-100..=100).contains(&brightness) || contrast > 200 || saturation > 200 {
            return Err(MediaError::InvalidColorAdjustments);
        }
        Ok(Self {
            brightness,
            contrast,
            saturation,
        })
    }
    pub const fn values(self) -> (i16, u16, u16) {
        (self.brightness, self.contrast, self.saturation)
    }
    pub fn is_neutral(self) -> bool {
        self == Self::default()
    }
    pub fn parameters(self) -> [f32; 3] {
        [
            f32::from(self.brightness) / 100.0,
            f32::from(self.contrast) / 100.0,
            f32::from(self.saturation) / 100.0,
        ]
    }
    /// Scalar reference for tests; not a CPU frame-processing path.
    pub fn reference_rgb(self, rgb: [f32; 3]) -> [f32; 3] {
        if self.is_neutral() {
            return rgb;
        }
        let [brightness, contrast, saturation] = self.parameters();
        let gray = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
        rgb.map(|channel| {
            (((gray + saturation * (channel - gray)) - 0.5) * contrast + 0.5 + brightness)
                .clamp(0.0, 1.0)
        })
    }
}

/// Shared output policy; legacy native quality mode remains an explicit None outside this type.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct VideoSettings {
    pub bitrate: VideoBitrate,
    pub frame_rate: FrameRateSpec,
    #[serde(default)]
    pub color: ColorAdjustments,
}

impl VideoSettings {
    pub fn resolve_bitrate(
        self,
        output: Size,
        codec: VideoCodec,
        source: BitrateSource,
    ) -> Result<u32, MediaError> {
        let (num, den) = self.frame_rate.resolve(source.fps_num, source.fps_den)?;
        self.bitrate
            .resolve_with_source(output, num, den, codec, source)
    }
}

/// A platform-normalized track interval. Unknown endpoint withholds estimates.
#[derive(Clone, Copy, Debug)]
pub struct TrackCoverage {
    pub start_us: i128,
    pub end_us: Option<i128>,
}

/// The backend supplies only the audio track actually included and its encoder target.
pub fn estimate_output_coverage(
    video_bps: u32,
    video: TrackCoverage,
    audio: Option<(TrackCoverage, u32)>,
) -> Option<u64> {
    let mut origin = video.start_us;
    let mut end = video.end_us?;
    if end <= origin {
        return None;
    }
    let audio_bps = if let Some((track, bps)) = audio {
        let audio_end = track.end_us?;
        if audio_end <= track.start_us {
            return None;
        }
        origin = origin.min(track.start_us);
        end = end.max(audio_end);
        bps
    } else {
        0
    };
    let duration = u64::try_from(end.checked_sub(origin)?).ok()?;
    estimate_output_bytes(video_bps, audio_bps, duration)
}

/// Checked rational output grid. Container endpoints may retain sub-microsecond ticks.
#[derive(Clone, Copy, Debug)]
pub struct FrameRateGrid {
    numerator: u32,
    denominator: u32,
}

impl FrameRateGrid {
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, MediaError> {
        FrameRateSpec::Constant {
            numerator,
            denominator,
        }
        .resolve(1, 1)?;
        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub fn index(
        self,
        microseconds_numerator: i128,
        divisor: i128,
        ceiling: bool,
    ) -> Result<i64, MediaError> {
        if microseconds_numerator < 0 || divisor <= 0 {
            return Err(MediaError::InvalidTimeBase);
        }
        let scale = i128::from(self.denominator)
            .checked_mul(1_000_000)
            .and_then(|value| value.checked_mul(divisor))
            .ok_or(MediaError::TimestampOverflow)?;
        let value = microseconds_numerator
            .checked_mul(i128::from(self.numerator))
            .and_then(|value| value.checked_add(if ceiling { scale - 1 } else { scale / 2 }))
            .ok_or(MediaError::TimestampOverflow)?
            / scale;
        i64::try_from(value).map_err(|_| MediaError::TimestampOverflow)
    }

    pub fn timestamp_us(self, index: i64) -> Result<i64, MediaError> {
        TimeBase::new(self.denominator, self.numerator)?.ticks_to_microseconds(index)
    }
}

impl FrameRateSpec {
    pub fn resolve(self, source_num: u32, source_den: u32) -> Result<(u32, u32), MediaError> {
        let (num, den) = match self {
            Self::Original => (source_num, source_den),
            Self::Constant {
                numerator,
                denominator,
            } => (numerator, denominator),
        };
        if num == 0
            || den == 0
            || (matches!(self, Self::Constant { .. })
                && (u64::from(num) < u64::from(den) || u64::from(num) > 120 * u64::from(den)))
        {
            return Err(MediaError::InvalidFrameRate);
        }
        Ok((num, den))
    }
}

/// Approximate decimal bytes from target rates, plus 2% muxing allowance.
/// VBR encoders can undershoot/overshoot; this is not a file-size limit.
pub fn estimate_output_bytes(video_bps: u32, audio_bps: u32, duration_us: u64) -> Option<u64> {
    if duration_us == 0 {
        return None;
    }
    let bits = (u128::from(video_bps) + u128::from(audio_bps)) * u128::from(duration_us);
    u64::try_from((bits * 102).div_ceil(8 * 1_000_000 * 100)).ok()
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
    InvalidColorAdjustments,
    InvalidBitrateInput,
    InvalidFrameRate,
    InvalidBitrate(u32),
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
            Self::InvalidColorAdjustments => {
                f.write_str("brightness must be -100..100 and contrast/saturation 0..200")
            }
            Self::InvalidBitrateInput => {
                f.write_str("Custom bitrate must be between 0.25 and 120 Mbps.")
            }
            Self::InvalidFrameRate => f.write_str(
                "constant frame rate must be between 1 and 120 fps, with positive rational terms",
            ),
            Self::InvalidBitrate(value) => write!(
                f,
                "video bitrate must be between 0.25 and 120 Mbps, got {value} bps"
            ),
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
    fn color_controls_are_neutral_bounded_and_apply_the_documented_order() {
        let rgb = [0.15, 0.7, 0.95];
        assert_eq!(ColorAdjustments::default().reference_rgb(rgb), rgb);
        let gray = ColorAdjustments::new(0, 100, 0)
            .unwrap()
            .reference_rgb([1.0, 0.0, 0.0]);
        for channel in gray {
            assert!((channel - 0.2126).abs() < 1e-6);
        }
        assert_eq!(
            ColorAdjustments::new(0, 0, 200).unwrap().reference_rgb(rgb),
            [0.5; 3]
        );
        assert_eq!(
            ColorAdjustments::new(100, 100, 100)
                .unwrap()
                .reference_rgb(rgb),
            [1.0; 3]
        );
        assert_eq!(
            ColorAdjustments::new(-100, 100, 100)
                .unwrap()
                .reference_rgb(rgb),
            [0.0; 3]
        );
        let combined = ColorAdjustments::new(10, 150, 50)
            .unwrap()
            .reference_rgb([1.0, 0.0, 0.0]);
        for (actual, expected) in combined.into_iter().zip([0.75945, 0.00945, 0.00945]) {
            assert!((actual - expected).abs() < 1e-6);
        }
        for values in [
            (-101, 100, 100),
            (101, 100, 100),
            (0, 201, 100),
            (0, 100, 201),
        ] {
            assert!(ColorAdjustments::new(values.0, values.1, values.2).is_err());
        }
    }

    #[test]
    fn shared_custom_bitrate_and_fps_parsers_reject_invalid_input() {
        assert_eq!(
            VideoBitrate::from_mbps(" 3.5 ").unwrap(),
            VideoBitrate::BitsPerSecond(3_500_000)
        );
        for value in ["", "NaN", "inf", "0", "0.249999", "120.01", "oops"] {
            assert!(VideoBitrate::from_mbps(value).is_err(), "{value}");
        }
        assert_eq!(
            VideoBitrate::from_mbps("0.25").unwrap(),
            VideoBitrate::BitsPerSecond(250_000)
        );
        assert_eq!(
            VideoBitrate::from_mbps("120").unwrap(),
            VideoBitrate::BitsPerSecond(120_000_000)
        );
        assert_eq!(
            "original".parse::<FrameRateSpec>().unwrap(),
            FrameRateSpec::Original
        );
        assert_eq!(
            "30000/1001"
                .parse::<FrameRateSpec>()
                .unwrap()
                .resolve(1, 1)
                .unwrap(),
            (30_000, 1001)
        );
        assert_eq!(
            " 15 "
                .parse::<FrameRateSpec>()
                .unwrap()
                .resolve(1, 1)
                .unwrap(),
            (15, 1)
        );
        for value in ["", "0", "121", "30/0", "1/2", "30/1/2", "NaN", "4294967296"] {
            assert!(value.parse::<FrameRateSpec>().is_err(), "{value}");
        }
    }

    #[test]
    fn shared_coverage_estimate_respects_included_tracks_and_unknown_endpoints() {
        let video = TrackCoverage {
            start_us: 1_000_000,
            end_us: Some(3_000_000),
        };
        let audio = TrackCoverage {
            start_us: -1_000_000,
            end_us: Some(4_000_000),
        };
        assert_eq!(
            estimate_output_coverage(1_000_000, video, Some((audio, 128_000))),
            estimate_output_bytes(1_000_000, 128_000, 5_000_000)
        );
        assert_eq!(
            estimate_output_coverage(1_000_000, video, None),
            estimate_output_bytes(1_000_000, 0, 2_000_000)
        );
        assert_eq!(
            estimate_output_coverage(
                1_000_000,
                video,
                Some((
                    TrackCoverage {
                        end_us: None,
                        ..audio
                    },
                    192_000
                ))
            ),
            None
        );
        assert_eq!(
            estimate_output_coverage(
                1_000_000,
                TrackCoverage {
                    end_us: None,
                    ..video
                },
                None
            ),
            None
        );
        assert_eq!(
            estimate_output_coverage(
                1_000_000,
                TrackCoverage {
                    end_us: Some(0),
                    ..video
                },
                None
            ),
            None
        );
        assert_eq!(
            estimate_output_coverage(
                1_000_000,
                TrackCoverage {
                    start_us: i128::MIN,
                    end_us: Some(i128::MAX)
                },
                None
            ),
            None
        );
    }

    #[test]
    fn shared_settings_resolve_original_and_selected_fps_against_source() {
        let source = BitrateSource {
            size: Size::new_unchecked(640, 360),
            fps_num: 30,
            fps_den: 1,
            codec: VideoCodec::H264,
            bitrate_bps: Some(2_000_000),
        };
        let settings = VideoSettings::default();
        assert_eq!(
            settings
                .resolve_bitrate(source.size, source.codec, source)
                .unwrap(),
            2_000_000
        );
        assert_eq!(
            VideoSettings {
                frame_rate: "15".parse().unwrap(),
                ..settings
            }
            .resolve_bitrate(source.size, source.codec, source)
            .unwrap(),
            1_000_000
        );
        assert_eq!(
            VideoSettings {
                bitrate: VideoBitrate::from_mbps("3.5").unwrap(),
                ..settings
            }
            .resolve_bitrate(source.size, VideoCodec::H265, source)
            .unwrap(),
            3_500_000
        );
    }

    #[test]
    fn frame_grid_handles_drop_duplicate_offsets_and_exact_eof() {
        let grid = FrameRateGrid::new(30, 1).unwrap();
        assert_eq!(grid.index(2_000_000, 30, true).unwrap(), 2);
        assert_eq!(grid.index(66_667, 1, true).unwrap(), 3);
        assert_eq!(grid.index(22_000, 1, false).unwrap(), 1);
        assert_eq!(
            FrameRateGrid::new(15, 1)
                .unwrap()
                .index(33_333, 1, false)
                .unwrap(),
            0
        );
        let ntsc = FrameRateGrid::new(30_000, 1001).unwrap();
        assert_eq!(ntsc.timestamp_us(60).unwrap(), 2_002_000);
        assert_eq!(ntsc.index(2_002_000, 1, true).unwrap(), 60);
        assert!(grid.index(i128::MAX, 1, true).is_err());
        assert!(grid.index(-1, 1, false).is_err());
    }

    #[test]
    fn adaptive_bitrate_uses_source_pixels_fps_codec_and_custom_override() {
        let size = Size::new(1920, 1080).unwrap();
        let source = BitrateSource {
            size,
            fps_num: 30,
            fps_den: 1,
            codec: VideoCodec::H264,
            bitrate_bps: Some(4_000_000),
        };
        let rate = |policy, size, fps, codec| {
            VideoBitrate::resolve_with_source(policy, size, fps, 1, codec, source).unwrap()
        };
        assert_eq!(
            rate(VideoBitrate::Recommended, size, 30, VideoCodec::H264),
            4_000_000
        );
        assert_eq!(
            rate(VideoBitrate::Smaller, size, 30, VideoCodec::H264),
            2_400_000
        );
        assert_eq!(
            rate(VideoBitrate::Higher, size, 30, VideoCodec::H264),
            6_000_000
        );
        let half = Size::new(960, 540).unwrap();
        assert_eq!(
            rate(VideoBitrate::Recommended, half, 30, VideoCodec::H264),
            2_000_000
        );
        assert_eq!(
            rate(VideoBitrate::Recommended, size, 15, VideoCodec::H264),
            2_000_000
        );
        assert_eq!(
            rate(VideoBitrate::Recommended, size, 30, VideoCodec::H265),
            2_800_000
        );
        assert_eq!(
            rate(
                VideoBitrate::BitsPerSecond(3_500_000),
                half,
                15,
                VideoCodec::H265
            ),
            3_500_000
        );
        let unknown = BitrateSource {
            bitrate_bps: None,
            ..source
        };
        assert_eq!(
            VideoBitrate::Recommended
                .resolve_with_source(size, 30, 1, VideoCodec::H264, unknown)
                .unwrap(),
            6_220_800
        );
    }

    #[test]
    fn recommended_bitrate_retains_detail_budget_when_downscaling_complex_sources() {
        let source = BitrateSource {
            size: Size::new(640, 360).unwrap(),
            fps_num: 30,
            fps_den: 1,
            codec: VideoCodec::H264,
            bitrate_bps: Some(1_000_000),
        };
        let half = Size::new(320, 180).unwrap();
        let resolve = |policy: VideoBitrate, size, fps| {
            policy
                .resolve_with_source(size, fps, 1, VideoCodec::Vp8, source)
                .unwrap()
        };
        assert_eq!(resolve(VideoBitrate::Recommended, half, 30), 500_000);
        assert_eq!(resolve(VideoBitrate::Recommended, half, 15), 250_000);
        assert_eq!(
            resolve(VideoBitrate::Recommended, source.size, 30),
            1_000_000
        );
        assert_eq!(resolve(VideoBitrate::Smaller, half, 30), 300_000);
        assert_eq!(resolve(VideoBitrate::Higher, half, 30), 750_000);
        assert_eq!(
            resolve(VideoBitrate::BitsPerSecond(800_000), half, 30),
            800_000
        );
        let complex = BitrateSource {
            bitrate_bps: Some(40_000_000),
            ..source
        };
        assert_eq!(
            VideoBitrate::Recommended
                .resolve_with_source(half, 30, 1, VideoCodec::H264, complex)
                .unwrap(),
            20_000_000
        );
    }

    #[test]
    fn frame_rate_preserves_original_or_validates_rational_resampling() {
        assert_eq!(
            FrameRateSpec::Original.resolve(30_000, 1001).unwrap(),
            (30_000, 1001)
        );
        assert_eq!(
            FrameRateSpec::Constant {
                numerator: 24_000,
                denominator: 1001
            }
            .resolve(30, 1)
            .unwrap(),
            (24_000, 1001)
        );
        for (numerator, denominator) in [(0, 1), (30, 0), (1, 2), (121, 1)] {
            assert!(
                FrameRateSpec::Constant {
                    numerator,
                    denominator
                }
                .resolve(30, 1)
                .is_err()
            );
        }
        assert!(FrameRateSpec::Original.resolve(30, 0).is_err());
    }

    #[test]
    fn bitrate_resolution_and_estimation_are_checked_and_geometry_aware() {
        let size = Size::new(1920, 1080).unwrap();
        let full = VideoBitrate::Recommended
            .resolve(size, 30_000, 1001, VideoCodec::H264)
            .unwrap();
        assert_eq!(full, 6_214_586);
        let half = ResizeSpec::Percent(50).output_size(size).unwrap();
        let half_rate = VideoBitrate::Recommended
            .resolve(half, 30_000, 1001, VideoCodec::H264)
            .unwrap();
        assert!((i64::from(full) - 4 * i64::from(half_rate)).abs() < 4);
        assert!(
            VideoBitrate::Recommended
                .resolve(size, 30_000, 1001, VideoCodec::H265)
                .unwrap()
                < full
        );
        assert_eq!(
            VideoBitrate::BitsPerSecond(4_000_000)
                .resolve(half, 60, 1, VideoCodec::H265)
                .unwrap(),
            4_000_000
        );
        assert!(
            VideoBitrate::BitsPerSecond(0)
                .resolve(size, 30, 1, VideoCodec::H264)
                .is_err()
        );
        assert!(
            VideoBitrate::BitsPerSecond(120_000_001)
                .resolve(size, 30, 1, VideoCodec::H264)
                .is_err()
        );
        assert!(
            VideoBitrate::Recommended
                .resolve(size, 30, 0, VideoCodec::H264)
                .is_err()
        );
        assert_eq!(
            estimate_output_bytes(4_000_000, 192_000, 60_000_000),
            Some(32_068_800)
        );
        assert_eq!(
            estimate_output_bytes(4_000_000, 0, 60_000_000),
            Some(30_600_000)
        );
        assert_eq!(estimate_output_bytes(4_000_000, 0, 0), None);
        assert_eq!(estimate_output_bytes(u32::MAX, u32::MAX, u64::MAX), None);
    }

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
        assert_eq!(
            OutputProfileId::Mp4H265Main10Aac.video_codec(),
            VideoCodec::H265
        );
        assert_eq!(OutputProfileId::Mp4H265Main10Aac.video_bit_depth(), 10);
        assert_eq!(
            OutputProfileId::Mp4H265Main10Aac.container(),
            ContainerFormat::Mp4
        );
        assert_eq!(OutputProfileId::Mp4H264Aac.video_bit_depth(), 8);
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
