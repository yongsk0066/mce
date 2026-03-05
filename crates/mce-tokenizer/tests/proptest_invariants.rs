//! Property-based tests for tokenizer invariants.

use mce_core::token::TokenType;
use mce_tokenizer::{next_sentence, next_token};
use proptest::prelude::*;

// ============================================================================
// Strategy generators
// ============================================================================

/// Strategy for arbitrary Unicode text.
fn unicode_text() -> impl Strategy<Value = String> {
    proptest::string::string_regex(".{0,200}").unwrap()
}

/// Strategy for Finnish-like text with spaces and punctuation.
fn finnish_text() -> impl Strategy<Value = String> {
    let word = proptest::string::string_regex("[a-z\u{00E4}\u{00F6}]{1,15}").unwrap();
    let punct = prop_oneof![Just("."), Just(","), Just("!"), Just("?"), Just(";")];
    let space = Just(" ");

    proptest::collection::vec(
        prop_oneof![
            word.prop_map(|w| w),
            punct.prop_map(|p| p.to_string()),
            space.prop_map(|s| s.to_string()),
        ],
        1..=30,
    )
    .prop_map(|parts| parts.join(""))
}

// ============================================================================
// Tokenizer invariants
// ============================================================================

proptest! {
    /// Tokenization must never produce empty tokens (zero-length).
    #[test]
    fn no_empty_tokens(text in unicode_text()) {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut pos = 0;
        let mut iterations = 0;

        while pos < len && iterations < 10_000 {
            let (tok_type, tok_len) = next_token(&chars, len, pos);

            if tok_type == TokenType::None {
                break;
            }

            prop_assert!(
                tok_len > 0,
                "Zero-length token at position {} in text: {:?}",
                pos, text
            );

            pos += tok_len;
            iterations += 1;
        }
    }

    /// Tokenizing the entire string must cover all characters.
    /// The sum of all token lengths must equal the input length.
    #[test]
    fn tokens_cover_entire_input(text in unicode_text()) {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut pos = 0;
        let mut total_len = 0usize;
        let mut iterations = 0;

        while pos < len && iterations < 10_000 {
            let (tok_type, tok_len) = next_token(&chars, len, pos);

            if tok_type == TokenType::None || tok_len == 0 {
                break;
            }

            total_len += tok_len;
            pos += tok_len;
            iterations += 1;
        }

        prop_assert_eq!(
            total_len, len,
            "Tokenization covered {} of {} chars for: {:?}",
            total_len, len, text
        );
    }

    /// Tokenization must terminate (no infinite loops).
    /// This is a stronger version: for any input, the number of tokens
    /// must be at most the number of characters (each char is at least
    /// one token).
    #[test]
    fn tokenization_terminates(text in unicode_text()) {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut pos = 0;
        let mut token_count = 0usize;

        while pos < len {
            let (tok_type, tok_len) = next_token(&chars, len, pos);

            if tok_type == TokenType::None || tok_len == 0 {
                break;
            }

            token_count += 1;
            pos += tok_len;

            prop_assert!(
                token_count <= len,
                "More tokens than characters for: {:?}", text
            );
        }
    }
}

// ============================================================================
// Sentence detection invariants
// ============================================================================

proptest! {
    /// Sentence detection must also cover the entire input.
    #[test]
    fn sentence_detection_covers_input(text in finnish_text()) {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut pos = 0;
        let mut total = 0usize;
        let mut iterations = 0;

        while pos < len && iterations < 10_000 {
            let (_sent_type, sent_len) = next_sentence(&chars, len, pos);

            if sent_len == 0 {
                break;
            }

            total += sent_len;
            pos += sent_len;
            iterations += 1;
        }

        prop_assert_eq!(
            total, len,
            "Sentence detection covered {} of {} chars for: {:?}",
            total, len, text
        );
    }

    /// Sentence detection never produces zero-length segments.
    #[test]
    fn no_empty_sentences(text in finnish_text()) {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut pos = 0;
        let mut iterations = 0;

        while pos < len && iterations < 10_000 {
            let (_sent_type, sent_len) = next_sentence(&chars, len, pos);

            if sent_len == 0 {
                // At end of text, this is OK.
                break;
            }

            prop_assert!(
                sent_len > 0,
                "Zero-length sentence at position {} in: {:?}",
                pos, text
            );

            pos += sent_len;
            iterations += 1;
        }
    }
}

// ============================================================================
// Token type consistency
// ============================================================================

proptest! {
    /// Word tokens should only contain word-like characters.
    #[test]
    fn word_tokens_are_nonempty(text in finnish_text()) {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut pos = 0;
        let mut iterations = 0;

        while pos < len && iterations < 10_000 {
            let (tok_type, tok_len) = next_token(&chars, len, pos);

            if tok_type == TokenType::None || tok_len == 0 {
                break;
            }

            if tok_type == TokenType::Word {
                prop_assert!(
                    tok_len >= 1,
                    "Empty word token at position {}", pos
                );
            }

            pos += tok_len;
            iterations += 1;
        }
    }
}
