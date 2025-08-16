// SPDX-License-Identifier: GPL-3.0-only
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumCount, EnumIter, VariantArray, VariantNames};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Serialize,
    Deserialize,
    AsRefStr,
    EnumCount,
    EnumIter,
    VariantArray,
    VariantNames,
    Default,
)]
pub enum AudioTheme {
    #[default]
    Classic, // Original ascending/descending beeps
    Gentle,  // Soft, pleasant tones
    Minimal, // Single short beeps
    SciFi,   // Futuristic computer sounds
    Musical, // Musical chord progressions
    Nature,  // Natural, organic sounds
    Retro,   // 8-bit style sounds
    Silent,  // No audio feedback
}

impl std::fmt::Display for AudioTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioTheme::Classic => write!(f, "classic"),
            AudioTheme::Gentle => write!(f, "gentle"),
            AudioTheme::Minimal => write!(f, "minimal"),
            AudioTheme::SciFi => write!(f, "scifi"),
            AudioTheme::Musical => write!(f, "musical"),
            AudioTheme::Nature => write!(f, "nature"),
            AudioTheme::Retro => write!(f, "retro"),
            AudioTheme::Silent => write!(f, "silent"),
        }
    }
}

impl AudioTheme {
    // Additional helpers below

    #[must_use]
    pub fn pretty_name(&self) -> String {
        match self {
            AudioTheme::Classic => "Classic".to_string(),
            AudioTheme::Gentle => "Gentle".to_string(),
            AudioTheme::Minimal => "Minimal".to_string(),
            AudioTheme::SciFi => "Sci-Fi".to_string(),
            AudioTheme::Musical => "Musical".to_string(),
            AudioTheme::Nature => "Nature".to_string(),
            AudioTheme::Retro => "Retro".to_string(),
            AudioTheme::Silent => "Silent".to_string(),
        }
    }

    /// Get all available audio themes
    #[must_use]
    pub fn all_themes() -> Vec<AudioTheme> {
        AudioTheme::iter().collect()
    }

    /// Get start sound frequencies and timings for this theme
    #[must_use]
    pub fn start_sound(&self) -> (Vec<f32>, u64) {
        match self {
            AudioTheme::Classic => (vec![440.0, 554.0, 659.0], 150),
            AudioTheme::Gentle => (vec![261.6, 329.6, 392.0], 200),
            AudioTheme::Minimal => (vec![800.0], 100),
            AudioTheme::SciFi => (vec![400.0, 600.0, 1000.0, 1400.0], 150),
            AudioTheme::Musical => (vec![261.6, 329.6, 392.0, 523.3], 180),
            AudioTheme::Nature => (vec![174.6, 220.0, 261.6], 250),
            AudioTheme::Retro => (vec![659.3, 1318.5, 1975.5], 120),
            AudioTheme::Silent => (vec![], 0), // No sound, no duration
        }
    }

    /// Get end sound frequencies and timings for this theme
    #[must_use]
    pub fn end_sound(&self) -> (Vec<f32>, u64) {
        match self {
            AudioTheme::Classic => (vec![659.0, 554.0, 440.0], 150),
            AudioTheme::Gentle => (vec![392.0, 329.6, 261.6], 200),
            AudioTheme::Minimal => (vec![600.0], 100),
            AudioTheme::SciFi => (vec![1400.0, 1000.0, 600.0, 400.0], 150),
            AudioTheme::Musical => (vec![523.3, 392.0, 329.6, 261.6], 180),
            AudioTheme::Nature => (vec![261.6, 220.0, 174.6], 250),
            AudioTheme::Retro => (vec![1975.5, 1318.5, 659.3], 120),
            AudioTheme::Silent => (vec![], 0), // No sound, no duration
        }
    }
}

impl std::str::FromStr for AudioTheme {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let theme = match s.to_lowercase().as_str() {
            "classic" => AudioTheme::Classic,
            "gentle" => AudioTheme::Gentle,
            "minimal" => AudioTheme::Minimal,
            "scifi" => AudioTheme::SciFi,
            "musical" => AudioTheme::Musical,
            "nature" => AudioTheme::Nature,
            "retro" => AudioTheme::Retro,
            "silent" => AudioTheme::Silent,
            _ => AudioTheme::default(),
        };
        Ok(theme)
    }
}
