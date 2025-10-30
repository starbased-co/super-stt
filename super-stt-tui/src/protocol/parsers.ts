/**
 * Binary packet parsers for Super STT UDP protocol
 *
 * All parsers validate packet sizes and types before attempting to parse data.
 * Invalid packets return null instead of throwing to prevent UI crashes.
 */

import {
  PARTIAL_STT_PACKET,
  FINAL_STT_PACKET,
  AUDIO_SAMPLES_PACKET,
  RECORDING_STATE_PACKET,
  FREQUENCY_BANDS_PACKET,
  MAX_PACKET_SIZE,
  MAX_SAMPLES,
  MAX_FREQUENCY_BANDS,
  type PacketType,
  type PacketHeader,
  type RecordingState,
  type FrequencyBands,
  type AudioSamples,
  type STTResult,
} from './types.js';

/**
 * Parse universal packet header (11 bytes)
 */
export function parseHeader(buffer: Buffer): PacketHeader | null {
  if (buffer.length < 11) return null;

  return {
    packetType: buffer[0] as PacketType,
    timestampMs: buffer.readUInt32LE(1),
    clientId: buffer.readUInt32LE(5),
    dataLen: buffer.readUInt16LE(9),
  };
}

/**
 * Parse Recording State packet (Type 5)
 * Total: 20 bytes (11 header + 9 data)
 */
export function parseRecordingState(buffer: Buffer): RecordingState | null {
  if (buffer.length < 20) {
    console.warn('Recording state packet too small:', buffer.length);
    return null;
  }

  if (buffer[0] !== RECORDING_STATE_PACKET) {
    console.warn('Invalid packet type for recording state:', buffer[0]);
    return null;
  }

  const data = buffer.slice(11);

  return {
    isRecording: data[0] !== 0,
    timestampMs: data.readBigUInt64LE(1),
  };
}

/**
 * Parse Frequency Bands packet (Type 6)
 * Total: 23+ bytes (11 header + 12+ data)
 *
 * This is the primary data source for visualizations:
 * - Use totalEnergy for VU meter
 * - Use bands array for equalizer/spectrum display
 */
export function parseFrequencyBands(buffer: Buffer): FrequencyBands | null {
  if (buffer.length < 23) {
    console.warn('Frequency bands packet too small:', buffer.length);
    return null;
  }

  if (buffer[0] !== FREQUENCY_BANDS_PACKET) {
    console.warn('Invalid packet type for frequency bands:', buffer[0]);
    return null;
  }

  if (buffer.length > MAX_PACKET_SIZE) {
    console.error('Packet exceeds maximum size:', buffer.length);
    return null;
  }

  const data = buffer.slice(11);

  const sampleRate = data.readFloatLE(0);
  const totalEnergy = data.readFloatLE(4);
  const numBands = data.readUInt32LE(8);

  // Security: validate band count
  if (numBands > MAX_FREQUENCY_BANDS) {
    console.error(`Band count ${numBands} exceeds maximum ${MAX_FREQUENCY_BANDS}`);
    return null;
  }

  // Validate we have enough bytes for all bands
  const expectedSize = 12 + numBands * 4;
  if (data.length < expectedSize) {
    console.warn(
      `Insufficient data for ${numBands} bands: got ${data.length}, need ${expectedSize}`,
    );
    return null;
  }

  const bands: number[] = [];
  for (let i = 0; i < numBands; i++) {
    const offset = 12 + i * 4;
    bands.push(data.readFloatLE(offset));
  }

  return { sampleRate, totalEnergy, bands };
}

/**
 * Parse Audio Samples packet (Type 4)
 * Total: 21+ bytes (11 header + 10+ data)
 */
