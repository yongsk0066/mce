//! Repeated word detection.
//!
//! Detects consecutive identical words like "koira koira" or "ja ja".
//! Some intentional repetitions are excluded (e.g., "no no" is common
//! in Finnish as an emphatic particle).

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Words that may be intentionally repeated in Finnish.
///
/// These are interjections and particles where repetition is a valid
/// stylistic or semantic pattern.
const ALLOWED_REPETITIONS: &[&str] = &[
    "no",   // "no no" — emphatic particle
    "niin", // "niin niin" — emphatic agreement
    "joo",  // "joo joo" — casual emphatic yes
    "ai",   // "ai ai" — interjection
    "hei",  // "hei hei" — greeting / goodbye
    "nyt",  // "nyt nyt" — emphatic now
    "tule", // "tule tule" — come come (beckoning)
];

/// Detects consecutive identical words.
///
/// Reports an error spanning the second occurrence of the repeated word,
/// with the suggestion to remove it.
///
/// # Error code
///
/// `REPEATED_WORD`
///
/// # Example
///
/// ```
/// use mce_grammar::{AnnotatedToken, GrammarRule};
/// use mce_grammar::rules::RepeatedWordRule;
///
/// let rule = RepeatedWordRule::new();
/// let tokens = vec![
///     AnnotatedToken::word("koira", 0, 5, None),
///     AnnotatedToken::word("koira", 6, 11, None),
/// ];
/// let errors = rule.check(&tokens);
/// assert_eq!(errors.len(), 1);
/// assert_eq!(errors[0].code, "REPEATED_WORD");
/// ```
pub struct RepeatedWordRule;

impl RepeatedWordRule {
    /// Create a new repeated word rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for RepeatedWordRule {
    fn default() -> Self {
        Self::new()
    }
}

impl GrammarRule for RepeatedWordRule {
    fn id(&self) -> &'static str {
        "REPEATED_WORD"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        let word_tokens: Vec<&AnnotatedToken> = tokens.iter().filter(|t| t.is_word).collect();

        for window in word_tokens.windows(2) {
            let prev = window[0];
            let curr = window[1];

            let prev_lower = prev.text.to_lowercase();
            let curr_lower = curr.text.to_lowercase();

            if prev_lower == curr_lower {
                if ALLOWED_REPETITIONS.contains(&prev_lower.as_str()) {
                    continue;
                }

                // Skip very short words (single character) — often intentional
                // in informal writing or special notation.
                if prev_lower.chars().count() <= 1 {
                    continue;
                }

                errors.push(GrammarError::with_suggestions(
                    curr.start,
                    curr.end,
                    "REPEATED_WORD",
                    format!("Repeated word: \"{}\"", curr.text),
                    vec![String::new()], // suggestion: remove the word
                ));
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

    fn punct(text: &str, start: usize, end: usize) -> AnnotatedToken {
        AnnotatedToken::non_word(text, start, end)
    }

    #[test]
    fn detects_repeated_word() {
        let rule = RepeatedWordRule::new();
        let tokens = vec![word("koira", 0, 5), word("koira", 6, 11)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "REPEATED_WORD");
        assert_eq!(errors[0].start, 6);
        assert_eq!(errors[0].end, 11);
        assert!(errors[0].message.contains("koira"));
    }

    #[test]
    fn detects_case_insensitive_repeat() {
        let rule = RepeatedWordRule::new();
        let tokens = vec![word("Koira", 0, 5), word("koira", 6, 11)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn no_error_for_different_words() {
        let rule = RepeatedWordRule::new();
        let tokens = vec![word("koira", 0, 5), word("kissa", 6, 11)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = RepeatedWordRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_single_word() {
        let rule = RepeatedWordRule::new();
        let tokens = vec![word("koira", 0, 5)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn allows_intentional_repetition_no_no() {
        let rule = RepeatedWordRule::new();
        let tokens = vec![word("no", 0, 2), word("no", 3, 5)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn allows_intentional_repetition_niin_niin() {
        let rule = RepeatedWordRule::new();
        let tokens = vec![word("niin", 0, 4), word("niin", 5, 9)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn allows_intentional_repetition_hei_hei() {
        let rule = RepeatedWordRule::new();
        let tokens = vec![word("hei", 0, 3), word("hei", 4, 7)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn skips_single_char_repeats() {
        let rule = RepeatedWordRule::new();
        let tokens = vec![word("a", 0, 1), word("a", 2, 3)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn ignores_punctuation_between_words() {
        // "koira , koira" — the comma is between the words but the two
        // adjacent *word* tokens are still "koira" and "koira".
        let rule = RepeatedWordRule::new();
        let tokens = vec![word("koira", 0, 5), punct(",", 5, 6), word("koira", 7, 12)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn multiple_repeats_detected() {
        let rule = RepeatedWordRule::new();
        let tokens = vec![
            word("koira", 0, 5),
            word("koira", 6, 11),
            word("kissa", 12, 17),
            word("kissa", 18, 23),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn triple_repeat_produces_two_errors() {
        let rule = RepeatedWordRule::new();
        let tokens = vec![word("ja", 0, 2), word("ja", 3, 5), word("ja", 6, 8)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn suggestion_is_removal() {
        let rule = RepeatedWordRule::new();
        let tokens = vec![word("koira", 0, 5), word("koira", 6, 11)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        // Suggestion is empty string (= remove the repeated word).
        assert_eq!(errors[0].suggestions, vec![""]);
    }

    #[test]
    fn rule_id() {
        let rule = RepeatedWordRule::new();
        assert_eq!(rule.id(), "REPEATED_WORD");
    }

    #[test]
    fn default_trait() {
        let rule = RepeatedWordRule::default();
        assert_eq!(rule.id(), "REPEATED_WORD");
    }
}
