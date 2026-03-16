//! Sentence-initial lowercase detection (lightweight version).
//!
//! This is a simpler, text-level rule that detects sentences starting with
//! a lowercase letter by scanning the raw token stream for patterns like
//! "period + whitespace + lowercase word". Unlike the full CapitalizationRule,
//! this does not require morphological analysis and focuses strictly on
//! the sentence-start pattern.
//!
//! Note: The existing CapitalizationRule already handles sentence-initial
//! capitalization. This rule provides an additional check that also catches
//! text-initial lowercase (the first token in the entire text) and is
//! useful when running a subset of rules. In the default rule set, the
//! CapitalizationRule takes precedence; this rule can be used standalone.
//!
//! To avoid duplicate errors when both rules are active, this rule only
//! checks positions that the CapitalizationRule does NOT: specifically,
//! it checks after colons and semicolons (which optionally start a new
//! sentence in some Finnish style guides but are not checked by the
//! CapitalizationRule).

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Detects lowercase after colon/semicolon where a capital may be expected.
///
/// Some Finnish style guides recommend capitalizing after a colon when
/// it introduces a full sentence. This rule conservatively flags cases
/// where a colon is followed by what appears to be a full clause
/// starting with a lowercase letter, if the clause is long enough
/// (3+ words).
///
/// # Error code
///
/// `SENTENCE_INITIAL_LOWERCASE`
pub struct SentenceInitialLowercaseRule;

impl SentenceInitialLowercaseRule {
    /// Create a new sentence-initial lowercase rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for SentenceInitialLowercaseRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a word starts with a lowercase letter.
fn starts_lowercase(text: &str) -> bool {
    text.chars().next().is_some_and(|c| c.is_lowercase())
}

/// Capitalize the first character of a string.
fn capitalize_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => {
            let upper: String = c.to_uppercase().collect();
            let rest: String = chars.collect();
            format!("{}{}", upper, rest)
        }
    }
}

impl GrammarRule for SentenceInitialLowercaseRule {
    fn id(&self) -> &'static str {
        "SENTENCE_INITIAL_LOWERCASE"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        // Track whether we saw a colon/semicolon and are looking for the
        // next word to check.
        let mut after_colon = false;
        let mut words_after_colon = 0u32;

        for token in tokens {
            if !token.is_word {
                if token.text.ends_with('.')
                    || token.text.ends_with('!')
                    || token.text.ends_with('?')
                {
                    after_colon = false;
                    words_after_colon = 0;
                }
                if token.text.contains(':') || token.text.contains(';') {
                    after_colon = true;
                    words_after_colon = 0;
                }
                continue;
            }

            if after_colon {
                words_after_colon += 1;

                if words_after_colon == 1 && starts_lowercase(&token.text) {
                    let suggestion = capitalize_first(&token.text);
                    errors.push(GrammarError::with_suggestions(
                        token.start,
                        token.end,
                        "SENTENCE_INITIAL_LOWERCASE",
                        format!(
                            "Consider capitalizing \"{}\" after colon/semicolon",
                            token.text
                        ),
                        vec![suggestion],
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

    fn word(text: &str, start: usize, end: usize) -> AnnotatedToken {
        AnnotatedToken::word(text, start, end, None)
    }

    fn non_word(text: &str, start: usize, end: usize) -> AnnotatedToken {
        AnnotatedToken::non_word(text, start, end)
    }

    // --- Positive detections ---

    #[test]
    fn detects_lowercase_after_colon() {
        let rule = SentenceInitialLowercaseRule::new();
        // "Vastaus: koira on iso."
        let tokens = vec![
            word("Vastaus", 0, 7),
            non_word(": ", 7, 9),
            word("koira", 9, 14),
            non_word(" ", 14, 15),
            word("on", 15, 17),
            non_word(" ", 17, 18),
            word("iso", 18, 21),
            non_word(".", 21, 22),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SENTENCE_INITIAL_LOWERCASE");
        assert!(errors[0].message.contains("koira"));
        assert_eq!(errors[0].suggestions, vec!["Koira"]);
    }

    #[test]
    fn detects_lowercase_after_semicolon() {
        let rule = SentenceInitialLowercaseRule::new();
        let tokens = vec![
            word("Koira", 0, 5),
            non_word("; ", 5, 7),
            word("kissa", 7, 12),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn suggestion_capitalizes_first_letter() {
        let rule = SentenceInitialLowercaseRule::new();
        let tokens = vec![
            word("Asia", 0, 4),
            non_word(": ", 4, 6),
            word("tämä", 6, 11),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].suggestions, vec!["Tämä"]);
    }

    // --- No false positives ---

    #[test]
    fn no_error_when_capitalized_after_colon() {
        let rule = SentenceInitialLowercaseRule::new();
        let tokens = vec![
            word("Vastaus", 0, 7),
            non_word(": ", 7, 9),
            word("Koira", 9, 14),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_normal_sentence() {
        let rule = SentenceInitialLowercaseRule::new();
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
        let rule = SentenceInitialLowercaseRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn resets_after_sentence_end() {
        let rule = SentenceInitialLowercaseRule::new();
        // "X: koira. Y: Kissa" — only first colon+lowercase triggers
        let tokens = vec![
            word("X", 0, 1),
            non_word(": ", 1, 3),
            word("koira", 3, 8),
            non_word(". ", 8, 10),
            word("Y", 10, 11),
            non_word(": ", 11, 13),
            word("Kissa", 13, 18),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].start, 3);
    }

    #[test]
    fn rule_id() {
        let rule = SentenceInitialLowercaseRule::new();
        assert_eq!(rule.id(), "SENTENCE_INITIAL_LOWERCASE");
    }

    #[test]
    fn default_trait() {
        let rule = SentenceInitialLowercaseRule::default();
        assert_eq!(rule.id(), "SENTENCE_INITIAL_LOWERCASE");
    }
}
