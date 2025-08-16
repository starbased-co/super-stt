<div align="center">

<img src=".github/assets/super-stt-icon.svg" width="128" height="128" alt="Super STT">

# Super STT

**High-performance speech-to-text service with real-time audio streaming**

*Built in Rust • COSMIC Desktop • GPU Acceleration*

</div>

## ✨ Features

- **⚡ Real-time transcription** built to be executed with a shortcut
- **⌨️ Auto-typing** transcribed text
- **🎯 GPU acceleration** (CUDA/cuDNN)
- **🔄 Model switching** on the fly
- **🎵 Real-time audio visualization** in COSMIC panel

## 🚀 Installation
```bash
git clone https://github.com/jorge-menjivar/super-stt.git
cd super-stt

# (Required) The Daemon and CLI tool
just install-daemon
# (Suggestion) If you have an NVIDIA GPU with CUDA installed, build with CUDA acceleration instead to load models onto the GPU
# just install-daemon --cuda
# (Suggestion) If you have an NVIDIA GPU with CUDA and cuDNN installed, build with cuDNN acceleration instead for better performance
# just install-daemon --cudnn

# (Recommended) The Desktop application to manage settings and models
just install-app

# (Optional) COSMIC Desktop applet to show visualizations
just install-applet
```

## ⌨️ Shortcuts
Add a shortcut that executes the following command to your desktop environment to start the app.
```sh
stt record --write
```

On COSMIC, you can add the shortcut in COSMIC Settings app: Input Devices -> Keyboard -> View and customize shortcuts -> Custom -> Add Shortcut

> **Note**: On first run, Super STT will automatically download the required AI model (~1-2GB). This may take a few minutes depending on your internet connection.

The following will then happen:

1. `stt` will contact the already running daemon and ask it to use the microphone to transcribe audio.
2. The daemon will then process the audio and start typing a preview of the transcription into whatever application is currently focused. This may be a bit inaccurate.
3. The daemon will automatically stop listening when it no longer detects speech.
4. Once it stops recording, it will process the full transcription one more time, this will produce an accurate transcription.
5. The daemon will automatically replace the preview with the accurate transcription.

### Usage
```bash
# Make sure Super STT is running.
systemctl --user status super-stt

# If not running, start it
systemctl --user start super-stt
```

### Troubleshooting

#### `super-stt` is not found
You may need to log out and back in to update your PATH environment variable.

#### `stt` is not found
Try rerunning the `just install-daemon` command.

### Model Selection

**From UI**: Open app → Settings → Select model

**When installing**:
```bash
# Install with specific model
just install-daemon --model whisper-large-v3

# Or specify during daemon start
stt --model whisper-base
```

## 🏗️ Architecture

- **`super-stt`** - Background ML service
- **`super-stt-app`** - Desktop configuration app
- **`super-stt-cosmic-applet`** - Panel applet with visualizations
- **`super-stt-shared`** - Common protocols

## 🔒 Security

Super STT implements comprehensive security controls:

- **Process Authentication**: Keyboard injection requires verification that the client is the legitimate `stt` binary
- **Group-based Access**: Production mode uses dedicated `stt` group for daemon access control

For detailed security information, see [`docs/SECURITY.md`](docs/SECURITY.md).

### Security Model

**Production Mode** (default):
- Socket permissions: `0660` (owner + stt group only)
- Process authentication required for keyboard access
- Resource limits and rate limiting active

**Development Mode**:
- Debug builds automatically enable development mode with relaxed permissions
- Use only in secure development environments
- For production security, always use release builds

## 🔧 Development

```bash
# Run the daemon
just run-daemon

# Run the app
just run-app

# Run the applet
just run-applet

# Run security audit
just audit
```

---

<div align="center">

**Jorge Menjivar** • jorge@menjivar.ai

</div>
