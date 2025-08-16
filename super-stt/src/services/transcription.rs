// SPDX-License-Identifier: GPL-3.0-only
use anyhow::Result;
use log::{debug, error, info, warn};
use rubato::{FastFixedIn, Resampler};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tokio::time::{Duration, interval};

use crate::daemon::types::STTModelInstance;
use crate::input::audio::AudioProcessor;
use super_stt_shared::services::notification::NotificationManager;

pub struct RealTimeSession {
    pub client_id: String,
    pub buffered_pcm: Vec<f32>,
    pub resampler: FastFixedIn<f32>,
    pub input_sample_rate: u32,
    pub language: Option<String>,
    pub language_token_set: bool,
    pub tx: broadcast::Sender<String>,
    pub decoding: bool,
    pub last_emit: Option<std::time::Instant>,
}

impl RealTimeSession {
    /// Create a new real-time transcription session
    ///
    /// # Errors
    ///
    /// Returns an error if the resampler cannot be constructed.
    pub fn new(
        client_id: String,
        input_sample_rate: u32,
        language: Option<String>,
    ) -> Result<Self> {
        let resample_ratio = 16000.0 / f64::from(input_sample_rate);
        let resampler = FastFixedIn::new(
            resample_ratio,
            10.0,
            rubato::PolynomialDegree::Septic,
            1024,
            1,
        )?;

        let (tx, _) = broadcast::channel(100);

        Ok(Self {
            client_id,
            buffered_pcm: Vec::new(),
            resampler,
            input_sample_rate,
            language,
            language_token_set: false,
            tx,
            decoding: false,
            last_emit: None,
        })
    }

    /// Append PCM audio data to the session buffer
    ///
    /// # Errors
    ///
    /// Currently infallible, returns `Ok(())`.
    pub fn add_audio_chunk(&mut self, audio_data: &[f32]) -> Result<()> {
        self.buffered_pcm.extend_from_slice(audio_data);
        Ok(())
    }

    /// Get a resampled processing window for preview decoding
    ///
    /// # Errors
    ///
    /// Returns an error if resampling fails.
    pub fn get_processing_window(&mut self) -> Result<Option<Vec<f32>>> {
        // Throttle: emit at most once every 3 seconds
        if let Some(last) = self.last_emit {
            if last.elapsed() < Duration::from_secs(3) {
                return Ok(None);
            }
        }

        // Minimal latency for first preview
        let min_first_seconds = 2usize; // start guessing after ~2s of audio
        let min_first = min_first_seconds * self.input_sample_rate as usize;
        if self.buffered_pcm.len() < min_first {
            return Ok(None);
        }

        // Only process the last N seconds to keep preview meaningful
        let window_seconds = 10usize; // 10s max
        let window_input_len =
            (window_seconds * self.input_sample_rate as usize).min(self.buffered_pcm.len());
        let start = self.buffered_pcm.len() - window_input_len;
        let slice = &self.buffered_pcm[start..];

        // Resample this tail slice
        let mut resampled_pcm = Vec::new();
        let full_chunks = slice.len() / 1024;
        for chunk in 0..full_chunks {
            let seg = &slice[chunk * 1024..(chunk + 1) * 1024];
            let pcm = self.resampler.process(&[seg], None)?;
            resampled_pcm.extend_from_slice(&pcm[0]);
        }
        // Do not process the remainder (<1024). Keep it for the next cycle to avoid rubato errors.

        // Keep a sliding buffer of recent audio (about 10s) to maintain some continuity
        let keep_size = (10 * self.input_sample_rate as usize).min(self.buffered_pcm.len());
        if self.buffered_pcm.len() > keep_size {
            let buffer_len = self.buffered_pcm.len();
            self.buffered_pcm.copy_within(buffer_len - keep_size.., 0);
            self.buffered_pcm.truncate(keep_size);
        }

        // Update emit time for throttling
        self.last_emit = Some(std::time::Instant::now());
        Ok(Some(resampled_pcm))
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Send a transcription string to session subscribers
    ///
    /// # Errors
    ///
    /// Currently infallible, returns `Ok(())`.
    pub fn send_transcription(&self, text: String) -> Result<()> {
        let _ = self.tx.send(text);
        Ok(())
    }
}

pub struct RealTimeTranscriptionManager {
    sessions: Arc<RwLock<HashMap<String, RealTimeSession>>>,
    model: Arc<RwLock<Option<STTModelInstance>>>,
    notification_manager: Arc<NotificationManager>,
    audio_processor: Arc<AudioProcessor>,
}

impl RealTimeTranscriptionManager {
    /// Construct a new real-time transcription manager
    pub fn new(
        model: Arc<RwLock<Option<STTModelInstance>>>,
        notification_manager: Arc<NotificationManager>,
        audio_processor: Arc<AudioProcessor>,
    ) -> Self {
        let manager = Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            model,
            notification_manager,
            audio_processor,
        };

