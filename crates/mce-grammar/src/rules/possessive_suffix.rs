//! Possessive suffix + pronoun redundancy detection.
//!
//! In Finnish, a possessive suffix on a noun makes the corresponding
//! possessive pronoun redundant. While using both is not strictly
//! grammatically incorrect, it is considered redundant/informal style
//! and is flagged by style guides. For example:
//! - "minun kirjani" — "my book-my" (redundant but common in speech)
//! - "kirjani" — "my book" (sufficient and preferred in formal writing)
//!
//! This rule detects the pattern: possessive pronoun + noun with
//! possessive suffix, and suggests removing the pronoun.
//!
//! # Limitations
//!
//! - Only checks adjacent pronoun + noun pairs
//! - Requires POSSESSIVE attribute from morphological analysis
//! - This is a style recommendation, not a hard error

use mce_core::analysis::{ATTR_CLASS, ATTR_POSSESSIVE};

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Finnish possessive pronouns (genitive forms) and their possessive type.
///
/// Each entry: (pronoun_lowercase, possessive_type).
const POSSESSIVE_PRONOUNS: &[(&str, &str)] = &[
    ("minun", "1s"),  // my
    ("sinun", "2s"),  // your (singular)
    ("hänen", "3"),   // his/her
    ("meidän", "1p"), // our
    ("teidän", "2p"), // your (plural)
    ("heidän", "3p"), // their
];

/// Noun classes that can have possessive suffixes.
const NOUN_CLASSES: &[&str] = &[
    "nimisana",   // common noun
    "etunimi",    // first name
    "sukunimi",   // surname
    "paikannimi", // place name
    "nimi",       // name
    "laatusana",  // adjective (used as noun)
];

/// Detects redundant possessive pronoun + possessive suffix combinations.
///
/// # Error code
///
/// `POSSESSIVE_REDUNDANCY`
///
/// # Requires
///
/// Morphological analysis with CLASS and POSSESSIVE attributes.
pub struct PossessiveSuffixRule;

impl PossessiveSuffixRule {
    /// Create a new possessive suffix rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for PossessiveSuffixRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a word is a possessive pronoun (case-insensitive).
fn is_possessive_pronoun(text: &str) -> bool {
    let lower = text.to_lowercase();
    POSSESSIVE_PRONOUNS
        .iter()
        .any(|(p, _)| *p == lower.as_str())
}

/// Check if a token has a possessive suffix.
fn has_possessive_suffix(token: &AnnotatedToken) -> bool {
    token
        .analysis
        .as_ref()
        .and_then(|a| a.get(ATTR_POSSESSIVE))
        .is_some()
}

/// Check if a token is a noun-like word.
fn is_noun_like(token: &AnnotatedToken) -> bool {
    token
        .analysis
        .as_ref()
        .and_then(|a| a.get(ATTR_CLASS))
        .is_some_and(|c| NOUN_CLASSES.contains(&c))
}

impl GrammarRule for PossessiveSuffixRule {
    fn id(&self) -> &'static str {
        "POSSESSIVE_REDUNDANCY"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        let word_tokens: Vec<&AnnotatedToken> = tokens.iter().filter(|t| t.is_word).collect();

        for window in word_tokens.windows(2) {
            let pronoun = window[0];
            let noun = window[1];

            if !is_possessive_pronoun(&pronoun.text) {
                continue;
            }

            if !is_noun_like(noun) {
                continue;
            }

            if !has_possessive_suffix(noun) {
                continue;
            }

            // The possessive pronoun is redundant when the noun already
            // has a possessive suffix.
            errors.push(GrammarError::with_suggestions(
                pronoun.start,
                noun.end,
                "POSSESSIVE_REDUNDANCY",
                format!(
                    "Possessive pronoun \"{}\" is redundant when \"{}\" already has a possessive suffix",
                    pronoun.text, noun.text
                ),
                vec![noun.text.clone()], // suggestion: remove the pronoun
            ));
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

    fn noun_with_possessive(text: &str, start: usize, end: usize) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "nimisana");
        a.set(ATTR_POSSESSIVE, "1s");
        AnnotatedToken::word(text, start, end, Some(a))
    }

    fn noun_without_possessive(text: &str, start: usize, end: usize) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "nimisana");
        AnnotatedToken::word(text, start, end, Some(a))
    }

    // --- Positive detections ---

    #[test]
    fn detects_minun_kirjani() {
        let rule = PossessiveSuffixRule::new();
        // "minun kirjani" — redundant possessive pronoun
        let tokens = vec![word("minun", 0, 5), noun_with_possessive("kirjani", 6, 13)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "POSSESSIVE_REDUNDANCY");
        assert!(errors[0].message.contains("minun"));
        assert!(errors[0].message.contains("kirjani"));
        assert_eq!(errors[0].suggestions, vec!["kirjani"]);
    }

    #[test]
    fn detects_hanen_talonsa() {
        let rule = PossessiveSuffixRule::new();
        let tokens = vec![word("hänen", 0, 6), noun_with_possessive("talonsa", 7, 14)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn detects_meidan_with_possessive() {
        let rule = PossessiveSuffixRule::new();
        let tokens = vec![word("meidän", 0, 7), noun_with_possessive("talomme", 8, 15)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    // --- No false positives ---

    #[test]
    fn no_error_without_possessive_suffix() {
        let rule = PossessiveSuffixRule::new();
        // "minun kirja" — pronoun but no possessive suffix on noun
        let tokens = vec![word("minun", 0, 5), noun_without_possessive("kirja", 6, 11)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_non_pronoun() {
        let rule = PossessiveSuffixRule::new();
        // "iso kirjani" — "iso" is not a possessive pronoun
        let tokens = vec![word("iso", 0, 3), noun_with_possessive("kirjani", 4, 11)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_without_analysis() {
        let rule = PossessiveSuffixRule::new();
        let tokens = vec![word("minun", 0, 5), word("kirjani", 6, 13)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = PossessiveSuffixRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_single_word() {
        let rule = PossessiveSuffixRule::new();
        let tokens = vec![word("minun", 0, 5)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn case_insensitive_pronoun() {
        let rule = PossessiveSuffixRule::new();
        // "Minun kirjani" — capitalized pronoun, should still detect
        let tokens = vec![word("Minun", 0, 5), noun_with_possessive("kirjani", 6, 13)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn rule_id() {
        let rule = PossessiveSuffixRule::new();
        assert_eq!(rule.id(), "POSSESSIVE_REDUNDANCY");
    }

    #[test]
    fn default_trait() {
        let rule = PossessiveSuffixRule::default();
        assert_eq!(rule.id(), "POSSESSIVE_REDUNDANCY");
    }
}
