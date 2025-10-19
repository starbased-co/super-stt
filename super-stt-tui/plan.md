# Super STT UDP Client Protocol - Complete Guide

Perfect for building a TUI with React and Ink! Here's everything you need to implement a UDP client.

## 🔌 Protocol Overview

The daemon broadcasts real-time audio visualization data via **UDP** on **127.0.0.1:8765**. Clients receive:

- **Frequency band data** (for visualizations)
- **Raw audio samples** (for custom DSP)
- **Recording state** (idle/recording/processing)
- **Partial transcriptions** (real-time STT)
- **Final transcriptions** (completed STT)

## 🔐 Authentication Flow

### 1. Read Shared Secret

The daemon creates a shared secret file at:

```
$XDG_RUNTIME_DIR/stt/udp_secret
```

**auth.rs:28-48**

```typescript
import { readFileSync } from "fs";
import { join } from "path";

function getSecretPath(): string {
  const runtimeDir =
    process.env.XDG_RUNTIME_DIR || process.env.TMPDIR || "/tmp";
  return join(runtimeDir, "stt", "udp_secret");
}

function readSecret(): string {
  return readFileSync(getSecretPath(), "utf-8").trim();
}
```

### 2. Register with Daemon

**lib.rs:229-252**

Send registration message with format: `REGISTER:client_type:secret`

```typescript
import dgram from "dgram";

const socket = dgram.createSocket("udp4");

async function registerWithDaemon(clientType: string = "tui"): Promise<void> {
  const secret = readSecret();
  const registrationMsg = `REGISTER:${clientType}:${secret}`;

  await new Promise<void>((resolve, reject) => {
    socket.send(registrationMsg, 8765, "127.0.0.1", (err) =>
      err ? reject(err) : resolve(),
    );
  });

  console.log("✓ Registered with daemon");
}
```

### 3. Listen for Acknowledgment

**streamer.rs:376-377**

The daemon responds with: `REGISTERED:udp_client_{port}`

```typescript
socket.on("message", (msg: Buffer, rinfo) => {
  const data = msg.toString();

  if (data.startsWith("REGISTERED:")) {
    const clientId = data.split(":")[1];
    console.log(`✓ Authenticated: ${clientId}`);
  }
});
```

### 4. Keep-Alive (Important!)

**lib.rs:296-302**

Send `PING` every 60 seconds to prevent cleanup:

```typescript
setInterval(() => {
  socket.send("PING", 8765, "127.0.0.1", (err) => {
    if (err) console.error("Keep-alive failed:", err);
  });
}, 60_000); // 60 seconds
```

## 📦 Packet Structure

### Universal Header (11 bytes)

**udp.rs:14-43**

```
Byte 0      : packet_type (u8)
Bytes 1-4   : timestamp_ms (u32, little-endian)
Bytes 5-8   : client_id (u32, little-endian)
Bytes 9-10  : data_len (u16, little-endian)
```

### Packet Types

**udp.rs:5-9**

```typescript
const PARTIAL_STT_PACKET = 2;
const FINAL_STT_PACKET = 3;
const AUDIO_SAMPLES_PACKET = 4;
const RECORDING_STATE_PACKET = 5;
const FREQUENCY_BANDS_PACKET = 6;
```

## 🎨 Parsing Packet Types

### 1. Recording State Packet (Type 5)

**daemon_state.rs + udp_parsing.rs:110-146**

```
Total: 20 bytes
├─ Header: 11 bytes
└─ Data: 9 bytes
   ├─ is_recording (u8): 1 byte (0 = idle, 1 = recording)
   └─ timestamp_ms (u64): 8 bytes (little-endian)
```

```typescript
interface RecordingState {
  isRecording: boolean;
  timestampMs: bigint;
}

function parseRecordingState(buffer: Buffer): RecordingState | null {
  if (buffer.length < 20) return null;
  if (buffer[0] !== 5) return null; // Check packet type

  const data = buffer.slice(11); // Skip header

  return {
    isRecording: data[0] !== 0,
    timestampMs: data.readBigUInt64LE(1),
  };
}
```

### 2. Frequency Bands Packet (Type 6)

**audio.rs:45-77 + udp_parsing.rs:154-211**

