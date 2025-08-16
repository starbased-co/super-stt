// SPDX-License-Identifier: GPL-3.0-only
use crate::daemon::types::SuperSTTDaemon;
use chrono::Utc;
use log::{debug, error, info, warn};
use std::sync::Arc;
use super_stt_shared::models::protocol::DaemonResponse;
use super_stt_shared::utils::audio::validate_audio;

impl SuperSTTDaemon {
    /// Handle transcribe command
    #[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
    pub async fn handle_transcribe(
        &self,
        audio_data: Vec<f32>,
        sample_rate: u32,
        client_id: String,
    ) -> DaemonResponse {
        info!("Processing transcription request from client: {client_id}");

        // Validate audio
        if let Err(e) = validate_audio(&audio_data, sample_rate) {
            warn!("Audio validation failed: {e}");
            return DaemonResponse::error(&format!("Invalid audio data: {e}"));
        }

        debug!("Audio validation completed");

        // Calculate audio level for visualization
        let audio_level = if audio_data.is_empty() {
            0.0
        } else {
            let rms: f32 =
                (audio_data.iter().map(|&x| x * x).sum::<f32>() / audio_data.len() as f32).sqrt();
            let is_speech = rms > 0.02; // Use same threshold as client

            // Emit D-Bus audio level signal
            if let Some(ref dbus_manager) = self.dbus_manager {
                let audio_level_event = crate::services::dbus::AudioLevelEvent {
                    client_id: client_id.clone(),
                    timestamp: Utc::now().to_rfc3339(),
                    level: rms,
                    is_speech,
                };

                if let Err(e) = dbus_manager.emit_audio_level(audio_level_event).await {
                    warn!("Failed to emit D-Bus audio_level signal: {e}");
                } else {
                    debug!(
                        "Emitted D-Bus audio_level signal for client: {client_id}, level: {rms:.3}, speech: {is_speech}"
                    );
                }
            }

            // Broadcast audio level event via notification system
            if let Err(e) = self
                .notification_manager
                .broadcast_event(
                    "audio_level".to_string(),
                    client_id.clone(),
                    serde_json::json!({
                        "level": rms,
                        "is_speech": is_speech,
                        "timestamp": Utc::now().to_rfc3339()
                    }),
                )
                .await
            {
                warn!("Failed to broadcast audio level event: {e}");
            }

            rms
        };

        debug!("Audio level calculated: {audio_level:.3}");

        // Broadcast transcription started event
        if let Err(e) = self
            .notification_manager
            .broadcast_event(
                "transcription_started".to_string(),
                client_id.clone(),
                serde_json::json!({
                    "audio_length_ms": (audio_data.len() as f64 / f64::from(sample_rate)) * 1000.0,
                    "sample_rate": sample_rate,
                    "timestamp": Utc::now().to_rfc3339()
                }),
            )
            .await
        {
            warn!("Failed to broadcast transcription started event: {e}");
        }

        debug!("Transcription started event broadcasted");

        // Emit D-Bus transcription started signal
        if let Some(ref dbus_manager) = self.dbus_manager {
            let event = crate::services::dbus::TranscriptionStartedEvent {
                client_id: client_id.clone(),
                timestamp: Utc::now().to_rfc3339(),
                audio_length_ms: (audio_data.len() as f64 / f64::from(sample_rate)) * 1000.0,
                sample_rate,
            };

            if let Err(e) = dbus_manager.emit_transcription_started(event).await {
                warn!("Failed to emit D-Bus transcription_started signal: {e}");
            } else {
                debug!("Emitted D-Bus transcription_started signal for client: {client_id}");
            }
        }

        // Process audio
        let processed_audio = match self.audio_processor.process_audio(&audio_data, sample_rate) {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to process audio: {e}");
                return DaemonResponse::error(&format!("Failed to process audio: {e}"));
            }
        };

        // Clone the model Arc for the blocking task
        let model_clone = Arc::clone(&self.model);

        // Run transcription in a blocking task to avoid blocking the async runtime
        let transcription_result = tokio::task::spawn_blocking(move || {
            let start_time = std::time::Instant::now();

            // Get exclusive write access to the model
            let mut model_guard = model_clone.blocking_write();

            if let Some(model) = model_guard.as_mut() {
                match model.transcribe_audio(&processed_audio, 16000) {
                    Ok(text) => {
                        let duration = start_time.elapsed();
                        info!("Transcription completed in {duration:?}: '{text}'");
                        Ok((text, duration))
                    }
                    Err(e) => {
                        // For transcription errors (like Voxtral mel generation issues),
                        // return empty string instead of failing the entire request
                        warn!("Transcription failed, returning empty result: {e}");
                        let duration = start_time.elapsed();
                        Ok((String::new(), duration))
                    }
                }
            } else {
                error!("Model not loaded");
                Err(anyhow::anyhow!("Model not loaded"))
            }
        })
        .await;

        // Handle the result of the blocking task
        match transcription_result {
            Ok(Ok((transcription, duration))) => {
                // Broadcast transcription completed event
                let _ = self
                    .notification_manager
                    .broadcast_event(
                        "transcription_completed".to_string(),
                        client_id.clone(),
                        serde_json::json!({
                            "transcription": transcription,
                            "duration_ms": duration.as_millis(),
                            "timestamp": Utc::now().to_rfc3339()
                        }),
                    )
                    .await;

                // Emit D-Bus transcription completed signal
                if let Some(ref dbus_manager) = self.dbus_manager {
                    let event = crate::services::dbus::TranscriptionCompletedEvent {
                        client_id: client_id.clone(),
                        timestamp: Utc::now().to_rfc3339(),
                        transcription: transcription.clone(),
                        duration_ms: u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
                    };

                    if let Err(e) = dbus_manager.emit_transcription_completed(event).await {
                        warn!("Failed to emit D-Bus transcription_completed signal: {e}");
                    } else {
                        debug!(
                            "Emitted D-Bus transcription_completed signal for client: {client_id}"
                        );
                    }
                }

                DaemonResponse::success().with_transcription(transcription)
            }
            Ok(Err(e)) => {
                // Transcription error
                let _ = self
                    .notification_manager
                    .broadcast_event(
                        "transcription_failed".to_string(),
                        client_id,
                        serde_json::json!({
                            "error": e.to_string(),
                            "timestamp": Utc::now().to_rfc3339()
                        }),
                    )
                    .await;
                DaemonResponse::error(&format!("Transcription failed: {e}"))
            }
            Err(e) => {
                // Task join error
                error!("Transcription task failed: {e}");
                let _ = self
                    .notification_manager
                    .broadcast_event(
                        "transcription_failed".to_string(),
                        client_id,
                        serde_json::json!({
                            "error": format!("Task execution failed: {}", e),
                            "timestamp": Utc::now().to_rfc3339()
                        }),
                    )
                    .await;
                DaemonResponse::error(&format!("Task execution failed: {e}"))
            }
        }
    }
}
