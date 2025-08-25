// SPDX-License-Identifier: GPL-3.0-only

use enigo::{Enigo, Keyboard};
use log::{debug, info, warn};
use std::sync::{Arc, atomic::AtomicUsize};

/// Unified, simplified preview typer that combines the best of both approaches
pub struct Typer;

/// State for tracking preview updates
pub struct State {
    pub last_transcription: String,
    pub prev_text: String,
    /// Complete transcription built from all audio (for final output)
    pub full_session_text: String,
    /// When we last saw substantial text growth (to commit to full session)
    pub last_growth_time: std::time::Instant,
    /// History of transcriptions for stabilization
    pub text_storage: Vec<String>,
    /// Text confirmed by appearing in multiple transcriptions
    pub stabilized_text: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            last_transcription: String::new(),
            prev_text: String::new(),
            full_session_text: String::new(),
            last_growth_time: std::time::Instant::now(),
            text_storage: Vec::new(),
            stabilized_text: String::new(),
        }
    }
}

impl Typer {
    /// Preprocess text - normalize, remove ellipses, capitalize
    #[must_use]
    pub fn preprocess_text(text: &str, is_preview: bool) -> String {
        // Remove leading whitespaces
        let mut text = text.trim_start().to_string();

        // Remove starting ellipses if present
        if text.starts_with("...") {
            text = text[3..].to_string();
        }

        // Remove any leading whitespaces again after ellipses removal
        text = text.trim_start().to_string();

        // Normalize whitespace
        text = text.split_whitespace().collect::<Vec<_>>().join(" ");

        if text.is_empty() {
            return text;
        }

        // Uppercase the first letter
        let mut chars: Vec<char> = text.chars().collect();
        if let Some(first_char) = chars.first_mut() {
            *first_char = first_char.to_ascii_uppercase();
        }
        text = chars.iter().collect();

        // Add period for final output if it ends with alphanumeric
        if !is_preview && text.chars().last().is_some_and(char::is_alphanumeric) {
            text.push('.');
        }

        text
    }

    /// Simple extension check - much faster than complex word matching
    #[must_use]
    pub fn is_simple_extension(current: &str, new_text: &str) -> bool {
        if current.is_empty() {
            return !new_text.is_empty();
        }

        // Check if new text starts with current text
        new_text.starts_with(current) && new_text.len() > current.len()
    }

    /// Find common prefix between two strings
    #[must_use]
    pub fn find_common_prefix(text1: &str, text2: &str) -> usize {
        text1
            .chars()
            .zip(text2.chars())
            .take_while(|(c1, c2)| c1 == c2)
            .count()
    }

