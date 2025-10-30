# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Super STT TUI is a terminal-based visualization client for the Super STT daemon. Built with React/Ink and a Rust N-API native module, it provides real-time audio visualization and transcription display.

**Critical Architecture Note**: This is a **passive listener client**. It does NOT capture or send audio to the daemon. Instead:
1. The daemon performs local audio capture
2. The TUI registers with the daemon's UDP broadcast system
3. The daemon broadcasts visualization/transcription data to all registered UDP clients
4. The TUI only sends control commands (start/stop recording) via Unix socket

## Build & Development Commands

### Initial Setup
```bash
# Install Node dependencies
pnpm install

# Build native Rust module (release)
pnpm build:native

# Build native Rust module (debug, faster compilation)
pnpm build:native:debug

# Build TypeScript
pnpm build
```

### Development Workflow
```bash
# Development mode with hot reload (uses tsx)
pnpm dev

# Production mode
pnpm start

# Type checking only
pnpm typecheck
```

### After Making Changes
```bash
# Changed Rust native code
pnpm build:native:debug

# Changed TypeScript code
pnpm build

# Changed both
pnpm build:native:debug && pnpm build
```

## Architecture

### Hybrid TypeScript + Rust Stack

**TypeScript Layer** (`src/`):
- React/Ink UI components
- UDP packet parsing and state management
- Real-time visualization rendering

**Rust Native Module** (`native/src/lib.rs`):
- UDP client with authenticated registration
- Unix socket command interface
- Exports N-API bindings to TypeScript via `UdpClient` class

### UDP Broadcast Protocol (Daemon → TUI)

The daemon broadcasts 5 packet types on port 8765:

1. **Type 2 - Partial STT**: In-progress transcription text
2. **Type 3 - Final STT**: Completed transcription text
3. **Type 4 - Audio Samples**: Raw f32 waveform data
4. **Type 5 - Recording State**: Idle/Recording/Processing status
5. **Type 6 - Frequency Bands**: Pre-computed spectrum for visualization

All packets follow the same header format (11 bytes):
```
[0] packet_type: u8
[1-4] timestamp_ms: u32 (little-endian)
[5-8] source_client_id: u32 (little-endian)
[9-10] payload_length: u16 (little-endian)
[11+] payload data...
```

### Packet Parsers (`src/protocol/`)

- `types.ts` - TypeScript type definitions for all packet types
- `parsers.ts` - Binary parsing functions with validation
- `native-client.ts` - Wrapper around Rust N-API `UdpClient`
- `client.ts` - Higher-level event emitter interface

**Security**: All parsers include DoS protection:
- Maximum packet size: 8192 bytes
- Maximum samples per packet: 192,000
- Maximum frequency bands: 64

### React Components (`src/components/`)

- `Header.tsx` - Application branding
- `ConnectionStatus.tsx` - Shows UDP registration status
- `RecordingIndicator.tsx` - Visual recording state (🔴/⚪)
- `AudioMeter.tsx` - VU meter using `totalEnergy` from frequency bands
- `FrequencyVisualizer.tsx` - Spectrum display from frequency bands
- `TranscriptionDisplay.tsx` - Partial and final text with confidence

### State Management (`src/hooks/useUdpClient.ts`)

Custom React hook managing:
- UDP socket connection lifecycle
- Authenticated registration with daemon
- Packet reception and parsing
- State updates for UI components
- Recording command dispatch

## Two-Channel Communication

**UDP (Port 8765)**: Daemon → TUI visualization data
- Client binds to random port (OS-assigned)
- Sends authenticated `REGISTER:tui:<secret>` message
- Receives broadcast packets from daemon
- Sends periodic `PING` keep-alives (every 60s)

**Unix Socket**: TUI → Daemon commands
- Default path: `/run/user/{uid}/stt/super-stt.sock`
- Sends `record` command with `write_mode` flag
- Uses 8-byte length-prefixed JSON protocol
- Daemon performs local audio capture and broadcasts UDP

## Authentication & Security

**UDP Secret File**: `$XDG_RUNTIME_DIR/stt/udp_secret`
- TUI automatically creates if missing (follows daemon pattern)
- 0600 permissions (owner-only read/write)
- Shared secret used for `REGISTER:client_type:secret` authentication
- Prevents unauthorized clients from receiving broadcasts

