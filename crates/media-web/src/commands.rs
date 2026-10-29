//! The same checked command codec is used before worker dispatch and in execution.
use media_core::{
    ColorAdjustments, FrameRateSpec, MediaError, ResizeSpec, VideoBitrate, VideoSettings,
};

fn invalid() -> MediaError {
    MediaError::Platform("invalid video settings command".into())
}

pub(crate) fn encode(resize: ResizeSpec, settings: Option<VideoSettings>) -> String {
    let mut value = resize_command(resize);
    if let Some(settings) = settings {
        let bitrate = match settings.bitrate {
            VideoBitrate::Smaller => "smaller".into(),
            VideoBitrate::Recommended => "recommended".into(),
            VideoBitrate::Higher => "higher".into(),
            VideoBitrate::BitsPerSecond(value) => format!("bps={value}"),
        };
        let fps = match settings.frame_rate {
            FrameRateSpec::Original => "original".into(),
            FrameRateSpec::Constant {
                numerator,
                denominator,
            } => format!("{numerator}/{denominator}"),
        };
        let (brightness, contrast, saturation) = settings.color.values();
        value.push_str(&format!(
            "~{bitrate}~{fps}~{brightness}/{contrast}/{saturation}~{}~{}",
            settings.equalization.command(),
            settings.ai.command()
        ));
    }
    value
}

pub(crate) fn decode(command: &str) -> Result<(ResizeSpec, Option<VideoSettings>), MediaError> {
    if command.len() > 512 {
        return Err(invalid());
    }
    let parts = command.split('~').collect::<Vec<_>>();
    let resize = resize_from_command(parts[0])?;
    if parts.len() == 1 {
        return Ok((resize, None));
    }
    if !(3..=6).contains(&parts.len()) {
        return Err(invalid());
    }
    let bitrate = match parts[1] {
        "smaller" => VideoBitrate::Smaller,
        "recommended" => VideoBitrate::Recommended,
        "higher" => VideoBitrate::Higher,
        value => VideoBitrate::BitsPerSecond(
            value
                .strip_prefix("bps=")
                .ok_or_else(invalid)?
                .parse()
                .map_err(|_| invalid())?,
        ),
    };
    let mut settings = VideoSettings {
        bitrate,
        frame_rate: parts[2].parse()?,
        ..Default::default()
    };
    if let Some(color) = parts.get(3) {
        let values = color.split('/').collect::<Vec<_>>();
        if values.len() != 3 {
            return Err(invalid());
        }
        settings.color = ColorAdjustments::new(
            values[0].parse().map_err(|_| invalid())?,
            values[1].parse().map_err(|_| invalid())?,
            values[2].parse().map_err(|_| invalid())?,
        )?;
    }
    if let Some(equalization) = parts.get(4) {
        settings.equalization = equalization.parse()?;
    }
    if let Some(ai) = parts.get(5) {
        settings.ai = ai.parse()?;
    }
    Ok((resize, Some(settings)))
}

pub(crate) fn resize_command(resize: ResizeSpec) -> String {
    match resize {
        ResizeSpec::Original => "original".into(),
        ResizeSpec::Percent(percent) => format!("percent:{percent}"),
        ResizeSpec::Exact {
            width,
            height,
            preserve_aspect_ratio,
        } => format!("exact:{width}:{height}:{}", u8::from(preserve_aspect_ratio)),
    }
}

fn resize_from_command(command: &str) -> Result<ResizeSpec, MediaError> {
    let fields = command.split(':').collect::<Vec<_>>();
    match fields.as_slice() {
        ["original"] => Ok(ResizeSpec::Original),
        ["percent", percent] => Ok(ResizeSpec::Percent(percent.parse().map_err(|_| invalid())?)),
        ["exact", width, height, preserve] => Ok(ResizeSpec::Exact {
            width: width.parse().map_err(|_| invalid())?,
            height: height.parse().map_err(|_| invalid())?,
            preserve_aspect_ratio: match *preserve {
                "1" => true,
                "0" => false,
                _ => return Err(invalid()),
            },
        }),
        _ => Err(invalid()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_core::ai_upscale::{AiModel, AiScale, AiSettings};

    #[test]
    fn old_command_versions_default_to_ai_off() {
        for command in [
            "original",
            "percent:50~recommended~original",
            "original~bps=2000000~15~0/100/100",
            "original~higher~60~0/100/100~1/75",
        ] {
            assert_eq!(
                decode(command).unwrap().1.unwrap_or_default().ai,
                AiSettings::Off
            );
        }
    }

    #[test]
    fn command_snapshots_preserve_ai_and_all_other_settings() {
        let resize = ResizeSpec::Exact {
            width: 640,
            height: 360,
            preserve_aspect_ratio: true,
        };
        let mut settings = VideoSettings {
            ai: AiSettings::OnnxRuntime {
                model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
                scale: AiScale::Four,
            },
            color: ColorAdjustments::new(10, 120, 80).unwrap(),
            equalization: media_core::equalization::Equalization::new(true, 75).unwrap(),
            frame_rate: FrameRateSpec::Constant {
                numerator: 30000,
                denominator: 1001,
            },
            bitrate: VideoBitrate::BitsPerSecond(2000000),
        };
        let snapshot = settings;
        let command = encode(resize, Some(settings));
        settings.ai = AiSettings::Off;
        assert_eq!(decode(&command).unwrap(), (resize, Some(snapshot)));
        assert_ne!(decode(&command).unwrap().1.unwrap().ai, settings.ai);
    }

    #[test]
    fn malformed_commands_cannot_drop_the_ai_field() {
        for command in [
            "original~recommended",
            "original~recommended~original~0/100/100~0/50~",
            "original~recommended~original~0/100/100~0/50~onnx-runtime/unknown/4x",
            "original~recommended~original~0/100/100~0/50~off~extra",
            "original~recommended~original~0/100/100/extra~0/50~off",
        ] {
            assert!(decode(command).is_err(), "{command}");
        }
    }
}
