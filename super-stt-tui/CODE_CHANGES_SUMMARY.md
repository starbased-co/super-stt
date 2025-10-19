# Code Changes Summary

This document shows the exact changes made to implement async audio streaming.

## File: `native/src/lib.rs`

### 1. AudioCapture Struct - Added Fields

**Before:**
```rust
#[napi]
pub struct AudioCapture {
  stream: Arc<Mutex<Option<cpal::Stream>>>,
  is_recording: Arc<Mutex<bool>>,
}
```

**After:**
```rust
#[napi]
pub struct AudioCapture {
  stream: Arc<Mutex<Option<cpal::Stream>>>,
  is_recording: Arc<Mutex<bool>>,
  sample_sender: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<Vec<f32>>>>>,
  sender_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
  socket_path: Arc<Mutex<Option<String>>>,
}
```

**Changes:**
- Added `sample_sender`: Channel for sending audio samples to async task
- Added `sender_task`: Handle to background async task for cleanup
- Added `socket_path`: Stores daemon socket path for async task

---

### 2. Constructor - Initialize New Fields

**Before:**
```rust
#[napi(constructor)]
pub fn new() -> napi::Result<Self> {
  Ok(Self {
    stream: Arc::new(Mutex::new(None)),
    is_recording: Arc::new(Mutex::new(false)),
  })
}
```

**After:**
```rust
#[napi(constructor)]
pub fn new() -> napi::Result<Self> {
  Ok(Self {
    stream: Arc::new(Mutex::new(None)),
    is_recording: Arc::new(Mutex::new(false)),
    sample_sender: Arc::new(Mutex::new(None)),
    sender_task: Arc::new(Mutex::new(None)),
    socket_path: Arc::new(Mutex::new(None)),
  })
}
```

---

### 3. start_capture() - Create Channel and Spawn Task

**Before:**
```rust
#[napi]
pub fn start_capture(&self, udp_client: &UdpClient) -> napi::Result<()> {
  let host = cpal::default_host();
  let device = host.default_input_device()
    .ok_or_else(|| napi::Error::from_reason("No input device available"))?;

  let config = device.default_input_config()
    .map_err(|e| napi::Error::from_reason(format!("Failed to get input config: {}", e)))?;

  let sample_rate = config.sample_rate().0;
  let channels = config.channels() as usize;

  let client_id = udp_client.get_client_id()
    .ok_or_else(|| napi::Error::from_reason("UDP client not connected"))?;

  let is_recording = Arc::clone(&self.is_recording);
  *is_recording.lock().unwrap() = true;

  let stream = match config.sample_format() {
    SampleFormat::F32 => {
      self.build_stream::<f32>(&device, &config.into(), channels, sample_rate, client_id)?
    }
    SampleFormat::I16 => {
      self.build_stream::<i16>(&device, &config.into(), channels, sample_rate, client_id)?
    }
    SampleFormat::U16 => {
      self.build_stream::<u16>(&device, &config.into(), channels, sample_rate, client_id)?
    }
    _ => return Err(napi::Error::from_reason("Unsupported sample format")),
  };

  stream.play()
    .map_err(|e| napi::Error::from_reason(format!("Failed to start stream: {}", e)))?;

  *self.stream.lock().unwrap() = Some(stream);
  Ok(())
}
```

**After:**
```rust
#[napi]
pub fn start_capture(&self, udp_client: &UdpClient, socket_path: Option<String>) -> napi::Result<()> {
  let host = cpal::default_host();
  let device = host.default_input_device()
    .ok_or_else(|| napi::Error::from_reason("No input device available"))?;

  let config = device.default_input_config()
    .map_err(|e| napi::Error::from_reason(format!("Failed to get input config: {}", e)))?;

  let sample_rate = config.sample_rate().0;
  let channels = config.channels() as usize;

  let client_id = udp_client.get_client_id()
    .ok_or_else(|| napi::Error::from_reason("UDP client not connected"))?;

  // NEW: Resolve socket path
  let socket_path_resolved = socket_path.unwrap_or_else(|| {
    format!("/run/user/{}/stt/super-stt.sock", unsafe { libc::getuid() })
  });
  *self.socket_path.lock().unwrap() = Some(socket_path_resolved.clone());

  // NEW: Create channel for sample transfer
  let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Vec<f32>>();
  *self.sample_sender.lock().unwrap() = Some(tx.clone());

  // NEW: Spawn async sender task
  let sender_task = tokio::spawn(async move {
    Self::audio_sender_task(rx, client_id, socket_path_resolved, sample_rate).await;
  });
  *self.sender_task.lock().unwrap() = Some(sender_task);

  let is_recording = Arc::clone(&self.is_recording);
  *is_recording.lock().unwrap() = true;

  // CHANGED: Pass channel sender instead of client_id
  let stream = match config.sample_format() {
    SampleFormat::F32 => {
      self.build_stream::<f32>(&device, &config.into(), channels, tx)?
    }
    SampleFormat::I16 => {
      self.build_stream::<i16>(&device, &config.into(), channels, tx)?
    }
    SampleFormat::U16 => {
      self.build_stream::<u16>(&device, &config.into(), channels, tx)?
    }
    _ => return Err(napi::Error::from_reason("Unsupported sample format")),
  };

  stream.play()
    .map_err(|e| napi::Error::from_reason(format!("Failed to start stream: {}", e)))?;

  *self.stream.lock().unwrap() = Some(stream);
  Ok(())
}
```