export function parseAudioSamples(buffer: Buffer): AudioSamples | null {
  if (buffer.length < 21) {
    console.warn('Audio samples packet too small:', buffer.length);
    return null;
  }

  if (buffer[0] !== AUDIO_SAMPLES_PACKET) {
    console.warn('Invalid packet type for audio samples:', buffer[0]);
    return null;
  }

  if (buffer.length > MAX_PACKET_SIZE) {
    console.error('Packet exceeds maximum size:', buffer.length);
    return null;
  }

  const data = buffer.slice(11);

  const sampleRate = data.readFloatLE(0);
  const channels = data.readUInt16LE(4);
  const numSamples = data.readUInt32LE(6);

  // Security: limit sample count to prevent DoS
  if (numSamples > MAX_SAMPLES) {
    console.error(`Sample count ${numSamples} exceeds maximum ${MAX_SAMPLES}`);
    return null;
  }

  // Validate we have enough bytes for all samples
  const expectedSize = 10 + numSamples * 4;
  if (data.length < expectedSize) {
    console.warn(
      `Insufficient data for ${numSamples} samples: got ${data.length}, need ${expectedSize}`,
    );
    return null;
  }

  const samples: number[] = [];
  for (let i = 0; i < numSamples; i++) {
    const offset = 10 + i * 4;
    samples.push(data.readFloatLE(offset));
  }

  return { sampleRate, channels, samples };
}

/**
 * Parse Partial STT packet (Type 2)
 * Total: 15+ bytes (11 header + 4+ data)
 */
export function parsePartialSTT(buffer: Buffer): STTResult | null {
  if (buffer.length < 15) {
    console.warn('Partial STT packet too small:', buffer.length);
    return null;
  }

  if (buffer[0] !== PARTIAL_STT_PACKET) {
    console.warn('Invalid packet type for partial STT:', buffer[0]);
    return null;
  }

  const data = buffer.slice(11);

  const confidence = data.readFloatLE(0);
  const text = data.slice(4).toString('utf-8');

  console.log('[parsePartialSTT] Parsed:', {
    text,
    textLength: text.length,
    confidence,
    bufferLength: buffer.length,
  });

  return { confidence, text };
}

/**
 * Parse Final STT packet (Type 3)
 * Total: 15+ bytes (11 header + 4+ data)
 * Same structure as Partial STT, different packet type
 */
export function parseFinalSTT(buffer: Buffer): STTResult | null {
  if (buffer.length < 15) {
    console.warn('Final STT packet too small:', buffer.length);
    return null;
  }

  if (buffer[0] !== FINAL_STT_PACKET) {
    console.warn('Invalid packet type for final STT:', buffer[0]);
    return null;
  }

  const data = buffer.slice(11);

  const confidence = data.readFloatLE(0);
  const text = data.slice(4).toString('utf-8');

  console.log('[parseFinalSTT] Parsed:', {
    text,
    textLength: text.length,
    confidence,
    bufferLength: buffer.length,
  });

  return { confidence, text };
}

/**
 * Parse any packet and return its type and parsed data
 */
export function parsePacket(
  buffer: Buffer,
):
  | { type: 'recording_state'; data: RecordingState }
  | { type: 'frequency_bands'; data: FrequencyBands }
  | { type: 'audio_samples'; data: AudioSamples }
  | { type: 'partial_stt'; data: STTResult }
  | { type: 'final_stt'; data: STTResult }
  | { type: 'unknown'; data: null }
  | null {
  if (buffer.length === 0) return null;

  const packetType = buffer[0];

  switch (packetType) {
    case RECORDING_STATE_PACKET: {
      const data = parseRecordingState(buffer);
      return data ? { type: 'recording_state', data } : null;
    }
    case FREQUENCY_BANDS_PACKET: {
      const data = parseFrequencyBands(buffer);
      return data ? { type: 'frequency_bands', data } : null;
    }
    case AUDIO_SAMPLES_PACKET: {
      const data = parseAudioSamples(buffer);
      return data ? { type: 'audio_samples', data } : null;
    }
    case PARTIAL_STT_PACKET: {
      const data = parsePartialSTT(buffer);
      return data ? { type: 'partial_stt', data } : null;
    }
    case FINAL_STT_PACKET: {
      const data = parseFinalSTT(buffer);
      return data ? { type: 'final_stt', data } : null;
    }
    default:
      console.warn('Unknown packet type:', packetType);
      return { type: 'unknown', data: null };
  }
}
