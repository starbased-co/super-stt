// SPDX-License-Identifier: GPL-3.0-only

//! Networking functionality for the Super STT application.

use crate::state::{AudioLevelData, RecordingStatus};

/// Convert raw audio energy level to decibels (dB)
/// This matches how professional audio software and system settings display volume
/// Adjusted for frequency band energy values which are typically 0.003-0.01 during speech
fn raw_level_to_db(raw_level: f32) -> f32 {
    if raw_level <= 0.0 {
        -60.0 // Floor at -60dB (standard noise floor for digital audio)
    } else {
        // Scale the input to handle frequency band energy range properly
        // Based on observed values: 0.003-0.008 should map to a good range
        // Use much smaller scaling factor to avoid hitting the ceiling
        let scaled_input = raw_level * 10.0; // Reduced from 1000x to 10x

        // Convert to dB: 20 * log10(scaled_level)
        // This maps (based on observed data):
        // - 0.003 (quiet speech) -> ~-10dB -> 83%
        // - 0.005 (normal speech) -> ~-6dB -> 90%
        // - 0.008 (loud speech) -> ~-2dB -> 97%
        // - 0.01+ (very loud) -> ~0dB -> 100%
        (20.0 * scaled_input.log10()).clamp(-60.0, 0.0)
    }
}

/// Convert dB level to display percentage for progress bar
/// Maps the useful dB range (-60dB to 0dB) to 0% to 100% display
/// This is how Windows/macOS/Linux audio settings work
fn db_to_display_percent(db_level: f32) -> f32 {
    // Standard audio meter range: -60dB (silence) to 0dB (digital maximum)
    // Map this to 0.0-1.0 for progress bar display
    ((db_level + 60.0) / 60.0).clamp(0.0, 1.0)
}

/// Combined function: raw audio level -> dB -> display percentage
/// This is the professional way to handle audio level metering
fn raw_level_to_db_display_percent(raw_level: f32) -> f32 {
    let db_level = raw_level_to_db(raw_level);
    db_to_display_percent(db_level)
}

/// Parse UDP packet containing audio level data from daemon
pub fn parse_audio_level_from_udp(data: &[u8]) -> AudioLevelData {
    // Check if this is a text-based registration response or control message
    if let Ok(text) = std::str::from_utf8(data) {
        if text.starts_with("REGISTER") || text.starts_with("OK") || text.starts_with("PONG") {
            // This is a control message, not audio data - ignore for audio level
            return AudioLevelData {
                level: 0.0,
                is_speech: false,
            };
        }
    }

    // Try parsing as frequency bands first (more common from daemon)
    if let Ok(frequency_data) = super_stt_shared::parse_frequency_bands_from_udp(data) {
        // Scale the total energy for better visibility in progress bar
        // Raw values are typically 0.00001-0.1, scale to 0.0-1.0 range
        let raw_level = frequency_data.total_energy;

        // Professional dB-based volume metering like system audio settings
        // Convert raw energy to decibels, then map to display percentage
        let scaled_level = raw_level_to_db_display_percent(raw_level);

        let is_speech = raw_level > 0.0001; // Lower threshold for speech detection

        return AudioLevelData {
            level: scaled_level,
            is_speech,
        };
    }

    // Fallback to audio samples parsing
    if let Ok(audio_data) = super_stt_shared::parse_audio_samples_from_udp(data) {
        // Calculate RMS level from samples (more accurate than average)
        let raw_level = if audio_data.samples.is_empty() {
            0.0
        } else {
            #[allow(clippy::cast_precision_loss)]
            let rms: f32 = audio_data.samples.iter().map(|&s| s * s).sum::<f32>()
                / audio_data.samples.len() as f32;
            rms.sqrt()
        };

        // Apply same professional dB-based metering for RMS audio samples
        // RMS values from audio samples are typically higher, so scale them appropriately
        let adjusted_level = raw_level * 0.1; // Scale RMS to frequency bands range
        let scaled_level = raw_level_to_db_display_percent(adjusted_level);

        // Simple speech detection based on level threshold
        let is_speech = raw_level > 0.01;

        return AudioLevelData {
            level: scaled_level,
            is_speech,
        };
    }

    // If we can't parse the packet, provide a default audio level
    // This prevents the app from failing completely
    AudioLevelData {
        level: 0.0,
        is_speech: false,
    }
}

/// Parse UDP packet containing recording state from daemon
pub fn parse_recording_state_from_udp(data: &[u8]) -> Option<RecordingStatus> {
    match super_stt_shared::parse_recording_state_from_udp(data) {
        Ok(state_data) => {
            if state_data.is_recording {
                Some(RecordingStatus::Recording)
            } else {
                Some(RecordingStatus::Idle)
            }
        }
        Err(_) => None,
    }
}
