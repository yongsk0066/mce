//! Partitive object in negated sentences.
//!
//! In Finnish, negated sentences require the object to be in the partitive
//! case. For example:
//! - "En osta autoa" (I don't buy a car) — correct (partitive)
//! - "En osta auton" (I don't buy a car-genitive) — incorrect
//!
//! This rule checks that when a negation verb form is followed by a verb
//! and then a noun, the noun is in partitive case.
//!
//! # Limitations
//!
//! - Only checks the pattern: negation + verb + noun (3-word window)
//! - Requires morphological analysis with CLASS and SIJAMUOTO
//! - Does not handle more complex sentence structures

use mce_core::analysis::{ATTR_CLASS, ATTR_SIJAMUOTO};

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Finnish negation verb forms.
const NEGATION_FORMS: &[&str] = &["en", "et", "ei", "emme", "ette", "eivät"];

/// Noun classes that can serve as objects.
const OBJECT_CLASSES: &[&str] = &[
    "nimisana",   // common noun
    "etunimi",    // first name
    "sukunimi",   // surname
    "paikannimi", // place name
    "nimi",       // name
];

/// Cases that indicate accusative/genitive object (incorrect after negation).
/// In negated sentences, these should be partitive instead.
const NON_PARTITIVE_OBJECT_CASES: &[&str] = &[
    "npitkasija",  // nominative (nimento)
    "ompitkasija", // genitive (omanto)
];

/// Detects non-partitive objects in negated sentences.
///
/// # Error code
///
/// `PARTITIVE_OBJECT`
///
/// # Requires
///
/// Morphological analysis with CLASS and SIJAMUOTO attributes.
pub struct PartitiveObjectRule;

impl PartitiveObjectRule {
    /// Create a new partitive object rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for PartitiveObjectRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a word is a negation form (case-insensitive).
fn is_negation(text: &str) -> bool {
    let lower = text.to_lowercase();
    NEGATION_FORMS.contains(&lower.as_str())
}

/// Check if a token is a verb (teonsana).
fn is_verb(token: &AnnotatedToken) -> bool {
    token
        .analysis
        .as_ref()
        .and_then(|a| a.get(ATTR_CLASS))
        .is_some_and(|c| c == "teonsana")
}

/// Check if a token is a noun-like object.
fn is_object_noun(token: &AnnotatedToken) -> bool {
    token
        .analysis
        .as_ref()
        .and_then(|a| a.get(ATTR_CLASS))
        .is_some_and(|c| OBJECT_CLASSES.contains(&c))
}

/// Get the case (sijamuoto) of a token.
fn get_case(token: &AnnotatedToken) -> Option<&str> {
    token.analysis.as_ref()?.get(ATTR_SIJAMUOTO)
}

impl GrammarRule for PartitiveObjectRule {
    fn id(&self) -> &'static str {
        "PARTITIVE_OBJECT"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        let word_tokens: Vec<&AnnotatedToken> = tokens.iter().filter(|t| t.is_word).collect();

        // Look for pattern: negation + verb + noun
        for window in word_tokens.windows(3) {
            let neg = window[0];
            let verb = window[1];
            let noun = window[2];

            if !is_negation(&neg.text) {
                continue;
            }

            if !is_verb(verb) {
                continue;
            }

            if !is_object_noun(noun) {
                continue;
            }

            // Check that the noun is NOT in partitive.
            if let Some(case) = get_case(noun) {
                if NON_PARTITIVE_OBJECT_CASES.contains(&case) {
                    errors.push(GrammarError::new(
                        noun.start,
                        noun.end,
                        "PARTITIVE_OBJECT",
                        format!(
                            "Object \"{}\" in negated sentence should be in partitive case",
                            noun.text
                        ),
                    ));
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

    fn word_verb(text: &str, start: usize, end: usize) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "teonsana");
        AnnotatedToken::word(text, start, end, Some(a))
    }

    fn word_noun(text: &str, start: usize, end: usize, case: &str) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "nimisana");
        a.set(ATTR_SIJAMUOTO, case);
        AnnotatedToken::word(text, start, end, Some(a))
    }

    // --- Positive detections ---

    #[test]
    fn detects_genitive_after_negation() {
        let rule = PartitiveObjectRule::new();
        // "ei osta auton" — genitive object in negated sentence
        let tokens = vec![
            word("ei", 0, 2),
            word_verb("osta", 3, 7),
            word_noun("auton", 8, 13, "ompitkasija"),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "PARTITIVE_OBJECT");
        assert!(errors[0].message.contains("auton"));
    }

    #[test]
    fn detects_nominative_after_negation() {
        let rule = PartitiveObjectRule::new();
        // "en osta auto" — nominative object in negated sentence
        let tokens = vec![
            word("en", 0, 2),
            word_verb("osta", 3, 7),
            word_noun("auto", 8, 12, "npitkasija"),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn detects_with_different_negation_forms() {
        let rule = PartitiveObjectRule::new();
        let tokens = vec![
            word("emme", 0, 4),
            word_verb("halua", 5, 10),
            word_noun("koiran", 11, 17, "ompitkasija"),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    // --- No false positives ---

    #[test]
    fn no_error_for_partitive_object() {
        let rule = PartitiveObjectRule::new();
        // "ei osta autoa" — partitive, correct
        let tokens = vec![
            word("ei", 0, 2),
            word_verb("osta", 3, 7),
            word_noun("autoa", 8, 13, "ospitkasija"),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_without_negation() {
        let rule = PartitiveObjectRule::new();
        // "osta auton" — not negated
        let tokens = vec![
            word_verb("osta", 0, 4),
            word_noun("auton", 5, 10, "ompitkasija"),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_without_analysis() {
        let rule = PartitiveObjectRule::new();
        let tokens = vec![word("ei", 0, 2), word("osta", 3, 7), word("autoa", 8, 13)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = PartitiveObjectRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_single_word() {
        let rule = PartitiveObjectRule::new();
        let tokens = vec![word("ei", 0, 2)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = PartitiveObjectRule::new();
        assert_eq!(rule.id(), "PARTITIVE_OBJECT");
    }

    #[test]
    fn default_trait() {
        let rule = PartitiveObjectRule::default();
        assert_eq!(rule.id(), "PARTITIVE_OBJECT");
    }
}
