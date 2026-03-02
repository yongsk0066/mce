//! Missing comma before subordinate clause (extended set).
//!
//! This rule extends the existing `CommaBeforeConjunctionRule` by covering
//! additional subordinating conjunctions and relative pronouns that require
//! a preceding comma in Finnish. While `CommaBeforeConjunctionRule` handles
//! the core set (kun, koska, vaikka, jos, etc.), this rule adds:
//!
//! - Relative pronouns: joka, mikä (when used as relative pronoun)
//! - Additional conjunctions: sillä, johon, jossa, josta, jolle, jolta,
//!   jolla, joiden, joita, joissa, joista
//!
//! In Finnish, relative clauses introduced by "joka" and its inflected
//! forms always require a preceding comma.
//!
//! # Difference from CommaBeforeConjunctionRule
//!
//! This rule specifically targets relative pronouns and their inflected
//! forms, which have different grammatical behavior from subordinating
//! conjunctions but share the comma requirement.

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Relative pronouns and related words that require a preceding comma.
/// These are forms of "joka" (which/that) and "mikä" (which/what).
const RELATIVE_WORDS: &[&str] = &[
    // Nominative
    "joka", "jotka", // Genitive
    "jonka", "joiden", // Partitive
    "jota", "joita", // Inessive
    "jossa", "joissa", // Elative
    "josta", "joista", // Illative
    "johon", "joihin", // Adessive
    "jolla", "joilla", // Ablative
    "jolta", "joilta", // Allative
    "jolle", "joille", // "mikä" as relative pronoun (common forms)
    "mikä", "mitkä", "minkä", "mitä", "missä", "mistä", "mihin",
    // "sillä" as a causal conjunction
    "sillä",
];

/// Detects missing comma before relative pronouns and related conjunctions.
///
/// # Error code
///
/// `COMMA_BEFORE_RELATIVE`
///
/// # Example
///
/// ```
/// use mce_grammar::{AnnotatedToken, GrammarRule};
/// use mce_grammar::rules::CommaInSubordinateRule;
///
/// let rule = CommaInSubordinateRule::new();
/// let tokens = vec![
///     AnnotatedToken::word("Talo", 0, 4, None),
///     AnnotatedToken::non_word(" ", 4, 5),
///     AnnotatedToken::word("joka", 5, 9, None),
///     AnnotatedToken::non_word(" ", 9, 10),
///     AnnotatedToken::word("on", 10, 12, None),
/// ];
/// let errors = rule.check(&tokens);
/// assert_eq!(errors.len(), 1);
/// assert_eq!(errors[0].code, "COMMA_BEFORE_RELATIVE");
/// ```
pub struct CommaInSubordinateRule;

impl CommaInSubordinateRule {
    /// Create a new comma-in-subordinate rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CommaInSubordinateRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a word is a relative pronoun or conjunction (case-insensitive).
fn is_relative_word(text: &str) -> bool {
    let lower = text.to_lowercase();
    RELATIVE_WORDS.contains(&lower.as_str())
}

impl GrammarRule for CommaInSubordinateRule {
    fn id(&self) -> &'static str {
        "COMMA_BEFORE_RELATIVE"
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
                sentence_start = false;
                prev_word = Some(token);
                comma_seen_since_last_word = false;
                continue;
            }

            if let Some(prev) = prev_word {
                if is_relative_word(&token.text) && !comma_seen_since_last_word {
                    errors.push(GrammarError::with_suggestions(
                        prev.end,
                        token.start,
                        "COMMA_BEFORE_RELATIVE",
                        format!(
                            "Missing comma before relative pronoun/conjunction \"{}\"",
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
    fn detects_missing_comma_before_joka() {
        let rule = CommaInSubordinateRule::new();
        // "Talo joka on iso" — missing comma before "joka"
        let tokens = vec![
            word("Talo", 0, 4),
            non_word(" ", 4, 5),
            word("joka", 5, 9),
            non_word(" ", 9, 10),
            word("on", 10, 12),
            non_word(" ", 12, 13),
            word("iso", 13, 16),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "COMMA_BEFORE_RELATIVE");
        assert!(errors[0].message.contains("joka"));
    }

    #[test]
    fn detects_missing_comma_before_jossa() {
        let rule = CommaInSubordinateRule::new();
        // "Talo jossa asun" — missing comma
        let tokens = vec![
            word("Talo", 0, 4),
            non_word(" ", 4, 5),
            word("jossa", 5, 10),
            non_word(" ", 10, 11),
            word("asun", 11, 15),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("jossa"));
    }

    #[test]
    fn detects_missing_comma_before_silla() {
        let rule = CommaInSubordinateRule::new();
        let tokens = vec![
            word("Tulin", 0, 5),
            non_word(" ", 5, 6),
            word("kotiin", 6, 12),
            non_word(" ", 12, 13),
            word("sillä", 13, 19),
            non_word(" ", 19, 20),
            word("satoi", 20, 25),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn suggestion_includes_comma() {
        let rule = CommaInSubordinateRule::new();
        let tokens = vec![word("Talo", 0, 4), non_word(" ", 4, 5), word("joka", 5, 9)];
        let errors = rule.check(&tokens);
        assert_eq!(errors[0].suggestions, vec![", joka"]);
    }

    // --- No false positives ---

    #[test]
    fn no_error_when_comma_present() {
        let rule = CommaInSubordinateRule::new();
        // "Talo, joka on iso" — comma present
        let tokens = vec![
            word("Talo", 0, 4),
            non_word(", ", 4, 6),
            word("joka", 6, 10),
            non_word(" ", 10, 11),
            word("on", 11, 13),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_at_sentence_start() {
        let rule = CommaInSubordinateRule::new();
        // "Joka päivä..." — "joka" at sentence start means "every"
        let tokens = vec![
            word("Joka", 0, 4),
            non_word(" ", 4, 5),
            word("päivä", 5, 11),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_non_relative_words() {
        let rule = CommaInSubordinateRule::new();
        let tokens = vec![
            word("Koira", 0, 5),
            non_word(" ", 5, 6),
            word("juoksee", 6, 13),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = CommaInSubordinateRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn resets_at_sentence_boundary() {
        let rule = CommaInSubordinateRule::new();
        // "Satoi. Joka tapauksessa..." — "Joka" starts new sentence
        let tokens = vec![
            word("Satoi", 0, 5),
            non_word(".", 5, 6),
            non_word(" ", 6, 7),
            word("Joka", 7, 11),
            non_word(" ", 11, 12),
            word("tapauksessa", 12, 23),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = CommaInSubordinateRule::new();
        assert_eq!(rule.id(), "COMMA_BEFORE_RELATIVE");
    }

    #[test]
    fn default_trait() {
        let rule = CommaInSubordinateRule::default();
        assert_eq!(rule.id(), "COMMA_BEFORE_RELATIVE");
    }
}
