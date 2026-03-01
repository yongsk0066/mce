//! Capitalization error detection.
//!
//! Detects two categories of capitalization errors:
//!
//! 1. **Sentence-initial**: The first word of a sentence should be capitalized.
//!    We detect sentence boundaries by looking for punctuation tokens ending
//!    with `.`, `!`, or `?` followed by a word token.
//!
//! 2. **Proper nouns**: Words analyzed as `etunimi` (first name), `sukunimi`
//!    (surname), or `paikannimi` (place name) should be capitalized. This
//!    requires morphological analysis to be present on the token.

use mce_core::analysis::ATTR_CLASS;

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Detects capitalization errors in Finnish text.
///
/// # Error code
///
/// `CAPITALIZATION_ERROR`
///
/// # Checks
///
/// - Sentence-initial word must be capitalized (after `.`, `!`, `?`)
/// - Proper nouns (`etunimi`, `sukunimi`, `paikannimi`) must be capitalized
///
/// # Example
///
/// ```
/// use mce_grammar::{AnnotatedToken, GrammarRule};
/// use mce_grammar::rules::CapitalizationRule;
///
/// let rule = CapitalizationRule::new();
/// let tokens = vec![
///     AnnotatedToken::non_word(".", 4, 5),
///     AnnotatedToken::non_word(" ", 5, 6),
///     AnnotatedToken::word("koira", 6, 11, None),
/// ];
/// let errors = rule.check(&tokens);
/// assert_eq!(errors.len(), 1);
/// assert_eq!(errors[0].code, "CAPITALIZATION_ERROR");
/// ```
pub struct CapitalizationRule;

