//! Subject-verb person agreement checking via pronouns.
//!
//! In Finnish, the personal pronouns must agree with the verb in person:
//! - "minä puhun" (I speak) — 1st person singular
//! - "hän puhuu" (he/she speaks) — 3rd person singular
//! - "he puhuvat" (they speak) — 3rd person plural
//!
//! This rule supplements the existing AgreementRule (which checks number
//! agreement via morphological analysis) by checking person agreement
//! using the PERSON attribute when a personal pronoun is adjacent to a verb.
//!
//! # Limitations
//!
//! - Only checks adjacent pronoun + verb pairs
//! - Requires morphological analysis with CLASS and PERSON attributes

use mce_core::analysis::{ATTR_CLASS, ATTR_PERSON};

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Finnish personal pronouns and their expected person values.
///
/// Each entry: (pronoun_lowercase, expected_person).
const PRONOUN_PERSON_MAP: &[(&str, &str)] = &[
    ("minä", "1"),
    ("mä", "1"),
    ("sinä", "2"),
    ("sä", "2"),
    ("hän", "3"),
    ("se", "3"),
    ("me", "1"),
    ("te", "2"),
    ("he", "3"),
    ("ne", "3"),
];

/// Detects subject-verb person agreement errors.
///
/// Checks that when a personal pronoun is followed by a verb (or vice versa),
/// the PERSON attribute of the verb matches the pronoun's expected person.
///
/// # Error code
///
/// `SUBJECT_VERB_PERSON`
///
/// # Requires
///
/// Morphological analysis with CLASS and PERSON attributes on verb tokens.
pub struct SubjectVerbAgreementRule;

impl SubjectVerbAgreementRule {
    /// Create a new subject-verb agreement rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for SubjectVerbAgreementRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the expected person for a pronoun (case-insensitive).
fn pronoun_person(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    PRONOUN_PERSON_MAP
        .iter()
        .find(|(p, _)| *p == lower.as_str())
        .map(|(_, person)| *person)
}

/// Check if a token is a verb and get its person.
fn verb_person(token: &AnnotatedToken) -> Option<&str> {
    let analysis = token.analysis.as_ref()?;
    let class = analysis.get(ATTR_CLASS)?;
    if class != "teonsana" {
        return None;
    }
    analysis.get(ATTR_PERSON)
}

impl GrammarRule for SubjectVerbAgreementRule {
    fn id(&self) -> &'static str {
        "SUBJECT_VERB_PERSON"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        let word_tokens: Vec<&AnnotatedToken> = tokens.iter().filter(|t| t.is_word).collect();

        for window in word_tokens.windows(2) {
            let first = window[0];
            let second = window[1];

            // Pattern 1: pronoun + verb
            if let Some(expected_person) = pronoun_person(&first.text) {
                if let Some(actual_person) = verb_person(second) {
                    if expected_person != actual_person {
                        errors.push(GrammarError::new(
                            first.start,
                            second.end,
                            "SUBJECT_VERB_PERSON",
                            format!(
                                "Person mismatch: \"{}\" (person {}) + \"{}\" (person {})",
                                first.text, expected_person, second.text, actual_person
                            ),
                        ));
                    }
                }
            }

            // Pattern 2: verb + pronoun (inverted order)
            if let Some(actual_person) = verb_person(first) {
                if let Some(expected_person) = pronoun_person(&second.text) {
                    if expected_person != actual_person {
                        errors.push(GrammarError::new(
                            first.start,
                            second.end,
                            "SUBJECT_VERB_PERSON",
                            format!(
                                "Person mismatch: \"{}\" (person {}) + \"{}\" (person {})",
                                first.text, actual_person, second.text, expected_person
                            ),
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
    use mce_core::analysis::Analysis;

    fn word(text: &str, start: usize, end: usize) -> AnnotatedToken {
        AnnotatedToken::word(text, start, end, None)
    }

    fn verb_with_person(text: &str, start: usize, end: usize, person: &str) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "teonsana");
        a.set(ATTR_PERSON, person);
        AnnotatedToken::word(text, start, end, Some(a))
    }

    // --- Positive detections ---

    #[test]
    fn detects_mina_with_third_person_verb() {
        let rule = SubjectVerbAgreementRule::new();
        // "minä puhuu" — 1st person pronoun + 3rd person verb
        let tokens = vec![word("minä", 0, 5), verb_with_person("puhuu", 6, 11, "3")];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SUBJECT_VERB_PERSON");
        assert!(errors[0].message.contains("person 1"));
        assert!(errors[0].message.contains("person 3"));
    }

    #[test]
    fn detects_han_with_first_person_verb() {
        let rule = SubjectVerbAgreementRule::new();
        // "hän puhun" — 3rd person pronoun + 1st person verb
        let tokens = vec![word("hän", 0, 4), verb_with_person("puhun", 5, 10, "1")];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn detects_inverted_order() {
        let rule = SubjectVerbAgreementRule::new();
        // "puhuu minä" — verb(3rd) + pronoun(1st)
        let tokens = vec![verb_with_person("puhuu", 0, 5, "3"), word("minä", 6, 11)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    // --- No false positives ---

    #[test]
    fn no_error_for_correct_first_person() {
        let rule = SubjectVerbAgreementRule::new();
        // "minä puhun" — correct
        let tokens = vec![word("minä", 0, 5), verb_with_person("puhun", 6, 11, "1")];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_correct_third_person() {
        let rule = SubjectVerbAgreementRule::new();
        // "hän puhuu" — correct
        let tokens = vec![word("hän", 0, 4), verb_with_person("puhuu", 5, 10, "3")];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_without_person_attribute() {
        let rule = SubjectVerbAgreementRule::new();
        // Verb has CLASS but no PERSON — skip.
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "teonsana");
        let verb = AnnotatedToken::word("puhuu", 5, 10, Some(a));
        let tokens = vec![word("minä", 0, 5), verb];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_non_pronoun() {
        let rule = SubjectVerbAgreementRule::new();
        // "koira puhuu" — koira is not a pronoun
        let tokens = vec![word("koira", 0, 5), verb_with_person("puhuu", 6, 11, "3")];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = SubjectVerbAgreementRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn case_insensitive_pronoun() {
        let rule = SubjectVerbAgreementRule::new();
        // "Minä puhun" — capitalized, correct
        let tokens = vec![word("Minä", 0, 5), verb_with_person("puhun", 6, 11, "1")];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = SubjectVerbAgreementRule::new();
        assert_eq!(rule.id(), "SUBJECT_VERB_PERSON");
    }

    #[test]
    fn default_trait() {
        let rule = SubjectVerbAgreementRule::default();
        assert_eq!(rule.id(), "SUBJECT_VERB_PERSON");
    }
}
