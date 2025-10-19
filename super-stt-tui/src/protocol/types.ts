/**
 * Type definitions for Super STT UDP protocol packets
 */

export const PARTIAL_STT_PACKET = 2;
export const FINAL_STT_PACKET = 3;
export const AUDIO_SAMPLES_PACKET = 4;
export const RECORDING_STATE_PACKET = 5;
export const FREQUENCY_BANDS_PACKET = 6;

export type PacketType =
  | typeof PARTIAL_STT_PACKET
  | typeof FINAL_STT_PACKET
  | typeof AUDIO_SAMPLES_PACKET
  | typeof RECORDING_STATE_PACKET
  | typeof FREQUENCY_BANDS_PACKET;

/**
 * Universal packet header (11 bytes total)
 */
export interface PacketHeader {
  packetType: PacketType;
  timestampMs: number;
  clientId: number;
  dataLen: number;
}

/**
 * Recording state data (Type 5)
 * Total: 20 bytes (11 header + 9 data)
 */
export interface RecordingState {
  isRecording: boolean;
  timestampMs: bigint;
}

/**
 * Frequency bands data (Type 6)
 * Total: 23+ bytes (11 header + 12+ data)
 * This is the primary visualization data source
 */
export interface FrequencyBands {
  sampleRate: number;
  totalEnergy: number; // Overall audio level (0.0-1.0), use for VU meter
  bands: number[]; // Per-frequency band levels
}

/**
 * Raw audio samples data (Type 4)
 * Total: 21+ bytes (11 header + 10+ data)
 */
export interface AudioSamples {
  sampleRate: number;
  channels: number;
  samples: number[];
}

/**
 * Speech-to-text result (Types 2 & 3)
 * Total: 15+ bytes (11 header + 4+ data)
 */
export interface STTResult {
  confidence: number;
  text: string;
}

/**
 * Security limits for packet validation
 */
export const MAX_PACKET_SIZE = 8192; // Maximum packet size in bytes
export const MAX_SAMPLES = 192_000; // Maximum audio samples per packet
export const MAX_FREQUENCY_BANDS = 64; // Reasonable limit for frequency bands
