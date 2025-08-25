// SPDX-License-Identifier: GPL-3.0-only

use crate::output::preview::Typer;
use anyhow::Result;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

/// Keyboard simulation utilities for text input
pub struct Simulator {
    typing_chunk: usize,
    backspace_batch_size: usize,
    enigo: Enigo,
}

impl Simulator {
    /// Creates a new Simulator instance
    ///
    /// # Errors
    ///
    /// Returns an error if the enigo keyboard connection cannot be established
    pub fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow::anyhow!("Failed to initialize keyboard simulator: {e}"))?;
        
        Ok(Self {
            typing_chunk: 64,
            backspace_batch_size: 20,
            enigo,
        })
    }
}

impl Simulator {
    // SPDX-License-Identifier: GPL-3.0-only

    /// Type text using keyboard simulation
    ///
    /// # Errors
    ///
    /// Returns an error if keyboard input cannot be simulated or
    /// if the typing task fails to execute.
    pub fn type_text(&mut self, text: &str) -> Result<()> {
        // Type in modest chunks to improve reliability
        let mut i = 0;
        let chars: Vec<char> = text.chars().collect();
        while i < chars.len() {
            let end = (i + self.typing_chunk).min(chars.len());
            let segment: String = chars[i..end].iter().collect();
            self.enigo
                .text(&segment)
                .map_err(|e| anyhow::anyhow!("Failed to type segment: {}", e))?;
            i = end;
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        Ok(())
    }

    /// Backspace a given number of characters
    ///
    /// # Errors
    /// This function can fail if the enigo initialization fails or if the text typing task fails.
    pub fn backspace_n(&mut self, n: usize) -> Result<()> {
        let mut remaining = n;
        while remaining > 0 {
            let batch_size = remaining.min(self.backspace_batch_size);
            for _ in 0..batch_size {
                let _ = self.enigo.key(Key::Backspace, Direction::Click);
            }
            remaining -= batch_size;

            // Small pause between batches and after finishing to reduce system load
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        Ok(())
    }

    /// In a single input session, backspace preview chars then type final text
    ///
    /// # Errors
    /// This function can fail if the enigo initialization fails or if the text typing task fails.
    pub fn replace_preview_and_type(&mut self, preview_chars: usize, text: &str) -> Result<()> {
        // Use unified preprocessor for final text (adds period, capitalizes)
        let processed_text = Typer::preprocess_text(text, false);
        let text_to_type = processed_text + " ";

        // Erase preview in batches
        if preview_chars > 0 {
            self.backspace_n(preview_chars)?;
        }

        self.type_text(&text_to_type)?;

        Ok(())
    }
}