impl CapitalizationRule {
    /// Create a new capitalization rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CapitalizationRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a punctuation token ends a sentence.
fn is_sentence_ending(text: &str) -> bool {
    text.ends_with('.') || text.ends_with('!') || text.ends_with('?')
}

/// Check if a word starts with an uppercase letter.
fn starts_uppercase(text: &str) -> bool {
    text.chars().next().is_some_and(|c| c.is_uppercase())
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

/// Finnish word classes that represent proper nouns.
const PROPER_NOUN_CLASSES: &[&str] = &["etunimi", "sukunimi", "paikannimi"];

impl GrammarRule for CapitalizationRule {
    fn id(&self) -> &'static str {
        "CAPITALIZATION_ERROR"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        // Track whether we expect the next word to be sentence-initial.
        // The very first word in the text is considered sentence-initial.
        let mut expect_sentence_start = true;

        for token in tokens {
            if !token.is_word {
                // Check if this non-word token is a sentence-ending punctuation.
                if is_sentence_ending(&token.text) {
                    expect_sentence_start = true;
                }
                continue;
            }

            // --- Check 1: Sentence-initial capitalization ---
            if expect_sentence_start {
                if !starts_uppercase(&token.text) && !token.text.is_empty() {
                    // The first char is lowercase at sentence start.
                    let suggestion = capitalize_first(&token.text);
                    errors.push(GrammarError::with_suggestions(
                        token.start,
                        token.end,
                        "CAPITALIZATION_ERROR",
                        format!(
                            "Sentence should start with a capital letter: \"{}\"",
                            token.text
                        ),
                        vec![suggestion],
                    ));
                }
                expect_sentence_start = false;
                // Don't also check proper noun rule for sentence-initial words,
                // because the capitalization requirement is already covered.
                continue;
            }

            // --- Check 2: Proper noun capitalization ---
            if let Some(ref analysis) = token.analysis {
                if let Some(class) = analysis.get(ATTR_CLASS) {
                    if PROPER_NOUN_CLASSES.contains(&class) && !starts_uppercase(&token.text) {
                        let suggestion = capitalize_first(&token.text);
                        errors.push(GrammarError::with_suggestions(
                            token.start,
                            token.end,
                            "CAPITALIZATION_ERROR",
                            format!("Proper noun should be capitalized: \"{}\"", token.text),
                            vec![suggestion],
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

    fn word_with_class(text: &str, start: usize, end: usize, class: &str) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, class);
        AnnotatedToken::word(text, start, end, Some(a))
    }

    fn punct(text: &str, start: usize, end: usize) -> AnnotatedToken {
        AnnotatedToken::non_word(text, start, end)
    }

    fn ws(start: usize, end: usize) -> AnnotatedToken {
        AnnotatedToken::non_word(" ", start, end)
    }

    // -----------------------------------------------------------------------
    // Sentence-initial capitalization
    // -----------------------------------------------------------------------

    #[test]
    fn error_when_first_word_lowercase() {
        let rule = CapitalizationRule::new();
        let tokens = vec![word("koira", 0, 5)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "CAPITALIZATION_ERROR");
        assert_eq!(errors[0].start, 0);
        assert_eq!(errors[0].end, 5);
        assert!(errors[0].message.contains("capital letter"));
        assert_eq!(errors[0].suggestions, vec!["Koira"]);
    }

    #[test]
    fn no_error_when_first_word_uppercase() {
        let rule = CapitalizationRule::new();
        let tokens = vec![word("Koira", 0, 5)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn error_after_period() {
        let rule = CapitalizationRule::new();
        // "Koira juoksee. kissa nukkuu."
        let tokens = vec![
            word("Koira", 0, 5),
            ws(5, 6),
            word("juoksee", 6, 13),
            punct(".", 13, 14),
            ws(14, 15),
            word("kissa", 15, 20),
            ws(20, 21),
            word("nukkuu", 21, 27),
            punct(".", 27, 28),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].start, 15); // "kissa"
        assert_eq!(errors[0].suggestions, vec!["Kissa"]);
    }

    #[test]
    fn error_after_question_mark() {
        let rule = CapitalizationRule::new();
        let tokens = vec![
            word("Miksi", 0, 5),
            punct("?", 5, 6),
            ws(6, 7),
            word("koska", 7, 12),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].start, 7);
    }

    #[test]
    fn error_after_exclamation_mark() {
        let rule = CapitalizationRule::new();
        let tokens = vec![
            word("Hei", 0, 3),
            punct("!", 3, 4),
            ws(4, 5),
            word("tule", 5, 9),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].start, 5);
    }

    #[test]
    fn no_error_after_comma() {
        let rule = CapitalizationRule::new();
        let tokens = vec![
            word("Koira", 0, 5),
            punct(",", 5, 6),
            ws(6, 7),
            word("kissa", 7, 12),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = CapitalizationRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    // -----------------------------------------------------------------------
    // Proper noun capitalization
    // -----------------------------------------------------------------------

    #[test]
    fn error_for_lowercase_first_name() {
        let rule = CapitalizationRule::new();
        let tokens = vec![
            word("Tapaan", 0, 6),
            ws(6, 7),
            word_with_class("matti", 7, 12, "etunimi"),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("Proper noun"));
        assert_eq!(errors[0].suggestions, vec!["Matti"]);
    }

    #[test]
    fn error_for_lowercase_place_name() {
        let rule = CapitalizationRule::new();
        let tokens = vec![
            word("Asun", 0, 4),
            ws(4, 5),
            word_with_class("helsinki", 5, 13, "paikannimi"),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("Proper noun"));
    }

    #[test]
    fn error_for_lowercase_surname() {
        let rule = CapitalizationRule::new();
        let tokens = vec![
            word("Herra", 0, 5),
            ws(5, 6),
            word_with_class("virtanen", 6, 14, "sukunimi"),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn no_error_for_capitalized_proper_noun() {
        let rule = CapitalizationRule::new();
        let tokens = vec![
            word("Tapaan", 0, 6),
            ws(6, 7),
            word_with_class("Matti", 7, 12, "etunimi"),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_common_noun() {
        let rule = CapitalizationRule::new();
        let tokens = vec![
            word("Iso", 0, 3),
            ws(3, 4),
            word_with_class("koira", 4, 9, "nimisana"),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_proper_noun_check_at_sentence_start() {
        // At sentence start, we only check for sentence-initial capitalization,
        // not the proper noun rule. A capitalized sentence start should not
        // produce a double error.
        let rule = CapitalizationRule::new();
        let tokens = vec![word_with_class("Matti", 0, 5, "etunimi")];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    // -----------------------------------------------------------------------
    // Helper function tests
    // -----------------------------------------------------------------------

    #[test]
    fn capitalize_first_basic() {
        assert_eq!(capitalize_first("koira"), "Koira");
        assert_eq!(capitalize_first("Koira"), "Koira");
        assert_eq!(capitalize_first(""), "");
    }

    #[test]
    fn capitalize_first_finnish_chars() {
        assert_eq!(capitalize_first("\u{00E4}iti"), "\u{00C4}iti"); // äiti -> Äiti
    }

    #[test]
    fn is_sentence_ending_checks() {
        assert!(is_sentence_ending("."));
        assert!(is_sentence_ending("!"));
        assert!(is_sentence_ending("?"));
        assert!(is_sentence_ending("..."));
        assert!(!is_sentence_ending(","));
        assert!(!is_sentence_ending(":"));
        assert!(!is_sentence_ending(";"));
    }

    #[test]
    fn starts_uppercase_checks() {
        assert!(starts_uppercase("Koira"));
        assert!(starts_uppercase("\u{00C4}iti")); // Äiti
        assert!(!starts_uppercase("koira"));
        assert!(!starts_uppercase("\u{00E4}iti")); // äiti
        assert!(!starts_uppercase(""));
    }

    #[test]
    fn rule_id() {
        let rule = CapitalizationRule::new();
        assert_eq!(rule.id(), "CAPITALIZATION_ERROR");
    }

    #[test]
    fn default_trait() {
        let rule = CapitalizationRule::default();
        assert_eq!(rule.id(), "CAPITALIZATION_ERROR");
    }
}