    /// Apply a simple differential update by backspacing and retyping from first difference
    pub fn apply_simple_diff(
        enigo: &mut Enigo,
        old_text: &str,
        new_text: &str,
        cancellation_token: &tokio_util::sync::CancellationToken,
    ) -> i32 {
        // Safety checks
        if old_text == new_text {
            return 0;
        }

        if old_text.is_empty() {
            if !new_text.is_empty() {
                if enigo.text(new_text).is_ok() {
                    debug!("Typed new text: {} chars", new_text.chars().count());
                    return i32::try_from(new_text.chars().count()).unwrap_or_default();
                }
                debug!("Failed to type new text");
                return 0;
            }
            return 0;
        }

        if new_text.is_empty() {
            let chars_to_delete = old_text.chars().count();
            let mut actually_deleted = 0i32;
            for i in 0..chars_to_delete {
                if enigo
                    .key(enigo::Key::Backspace, enigo::Direction::Click)
                    .is_ok()
                {
                    actually_deleted += 1;
                } else {
                    debug!("Failed to backspace at position {i}");
                    break;
                }
            }
            debug!("Cleared {actually_deleted}/{chars_to_delete} chars");
            return -actually_deleted;
        }

        let old_chars: Vec<char> = old_text.chars().collect();
        let new_chars: Vec<char> = new_text.chars().collect();

        // Find first different character position
        let common_prefix = Self::find_common_prefix(old_text, new_text);

        // Calculate what to delete and what to type
        let chars_to_delete = old_chars.len() - common_prefix;
        let text_to_type: String = new_chars[common_prefix..].iter().collect();

        debug!(
            "Simple diff: prefix={}, delete={}, type='{}'",
            common_prefix,
            chars_to_delete,
            text_to_type.chars().take(20).collect::<String>()
        );

        // Backspace to the first different position
        let mut actually_deleted = 0i32;
        for i in 0..chars_to_delete {
            if cancellation_token.is_cancelled() {
                debug!("Cancelled during backspace at {i}/{chars_to_delete}");
                break;
            }
            if enigo
                .key(enigo::Key::Backspace, enigo::Direction::Click)
                .is_ok()
            {
                actually_deleted += 1;
            } else {
                debug!("Failed to backspace at position {i}");
                break;
            }
        }

        // Type the new part
        let mut actually_typed_chars = 0i32;
        if !text_to_type.is_empty() && !cancellation_token.is_cancelled() {
            if enigo.text(&text_to_type).is_ok() {
                actually_typed_chars =
                    i32::try_from(text_to_type.chars().count()).unwrap_or_default();
                debug!(
                    "Typed new text: '{}'",
                    text_to_type.chars().take(20).collect::<String>()
                );
            } else {
                debug!("Failed to type new text portion");
            }
        }

        let net_change = actually_typed_chars - actually_deleted;
        debug!(
            "Net change: {net_change} (deleted: {actually_deleted}, typed: {actually_typed_chars})"
        );
        net_change
    }

    /// Update preview text using two-phase approach
    pub fn update_preview(
        enigo: &mut Enigo,
        new_text: &str,
        actually_typed: &mut String,
        typed_counter: &Arc<AtomicUsize>,
        state: &mut State,
        cancellation_token: &tokio_util::sync::CancellationToken,
    ) {
        let processed_text = Self::preprocess_text(new_text, true);

        // Skip if text hasn't changed
        if processed_text == state.prev_text {
            debug!("Text unchanged, skipping");
            return;
        }

        // Skip empty text
        if processed_text.is_empty() {
            debug!("Empty text, skipping");
            return;
        }

        // PHASE 1: Stabilization and session text update
        Self::update_with_stabilization(state, &processed_text);

        // PHASE 2: Decide what to show on screen
        let display_text = Self::build_display_text(state, &processed_text);

        debug!(
            "Display text: '{}' (session: {} chars, preview: {} chars)",
            display_text.chars().take(50).collect::<String>(),
            state.full_session_text.chars().count(),
            processed_text.chars().count()
        );

        // Apply the update to screen
        Self::apply_text_update(
            enigo,
            &display_text,
            actually_typed,
            typed_counter,
            cancellation_token,
        );
        state.prev_text = processed_text;
    }

    /// Stabilization and session text update (Phase 1)
    fn update_with_stabilization(state: &mut State, new_preview_text: &str) {
        // Add current text to storage
        state.text_storage.push(new_preview_text.to_string());

        // Keep only recent texts for stabilization (prevent unbounded growth)
        if state.text_storage.len() > 10 {
            state.text_storage.remove(0);
        }

        // Find common prefix between last two texts
        if state.text_storage.len() >= 2 {
            let last_two = &state.text_storage[state.text_storage.len() - 2..];
            let common_prefix = Self::find_common_prefix(&last_two[0], &last_two[1]);
            let prefix_text = last_two[0].chars().take(common_prefix).collect::<String>();

            // Only update stabilized text if we found a longer stable prefix
            if prefix_text.len() > state.stabilized_text.len() {
                state.stabilized_text = prefix_text;
                debug!(
                    "Updated stabilized text: '{}'",
                    state.stabilized_text.chars().take(30).collect::<String>()
                );
            }
        }

        // Update full session text using stabilized text + tail matching
        Self::update_full_session_text(state, new_preview_text);
    }

