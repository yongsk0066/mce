//! Double negation detection.
//!
//! In standard Finnish, double negation is non-standard and typically
//! an error. For example:
//! - "en tiedä mitään" (I don't know anything) — correct
//! - "en tiedä ei mitään" (I don't know not anything) — double negation
//!
//! This rule detects when two negation verb forms appear in close
//! proximity within the same clause (before a sentence boundary).

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// All Finnish negation verb forms (case-insensitive lookup).
const NEGATION_FORMS: &[&str] = &["en", "et", "ei", "emme", "ette", "eivät"];

/// Words that commonly follow negation and are NOT themselves negation verbs
/// but look similar. We exclude these to avoid false positives.
const FALSE_POSITIVE_WORDS: &[&str] = &[
    "ei-",     // compound prefix
    "eikä",    // "nor" — coordinating, not double negation
    "eikö",    // "isn't it" — question particle
    "ettei",   // "that not" — subordinating conjunction
    "ellei",   // "unless" — subordinating conjunction
    "eipä",    // emphatic
    "eihän",   // emphatic
    "eikään",  // emphatic
    "enkä",    // "and I not" — coordinating
    "etkä",    // "and you not"
    "emmekä",  // "and we not"
    "ettekä",  // "and you(pl) not"
    "eivätkä", // "and they not"
];

/// Detects double negation in Finnish text.
///
/// Scans for two negation verb forms within the same clause (delimited
/// by sentence-ending punctuation). Reports an error when a second
/// negation form is found.
///
/// # Error code
///
/// `DOUBLE_NEGATION`
///
/// # Limitations
///
/// - Does not track clause boundaries beyond sentence-ending punctuation
/// - Coordinating "eikä" (nor) is excluded to avoid false positives
pub struct DoubleNegationRule;

impl DoubleNegationRule {
    /// Create a new double negation rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DoubleNegationRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a word is a negation verb form (case-insensitive).
fn is_negation_form(text: &str) -> bool {
    let lower = text.to_lowercase();
    NEGATION_FORMS.contains(&lower.as_str())
}

/// Check if a word is a false positive (eikä, ettei, etc.).
fn is_false_positive(text: &str) -> bool {
    let lower = text.to_lowercase();
    FALSE_POSITIVE_WORDS.contains(&lower.as_str())
}

impl GrammarRule for DoubleNegationRule {
    fn id(&self) -> &'static str {
        "DOUBLE_NEGATION"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        // Track the first negation in the current clause.
        let mut first_negation: Option<&AnnotatedToken> = None;

        for token in tokens {
            if !token.is_word {
                if token.text.ends_with('.')
                    || token.text.ends_with('!')
                    || token.text.ends_with('?')
                {
                    first_negation = None;
                }
                continue;
            }

            if is_false_positive(&token.text) {
                continue;
            }

            if is_negation_form(&token.text) {
                if let Some(first) = first_negation {
                    errors.push(GrammarError::new(
                        first.start,
                        token.end,
                        "DOUBLE_NEGATION",
                        format!(
                            "Double negation: \"{}\" ... \"{}\" — \
                             standard Finnish uses single negation",
                            first.text, token.text
                        ),
                    ));
                    // Don't reset — keep tracking the first one for triple negation.
                } else {
                    first_negation = Some(token);
                }
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
    fn detects_double_ei() {
        let rule = DoubleNegationRule::new();
        // "ei tiedä ei mitään"
        let tokens = vec![
            word("ei", 0, 2),
            word("tiedä", 3, 9),
            word("ei", 10, 12),
            word("mitään", 13, 20),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "DOUBLE_NEGATION");
        assert_eq!(errors[0].start, 0);
        assert_eq!(errors[0].end, 12);
    }

    #[test]
    fn detects_en_ei_double() {
        let rule = DoubleNegationRule::new();
        // "en halua ei mitään"
        let tokens = vec![
            word("en", 0, 2),
            word("halua", 3, 8),
            word("ei", 9, 11),
            word("mitään", 12, 19),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn detects_emme_ei_double() {
        let rule = DoubleNegationRule::new();
        let tokens = vec![
            word("emme", 0, 4),
            word("tiedä", 5, 11),
            word("ei", 12, 14),
            word("mitään", 15, 22),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    // --- No false positives ---

    #[test]
    fn no_error_for_single_negation() {
        let rule = DoubleNegationRule::new();
        let tokens = vec![
            word("en", 0, 2),
            word("tiedä", 3, 9),
            word("mitään", 10, 17),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_across_sentence_boundary() {
        let rule = DoubleNegationRule::new();
        // "En tiedä. Ei ole."
        let tokens = vec![
            word("En", 0, 2),
            word("tiedä", 3, 9),
            non_word(".", 9, 10),
            word("Ei", 11, 13),
            word("ole", 14, 17),
            non_word(".", 17, 18),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_eika() {
        let rule = DoubleNegationRule::new();
        // "en tiedä eikä halua" — "eikä" is not double negation.
        let tokens = vec![
            word("en", 0, 2),
            word("tiedä", 3, 9),
            word("eikä", 10, 15),
            word("halua", 16, 21),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = DoubleNegationRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_non_negation_words() {
        let rule = DoubleNegationRule::new();
        let tokens = vec![word("koira", 0, 5), word("juoksee", 6, 13)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = DoubleNegationRule::new();
        assert_eq!(rule.id(), "DOUBLE_NEGATION");
    }

    #[test]
    fn default_trait() {
        let rule = DoubleNegationRule::default();
        assert_eq!(rule.id(), "DOUBLE_NEGATION");
    }
}
