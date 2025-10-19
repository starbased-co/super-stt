# Super STT TUI - Native Module API

Rust N-API bindings for high-performance audio capture and daemon communication.

## Overview

This module provides three main components:

1. **UdpClient** - UDP client for receiving visualization data from daemon
2. **AudioCapture** - Real-time microphone capture using cpal
3. **Unix Socket Communication** - Send real-time transcription commands to daemon

## Building

```bash
# Debug build
pnpm build:native:debug

# Release build
pnpm build:native
```

## API Reference

### UdpClient

Manages UDP connection to daemon for receiving visualization data.

#### Constructor

```typescript
const client = new UdpClient();
```

#### Methods

##### `connect(clientType: string): Promise<string>`

Connect to daemon and register client.

**Parameters:**
- `clientType` - Client identifier (e.g., "tui")

**Returns:** Promise resolving to registration response (e.g., "REGISTERED:udp_client_1")

**Example:**
```typescript
const response = await client.connect('tui');
console.log(response); // "REGISTERED:udp_client_1"
```

##### `receivePacket(): Promise<Buffer>`

Receive next packet from daemon (blocking).

**Returns:** Promise resolving to packet buffer (max 8192 bytes)

**Example:**
```typescript
const packet = await client.receivePacket();
// Parse packet using protocol parsers
```

##### `sendPing(): Promise<void>`

Send keep-alive ping to daemon.

**Returns:** Promise resolving when ping is sent

##### `disconnect(): void`

Close connection and cleanup resources.

##### `isConnected(): boolean`

Check connection status.

**Returns:** `true` if connected, `false` otherwise

##### `getClientId(): string | null`

Get registered client ID.

**Returns:** Client ID string or `null` if not connected

##### `startRealtimeTranscription(socketPath?: string): Promise<void>`

Initiate real-time transcription session with daemon.

**Parameters:**
- `socketPath` - Optional Unix socket path (defaults to `/run/user/<uid>/stt/super-stt.sock`)

**Returns:** Promise resolving when session is started

**Example:**
```typescript
await client.startRealtimeTranscription();
// Now ready to send audio chunks
```

##### `sendAudioChunk(audioData: Float32Array, socketPath?: string): Promise<void>`

Send audio chunk to daemon for real-time transcription.

**Parameters:**
- `audioData` - Audio samples (f32 PCM, mono, 16kHz recommended)
- `socketPath` - Optional Unix socket path

**Returns:** Promise resolving when chunk is sent

**Example:**
```typescript
const samples = new Float32Array(1600); // 100ms @ 16kHz
// ... fill with audio data ...
await client.sendAudioChunk(samples);
```

---

### AudioCapture

Captures audio from system microphone using cpal.

#### Constructor

```typescript
const capture = new AudioCapture();
```

#### Methods

##### `startCapture(udpClient: UdpClient): void`

Start capturing audio from default input device.

**Parameters:**
- `udpClient` - UdpClient instance (used to get client ID)

**Behavior:**
- Detects default input device
- Configures for mono output at device's native sample rate
- Starts capture stream
- Logs captured samples (currently debug only)

**Example:**
```typescript
const client = new UdpClient();
await client.connect('tui');

const capture = new AudioCapture();
capture.startCapture(client); // Starts capture
```

**Note:** Current implementation logs samples but doesn't send to daemon yet. Async sender task needs implementation.

##### `stopCapture(): void`

Stop audio capture and release resources.

**Example:**
```typescript
capture.stopCapture();
```

##### `isRecording(): boolean`

Check if currently capturing audio.

**Returns:** `true` if recording, `false` otherwise

---

## Implementation Status

### ✅ Implemented

- UDP client with authentication
- Unix socket command sender
- Audio device detection and capture
- Multi-format sample support (f32, i16, u16)
- Mono conversion from stereo

### ⚠️ In Progress

- **Async audio chunk sender**: Audio callback currently logs samples but doesn't send to daemon
  - Need to implement channel-based architecture
  - Sync callback → Channel → Async task → `sendAudioChunk()`
  - Batch samples into ~100ms chunks before sending

### 📋 Planned

- Configurable sample rate and resampling
- Audio level monitoring
- Buffer overflow handling
- Graceful device change handling

## Architecture

### Audio Flow

```
Microphone
    ↓
cpal Stream (sync callback)
    ↓
Channel (sync → async)
    ↓
Async Task (batching)
    ↓
sendAudioChunk() (Unix socket)
    ↓
Daemon (real-time transcription)
    ↓
UDP broadcast (visualization data)
    ↓
receivePacket() → TUI
```

### Threading Model

- **Audio Thread**: cpal stream callback (sync, high priority)
- **Sender Task**: Tokio async task (batches and sends chunks)
- **Receiver Loop**: Async packet receiver in `NativeUdpClient`

## Error Handling

All async methods return `napi::Result<T>` which maps to JavaScript Promise rejections.

**Common errors:**
- `"Not connected - no client ID"` - Call `connect()` first
- `"Failed to connect to daemon"` - Check daemon is running and socket exists
- `"No input device available"` - No microphone detected
- `"Unsupported sample format"` - Device uses exotic format (rare)

## Performance

- **Zero-copy buffers**: Direct memory access for packet data
- **Native speed**: Rust performance for audio processing
- **Async I/O**: Non-blocking communication with daemon

## Security

- **Unix socket**: Process-local communication only
- **UDP secret**: Shared secret authentication (see main README)
- **No network exposure**: All communication is localhost-only

## Development

### Adding New Methods

1. Define function in `src/lib.rs` with `#[napi]` attribute
2. Rebuild: `pnpm build:native:debug`
3. TypeScript types auto-generated by napi-rs

### Debugging

```bash
# Enable Rust logging
RUST_LOG=debug pnpm dev

# Check native module load
node -e "console.log(require('./super-stt-tui-native.linux-x64-gnu.node'))"
```

### Dependencies

- `napi-rs` - N-API bindings generator
- `cpal` - Cross-platform audio library
- `tokio` - Async runtime
- `super-stt-shared` - Protocol definitions
- `serde_json` - JSON serialization

## See Also

- Main README: `../README.md`
- Protocol specification: `../plan.md`
- TypeScript wrapper: `../src/protocol/native-client.ts`