    /// Update the full session text using stabilized text as base
    #[allow(clippy::cast_sign_loss)]
    fn update_full_session_text(state: &mut State, new_preview_text: &str) {
        // If we have stabilized text, use it as our base
        if !state.stabilized_text.is_empty()
            && state.stabilized_text.len() > state.full_session_text.len()
        {
            state.full_session_text = state.stabilized_text.clone();
            state.last_growth_time = std::time::Instant::now();
            debug!(
                "Updated session from stabilized: '{}'",
                state.full_session_text.chars().take(30).collect::<String>()
            );
        }

        // Only grow the session text, never shrink it
        if state.full_session_text.is_empty() {
            state.full_session_text = new_preview_text.to_string();
            state.last_growth_time = std::time::Instant::now();
            debug!(
                "Started session text: '{}'",
                state.full_session_text.chars().take(30).collect::<String>()
            );
            return;
        }

        // Check if preview text extends our session text
        if new_preview_text.len() > state.full_session_text.len()
            && new_preview_text.starts_with(&state.full_session_text)
        {
            // Perfect extension - just grow
            state.full_session_text = new_preview_text.to_string();
            state.last_growth_time = std::time::Instant::now();
            debug!(
                "Extended session text to: '{}'",
                state.full_session_text.chars().take(40).collect::<String>()
            );
            return;
        }

        // Use tail matching to extend session with new content
        let matching_pos =
            Self::find_tail_match_in_text(&state.full_session_text, new_preview_text, 3);
        if matching_pos >= 0 {
            let extended = format!(
                "{}{}",
                state.full_session_text,
                &new_preview_text[matching_pos as usize..]
            );
            if extended.len() > state.full_session_text.len() {
                state.full_session_text = extended;
                state.last_growth_time = std::time::Instant::now();
                debug!(
                    "Extended session via tail match: '{}'",
                    state.full_session_text.chars().take(40).collect::<String>()
                );
            }
        }
    }

    /// Build the display text (Phase 2) - what actually shows on screen
    #[allow(clippy::cast_sign_loss)]
    fn build_display_text(state: &State, preview_text: &str) -> String {
        // Use stabilized text as base, but be smart about it

        // If no stabilized text yet, show the preview
        if state.stabilized_text.is_empty() {
            return preview_text.to_string();
        }

        // Try tail matching first
        let matching_pos = Self::find_tail_match_in_text(&state.stabilized_text, preview_text, 3);

        if matching_pos >= 0 {
            // Found overlap - combine stabilized text with new part from preview
            let combined = format!(
                "{}{}",
                state.stabilized_text,
                &preview_text[matching_pos as usize..]
            );
            return combined;
        }

        // No tail match found - be conservative to avoid text loss
        // Prefer the longer text (session text or preview) to avoid disappearing words
        let best_text = if state.full_session_text.len() >= preview_text.len() {
            &state.full_session_text
        } else {
            preview_text
        };

        best_text.to_string()
    }

    /// Find the position where the last 'n' characters of text1 match with a substring in text2.
    fn find_tail_match_in_text(text1: &str, text2: &str, length_of_match: usize) -> i32 {
        // Check if either text is too short
        if text1.chars().count() < length_of_match || text2.chars().count() < length_of_match {
            return -1;
        }

        let text1_chars: Vec<char> = text1.chars().collect();
        let text2_chars: Vec<char> = text2.chars().collect();

        // The end portion of text1 that we want to find in text2
        let target_substring: Vec<char> =
            text1_chars[text1_chars.len() - length_of_match..].to_vec();

        // Loop through text2 from right to left
        for i in 0..=(text2_chars.len() - length_of_match) {
            let start_pos = text2_chars.len() - i - length_of_match;
            let end_pos = text2_chars.len() - i;
            let current_substring: Vec<char> = text2_chars[start_pos..end_pos].to_vec();

            // Compare substrings
            if current_substring == target_substring {
                return i32::try_from(end_pos).unwrap_or_default();
            }
        }

        -1
    }

