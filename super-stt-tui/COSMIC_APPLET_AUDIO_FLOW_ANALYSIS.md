# COSMIC Applet Audio Streaming Flow - Complete Analysis

## Executive Summary

**CRITICAL DISCOVERY**: The COSMIC applet does NOT send audio to the daemon. Instead:
1. The daemon performs local recording using `DaemonAudioRecorder`
2. The daemon broadcasts UDP packets during recording to ALL registered clients
3. The COSMIC applet simply RECEIVES UDP broadcasts - it's a passive listener
4. UDP registration happens BEFORE recording starts via authenticated handshake

## Exact Flow: COSMIC Applet Receives UDP Packets

### Step 1: COSMIC Applet Initialization (On Startup)

**File**: `/home/starbased/dev/src/starbased-co/super-stt/super-stt-cosmic-applet/src/lib.rs:215-311`

```rust
// UDP subscription starts immediately on applet init
Subscription::run_with_id(
    self.udp_restart_counter,  // Restarts when daemon reconnects
    cosmic::iced::stream::channel(100, |mut channel| async move {
        // 1. Bind UDP socket to random local port
        let socket = UdpSocket::bind("127.0.0.1:0").await?;

        // 2. Create authentication credentials
        let auth = UdpAuth::new()?;

        // 3. Send AUTHENTICATED registration to daemon
        let registration_msg = auth.create_auth_message("applet")?;
        socket.send_to(registration_msg.as_bytes(), "127.0.0.1:8765").await?;

        // 4. Send test PING to verify registration
        socket.send_to(b"PING", "127.0.0.1:8765").await?;

        // 5. Enter receive loop
        let mut buffer = [0u8; 1024];
        loop {
            tokio::select! {
                // Listen for UDP broadcasts from daemon
                recv_result = socket.recv_from(&mut buffer) => {
                    match recv_result {
                        Ok((len, _addr)) => {
                            let data = buffer[..len].to_vec();
                            channel.send(Message::UdpData(data)).await?;
                        }
                        Err(e) => warn!("UDP receive error: {e}"),
                    }
                }
                // Send periodic keep-alive pings
                _ = keepalive_interval.tick() => {
                    socket.send_to(b"PING", "127.0.0.1:8765").await?;
                }
            }
        }
    })
)
```

**Key Points**:
- UDP socket binds to `127.0.0.1:0` (random port assigned by OS)
- Registration uses authenticated message: `REGISTER:applet:<secret>`
- Registration happens ONCE on startup
- Applet becomes a registered UDP broadcast recipient

### Step 2: Daemon Registration Listener

**File**: `/home/starbased/dev/src/starbased-co/super-stt/super-stt/src/audio/streamer.rs:333-424`

```rust
pub async fn start_registration_listener(&self, shutdown_tx: &broadcast::Sender<()>) -> Result<()> {
    loop {
        tokio::select! {
            result = socket.recv_from(&mut buf) => {
                match result {
                    Ok((len, addr)) => {
                        if len >= 8 && &buf[0..8] == b"REGISTER" {
                            // Parse: "REGISTER:client_type:secret"
                            let msg = String::from_utf8_lossy(&buf[0..len]);

                            match auth.verify_auth_message(&msg) {
                                Ok(Some(client_type)) => {
                                    // Register client for UDP broadcasts
                                    let client_id = format!("udp_client_{}", addr.port());
                                    clients_guard.insert(
                                        client_id.clone(),
                                        StreamClient {
                                            addr,
                                            last_seen: Instant::now(),
                                            client_type,
                                        },
                                    );

                                    // Send acknowledgment
                                    socket.send_to(format!("REGISTERED:{client_id}").as_bytes(), addr).await;
                                }
                            }
                        } else if &buf[0..4] == b"PING" {
                            // Update client last_seen timestamp
                            // Send PONG response
                        }
                    }
                }
            }
        }
    }
}
```

**Key Points**:
- Daemon maintains a `HashMap<String, StreamClient>` of registered clients
- Each client stores: `addr: SocketAddr`, `last_seen: Instant`, `client_type: String`
- Authentication prevents unauthorized clients from receiving broadcasts
- PING messages keep clients alive (prevent stale cleanup)

### Step 3: User Triggers Recording (e.g., via TUI or another client)

**How Recording is Triggered**:
1. TUI sends `record` command via Unix socket to daemon
2. Daemon's `handle_record()` method is called
3. Daemon performs LOCAL audio capture (NOT from client)

**File**: `/home/starbased/dev/src/starbased-co/super-stt/super-stt/src/daemon/recording.rs:72-286`

