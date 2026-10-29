//! Pure AI job policy. Capability records are supplied by a validated platform
//! adapter; these types neither enable a backend nor acquire any assets.
use crate::{MediaError, ResizeSpec, Size, codec_size};
use serde::{Deserialize, Serialize};

pub const ASSET_MANIFEST_JSON: &str = include_str!("ai-upscale-model.json");

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AiScale {
    #[serde(rename = "2x")]
    Two,
    #[serde(rename = "4x")]
    Four,
}

impl AiScale {
    pub const fn factor(self) -> u32 {
        match self {
            Self::Two => 2,
            Self::Four => 4,
        }
    }
}

/// Immutable identity of the allowlisted artifact, not an arbitrary file path.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AiModel {
    #[serde(rename = "realesr-general-x4v3-strong-fp32-v1")]
    RealEsrGeneralX4v3StrongFp32V1,
}

impl AiModel {
    pub const fn internal_scale(self) -> AiScale {
        AiScale::Four
    }

    pub const fn onnx_sha256(self) -> &'static str {
        "1793a6e7fdf15a53eed213ba269ea768b5373858ae433d56ee8e1b0424377cc5"
    }

    pub const fn upstream_weights_sha256(self) -> &'static str {
        "8dc7edb9ac80ccdc30c3a5dca6616509367f05fbc184ad95b731f05bece96292"
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RtxConfiguration {
    #[serde(rename = "sdk-1.1-sdr-v1")]
    Sdk11SdrV1,
}

/// Saved intent is separate from runtime availability. Unknown or incompatible
/// backend/model/configuration/scale values fail deserialization, never fall back.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "backend", rename_all = "kebab-case", from = "AiSettingsWire")]
pub enum AiSettings {
    #[default]
    Off,
    OnnxRuntime {
        model: AiModel,
        scale: AiScale,
    },
    Burn {
        model: AiModel,
        scale: AiScale,
    },
    RtxVideo {
        configuration: RtxConfiguration,
        scale: AiScale,
    },
}

// Serde's internally tagged unit variants ignore extra fields. An empty struct
// variant here makes even an Off payload reject hidden model/scale fields.
#[derive(Deserialize)]
#[serde(tag = "backend", rename_all = "kebab-case", deny_unknown_fields)]
enum AiSettingsWire {
    Off {},
    OnnxRuntime {
        model: AiModel,
        scale: AiScale,
    },
    Burn {
        model: AiModel,
        scale: AiScale,
    },
    RtxVideo {
        configuration: RtxConfiguration,
        scale: AiScale,
    },
}

impl From<AiSettingsWire> for AiSettings {
    fn from(value: AiSettingsWire) -> Self {
        match value {
            AiSettingsWire::Off {} => Self::Off,
            AiSettingsWire::OnnxRuntime { model, scale } => Self::OnnxRuntime { model, scale },
            AiSettingsWire::Burn { model, scale } => Self::Burn { model, scale },
            AiSettingsWire::RtxVideo {
                configuration,
                scale,
            } => Self::RtxVideo {
                configuration,
                scale,
            },
        }
    }
}

impl AiSettings {
    /// Compact versioned identity for the browser's existing command envelope.
    pub fn command(self) -> String {
        match self {
            Self::Off => "off".into(),
            Self::OnnxRuntime { scale, .. } | Self::Burn { scale, .. } => format!(
                "{}/realesr-general-x4v3-strong-fp32-v1/{}x",
                if matches!(self, Self::OnnxRuntime { .. }) {
                    "onnx-runtime"
                } else {
                    "burn"
                },
                scale.factor()
            ),
            Self::RtxVideo { scale, .. } => format!("rtx-video/sdk-1.1-sdr-v1/{}x", scale.factor()),
        }
    }

    pub const fn scale(self) -> Option<AiScale> {
        match self {
            Self::Off => None,
            Self::OnnxRuntime { scale, .. }
            | Self::Burn { scale, .. }
            | Self::RtxVideo { scale, .. } => Some(scale),
        }
    }

    /// Input must already have crop/PAR/orientation normalization applied. No
    /// ordinary resize is applied before inference. Off delegates unchanged.
    /// This is an intent/codec-probe size only; it grants no backend availability.
    pub fn requested_output_size(
        self,
        normalized_source: Size,
        ordinary_resize: ResizeSpec,
    ) -> Result<Size, MediaError> {
        let Some(scale) = self.scale() else {
            return ordinary_resize.output_size(normalized_source);
        };
        Size::new(normalized_source.width, normalized_source.height)?;
        codec_size(Size::new(
            normalized_source
                .width
                .checked_mul(scale.factor())
                .ok_or(MediaError::ResizeOverflow)?,
            normalized_source
                .height
                .checked_mul(scale.factor())
                .ok_or(MediaError::ResizeOverflow)?,
        )?)
    }

