// SPDX-License-Identifier: GPL-3.0-only
use crate::audio::recorder::DaemonAudioRecorder;
use crate::daemon::types::SuperSTTDaemon;
use crate::output::keyboard::KeyboardSimulator;
use crate::output::preview_typing::PreviewTyper;
use crate::services::dbus::ListeningEvent;
use anyhow::{Context, Result};
use chrono::Utc;
use log::{debug, error, info, warn};
use std::sync::Arc;
use super_stt_shared::models::protocol::DaemonResponse;
use tokio::net::UnixStream;

const ERASE_BATCH_SIZE: usize = 20;

/// Context for preview session cleanup
struct PreviewContext {
    audio_tx: Option<tokio::sync::mpsc::UnboundedSender<(Vec<f32>, u32)>>,
    forwarder: Option<tokio::task::JoinHandle<()>>,
    typer: Option<tokio::task::JoinHandle<()>>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    client_id: String,
}

impl SuperSTTDaemon {
    /// Handle record command with process authentication for write mode
    pub async fn handle_record_with_auth(
        &self,
        client_id: String,
        write_mode: bool,
        stream: Option<&UnixStream>,
    ) -> DaemonResponse {
        info!("Handling record command, write_mode: {write_mode}, client: {client_id}");

        // Check write permission if write_mode is requested
        if write_mode {
            if let Some(stream) = stream {
                if !self.process_auth.verify_write_permission(stream) {
                    warn!("Write mode denied - client process not verified: {client_id}");
                    return DaemonResponse::error(
                        "Write mode access denied. Only the official stt client can use keyboard injection.",
                    );
                }
            } else {
                warn!("Write mode denied - no stream available for verification");
                return DaemonResponse::error(
                    "Write mode access denied. Stream authentication required.",
                );
            }
        }

        // Proceed with the actual recording
        self.handle_record_internal(client_id, write_mode).await
    }

    /// Handle record command - direct recording in daemon (legacy method)
    pub async fn handle_record(&self, client_id: String, write_mode: bool) -> DaemonResponse {
        self.handle_record_internal(client_id, write_mode).await
    }

