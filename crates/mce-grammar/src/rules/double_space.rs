//! Double/triple space detection.
//!
//! Flags sequences of two or more consecutive spaces between words.
//! While a single space is normal word separation, multiple spaces are
//! almost always typos or formatting errors.

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Detects multiple consecutive spaces between words.
///
/// Reports an error spanning the whitespace token that contains two or
/// more spaces, with a suggestion to replace it with a single space.
///
/// # Error code
///
/// `DOUBLE_SPACE`
///
/// # Example
///
/// ```
/// use mce_grammar::{AnnotatedToken, GrammarRule};
/// use mce_grammar::rules::DoubleSpaceRule;
///
/// let rule = DoubleSpaceRule::new();
/// let tokens = vec![
///     AnnotatedToken::word("Koira", 0, 5, None),
///     AnnotatedToken::non_word("  ", 5, 7),
///     AnnotatedToken::word("juoksee", 7, 14, None),
/// ];
/// let errors = rule.check(&tokens);
/// assert_eq!(errors.len(), 1);
/// assert_eq!(errors[0].code, "DOUBLE_SPACE");
/// ```
pub struct DoubleSpaceRule;

impl DoubleSpaceRule {
    /// Create a new double space rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DoubleSpaceRule {
    fn default() -> Self {
        Self::new()
    }
}

impl GrammarRule for DoubleSpaceRule {
    fn id(&self) -> &'static str {
        "DOUBLE_SPACE"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        for token in tokens {
            if token.is_word {
                continue;
            }

            let space_count = token.text.chars().filter(|&c| c == ' ').count();
            let all_spaces = token.text.chars().all(|c| c == ' ');

            if all_spaces && space_count >= 2 {
                errors.push(GrammarError::with_suggestions(
                    token.start,
                    token.end,
                    "DOUBLE_SPACE",
                    format!(
                        "Multiple spaces ({}) between words — use a single space",
                        space_count
                    ),
                    vec![" ".to_string()],
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
    fn detects_double_space() {
        let rule = DoubleSpaceRule::new();
        let tokens = vec![
            word("Koira", 0, 5),
            non_word("  ", 5, 7),
            word("juoksee", 7, 14),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "DOUBLE_SPACE");
        assert_eq!(errors[0].start, 5);
        assert_eq!(errors[0].end, 7);
        assert_eq!(errors[0].suggestions, vec![" "]);
    }

    #[test]
    fn detects_triple_space() {
        let rule = DoubleSpaceRule::new();
        let tokens = vec![
            word("Koira", 0, 5),
            non_word("   ", 5, 8),
            word("juoksee", 8, 15),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("3"));
    }

    #[test]
    fn detects_multiple_double_spaces() {
        let rule = DoubleSpaceRule::new();
        let tokens = vec![
            word("Koira", 0, 5),
            non_word("  ", 5, 7),
            word("juoksee", 7, 14),
            non_word("  ", 14, 16),
            word("nopeasti", 16, 24),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 2);
    }

    // --- No false positives ---

    #[test]
    fn no_error_for_single_space() {
        let rule = DoubleSpaceRule::new();
        let tokens = vec![
            word("Koira", 0, 5),
            non_word(" ", 5, 6),
            word("juoksee", 6, 13),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_punctuation() {
        let rule = DoubleSpaceRule::new();
        let tokens = vec![word("Koira", 0, 5), non_word(".", 5, 6)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = DoubleSpaceRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = DoubleSpaceRule::new();
        assert_eq!(rule.id(), "DOUBLE_SPACE");
    }

    #[test]
    fn default_trait() {
        let rule = DoubleSpaceRule::default();
        assert_eq!(rule.id(), "DOUBLE_SPACE");
    }
}
