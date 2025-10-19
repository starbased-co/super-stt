# Quick Start Guide - Audio Streaming

## Build & Test

```bash
# Build native module
pnpm build:native

# Test audio capture (needs daemon running)
./test-audio-capture.ts

# With debug logs
RUST_LOG=debug ./test-audio-capture.ts
```

## Usage in TypeScript

```typescript
import { AudioCapture, UdpClient } from './super-stt-tui-native.linux-x64-gnu.node';

// 1. Connect to daemon
const udpClient = new UdpClient();
const clientId = await udpClient.connect('tui');

// 2. Start transcription session
await udpClient.startRealtimeTranscription();

// 3. Start audio capture (automatically sends to daemon)
const audioCapture = new AudioCapture();
audioCapture.startCapture(udpClient);

// 4. Receive visualization data
while (audioCapture.isRecording()) {
  const packet = await udpClient.receivePacket();
  const text = Buffer.from(packet).toString('utf-8');

  if (text.startsWith('TRANSCRIPTION:')) {
    console.log('Final:', text);
  } else if (text.startsWith('PARTIAL:')) {
    console.log('Partial:', text);
  } else if (text.startsWith('VIZ:')) {
    // Parse visualization data for audio meter/spectrum
  }
}

// 5. Stop capture
audioCapture.stopCapture();
udpClient.disconnect();
```

## Architecture

```
┌──────────────┐
│ Audio Device │ (any sample rate, any format)
└──────┬───────┘
       │ cpal audio callback
       ↓
┌──────────────────────────┐
│ AudioCapture.build_stream│
│ - Convert to mono f32    │
│ - Send to channel        │ (non-blocking)
└──────┬───────────────────┘
       │ tokio::sync::mpsc::unbounded_channel
       ↓
┌──────────────────────────┐
│ audio_sender_task        │
│ - Resample to 16kHz      │
│ - Batch 100ms chunks     │
│ - Send via Unix socket   │
└──────┬───────────────────┘
       │ /run/user/<uid>/stt/super-stt.sock
       ↓
┌──────────────────────────┐
│ Super STT Daemon         │
│ - Process with ML model  │
│ - Send transcription     │
└──────┬───────────────────┘
       │ UDP (127.0.0.1:8765)
       ↓
┌──────────────────────────┐
│ UdpClient.receivePacket  │
│ - Get visualization data │
│ - Get transcription      │
└──────────────────────────┘
```

## Key Parameters

- **Sample Rate**: Automatic resampling to 16kHz
- **Chunk Size**: 1600 samples (100ms @ 16kHz)
- **Channel**: Mono (multi-channel downmixed)
- **Format**: f32 PCM
- **Latency**: ~110-150ms end-to-end

## Error Handling

All errors are logged but **never crash the TUI**:

```typescript
try {
  audioCapture.startCapture(udpClient);
} catch (err) {
  // Handle:
  // - No microphone available
  // - PipeWire/PulseAudio permissions
  // - Audio device in use
}
```

Daemon disconnection:
- Audio capture continues
- Logs errors: "Failed to send audio chunk"
- TUI should show "Disconnected" status

## Debugging

### Check daemon is running
```bash
systemctl --user status super-stt
```

### Check socket exists
```bash
ls -la /run/user/$(id -u)/stt/super-stt.sock
```

### View audio logs
```bash
RUST_LOG=debug ./test-audio-capture.ts 2>&1 | grep -i audio
```

### Profile performance
```bash
perf record -F 99 -g ./test-audio-capture.ts
perf report
```

## Documentation

- **ASYNC_AUDIO_ARCHITECTURE.md**: Technical deep dive
- **CODE_CHANGES_SUMMARY.md**: Before/after code
- **IMPLEMENTATION_COMPLETE.md**: Full implementation summary

## Troubleshooting

| Issue | Solution |
|-------|----------|
| No audio captured | Check device permissions, PipeWire/PulseAudio |
| High CPU usage | Check if resampling is happening (see logs) |
| No data sent to daemon | Verify socket path, check daemon is running |
| Crackling audio | Should never happen (non-blocking callback) |
| Memory leak | Check `stop_capture()` is called on cleanup |

## Performance Tips

1. **Use release build**: `pnpm build:native` (not debug)
2. **Check sample rate**: Native 16kHz device = no resampling
3. **Monitor logs**: Look for "resampling: true" (slower)
4. **Profile first**: Don't optimize prematurely
