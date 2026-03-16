//! Comma before subordinating conjunction detection.
//!
//! In Finnish, a comma is required before subordinating conjunctions such as
//! "kun" (when), "koska" (because), "vaikka" (although), "jos" (if),
//! "että" (that), "jotta" (so that), "kunnes" (until), and "ellei" (unless),
//! when they introduce a subordinate clause in the middle of a sentence.
//!
//! This rule checks that a comma appears before these conjunctions
//! when they are preceded by a word (i.e., not at the start of a sentence).

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Subordinating conjunctions that require a preceding comma in Finnish.
const SUBORDINATING_CONJUNCTIONS: &[&str] = &[
    "kun", "koska", "vaikka", "jos", "että", "jotta", "kunnes", "ellei", "ettei", "mikäli", "vaan",
];

/// Detects missing comma before subordinating conjunctions.
///
/// # Error code
///
/// `COMMA_BEFORE_CONJUNCTION`
///
/// # Example
///
/// ```
/// use mce_grammar::{AnnotatedToken, GrammarRule};
/// use mce_grammar::rules::CommaBeforeConjunctionRule;
///
/// let rule = CommaBeforeConjunctionRule::new();
/// let tokens = vec![
///     AnnotatedToken::word("Tulin", 0, 5, None),
///     AnnotatedToken::non_word(" ", 5, 6),
///     AnnotatedToken::word("kotiin", 6, 12, None),
///     AnnotatedToken::non_word(" ", 12, 13),
///     AnnotatedToken::word("kun", 13, 16, None),
///     AnnotatedToken::non_word(" ", 16, 17),
///     AnnotatedToken::word("satoi", 17, 22, None),
/// ];
/// let errors = rule.check(&tokens);
/// assert_eq!(errors.len(), 1);
/// assert_eq!(errors[0].code, "COMMA_BEFORE_CONJUNCTION");
/// ```
pub struct CommaBeforeConjunctionRule;

impl CommaBeforeConjunctionRule {
    /// Create a new comma-before-conjunction rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CommaBeforeConjunctionRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a conjunction is a subordinating conjunction (case-insensitive).
fn is_subordinating_conjunction(text: &str) -> bool {
    let lower = text.to_lowercase();
    SUBORDINATING_CONJUNCTIONS.contains(&lower.as_str())
}

impl GrammarRule for CommaBeforeConjunctionRule {
    fn id(&self) -> &'static str {
        "COMMA_BEFORE_CONJUNCTION"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        let mut prev_word: Option<&AnnotatedToken> = None;
        let mut comma_seen_since_last_word = false;
        let mut sentence_start = true;

        for token in tokens {
            if !token.is_word {
                if token.text.contains(',') {
                    comma_seen_since_last_word = true;
                }
                if token.text.ends_with('.')
                    || token.text.ends_with('!')
                    || token.text.ends_with('?')
                {
                    sentence_start = true;
                    prev_word = None;
                    comma_seen_since_last_word = false;
                }
                continue;
            }

            if sentence_start {
                // The conjunction at the start of a sentence does not need a comma.
                sentence_start = false;
                prev_word = Some(token);
                comma_seen_since_last_word = false;
                continue;
            }

            if let Some(prev) = prev_word {
                if is_subordinating_conjunction(&token.text) && !comma_seen_since_last_word {
                    errors.push(GrammarError::with_suggestions(
                        prev.end,
                        token.start,
                        "COMMA_BEFORE_CONJUNCTION",
                        format!(
                            "Missing comma before subordinating conjunction \"{}\"",
                            token.text
                        ),
                        vec![format!(", {}", token.text)],
                    ));
                }
            }

            prev_word = Some(token);
            comma_seen_since_last_word = false;
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
    fn detects_missing_comma_before_kun() {
        let rule = CommaBeforeConjunctionRule::new();
        // "Tulin kotiin kun satoi" — missing comma before "kun"
        let tokens = vec![
            word("Tulin", 0, 5),
            non_word(" ", 5, 6),
            word("kotiin", 6, 12),
            non_word(" ", 12, 13),
            word("kun", 13, 16),
            non_word(" ", 16, 17),
            word("satoi", 17, 22),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "COMMA_BEFORE_CONJUNCTION");
        assert!(errors[0].message.contains("kun"));
    }

    #[test]
    fn detects_missing_comma_before_koska() {
        let rule = CommaBeforeConjunctionRule::new();
        // "Jäin kotiin koska olin sairas"
        let tokens = vec![
            word("Jäin", 0, 5),
            non_word(" ", 5, 6),
            word("kotiin", 6, 12),
            non_word(" ", 12, 13),
            word("koska", 13, 18),
            non_word(" ", 18, 19),
            word("olin", 19, 23),
            non_word(" ", 23, 24),
            word("sairas", 24, 30),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("koska"));
    }

    #[test]
    fn detects_missing_comma_before_vaikka() {
        let rule = CommaBeforeConjunctionRule::new();
        // "Menin ulos vaikka satoi"
        let tokens = vec![
            word("Menin", 0, 5),
            non_word(" ", 5, 6),
            word("ulos", 6, 10),
            non_word(" ", 10, 11),
            word("vaikka", 11, 17),
            non_word(" ", 17, 18),
            word("satoi", 18, 23),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    // --- No false positives ---

    #[test]
    fn no_error_when_comma_present() {
        let rule = CommaBeforeConjunctionRule::new();
        // "Tulin kotiin, kun satoi"
        let tokens = vec![
            word("Tulin", 0, 5),
            non_word(" ", 5, 6),
            word("kotiin", 6, 12),
            non_word(", ", 12, 14),
            word("kun", 14, 17),
            non_word(" ", 17, 18),
            word("satoi", 18, 23),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_at_sentence_start() {
        let rule = CommaBeforeConjunctionRule::new();
        // "Kun satoi, jäin kotiin." — "kun" starts the sentence.
        let tokens = vec![
            word("Kun", 0, 3),
            non_word(" ", 3, 4),
            word("satoi", 4, 9),
            non_word(", ", 9, 11),
            word("jäin", 11, 16),
            non_word(" ", 16, 17),
            word("kotiin", 17, 23),
            non_word(".", 23, 24),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_non_conjunction_words() {
        let rule = CommaBeforeConjunctionRule::new();
        let tokens = vec![
            word("Koira", 0, 5),
            non_word(" ", 5, 6),
            word("juoksee", 6, 13),
            non_word(".", 13, 14),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = CommaBeforeConjunctionRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn conjunction_after_sentence_boundary() {
        let rule = CommaBeforeConjunctionRule::new();
        // "Satoi. Koska oli kylmä..." — "Koska" starts new sentence.
        let tokens = vec![
            word("Satoi", 0, 5),
            non_word(".", 5, 6),
            non_word(" ", 6, 7),
            word("Koska", 7, 12),
            non_word(" ", 12, 13),
            word("oli", 13, 16),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = CommaBeforeConjunctionRule::new();
        assert_eq!(rule.id(), "COMMA_BEFORE_CONJUNCTION");
    }

    #[test]
    fn default_trait() {
        let rule = CommaBeforeConjunctionRule::default();
        assert_eq!(rule.id(), "COMMA_BEFORE_CONJUNCTION");
    }
}