    pub fn validate_format(
        self,
        source_bit_depth: u8,
        opaque_sdr: bool,
        profile: crate::OutputProfileId,
    ) -> Result<(), MediaError> {
        if self != Self::Off
            && (source_bit_depth != 8 || !opaque_sdr || profile.video_bit_depth() != 8)
        {
            return Err(MediaError::AiUnavailable(
                "initial AI requires known opaque 8-bit SDR source and output".into(),
            ));
        }
        Ok(())
    }

    pub fn output_size(
        self,
        normalized_source: Size,
        ordinary_resize: ResizeSpec,
        capability: &AiCapability,
    ) -> Result<Size, MediaError> {
        let output = self.requested_output_size(normalized_source, ordinary_resize)?;
        if self == Self::Off {
            return Ok(output);
        }
        let limits = match capability {
            AiCapability::Validated { settings, limits } if *settings == self => limits,
            AiCapability::Unavailable { reason } | AiCapability::PilotOnly { reason } => {
                return Err(MediaError::AiUnavailable(reason.clone()));
            }
            AiCapability::Validated { .. } => {
                return Err(MediaError::AiUnavailable(
                    "AI capability does not match the requested backend/model/configuration/scale"
                        .into(),
                ));
            }
        };
        if normalized_source.width > limits.max_source.width
            || normalized_source.height > limits.max_source.height
            || output.width > limits.max_output.width
            || output.height > limits.max_output.height
            || u64::from(output.width) * u64::from(output.height) > limits.max_output_pixels
        {
            return Err(MediaError::AiUnavailable(
                "AI source/output geometry exceeds the validated dimension or pixel limits".into(),
            ));
        }
        Ok(output)
    }
}

/// Model-specific clipped source halo. The platform owns GPU resources; core
/// supplies the same checked tile/crop coordinates to every adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AiTile {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

pub fn pilot_tiles(source: Size, interior: u32) -> Result<Vec<AiTile>, MediaError> {
    if source.width == 0
        || source.height == 0
        || source.width > 577
        || source.height > 579
        || ![128, 256].contains(&interior)
    {
        return Err(MediaError::AiUnavailable(
            "outside the measured AI tile pilot envelope".into(),
        ));
    }
    let mut tiles = Vec::new();
    for top in (0..source.height).step_by(interior as usize) {
        for left in (0..source.width).step_by(interior as usize) {
            let right = (left + interior).min(source.width);
            let bottom = (top + interior).min(source.height);
            tiles.push(AiTile {
                left,
                top,
                right,
                bottom,
                x0: left.saturating_sub(34),
                y0: top.saturating_sub(34),
                x1: (right + 34).min(source.width),
                y1: (bottom + 34).min(source.height),
            });
        }
    }
    Ok(tiles)
}

/// Per-job admission arithmetic, not a measurement of total VRAM. The platform
/// supplies separately qualified model/workspace estimates and actual downstream
/// and retained-frame reservations; opaque driver allocations remain unknown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AiMemoryPlan {
    pub tile_buffers: u64,
    pub source_texture: u64,
    pub output_texture: u64,
    pub tile_uniforms: u64,
    pub weights: u64,
    pub workspace_estimate: u64,
    pub downstream: u64,
    pub retained: u64,
    pub total: u64,
    pub budget: u64,
}

impl AiMemoryPlan {
    pub fn new(
        source: Size,
        weights: u64,
        workspace_estimate: u64,
        downstream: u64,
        retained: u64,
        budget: u64,
    ) -> Result<Self, MediaError> {
        let tiles = pilot_tiles(source, 128)?;
        let tile_buffers = 196 * 196 * 3 * 4 * 17;
        let source_texture = u64::from(source.width) * u64::from(source.height) * 4;
        let output_texture = source_texture * 16;
        let tile_uniforms = tiles.len() as u64 * 2 * 32;
        let total = [
            tile_buffers,
            source_texture,
            output_texture,
            tile_uniforms,
            weights,
            workspace_estimate,
            downstream,
            retained,
        ]
        .into_iter()
        .try_fold(0u64, |total, bytes| total.checked_add(bytes))
        .ok_or_else(|| {
            MediaError::AiUnavailable("AI memory admission arithmetic overflow".into())
        })?;
        if total > budget {
            return Err(MediaError::AiUnavailable(format!(
                "AI memory admission requires {total} bytes, budget is {budget}; reduce geometry or downstream retention"
            )));
        }
        Ok(Self {
            tile_buffers,
            source_texture,
            output_texture,
            tile_uniforms,
            weights,
            workspace_estimate,
            downstream,
            retained,
            total,
            budget,
        })
    }
}