```rust
pub async fn record_and_transcribe(&self, typer: &mut Typer, write_mode: bool) -> Result<String> {
    // Create LOCAL audio recorder
    let mut recorder = self.setup_recording_session(write_mode).await?;

    // Start recorder thread with UDP streaming
    let recorder_handle = tokio::spawn({
        let udp_streamer = Arc::clone(&self.udp_streamer);
        async move {
            recorder
                .record_until_silence_with_streaming(udp_streamer, None)
                .await
        }
    });

    // Wait for recording to complete
    let full_audio_data = recorder_handle.await??;

    // Transcribe
    let transcription = self.transcribe_with_spinner(typer, &full_audio_data, write_mode).await?;

    Ok(transcription)
}
```

**Key Points**:
- `DaemonAudioRecorder` captures audio from system microphone
- `record_until_silence_with_streaming()` broadcasts to UDP clients
- Recording is triggered by daemon commands (NOT by client audio upload)

### Step 4: Daemon Broadcasts UDP Packets During Recording

**File**: `/home/starbased/dev/src/starbased-co/super-stt/super-stt/src/audio/recorder.rs:127` (implementation)

During recording, the daemon broadcasts:

1. **Recording State Changes**:
   ```rust
   // Broadcast recording started
   udp_streamer.broadcast_recording_state(true, 0).await?;

   // Later: Broadcast recording stopped
   udp_streamer.broadcast_recording_state(false, 0).await?;
   ```

2. **Audio Samples** (for visualization):
   ```rust
   udp_streamer.broadcast_audio_samples(
       &samples,     // Raw f32 samples
       sample_rate,  // e.g., 16000.0
       channels,     // e.g., 1
       0             // source_client_id
   ).await?;
   ```

3. **Frequency Bands** (pre-computed for efficient visualization):
   ```rust
   udp_streamer.broadcast_frequency_bands(
       &bands,        // Pre-computed frequency bands
       sample_rate,
       total_energy,  // Overall audio level
       0
   ).await?;
   ```

**Broadcasting Mechanism** (`streamer.rs:260-286`):
```rust
async fn broadcast_packet(&self, packet: &[u8]) -> Result<()> {
    let mut clients = self.clients.write().await;

    for (client_id, client) in clients.iter_mut() {
        match self.socket.send_to(packet, &client.addr).await {
            Ok(_) => {
                // Update last_seen to prevent stale cleanup
                client.last_seen = Instant::now();
            }
            Err(e) => {
                // Remove failed clients
                failed_clients.push(client_id.clone());
            }
        }
    }

    // Clean up failed clients
    for client_id in failed_clients {
        clients.remove(&client_id);
    }
}
```

### Step 5: COSMIC Applet Processes UDP Data

**File**: `/home/starbased/dev/src/starbased-co/super-stt/super-stt-cosmic-applet/src/lib.rs:514-568`

```rust
Message::UdpData(data) => {
    // Update last UDP data timestamp
    self.last_udp_data = std::time::Instant::now();

    // Parse recording state
    if let Ok(state_data) = parse_recording_state_from_udp(&data) {
        let new_state = if state_data.is_recording {
            RecordingState::Recording
        } else {
            RecordingState::Processing  // Transitioning to transcription
        };
        self.recording_state = new_state;
    }
    // Parse frequency bands for visualization
    else if let Ok(frequency_data) = parse_frequency_bands_from_udp(&data) {
        self.visualization.update_frequency_bands(
            &frequency_data.bands,
            frequency_data.total_energy
        );
        self.audio_level = frequency_data.total_energy;
        self.is_speech_detected = frequency_data.total_energy > 0.02;
    }
    // Parse raw audio samples
    else if let Ok(samples_data) = parse_audio_samples_from_udp(&data) {
        self.visualization.update_audio_samples(&samples_data.samples);

        // Calculate RMS audio level
        let rms: f32 = samples_data.samples.iter()
            .map(|&s| s * s)
            .sum::<f32>() / samples_data.samples.len() as f32;
        self.audio_level = rms.sqrt().min(1.0);
        self.is_speech_detected = self.audio_level > 0.02;
    }
}
```

## UDP Packet Formats

### Recording State Packet
```
Header (11 bytes):
  - packet_type: u8 = 0x03 (RECORDING_STATE_PACKET)
  - timestamp: u32
  - source_client_id: u32
  - payload_length: u16

Payload (9 bytes):
  - is_recording: u8 (0 or 1)
  - timestamp_ms: u64
```

### Frequency Bands Packet
```
Header (11 bytes):
  - packet_type: u8 = 0x02 (FREQUENCY_BANDS_PACKET)
  - timestamp: u32
  - source_client_id: u32
  - payload_length: u16

Payload:
  - sample_rate: f32 (4 bytes)
  - total_energy: f32 (4 bytes)
  - num_bands: u32 (4 bytes)
  - bands: [f32; num_bands] (4 * num_bands bytes)
```

### Audio Samples Packet
```
Header (11 bytes):
  - packet_type: u8 = 0x01 (AUDIO_SAMPLES_PACKET)
  - timestamp: u32
  - source_client_id: u32
  - payload_length: u16

Payload:
  - sample_rate: f32 (4 bytes)
  - channels: u16 (2 bytes)
  - num_samples: u32 (4 bytes)
  - samples: [f32; num_samples] (4 * num_samples bytes)
```

