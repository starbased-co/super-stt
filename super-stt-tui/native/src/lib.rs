#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::sync::{Arc, Mutex};
use tokio::net::{UdpSocket, UnixStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use super_stt_shared::UdpAuth;
use super_stt_shared::models::protocol::{DaemonRequest, DaemonResponse};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};

#[napi]
pub struct UdpClient {
  socket: Arc<Mutex<Option<Arc<UdpSocket>>>>,
  auth: UdpAuth,
  client_id: Arc<Mutex<Option<String>>>,
}

#[napi]
impl UdpClient {
  #[napi(constructor)]
  pub fn new() -> napi::Result<Self> {
    let auth = UdpAuth::new()
      .map_err(|e| napi::Error::from_reason(format!("Failed to create UDP auth: {}", e)))?;

    Ok(Self {
      socket: Arc::new(Mutex::new(None)),
      auth,
      client_id: Arc::new(Mutex::new(None)),
    })
  }

  #[napi]
  pub async fn connect(&self, client_type: String) -> napi::Result<String> {
    let socket = UdpSocket::bind("127.0.0.1:0")
      .await
      .map_err(|e| napi::Error::from_reason(format!("Failed to bind socket: {}", e)))?;

    let registration_msg = self.auth
      .create_auth_message(&client_type)
      .map_err(|e| napi::Error::from_reason(format!("Failed to create auth message: {}", e)))?;

    socket
      .send_to(registration_msg.as_bytes(), "127.0.0.1:8765")
      .await
      .map_err(|e| napi::Error::from_reason(format!("Failed to send registration: {}", e)))?;

    let mut buf = [0u8; 1024];
    let (len, _addr) = socket
      .recv_from(&mut buf)
      .await
      .map_err(|e| napi::Error::from_reason(format!("Failed to receive response: {}", e)))?;

    let response = String::from_utf8_lossy(&buf[..len]).to_string();

    if response.starts_with("REGISTERED:") {
      *self.client_id.lock().unwrap() = Some(response.clone());
      *self.socket.lock().unwrap() = Some(Arc::new(socket));
      Ok(response)
    } else if response.starts_with("AUTH_FAILED") {
      Err(napi::Error::from_reason("Authentication failed - check UDP secret"))
    } else {
      Err(napi::Error::from_reason(format!("Unexpected response: {}", response)))
    }
  }

  #[napi]
  pub async fn receive_packet(&self) -> napi::Result<Buffer> {
    let socket = {
      let guard = self.socket.lock().unwrap();
      guard.as_ref()
        .ok_or_else(|| napi::Error::from_reason("Not connected - call connect() first"))?
        .clone()
    };

    let mut buf = vec![0u8; 8192];
    let (len, _addr) = socket
      .recv_from(&mut buf)
      .await
      .map_err(|e| napi::Error::from_reason(format!("Failed to receive packet: {}", e)))?;

    buf.truncate(len);
    Ok(buf.into())
  }

  #[napi]
  pub async fn send_ping(&self) -> napi::Result<()> {
    let socket = {
      let guard = self.socket.lock().unwrap();
      guard.as_ref()
        .ok_or_else(|| napi::Error::from_reason("Not connected - call connect() first"))?
        .clone()
    };

    socket
      .send_to(b"PING", "127.0.0.1:8765")
      .await
      .map_err(|e| napi::Error::from_reason(format!("Failed to send ping: {}", e)))?;

    Ok(())
  }

  #[napi]
  pub fn disconnect(&self) {
    *self.socket.lock().unwrap() = None;
    *self.client_id.lock().unwrap() = None;
  }

  #[napi]
  pub fn is_connected(&self) -> bool {
    self.socket.lock().unwrap().is_some()
  }

  #[napi]
  pub fn get_client_id(&self) -> Option<String> {
    self.client_id.lock().unwrap().clone()
  }

  #[napi]
  pub async fn start_realtime_transcription(&self, socket_path: Option<String>) -> napi::Result<()> {
    let client_id = self.get_client_id()
      .ok_or_else(|| napi::Error::from_reason("Not connected - no client ID"))?;

    let socket_path = socket_path.unwrap_or_else(|| {
      format!("/run/user/{}/stt/super-stt.sock", unsafe { libc::getuid() })
    });

    let request = DaemonRequest {
      command: "start_realtime".to_string(),
      client_id: Some(client_id),
      sample_rate: Some(16000),
      language: None,
      audio_data: None,
      event_types: None,
      client_info: None,
      since_timestamp: None,
      limit: None,
      event_type: None,
      data: None,
      enabled: None,
    };

    send_daemon_command(&socket_path, &request).await?;
    Ok(())
  }

  #[napi]
  pub async fn send_audio_chunk(&self, audio_data: Float32Array, socket_path: Option<String>) -> napi::Result<()> {
    let client_id = self.get_client_id()
      .ok_or_else(|| napi::Error::from_reason("Not connected - no client ID"))?;

    let socket_path = socket_path.unwrap_or_else(|| {
      format!("/run/user/{}/stt/super-stt.sock", unsafe { libc::getuid() })
    });

    let request = DaemonRequest {
      command: "realtime_audio".to_string(),
      client_id: Some(client_id),
      sample_rate: Some(16000),
      audio_data: Some(audio_data.to_vec()),
      language: None,
      event_types: None,
      client_info: None,
      since_timestamp: None,
      limit: None,
      event_type: None,
      data: None,
      enabled: None,
    };

    send_daemon_command(&socket_path, &request).await?;
    Ok(())
  }
}