        // Start processing task
        let sessions_clone = Arc::clone(&manager.sessions);
        let model_clone = Arc::clone(&manager.model);
        let processor_clone = Arc::clone(&manager.audio_processor);
        let notification_clone = Arc::clone(&manager.notification_manager);

        tokio::spawn(async move {
            let mut processing_interval = interval(Duration::from_millis(250)); // Check every 250ms for more responsiveness
            loop {
                processing_interval.tick().await;
                if let Err(e) = Self::process_all_sessions(
                    &sessions_clone,
                    &model_clone,
                    &processor_clone,
                    &notification_clone,
                )
                .await
                {
                    error!("Error processing real-time sessions: {e}");
                }
            }
        });

        manager
    }

    /// Start a real-time session and return a subscription receiver
    ///
    /// # Errors
    ///
    /// Returns an error if session creation or resampler setup fails.
    pub async fn start_session(
        &self,
        client_id: String,
        sample_rate: Option<u32>,
        language: Option<String>,
    ) -> Result<broadcast::Receiver<String>> {
        let sample_rate = sample_rate.unwrap_or(16000);
        let session = RealTimeSession::new(client_id.clone(), sample_rate, language)?;
        let receiver = session.subscribe();

        let mut sessions = self.sessions.write().await;
        sessions.insert(client_id.clone(), session);

        info!("Started real-time transcription session for client: {client_id}");

        // Broadcast session started event
        let _ = self
            .notification_manager
            .broadcast_event(
                "realtime_session_started".to_string(),
                client_id,
                serde_json::json!({
                    "sample_rate": sample_rate,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }),
            )
            .await;

        Ok(receiver)
    }

    /// Stop a session for a given client id
    ///
    /// # Errors
    ///
    /// Currently infallible, returns `Ok(())`.
    pub async fn stop_session(&self, client_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if sessions.remove(client_id).is_some() {
            info!("Stopped real-time transcription session for client: {client_id}");

            // Broadcast session stopped event
            let _ = self
                .notification_manager
                .broadcast_event(
                    "realtime_session_stopped".to_string(),
                    client_id.to_string(),
                    serde_json::json!({
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    }),
                )
                .await;
        }

        Ok(())
    }

    /// Ingest an audio chunk for a given client
    ///
    /// # Errors
    ///
    /// Returns an error if processing fails.
    pub async fn process_audio_chunk(
        &self,
        client_id: &str,
        audio_data: Vec<f32>,
        sample_rate: u32,
    ) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(client_id) {
            if session.input_sample_rate != sample_rate {
                warn!(
                    "Sample rate mismatch for client {}: expected {}, got {}",
                    client_id, session.input_sample_rate, sample_rate
                );
            }

            // Add the audio chunk to the session buffer
            session.add_audio_chunk(&audio_data)?;
            debug!("Added audio chunk for client: {client_id}");
        }

