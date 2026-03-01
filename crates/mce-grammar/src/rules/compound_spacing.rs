//! Compound word spacing error detection.
//!
//! In Finnish, compound words are written as a single word without spaces
//! (e.g., "jääkaappi" not "jää kaappi"). This rule detects common cases
//! where two adjacent words should likely be a single compound word.
//!
//! The rule maintains a list of known compound words that are frequently
//! written incorrectly with a space.

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Common Finnish compound words that are often incorrectly split.
///
/// Each entry is (first_part, second_part, correct_compound).
const KNOWN_COMPOUNDS: &[(&str, &str, &str)] = &[
    ("jää", "kaappi", "jääkaappi"),
    ("kahvi", "kuppi", "kahvikuppi"),
    ("auto", "talli", "autotalli"),
    ("posti", "laatikko", "postilaatikko"),
    ("kirja", "hylly", "kirjahylly"),
    ("maa", "pallo", "maapallo"),
    ("rautatie", "asema", "rautatieasema"),
    ("pöytä", "liina", "pöytäliina"),
    ("pyyhe", "teline", "pyyheteline"),
    ("huone", "kalu", "huonekalu"),
    ("pesu", "kone", "pesukone"),
    ("tieto", "kone", "tietokone"),
    ("matka", "puhelin", "matkapuhelin"),
    ("palo", "auto", "paloauto"),
    ("poliisi", "auto", "poliisiauto"),
    ("koulu", "kirja", "koulukirja"),
    ("työ", "paikka", "työpaikka"),
    ("työ", "huone", "työhuone"),
    ("koti", "maa", "kotimaa"),
    ("ulko", "maa", "ulkomaa"),
    ("vuokra", "asunto", "vuokra-asunto"),
    ("asuin", "paikka", "asuinpaikka"),
    ("lämpö", "mittari", "lämpömittari"),
    ("sade", "takki", "sadetakki"),
    ("talvi", "takki", "talvitakki"),
    ("kesä", "loma", "kesäloma"),
    ("joulu", "pukki", "joulupukki"),
    ("joulu", "kuusi", "joulukuusi"),
    ("synnyin", "päivä", "syntymäpäivä"),
    ("ruoka", "kauppa", "ruokakauppa"),
    ("koiran", "ruoka", "koiranruoka"),
    ("kissan", "ruoka", "kissanruoka"),
    ("hammas", "harja", "hammasharja"),
    ("hammas", "lääkäri", "hammaslääkäri"),
    ("silmä", "lääkäri", "silmälääkäri"),
    ("pää", "kaupunki", "pääkaupunki"),
    ("vesi", "johto", "vesijohto"),
    ("sähkö", "johto", "sähköjohto"),
];

/// Detects common compound word spacing errors.
///
/// Checks adjacent word pairs against a known list of Finnish compound
/// words that are frequently split incorrectly.
///
/// # Error code
///
/// `COMPOUND_SPACING`
///
/// # Example
///
/// ```
/// use mce_grammar::{AnnotatedToken, GrammarRule};
/// use mce_grammar::rules::CompoundSpacingRule;
///
/// let rule = CompoundSpacingRule::new();
/// let tokens = vec![
///     AnnotatedToken::word("jää", 0, 4, None),
///     AnnotatedToken::non_word(" ", 4, 5),
///     AnnotatedToken::word("kaappi", 5, 11, None),
/// ];
/// let errors = rule.check(&tokens);
/// assert_eq!(errors.len(), 1);
/// assert_eq!(errors[0].code, "COMPOUND_SPACING");
/// ```
pub struct CompoundSpacingRule;

impl CompoundSpacingRule {
    /// Create a new compound spacing rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CompoundSpacingRule {
    fn default() -> Self {
        Self::new()
    }
}

impl GrammarRule for CompoundSpacingRule {
    fn id(&self) -> &'static str {
        "COMPOUND_SPACING"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        let word_tokens: Vec<&AnnotatedToken> = tokens.iter().filter(|t| t.is_word).collect();

        for window in word_tokens.windows(2) {
            let first = window[0];
            let second = window[1];

            let first_lower = first.text.to_lowercase();
            let second_lower = second.text.to_lowercase();

            for &(part1, part2, compound) in KNOWN_COMPOUNDS {
                if first_lower == part1 && second_lower == part2 {
                    errors.push(GrammarError::with_suggestions(
                        first.start,
                        second.end,
                        "COMPOUND_SPACING",
                        format!(
                            "\"{} {}\" should be written as one word: \"{}\"",
                            first.text, second.text, compound
                        ),
                        vec![compound.to_string()],
                    ));
                    break;
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

    // --- Positive detections ---

    #[test]
    fn detects_jaa_kaappi() {
        let rule = CompoundSpacingRule::new();
        let tokens = vec![word("jää", 0, 4), word("kaappi", 5, 11)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "COMPOUND_SPACING");
        assert_eq!(errors[0].suggestions, vec!["jääkaappi"]);
    }

    #[test]
    fn detects_kahvi_kuppi() {
        let rule = CompoundSpacingRule::new();
        let tokens = vec![word("kahvi", 0, 5), word("kuppi", 6, 11)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].suggestions, vec!["kahvikuppi"]);
    }

    #[test]
    fn detects_tieto_kone() {
        let rule = CompoundSpacingRule::new();
        let tokens = vec![word("tieto", 0, 5), word("kone", 6, 10)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].suggestions, vec!["tietokone"]);
    }

    #[test]
    fn case_insensitive_detection() {
        let rule = CompoundSpacingRule::new();
        let tokens = vec![word("Jää", 0, 4), word("kaappi", 5, 11)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    // --- No false positives ---

    #[test]
    fn no_error_for_correct_compound() {
        let rule = CompoundSpacingRule::new();
        // "jääkaappi" as a single word — no split to detect.
        let tokens = vec![word("jääkaappi", 0, 10)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_unrelated_words() {
        let rule = CompoundSpacingRule::new();
        let tokens = vec![word("koira", 0, 5), word("juoksee", 6, 13)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = CompoundSpacingRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_single_word() {
        let rule = CompoundSpacingRule::new();
        let tokens = vec![word("jää", 0, 4)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = CompoundSpacingRule::new();
        assert_eq!(rule.id(), "COMPOUND_SPACING");
    }

    #[test]
    fn default_trait() {
        let rule = CompoundSpacingRule::default();
        assert_eq!(rule.id(), "COMPOUND_SPACING");
    }
}
