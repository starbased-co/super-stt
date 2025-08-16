// SPDX-License-Identifier: GPL-3.0-only
#[derive(Debug, Clone)]
pub struct STTData {
    pub text: String,
    pub confidence: f32,
}

impl STTData {
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.confidence.to_le_bytes());
        bytes.extend_from_slice(self.text.as_bytes());
        bytes
    }
}