## TUI Must Replicate This Exact Flow

### What TUI Should Do

**1. On Startup - Register for UDP Broadcasts**:
```typescript
// Create UDP socket
const socket = dgram.createSocket('udp4');
socket.bind();  // Bind to random port

// Generate authentication message
const auth = new UdpAuth();
const registrationMsg = auth.createAuthMessage('tui');

// Send registration to daemon
socket.send(registrationMsg, 8765, '127.0.0.1');

// Start listening for broadcasts
socket.on('message', (msg, rinfo) => {
    // Parse and display UDP packets
    handleUdpPacket(msg);
});

// Send periodic keep-alive pings
setInterval(() => {
    socket.send('PING', 8765, '127.0.0.1');
}, 60000);
```

**2. When User Wants to Record**:
```typescript
// Send 'record' command via Unix socket (existing code)
await sendDaemonCommand({
    command: 'record',
    write_mode: true
});

// UDP packets will automatically flow because TUI is registered
// No need to send audio chunks - daemon captures locally!
```

**3. Display UDP Packets**:
```typescript
function handleUdpPacket(data: Buffer) {
    const packetType = data[0];

    switch (packetType) {
        case RECORDING_STATE_PACKET:
            const state = parseRecordingState(data);
            updateRecordingIndicator(state.is_recording);
            break;

        case FREQUENCY_BANDS_PACKET:
            const bands = parseFrequencyBands(data);
            displayVisualization(bands.bands, bands.total_energy);
            break;

        case AUDIO_SAMPLES_PACKET:
            const samples = parseAudioSamples(data);
            displayWaveform(samples.samples);
            break;
    }
}
```

## Critical Differences from Current TUI Implementation

**Current TUI Mistake**:
- TUI tries to capture and send audio chunks to daemon ❌
- Uses `realtime_audio` command ❌
- Expects daemon to process client-provided audio ❌

**Correct Approach (COSMIC Applet)**:
- TUI only RECEIVES UDP broadcasts ✓
- Sends `record` command to trigger daemon's LOCAL recording ✓
- Daemon captures audio from system microphone ✓
- Daemon broadcasts UDP packets during recording ✓
- TUI visualizes received UDP packets ✓

## Why No UDP Packets in Current TUI

**Root Cause**: TUI is NOT registered for UDP broadcasts!

The TUI code shows:
1. No UDP socket creation
2. No registration message sent to daemon
3. No UDP receive loop

**Solution**: Implement UDP registration exactly like COSMIC applet does.

## Answer to Original Question

**Q: Does the COSMIC applet use `record` command or `realtime_audio` command?**

**A**: The COSMIC applet uses NEITHER command directly. Instead:

1. **COSMIC applet ONLY receives UDP broadcasts**
   - It's a passive visualization client
   - No daemon commands sent by applet
   - Just listens for UDP packets on port 8765

2. **Other clients trigger recording** (e.g., TUI, CLI)
   - They send `record` command to daemon
   - Daemon performs LOCAL audio capture
   - Daemon broadcasts to ALL registered UDP clients (including COSMIC)

3. **The `realtime_audio` command** appears to be for a different use case
   - Allows external clients to stream audio TO daemon
   - Not used by COSMIC applet
   - TUI should NOT use this either for standard recording

## Implementation Checklist for TUI

- [ ] Remove audio capture code from TUI
- [ ] Remove `realtime_audio` command usage
- [ ] Implement UDP socket creation and binding
- [ ] Implement authenticated UDP registration
- [ ] Implement UDP receive loop
- [ ] Parse received UDP packets (recording state, frequency bands, audio samples)
- [ ] Display visualizations based on UDP data
- [ ] Send `record` command to trigger daemon recording
- [ ] Send periodic PING keep-alives
- [ ] Handle UDP connection lifecycle (registration, keep-alive, cleanup)

## Files to Reference

**COSMIC Applet UDP Implementation**:
- `/home/starbased/dev/src/starbased-co/super-stt/super-stt-cosmic-applet/src/lib.rs:215-311` - UDP subscription
- `/home/starbased/dev/src/starbased-co/super-stt/super-stt-cosmic-applet/src/lib.rs:514-568` - UDP data processing

**Daemon UDP Broadcasting**:
- `/home/starbased/dev/src/starbased-co/super-stt/super-stt/src/audio/streamer.rs:333-424` - Registration listener
- `/home/starbased/dev/src/starbased-co/super-stt/super-stt/src/audio/streamer.rs:260-286` - Broadcasting mechanism

**UDP Packet Parsing**:
- `/home/starbased/dev/src/starbased-co/super-stt/super-stt-shared/src/networking/udp_parsing.rs` - All packet parsers

**Daemon Recording Flow**:
- `/home/starbased/dev/src/starbased-co/super-stt/super-stt/src/daemon/recording.rs:72-286` - Local recording with UDP streaming