    /// Process final text (completed sentence) - Uses full session audio
    pub fn process_final_text(
        enigo: &mut Enigo,
        final_text: &str,
        actually_typed: &mut String,
        typed_counter: &Arc<AtomicUsize>,
        state: &mut State,
        cancellation_token: &tokio_util::sync::CancellationToken,
    ) {
        // Use the full session text if it's longer/better than the final text
        let text_to_use = if state.full_session_text.len() > final_text.len() {
            debug!(
                "Using full session text for final ({}  chars vs {} chars)",
                state.full_session_text.len(),
                final_text.len()
            );
            &state.full_session_text
        } else {
            debug!("Using provided final text ({} chars)", final_text.len());
            final_text
        };

        let processed_text = Self::preprocess_text(text_to_use, false); // is_preview = false (adds period)

        // Clear preview and type final text
        Self::apply_text_update(
            enigo,
            &processed_text,
            actually_typed,
            typed_counter,
            cancellation_token,
        );

        // Add space after final text
        if !cancellation_token.is_cancelled() {
            let _ = enigo.text(" ");
            actually_typed.push(' ');
            typed_counter.store(
                actually_typed.chars().count(),
                std::sync::atomic::Ordering::Relaxed,
            );
        }

        // Reset state for next sentence - but keep the full session text for user reference
        state.prev_text.clear();
        state.last_transcription = processed_text;
        state.last_growth_time = std::time::Instant::now();

        info!(
            "Completed sentence. Session text: '{}'",
            state.full_session_text.chars().take(50).collect::<String>()
        );

        // Clear session for next recording
        state.full_session_text.clear();
    }

    /// Apply text update to screen (common logic)
    fn apply_text_update(
        enigo: &mut Enigo,
        new_text: &str,
        actually_typed: &mut String,
        typed_counter: &Arc<AtomicUsize>,
        cancellation_token: &tokio_util::sync::CancellationToken,
    ) {
        let old_char_count = actually_typed.chars().count();
        let new_char_count = new_text.chars().count();
        let mut actual_chars_on_screen = old_char_count;

        if old_char_count == 0 {
            // Screen is empty, just type the new text
            if enigo.text(new_text).is_ok() {
                actual_chars_on_screen = new_char_count;
                info!(
                    "Initial typed: '{}' ({} chars)",
                    new_text.chars().take(40).collect::<String>(),
                    new_char_count
                );
            } else {
                warn!("Failed to type initial text, keeping counter at 0");
                actual_chars_on_screen = 0;
            }
        } else if new_text.starts_with(actually_typed.as_str())
            && new_text.len() > actually_typed.len()
        {
            // Perfect extension - just add the suffix (most common case)
            let suffix = &new_text[actually_typed.len()..];
            if enigo.text(suffix).is_ok() {
                actual_chars_on_screen = new_char_count;
                info!(
                    "Extended: '{}' (+{} chars, total: {})",
                    suffix.chars().take(20).collect::<String>(),
                    suffix.chars().count(),
                    new_char_count
                );
            } else {
                warn!("Failed to type suffix, keeping old counter");
                // Keep the old character count since typing failed
            }
        } else {
            // Need to replace - use differential update
            let net_change =
                Self::apply_simple_diff(enigo, actually_typed, new_text, cancellation_token);

            if cancellation_token.is_cancelled() {
                debug!("Update cancelled, keeping existing character count");
                return;
            }

            // Calculate actual characters on screen based on the net change
            let new_count = i32::try_from(old_char_count).unwrap_or_default() + net_change;
            actual_chars_on_screen = usize::try_from(new_count.max(0)).unwrap_or_default();

            info!(
                "Replaced: {}{} chars (screen total: {})",
                if net_change >= 0 { "+" } else { "" },
                net_change,
                actual_chars_on_screen
            );
        }

        // Update state to match what we think is actually on screen
        if actual_chars_on_screen == new_char_count {
            // Typing succeeded completely
            actually_typed.clear();
            actually_typed.push_str(new_text);
        } else {
            // Typing failed or was partial - we can't be sure what's on screen
            // Keep the old actually_typed but log the discrepancy
            warn!(
                "Character count mismatch: expected {new_char_count}, actual {actual_chars_on_screen}"
            );
        }

        typed_counter.store(actual_chars_on_screen, std::sync::atomic::Ordering::Relaxed);
    }

