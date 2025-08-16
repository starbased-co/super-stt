// SPDX-License-Identifier: GPL-3.0-only

//! Audio processing and networking module.

pub mod networking;

pub use networking::{parse_audio_level_from_udp, parse_recording_state_from_udp};
