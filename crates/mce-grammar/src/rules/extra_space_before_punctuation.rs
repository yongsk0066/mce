//! Extra space before punctuation detection.
//!
//! In Finnish (and most Western typographic conventions), there should be
//! no space before a comma, period, semicolon, colon, exclamation mark,
//! or question mark. For example:
//! - "Koira juoksee ." — incorrect (space before period)
//! - "Hei , miten menee?" — incorrect (space before comma)
//!
//! This rule detects whitespace immediately preceding punctuation tokens.

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Punctuation characters that should not be preceded by a space.
const NO_SPACE_BEFORE: &[char] = &['.', ',', ';', ':', '!', '?'];

/// Detects extra space before punctuation.
///
/// Reports an error when a word token is followed by whitespace and then
/// punctuation, where the whitespace is unnecessary.
///
/// # Error code
///
/// `EXTRA_SPACE_BEFORE_PUNCT`
///
/// # Example
///
/// ```
/// use mce_grammar::{AnnotatedToken, GrammarRule};
/// use mce_grammar::rules::ExtraSpaceBeforePunctuationRule;
///
/// let rule = ExtraSpaceBeforePunctuationRule::new();
/// let tokens = vec![
///     AnnotatedToken::word("Koira", 0, 5, None),
///     AnnotatedToken::non_word(" ", 5, 6),
///     AnnotatedToken::non_word(".", 6, 7),
/// ];
/// let errors = rule.check(&tokens);
/// assert_eq!(errors.len(), 1);
/// assert_eq!(errors[0].code, "EXTRA_SPACE_BEFORE_PUNCT");
/// ```
pub struct ExtraSpaceBeforePunctuationRule;

impl ExtraSpaceBeforePunctuationRule {
    /// Create a new extra-space-before-punctuation rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExtraSpaceBeforePunctuationRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a token is a whitespace-only non-word token.
fn is_whitespace_token(token: &AnnotatedToken) -> bool {
    !token.is_word && token.text.chars().all(|c| c.is_whitespace())
}

/// Check if a non-word token starts with punctuation that should not
/// be preceded by a space.
fn starts_with_no_space_punct(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|c| NO_SPACE_BEFORE.contains(&c))
}

impl GrammarRule for ExtraSpaceBeforePunctuationRule {
    fn id(&self) -> &'static str {
        "EXTRA_SPACE_BEFORE_PUNCT"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        // Look for pattern: [word] [whitespace] [punctuation]
        // We scan windows of 3, but also handle window of 2:
        // sometimes the token stream is [word] [" ."] (space+punct merged).

        // Pattern 1: three-token sequence — word, whitespace, punctuation.
        if tokens.len() >= 3 {
            for window in tokens.windows(3) {
                let before = &window[0];
                let space = &window[1];
                let punct = &window[2];

                if !before.is_word {
                    continue;
                }

                if !is_whitespace_token(space) {
                    continue;
                }

                if punct.is_word || !starts_with_no_space_punct(&punct.text) {
                    continue;
                }

                let punct_char = punct.text.chars().next().unwrap();
                errors.push(GrammarError::with_suggestions(
                    space.start,
                    punct.end,
                    "EXTRA_SPACE_BEFORE_PUNCT",
                    format!("Unexpected space before '{}'", punct_char),
                    vec![punct.text.clone()],
                ));
            }
        }

        // Pattern 2: two-token sequence — word, then non-word starting with
        // whitespace followed by punctuation (e.g., " .").
        for window in tokens.windows(2) {
            let before = &window[0];
            let combined = &window[1];

            if !before.is_word || combined.is_word {
                continue;
            }

            let trimmed = combined.text.trim_start();
            if trimmed.is_empty() {
                continue;
            }

            let leading_spaces = combined.text.len() - trimmed.len();
            if leading_spaces == 0 {
                continue;
            }

            if starts_with_no_space_punct(trimmed) {
                let punct_char = trimmed.chars().next().unwrap();
                errors.push(GrammarError::with_suggestions(
                    combined.start,
                    combined.end,
                    "EXTRA_SPACE_BEFORE_PUNCT",
                    format!("Unexpected space before '{}'", punct_char),
                    vec![trimmed.to_string()],
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
    fn detects_space_before_period() {
        let rule = ExtraSpaceBeforePunctuationRule::new();
        // "Koira juoksee ."
        let tokens = vec![
            word("juoksee", 0, 7),
            non_word(" ", 7, 8),
            non_word(".", 8, 9),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "EXTRA_SPACE_BEFORE_PUNCT");
        assert!(errors[0].message.contains("'.'"));
    }

    #[test]
    fn detects_space_before_comma() {
        let rule = ExtraSpaceBeforePunctuationRule::new();
        let tokens = vec![word("Hei", 0, 3), non_word(" ", 3, 4), non_word(",", 4, 5)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("','"));
    }

    #[test]
    fn detects_space_before_question_mark() {
        let rule = ExtraSpaceBeforePunctuationRule::new();
        let tokens = vec![
            word("Miksi", 0, 5),
            non_word(" ", 5, 6),
            non_word("?", 6, 7),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn detects_combined_space_punct_token() {
        let rule = ExtraSpaceBeforePunctuationRule::new();
        // Tokenizer might produce " ." as a single non-word token.
        let tokens = vec![word("Koira", 0, 5), non_word(" .", 5, 7)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].suggestions, vec!["."]);
    }

    // --- No false positives ---

    #[test]
    fn no_error_when_no_space() {
        let rule = ExtraSpaceBeforePunctuationRule::new();
        // "Koira juoksee." — no space before period
        let tokens = vec![word("juoksee", 0, 7), non_word(".", 7, 8)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = ExtraSpaceBeforePunctuationRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_space_between_words() {
        let rule = ExtraSpaceBeforePunctuationRule::new();
        let tokens = vec![
            word("Koira", 0, 5),
            non_word(" ", 5, 6),
            word("juoksee", 6, 13),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_single_word() {
        let rule = ExtraSpaceBeforePunctuationRule::new();
        let tokens = vec![word("Koira", 0, 5)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = ExtraSpaceBeforePunctuationRule::new();
        assert_eq!(rule.id(), "EXTRA_SPACE_BEFORE_PUNCT");
    }

    #[test]
    fn default_trait() {
        let rule = ExtraSpaceBeforePunctuationRule::default();
        assert_eq!(rule.id(), "EXTRA_SPACE_BEFORE_PUNCT");
    }
}