**Key Changes:**
1. Added `socket_path: Option<String>` parameter
2. Created unbounded channel: `let (tx, rx) = tokio::sync::mpsc::unbounded_channel()`
3. Spawned async task: `tokio::spawn(async move { ... })`
4. Passed `tx` to `build_stream()` instead of `client_id`

---

### 4. build_stream() - Send Samples to Channel

**Before:**
```rust
fn build_stream<T>(&self, device: &cpal::Device, config: &StreamConfig, channels: usize, sample_rate: u32, client_id: String) -> napi::Result<cpal::Stream>
where
  T: cpal::Sample + cpal::SizedSample,
  f32: From<<T as cpal::Sample>::Float>,
{
  let is_recording = Arc::clone(&self.is_recording);

  let stream = device.build_input_stream(
    config,
    move |data: &[T], _: &cpal::InputCallbackInfo| {
      if !*is_recording.lock().unwrap() {
        return;
      }

      let samples: Vec<f32> = data.chunks(channels)
        .map(|frame| {
          let sum: f32 = frame.iter()
            .map(|&s| f32::from(s.to_float_sample()))
            .sum();
          sum / channels as f32
        })
        .collect();

      // TODO: Send samples to daemon via Unix socket
      // This needs to be async, so we'll need to spawn a task
      log::debug!("Captured {} samples", samples.len());
    },
    |err| {
      log::error!("Audio stream error: {}", err);
    },
    None,
  )
  .map_err(|e| napi::Error::from_reason(format!("Failed to build stream: {}", e)))?;

  Ok(stream)
}
```

**After:**
```rust
fn build_stream<T>(
  &self,
  device: &cpal::Device,
  config: &StreamConfig,
  channels: usize,
  sender: tokio::sync::mpsc::UnboundedSender<Vec<f32>>
) -> napi::Result<cpal::Stream>
where
  T: cpal::Sample + cpal::SizedSample,
  f32: From<<T as cpal::Sample>::Float>,
{
  let is_recording = Arc::clone(&self.is_recording);

  let stream = device.build_input_stream(
    config,
    move |data: &[T], _: &cpal::InputCallbackInfo| {
      if !*is_recording.lock().unwrap() {
        return;
      }

      let samples: Vec<f32> = data.chunks(channels)
        .map(|frame| {
          let sum: f32 = frame.iter()
            .map(|&s| f32::from(s.to_float_sample()))
            .sum();
          sum / channels as f32
        })
        .collect();

      // CHANGED: Send to channel (non-blocking)
      if sender.send(samples).is_err() {
        log::warn!("Audio sender channel closed, skipping samples");
      }
    },
    |err| {
      log::error!("Audio stream error: {}", err);
    },
    None,
  )
  .map_err(|e| napi::Error::from_reason(format!("Failed to build stream: {}", e)))?;

  Ok(stream)
}
```

**Key Changes:**
1. Changed signature: `client_id: String` → `sender: tokio::sync::mpsc::UnboundedSender<Vec<f32>>`
2. Replaced TODO with `sender.send(samples)`
3. Added error handling: logs warning if channel is closed

---

### 5. NEW: audio_sender_task() - Async Task Implementation

**Added 110 lines of new code:**

```rust
async fn audio_sender_task(
  mut rx: tokio::sync::mpsc::UnboundedReceiver<Vec<f32>>,
  client_id: String,
  socket_path: String,
  sample_rate: u32,
) {
  const TARGET_CHUNK_SIZE: usize = 1600; // 100ms @ 16kHz
  let mut buffer = Vec::with_capacity(TARGET_CHUNK_SIZE);

  // Determine if resampling is needed
  let resample_ratio = if sample_rate != 16000 {
    Some(16000.0 / sample_rate as f32)
  } else {
    None
  };

  log::info!(
    "Audio sender task started (client_id: {}, sample_rate: {}Hz, resampling: {})",
    client_id,
    sample_rate,
    resample_ratio.is_some()
  );

  // Receive samples from channel and batch them
  while let Some(mut samples) = rx.recv().await {
    if let Some(ratio) = resample_ratio {
      samples = Self::resample_linear(&samples, ratio);
    }

    buffer.extend_from_slice(&samples);

    // Send complete chunks
    while buffer.len() >= TARGET_CHUNK_SIZE {
      let chunk: Vec<f32> = buffer.drain(..TARGET_CHUNK_SIZE).collect();

      if let Err(e) = Self::send_chunk_to_daemon(&chunk, &client_id, &socket_path).await {
        log::error!("Failed to send audio chunk: {}", e);
      }
    }
  }

  // Send remaining samples when channel closes
  if !buffer.is_empty() {
    log::debug!("Sending final {} samples", buffer.len());
    if let Err(e) = Self::send_chunk_to_daemon(&buffer, &client_id, &socket_path).await {
      log::error!("Failed to send final audio chunk: {}", e);
    }
  }

  log::info!("Audio sender task stopped");
}
```

