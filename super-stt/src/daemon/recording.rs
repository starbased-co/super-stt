// SPDX-License-Identifier: GPL-3.0-only

use crate::audio::recorder::DaemonAudioRecorder;
use crate::daemon::types::SuperSTTDaemon;
use crate::output::keyboard::Simulator;
use crate::services::dbus::ListeningEvent;
use anyhow::{Context, Result};
use chrono::Utc;
use log::{debug, error, info, warn};
use std::sync::Arc;
use super_stt_shared::models::protocol::DaemonResponse;

// Removed PreviewContext - no longer needed with simplified architecture

impl SuperSTTDaemon {
    /// Handle record command - direct recording in daemon (legacy method)
    pub async fn handle_record(
        &self,
        keyboard_simulator: &mut Simulator,
        write_mode: bool,
    ) -> DaemonResponse {
        self.handle_record_internal(keyboard_simulator, write_mode)
            .await
    }

    /// Internal record handling implementation
    pub async fn handle_record_internal(
        &self,
        keyboard_simulator: &mut Simulator,
        write_mode: bool,
    ) -> DaemonResponse {
        // Check if already recording - prevent multiple simultaneous recordings
        {
            let is_recording_guard = self.is_recording.read().await;
            if *is_recording_guard {
                warn!("Recording request rejected - already recording");
                return DaemonResponse::error(
                    "Recording already in progress. Please wait for current recording to complete.",
                );
            }
        }

        // Wait for recording to complete and return the transcription
        match self
            .record_and_transcribe(keyboard_simulator, write_mode)
            .await
        {
            Ok((transcription, _preview_typed_count)) => {
                if transcription.trim().is_empty() {
                    info!("🎤 Recording completed - No speech detected");
                    DaemonResponse::success()
                        .with_message("Recording completed - No speech detected".to_string())
                        .with_transcription(String::new())
                } else {
                    info!("🎤 Recording completed: '{transcription}'");

                    // Final typing will be handled in record_and_transcribe_impl method
                    // after proper GPU completion and sequencing

                    DaemonResponse::success()
                        .with_message("Recording completed successfully".to_string())
                        .with_transcription(transcription)
                }
            }
            Err(e) => {
                error!("🎤 Recording failed: {e}");
                DaemonResponse::error(&format!("Recording failed: {e}"))
            }
        }
    }

    /// Record audio directly in daemon and transcribe
    ///
    /// # Errors
    ///
    /// Returns an error if recording setup fails, audio processing fails,
    /// or if model execution encounters a fatal error.
    ///
    /// # Panics
    ///
    /// Panics if internal locks (e.g., audio theme or buffers) are poisoned.
    pub async fn record_and_transcribe(
        &self,
        keyboard_simulator: &mut Simulator,
        write_mode: bool,
    ) -> Result<(String, usize)> {
        self.record_and_transcribe_impl(keyboard_simulator, write_mode)
            .await
    }

    /// Internal implementation split out to reduce function size
    #[allow(clippy::too_many_lines)]
    async fn record_and_transcribe_impl(
        &self,
        keyboard_simulator: &mut Simulator,
        write_mode: bool,
    ) -> Result<(String, usize)> {
        info!("Starting direct audio recording in daemon with simplified architecture");

        // Set up recording state and create recorder
        let mut recorder = self.setup_recording_session().await?;

        // Simple architecture: just record audio, then process sequentially
        let mut preview_typed_count = 0;

        // Set up preview typing if enabled
        let mut typing_handle = None;
        if write_mode
            && self
                .preview_typing_enabled
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            // Create typing thread for preview
            typing_handle = match crate::output::typing_thread::TypingThreadHandle::spawn() {
                Ok(handle) => {
                    info!("Preview typing enabled - created typing thread");
                    Some(handle)
                }
                Err(e) => {
                    warn!("Failed to create typing thread: {e}");
                    None
                }
            };
        }

