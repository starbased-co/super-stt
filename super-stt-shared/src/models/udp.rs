// SPDX-License-Identifier: GPL-3.0-only
use tokio::time::Instant;

// UDP packet types
pub const PARTIAL_STT_PACKET: u8 = 2;
pub const FINAL_STT_PACKET: u8 = 3;
pub const AUDIO_SAMPLES_PACKET: u8 = 4;
pub const RECORDING_STATE_PACKET: u8 = 5;
pub const FREQUENCY_BANDS_PACKET: u8 = 6;

// Maximum packet size for UDP
pub const MAX_PACKET_SIZE: usize = 1400;

#[repr(C, packed)]
#[derive(Debug, Clone)]
pub struct PacketHeader {
    pub packet_type: u8,
    pub timestamp_ms: u32,
    pub client_id: u32,
    pub data_len: u16,
}

impl PacketHeader {
    #[must_use]
    pub fn new(packet_type: u8, client_id: u32, data_len: u16) -> Self {
        Self {
            packet_type,
            timestamp_ms: u32::try_from(Instant::now().elapsed().as_millis()).unwrap_or(u32::MAX),
            client_id,
            data_len,
        }
    }

    #[must_use]
    pub fn to_bytes(&self) -> [u8; 11] {
        let mut bytes = [0u8; 11];
        bytes[0] = self.packet_type;
        bytes[1..5].copy_from_slice(&self.timestamp_ms.to_le_bytes());
        bytes[5..9].copy_from_slice(&self.client_id.to_le_bytes());
        bytes[9..11].copy_from_slice(&self.data_len.to_le_bytes());
        bytes
    }
}