async fn send_daemon_command(socket_path: &str, request: &DaemonRequest) -> napi::Result<DaemonResponse> {
  let mut stream = UnixStream::connect(socket_path)
    .await
    .map_err(|e| napi::Error::from_reason(format!("Failed to connect to daemon: {}", e)))?;

  let request_json = serde_json::to_string(request)
    .map_err(|e| napi::Error::from_reason(format!("Failed to serialize request: {}", e)))?;

  // Daemon protocol: 8-byte message size (u64 big-endian) + message content
  let message_size = request_json.len() as u64;
  let size_bytes = message_size.to_be_bytes();

  // Send size prefix
  stream.write_all(&size_bytes)
    .await
    .map_err(|e| napi::Error::from_reason(format!("Failed to send message size: {}", e)))?;

  // Send message content
  stream.write_all(request_json.as_bytes())
    .await
    .map_err(|e| napi::Error::from_reason(format!("Failed to send request: {}", e)))?;

  stream.shutdown()
    .await
    .map_err(|e| napi::Error::from_reason(format!("Failed to shutdown write: {}", e)))?;

  // Read response size
  let mut response_size_buf = [0u8; 8];
  stream.read_exact(&mut response_size_buf)
    .await
    .map_err(|e| napi::Error::from_reason(format!("Failed to read response size: {}", e)))?;

  let response_size = u64::from_be_bytes(response_size_buf) as usize;
  if response_size > 100 * 1024 * 1024 {
    return Err(napi::Error::from_reason("Response too large"));
  }

  // Read response content
  let mut response_buf = vec![0u8; response_size];
  stream.read_exact(&mut response_buf)
    .await
    .map_err(|e| napi::Error::from_reason(format!("Failed to read response: {}", e)))?;

  let response: DaemonResponse = serde_json::from_slice(&response_buf)
    .map_err(|e| napi::Error::from_reason(format!("Failed to parse response: {}", e)))?;

  if response.status == "error" {
    return Err(napi::Error::from_reason(
      response.message.unwrap_or_else(|| "Unknown error".to_string())
    ));
  }

  Ok(response)
}

#[napi]
pub struct AudioCapture {
  stream: Arc<Mutex<Option<cpal::Stream>>>,
  is_recording: Arc<Mutex<bool>>,
  sample_sender: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<Vec<f32>>>>>,
  sender_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
  socket_path: Arc<Mutex<Option<String>>>,
}

#[napi]
impl AudioCapture {
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

    let socket_path_resolved = socket_path.unwrap_or_else(|| {
      format!("/run/user/{}/stt/super-stt.sock", unsafe { libc::getuid() })
    });
    *self.socket_path.lock().unwrap() = Some(socket_path_resolved.clone());

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Vec<f32>>();
    *self.sample_sender.lock().unwrap() = Some(tx.clone());

    let sender_task = tokio::spawn(async move {
      Self::audio_sender_task(rx, client_id, socket_path_resolved, sample_rate).await;
    });
    *self.sender_task.lock().unwrap() = Some(sender_task);

    let is_recording = Arc::clone(&self.is_recording);
    *is_recording.lock().unwrap() = true;

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

  async fn audio_sender_task(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Vec<f32>>,
    client_id: String,
    socket_path: String,
    sample_rate: u32,
  ) {
    const TARGET_CHUNK_SIZE: usize = 1600;
    let mut buffer = Vec::with_capacity(TARGET_CHUNK_SIZE);

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

    while let Some(mut samples) = rx.recv().await {
      if let Some(ratio) = resample_ratio {
        samples = Self::resample_linear(&samples, ratio);
      }

      buffer.extend_from_slice(&samples);

      while buffer.len() >= TARGET_CHUNK_SIZE {
        let chunk: Vec<f32> = buffer.drain(..TARGET_CHUNK_SIZE).collect();

        if let Err(e) = Self::send_chunk_to_daemon(&chunk, &client_id, &socket_path).await {
          log::error!("Failed to send audio chunk: {}", e);
        }
      }
    }

    if !buffer.is_empty() {
      log::debug!("Sending final {} samples", buffer.len());
      if let Err(e) = Self::send_chunk_to_daemon(&buffer, &client_id, &socket_path).await {
        log::error!("Failed to send final audio chunk: {}", e);
      }
    }

    log::info!("Audio sender task stopped");
  }

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

  #[napi]
  pub fn stop_capture(&self) {
    *self.is_recording.lock().unwrap() = false;
    *self.stream.lock().unwrap() = None;
    *self.sample_sender.lock().unwrap() = None;

    if let Some(task) = self.sender_task.lock().unwrap().take() {
      task.abort();
    }
  }

  #[napi]
  pub fn is_recording(&self) -> bool {
    *self.is_recording.lock().unwrap()
  }
}
