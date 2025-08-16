// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use super_stt_shared::UdpAuth;
use super_stt_shared::daemon_state::RecordingStateData;
use super_stt_shared::models::audio::{AudioSamplesData, FrequencyBandsData};
use super_stt_shared::stt::STTData;
use super_stt_shared::udp::{
    AUDIO_SAMPLES_PACKET, FINAL_STT_PACKET, FREQUENCY_BANDS_PACKET, MAX_PACKET_SIZE,
    PARTIAL_STT_PACKET, PacketHeader, RECORDING_STATE_PACKET,
};
use tokio::net::UdpSocket;
use tokio::sync::{RwLock, broadcast};
use tokio::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct StreamClient {
    pub addr: SocketAddr,
    pub last_seen: Instant,
    pub client_type: String, // "cosmic", "web", etc.
}

pub struct UdpAudioStreamer {
    socket: Arc<UdpSocket>,
    clients: Arc<RwLock<HashMap<String, StreamClient>>>,
    next_client_id: Arc<RwLock<u32>>,
    auth: UdpAuth,
}

impl UdpAudioStreamer {
    /// Create a new UDP audio streamer
    ///
    /// # Errors
    ///
    /// Returns an error if binding the UDP socket fails.
    pub async fn new(bind_addr: &str) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        log::info!("UDP Audio Streamer listening on {bind_addr}");

        let auth = UdpAuth::new()?;
        log::info!("UDP authentication initialized");