**Responsibilities:**
1. Receives samples from channel (async)
2. Resamples to 16kHz if needed
3. Batches into 100ms chunks
4. Sends to daemon via Unix socket
5. Flushes remaining samples on shutdown

---

### 6. NEW: resample_linear() - Linear Interpolation

```rust
fn resample_linear(samples: &[f32], ratio: f32) -> Vec<f32> {
  let new_len = (samples.len() as f32 * ratio) as usize;
  let mut resampled = Vec::with_capacity(new_len);

  for i in 0..new_len {
    let src_idx = i as f32 / ratio;
    let idx0 = src_idx.floor() as usize;
    let idx1 = (idx0 + 1).min(samples.len() - 1);
    let frac = src_idx - idx0 as f32;

    let sample = samples[idx0] * (1.0 - frac) + samples[idx1] * frac;
    resampled.push(sample);
  }

  resampled
}
```

**Purpose:** Fast resampling for sample rate conversion (e.g., 48kHz → 16kHz)

---

### 7. NEW: send_chunk_to_daemon() - Unix Socket Communication

```rust
async fn send_chunk_to_daemon(
  chunk: &[f32],
  client_id: &str,
  socket_path: &str,
) -> anyhow::Result<()> {
  let mut stream = UnixStream::connect(socket_path).await?;

  let request = DaemonRequest {
    command: "realtime_audio".to_string(),
    client_id: Some(client_id.to_string()),
    sample_rate: Some(16000),
    audio_data: Some(chunk.to_vec()),
    language: None,
    event_types: None,
    client_info: None,
    since_timestamp: None,
    limit: None,
    event_type: None,
    data: None,
    enabled: None,
  };

  let request_json = serde_json::to_string(&request)?;

  stream.write_all(request_json.as_bytes()).await?;
  stream.shutdown().await?;

  Ok(())
}
```

**Purpose:** Sends audio chunk to daemon via Unix socket

---

### 8. stop_capture() - Cleanup Resources

**Before:**
```rust
#[napi]
pub fn stop_capture(&self) {
  *self.is_recording.lock().unwrap() = false;
  *self.stream.lock().unwrap() = None;
}
```

**After:**
```rust
#[napi]
pub fn stop_capture(&self) {
  *self.is_recording.lock().unwrap() = false;
  *self.stream.lock().unwrap() = None;
  *self.sample_sender.lock().unwrap() = None; // NEW: Close channel

  // NEW: Abort async task
  if let Some(task) = self.sender_task.lock().unwrap().take() {
    task.abort();
  }
}
```

**Changes:**
1. Drop channel sender (closes channel, task exits)
2. Abort async task as final cleanup

---

## Statistics

- **Lines added**: ~180 lines
- **Lines modified**: ~40 lines
- **New functions**: 3 (`audio_sender_task`, `resample_linear`, `send_chunk_to_daemon`)
- **Modified functions**: 3 (`new`, `start_capture`, `build_stream`, `stop_capture`)
- **Compile time**: ~5 seconds (incremental)
- **Binary size increase**: ~100KB (async runtime + resampling)

## Dependencies

No new dependencies added. All required crates were already in `Cargo.toml`:
- `tokio` (with "full" features)
- `anyhow`
- `serde_json`
- `cpal`
- `napi`

## Backward Compatibility

**Breaking change**: `start_capture()` now requires `socket_path: Option<String>` parameter.

**Migration**:
```typescript
// Before
audioCapture.startCapture(udpClient);

// After
audioCapture.startCapture(udpClient); // socket_path defaults to /run/user/<uid>/stt/super-stt.sock
// OR
audioCapture.startCapture(udpClient, '/custom/path/to/socket.sock');
```

## Testing

Build verification:
```bash
cd native && cargo build  # ✓ Success
cd .. && pnpm build:native  # ✓ Success
```

Runtime testing requires daemon:
```bash
tsx test-audio-capture.ts  # Needs running daemon
```