**Rust Native Module** (`UdpAuth` from `super-stt-shared`):
- `UdpAuth::new()` - Reads or creates secret file
- `create_auth_message(client_type)` - Formats registration message
- Daemon validates via `verify_auth_message()`

## Key Implementation Details

### Why Passive Listener Pattern?

The TUI does NOT capture audio because:
1. **Daemon already has audio capture** - `DaemonAudioRecorder` uses cpal
2. **Multiple visualization clients** - COSMIC applet, TUI, etc. all receive same UDP stream
3. **No redundant audio pipelines** - Single source of truth in daemon
4. **Simplified client architecture** - TUI just renders what daemon broadcasts

### Frequency Bands vs Audio Samples

**Frequency Bands (Type 6)** - Primary visualization source:
- Pre-computed by daemon using FFT
- Optimized for efficient network transmission
- `totalEnergy` field provides instant VU meter value
- Bands array provides spectrum data

**Audio Samples (Type 4)** - Raw waveform:
- Larger packet size (4 bytes per sample)
- Requires client-side RMS/FFT computation
- Use for detailed waveform visualization

**Recommendation**: Prefer frequency bands for performance.

## Common Pitfalls

### ❌ DON'T send audio chunks to daemon
The `realtime_audio` command exists for external audio sources. The TUI should trigger daemon's LOCAL recording via `record` command.

### ❌ DON'T create UDP socket without authentication
Unauthenticated clients are rejected. Always use `UdpAuth` for registration.

### ❌ DON'T parse packets without validation
All parsers check minimum sizes and enforce security limits. Invalid packets return `null`.

### ✅ DO send periodic PINGs
Daemon cleans up stale clients. Send `PING` every 60s to maintain registration.

### ✅ DO handle connection failures gracefully
UDP registration can fail if daemon isn't running. Display clear error states.

## Dependencies

**Parent Workspace**: `super-stt-shared`
- Located at `../../super-stt-shared` (workspace member)
- Provides: `UdpAuth`, `DaemonRequest`, `DaemonResponse`, protocol types
- Shared between daemon, applet, and TUI

**TypeScript Dependencies**:
- `ink` - React for CLI rendering
- `@inkjs/ui` - UI component library for Ink
- `tsx` - TypeScript execution for development

**Rust Dependencies** (`native/Cargo.toml`):
- `napi` + `napi-derive` - N-API bindings for Node.js
- `super-stt-shared` - Protocol types and auth
- `tokio` - Async runtime for UDP/Unix socket
- `serde_json` - JSON serialization for daemon commands

## File Locations

- **UDP Secret**: `$XDG_RUNTIME_DIR/stt/udp_secret` (typically `/run/user/1000/stt/`)
- **Unix Socket**: `/run/user/{uid}/stt/super-stt.sock`
- **UDP Port**: 8765 (hardcoded in native module and daemon)

## Troubleshooting

### No UDP packets received
1. Check daemon is running: `pgrep super-stt`
2. Check UDP registration succeeded: Look for `REGISTERED:udp_client_*` in logs
3. Verify daemon broadcasts: `ss -ulnp | grep 8765`
4. Ensure periodic PINGs are sent (check `send_ping()` interval)

### Authentication failed
1. Check secret file exists: `cat $XDG_RUNTIME_DIR/stt/udp_secret`
2. Verify file permissions: `ls -la $XDG_RUNTIME_DIR/stt/`
3. Ensure daemon and TUI use same secret file path

### Recording command fails
1. Check Unix socket path is correct
2. Verify daemon is listening on socket: `ls -la /run/user/*/stt/super-stt.sock`
3. Check daemon logs for command processing errors
4. Ensure `client_id` is set before sending record command

## Reference Documentation

- **Protocol Specification**: See `COSMIC_APPLET_AUDIO_FLOW_ANALYSIS.md` for detailed flow
- **UDP Packet Formats**: See `src/protocol/types.ts` and `parsers.ts`
- **Native API**: See `native/README.md` for N-API interface details
- **COSMIC Applet Reference**: `../../super-stt-cosmic-applet/src/lib.rs` (canonical UDP client implementation)
