//! Subject-verb number agreement checking.
//!
//! In Finnish, the subject and verb must agree in number:
//! - Singular subject requires singular verb: "koira juoksee" (dog runs)
//! - Plural subject requires plural verb: "koirat juoksevat" (dogs run)
//!
//! This rule checks adjacent noun-verb pairs for number mismatch. It
//! requires morphological analysis with at least CLASS and NUMBER
//! attributes present on the tokens.
//!
//! # Limitations
//!
//! - Only checks adjacent noun-verb pairs (does not handle non-adjacent
//!   subjects and verbs separated by modifiers)
//! - Does not handle inverted word order (verb-subject)
//! - Does not handle coordinated subjects ("koira ja kissa juoksevat")
//! - Requires disambiguation to have correctly identified NUMBER

use mce_core::analysis::{ATTR_CLASS, ATTR_NUMBER};

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Finnish word classes that can be subjects (nouns).
const SUBJECT_CLASSES: &[&str] = &[
    "nimisana",   // common noun
    "etunimi",    // first name
    "sukunimi",   // surname
    "paikannimi", // place name
    "nimi",       // name (organization)
    "asemosana",  // pronoun (some)
];

/// Finnish word classes that are verbs.
const VERB_CLASSES: &[&str] = &[
    "teonsana",   // verb
    "kieltosana", // negation verb (ei)
];

/// Detects subject-verb number agreement errors.
///
/// Scans adjacent word pairs looking for a noun/pronoun followed by a verb
/// (or vice versa) where the NUMBER attribute disagrees.
///
/// # Error code
///
/// `AGREEMENT_ERROR`
///
/// # Example
///
/// ```
/// use mce_core::analysis::Analysis;
/// use mce_grammar::{AnnotatedToken, GrammarRule};
/// use mce_grammar::rules::AgreementRule;
///
/// let rule = AgreementRule::new();
///
/// let mut noun = Analysis::new();
/// noun.set("CLASS", "nimisana");
/// noun.set("NUMBER", "singular");
///
/// let mut verb = Analysis::new();
/// verb.set("CLASS", "teonsana");
/// verb.set("NUMBER", "plural");
///
/// let tokens = vec![
///     AnnotatedToken::word("koira", 0, 5, Some(noun)),
///     AnnotatedToken::word("juoksevat", 6, 15, Some(verb)),
/// ];
/// let errors = rule.check(&tokens);
/// assert_eq!(errors.len(), 1);
/// assert_eq!(errors[0].code, "AGREEMENT_ERROR");
/// ```
pub struct AgreementRule;

impl AgreementRule {
    /// Create a new agreement rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for AgreementRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the word class from an annotated token, if available.
fn get_class(token: &AnnotatedToken) -> Option<&str> {
    token.analysis.as_ref()?.get(ATTR_CLASS)
}

/// Extract the number (singular/plural) from an annotated token, if available.
fn get_number(token: &AnnotatedToken) -> Option<&str> {
    token.analysis.as_ref()?.get(ATTR_NUMBER)
}

/// Check if a class is a subject-like class.
fn is_subject_class(class: &str) -> bool {
    SUBJECT_CLASSES.contains(&class)
}

/// Check if a class is a verb class.
fn is_verb_class(class: &str) -> bool {
    VERB_CLASSES.contains(&class)
}

impl GrammarRule for AgreementRule {
    fn id(&self) -> &'static str {
        "AGREEMENT_ERROR"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();

        let word_tokens: Vec<&AnnotatedToken> = tokens.iter().filter(|t| t.is_word).collect();

