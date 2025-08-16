// SPDX-License-Identifier: GPL-3.0-only
use enigo::{Enigo, Keyboard};
use log::{debug, info};
use std::sync::{Arc, atomic::AtomicUsize};

/// Preview typing functionality for live transcription with intelligent fuzzy matching
pub struct PreviewTyper;

impl PreviewTyper {
    /// Calculate Levenshtein distance between two strings for fuzzy matching
    fn levenshtein_distance(s1: &str, s2: &str) -> usize {
        let len1 = s1.chars().count();
        let len2 = s2.chars().count();

        if len1 == 0 {
            return len2;
        }
        if len2 == 0 {
            return len1;
        }

        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        // Initialize first column
        for (i, row) in matrix.iter_mut().enumerate().take(len1 + 1) {
            row[0] = i;
        }
        // Initialize first row
        if let Some(first_row) = matrix.get_mut(0) {
            for (j, cell) in first_row.iter_mut().enumerate().take(len2 + 1) {
                *cell = j;
            }
        }

        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();

        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = usize::from(s1_chars[i - 1] != s2_chars[j - 1]);
                matrix[i][j] = (matrix[i - 1][j] + 1) // deletion
                    .min(matrix[i][j - 1] + 1) // insertion
                    .min(matrix[i - 1][j - 1] + cost); // substitution
            }
        }

        matrix[len1][len2]
    }

    /// Calculate similarity score between two words (0.0 to 1.0)
    #[allow(clippy::cast_precision_loss)]
    fn word_similarity(word1: &str, word2: &str) -> f32 {
        if word1.is_empty() || word2.is_empty() {
            return 0.0;
        }

        let w1_lower = word1.to_lowercase();
        let w2_lower = word2.to_lowercase();

        // Exact match
        if w1_lower == w2_lower {
            return 1.0;
        }

        let max_len = word1.len().max(word2.len()) as f32;
        let distance = Self::levenshtein_distance(&w1_lower, &w2_lower) as f32;

        // Calculate similarity as 1 - (distance / max_length)
        let base_similarity = 1.0 - (distance / max_len);

        // Boost score for prefix matches (common in speech-to-text)
        let prefix_bonus = if w1_lower.starts_with(&w2_lower) || w2_lower.starts_with(&w1_lower) {
            0.2
        } else {
            0.0
        };

        // Boost score for suffix matches
        let suffix_bonus = if w1_lower.ends_with(&w2_lower) || w2_lower.ends_with(&w1_lower) {
            0.1
        } else {
            0.0
        };

        // Apply phonetic bonus for similar sounding words
        let phonetic_bonus = Self::phonetic_similarity(&w1_lower, &w2_lower);

        (base_similarity + prefix_bonus + suffix_bonus + phonetic_bonus).min(1.0)
    }

    /// Simple phonetic similarity for common speech-to-text confusions
    fn phonetic_similarity(word1: &str, word2: &str) -> f32 {
        if word1.len() < 2 || word2.len() < 2 {
            return 0.0; // Skip very short words
        }

        // Check for common phonetic patterns directly
        let mut score: f32 = 0.0;

        // Check specific substitution patterns
        if (word1.contains("ph") && word2.contains('f'))
            || (word1.contains('f') && word2.contains("ph"))
        {
            score += 0.15;
        }
        if (word1.contains('k') && word2.contains('c'))
            || (word1.contains('c') && word2.contains('k'))
        {
            score += 0.1;
        }
        if (word1.contains('s') && word2.contains('z'))
            || (word1.contains('z') && word2.contains('s'))
        {
            score += 0.1;
        }
        if (word1.ends_with("tion") && word2.ends_with("shun"))
            || (word1.ends_with("shun") && word2.ends_with("tion"))
        {
            score += 0.2;
        }

        score.min(0.3) // Cap phonetic bonus
    }

    /// Check if two words match, handling punctuation and improved fuzzy matching
    #[must_use]
    pub fn words_match(typed_word: &str, new_word: &str) -> bool {
        let typed_clean = typed_word.trim_end_matches(['.', ',', '!', '?', ';', ':']);
        let new_clean = new_word.trim_end_matches(['.', ',', '!', '?', ';', ':']);

        if typed_clean.is_empty() || new_clean.is_empty() {
            return typed_clean.is_empty() && new_clean.is_empty();
        }

        // Calculate similarity score
        let similarity = Self::word_similarity(typed_clean, new_clean);

        // Dynamic threshold based on word length
        let threshold = if typed_clean.len().min(new_clean.len()) <= 3 {
            0.8 // Higher threshold for short words
        } else if typed_clean.len().min(new_clean.len()) <= 6 {
            0.6 // Medium threshold for medium words
        } else {
            0.5 // Lower threshold for long words
        };

        similarity >= threshold
    }

    /// Find the best word match position in new text for typed words with improved scoring
    #[must_use]
    pub fn find_best_word_match(typed_words: &[&str], new_words: &[&str]) -> (usize, usize) {
        if typed_words.is_empty() || new_words.is_empty() {
            return (0, 0);
        }

        let mut best_match_start = 0;
        let mut best_match_len = 0;
        let mut best_score = 0.0;

        for start in 0..new_words.len() {
            let mut match_len = 0;
            let mut total_score = 0.0;
            let mut consecutive_matches = 0;

            for (i, typed_word) in typed_words.iter().enumerate() {
                if start + i < new_words.len() {
                    let new_word = new_words[start + i];
                    let similarity = Self::word_similarity(typed_word, new_word);

                    // Consider it a match if similarity is above minimum threshold
                    if similarity >= 0.4 {
                        match_len += 1;
                        total_score += similarity;
                        consecutive_matches += 1;

                        // Bonus for consecutive matches
                        if consecutive_matches > 1 {
                            total_score += 0.1;
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            // Calculate weighted score considering both length and quality of matches
            #[allow(clippy::cast_precision_loss)]
            let weighted_score = if match_len > 0 {
                (total_score / match_len as f32) * match_len as f32 * 0.8 + match_len as f32 * 0.2
            } else {
                0.0
            };

            // Prefer longer matches, but also consider match quality
            if match_len > best_match_len
                || (match_len == best_match_len && weighted_score > best_score)
            {
                best_match_len = match_len;
                best_match_start = start;
                best_score = weighted_score;
            }
        }

        (best_match_start, best_match_len)
    }

    /// Check if we should replace the entire preview text
    #[must_use]
    pub fn should_replace_preview(actually_typed: &str, new_text: &str) -> bool {
        if actually_typed.is_empty() {
            return false; // Never replace if nothing typed yet
        }

        // Only replace in very extreme cases:
        // 1. The new text is completely unrelated to what we typed
        // 2. AND we've typed a lot (at least 50 characters)
        // 3. AND the new text doesn't contain any significant portion of what we typed

        let typed_clean = actually_typed
            .to_lowercase()
            .replace([',', '.', '!', '?'], "");
        let new_clean = new_text.to_lowercase().replace([',', '.', '!', '?'], "");

        // Check if any substantial portion (15+ chars) of what we typed appears in new text
        let has_substantial_overlap = if typed_clean.len() >= 15 {
            // Check overlapping windows of 15 characters
            (0..=typed_clean.len().saturating_sub(15)).any(|i| {
                let window = &typed_clean[i..i + 15];
                new_clean.contains(window)
            })
        } else {
            // For short text, check if most of it appears in new text
            new_clean.contains(&typed_clean)
        };

        actually_typed.len() >= 50 && !has_substantial_overlap && new_text.len() > 20
    }

    /// Check if a perfect extension is valid
    #[must_use]
    pub fn is_valid_perfect_extension(suffix: &str) -> bool {
        let suffix_trimmed = suffix.trim();

        // Allow extensions that:
        // 1. Are at least 3 characters
        // 2. Don't start with "..." (incomplete)
        // 3. Either end with punctuation/space OR contain complete words
        let has_complete_words = suffix_trimmed.contains(' ')
            || suffix_trimmed.ends_with('.')
            || suffix_trimmed.ends_with(',')
            || suffix_trimmed.ends_with('!')
            || suffix_trimmed.ends_with('?');

        let is_substantial = suffix_trimmed.len() >= 3 && !suffix_trimmed.starts_with("...");

        is_substantial && (has_complete_words || suffix.len() > 10)
    }

    /// Calculate adaptive word limit based on typed text length
    #[must_use]
    pub fn calculate_word_limit(typed_length: usize) -> usize {
        if typed_length < 20 {
            10 // Allow more words for very short typed text
        } else {
            7 // Moderate limit for longer text
        }
    }

    /// Handle perfect extension case where new text exactly extends typed text
    pub fn handle_perfect_extension(
        enigo: &mut Enigo,
        new_text: &str,
        actually_typed: &mut String,
        typed_counter: &Arc<AtomicUsize>,
    ) {
        let suffix = &new_text[actually_typed.len()..];
        let suffix_trimmed = suffix.trim();

        debug!(
            "Perfect extension check: suffix='{}', trimmed='{}', len={}",
            suffix,
            suffix_trimmed,
            suffix_trimmed.len()
        );

        if Self::is_valid_perfect_extension(suffix) {
            let _ = enigo.text(suffix);
            *actually_typed = new_text.to_string();
            typed_counter.store(
                actually_typed.chars().count(),
                std::sync::atomic::Ordering::Relaxed,
            );

            let sample: String = suffix.chars().take(40).collect();
            info!("Preview extended with: '{sample}'");
        } else {
            debug!("Skipping invalid extension: '{suffix_trimmed}'",);
        }
    }

    /// Handle smart append case where we find word matches in new text
    pub fn handle_smart_append(
        enigo: &mut Enigo,
        new_text: &str,
        actually_typed: &mut String,
        typed_counter: &Arc<AtomicUsize>,
    ) {
        info!(
            "Trying smart append: typed_len={}, new_len={}",
            actually_typed.len(),
            new_text.len()
        );

        let typed_text_clone = actually_typed.clone();
        let typed_words: Vec<&str> = typed_text_clone.split_whitespace().collect();
        let new_words: Vec<&str> = new_text.split_whitespace().collect();
        debug!("Words: typed={typed_words:?}, new={new_words:?}");

        let (best_match_start, best_match_len) =
            Self::find_best_word_match(&typed_words, &new_words);

        // More permissive matching - allow fuzzy matches for short text
        let min_required_match = if typed_words.len() <= 2 {
            1 // For 1-2 words, require at least 1 match
        } else {
            (typed_words.len() * 2 / 3).max(1) // Match at least 2/3 of words
        };

        info!(
            "Smart append analysis: best_match={}/{} at pos {}, min_required={}",
            best_match_len,
            typed_words.len(),
            best_match_start,
            min_required_match
        );

        if best_match_len >= min_required_match
            && best_match_start + best_match_len < new_words.len()
        {
            let suffix_words = &new_words[best_match_start + best_match_len..];
            debug!(
                "Potential suffix words: {:?} (len={})",
                suffix_words,
                suffix_words.len()
            );

            let max_words = Self::calculate_word_limit(actually_typed.len());

            if !suffix_words.is_empty() && suffix_words.len() <= max_words {
                // For long suffixes, take only the first chunk to be conservative
                let words_to_append = if suffix_words.len() > 5 {
                    &suffix_words[0..5] // Take first 5 words for long suffixes
                } else {
                    suffix_words
                };

                let suffix_text = format!(" {}", words_to_append.join(" "));

                let _ = enigo.text(&suffix_text);
                actually_typed.push_str(&suffix_text);

                typed_counter.store(
                    actually_typed.chars().count(),
                    std::sync::atomic::Ordering::Relaxed,
                );

                info!(
                    "Preview smart-appended: '{}' (matched {}/{} words, took {}/{} words)",
                    suffix_text.trim(),
                    best_match_len,
                    typed_words.len(),
                    words_to_append.len(),
                    suffix_words.len()
                );
            } else {
                info!(
                    "Suffix too long for smart append: {} words (limit={})",
                    suffix_words.len(),
                    max_words
                );
            }
        } else {
            info!(
                "Insufficient word match for smart append: {}/{} (need {})",
                best_match_len,
                typed_words.len(),
                min_required_match
            );
        }
    }
}

#[cfg(test)]
#[path = "preview_typing_tests.rs"]
mod tests;
