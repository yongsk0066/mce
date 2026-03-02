//! Comparative + partitive checking.
//!
//! In Finnish, when a comparative adjective is followed by "kuin" (than),
//! the noun after "kuin" should generally be in the same case as the
//! compared noun. However, a common error is using the wrong case after
//! "kuin" with comparatives.
//!
//! More specifically, this rule checks that a comparative form (COMPARISON
//! = "comparative") is actually followed by a proper "kuin" construction
//! rather than a partitive noun directly. The pattern "isompi taloa"
//! (bigger house-partitive) without "kuin" is incorrect — it should be
//! "isompi kuin talo" or "isompi talo".
//!
//! # Limitations
//!
//! - Only checks comparative adjective + partitive noun (without "kuin")
//! - Requires COMPARISON attribute on the adjective
//! - Does not handle all comparative constructions

use mce_core::analysis::{ATTR_CLASS, ATTR_COMPARISON, ATTR_SIJAMUOTO};

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Detects incorrect partitive after comparative without "kuin".
///
/// # Error code
///
/// `COMPARATIVE_PARTITIVE`
///
/// # Requires
///
/// Morphological analysis with CLASS, COMPARISON, and SIJAMUOTO attributes.
pub struct ComparativePartitiveRule;

impl ComparativePartitiveRule {
    /// Create a new comparative partitive rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ComparativePartitiveRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Noun classes.
const NOUN_CLASSES: &[&str] = &["nimisana", "etunimi", "sukunimi", "paikannimi", "nimi"];

/// Check if a token is a comparative adjective.
fn is_comparative(token: &AnnotatedToken) -> bool {
    let analysis = match &token.analysis {
        Some(a) => a,
        None => return false,
    };
    let class = match analysis.get(ATTR_CLASS) {
        Some(c) => c,
        None => return false,
    };
    if class != "laatusana" {
        return false;
    }
    analysis.get(ATTR_COMPARISON) == Some("comparative")
}

/// Check if a token is a noun in partitive case.
fn is_partitive_noun(token: &AnnotatedToken) -> bool {
    let analysis = match &token.analysis {
        Some(a) => a,
        None => return false,
    };
    let class = match analysis.get(ATTR_CLASS) {
        Some(c) => c,
        None => return false,
    };
    if !NOUN_CLASSES.contains(&class) {
        return false;
    }
    let case = match analysis.get(ATTR_SIJAMUOTO) {
        Some(c) => c,
        None => return false,
    };
    case == "ospitkasija" || case == "partitiivi"
}

impl GrammarRule for ComparativePartitiveRule {
    fn id(&self) -> &'static str {
        "COMPARATIVE_PARTITIVE"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        let word_tokens: Vec<&AnnotatedToken> = tokens.iter().filter(|t| t.is_word).collect();

        // Look for: comparative adjective + partitive noun (without "kuin" between).
        for window in word_tokens.windows(2) {
            let adj = window[0];
            let noun = window[1];

            if !is_comparative(adj) {
                continue;
            }

            if !is_partitive_noun(noun) {
                continue;
            }

            // This pattern (comparative + partitive without kuin) is suspicious.
            errors.push(GrammarError::new(
                adj.start,
                noun.end,
                "COMPARATIVE_PARTITIVE",
                format!(
                    "Comparative \"{}\" followed by partitive \"{}\" — \
                     did you mean \"{} kuin {}\" or \"{} {}\" (nominative)?",
                    adj.text, noun.text, adj.text, noun.text, adj.text, noun.text
                ),
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

    fn comparative_adj(text: &str, start: usize, end: usize) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "laatusana");
        a.set(ATTR_COMPARISON, "comparative");
        AnnotatedToken::word(text, start, end, Some(a))
    }

    fn partitive_noun(text: &str, start: usize, end: usize) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "nimisana");
        a.set(ATTR_SIJAMUOTO, "ospitkasija");
        AnnotatedToken::word(text, start, end, Some(a))
    }

    fn nominative_noun(text: &str, start: usize, end: usize) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "nimisana");
        a.set(ATTR_SIJAMUOTO, "npitkasija");
        AnnotatedToken::word(text, start, end, Some(a))
    }

    // --- Positive detections ---

    #[test]
    fn detects_comparative_plus_partitive() {
        let rule = ComparativePartitiveRule::new();
        // "isompi taloa" — comparative + partitive without kuin
        let tokens = vec![
            comparative_adj("isompi", 0, 6),
            partitive_noun("taloa", 7, 12),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "COMPARATIVE_PARTITIVE");
        assert!(errors[0].message.contains("isompi"));
        assert!(errors[0].message.contains("taloa"));
    }

    #[test]
    fn detects_another_comparative_partitive() {
        let rule = ComparativePartitiveRule::new();
        let tokens = vec![
            comparative_adj("nopeampi", 0, 8),
            partitive_noun("autoa", 9, 14),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    // --- No false positives ---

    #[test]
    fn no_error_for_comparative_kuin_noun() {
        let rule = ComparativePartitiveRule::new();
        // "isompi kuin talo" — kuin separates, so "talo" isn't adjacent
        let tokens = vec![
            comparative_adj("isompi", 0, 6),
            word("kuin", 7, 11),
            nominative_noun("talo", 12, 16),
        ];
        let errors = rule.check(&tokens);
        // The word_tokens window [comparative, "kuin"] does not match
        // because "kuin" is not a partitive noun.
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_comparative_plus_nominative() {
        let rule = ComparativePartitiveRule::new();
        // "isompi talo" — nominative, correct
        let tokens = vec![
            comparative_adj("isompi", 0, 6),
            nominative_noun("talo", 7, 11),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_without_analysis() {
        let rule = ComparativePartitiveRule::new();
        let tokens = vec![word("isompi", 0, 6), word("taloa", 7, 12)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = ComparativePartitiveRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_non_comparative_adjective() {
        let rule = ComparativePartitiveRule::new();
        // Regular (positive) adjective + partitive noun is fine.
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "laatusana");
        a.set(ATTR_COMPARISON, "positive");
        let adj = AnnotatedToken::word("iso", 0, 3, Some(a));
        let tokens = vec![adj, partitive_noun("taloa", 4, 9)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = ComparativePartitiveRule::new();
        assert_eq!(rule.id(), "COMPARATIVE_PARTITIVE");
    }

    #[test]
    fn default_trait() {
        let rule = ComparativePartitiveRule::default();
        assert_eq!(rule.id(), "COMPARATIVE_PARTITIVE");
    }
}