        Ok(Self {
            socket: Arc::new(socket),
            clients: Arc::new(RwLock::new(HashMap::new())),
            next_client_id: Arc::new(RwLock::new(1)),
            auth,
        })
    }

    /// Register a new client for streaming
    pub async fn register_client(&self, addr: SocketAddr, client_type: String) -> String {
        let mut clients = self.clients.write().await;
        let mut next_id = self.next_client_id.write().await;

        let client_id = format!("udp_client_{}", *next_id);
        *next_id += 1;

        clients.insert(
            client_id.clone(),
            StreamClient {
                addr,
                last_seen: Instant::now(),
                client_type,
            },
        );

        log::info!("Registered UDP client: {client_id} at {addr}");
        client_id
    }

    /// Remove a client
    pub async fn unregister_client(&self, client_id: &str) {
        let mut clients = self.clients.write().await;
        if clients.remove(client_id).is_some() {
            log::info!("Unregistered UDP client: {client_id}");
        }
    }

    /// Check if there are any registered clients
    pub async fn has_clients(&self) -> bool {
        let clients = self.clients.read().await;
        !clients.is_empty()
    }

    /// Broadcast recording state change to all clients
    ///
    /// # Errors
    ///
    /// Returns an error if packet serialization or sending fails.
    pub async fn broadcast_recording_state(
        &self,
        is_recording: bool,
        source_client_id: u32,
    ) -> Result<()> {
        let data = RecordingStateData::new(is_recording);
        let data_bytes = data.to_bytes();

        let header = PacketHeader::new(
            RECORDING_STATE_PACKET,
            source_client_id,
            u16::try_from(data_bytes.len()).unwrap_or(u16::MAX),
        );
        let header_bytes = header.to_bytes();

        // Combine header and data
        let mut packet = Vec::with_capacity(header_bytes.len() + data_bytes.len());
        packet.extend_from_slice(&header_bytes);
        packet.extend_from_slice(&data_bytes);

        log::info!("Broadcasting recording state: is_recording={is_recording}");
        self.broadcast_packet(&packet).await
    }

    /// Broadcast partial STT result
    ///
    /// # Errors
    ///
    /// Returns an error if packet serialization or sending fails.
    pub async fn broadcast_partial_stt(
        &self,
        text: String,
        confidence: f32,
        source_client_id: u32,
    ) -> Result<()> {
        let data = STTData { text, confidence };
        let data_bytes = data.to_bytes();

        // Split large messages if needed
        if data_bytes.len() > MAX_PACKET_SIZE - 11 {
            log::warn!("Partial STT message too large, truncating");
            return Ok(());
        }

        let header = PacketHeader::new(
            PARTIAL_STT_PACKET,
            source_client_id,
            u16::try_from(data_bytes.len()).unwrap_or(u16::MAX),
        );
        let header_bytes = header.to_bytes();

        let mut packet = Vec::with_capacity(header_bytes.len() + data_bytes.len());
        packet.extend_from_slice(&header_bytes);
        packet.extend_from_slice(&data_bytes);

        self.broadcast_packet(&packet).await
    }

    /// Broadcast final STT result
    ///
    /// # Errors
    ///
    /// Returns an error if packet serialization or sending fails.
    pub async fn broadcast_final_stt(
        &self,
        text: String,
        confidence: f32,
        source_client_id: u32,
    ) -> Result<()> {
        let data = STTData { text, confidence };
        let data_bytes = data.to_bytes();

        if data_bytes.len() > MAX_PACKET_SIZE - 11 {
            log::warn!("Final STT message too large, truncating");
            return Ok(());
        }

        let header = PacketHeader::new(
            FINAL_STT_PACKET,
            source_client_id,
            u16::try_from(data_bytes.len()).unwrap_or(u16::MAX),
        );
        let header_bytes = header.to_bytes();

        let mut packet = Vec::with_capacity(header_bytes.len() + data_bytes.len());
        packet.extend_from_slice(&header_bytes);
        packet.extend_from_slice(&data_bytes);

        self.broadcast_packet(&packet).await
    }

    /// Broadcast raw audio samples for real-time frequency analysis
    ///
    /// # Errors
    ///
    /// Returns an error if packet serialization or sending fails.
    pub async fn broadcast_audio_samples(
        &self,
        samples: &[f32],
        sample_rate: f32,
        channels: u16,
        source_client_id: u32,
    ) -> Result<()> {
        // Limit sample count to fit in UDP packet
        // Packet structure: Header(11) + sample_rate(4) + channels(2) + num_samples(4) + samples(4*n)
        let max_samples = (MAX_PACKET_SIZE - 11 - 4 - 2 - 4) / 4; // header + sample_rate + channels + num_samples + 4 bytes per sample
        let samples_to_send = if samples.len() > max_samples {
            log::debug!(
                "Truncating audio samples from {} to {} to fit in UDP packet",
                samples.len(),
                max_samples
            );
            &samples[..max_samples]
        } else {
            samples
        };

        let data = AudioSamplesData {
            samples: samples_to_send.to_vec(),
            sample_rate,
            channels,
        };
        let data_bytes = data.to_bytes();

        let header = PacketHeader::new(
            AUDIO_SAMPLES_PACKET,
            source_client_id,
            u16::try_from(data_bytes.len()).unwrap_or(u16::MAX),
        );
        let header_bytes = header.to_bytes();

        let mut packet = Vec::with_capacity(header_bytes.len() + data_bytes.len());
        packet.extend_from_slice(&header_bytes);
        packet.extend_from_slice(&data_bytes);

        log::info!(
            "Broadcasting {} audio samples: packet_size={} (header={}, audio_data={}), data_len_in_header={}",
            samples_to_send.len(),
            packet.len(),
            header_bytes.len(),
            data_bytes.len(),
            u16::from_le_bytes([header_bytes[9], header_bytes[10]])
        );
        self.broadcast_packet(&packet).await
    }

    /// Broadcast pre-computed frequency bands for real-time visualization
    /// This replaces raw audio sample broadcasting with much smaller, more efficient packets
    ///
    /// # Errors
    ///
    /// Returns an error if packet serialization or sending fails.
    pub async fn broadcast_frequency_bands(
        &self,
        bands: &[f32],
        sample_rate: f32,
        total_energy: f32,
        source_client_id: u32,
    ) -> Result<()> {
        let data = FrequencyBandsData {
            bands: bands.to_vec(),
            sample_rate,
            total_energy,
        };
        let data_bytes = data.to_bytes();

        let header = PacketHeader::new(
            FREQUENCY_BANDS_PACKET,
            source_client_id,
            u16::try_from(data_bytes.len()).unwrap_or(u16::MAX),
        );
        let header_bytes = header.to_bytes();

        let mut packet = Vec::with_capacity(header_bytes.len() + data_bytes.len());
        packet.extend_from_slice(&header_bytes);
        packet.extend_from_slice(&data_bytes);

        log::trace!(
            "Broadcasting {} frequency bands: packet_size={} bytes, total_energy={:.3}",
            bands.len(),
            packet.len(),
            total_energy
        );
        self.broadcast_packet(&packet).await
    }

    /// Internal method to broadcast a packet to all registered clients
    async fn broadcast_packet(&self, packet: &[u8]) -> Result<()> {
        let clients = self.clients.read().await;
        let mut failed_clients = Vec::new();

        for (client_id, client) in clients.iter() {
            match self.socket.send_to(packet, &client.addr).await {
                Ok(_) => {
                    log::trace!("Sent packet to client: {client_id}");
                }
                Err(e) => {
                    log::warn!("Failed to send packet to client {client_id}: {e}");
                    failed_clients.push(client_id.clone());
                }
            }
        }

        // Remove failed clients (but don't hold the lock while doing it)
        drop(clients);
        if !failed_clients.is_empty() {
            let mut clients = self.clients.write().await;
            for client_id in failed_clients {
                clients.remove(&client_id);
                log::info!("Removed failed client: {client_id}");
            }
        }

        Ok(())
    }

    /// Start a cleanup task to remove stale clients
    pub fn start_cleanup_task(&self, shutdown_tx: &broadcast::Sender<()>) {
        let clients = Arc::clone(&self.clients);
        let mut shutdown_rx = shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let mut clients_guard = clients.write().await;
                        let now = Instant::now();
                        let stale_timeout = Duration::from_secs(300); // 5 minutes

                        let stale_clients: Vec<String> = clients_guard
                            .iter()
                            .filter(|(_, client)| now.duration_since(client.last_seen) > stale_timeout)
                            .map(|(id, _)| id.clone())
                            .collect();

                        for client_id in stale_clients {
                            clients_guard.remove(&client_id);
                            log::info!("Removed stale client: {client_id}");
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        log::info!("UDP cleanup task shutting down gracefully");
                        break;
                    }
                }
            }
        });
    }

    /// Get current client count
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Start listening for client registration messages
    #[allow(clippy::unused_async)]
    /// Start background task to register UDP clients
    ///
    /// # Errors
    ///
    /// Returns an error if spawning or socket operations fail (non-fatal; task retries).
    pub async fn start_registration_listener(
        &self,
        shutdown_tx: &broadcast::Sender<()>,
    ) -> Result<()> {
        let socket = Arc::clone(&self.socket);
        let clients = Arc::clone(&self.clients);
        let auth = self.auth.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];

            loop {
                tokio::select! {
                    result = socket.recv_from(&mut buf) => {
                        match result {
                            Ok((len, addr)) => {
                                if len >= 8 && &buf[0..8] == b"REGISTER" {
                                    // Authenticated registration protocol: "REGISTER:client_type:secret"
                                    let msg = String::from_utf8_lossy(&buf[0..len]);

                                    match auth.verify_auth_message(&msg) {
                                        Ok(Some(client_type)) => {
                                            let client_id = format!("udp_client_{}", addr.port());
                                            let mut clients_guard = clients.write().await;
                                            clients_guard.insert(
                                                client_id.clone(),
                                                StreamClient {
                                                    addr,
                                                    last_seen: Instant::now(),
                                                    client_type,
                                                },
                                            );

                                            log::info!("✓ Authenticated UDP client registered: {client_id} at {addr}");

                                            // Send acknowledgment
                                            let ack_msg = format!("REGISTERED:{client_id}");
                                            let _ = socket.send_to(ack_msg.as_bytes(), addr).await;
                                        }
                                        Ok(None) => {
                                            log::warn!("✗ Authentication failed for UDP registration from {addr}");
                                            let _ = socket.send_to(b"AUTH_FAILED", addr).await;
                                        }
                                        Err(e) => {
                                            log::error!("UDP authentication error: {e}");
                                            let _ = socket.send_to(b"AUTH_ERROR", addr).await;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("UDP registration listener error: {e}");
                                tokio::time::sleep(Duration::from_secs(1)).await;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        log::info!("UDP registration listener shutting down gracefully");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Clean up authentication resources
    ///
    /// # Errors
    /// This function will return an error if the authentication resources cannot be cleaned up.
    pub fn cleanup_auth(&self) -> Result<()> {
        self.auth.cleanup()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_header_serialization() {
        let header = PacketHeader::new(AUDIO_SAMPLES_PACKET, 12345, 5);
        let bytes = header.to_bytes();

        assert_eq!(bytes[0], AUDIO_SAMPLES_PACKET);
        assert_eq!(
            u32::from_le_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]),
            12345
        );
        assert_eq!(u16::from_le_bytes([bytes[9], bytes[10]]), 5);
    }
}