        // Start preview transcription if typing enabled
        let preview_session_id = if let Some(ref handle) = typing_handle {
            let input_rate = recorder.detect_default_input_sample_rate().ok();
            match self
                .realtime_manager
                .start_session("preview".to_string(), input_rate, None)
                .await
            {
                Ok(mut preview_rx) => {
                    let handle_clone = handle.clone();
                    tokio::spawn(async move {
                        while let Ok(text) = preview_rx.recv().await {
                            if let Err(e) = handle_clone.update_preview(text).await {
                                debug!("Preview typing failed: {e}");
                                break;
                            }
                        }
                    });
                    Some("preview".to_string())
                }
                Err(e) => {
                    warn!("Failed to start preview session: {e}");
                    None
                }
            }
        } else {
            None
        };

        // Emit D-Bus listening started event
        self.emit_listening_started_dbus(write_mode).await;

        // Record audio with optional preview streaming
        let audio_data = if let Some(ref session_id) = preview_session_id {
            // Record with preview streaming
            self.record_with_preview_streaming(&mut recorder, session_id)
                .await?
        } else {
            // Simple recording without preview
            recorder
                .record_until_silence_with_streaming(Arc::clone(&self.udp_streamer), None)
                .await?
        };

        // STEP 1: GPU preview finish - wait for preview transcription to completely stop
        if let Some(session_id) = preview_session_id {
            info!("Step 1: Waiting for GPU preview transcription to finish...");
            let _ = self.realtime_manager.stop_session(&session_id).await; // This now waits for GPU completion

            if let Some(ref handle) = typing_handle {
                preview_typed_count = handle.get_char_count().await.unwrap_or(0);
            }
            info!("Step 1 complete: GPU preview transcription finished");
        }

        // STEP 2: Clear preview (now safe since GPU preview finished)
        if write_mode && preview_typed_count > 0 {
            info!("Step 2: Clearing {preview_typed_count} preview characters");
            if let Some(ref handle) = typing_handle
                && let Err(e) = handle.clear().await
            {
                warn!("Failed to clear preview: {e}");
            }
            info!("Step 2 complete: Preview cleared");
        }

        // STEP 3: Loader start + STEP 4: GPU final transcription + STEP 5: Loader end
        info!("Step 3-5: Starting loader, running GPU final transcription, stopping loader");
        let transcription_result = self
            .transcribe_with_spinner(keyboard_simulator, &audio_data, write_mode, &typing_handle)
            .await?;
        info!("Step 3-5 complete: Final GPU transcription finished");

