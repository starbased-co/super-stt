// SPDX-License-Identifier: GPL-3.0-only
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct AudioLevel {
    pub level: f32,
    pub is_speech: bool,
    pub timestamp: Instant,
}

#[derive(Debug, Clone)]
pub struct AudioSamplesData {
    pub samples: Vec<f32>,
    pub sample_rate: f32,
    pub channels: u16,
}

impl AudioSamplesData {
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Sample rate (4 bytes)
        bytes.extend_from_slice(&self.sample_rate.to_le_bytes());

        // Number of channels (2 bytes)
        bytes.extend_from_slice(&self.channels.to_le_bytes());

        // Number of samples (4 bytes)
        bytes.extend_from_slice(
            &u32::try_from(self.samples.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );

        // Sample data (4 bytes per sample)
        for sample in &self.samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }

        bytes
    }
}

#[derive(Debug, Clone)]
pub struct FrequencyBandsData {
    pub bands: Vec<f32>,
    pub sample_rate: f32,
    pub total_energy: f32,
}

impl FrequencyBandsData {
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Sample rate (4 bytes)
        bytes.extend_from_slice(&self.sample_rate.to_le_bytes());

        // Total energy (4 bytes)
        bytes.extend_from_slice(&self.total_energy.to_le_bytes());

        // Number of bands (4 bytes)
        bytes.extend_from_slice(
            &u32::try_from(self.bands.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );

        // Band data (4 bytes per band)
        for band in &self.bands {
            bytes.extend_from_slice(&band.to_le_bytes());
        }

        bytes
    }
}