impl std::str::FromStr for AiSettings {
    type Err = MediaError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "off" {
            return Ok(Self::Off);
        }
        let mut parts = value.split('/');
        let invalid = || {
            MediaError::AiUnavailable("invalid AI backend/model/configuration/scale command".into())
        };
        let backend = parts.next().ok_or_else(invalid)?;
        let identity = parts.next().ok_or_else(invalid)?;
        let scale = match parts.next() {
            Some("2x") => AiScale::Two,
            Some("4x") => AiScale::Four,
            _ => return Err(invalid()),
        };
        if parts.next().is_some() {
            return Err(invalid());
        }
        let model = AiModel::RealEsrGeneralX4v3StrongFp32V1;
        match (backend, identity) {
            ("onnx-runtime", "realesr-general-x4v3-strong-fp32-v1") => {
                Ok(Self::OnnxRuntime { model, scale })
            }
            ("burn", "realesr-general-x4v3-strong-fp32-v1") => Ok(Self::Burn { model, scale }),
            ("rtx-video", "sdk-1.1-sdr-v1") => Ok(Self::RtxVideo {
                configuration: RtxConfiguration::Sdk11SdrV1,
                scale,
            }),
            _ => Err(invalid()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AiGeometryLimits {
    pub max_source: Size,
    pub max_output: Size,
    pub max_output_pixels: u64,
}

/// Numerical feasibility is deliberately distinct from release acceptance. A
/// platform constructs Validated only after its full model/route/lifecycle gates.
/// Device identity/generation and runtime handles remain in that platform owner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AiCapability {
    Unavailable {
        reason: String,
    },
    PilotOnly {
        reason: String,
    },
    Validated {
        settings: AiSettings,
        limits: AiGeometryLimits,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FrameGeometry, Rect, Rotation, VideoSettings};
    use serde_json::json;

    #[test]
    fn memory_admission_accounts_for_downstream_retention_and_overflow() {
        let size = Size::new(577, 579).unwrap();
        let plan = AiMemoryPlan::new(size, 8 << 20, 128 << 20, 0, 0, 384 << 20).unwrap();
        assert!(plan.total > plan.output_texture + plan.tile_buffers);
        let slack = plan.budget - plan.total;
        assert!(AiMemoryPlan::new(size, 8 << 20, 128 << 20, slack, 1, 384 << 20).is_err());
        assert!(AiMemoryPlan::new(size, 8 << 20, 128 << 20, slack, 0, 384 << 20).is_ok());
        assert!(AiMemoryPlan::new(size, u64::MAX, 1, 0, 0, u64::MAX).is_err());
    }

    #[test]
    fn enlargement_updates_source_aware_rates_and_estimates_without_changing_fps() {
        use crate::{BitrateSource, FrameRateSpec, VideoCodec, estimate_output_bytes};
        let source_size = Size::new(640, 360).unwrap();
        let source = BitrateSource {
            size: source_size,
            fps_num: 30_000,
            fps_den: 1001,
            codec: VideoCodec::H264,
            bitrate_bps: Some(2_000_000),
        };
        for fps in [
            FrameRateSpec::Original,
            FrameRateSpec::Constant {
                numerator: 60,
                denominator: 1,
            },
        ] {
            let mut prior = 0;
            for ai in [
                AiSettings::Off,
                settings(AiScale::Two),
                settings(AiScale::Four),
            ] {
                let settings = VideoSettings {
                    ai,
                    frame_rate: fps,
                    ..Default::default()
                };
                let snapshot = settings;
                let size = ai
                    .requested_output_size(source_size, ResizeSpec::Percent(50))
                    .unwrap();
                let rate = settings
                    .resolve_bitrate(size, VideoCodec::H264, source)
                    .unwrap();
                assert!(rate > prior);
                let bytes = estimate_output_bytes(rate, 192_000, 5_000_000).unwrap();
                assert!(bytes > estimate_output_bytes(prior, 192_000, 5_000_000).unwrap());
                assert_eq!(settings, snapshot);
                assert_eq!(
                    settings
                        .frame_rate
                        .resolve(source.fps_num, source.fps_den)
                        .unwrap(),
                    fps.resolve(source.fps_num, source.fps_den).unwrap()
                );
                prior = rate;
            }
        }
    }

    fn settings(scale: AiScale) -> AiSettings {
        AiSettings::OnnxRuntime {
            model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
            scale,
        }
    }

    #[test]
    fn tile_interiors_cover_odd_source_once_with_clipped_halos() {
        let source = Size::new(577, 579).unwrap();
        for interior in [128, 256] {
            let tiles = pilot_tiles(source, interior).unwrap();
            let mut coverage = vec![0u8; (source.width * source.height) as usize];
            for tile in tiles {
                assert!(
                    tile.x0 <= tile.left
                        && tile.y0 <= tile.top
                        && tile.x1 >= tile.right
                        && tile.y1 >= tile.bottom
                );
                assert!(tile.x1 <= source.width && tile.y1 <= source.height);
                assert!(tile.x1 - tile.x0 <= interior + 68 && tile.y1 - tile.y0 <= interior + 68);
                for y in tile.top..tile.bottom {
                    for x in tile.left..tile.right {
                        coverage[(y * source.width + x) as usize] += 1;
                    }
                }
            }
            assert!(coverage.into_iter().all(|n| n == 1));
        }
        assert!(pilot_tiles(Size::new_unchecked(u32::MAX, u32::MAX), 128).is_err());
    }

    #[test]
    fn probe_sizing_never_grants_availability_and_precision_is_explicit() {
        use crate::OutputProfileId;
        let ai = settings(AiScale::Two);
        let input = Size::new(640, 360).unwrap();
        assert_eq!(
            ai.requested_output_size(input, ResizeSpec::Percent(50))
                .unwrap(),
            Size::new(1280, 720).unwrap()
        );
        assert!(
            ai.output_size(
                input,
                ResizeSpec::Original,
                &AiCapability::PilotOnly {
                    reason: "not accepted".into()
                }
            )
            .is_err()
        );
        for (depth, sdr, profile) in [
            (10, true, OutputProfileId::Mp4H264Aac),
            (0, true, OutputProfileId::Mp4H264Aac),
            (8, false, OutputProfileId::Mp4H264Aac),
            (8, true, OutputProfileId::Mp4H265Main10Aac),
        ] {
            assert!(ai.validate_format(depth, sdr, profile).is_err());
            assert!(AiSettings::Off.validate_format(depth, sdr, profile).is_ok());
        }
    }

    #[test]
    fn command_identity_round_trips_and_rejects_unrecognized_components() {
        assert_eq!("off".parse::<AiSettings>().unwrap(), AiSettings::Off);
        for scale in [AiScale::Two, AiScale::Four] {
            for choice in [
                settings(scale),
                AiSettings::Burn {
                    model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
                    scale,
                },
                AiSettings::RtxVideo {
                    configuration: RtxConfiguration::Sdk11SdrV1,
                    scale,
                },
            ] {
                assert_eq!(choice.command().parse::<AiSettings>().unwrap(), choice);
            }
        }
        for malformed in [
            "",
            "off/4x",
            "onnx-runtime/file.onnx/4x",
            "burn/realesr-general-x4v3-strong-fp32-v1/3x",
            "rtx-video/realesr-general-x4v3-strong-fp32-v1/4x",
            "onnx-runtime/realesr-general-x4v3-strong-fp32-v1/4x/extra",
        ] {
            assert!(malformed.parse::<AiSettings>().is_err());
        }
    }

    #[test]
    fn asset_manifest_matches_the_immutable_model_identity() {
        let manifest: serde_json::Value = serde_json::from_str(ASSET_MANIFEST_JSON).unwrap();
        let model = AiModel::RealEsrGeneralX4v3StrongFp32V1;
        let requested = AiSettings::OnnxRuntime {
            model,
            scale: AiScale::Four,
        };
        assert_eq!(
            requested.command().split('/').nth(1).unwrap(),
            manifest["id"].as_str().unwrap()
        );
        assert_eq!(
            model.onnx_sha256(),
            manifest["model"]["sha256"].as_str().unwrap()
        );
        assert_eq!(
            model.internal_scale().factor(),
            manifest["model"]["internal_scale"].as_u64().unwrap() as u32
        );
    }

    // Synthetic policy limits do not assert any runtime has release acceptance.
    fn capability(settings: AiSettings) -> AiCapability {
        AiCapability::Validated {
            settings,
            limits: AiGeometryLimits {
                max_source: Size::new_unchecked(1920, 1080),
                max_output: Size::new_unchecked(7680, 4320),
                max_output_pixels: 7680 * 4320,
            },
        }
    }

    #[test]
    fn legacy_settings_stay_off_and_snapshots_keep_their_ai_identity() {
        let mut legacy = serde_json::to_value(VideoSettings::default()).unwrap();
        legacy.as_object_mut().unwrap().remove("ai");
        let mut current: VideoSettings = serde_json::from_value(legacy).unwrap();
        assert_eq!(current.ai, AiSettings::Off);
        current.ai = settings(AiScale::Four);
        let job = current;
        current.ai = AiSettings::Off;
        assert_eq!(job.ai, settings(AiScale::Four));
        assert_eq!(current.ai, AiSettings::Off);
        assert_eq!(
            serde_json::from_value::<VideoSettings>(serde_json::to_value(job).unwrap()).unwrap(),
            job
        );
    }

    #[test]
    fn malformed_settings_never_deserialize_as_off_or_another_backend() {
        for ai in [
            json!({"backend":"unknown"}),
            json!({"backend":"onnx-runtime","model":"arbitrary.onnx","scale":"4x"}),
            json!({"backend":"onnx-runtime","model":"realesr-general-x4v3-strong-fp32-v1","scale":"3x"}),
            json!({"backend":"burn","scale":"4x"}),
            json!({"backend":"rtx-video","model":"realesr-general-x4v3-strong-fp32-v1","scale":"4x"}),
            json!({"backend":"off","scale":"4x"}),
            json!({"backend":"rtx-video","configuration":"sdk-unknown","scale":"2x"}),
        ] {
            let mut saved = serde_json::to_value(VideoSettings::default()).unwrap();
            saved["ai"] = ai;
            assert!(serde_json::from_value::<VideoSettings>(saved).is_err());
        }
    }

    #[test]
    fn selected_scale_replaces_resize_and_off_restores_it_without_mutation() {
        let source = Size::new(640, 360).unwrap();
        let ordinary = ResizeSpec::Percent(50);
        for (scale, expected) in [(AiScale::Two, (1280, 720)), (AiScale::Four, (2560, 1440))] {
            let ai = settings(scale);
            assert_eq!(
                ai.output_size(source, ordinary, &capability(ai)).unwrap(),
                Size::new(expected.0, expected.1).unwrap()
            );
        }
        let unavailable = AiCapability::Unavailable {
            reason: "no runtime".into(),
        };
        assert_eq!(
            AiSettings::Off
                .output_size(source, ordinary, &unavailable)
                .unwrap(),
            Size::new(320, 180).unwrap()
        );
        assert!(
            AiSettings::Off
                .output_size(source, ResizeSpec::Percent(200), &unavailable)
                .is_err()
        );
        assert!(
            AiSettings::Off
                .output_size(
                    source,
                    ResizeSpec::Exact {
                        width: 1280,
                        height: 720,
                        preserve_aspect_ratio: true
                    },
                    &unavailable
                )
                .is_err()
        );
    }

    #[test]
    fn geometry_uses_oriented_source_and_preserves_odd_dimension_aspect_ratio() {
        let raw = Size::new(321, 179).unwrap();
        let geometry = FrameGeometry::new(
            raw,
            Rect::new(0, 0, 321, 179).unwrap(),
            raw,
            Rotation::Deg90,
            false,
        )
        .unwrap();
        let ai = settings(AiScale::Four);
        let output = ai
            .output_size(
                geometry.display_size(),
                ResizeSpec::Original,
                &capability(ai),
            )
            .unwrap();
        assert_eq!(output, Size::new(716, 1284).unwrap());
        assert_eq!(output.width % 2, 0);
        assert_eq!(output.height % 2, 0);
    }

    #[test]
    fn overflow_limits_and_unaccepted_or_mismatched_capabilities_reject() {
        let ai = settings(AiScale::Four);
        let resolve =
            |source, cap: &AiCapability| ai.output_size(source, ResizeSpec::Original, cap);
        assert_eq!(
            resolve(Size::new_unchecked(u32::MAX, 1), &capability(ai)),
            Err(MediaError::ResizeOverflow)
        );
        assert!(resolve(Size::new_unchecked(0, 1), &capability(ai)).is_err());
        assert!(resolve(Size::new_unchecked(1921, 1080), &capability(ai)).is_err());
        let source = Size::new(640, 360).unwrap();
        assert!(resolve(source, &capability(settings(AiScale::Two))).is_err());
        assert!(
            resolve(
                source,
                &AiCapability::PilotOnly {
                    reason: "numeric probe only".into()
                }
            )
            .is_err()
        );
        let AiCapability::Validated {
            settings,
            mut limits,
        } = capability(ai)
        else {
            unreachable!()
        };
        limits.max_output_pixels = 100;
        assert!(resolve(source, &AiCapability::Validated { settings, limits }).is_err());
        limits.max_output_pixels = u64::MAX;
        limits.max_output.width = 100;
        assert!(resolve(source, &AiCapability::Validated { settings, limits }).is_err());
    }
}
