//! CLAHE revision 1 policy. No pixels, codec handles or GPU resources.
use crate::MediaError;
use serde::{Deserialize, Serialize};

pub const REVISION: u32 = 1;
pub const BINS: u32 = 1024;
pub const HISTORY: usize = 4;
pub const HISTORY_US: i64 = 100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Values")]
pub struct Equalization {
    enabled: bool,
    strength: u16,
}
#[derive(Deserialize)]
struct Values {
    enabled: bool,
    strength: u16,
}
impl TryFrom<Values> for Equalization {
    type Error = MediaError;
    fn try_from(v: Values) -> Result<Self, Self::Error> {
        Self::new(v.enabled, v.strength)
    }
}
impl Default for Equalization {
    fn default() -> Self {
        Self {
            enabled: false,
            strength: 50,
        }
    }
}
impl Equalization {
    pub fn command(self) -> String {
        format!("{}/{}", u8::from(self.enabled), self.strength)
    }
    pub fn new(enabled: bool, strength: u16) -> Result<Self, MediaError> {
        if strength > 100 {
            return Err(MediaError::InvalidEqualization);
        }
        Ok(Self { enabled, strength })
    }
    pub const fn values(self) -> (bool, u16) {
        (self.enabled, self.strength)
    }
    pub const fn active(self) -> bool {
        self.enabled && self.strength != 0
    }
    pub fn blend(self) -> f32 {
        if self.active() {
            f32::from(self.strength) / 100.0
        } else {
            0.0
        }
    }
}

impl std::str::FromStr for Equalization {
    type Err = MediaError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (enabled, strength) = value
            .split_once('/')
            .ok_or(MediaError::InvalidEqualization)?;
        let enabled = match enabled {
            "0" => false,
            "1" => true,
            _ => return Err(MediaError::InvalidEqualization),
        };
        Self::new(
            enabled,
            strength
                .parse()
                .map_err(|_| MediaError::InvalidEqualization)?,
        )
    }
}

/// Four raw mapping slots, current plus at most three predecessors <100 ms old.
/// A duplicate source PTS reuses statistics and does not advance the ring.
#[derive(Clone, Debug, Default)]
pub struct MappingHistory {
    timestamps: [Option<i64>; HISTORY],
    latest: Option<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FramePlan {
    pub slot: u32,
    pub previous: u32,
    pub reset: bool,
    pub reuse: bool,
    pub weights: [f32; HISTORY],
}
impl MappingHistory {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn advance(&mut self, pts: i64) -> Result<FramePlan, MediaError> {
        if pts < 0 {
            return Err(MediaError::TimestampOverflow);
        }
        let last = self.latest.and_then(|i| self.timestamps[i]);
        let reuse = last == Some(pts);
        let reset = last.is_none_or(|last| pts < last || pts - last >= HISTORY_US);
        let previous = self.latest.unwrap_or(0);
        if reset {
            self.reset();
        }
        let slot = if reuse {
            previous
        } else {
            self.latest.map_or(0, |i| (i + 1) % HISTORY)
        };
        self.timestamps[slot] = Some(pts);
        self.latest = Some(slot);
        let weights = self.timestamps.map(|stamp| {
            stamp.map_or(0.0, |t| {
                let age = pts - t;
                if (0..HISTORY_US).contains(&age) {
                    (HISTORY_US - age) as f32 / HISTORY_US as f32
                } else {
                    0.0
                }
            })
        });
        Ok(FramePlan {
            slot: slot as u32,
            previous: previous as u32,
            reset,
            reuse,
            weights,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_policy_round_trips_and_rejects_malformed_values() {
        for policy in [
            Equalization::default(),
            Equalization::new(true, 0).unwrap(),
            Equalization::new(true, 73).unwrap(),
        ] {
            assert_eq!(policy.command().parse::<Equalization>().unwrap(), policy);
        }
        for command in [
            "2/50", "1/101", "0/101", "1/-1", "1/50.5", "1/NaN", "1", "1/50/0",
        ] {
            assert!(command.parse::<Equalization>().is_err(), "{command}");
        }
    }
    #[test]
    fn finite_causal_history_handles_vfr_duplicates_seek_and_gap() {
        let mut history = MappingHistory::default();
        assert!(history.advance(1_000).unwrap().reset);
        let second = history.advance(21_000).unwrap();
        assert_eq!(second.weights, [0.8, 1., 0., 0.]);
        let duplicate = history.advance(21_000).unwrap();
        assert!(duplicate.reuse);
        assert_eq!(duplicate.slot, second.slot);
        history.advance(51_000).unwrap();
        history.advance(71_000).unwrap();
        let fifth = history.advance(91_000).unwrap();
        assert_eq!(fifth.slot, 0);
        assert_eq!(fifth.weights, [1., 0.3, 0.6, 0.8]);
        let seek = history.advance(10_000).unwrap();
        assert!(seek.reset);
        assert_eq!(seek.weights, [1., 0., 0., 0.]);
        assert!(history.advance(110_000).unwrap().reset);
        assert!(history.advance(-1).is_err());
        assert!(!history.advance(i64::MAX).unwrap().reuse);
    }
    #[test]
    fn settings_are_independent_and_zero_strength_bypasses() {
        assert!(!Equalization::default().active());
        assert!(!Equalization::new(true, 0).unwrap().active());
        assert_eq!(Equalization::new(true, 50).unwrap().blend(), 0.5);
        assert!(Equalization::new(false, 101).is_err());
    }

    #[test]
    fn serialized_settings_validate_even_when_disabled() {
        for payload in [
            r#"{"enabled":false,"strength":101}"#,
            r#"{"enabled":true,"strength":-1}"#,
            r#"{"enabled":true,"strength":50.5}"#,
        ] {
            assert!(serde_json::from_str::<Equalization>(payload).is_err());
        }
        let value = Equalization::new(true, 67).unwrap();
        assert_eq!(
            serde_json::from_str::<Equalization>(&serde_json::to_string(&value).unwrap()).unwrap(),
            value
        );
    }

    #[test]
    fn shared_output_settings_default_legacy_equalization_and_preserve_snapshot() {
        let legacy = r#"{"bitrate":"Recommended","frame_rate":"Original"}"#;
        let mut settings: crate::VideoSettings = serde_json::from_str(legacy).unwrap();
        assert_eq!(settings.equalization, Equalization::default());
        settings.equalization = Equalization::new(true, 73).unwrap();
        let encoded = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<crate::VideoSettings>(&encoded).unwrap(),
            settings
        );
        assert!(
            serde_json::from_str::<crate::VideoSettings>(&encoded.replace("73", "101")).is_err()
        );
    }
}
