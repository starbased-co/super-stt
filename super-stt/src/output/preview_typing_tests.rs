// SPDX-License-Identifier: GPL-3.0-only
use super::PreviewTyper;
use std::time::Instant;

// Basic functionality tests
#[test]
fn test_words_match_exact() {
    assert!(PreviewTyper::words_match("hello", "hello"));
    assert!(PreviewTyper::words_match("world", "world"));
    assert!(PreviewTyper::words_match("", ""));
}

#[test]
fn test_words_match_case_insensitive() {
    assert!(PreviewTyper::words_match("Hello", "hello"));
    assert!(PreviewTyper::words_match("WORLD", "world"));
    assert!(PreviewTyper::words_match("MiXeD", "mixed"));
}

#[test]
fn test_words_match_punctuation() {
    assert!(PreviewTyper::words_match("hello.", "hello"));
    assert!(PreviewTyper::words_match("hello", "hello!"));
    assert!(PreviewTyper::words_match("hello,", "hello?"));
    assert!(PreviewTyper::words_match("word;", "word:"));
}

// Improved fuzzy matching tests
#[test]
fn test_improved_fuzzy_matching() {
    // Test Levenshtein distance-based matching
    assert!(PreviewTyper::words_match("hello", "helo")); // Missing letter
    assert!(PreviewTyper::words_match("world", "wrold")); // Transposition
    assert!(PreviewTyper::words_match("testing", "testng")); // Deletion
    assert!(PreviewTyper::words_match("quick", "quik")); // Missing letter

    // Test phonetic matching
    assert!(PreviewTyper::words_match("phone", "fone")); // ph -> f
    // Note: "cat" vs "kat" may not match due to stricter phonetic rules

    // Test cases that should NOT match
    assert!(!PreviewTyper::words_match("hello", "goodbye"));
    assert!(!PreviewTyper::words_match("short", "verylongword"));
}

#[test]
fn test_common_speech_errors() {
    // Test common speech recognition error patterns
    let test_cases = vec![
        ("thank", "thanks"),           // Plural variations
        ("going", "goin"),             // Dropped 'g'
        ("because", "becaus"),         // Dropped letters
        ("recognize", "recognise"),    // British vs American spelling
        ("color", "colour"),           // British vs American spelling
        ("programming", "programing"), // Double letter
        ("receive", "recieve"),        // ie/ei confusion
        ("definitely", "definately"),  // Common misspelling
    ];

    for (word1, word2) in test_cases {
        let similarity = PreviewTyper::word_similarity(word1, word2);
        assert!(
            similarity > 0.5,
            "Words '{}' and '{}' should have similarity > 0.5, got {}",
            word1,
            word2,
            similarity
        );
    }
}

// Word matching array tests
#[test]
fn test_find_best_word_match_perfect() {
    let typed = vec!["hello", "world"];
    let new_text = vec!["hello", "world", "how", "are", "you"];

    let (start, len) = PreviewTyper::find_best_word_match(&typed, &new_text);
    assert_eq!(start, 0);
    assert_eq!(len, 2);
}

#[test]
fn test_find_best_word_match_with_improvements() {
    // Test with typos and variations
    let typed = vec!["hello", "wrold", "how"];
    let new_text = vec!["hi", "hello", "world", "how", "are", "you"];

    let (start, len) = PreviewTyper::find_best_word_match(&typed, &new_text);
    assert_eq!(start, 1); // Should start at "hello"
    assert_eq!(len, 3); // Should match all three words despite "wrold" vs "world"

    // Test with phonetic variations
    let typed = vec!["fone", "number"];
    let new_text = vec!["the", "phone", "number", "is"];

    let (start, len) = PreviewTyper::find_best_word_match(&typed, &new_text);
    assert_eq!(start, 1); // Should start at "phone"
    assert_eq!(len, 2); // Should match both words
}

// Preview replacement tests
#[test]
fn test_should_replace_preview_basic() {
    assert!(!PreviewTyper::should_replace_preview("", "anything"));
    assert!(!PreviewTyper::should_replace_preview("", ""));

    // Test identical text
    let text = "The same text exactly";
    assert!(!PreviewTyper::should_replace_preview(&text, &text));
}

// Utility function tests
#[test]
fn test_is_valid_perfect_extension() {
    // Valid extensions
    assert!(PreviewTyper::is_valid_perfect_extension(" world!"));
    assert!(PreviewTyper::is_valid_perfect_extension(" how are you?"));

    // Invalid extensions
    assert!(!PreviewTyper::is_valid_perfect_extension(""));
    assert!(!PreviewTyper::is_valid_perfect_extension(" ab")); // Too short
}

#[test]
fn test_calculate_word_limit() {
    // Short typed text allows more words
    assert_eq!(PreviewTyper::calculate_word_limit(10), 10);
    assert_eq!(PreviewTyper::calculate_word_limit(19), 10);

    // Longer typed text has moderate limit
    assert_eq!(PreviewTyper::calculate_word_limit(20), 7);
    assert_eq!(PreviewTyper::calculate_word_limit(100), 7);
}

// Performance tests
#[test]
fn test_performance() {
    let test_cases = vec![
        ("hello", "hello"),
        ("programming", "programing"),
        ("definitely", "definately"),
        ("phone", "fone"),
        ("hello", "goodbye"),
    ];

    let iterations = 1000u128;
    let start = Instant::now();

    for _ in 0..iterations {
        for (word1, word2) in &test_cases {
            let _ = PreviewTyper::words_match(word1, word2);
        }
    }

    let duration = start.elapsed();
    let total_calls = iterations * test_cases.len() as u128;
    let avg_per_call = duration.as_nanos() / total_calls;

    // Should be fast - under 10 microseconds per call
    assert!(
        avg_per_call < 10000,
        "Improved matching is too slow: {} ns per call",
        avg_per_call
    );

    println!(
        "Improved matching performance: {} ns per call",
        avg_per_call
    );
}

// Edge case tests
#[test]
fn test_edge_cases() {
    // Empty strings
    assert!(!PreviewTyper::words_match("hello", ""));
    assert!(!PreviewTyper::words_match("", "hello"));
    assert!(PreviewTyper::words_match("", ""));

    // Very similar but not identical
    assert!(PreviewTyper::words_match("test", "tests"));
    assert!(PreviewTyper::words_match("testing", "test"));

    // Numbers and special characters
    assert!(PreviewTyper::words_match("123", "123"));
    assert!(PreviewTyper::words_match("hello!", "hello"));
    assert!(PreviewTyper::words_match("don't", "dont"));

    // Dynamic thresholds
    assert!(!PreviewTyper::words_match("hi", "by")); // Should not match short dissimilar words
    assert!(PreviewTyper::words_match("programming", "programing")); // More forgiving for long words
}
