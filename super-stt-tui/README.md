# Super STT TUI

Terminal User Interface client for the Super STT speech-to-text daemon.

## Overview

This is a React-based terminal UI built with Ink that connects to the Super STT daemon via UDP to display:
- Real-time audio level visualization
- Frequency spectrum analysis
- Live speech-to-text transcription
- Recording status

## Prerequisites

- Node.js 18+ and pnpm
- Super STT daemon running on the system
- Unix-like environment (Linux/macOS) for UDP socket support

## Installation

```bash
# Install dependencies
pnpm install

# Build the TypeScript code
pnpm build
```

## Development

```bash
# Run in development mode with hot reload
pnpm dev
```

## Production

```bash
# Build the project
pnpm build

# Run the compiled version
pnpm start
```

## Scripts

- `pnpm dev` - Run in development mode with tsx
- `pnpm build` - Compile TypeScript to JavaScript
- `pnpm start` - Run the compiled production version
- `pnpm typecheck` - Type-check without building

## Project Structure

```
super-stt-tui/
├── src/
│   ├── index.tsx       # Entry point
│   ├── app.tsx         # Main App component
│   └── components/     # UI components (to be added)
├── dist/               # Compiled output (after build)
├── plan.md             # Protocol documentation
├── package.json        # Project configuration
├── tsconfig.json       # TypeScript configuration
└── README.md           # This file
```

## Protocol

The TUI communicates with the Super STT daemon using the UDP protocol on port 8765.
See `plan.md` for detailed protocol documentation.

## Next Steps

The current implementation is a minimal scaffold. The following features will be added:

1. **UDP Client Implementation**
   - Connection management
   - Authentication with shared secret
   - Keep-alive mechanism

2. **Protocol Parsing**
   - Packet header parsing
   - Recording state packets
   - Frequency band packets
   - STT result packets

3. **Visualization Components**
   - Audio level meter
   - Frequency spectrum display
   - Waveform visualization

4. **User Interface**
   - Keyboard controls
   - Status indicators
   - Transcription display

## License

MIT