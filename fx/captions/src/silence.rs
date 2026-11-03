//! The silence threshold of `captions gen` (owner's decision 2026-11-03): `auto` — the
//! recording's own (`dc_audio::silence::threshold`, halfway between its noise and its speech),
//! or a level in dBFS set by hand. Words wholly inside a silence of 1.5 s are dropped and cues
//! end where a silence starts (`dc_transcribe::cues`).

use dc_core::{CommandError, within};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The lowest and the highest threshold set by hand, dBFS.
pub const SILENCE_MIN: f64 = -70.0;
pub const SILENCE_MAX: f64 = -20.0;

/// Silence threshold of the recognition: `auto` or dBFS, −70..−20.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Silence {
    /// `auto`: the threshold of each recording, from its own levels.
    Auto(Auto),
    /// Quieter than this, dBFS, is silence.
    Db(f64),
}

/// The word `auto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Auto {
    /// Each recording's own threshold.
    #[serde(rename = "auto")]
    Auto,
}

impl Default for Silence {
    fn default() -> Self {
        Self::AUTO
    }
}

impl Silence {
    pub const AUTO: Self = Self::Auto(Auto::Auto);

    /// The threshold set by hand; none — each recording's own.
    pub fn db(self) -> Option<f64> {
        match self {
            Self::Auto(_) => None,
            Self::Db(db) => Some(db),
        }
    }

    /// `auto` or `-45 dB`, for the answers.
    pub fn name(self) -> String {
        match self {
            Self::Auto(_) => "auto".to_owned(),
            Self::Db(db) => format!("{db} dB"),
        }
    }

    pub(crate) fn checked(self) -> Result<Self, CommandError> {
        match self {
            Self::Auto(_) => Ok(self),
            Self::Db(db) => Ok(Self::Db(within(
                "порог тишины субтитров (dBFS; auto — по записи)",
                db,
                SILENCE_MIN,
                SILENCE_MAX,
            )?)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_and_decibels_read_and_write_as_json() {
        assert_eq!(serde_json::to_value(Silence::AUTO).unwrap(), "auto");
        assert_eq!(serde_json::to_value(Silence::Db(-45.0)).unwrap(), -45.0);
        let read = |v: serde_json::Value| serde_json::from_value::<Silence>(v).ok();
        assert_eq!(read("auto".into()), Some(Silence::AUTO));
        assert_eq!(read((-45).into()), Some(Silence::Db(-45.0)));
        assert_eq!(read("тихо".into()), None);
    }

    #[test]
    fn a_level_by_hand_is_within_the_range() {
        assert!(Silence::Db(-10.0).checked().is_err());
        assert!(Silence::Db(-80.0).checked().is_err());
        assert_eq!(Silence::Db(-45.0).checked().unwrap().db(), Some(-45.0));
        assert_eq!(Silence::AUTO.checked().unwrap().db(), None);
    }
}
