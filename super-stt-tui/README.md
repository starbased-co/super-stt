# Super STT - Terminal User Interface

Production-ready Terminal UI client for the Super STT speech-to-text daemon. Built with React and Ink for beautiful, interactive terminal visualizations.

## Features

- **Real-time Audio Capture**: Native microphone capture using cpal (Cross-Platform Audio Library)
- **Live Audio Visualization**: VU meter and frequency spectrum display
- **Streaming Transcription**: Shows both partial (in-progress) and final transcriptions
- **Connection Management**: Automatic authentication and reconnection via UDP and Unix sockets
- **Recording Controls**: Space to start/stop, auto-start on launch, auto-stop on exit
- **Responsive UI**: Updates at 60fps with smooth animations

## Prerequisites

1. **Super STT Daemon**: Must be running on `127.0.0.1:8765`
2. **Node.js**: 18+ recommended
3. **pnpm**: Package manager (or npm/yarn)

## Installation

```bash
# Install dependencies
pnpm install

# Build native module (Rust)
pnpm build:native

# Build TypeScript
pnpm build
```

## Usage

### Development Mode

```bash
pnpm dev
```

### Production Mode

```bash
# Run built version
pnpm start

# Or run directly with node
node dist/index.js
```

### Controls

- **Space** - Start/Stop audio capture and transcription
- **q** - Quit the application (auto-stops recording)

## Architecture

### Native Module (`native/`)

Rust N-API bindings providing:
- **Audio Capture**: cpal-based microphone access
- **UDP Client**: Fast binary protocol communication
- **Unix Socket**: Real-time transcription command channel

See `native/README.md` for API documentation.

### Protocol Layer (`src/protocol/`)

- **types.ts**: TypeScript definitions for all UDP packet types
- **parsers.ts**: Binary packet parsers with validation
- **native-client.ts**: Native module wrapper with event emitters

### React Layer

- **hooks/useUdpClient.ts**: Custom hook managing connection state
- **components/**: Modular UI components
  - `Header.tsx` - Application branding
  - `ConnectionStatus.tsx` - Connection indicator
  - `RecordingIndicator.tsx` - Recording state display
  - `AudioMeter.tsx` - VU meter for audio level
  - `FrequencyVisualizer.tsx` - Frequency spectrum bars
  - `TranscriptionDisplay.tsx` - Transcription text
- **app.tsx**: Main application component

### Communication Protocols

**UDP (Port 8765)** - Daemon → TUI visualization data:
1. **Type 2**: Partial STT (in-progress transcription)
2. **Type 3**: Final STT (completed transcription)
3. **Type 4**: Audio Samples (raw waveform data)
4. **Type 5**: Recording State (idle/recording/processing)
5. **Type 6**: Frequency Bands (spectrum visualization)

**Unix Socket** - TUI → Daemon commands:
- `start_realtime`: Begin real-time transcription session
- `realtime_audio`: Send audio chunks (f32 PCM, 16kHz mono)

The TUI captures audio via cpal, sends chunks to daemon via Unix socket, and receives visualization/transcription data via UDP.

## Security

- **Secret Authentication**: Auto-creates or reads shared secret from `$XDG_RUNTIME_DIR/stt/udp_secret`
- **Lazy Initialization**: Secret file created on first client connection (follows daemon pattern)
- **Restrictive Permissions**: Secret file has 0600 permissions (owner-only read/write)
- **Packet Validation**: All packets validated for size and structure
- **DoS Protection**: Maximum limits on samples and frequency bands

## Performance

- **Efficient Parsing**: Binary packet parsing with zero-copy buffers
- **Debounced Updates**: React state updates optimized for 60fps
- **Memory Safe**: Bounded data structures prevent memory leaks

## Troubleshooting

### "Secret file not found"

**This should never happen** - the TUI automatically creates the secret file if it doesn't exist. If you see this error:

1. Check directory permissions:
```bash
ls -la $XDG_RUNTIME_DIR/stt/
```

2. Manually verify the secret file was created:
```bash
cat $XDG_RUNTIME_DIR/stt/udp_secret
```

### "Connection timeout"

Ensure the daemon is running and listening on `127.0.0.1:8765`:

```bash
# Check if daemon is running
pgrep super-stt

# Check if daemon is listening on UDP port 8765
ss -ulnp | grep 8765
```

### No Audio Visualization

The daemon sends frequency bands continuously. If you see no visualization:

1. Check recording state indicator
2. Verify daemon is processing audio
3. Check for errors in connection status

## Development

### Type Checking

```bash
pnpm typecheck
```

### Project Structure

```
super-stt-tui/
├── src/
│   ├── index.tsx           # Entry point with signal handlers
│   ├── app.tsx             # Main React component
│   ├── protocol/           # UDP protocol implementation
│   │   ├── types.ts
│   │   ├── parsers.ts
│   │   ├── client.ts
│   │   └── index.ts
│   ├── hooks/
│   │   └── useUdpClient.ts
│   └── components/
│       ├── Header.tsx
│       ├── ConnectionStatus.tsx
│       ├── RecordingIndicator.tsx
│       ├── AudioMeter.tsx
│       ├── FrequencyVisualizer.tsx
│       ├── TranscriptionDisplay.tsx
│       └── index.ts
├── dist/                   # Compiled JavaScript
├── package.json
├── tsconfig.json
├── plan.md                 # Protocol specification
└── README.md
```

## License

MIT