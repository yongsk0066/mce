//! Missing main verb detection (simple heuristic).
//!
//! In Finnish, every independent clause (sentence) should contain a finite
//! verb. This rule uses a simple heuristic: if a sentence has 3 or more
//! words but none of them is classified as a verb (teonsana or kieltosana),
//! it reports a warning.
//!
//! # Limitations
//!
//! - Requires morphological analysis with CLASS attribute
//! - Does not handle imperative forms that may be analyzed differently
//! - Short sentences (1-2 words) are excluded to avoid false positives
//!   on titles, headings, and fragments
//! - Does not distinguish between finite and non-finite verb forms

use mce_core::analysis::ATTR_CLASS;

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Minimum number of words in a sentence to trigger the check.
/// Shorter sentences are likely titles, headings, or intentional fragments.
const MIN_SENTENCE_WORDS: usize = 3;

/// Word classes that count as verbs.
const VERB_CLASSES: &[&str] = &[
    "teonsana",   // verb
    "kieltosana", // negation verb
];

/// Detects sentences that may be missing a finite verb.
///
/// Scans each sentence (delimited by `.`, `!`, `?`) and flags those
/// with 3+ words but no verb-class token.
///
/// # Error code
///
/// `MISSING_MAIN_VERB`
///
/// # Requires
///
/// Morphological analysis with CLASS attribute.
pub struct MissingMainVerbRule;

impl MissingMainVerbRule {
    /// Create a new missing main verb rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for MissingMainVerbRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a token is a verb.
fn is_verb(token: &AnnotatedToken) -> bool {
    token
        .analysis
        .as_ref()
        .and_then(|a| a.get(ATTR_CLASS))
        .is_some_and(|c| VERB_CLASSES.contains(&c))
}

/// Check if a non-word token ends a sentence.
fn is_sentence_ending(text: &str) -> bool {
    text.ends_with('.') || text.ends_with('!') || text.ends_with('?')
}

impl GrammarRule for MissingMainVerbRule {
    fn id(&self) -> &'static str {
        "MISSING_MAIN_VERB"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        let mut sentence_words: Vec<&AnnotatedToken> = Vec::new();
        let mut sentence_start: Option<usize> = None;
        let mut has_analysis = false;

        for token in tokens {
            if token.is_word {
                if sentence_start.is_none() {
                    sentence_start = Some(token.start);
                }
                if token.analysis.is_some() {
                    has_analysis = true;
                }
                sentence_words.push(token);
            } else if is_sentence_ending(&token.text) {
                if has_analysis
                    && sentence_words.len() >= MIN_SENTENCE_WORDS
                    && !sentence_words.iter().any(|t| is_verb(t))
                {
                    let start = sentence_start.unwrap_or(0);
                    let end = token.end;
                    errors.push(GrammarError::new(
                        start,
                        end,
                        "MISSING_MAIN_VERB",
                        "Sentence appears to be missing a finite verb".to_string(),
                    ));
                }
                sentence_words.clear();
                sentence_start = None;
                has_analysis = false;
            }
        }

        // Check the last sentence if it doesn't end with punctuation.
        if has_analysis
            && sentence_words.len() >= MIN_SENTENCE_WORDS
            && !sentence_words.iter().any(|t| is_verb(t))
        {
            let start = sentence_start.unwrap_or(0);
            let end = sentence_words.last().map(|t| t.end).unwrap_or(0);
            errors.push(GrammarError::new(
                start,
                end,
                "MISSING_MAIN_VERB",
                "Sentence appears to be missing a finite verb".to_string(),
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

    fn word_noun(text: &str, start: usize, end: usize) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "nimisana");
        AnnotatedToken::word(text, start, end, Some(a))
    }

    fn word_verb(text: &str, start: usize, end: usize) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "teonsana");
        AnnotatedToken::word(text, start, end, Some(a))
    }

    fn word_adj(text: &str, start: usize, end: usize) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "laatusana");
        AnnotatedToken::word(text, start, end, Some(a))
    }

    fn non_word(text: &str, start: usize, end: usize) -> AnnotatedToken {
        AnnotatedToken::non_word(text, start, end)
    }

    // --- Positive detections ---

    #[test]
    fn detects_missing_verb_in_sentence() {
        let rule = MissingMainVerbRule::new();
        // "Iso koira talossa." — 3 words, no verb
        let tokens = vec![
            word_adj("Iso", 0, 3),
            word_noun("koira", 4, 9),
            word_noun("talossa", 10, 17),
            non_word(".", 17, 18),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "MISSING_MAIN_VERB");
        assert!(errors[0].message.contains("finite verb"));
    }

    #[test]
    fn detects_missing_verb_without_trailing_punct() {
        let rule = MissingMainVerbRule::new();
        // "Iso koira talossa" — no period but 3 words
        let tokens = vec![
            word_adj("Iso", 0, 3),
            word_noun("koira", 4, 9),
            word_noun("talossa", 10, 17),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn detects_in_second_sentence() {
        let rule = MissingMainVerbRule::new();
        // "Koira juoksee. Iso punainen talo."
        let tokens = vec![
            word_noun("Koira", 0, 5),
            word_verb("juoksee", 6, 13),
            non_word(".", 13, 14),
            word_adj("Iso", 15, 18),
            word_adj("punainen", 19, 27),
            word_noun("talo", 28, 32),
            non_word(".", 32, 33),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].start, 15);
    }

    // --- No false positives ---

    #[test]
    fn no_error_when_verb_present() {
        let rule = MissingMainVerbRule::new();
        // "Koira juoksee nopeasti."
        let tokens = vec![
            word_noun("Koira", 0, 5),
            word_verb("juoksee", 6, 13),
            word_adj("nopeasti", 14, 22),
            non_word(".", 22, 23),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_short_sentence() {
        let rule = MissingMainVerbRule::new();
        // "Iso talo." — only 2 words, below threshold
        let tokens = vec![
            word_adj("Iso", 0, 3),
            word_noun("talo", 4, 8),
            non_word(".", 8, 9),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_without_analysis() {
        let rule = MissingMainVerbRule::new();
        // No morphological analysis — rule cannot determine verb presence.
        let tokens = vec![
            word("Iso", 0, 3),
            word("koira", 4, 9),
            word("talossa", 10, 17),
            non_word(".", 17, 18),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = MissingMainVerbRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_single_word() {
        let rule = MissingMainVerbRule::new();
        let tokens = vec![word_noun("Koira", 0, 5), non_word(".", 5, 6)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = MissingMainVerbRule::new();
        assert_eq!(rule.id(), "MISSING_MAIN_VERB");
    }

    #[test]
    fn default_trait() {
        let rule = MissingMainVerbRule::default();
        assert_eq!(rule.id(), "MISSING_MAIN_VERB");
    }
}
