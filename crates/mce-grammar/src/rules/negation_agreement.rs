//! Negation verb agreement checking.
//!
//! In Finnish, the negation verb "ei" conjugates to agree with the subject
//! in person and number:
//!
//! | Person   | Singular | Plural  |
//! |----------|----------|---------|
//! | 1st      | en       | emme    |
//! | 2nd      | et       | ette    |
//! | 3rd      | ei       | eivät   |
//!
//! This rule detects mismatches between the negation form and an adjacent
//! subject pronoun.

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Mapping of Finnish personal pronouns to their expected negation form.
///
/// Each entry: (pronoun, expected_negation, person_label).
const PRONOUN_NEGATION_MAP: &[(&str, &str, &str)] = &[
    ("minä", "en", "1st person singular"),
    ("mä", "en", "1st person singular"),
    ("sinä", "et", "2nd person singular"),
    ("sä", "et", "2nd person singular"),
    ("hän", "ei", "3rd person singular"),
    ("se", "ei", "3rd person singular"),
    ("me", "emme", "1st person plural"),
    ("te", "ette", "2nd person plural"),
    ("he", "eivät", "3rd person plural"),
    ("ne", "eivät", "3rd person plural"),
];

/// All Finnish negation forms.
const NEGATION_FORMS: &[&str] = &["en", "et", "ei", "emme", "ette", "eivät"];

/// Detects negation verb agreement errors.
///
/// Checks that when a personal pronoun is followed by a negation verb
/// (or vice versa), the negation form matches the pronoun's person
/// and number.
///
/// # Error code
///
/// `NEGATION_AGREEMENT`
///
/// # Example
///
/// ```
/// use mce_grammar::{AnnotatedToken, GrammarRule};
/// use mce_grammar::rules::NegationAgreementRule;
///
/// let rule = NegationAgreementRule::new();
/// let tokens = vec![
///     AnnotatedToken::word("Minä", 0, 5, None),
///     AnnotatedToken::non_word(" ", 5, 6),
///     AnnotatedToken::word("ei", 6, 8, None),
///     AnnotatedToken::non_word(" ", 8, 9),
///     AnnotatedToken::word("tiedä", 9, 15, None),
/// ];
/// let errors = rule.check(&tokens);
/// assert_eq!(errors.len(), 1);
/// assert_eq!(errors[0].code, "NEGATION_AGREEMENT");
/// ```
pub struct NegationAgreementRule;

impl NegationAgreementRule {
    /// Create a new negation agreement rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for NegationAgreementRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a word is a negation verb form.
fn is_negation(text: &str) -> bool {
    let lower = text.to_lowercase();
    NEGATION_FORMS.contains(&lower.as_str())
}

/// Look up the expected negation form for a pronoun (case-insensitive).
fn expected_negation_for_pronoun(pronoun: &str) -> Option<(&'static str, &'static str)> {
    let lower = pronoun.to_lowercase();
    PRONOUN_NEGATION_MAP
        .iter()
        .find(|(p, _, _)| *p == lower.as_str())
        .map(|(_, neg, label)| (*neg, *label))
}

impl GrammarRule for NegationAgreementRule {
    fn id(&self) -> &'static str {
        "NEGATION_AGREEMENT"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        let word_tokens: Vec<&AnnotatedToken> = tokens.iter().filter(|t| t.is_word).collect();

        for window in word_tokens.windows(2) {
            let first = window[0];
            let second = window[1];

            // Pattern 1: pronoun + negation
            if let Some((expected_neg, person_label)) = expected_negation_for_pronoun(&first.text) {
                if is_negation(&second.text) {
                    let actual_neg = second.text.to_lowercase();
                    if actual_neg != expected_neg {
                        errors.push(GrammarError::with_suggestions(
                            first.start,
                            second.end,
                            "NEGATION_AGREEMENT",
                            format!(
                                "Negation verb mismatch: \"{}\" ({}) requires \"{}\" \
                                 but found \"{}\"",
                                first.text, person_label, expected_neg, second.text
                            ),
                            vec![format!("{} {}", first.text, expected_neg)],
                        ));
                    }
                }
            }

            // Pattern 2: negation + pronoun (inverted: "ei minä" → should be "en minä")
            if is_negation(&first.text) {
                if let Some((expected_neg, person_label)) =
                    expected_negation_for_pronoun(&second.text)
                {
                    let actual_neg = first.text.to_lowercase();
                    if actual_neg != expected_neg {
                        errors.push(GrammarError::with_suggestions(
                            first.start,
                            second.end,
                            "NEGATION_AGREEMENT",
                            format!(
                                "Negation verb mismatch: \"{}\" ({}) requires \"{}\" \
                                 but found \"{}\"",
                                second.text, person_label, expected_neg, first.text
                            ),
                            vec![format!("{} {}", expected_neg, second.text)],
                        ));
                    }
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

    // --- Positive detections ---

    #[test]
    fn detects_mina_ei_mismatch() {
        let rule = NegationAgreementRule::new();
        // "minä ei" — should be "minä en"
        let tokens = vec![word("minä", 0, 5), word("ei", 6, 8)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "NEGATION_AGREEMENT");
        assert!(errors[0].message.contains("en"));
        assert_eq!(errors[0].suggestions, vec!["minä en"]);
    }

    #[test]
    fn detects_me_ei_mismatch() {
        let rule = NegationAgreementRule::new();
        // "me ei" — should be "me emme"
        let tokens = vec![word("me", 0, 2), word("ei", 3, 5)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("emme"));
    }

    #[test]
    fn detects_he_ei_mismatch() {
        let rule = NegationAgreementRule::new();
        // "he ei" — should be "he eivät"
        let tokens = vec![word("he", 0, 2), word("ei", 3, 5)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("eivät"));
    }

    // --- No false positives ---

    #[test]
    fn no_error_for_correct_mina_en() {
        let rule = NegationAgreementRule::new();
        let tokens = vec![word("minä", 0, 5), word("en", 6, 8)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_correct_he_eivat() {
        let rule = NegationAgreementRule::new();
        let tokens = vec![word("he", 0, 2), word("eivät", 3, 9)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_correct_han_ei() {
        let rule = NegationAgreementRule::new();
        let tokens = vec![word("hän", 0, 4), word("ei", 5, 7)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_non_pronoun() {
        let rule = NegationAgreementRule::new();
        // "koira ei" — koira is not a pronoun, no check.
        let tokens = vec![word("koira", 0, 5), word("ei", 6, 8)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = NegationAgreementRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn case_insensitive_pronoun() {
        let rule = NegationAgreementRule::new();
        // "Minä en" — capitalized pronoun, correct.
        let tokens = vec![word("Minä", 0, 5), word("en", 6, 8)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = NegationAgreementRule::new();
        assert_eq!(rule.id(), "NEGATION_AGREEMENT");
    }

    #[test]
    fn default_trait() {
        let rule = NegationAgreementRule::default();
        assert_eq!(rule.id(), "NEGATION_AGREEMENT");
    }
}