    /// Internal record handling implementation
    async fn handle_record_internal(&self, _client_id: String, write_mode: bool) -> DaemonResponse {
        info!("Handling record command, write_mode: {write_mode}");

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
        match self.record_and_transcribe(write_mode).await {
            Ok((transcription, preview_typed_count)) => {
                if transcription.trim().is_empty() {
                    info!("🎤 Recording completed - No speech detected");
                    DaemonResponse::success()
                        .with_message("Recording completed - No speech detected".to_string())
                        .with_transcription(String::new())
                } else {
                    info!("🎤 Recording completed: '{transcription}'");

                    // Handle delete preview and final text typing if write_mode is true
                    if write_mode {
                        // Perform erase + final typing in a single input session to avoid compositor races
                        if let Err(e) = KeyboardSimulator::replace_preview_and_type(
                            preview_typed_count,
                            &transcription,
                        )
                        .await
                        {
                            warn!("Failed to erase+type final transcription: {e}");
                        }
                    }

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
    pub async fn record_and_transcribe(&self, write_mode: bool) -> Result<(String, usize)> {
        self.record_and_transcribe_impl(write_mode).await
    }

    /// Internal implementation split out to reduce function size
    async fn record_and_transcribe_impl(&self, write_mode: bool) -> Result<(String, usize)> {
        info!("Starting direct audio recording in daemon");

        // Set up recording state and create recorder
        let mut recorder = self.setup_recording_session().await?;

        // Set up real-time preview if in write mode
        let (
            preview_audio_tx,
            preview_forwarder,
            preview_typer,
            preview_cancel,
            preview_typed_count,
            preview_client_id,
        ) = self.setup_preview_session(write_mode, &mut recorder).await;

        // Emit D-Bus listening started event
        self.emit_listening_started_dbus(write_mode).await;

        // Record audio and handle preview cleanup
        let preview_context = PreviewContext {
            audio_tx: preview_audio_tx,
            forwarder: preview_forwarder,
            typer: preview_typer,
            cancel: preview_cancel,
            client_id: preview_client_id,
        };
        let audio_data = self
            .record_audio_and_cleanup_preview(recorder, preview_context, write_mode)
            .await?;

        // Transcribe audio with spinner if needed
        let transcription_result = self
            .transcribe_with_spinner(&audio_data, write_mode)
            .await?;

        // Finalize recording session and return result
        self.finalize_recording_session(&transcription_result, &preview_typed_count)
            .await;

        Ok((
            transcription_result,
            preview_typed_count.load(std::sync::atomic::Ordering::Relaxed),
        ))
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

    /// Set up preview session for real-time typing
    #[allow(clippy::too_many_lines)]
    async fn setup_preview_session(
        &self,
        write_mode: bool,
        recorder: &mut DaemonAudioRecorder,
    ) -> (
        Option<tokio::sync::mpsc::UnboundedSender<(Vec<f32>, u32)>>,
        Option<tokio::task::JoinHandle<()>>,
        Option<tokio::task::JoinHandle<()>>,
        std::sync::Arc<std::sync::atomic::AtomicBool>,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
        String,
    ) {
        let mut preview_audio_tx: Option<tokio::sync::mpsc::UnboundedSender<(Vec<f32>, u32)>> =
            None;
        let mut preview_forwarder: Option<tokio::task::JoinHandle<()>> = None;
        let mut preview_typer: Option<tokio::task::JoinHandle<()>> = None;
        let preview_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let preview_typed_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let preview_client_id = "record_preview".to_string();

        if write_mode {
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
                    // Spawn typer task: types incremental suffixes as they arrive
                    let cancel_flag = std::sync::Arc::clone(&preview_cancel);
                    let typed_counter = std::sync::Arc::clone(&preview_typed_count);
                    preview_typer = Some(tokio::spawn(async move {
                        use enigo::{Direction, Enigo, Key, Keyboard, Settings};
                        let Ok(mut enigo) = Enigo::new(&Settings::default()) else {
                            return;
                        };
                        let mut last = String::new();
                        let mut actually_typed = String::new(); // Track what we actually typed
                        let mut last_update_time = std::time::Instant::now();

                        while !cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            let msg = tokio::select! {
                                res = rx.recv() => res,
                                () = tokio::time::sleep(std::time::Duration::from_millis(50)) => continue,
                            };
                            let Ok(new_text) = msg else { break };

                            // Skip duplicate updates (same text received within 100ms)
                            if new_text == last
                                && last_update_time.elapsed()
                                    < std::time::Duration::from_millis(100)
                            {
                                debug!("Skipping duplicate preview update");
                                continue;
                            }

                            info!(
                                "Preview update: '{}' -> '{}' (typed so far: '{}')",
                                last.chars().take(30).collect::<String>(),
                                new_text.chars().take(30).collect::<String>(),
                                actually_typed.chars().take(30).collect::<String>()
                            );

                            let should_replace =
                                PreviewTyper::should_replace_preview(&actually_typed, &new_text);

                            if should_replace {
                                // Significant change detected - replace what we typed
                                let chars_to_erase = actually_typed.chars().count();
                                info!(
                                    "Preview correction needed: erasing {chars_to_erase} chars and retyping"
                                );

                                // Erase in batches
                                let mut remaining = chars_to_erase;
                                while remaining > 0 {
                                    let batch_size = remaining.min(ERASE_BATCH_SIZE);
                                    for _ in 0..batch_size {
                                        let _ = enigo.key(Key::Backspace, Direction::Click);
                                        std::thread::sleep(std::time::Duration::from_millis(1));
                                    }
                                    remaining -= batch_size;
                                    if remaining > 0 {
                                        std::thread::sleep(std::time::Duration::from_millis(10));
                                    }
                                }

                                // Type the new text
                                let _ = enigo.text(&new_text);
                                actually_typed.clone_from(&new_text);

                                typed_counter.store(
                                    actually_typed.chars().count(),
                                    std::sync::atomic::Ordering::Relaxed,
                                );

                                info!(
                                    "Preview replaced with: '{}'",
                                    new_text.chars().take(40).collect::<String>()
                                );
                            } else if new_text.starts_with(&actually_typed)
                                && new_text.len() > actually_typed.len()
                            {
                                // Perfect extension case: new text exactly extends what we typed
                                PreviewTyper::handle_perfect_extension(
                                    &mut enigo,
                                    &new_text,
                                    &mut actually_typed,
                                    &typed_counter,
                                );
                            } else if new_text.len() > actually_typed.len()
                                && actually_typed.len() < 50
                            {
                                // For shorter typed text, try smart append based on word overlap
                                PreviewTyper::handle_smart_append(
                                    &mut enigo,
                                    &new_text,
                                    &mut actually_typed,
                                    &typed_counter,
                                );
                            } else {
                                debug!(
                                    "Preview update ignored - incompatible or no clear extension"
                                );
                            }

                            last = new_text;
                            last_update_time = std::time::Instant::now();
                        }
                        info!(
                            "Preview typer exiting. Total preview chars typed: {}",
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
        }

        (
            preview_audio_tx,
            preview_forwarder,
            preview_typer,
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

    /// Record audio and clean up preview session
    async fn record_audio_and_cleanup_preview(
        &self,
        mut recorder: DaemonAudioRecorder,
        preview_context: PreviewContext,
        write_mode: bool,
    ) -> Result<Vec<f32>> {
        // Record audio until silence and broadcast samples if UDP streamer available
        let audio_data = recorder
            .record_until_silence_with_streaming(
                Arc::clone(&self.udp_streamer),
                preview_context.audio_tx,
            )
            .await
            .context("Failed to record audio with streaming")?;

        // Recording has stopped - notify clients immediately
        // Set recording state to false but don't reset the overall is_recording flag yet
        // We still need to keep is_recording=true to prevent new recordings during transcription
        self.broadcast_recording_state_change(false).await;

        // Stop preview session/tasks if any
        if write_mode {
            preview_context
                .cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
            let _ = self
                .realtime_manager
                .stop_session(&preview_context.client_id)
                .await;
            if let Some(handle) = preview_context.forwarder {
                let _ = handle.await;
            }
            if let Some(handle) = preview_context.typer {
                let _ = handle.await;
            }
        }

        info!(
            "Recording complete, processing {} samples",
            audio_data.len()
        );

        Ok(audio_data)
    }

    /// Transcribe audio with spinner if needed
    async fn transcribe_with_spinner(
        &self,
        audio_data: &[f32],
        write_mode: bool,
    ) -> Result<String> {
        // If we'll type the result, show a simple spinner by typing characters and backspacing
        // This indicates work while transcription runs.
        let mut spinner_handle: Option<tokio::task::JoinHandle<()>> = None;
        let spinner_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        // Track how many temporary spinner characters are visible (0-3)
        let visible_temp_chars = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        if write_mode {
            let cancel_flag = std::sync::Arc::clone(&spinner_cancel);
            let visible_temp_chars_inner = std::sync::Arc::clone(&visible_temp_chars);
            spinner_handle = Some(tokio::task::spawn_blocking(move || {
                use enigo::{Direction, Enigo, Key, Keyboard, Settings};

                // Initialize input simulator; if it fails, just skip spinner.
                let Ok(mut enigo) = Enigo::new(&Settings::default()) else {
                    return;
                };

                // Loop until cancelled: type three dots, then backspace three times
                while !cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    // Type three dots
                    for _ in 0..3 {
                        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                        let _ = enigo.text(".");
                        visible_temp_chars_inner.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        std::thread::sleep(std::time::Duration::from_millis(90));
                    }

                    // Backspace three times
                    for _ in 0..3 {
                        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                        let _ = enigo.key(Key::Backspace, Direction::Click);
                        let prev =
                            visible_temp_chars_inner.load(std::sync::atomic::Ordering::Relaxed);
                        if prev > 0 {
                            visible_temp_chars_inner
                                .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                        }
                        std::thread::sleep(std::time::Duration::from_millis(90));
                    }
                }
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
            // Wait for the spinner task to exit
            let _ = handle.await;

            // Clean up any remaining temporary characters (1-3)
            let leftover = visible_temp_chars
                .load(std::sync::atomic::Ordering::Relaxed)
                .min(3);
            if leftover > 0 {
                let _ = KeyboardSimulator::backspace_n(leftover).await;
                visible_temp_chars.store(0, std::sync::atomic::Ordering::Relaxed);
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

    /// Type text using keyboard simulation
    ///
    /// # Errors
    ///
    /// Returns an error if keyboard input cannot be simulated or
    /// if the typing task fails to execute.
    pub async fn type_text(&self, text: &str) -> Result<()> {
        KeyboardSimulator::type_text(text).await
    }
}
