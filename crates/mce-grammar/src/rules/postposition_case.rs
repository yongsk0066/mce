//! Postposition case government checking.
//!
//! In Finnish, postpositions require their complement noun to be in a
//! specific grammatical case, most commonly genitive. For example:
//! - "talon jälkeen" (after the house) — genitive + postposition, correct
//! - "talossa jälkeen" (in the house after) — inessive + postposition, wrong
//!
//! This rule checks that when a known postposition follows a noun, the
//! noun is in the expected case (typically genitive = "ompitkasija").
//!
//! # Limitations
//!
//! - Only checks adjacent noun + postposition pairs
//! - Uses a fixed list of common postpositions requiring genitive
//! - Requires morphological analysis with CLASS and SIJAMUOTO

use mce_core::analysis::{ATTR_CLASS, ATTR_SIJAMUOTO};

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Common Finnish postpositions that require genitive case on the preceding noun.
const GENITIVE_POSTPOSITIONS: &[&str] = &[
    "jälkeen",  // after
    "aikana",   // during
    "kautta",   // through
    "kanssa",   // with
    "vuoksi",   // because of
    "takia",    // because of
    "puolesta", // on behalf of
    "sijaan",   // instead of
    "ympäri",   // around
    "keskellä", // in the middle of
    "takana",   // behind
    "edessä",   // in front of
    "vieressä", // next to
    "alla",     // under
    "päällä",   // on top of
    "sisällä",  // inside
    "lähellä",  // near
    "luona",    // at (someone's place)
    "mukaan",   // according to
];

/// Noun classes whose case we check.
const NOUN_CLASSES: &[&str] = &[
    "nimisana",   // common noun
    "etunimi",    // first name
    "sukunimi",   // surname
    "paikannimi", // place name
    "nimi",       // name
];

/// Detects postposition case government errors.
///
/// # Error code
///
/// `POSTPOSITION_CASE`
///
/// # Requires
///
/// Morphological analysis with CLASS and SIJAMUOTO attributes.
pub struct PostpositionCaseRule;

impl PostpositionCaseRule {
    /// Create a new postposition case rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for PostpositionCaseRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a word is a known genitive-requiring postposition (case-insensitive).
fn is_genitive_postposition(text: &str) -> bool {
    let lower = text.to_lowercase();
    GENITIVE_POSTPOSITIONS.contains(&lower.as_str())
}

impl GrammarRule for PostpositionCaseRule {
    fn id(&self) -> &'static str {
        "POSTPOSITION_CASE"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        let word_tokens: Vec<&AnnotatedToken> = tokens.iter().filter(|t| t.is_word).collect();

        for window in word_tokens.windows(2) {
            let noun = window[0];
            let postp = window[1];

            // Check if the second word is a postposition.
            if !is_genitive_postposition(&postp.text) {
                continue;
            }

            // Check if the first word is a noun.
            let noun_class = match noun.analysis.as_ref().and_then(|a| a.get(ATTR_CLASS)) {
                Some(c) if NOUN_CLASSES.contains(&c) => c,
                _ => continue,
            };
            let _ = noun_class;

            // Check that the noun is in genitive case.
            let case = match noun.analysis.as_ref().and_then(|a| a.get(ATTR_SIJAMUOTO)) {
                Some(c) => c,
                None => continue, // no case info, skip
            };

            if case != "ompitkasija" {
                errors.push(GrammarError::new(
                    noun.start,
                    postp.end,
                    "POSTPOSITION_CASE",
                    format!(
                        "Postposition \"{}\" requires genitive case, but \"{}\" appears to be in a different case",
                        postp.text, noun.text
                    ),
                ));
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

    fn noun_with_case(text: &str, start: usize, end: usize, case: &str) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "nimisana");
        a.set(ATTR_SIJAMUOTO, case);
        AnnotatedToken::word(text, start, end, Some(a))
    }

    // --- Positive detections ---

    #[test]
    fn detects_inessive_before_jalkeen() {
        let rule = PostpositionCaseRule::new();
        // "talossa jälkeen" — inessive + postposition, wrong
        let tokens = vec![
            noun_with_case("talossa", 0, 7, "sispitkasija"),
            word("jälkeen", 8, 16),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "POSTPOSITION_CASE");
        assert!(errors[0].message.contains("jälkeen"));
    }

    #[test]
    fn detects_partitive_before_kanssa() {
        let rule = PostpositionCaseRule::new();
        let tokens = vec![
            noun_with_case("koiraa", 0, 6, "ospitkasija"),
            word("kanssa", 7, 13),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("kanssa"));
    }

    #[test]
    fn detects_nominative_before_aikana() {
        let rule = PostpositionCaseRule::new();
        let tokens = vec![
            noun_with_case("kesä", 0, 5, "npitkasija"),
            word("aikana", 6, 12),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    // --- No false positives ---

    #[test]
    fn no_error_for_genitive_before_postposition() {
        let rule = PostpositionCaseRule::new();
        // "talon jälkeen" — genitive, correct
        let tokens = vec![
            noun_with_case("talon", 0, 5, "ompitkasija"),
            word("jälkeen", 6, 14),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_non_postposition_word() {
        let rule = PostpositionCaseRule::new();
        let tokens = vec![
            noun_with_case("talossa", 0, 7, "sispitkasija"),
            word("asuu", 8, 12),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_without_analysis() {
        let rule = PostpositionCaseRule::new();
        let tokens = vec![word("talon", 0, 5), word("jälkeen", 6, 14)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = PostpositionCaseRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_single_word() {
        let rule = PostpositionCaseRule::new();
        let tokens = vec![word("jälkeen", 0, 8)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = PostpositionCaseRule::new();
        assert_eq!(rule.id(), "POSTPOSITION_CASE");
    }

    #[test]
    fn default_trait() {
        let rule = PostpositionCaseRule::default();
        assert_eq!(rule.id(), "POSTPOSITION_CASE");
    }
}
