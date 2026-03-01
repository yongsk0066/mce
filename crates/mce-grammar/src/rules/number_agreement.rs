//! Number-noun agreement checking.
//!
//! In Finnish, cardinal numbers 2 and above require the following noun
//! to be in singular partitive case:
//! - "kaksi autoa" (two cars) — correct (singular partitive)
//! - "kaksi autot" (two cars-nominative-plural) — incorrect
//! - "kolme kissaa" (three cats) — correct
//!
//! The number "yksi" (one) requires nominative singular, which is the
//! default and not checked here.
//!
//! This rule checks for numeral words followed by nouns that are not
//! in partitive case.

use mce_core::analysis::{ATTR_CLASS, ATTR_NUMBER, ATTR_SIJAMUOTO};

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Finnish cardinal numbers (2+) that require partitive.
const NUMERALS_REQUIRING_PARTITIVE: &[&str] = &[
    "kaksi",
    "kolme",
    "neljä",
    "viisi",
    "kuusi",
    "seitsemän",
    "kahdeksan",
    "yhdeksän",
    "kymmenen",
    "yksitoista",
    "kaksitoista",
    "kolmetoista",
    "neljätoista",
    "viisitoista",
    "kaksikymmentä",
    "kolmekymmentä",
    "sata",
    "tuhat",
    "miljoona",
];

/// Noun classes in Finnish that can follow a numeral.
const NOUN_CLASSES: &[&str] = &[
    "nimisana",   // common noun
    "etunimi",    // first name
    "sukunimi",   // surname
    "paikannimi", // place name
    "nimi",       // name
];

/// Detects number-noun agreement errors.
///
/// When a Finnish cardinal numeral (2+) is followed by a noun, the
/// noun should be in singular partitive. This rule flags cases where
/// the noun is plural or in non-partitive case.
///
/// # Error code
///
/// `NUMBER_AGREEMENT`
///
/// # Requires
///
/// Morphological analysis with CLASS, NUMBER, and SIJAMUOTO attributes.
pub struct NumberAgreementRule;

impl NumberAgreementRule {
    /// Create a new number agreement rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for NumberAgreementRule {
    fn default() -> Self {
        Self::new()
    }
}

impl GrammarRule for NumberAgreementRule {
    fn id(&self) -> &'static str {
        "NUMBER_AGREEMENT"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        let word_tokens: Vec<&AnnotatedToken> = tokens.iter().filter(|t| t.is_word).collect();

        for window in word_tokens.windows(2) {
            let numeral = window[0];
            let noun = window[1];

            // Check if first token is a numeral requiring partitive.
            let num_lower = numeral.text.to_lowercase();
            if !NUMERALS_REQUIRING_PARTITIVE.contains(&num_lower.as_str()) {
                continue;
            }

            // Check if second token is a noun.
            let noun_class = match noun.analysis.as_ref().and_then(|a| a.get(ATTR_CLASS)) {
                Some(c) if NOUN_CLASSES.contains(&c) => c,
                _ => continue,
            };
            let _ = noun_class;

            // Check number: should be singular.
            let noun_number = noun.analysis.as_ref().and_then(|a| a.get(ATTR_NUMBER));

            // Check case: should be partitive ("ospitkasija" / "ospitkasija"
            // — in MCE the Voikko attribute value is "ospitkasija" but let's
            // also accept "partitiivi" for flexibility).
            let noun_case = noun.analysis.as_ref().and_then(|a| a.get(ATTR_SIJAMUOTO));

            let is_plural = noun_number == Some("plural");
            let is_not_partitive = match noun_case {
                Some(case) => case != "ospitkasija" && case != "partitiivi",
                None => false, // if no case info, don't flag
            };

            if is_plural || is_not_partitive {
                errors.push(GrammarError::new(
                    numeral.start,
                    noun.end,
                    "NUMBER_AGREEMENT",
                    format!(
                        "After \"{}\" the noun should be in singular partitive: \
                         \"{}\" may need a different form",
                        numeral.text, noun.text
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

    fn make_numeral(text: &str, start: usize, end: usize) -> AnnotatedToken {
        AnnotatedToken::word(text, start, end, None)
    }

    fn make_noun(text: &str, start: usize, end: usize, number: &str, case: &str) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "nimisana");
        a.set(ATTR_NUMBER, number);
        a.set(ATTR_SIJAMUOTO, case);
        AnnotatedToken::word(text, start, end, Some(a))
    }

    fn make_noun_no_analysis(text: &str, start: usize, end: usize) -> AnnotatedToken {
        AnnotatedToken::word(text, start, end, None)
    }

    // --- Positive detections ---

    #[test]
    fn detects_plural_after_kaksi() {
        let rule = NumberAgreementRule::new();
        let tokens = vec![
            make_numeral("kaksi", 0, 5),
            make_noun("autot", 6, 11, "plural", "npitkasija"),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "NUMBER_AGREEMENT");
        assert!(errors[0].message.contains("kaksi"));
    }

    #[test]
    fn detects_nominative_after_kolme() {
        let rule = NumberAgreementRule::new();
        let tokens = vec![
            make_numeral("kolme", 0, 5),
            make_noun("kissa", 6, 11, "singular", "npitkasija"),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn detects_plural_after_viisi() {
        let rule = NumberAgreementRule::new();
        let tokens = vec![
            make_numeral("viisi", 0, 5),
            make_noun("talot", 6, 11, "plural", "npitkasija"),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    // --- No false positives ---

    #[test]
    fn no_error_for_correct_partitive() {
        let rule = NumberAgreementRule::new();
        // "kaksi autoa" — singular partitive, correct.
        let tokens = vec![
            make_numeral("kaksi", 0, 5),
            make_noun("autoa", 6, 11, "singular", "ospitkasija"),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_without_analysis() {
        let rule = NumberAgreementRule::new();
        let tokens = vec![
            make_numeral("kaksi", 0, 5),
            make_noun_no_analysis("autoa", 6, 11),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_non_numeral() {
        let rule = NumberAgreementRule::new();
        let tokens = vec![
            AnnotatedToken::word("iso", 0, 3, None),
            make_noun("autot", 4, 9, "plural", "npitkasija"),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = NumberAgreementRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = NumberAgreementRule::new();
        assert_eq!(rule.id(), "NUMBER_AGREEMENT");
    }

    #[test]
    fn default_trait() {
        let rule = NumberAgreementRule::default();
        assert_eq!(rule.id(), "NUMBER_AGREEMENT");
    }
}