        Ok(())
    }

    async fn process_all_sessions(
        sessions: &Arc<RwLock<HashMap<String, RealTimeSession>>>,
        model: &Arc<RwLock<Option<STTModelInstance>>>,
        audio_processor: &Arc<AudioProcessor>,
        notification_manager: &Arc<NotificationManager>,
    ) -> Result<()> {
        // Collect clients that have audio ready for processing
        let mut ready_clients = Vec::new();

        {
            let mut sessions_write = sessions.write().await;
            for (client_id, session) in sessions_write.iter_mut() {
                if session.decoding {
                    continue; // skip if a decode is already running
                }
                if let Some(resampled_audio) = session.get_processing_window()? {
                    session.decoding = true; // mark in-progress
                    ready_clients.push((client_id.clone(), resampled_audio));
                }
            }
        }

        // Process each ready client in parallel
        for (client_id, resampled_audio) in ready_clients {
            let model_clone = Arc::clone(model);
            let proc_clone = Arc::clone(audio_processor);
            let sessions_clone = Arc::clone(sessions);
            let notification_clone = Arc::clone(notification_manager);

            tokio::spawn(async move {
                if let Err(e) = Self::transcribe_audio_chunk(
                    &client_id,
                    resampled_audio,
                    &model_clone,
                    &proc_clone,
                    &sessions_clone,
                    &notification_clone,
                )
                .await
                {
                    error!("Error transcribing audio for client {client_id}: {e}");
                }
                // Clear decoding flag
                let mut sessions_w = sessions_clone.write().await;
                if let Some(s) = sessions_w.get_mut(&client_id) {
                    s.decoding = false;
                }
            });
        }

        Ok(())
    }

    async fn transcribe_audio_chunk(
        client_id: &str,
        audio_data: Vec<f32>,
        model: &Arc<RwLock<Option<STTModelInstance>>>,
        audio_processor: &Arc<AudioProcessor>,
        sessions: &Arc<RwLock<HashMap<String, RealTimeSession>>>,
        notification_manager: &Arc<NotificationManager>,
    ) -> Result<()> {
        // Prepare and submit audio to model (works for Whisper and Voxtral)
        let resampled_len = audio_data.len();
        let processed = audio_processor.process_audio(&audio_data, 16000)?;
        info!(
            "Submitting preview audio to model: {} samples @ 16k (client_id={})",
            processed.len(),
            client_id
        );

        let transcription_result = tokio::task::spawn_blocking({
            let model_clone = Arc::clone(model);
            let audio = processed; // move into closure
            move || {
                let mut model_guard = model_clone.blocking_write();
                if let Some(model) = model_guard.as_mut() {
                    log::info!(
                        "[preview] Calling model.transcribe_audio: {} samples @16k",
                        audio.len()
                    );
                    model.transcribe_audio(&audio, 16000)
                } else {
                    Err(anyhow::anyhow!("Model not loaded"))
                }
            }
        })
        .await;

        match transcription_result {
            Ok(Ok(transcription)) => {
                if transcription.trim().is_empty() {
                    info!(
                        "Real-time preview produced empty transcription for {client_id} (resampled_len={resampled_len})"
                    );
                } else {
                    info!(
                        "Real-time preview transcription ({} chars): '{}'",
                        transcription.chars().count(),
                        transcription.chars().take(60).collect::<String>()
                    );
                    // Send to session subscribers
                    let sessions_read = sessions.read().await;
                    if let Some(session) = sessions_read.get(client_id) {
                        let _ = session.send_transcription(transcription.clone());
                    }

                    // Broadcast transcription event
                    let _ = notification_manager
                        .broadcast_event(
                            "realtime_transcription".to_string(),
                            client_id.to_string(),
                            serde_json::json!({
                                "transcription": transcription,
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            }),
                        )
                        .await;

                    debug!("Real-time transcription for {}: {}", client_id, "<omitted>");
                }
            }
            Ok(Err(e)) => {
                warn!("Transcription error for client {client_id}: {e}");
            }
            Err(e) => {
                error!("Task error for client {client_id}: {e}");
            }
        }

        Ok(())
    }

    pub async fn get_active_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }
}