**This is the most useful for TUI visualizations!**

```
Total: 23+ bytes
├─ Header: 11 bytes
└─ Data: 12+ bytes
   ├─ sample_rate (f32): 4 bytes
   ├─ total_energy (f32): 4 bytes  ← Use this for VU meter!
   ├─ num_bands (u32): 4 bytes
   └─ bands (f32[]): 4 bytes × num_bands
```

```typescript
interface FrequencyBands {
  sampleRate: number;
  totalEnergy: number; // Overall audio level (0.0-1.0)
  bands: number[]; // Per-frequency band levels
}

function parseFrequencyBands(buffer: Buffer): FrequencyBands | null {
  if (buffer.length < 23) return null;
  if (buffer[0] !== 6) return null;

  const data = buffer.slice(11);

  const sampleRate = data.readFloatLE(0);
  const totalEnergy = data.readFloatLE(4);
  const numBands = data.readUInt32LE(8);

  const bands: number[] = [];
  for (let i = 0; i < numBands; i++) {
    const offset = 12 + i * 4;
    bands.push(data.readFloatLE(offset));
  }

  return { sampleRate, totalEnergy, bands };
}
```

### 3. Audio Samples Packet (Type 4)

**audio.rs:11-43 + udp_parsing.rs:19-103**

```
Total: 21+ bytes
├─ Header: 11 bytes
└─ Data: 10+ bytes
   ├─ sample_rate (f32): 4 bytes
   ├─ channels (u16): 2 bytes
   ├─ num_samples (u32): 4 bytes
   └─ samples (f32[]): 4 bytes × num_samples
```

```typescript
interface AudioSamples {
  sampleRate: number;
  channels: number;
  samples: number[];
}

function parseAudioSamples(buffer: Buffer): AudioSamples | null {
  if (buffer.length < 21) return null;
  if (buffer[0] !== 4) return null;

  const data = buffer.slice(11);

  const sampleRate = data.readFloatLE(0);
  const channels = data.readUInt16LE(4);
  const numSamples = data.readUInt32LE(6);

  // Security: limit sample count (prevent DoS)
  const MAX_SAMPLES = 192_000;
  if (numSamples > MAX_SAMPLES) {
    console.error(`Sample count ${numSamples} exceeds max`);
    return null;
  }

  const samples: number[] = [];
  for (let i = 0; i < numSamples; i++) {
    const offset = 10 + i * 4;
    samples.push(data.readFloatLE(offset));
  }

  return { sampleRate, channels, samples };
}
```

### 4. Partial STT Packet (Type 2)

**stt.rs + streamer.rs:122-149**

```
Total: 15+ bytes
├─ Header: 11 bytes
└─ Data: 4+ bytes
   ├─ confidence (f32): 4 bytes
   └─ text (string): remaining bytes (UTF-8)
```

```typescript
interface STTResult {
  confidence: number;
  text: string;
}

function parsePartialSTT(buffer: Buffer): STTResult | null {
  if (buffer.length < 15) return null;
  if (buffer[0] !== 2) return null;

  const data = buffer.slice(11);

  const confidence = data.readFloatLE(0);
  const text = data.slice(4).toString("utf-8");

  return { confidence, text };
}
```

### 5. Final STT Packet (Type 3)

**Same structure as Partial STT, just different packet type**

```typescript
function parseFinalSTT(buffer: Buffer): STTResult | null {
  if (buffer.length < 15) return null;
  if (buffer[0] !== 3) return null; // Changed type check

  const data = buffer.slice(11);
  return {
    confidence: data.readFloatLE(0),
    text: data.slice(4).toString("utf-8"),
  };
}
```

## 🚀 Complete React Ink TUI Example

