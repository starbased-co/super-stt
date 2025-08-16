// SPDX-License-Identifier: GPL-3.0-only
#[derive(Debug, Clone)]
pub struct RecordingStateData {
    pub is_recording: bool,
    pub timestamp_ms: u64,
}

impl RecordingStateData {
    #[must_use]
    pub fn new(is_recording: bool) -> Self {
        Self {
            is_recording,
            timestamp_ms: u64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
            )
            .unwrap_or(u64::MAX),
        }
    }

    #[must_use]
    pub fn to_bytes(&self) -> [u8; 9] {
        let mut bytes = [0u8; 9];
        bytes[0] = u8::from(self.is_recording);
        bytes[1..9].copy_from_slice(&self.timestamp_ms.to_le_bytes());
        bytes
    }
}
