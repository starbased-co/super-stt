#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::sync::{Arc, Mutex};
use tokio::net::{UdpSocket, UnixStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use super_stt_shared::UdpAuth;
use super_stt_shared::models::protocol::{DaemonRequest, DaemonResponse};

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

    eprintln!("[DEBUG] Sending registration message: {}", registration_msg);
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
    eprintln!("[DEBUG] Received response: {}", response);

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
  pub async fn send_record_command(&self, socket_path: Option<String>, write_mode: bool) -> napi::Result<String> {
    let client_id = self.get_client_id()
      .ok_or_else(|| napi::Error::from_reason("Not connected - no client ID"))?;

    let socket_path = socket_path.unwrap_or_else(|| {
      format!("/run/user/{}/stt/super-stt.sock", unsafe { libc::getuid() })
    });

    let request = DaemonRequest {
      command: "record".to_string(),
      client_id: Some(client_id),
      data: Some(serde_json::json!({
        "write_mode": write_mode
      })),
      sample_rate: None,
      language: None,
      audio_data: None,
      event_types: None,
      client_info: None,
      since_timestamp: None,
      limit: None,
      event_type: None,
      enabled: None,
    };

    let response = send_daemon_command(&socket_path, &request).await?;
    Ok(response.message.unwrap_or_else(|| "Recording started".to_string()))
  }
}

async fn send_daemon_command(socket_path: &str, request: &DaemonRequest) -> napi::Result<DaemonResponse> {
  eprintln!("[DEBUG] Connecting to socket: {}", socket_path);
  let mut stream = UnixStream::connect(socket_path)
    .await
    .map_err(|e| napi::Error::from_reason(format!("Failed to connect to daemon: {}", e)))?;
  eprintln!("[DEBUG] Connected successfully");

  let request_data = serde_json::to_vec(request)
    .map_err(|e| napi::Error::from_reason(format!("Failed to serialize request: {}", e)))?;
  eprintln!("[DEBUG] Request serialized: {} bytes", request_data.len());
  eprintln!("[DEBUG] Request JSON: {}", String::from_utf8_lossy(&request_data));

  // Daemon protocol: 8-byte message size (u64 big-endian) + message content
  let message_size = request_data.len() as u64;
  let size_bytes = message_size.to_be_bytes();

  // Send size prefix
  eprintln!("[DEBUG] Sending size prefix: {}", message_size);
  stream.write_all(&size_bytes)
    .await
    .map_err(|e| napi::Error::from_reason(format!("Failed to send message size: {}", e)))?;

  // Send message content
  eprintln!("[DEBUG] Sending request data");
  stream.write_all(&request_data)
    .await
    .map_err(|e| napi::Error::from_reason(format!("Failed to send request: {}", e)))?;
  eprintln!("[DEBUG] Request sent, waiting for response size");

  // Read response size
  let mut response_size_buf = [0u8; 8];
  stream.read_exact(&mut response_size_buf)
    .await
    .map_err(|e| napi::Error::from_reason(format!("Failed to read response size: {}", e)))?;
  eprintln!("[DEBUG] Response size received");

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
