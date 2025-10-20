# Super STT TUI - Usage Guide

## Quick Start

### 1. Start the Daemon

```bash
# Development build
/home/starbased/dev/src/starbased-co/super-stt/target/debug/super-stt \
  --model whisper-base \
  --device cpu \
  --udp-port 8766 \
  --socket /run/user/1000/super-stt/super-stt.sock \
  --verbose

# Or use release build for production
cargo build --release
/home/starbased/dev/src/starbased-co/super-stt/target/release/super-stt \
  --model whisper-base \
  --device cpu \
  --udp-port 8766
```

### 2. Build the TUI

```bash
cd /home/starbased/dev/src/starbased-co/super-stt/super-stt-tui

# Build native Rust module
pnpm build:native:debug    # Development
pnpm build:native          # Production

# Build TypeScript
pnpm build
```

### 3. Run the TUI

```bash
# Development mode (uses tsx for live reload)
pnpm dev

# Production mode
pnpm start
```

## Keyboard Shortcuts

- **`r`** - Start recording (triggers daemon to capture audio)
- **`q`** - Quit the application

## What You'll See

### Connection Status
```
● Status: Connected
  Client ID: udp_client_12345
```

### Recording Indicator
```
🔴 RECORDING    (when recording)
⚪ Idle         (when not recording)
```

### Audio Visualization
```
Audio Level: ████████░░░░░░░░░░░░ (0.234)

Frequency Spectrum:
  20Hz  ████░░░░░░
  40Hz  ██████░░░░
  80Hz  ████████░░
  ...
```

### Transcription Display
```
Transcription:

"Hello world, this is a test of the speech recognition system."
(Confidence: 0.95)
```

## Troubleshooting

### "Connection failed" Error

**Cause**: Daemon not running or socket path incorrect

**Solution**:
```bash
# Check daemon is running
pgrep super-stt

# Check socket exists
ls -la /run/user/1000/super-stt/super-stt.sock

# Start daemon if not running (see step 1 above)
```

### "UDP registration failed" Error

**Cause**: UDP port mismatch or auth secret not found

**Solution**:
```bash
# Check UDP secret exists
cat /run/user/1000/super-stt/udp_secret

# Ensure daemon and TUI use same port
# Daemon: --udp-port 8766
# TUI: hardcoded to 8766 in native/src/lib.rs
```

### No Audio Visualization

**Cause**: Microphone not accessible by daemon

**Solution**:
```bash
# Check microphone is available
pactl list sources short

# Test microphone with arecord
arecord -d 5 test.wav
aplay test.wav

# Check daemon has mic permissions
# (should see in daemon logs)
```

### Transcription Not Appearing

**Possible causes**:

1. **No speech detected** - Speak louder or closer to mic
2. **Silence threshold too high** - Check daemon logs for VAD settings
3. **Model not loaded** - Check daemon logs for model loading errors

**Daemon logs**:
```bash
# If running with --verbose, check logs for:
tail -f /path/to/daemon.log

# Look for:
# - "Recording started"
# - "Silence detected"
# - "Transcribing audio..."
# - "Transcription: ..."
```

## Development

### Testing Record Command

```bash
# Run test script
pnpm tsx test-record-command.ts

# Expected output:
# Step 1: Connecting to UDP server...
# ✓ REGISTERED:udp_client_XXXXX
#
# Step 2: Sending record command...
# ✓ Recording started
#
# Step 3: Listening for UDP packets...
# [1] Recording state: ACTIVE 🔴
# [2] Audio level: ████████ (0.234)
# [3] Transcription: "test" (confidence: 0.92)
```

### Rebuilding After Changes

```bash
# Rebuild native module (after Rust changes)
pnpm build:native:debug

# Rebuild TypeScript (after TS changes)
pnpm build

# Or rebuild everything
pnpm build:native:debug && pnpm build
```

### Debugging

**Enable verbose logging**:
```bash
# In native Rust code, logs are sent to console.log
# In TypeScript, use console.log/error
```

**Inspect UDP packets**:
```bash
# Listen to UDP broadcasts directly
nc -ul 8766

# You should see binary packet data when daemon is recording
```

**Check daemon state**:
```bash
# Send status command
echo '{"command":"status"}' | nc -U /run/user/1000/super-stt/super-stt.sock
```

## Configuration

### Change UDP Port

**In native module** (`native/src/lib.rs`):
```rust
// Line 43 and 95
.send_to(registration_msg.as_bytes(), "127.0.0.1:YOUR_PORT")
.send_to(b"PING", "127.0.0.1:YOUR_PORT")
```

**Start daemon with same port**:
```bash
super-stt --udp-port YOUR_PORT ...
```

**Rebuild**:
```bash
pnpm build:native:debug
```

### Change Socket Path

**In native module** (`native/src/lib.rs`):
```rust
// Line 124
format!("/path/to/your/socket.sock", unsafe { libc::getuid() })
```

**Start daemon with same path**:
```bash
super-stt --socket /path/to/your/socket.sock ...
```

## Performance Notes

- **CPU Usage**: TUI is lightweight (<5% CPU), daemon does heavy lifting
- **Memory**: ~50MB for TUI, ~500MB for daemon (with model loaded)
- **Network**: UDP packets are small (~100-500 bytes each)
- **Latency**: Near real-time (<100ms) for visualization updates

## Known Limitations

1. **UDP Port Hardcoded**: Port 8766 is hardcoded in native module
2. **No Stop Command**: Recording stops on silence detection only
3. **Single Recording**: Can't queue multiple recordings
4. **No Partial Updates**: Partial transcription not yet displayed in TUI
5. **Raw Mode Required**: TUI needs interactive terminal (no pipes)

## Next Steps

See `/home/starbased/dev/src/starbased-co/super-stt/super-stt-tui/REFACTOR_COMPLETE.md` for technical details about the architecture refactor.