```typescript
import React, { useState, useEffect } from 'react';
import { render, Box, Text } from 'ink';
import dgram from 'dgram';
import { readFileSync } from 'fs';
import { join } from 'path';

const App = () => {
  const [isRecording, setIsRecording] = useState(false);
  const [audioLevel, setAudioLevel] = useState(0);
  const [freqBands, setFreqBands] = useState<number[]>([]);
  const [transcription, setTranscription] = useState('');
  const [partialText, setPartialText] = useState('');

  useEffect(() => {
    const socket = dgram.createSocket('udp4');

    // 1. Register with daemon
    const secret = readFileSync(
      join(process.env.XDG_RUNTIME_DIR || '/tmp', 'stt', 'udp_secret'),
      'utf-8'
    ).trim();

    socket.send(`REGISTER:tui:${secret}`, 8765, '127.0.0.1');

    // 2. Set up keep-alive
    const keepAlive = setInterval(() => {
      socket.send('PING', 8765, '127.0.0.1');
    }, 60_000);

    // 3. Handle incoming packets
    socket.on('message', (msg: Buffer) => {
      const packetType = msg[0];

      switch (packetType) {
        case 5: // Recording state
          const recState = parseRecordingState(msg);
          if (recState) {
            setIsRecording(recState.isRecording);
            if (!recState.isRecording) {
              setPartialText(''); // Clear on stop
            }
          }
          break;

        case 6: // Frequency bands
          const bands = parseFrequencyBands(msg);
          if (bands) {
            setAudioLevel(bands.totalEnergy);
            setFreqBands(bands.bands);
          }
          break;

        case 2: // Partial STT
          const partial = parsePartialSTT(msg);
          if (partial) setPartialText(partial.text);
          break;

        case 3: // Final STT
          const final = parseFinalSTT(msg);
          if (final) {
            setTranscription(final.text);
            setPartialText('');
          }
          break;
      }
    });

    return () => {
      clearInterval(keepAlive);
      socket.close();
    };
  }, []);

  // Render visualization
  const bars = freqBands.map((level, i) => {
    const height = Math.floor(level * 10);
    return '█'.repeat(height).padEnd(10, ' ');
  });

  return (
    <Box flexDirection="column" padding={1}>
      <Text bold color="cyan">
        Super STT TUI Client
      </Text>

      <Box marginTop={1}>
        <Text>
          Status: {isRecording ? '🔴 RECORDING' : '⚪ Idle'}
        </Text>
      </Box>

      <Box marginTop={1}>
        <Text>
          Audio Level: {'█'.repeat(Math.floor(audioLevel * 50))}
          {' '}
          {(audioLevel * 100).toFixed(0)}%
        </Text>
      </Box>

      {freqBands.length > 0 && (
        <Box marginTop={1} flexDirection="column">
          <Text bold>Frequency Spectrum:</Text>
          {bars.map((bar, i) => (
            <Text key={i}>{bar}</Text>
          ))}
        </Box>
      )}

      {partialText && (
        <Box marginTop={1}>
          <Text dimColor>Transcribing: {partialText}...</Text>
        </Box>
      )}

      {transcription && (
        <Box marginTop={1}>
          <Text bold color="green">
            Result: {transcription}
          </Text>
        </Box>
      )}
    </Box>
  );
};

render(<App />);
```

## 🎯 Key Implementation Notes

1. **Always read the secret file** - Don't hardcode secrets
2. **Send keep-alive pings** - Every 60s or you'll be disconnected after 5 minutes
3. **Handle packet type 6 (frequency bands)** - Best for visualizations
4. **Use `totalEnergy`** - Already computed RMS audio level
5. **Rate limit packet processing** - Consider dropping packets if overwhelmed
6. **Security limits** - Validate packet sizes (max 8192 bytes, max 192k samples)

## 📊 Visualization Ideas for TUI

### Using Frequency Bands (Recommended)

- **Equalizer bars** - Map each band to a vertical bar
- **Waveform** - Plot band values horizontally
- **VU meter** - Use `totalEnergy` for classic needle meter
- **ASCII spectrogram** - Time-series of bands scrolling up

### Using Raw Samples (Advanced)

- **Oscilloscope** - Plot raw waveform
- **Custom FFT** - Compute your own frequency analysis
- **Custom filters** - Apply DSP before visualization

## 🔧 Testing Without Recording

The daemon continuously broadcasts frequency bands even when idle (for responsive UI). You'll see:

- **Low total_energy** (~0.0-0.01) when quiet
- **Higher values** (>0.02) during speech
- **Recording state changes** when daemon processes audio

Start the daemon, run your TUI, and you'll immediately see data flowing!
