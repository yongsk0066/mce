//! Excessive exclamation/question mark detection.
//!
//! Detects multiple consecutive exclamation marks (!!!) or question marks
//! (???) and mixed sequences (!?!, ?!?) which are informal and inappropriate
//! in formal Finnish writing.
//!
//! Single `!` or `?` is fine. Double `!?` or `?!` for rhetorical effect is
//! borderline but common. Three or more consecutive marks are flagged.

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Minimum number of consecutive exclamation/question marks to trigger an error.
const MIN_EXCESSIVE_COUNT: usize = 3;

/// Detects excessive exclamation or question marks.
///
/// Reports an error when three or more consecutive `!` and/or `?`
/// characters appear in a non-word token.
///
/// # Error code
///
/// `EXCESSIVE_EXCLAMATION`
///
/// # Example
///
/// ```
/// use mce_grammar::{AnnotatedToken, GrammarRule};
/// use mce_grammar::rules::ExcessiveExclamationRule;
///
/// let rule = ExcessiveExclamationRule::new();
/// let tokens = vec![
///     AnnotatedToken::word("Hei", 0, 3, None),
///     AnnotatedToken::non_word("!!!", 3, 6),
/// ];
/// let errors = rule.check(&tokens);
/// assert_eq!(errors.len(), 1);
/// assert_eq!(errors[0].code, "EXCESSIVE_EXCLAMATION");
/// ```
pub struct ExcessiveExclamationRule;

impl ExcessiveExclamationRule {
    /// Create a new excessive exclamation rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExcessiveExclamationRule {
    fn default() -> Self {
        Self::new()
    }
}

impl GrammarRule for ExcessiveExclamationRule {
    fn id(&self) -> &'static str {
        "EXCESSIVE_EXCLAMATION"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        for token in tokens {
            if token.is_word {
                continue;
            }

            let mark_count = token.text.chars().filter(|&c| c == '!' || c == '?').count();

            // Only flag if ALL characters in the token are ! or ? marks
            // (to avoid false positives on tokens like "!)" or "?\"").
            let all_marks =
                !token.text.is_empty() && token.text.chars().all(|c| c == '!' || c == '?');

            if all_marks && mark_count >= MIN_EXCESSIVE_COUNT {
                let has_question = token.text.contains('?');
                let has_exclamation = token.text.contains('!');
                let suggestion = if has_question && has_exclamation {
                    "!".to_string() // mixed — suggest single exclamation
                } else if has_question {
                    "?".to_string()
                } else {
                    "!".to_string()
                };

                errors.push(GrammarError::with_suggestions(
                    token.start,
                    token.end,
                    "EXCESSIVE_EXCLAMATION",
                    format!(
                        "Excessive punctuation: \"{}\" — use a single mark in formal writing",
                        token.text
                    ),
                    vec![suggestion],
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
    fn detects_triple_exclamation() {
        let rule = ExcessiveExclamationRule::new();
        let tokens = vec![word("Hei", 0, 3), non_word("!!!", 3, 6)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "EXCESSIVE_EXCLAMATION");
        assert_eq!(errors[0].suggestions, vec!["!"]);
    }

    #[test]
    fn detects_triple_question() {
        let rule = ExcessiveExclamationRule::new();
        let tokens = vec![word("Miksi", 0, 5), non_word("???", 5, 8)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].suggestions, vec!["?"]);
    }

    #[test]
    fn detects_mixed_marks() {
        let rule = ExcessiveExclamationRule::new();
        let tokens = vec![word("Mitä", 0, 5), non_word("?!?", 5, 8)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].suggestions, vec!["!"]);
    }

    #[test]
    fn detects_many_exclamations() {
        let rule = ExcessiveExclamationRule::new();
        let tokens = vec![word("Ei", 0, 2), non_word("!!!!!", 2, 7)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    // --- No false positives ---

    #[test]
    fn no_error_for_single_exclamation() {
        let rule = ExcessiveExclamationRule::new();
        let tokens = vec![word("Hei", 0, 3), non_word("!", 3, 4)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_single_question() {
        let rule = ExcessiveExclamationRule::new();
        let tokens = vec![word("Miksi", 0, 5), non_word("?", 5, 6)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_double_marks() {
        let rule = ExcessiveExclamationRule::new();
        // "!?" — two marks, below threshold.
        let tokens = vec![word("Mitä", 0, 5), non_word("!?", 5, 7)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = ExcessiveExclamationRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_word_tokens() {
        let rule = ExcessiveExclamationRule::new();
        let tokens = vec![word("Koira", 0, 5)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_period() {
        let rule = ExcessiveExclamationRule::new();
        let tokens = vec![word("Koira", 0, 5), non_word(".", 5, 6)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = ExcessiveExclamationRule::new();
        assert_eq!(rule.id(), "EXCESSIVE_EXCLAMATION");
    }

    #[test]
    fn default_trait() {
        let rule = ExcessiveExclamationRule::default();
        assert_eq!(rule.id(), "EXCESSIVE_EXCLAMATION");
    }
}
