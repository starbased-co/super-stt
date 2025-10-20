# TUI Refactor: Passive Listener Pattern Complete

## Summary

The Super STT TUI has been successfully refactored to follow the **COSMIC applet's passive listener pattern**. The TUI is now a lightweight UDP client that:

1. **Registers** with the daemon's UDP server for event broadcasts
2. **Sends** record commands via Unix socket to trigger daemon recording
3. **Receives** UDP packets passively (recording state, audio visualization, transcription)
4. **NO audio capture** - all recording happens in the daemon

## What Changed

### Native Rust Module (`native/src/lib.rs`)

**REMOVED**:
- ❌ `AudioCapture` struct (lines 228-471)
- ❌ All cpal audio capture code
- ❌ `start_realtime_transcription()` method
- ❌ `send_audio_chunk()` method
- ❌ Audio resampling logic
- ❌ Audio sender task
- ❌ cpal dependency from Cargo.toml

**ADDED**:
- ✅ `send_record_command(socket_path, write_mode)` method
  - Sends `record` command to daemon via Unix socket
  - Passes `write_mode` in JSON data field
  - Returns daemon's response message
  - Uses correct socket path: `/run/user/1000/super-stt/super-stt.sock`

**KEPT** (unchanged):
- ✅ `UdpClient` struct
- ✅ `connect()` - UDP registration
- ✅ `receive_packet()` - Passive UDP listener
- ✅ `disconnect()` - Cleanup
- ✅ `send_ping()` - Keep-alive

### TypeScript Protocol Client (`src/protocol/native-client.ts`)

**ADDED**:
```typescript
async sendRecordCommand(socketPath?: string, writeMode = true): Promise<string> {
  if (!this.client) {
    throw new Error('Client not connected');
  }
  return await this.client.sendRecordCommand(socketPath, writeMode);
}
```

### React Hook (`src/hooks/useUdpClient.ts`)

**ADDED**:
```typescript
export interface UdpClientState {
  // ... existing fields
  startRecording: () => Promise<void>;
  stopRecording: () => Promise<void>;
}

const startRecording = async () => {
  if (!clientRef.current) {
    throw new Error('Client not connected');
  }
  try {
    const response = await clientRef.current.sendRecordCommand(undefined, true);
    console.log('Recording started:', response);
  } catch (error) {
    setState((prev) => ({ ...prev, error }));
    throw error;
  }
};
```

### Main App Component (`src/app.tsx`)

**UPDATED**:
```typescript
const {
  // ... existing state
  startRecording,  // NEW
} = useUdpClient();

// Handle keyboard input
useInput((input) => {
  if (input === 'q' || input === 'Q') {
    exit();
  } else if (input === 'r' || input === 'R') {  // NEW
    if (isRegistered && !isRecording) {
      startRecording().catch((err) => {
        console.error('Failed to start recording:', err);
      });
    }
  }
});
```

**Footer updated**:
```
Press r to record | q to quit
```

## Architecture Flow

### Before (INCORRECT - TUI Captured Audio)
```
User presses 'r'
  → TUI captures audio via cpal
  → TUI resamples audio
  → TUI sends chunks via Unix socket
  → Daemon processes chunks
```

### After (CORRECT - Daemon Captures Audio)
```
User presses 'r'
  → TUI sends "record" command via Unix socket
  → Daemon captures audio locally (DaemonAudioRecorder)
  → Daemon broadcasts UDP packets:
      ├─ Recording state (packet type 5)
      ├─ Frequency bands (packet type 6)
      ├─ Audio samples (packet type 4)
      ├─ Partial STT (packet type 2)
      └─ Final STT (packet type 3)
  → TUI receives and displays packets passively
```

## Files Modified

1. **`native/src/lib.rs`** - Removed AudioCapture, added sendRecordCommand
2. **`native/Cargo.toml`** - Removed cpal and anyhow dependencies
3. **`src/protocol/native-client.ts`** - Added sendRecordCommand wrapper
4. **`src/hooks/useUdpClient.ts`** - Added startRecording/stopRecording functions
5. **`src/app.tsx`** - Added 'r' key handler for recording

## Testing

### Test Script: `test-record-command.ts`

The test demonstrates the complete flow:

```typescript
// 1. Connect to UDP server
await client.connect('tui');

// 2. Send record command
await client.sendRecordCommand(undefined, true);

// 3. Listen for UDP packets (passive)
while (running) {
  const buffer = await client.receivePacket();
  // Parse packet type and display data
}
```

### Daemon Testing

Start daemon with correct configuration:
```bash
/home/starbased/dev/src/starbased-co/super-stt/target/debug/super-stt \
  --model whisper-base \
  --device cpu \
  --udp-port 8766 \
  --socket /run/user/1000/super-stt/super-stt.sock \
  --verbose
```

### Verified Functionality

1. ✅ UDP registration works (`REGISTERED:udp_client_XXXXX`)
2. ✅ Record command sent successfully
3. ✅ Daemon starts local recording
4. ✅ Daemon broadcasts UDP packets during recording
5. ✅ TUI displays recording indicator
6. ✅ Audio visualization receives frequency band data
7. ✅ Final transcription appears after silence

## Key Benefits

1. **Simplified Architecture**: TUI no longer deals with audio capture complexity
2. **No Audio Dependencies**: Removed cpal, reduced binary size
3. **Passive Listener**: TUI is lightweight and reactive
4. **Daemon Controls Audio**: Single source of truth for audio handling
5. **Matches COSMIC Applet**: Both clients now follow same pattern
6. **Write Mode Support**: Daemon can type transcription when enabled

## Configuration Notes

- **UDP Port**: Currently hardcoded to `8766` (was `8765` but had conflicts)
- **Socket Path**: `/run/user/1000/super-stt/super-stt.sock`
- **UDP Secret**: Read from `/run/user/1000/super-stt/udp_secret`
- **Keyboard Shortcut**: `r` to start recording, `q` to quit

## Future Improvements

1. Make UDP port configurable via CLI argument
2. Add stop recording command (currently auto-stops on silence)
3. Add status indicator for when daemon is processing
4. Display partial transcription during recording
5. Add error handling for daemon not running

## Conclusion

The refactor is **complete** and the TUI now correctly follows the passive listener pattern as specified in the COSMIC applet analysis. The TUI:

- Does NOT capture audio
- Does NOT send audio chunks
- Does NOT use cpal
- ONLY sends control commands (record)
- ONLY receives UDP broadcasts passively
- Displays real-time visualization and transcription

This aligns perfectly with the daemon/client architecture where **the daemon owns audio recording** and clients are lightweight observers.
