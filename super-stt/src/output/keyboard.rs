// SPDX-License-Identifier: GPL-3.0-only
use anyhow::Result;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use log::info;

/// Keyboard simulation utilities for text input
pub struct KeyboardSimulator;

impl KeyboardSimulator {
    /// Typing chunk size for reliable text input
    const TYPING_CHUNK: usize = 64;
    /// Backspace batch size for efficient deletion
    const BACKSPACE_BATCH_SIZE: usize = 20;

    /// Type text using keyboard simulation
    ///
    /// # Errors
    ///
    /// Returns an error if keyboard input cannot be simulated or
    /// if the typing task fails to execute.
    pub async fn type_text(text: &str) -> Result<()> {
        // Add a space at the end of the text to allow user to continue typing more easily
        let text_to_type = text.to_string() + " ";

        // Run text typing in a blocking task to avoid blocking the async runtime
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut enigo = Enigo::new(&Settings::default())
                .map_err(|e| anyhow::anyhow!("Failed to initialize enigo: {}", e))?;

            // Type in modest chunks to improve reliability
            let mut i = 0;
            let chars: Vec<char> = text_to_type.chars().collect();
            while i < chars.len() {
                let end = (i + Self::TYPING_CHUNK).min(chars.len());
                let segment: String = chars[i..end].iter().collect();
                enigo
                    .text(&segment)
                    .map_err(|e| anyhow::anyhow!("Failed to type segment: {}", e))?;
                i = end;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("Text typing task failed: {}", e))?
    }

    /// In a single input session, backspace preview chars then type final text
    ///
    /// # Errors
    /// This function can fail if the enigo initialization fails or if the text typing task fails.
    pub async fn replace_preview_and_type(preview_chars: usize, text: &str) -> Result<()> {
        let text_to_type = text.to_string() + " ";

        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut enigo = Enigo::new(&Settings::default())
                .map_err(|e| anyhow::anyhow!("Failed to initialize enigo: {}", e))?;

            // Erase preview in batches
            if preview_chars > 0 {
                info!(
                    "Erasing preview in unified session: {preview_chars} chars (batch size: {})",
                    Self::BACKSPACE_BATCH_SIZE
                );
                let mut remaining = preview_chars;
                while remaining > 0 {
                    let batch_size = remaining.min(Self::BACKSPACE_BATCH_SIZE);
                    for _ in 0..batch_size {
                        let _ = enigo.key(Key::Backspace, Direction::Click);
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    remaining -= batch_size;

                    // Small pause between batches to reduce system load
                    if remaining > 0 {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                }
                // Give focus a moment to settle
                std::thread::sleep(std::time::Duration::from_millis(120));
            }

            // Type final text in chunks for reliability
            let mut i = 0;
            let chars: Vec<char> = text_to_type.chars().collect();
            info!("Typing final transcription ({} chars)", chars.len());
            while i < chars.len() {
                let end = (i + Self::TYPING_CHUNK).min(chars.len());
                let segment: String = chars[i..end].iter().collect();
                enigo
                    .text(&segment)
                    .map_err(|e| anyhow::anyhow!("Failed to type segment: {}", e))?;
                i = end;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("Replace+type task failed: {}", e))?
    }

    /// Backspace N characters using keyboard simulation
    ///
    /// # Errors
    /// Returns an error if the keyboard simulation fails.
    #[allow(dead_code)]
    pub async fn backspace_n(n: usize) -> Result<()> {
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut enigo = Enigo::new(&Settings::default())
                .map_err(|e| anyhow::anyhow!("Failed to initialize enigo: {}", e))?;

            let mut remaining = n;
            while remaining > 0 {
                let batch_size = remaining.min(Self::BACKSPACE_BATCH_SIZE);
                for _ in 0..batch_size {
                    let _ = enigo.key(Key::Backspace, Direction::Click);
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                remaining -= batch_size;

                // Small pause between batches to reduce system load
                if remaining > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("Backspace task failed: {}", e))?
    }
}