        // Finalize recording session and return result
        let preview_count_atomic =
            std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(preview_typed_count));
        self.finalize_recording_session(&transcription_result, &preview_count_atomic)
            .await;

        // STEP 6: Type final transcript
        if write_mode {
            info!("Step 6: Typing final transcription");
            if let Some(handle) = typing_handle {
                let processed_text =
                    crate::output::preview::Typer::preprocess_text(&transcription_result, false);
                let final_text = format!("{processed_text} ");

                if let Err(e) = handle.process_final(final_text).await {
                    warn!("Failed to process final transcription: {e}");
                } else {
                    info!("Step 6 complete: Final transcription typed successfully");
                }

                // Shutdown typing thread
                handle.shutdown();
            } else {
                // No preview typing, type directly
                let processed_text =
                    crate::output::preview::Typer::preprocess_text(&transcription_result, false);
                let final_text = format!("{processed_text} ");
                if let Err(e) = keyboard_simulator.type_text(&final_text) {
                    warn!("Failed to type final transcription: {e}");
                } else {
                    info!("Step 6 complete: Final transcription typed directly");
                }
            }
        }

        info!(
            "🎯 Perfect sequence completed: GPU preview finish → clear → loader → GPU final → type final"
        );

        Ok((transcription_result, preview_typed_count))
    }

    /// Record audio and stream to preview session
    async fn record_with_preview_streaming(
        &self,
        recorder: &mut DaemonAudioRecorder,
        session_id: &str,
    ) -> Result<Vec<f32>> {
        // Create channel to send audio to preview transcription
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(Vec<f32>, u32)>();

        // Spawn task to forward audio to realtime manager
        let manager = Arc::clone(&self.realtime_manager);
        let session_id = session_id.to_string();
        let _forwarder = tokio::spawn(async move {
            while let Some((samples, sr)) = rx.recv().await {
                if let Err(e) = manager.process_audio_chunk(&session_id, samples, sr).await {
                    debug!("Preview audio processing failed: {e}");
                    break;
                }
            }
        });

        // Record audio with preview streaming
        recorder
            .record_until_silence_with_streaming(Arc::clone(&self.udp_streamer), Some(tx))
            .await
    }

    /// Set up recording state and create audio recorder
    async fn setup_recording_session(&self) -> Result<DaemonAudioRecorder> {
        // Double-check recording state and set atomically
        {
            let mut is_recording_guard = self.is_recording.write().await;
            if *is_recording_guard {
                error!("Recording already in progress - rejecting duplicate request");
                return Err(anyhow::anyhow!("Recording already in progress"));
            }
            // Set recording state to true atomically
            *is_recording_guard = true;
        }
        self.broadcast_recording_state_change(true).await;

        // Create audio recorder with current theme
        let current_theme = self.get_audio_theme();
        DaemonAudioRecorder::new_with_theme(current_theme)
            .context("Failed to create audio recorder")
    }

    /// Set up preview session for real-time typing (legacy - kept for reference)
    #[allow(dead_code)]
    #[allow(clippy::too_many_lines)]
    async fn setup_preview_session(
        &self,
        write_mode: bool,
        recorder: &mut DaemonAudioRecorder,
    ) -> (
        Option<tokio::sync::mpsc::UnboundedSender<(Vec<f32>, u32)>>,
        Option<tokio::task::JoinHandle<()>>,
        Option<tokio::task::JoinHandle<()>>,
        Option<crate::output::typing_thread::TypingThreadHandle>,
        std::sync::Arc<std::sync::atomic::AtomicBool>,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
        String,
    ) {
        let mut preview_audio_tx: Option<tokio::sync::mpsc::UnboundedSender<(Vec<f32>, u32)>> =
            None;
        let mut preview_forwarder: Option<tokio::task::JoinHandle<()>> = None;
        let mut preview_typer: Option<tokio::task::JoinHandle<()>> = None;
        let mut typing_handle_option: Option<crate::output::typing_thread::TypingThreadHandle> =
            None;
        let preview_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let preview_typed_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let preview_client_id = "record_preview".to_string();

        if write_mode
            && self
                .preview_typing_enabled
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            info!("Preview typing is enabled - setting up preview session");
            // Detect the input sample rate the recorder will use
            let input_rate = match recorder.detect_default_input_sample_rate() {
                Ok(r) => Some(r),
                Err(e) => {
                    warn!("Could not detect input sample rate for preview: {e}");
                    None
                }
            };
            // Start real-time session and subscribe to updates
            match self
                .realtime_manager
                .start_session(preview_client_id.clone(), input_rate, None)
                .await
            {
                Ok(mut rx) => {
                    info!(
                        "Preview session started (client_id='{preview_client_id}', input_rate={input_rate:?})"
                    );
                    // Create single typing thread handle
                    let typing_handle =
                        match crate::output::typing_thread::TypingThreadHandle::spawn() {
                            Ok(handle) => handle,
                            Err(e) => {
                                warn!("Failed to create typing thread: {e}");
                                return (
                                    None,
                                    None,
                                    None,
                                    None,
                                    preview_cancel,
                                    preview_typed_count,
                                    preview_client_id,
                                );
                            }
                        };
                    let typing_handle_clone = typing_handle.clone();
                    typing_handle_option = Some(typing_handle.clone());

                    // Spawn task to forward transcriptions to typing thread
                    let cancel_flag = std::sync::Arc::clone(&preview_cancel);
                    let typed_counter = std::sync::Arc::clone(&preview_typed_count);
                    preview_typer = Some(tokio::spawn(async move {
                        let mut last = String::new();
                        let mut last_update_time = std::time::Instant::now();

                        while !cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            // Check cancel flag
                            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                break;
                            }
                            let msg = tokio::select! {
                                res = rx.recv() => res,
                                () = tokio::time::sleep(std::time::Duration::from_millis(50)) => continue,
                            };
                            let Ok(new_text) = msg else { break };

                            // Check cancel flag again after receiving message (race condition protection)
                            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                debug!("Ignoring late preview update - recording stopped");
                                break;
                            }

                            // Skip duplicate updates (same text received within 100ms)
                            if new_text == last
                                && last_update_time.elapsed()
                                    < std::time::Duration::from_millis(100)
                            {
                                debug!("Skipping duplicate preview update");
                                continue;
                            }

                            info!(
                                "Preview update: '{}' -> '{}'",
                                last.chars().take(30).collect::<String>(),
                                new_text.chars().take(30).collect::<String>()
                            );

                            // Send to typing thread
                            if let Err(e) =
                                typing_handle_clone.update_preview(new_text.clone()).await
                            {
                                warn!("Failed to update preview: {e}");
                                break;
                            }

                            // Update counter from typing thread
                            if let Ok(count) = typing_handle_clone.get_char_count().await {
                                typed_counter.store(count, std::sync::atomic::Ordering::Relaxed);
                            }

                            last = new_text;
                            last_update_time = std::time::Instant::now();
                        }

                        // Clear all preview text before exiting
                        let chars_to_clear =
                            typed_counter.load(std::sync::atomic::Ordering::Relaxed);
                        if chars_to_clear > 0 {
                            info!("Clearing {chars_to_clear} preview characters before exit");
                            if let Err(e) = typing_handle_clone.clear().await {
                                warn!("Failed to clear preview: {e}");
                            }
                        }

                        // Shutdown typing thread
                        typing_handle.shutdown();

                        info!(
                            "Preview typer exiting. Final chars on screen: {}",
                            typed_counter.load(std::sync::atomic::Ordering::Relaxed)
                        );
                    }));

                    // Create channel for audio forwarding to real-time manager
                    let (tx, mut rx_audio) =
                        tokio::sync::mpsc::unbounded_channel::<(Vec<f32>, u32)>();
                    preview_audio_tx = Some(tx);
                    let manager = std::sync::Arc::clone(&self.realtime_manager);
                    let client_id = preview_client_id.clone();
                    let cancel_flag = std::sync::Arc::clone(&preview_cancel);
                    preview_forwarder = Some(tokio::spawn(async move {
                        while let Some((samples, sr)) = rx_audio.recv().await {
                            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                break;
                            }
                            let _ = manager.process_audio_chunk(&client_id, samples, sr).await;
                        }
                    }));
                }
                Err(e) => {
                    warn!("Failed to start real-time preview session: {e}");
                }
            }
        } else if write_mode {
            info!("Preview typing is disabled - skipping preview session");
        }

        (
            preview_audio_tx,
            preview_forwarder,
            preview_typer,
            typing_handle_option,
            preview_cancel,
            preview_typed_count,
            preview_client_id,
        )
    }

    /// Emit D-Bus listening started event
    async fn emit_listening_started_dbus(&self, write_mode: bool) {
        if let Some(ref dbus_manager) = self.dbus_manager {
            let event = ListeningEvent {
                client_id: "daemon_recorder".to_string(),
                timestamp: Utc::now().to_rfc3339(),
                write_mode,
                timeout_seconds: 0,
                audio_level: 0.0,
            };

            if let Err(e) = dbus_manager.emit_listening_started(event).await {
                warn!("Failed to emit D-Bus listening_started signal: {e}");
            }
        }
    }

    /// Record audio and clean up preview session (legacy - kept for reference)
    #[allow(dead_code)]
    async fn record_audio_and_cleanup_preview(
        &self,
        mut recorder: DaemonAudioRecorder,
        _legacy_param: (),
        _write_mode: bool,
    ) -> Result<Vec<f32>> {
        // Legacy method - replaced with simplified architecture
        recorder
            .record_until_silence_with_streaming(Arc::clone(&self.udp_streamer), None)
            .await
    }

    /// Transcribe audio with spinner if needed
    async fn transcribe_with_spinner(
        &self,
        keyboard_simulator: &mut Simulator,
        audio_data: &[f32],
        write_mode: bool,
        typing_handle: &Option<crate::output::typing_thread::TypingThreadHandle>,
    ) -> Result<String> {
        // If we'll type the result, show a simple spinner by typing characters and backspacing
        // This indicates work while transcription runs.
        let mut spinner_handle: Option<tokio::task::JoinHandle<()>> = None;
        let spinner_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        // Track how many temporary spinner characters are visible (0-3)
        let visible_temp_chars = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        if write_mode && typing_handle.is_some() {
            // Use the typing thread for loader animation instead of separate Enigo instance
            let handle = typing_handle.as_ref().unwrap();
            let cancel_flag = std::sync::Arc::clone(&spinner_cancel);
            let handle_clone = handle.clone();
            
            spinner_handle = Some(tokio::spawn(async move {
                let mut dots = 0;
                
                while !cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    // Add dots (up to 3)
                    while dots < 3 && !cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                        if let Err(_) = handle_clone.update_preview(".".repeat(dots + 1)).await {
                            return;
                        }
                        dots += 1;
                        tokio::time::sleep(std::time::Duration::from_millis(90)).await;
                    }
                    
                    // Remove dots
                    while dots > 0 && !cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                        dots -= 1;
                        if let Err(_) = handle_clone.update_preview(".".repeat(dots)).await {
                            return;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(90)).await;
                    }
                }
                
                // Clean up - clear all dots
                let _ = handle_clone.update_preview(String::new()).await;
            }));
        }

        // Process audio
        let processed_audio = self
            .audio_processor
            .process_audio(audio_data, 16000)
            .context("Failed to process audio")?;

        // Transcribe the audio
        let transcription_result = {
            // Clone the model Arc for the blocking task
            let model_clone = Arc::clone(&self.model);

            // Run transcription in a blocking task to avoid blocking the async runtime
            tokio::task::spawn_blocking(move || {
                let start_time = std::time::Instant::now();

                // Get exclusive write access to the model
                let mut model_guard = model_clone.blocking_write();

                if let Some(model) = model_guard.as_mut() {
                    match model.transcribe_audio(&processed_audio, 16000) {
                        Ok(text) => {
                            let duration = start_time.elapsed();
                            info!("Transcription completed in {duration:?}: '{text}'");
                            Ok(text)
                        }
                        Err(e) => {
                            // For transcription errors (like Voxtral mel generation issues),
                            // return empty string instead of failing the entire request
                            warn!("Transcription failed, returning empty result: {e}");
                            Ok(String::new())
                        }
                    }
                } else {
                    error!("Model not loaded");
                    Err(anyhow::anyhow!("Model not loaded"))
                }
            })
            .await
            .map_err(|e| anyhow::anyhow!("Transcription task failed: {}", e))?
        };

        // Stop spinner if it was started
        if let Some(handle) = spinner_handle.take() {
            spinner_cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            // Wait for the spinner task to exit and clean up
            if let Err(e) = handle.await {
                warn!("Spinner task panicked: {e}");
            }
        }

        transcription_result
    }

    /// Finalize recording session and emit events
    async fn finalize_recording_session(
        &self,
        _transcription_result: &str,
        _preview_typed_count: &std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) {
        // Reset recording state
        {
            let mut is_recording_guard = self.is_recording.write().await;
            *is_recording_guard = false;
        }

        // Emit D-Bus listening stopped event
        if let Some(ref dbus_manager) = self.dbus_manager {
            let event = crate::services::dbus::ListeningStoppedEvent {
                client_id: "daemon_recorder".to_string(),
                timestamp: Utc::now().to_rfc3339(),
                transcription_success: true, // We only call this on success
                error: String::new(),
            };

            if let Err(e) = dbus_manager.emit_listening_stopped(event).await {
                warn!("Failed to emit D-Bus listening_stopped signal: {e}");
            }
        }
    }
}