        for window in word_tokens.windows(2) {
            let first = window[0];
            let second = window[1];

            let first_class = match get_class(first) {
                Some(c) => c,
                None => continue,
            };
            let second_class = match get_class(second) {
                Some(c) => c,
                None => continue,
            };

            // Pattern 1: subject (noun) followed by verb
            if is_subject_class(first_class) && is_verb_class(second_class) {
                if let (Some(subj_num), Some(verb_num)) = (get_number(first), get_number(second)) {
                    if subj_num != verb_num {
                        errors.push(GrammarError::new(
                            first.start,
                            second.end,
                            "AGREEMENT_ERROR",
                            format!(
                                "Subject-verb number mismatch: \"{}\" ({}) + \"{}\" ({})",
                                first.text, subj_num, second.text, verb_num
                            ),
                        ));
                    }
                }
            }

            // Pattern 2: verb followed by subject (inverted order, less common
            // but possible in Finnish: "Juoksee koira")
            if is_verb_class(first_class) && is_subject_class(second_class) {
                if let (Some(verb_num), Some(subj_num)) = (get_number(first), get_number(second)) {
                    if verb_num != subj_num {
                        errors.push(GrammarError::new(
                            first.start,
                            second.end,
                            "AGREEMENT_ERROR",
                            format!(
                                "Subject-verb number mismatch: \"{}\" ({}) + \"{}\" ({})",
                                first.text, verb_num, second.text, subj_num
                            ),
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

    fn make_token(
        text: &str,
        start: usize,
        end: usize,
        class: &str,
        number: &str,
    ) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, class);
        a.set(ATTR_NUMBER, number);
        AnnotatedToken::word(text, start, end, Some(a))
    }

    fn make_token_no_number(text: &str, start: usize, end: usize, class: &str) -> AnnotatedToken {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, class);
        AnnotatedToken::word(text, start, end, Some(a))
    }

    fn word_no_analysis(text: &str, start: usize, end: usize) -> AnnotatedToken {
        AnnotatedToken::word(text, start, end, None)
    }

    // -----------------------------------------------------------------------
    // Subject-Verb agreement (noun + verb)
    // -----------------------------------------------------------------------

    #[test]
    fn correct_singular_agreement() {
        let rule = AgreementRule::new();
        let tokens = vec![
            make_token("koira", 0, 5, "nimisana", "singular"),
            make_token("juoksee", 6, 13, "teonsana", "singular"),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn correct_plural_agreement() {
        let rule = AgreementRule::new();
        let tokens = vec![
            make_token("koirat", 0, 6, "nimisana", "plural"),
            make_token("juoksevat", 7, 16, "teonsana", "plural"),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn singular_noun_plural_verb_error() {
        let rule = AgreementRule::new();
        let tokens = vec![
            make_token("koira", 0, 5, "nimisana", "singular"),
            make_token("juoksevat", 6, 15, "teonsana", "plural"),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "AGREEMENT_ERROR");
        assert_eq!(errors[0].start, 0);
        assert_eq!(errors[0].end, 15);
        assert!(errors[0].message.contains("singular"));
        assert!(errors[0].message.contains("plural"));
    }

    #[test]
    fn plural_noun_singular_verb_error() {
        let rule = AgreementRule::new();
        let tokens = vec![
            make_token("koirat", 0, 6, "nimisana", "plural"),
            make_token("juoksee", 7, 14, "teonsana", "singular"),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn inverted_order_verb_subject_error() {
        let rule = AgreementRule::new();
        let tokens = vec![
            make_token("juoksee", 0, 7, "teonsana", "singular"),
            make_token("koirat", 8, 14, "nimisana", "plural"),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn inverted_order_correct() {
        let rule = AgreementRule::new();
        let tokens = vec![
            make_token("juoksevat", 0, 9, "teonsana", "plural"),
            make_token("koirat", 10, 16, "nimisana", "plural"),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn no_error_for_empty_input() {
        let rule = AgreementRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_single_word() {
        let rule = AgreementRule::new();
        let tokens = vec![make_token("koira", 0, 5, "nimisana", "singular")];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_without_analysis() {
        let rule = AgreementRule::new();
        let tokens = vec![
            word_no_analysis("koira", 0, 5),
            word_no_analysis("juoksevat", 6, 15),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_without_number_attribute() {
        let rule = AgreementRule::new();
        let tokens = vec![
            make_token_no_number("koira", 0, 5, "nimisana"),
            make_token_no_number("juoksee", 6, 13, "teonsana"),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_non_subject_verb_pair() {
        // adjective + noun — not a subject-verb pair
        let rule = AgreementRule::new();
        let tokens = vec![
            make_token("iso", 0, 3, "laatusana", "singular"),
            make_token("koirat", 4, 10, "nimisana", "plural"),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn proper_noun_as_subject() {
        let rule = AgreementRule::new();
        let tokens = vec![
            make_token("Matti", 0, 5, "etunimi", "singular"),
            make_token("juoksevat", 6, 15, "teonsana", "plural"),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn negation_verb_agreement() {
        let rule = AgreementRule::new();
        // "koirat ei" — singular negation with plural subject
        let tokens = vec![
            make_token("koirat", 0, 6, "nimisana", "plural"),
            make_token("ei", 7, 9, "kieltosana", "singular"),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn punctuation_between_noun_and_verb_skipped() {
        // Punctuation tokens should be filtered; we check word adjacency.
        let rule = AgreementRule::new();
        let tokens = vec![
            make_token("koira", 0, 5, "nimisana", "singular"),
            AnnotatedToken::non_word(",", 5, 6),
            make_token("juoksevat", 7, 16, "teonsana", "plural"),
        ];
        let errors = rule.check(&tokens);
        // Even with comma between, adjacent word tokens are checked.
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn multiple_agreement_errors() {
        let rule = AgreementRule::new();
        let tokens = vec![
            make_token("koira", 0, 5, "nimisana", "singular"),
            make_token("juoksevat", 6, 15, "teonsana", "plural"),
            make_token("kissat", 16, 22, "nimisana", "plural"),
            make_token("nukkuu", 23, 29, "teonsana", "singular"),
        ];
        let errors = rule.check(&tokens);
        // Error 1: koira (sg) + juoksevat (pl)
        // juoksevat (pl,verb) + kissat (pl,noun) — inverted order check, both plural — ok
        // Error 2: kissat (pl) + nukkuu (sg)
        assert_eq!(errors.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Rule metadata
    // -----------------------------------------------------------------------

    #[test]
    fn rule_id() {
        let rule = AgreementRule::new();
        assert_eq!(rule.id(), "AGREEMENT_ERROR");
    }

    #[test]
    fn default_trait() {
        let rule = AgreementRule::default();
        assert_eq!(rule.id(), "AGREEMENT_ERROR");
    }
}
