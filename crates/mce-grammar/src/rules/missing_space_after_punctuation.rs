//! Missing space after punctuation detection.
//!
//! Detects cases where a comma, period, semicolon, colon, or exclamation/
//! question mark is immediately followed by a word without an intervening
//! space. For example:
//! - "Koira juoksee.Kissa nukkuu." (missing space after period)
//! - "Hei,miten menee?" (missing space after comma)
//!
//! This rule operates on the raw token stream, looking for non-word tokens
//! that end with punctuation immediately followed by a word token with no
//! whitespace gap.

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Punctuation characters after which a space is normally required.
const SPACE_REQUIRED_AFTER: &[char] = &['.', ',', ';', ':', '!', '?'];

/// Detects missing space after punctuation.
///
/// Reports an error when a punctuation token (ending with `.`, `,`, `;`,
/// `:`, `!`, or `?`) is immediately followed by a word token with no
/// whitespace between them (i.e., the word's byte start equals the
/// punctuation's byte end).
///
/// # Error code
///
/// `MISSING_SPACE_AFTER_PUNCT`
///
/// # Example
///
/// ```
/// use mce_grammar::{AnnotatedToken, GrammarRule};
/// use mce_grammar::rules::MissingSpaceAfterPunctuationRule;
///
/// let rule = MissingSpaceAfterPunctuationRule::new();
/// let tokens = vec![
///     AnnotatedToken::word("Koira", 0, 5, None),
///     AnnotatedToken::non_word(".", 5, 6),
///     AnnotatedToken::word("Kissa", 6, 11, None),
/// ];
/// let errors = rule.check(&tokens);
/// assert_eq!(errors.len(), 1);
/// assert_eq!(errors[0].code, "MISSING_SPACE_AFTER_PUNCT");
/// ```
pub struct MissingSpaceAfterPunctuationRule;

impl MissingSpaceAfterPunctuationRule {
    /// Create a new missing-space-after-punctuation rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for MissingSpaceAfterPunctuationRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a non-word token ends with punctuation that requires a following space.
fn ends_with_space_requiring_punct(text: &str) -> bool {
    text.chars()
        .last()
        .is_some_and(|c| SPACE_REQUIRED_AFTER.contains(&c))
}

impl GrammarRule for MissingSpaceAfterPunctuationRule {
    fn id(&self) -> &'static str {
        "MISSING_SPACE_AFTER_PUNCT"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        for window in tokens.windows(2) {
            let prev = &window[0];
            let curr = &window[1];

            // We need: prev is a non-word ending with punctuation,
            // curr is a word, and there is no gap between them.
            if prev.is_word || !curr.is_word {
                continue;
            }

            if !ends_with_space_requiring_punct(&prev.text) {
                continue;
            }

            // Check adjacency: the word starts exactly where the punctuation ends.
            if curr.start == prev.end {
                let punct_char = prev.text.chars().last().unwrap();
                errors.push(GrammarError::with_suggestions(
                    prev.start,
                    curr.end,
                    "MISSING_SPACE_AFTER_PUNCT",
                    format!(
                        "Missing space after '{}' before \"{}\"",
                        punct_char, curr.text
                    ),
                    vec![format!("{} {}", prev.text, curr.text)],
                ));
            }
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, start: usize, end: usize) -> AnnotatedToken {
        AnnotatedToken::word(text, start, end, None)
    }

    fn non_word(text: &str, start: usize, end: usize) -> AnnotatedToken {
        AnnotatedToken::non_word(text, start, end)
    }

    // --- Positive detections ---

    #[test]
    fn detects_missing_space_after_period() {
        let rule = MissingSpaceAfterPunctuationRule::new();
        // "Koira.Kissa" — no space after period
        let tokens = vec![
            word("Koira", 0, 5),
            non_word(".", 5, 6),
            word("Kissa", 6, 11),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "MISSING_SPACE_AFTER_PUNCT");
        assert!(errors[0].message.contains("'.'"));
    }

    #[test]
    fn detects_missing_space_after_comma() {
        let rule = MissingSpaceAfterPunctuationRule::new();
        // "Hei,miten"
        let tokens = vec![word("Hei", 0, 3), non_word(",", 3, 4), word("miten", 4, 9)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("','"));
    }

    #[test]
    fn detects_missing_space_after_semicolon() {
        let rule = MissingSpaceAfterPunctuationRule::new();
        let tokens = vec![
            word("yksi", 0, 4),
            non_word(";", 4, 5),
            word("kaksi", 5, 10),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn detects_missing_space_after_exclamation() {
        let rule = MissingSpaceAfterPunctuationRule::new();
        let tokens = vec![word("Hei", 0, 3), non_word("!", 3, 4), word("Tule", 4, 8)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    // --- No false positives ---

    #[test]
    fn no_error_when_space_present() {
        let rule = MissingSpaceAfterPunctuationRule::new();
        // "Koira. Kissa" — space after period
        let tokens = vec![
            word("Koira", 0, 5),
            non_word(". ", 5, 7),
            word("Kissa", 7, 12),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_with_separate_space_token() {
        let rule = MissingSpaceAfterPunctuationRule::new();
        let tokens = vec![
            word("Koira", 0, 5),
            non_word(".", 5, 6),
            non_word(" ", 6, 7),
            word("Kissa", 7, 12),
        ];
        let errors = rule.check(&tokens);
        // The period is followed by a space token (non-word), not a word, so no error.
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = MissingSpaceAfterPunctuationRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_single_punctuation() {
        let rule = MissingSpaceAfterPunctuationRule::new();
        let tokens = vec![non_word(".", 0, 1)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_abbreviation_like_pattern() {
        // When two punctuation tokens follow each other (e.g., "..."), no error.
        let rule = MissingSpaceAfterPunctuationRule::new();
        let tokens = vec![
            non_word(".", 0, 1),
            non_word(".", 1, 2),
            non_word(".", 2, 3),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn suggestion_includes_space() {
        let rule = MissingSpaceAfterPunctuationRule::new();
        let tokens = vec![word("Hei", 0, 3), non_word(",", 3, 4), word("miten", 4, 9)];
        let errors = rule.check(&tokens);
        assert_eq!(errors[0].suggestions, vec![", miten"]);
    }

    #[test]
    fn rule_id() {
        let rule = MissingSpaceAfterPunctuationRule::new();
        assert_eq!(rule.id(), "MISSING_SPACE_AFTER_PUNCT");
    }

    #[test]
    fn default_trait() {
        let rule = MissingSpaceAfterPunctuationRule::default();
        assert_eq!(rule.id(), "MISSING_SPACE_AFTER_PUNCT");
    }
}