    /// Clear all typed text and reset state
    pub fn clear_preview(
        enigo: &mut Enigo,
        actually_typed: &mut String,
        typed_counter: &Arc<AtomicUsize>,
        state: &mut State,
        _cancellation_token: &tokio_util::sync::CancellationToken,
    ) {
        if actually_typed.is_empty() {
            return;
        }

        let chars_to_delete = actually_typed.chars().count();
        let batch_size = 20;
        let mut remaining = chars_to_delete;

        while remaining > 0 {
            let batch = remaining.min(batch_size);
            for _ in 0..batch {
                let _ = enigo.key(enigo::Key::Backspace, enigo::Direction::Click);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            remaining -= batch;

            if remaining > 0 {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }

        actually_typed.clear();
        typed_counter.store(0, std::sync::atomic::Ordering::Relaxed);

        // Also clear state when explicitly clearing preview
        state.prev_text.clear();
        state.last_transcription.clear();
        state.full_session_text.clear();
        state.last_growth_time = std::time::Instant::now();

        debug!("Cleared all {chars_to_delete} characters and reset state");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocess_text() {
        // Basic functionality
        assert_eq!(Typer::preprocess_text("hello world", true), "Hello world");
        assert_eq!(Typer::preprocess_text("hello world", false), "Hello world.");
        assert_eq!(Typer::preprocess_text("", true), "");

        assert_eq!(
            Typer::preprocess_text("...hello world", true),
            "Hello world"
        );
        assert_eq!(
            Typer::preprocess_text("  ...  hello world  ", true),
            "Hello world"
        );
        assert_eq!(
            Typer::preprocess_text("  multiple   spaces  ", true),
            "Multiple spaces"
        );
    }

    #[test]
    fn test_is_simple_extension() {
        assert!(Typer::is_simple_extension("hello", "hello world"));
        assert!(Typer::is_simple_extension("", "hello"));
        assert!(!Typer::is_simple_extension("hello", "hi world"));
        assert!(!Typer::is_simple_extension("hello", "hello"));
        assert!(!Typer::is_simple_extension("hello world", "hello"));
    }

    #[test]
    fn test_find_tail_match_in_text() {
        // Test the key case: "engi" should match with "engineer"
        assert_eq!(
            Typer::find_tail_match_in_text("hello engi", "engineer is good", 4),
            4
        );

        // Test basic tail matching
        assert_eq!(
            Typer::find_tail_match_in_text("hello world", "world is nice", 5),
            5
        );

        // Test no match
        assert_eq!(Typer::find_tail_match_in_text("hello", "goodbye", 3), -1);

        // Test short strings
        assert_eq!(Typer::find_tail_match_in_text("hi", "hello", 3), -1);

        // Test exact match at end
        assert_eq!(Typer::find_tail_match_in_text("abc", "xyzabc", 3), 6);
    }

    #[test]
    fn test_find_common_prefix() {
        assert_eq!(Typer::find_common_prefix("hello world", "hello there"), 6);
        assert_eq!(Typer::find_common_prefix("abc", "def"), 0);
        assert_eq!(Typer::find_common_prefix("same text", "same text"), 9);
    }
}
