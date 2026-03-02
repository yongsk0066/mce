//! Finnish grammar checker combining tokenization, analysis, disambiguation,
//! and grammar rules.
//!
//! [`FinnishGrammarChecker`] is the main entry point for checking Finnish text.
//! It orchestrates the full pipeline:
//!
//! 1. Tokenize the input text into tokens with byte offsets
//! 2. Analyze each word token with `FinnishAnalyzer`
//! 3. Disambiguate using `ViterbiDisambiguator`
//! 4. Run all registered grammar rules on the annotated tokens
//!
//! # Constructing without a dictionary
//!
//! For testing or lightweight checks (repeated words, capitalization without
//! proper noun detection), use [`FinnishGrammarChecker::without_analyzer`],
//! which skips morphological analysis.

use mce_core::token::TokenType;
use mce_disambig::ViterbiDisambiguator;
use mce_fi::morphology::{Analyzer, FinnishAnalyzer};
use mce_tokenizer::next_token;

use crate::rules::{
    AgreementRule, CapitalizationRule, CommaBeforeConjunctionRule, CommaInSubordinateRule,
    ComparativePartitiveRule, CompoundSpacingRule, DoubleNegationRule, DoubleSpaceRule,
    ExcessiveExclamationRule, ExtraSpaceBeforePunctuationRule, MissingMainVerbRule,
    MissingSpaceAfterPunctuationRule, NegationAgreementRule, NumberAgreementRule,
    PartitiveObjectRule, PossessiveSuffixRule, PostpositionCaseRule, QuotationMarkRule,
    RepeatedWordRule, SentenceInitialLowercaseRule, SubjectVerbAgreementRule,
};
use crate::{AnnotatedToken, GrammarChecker, GrammarError, GrammarRule};

/// Finnish grammar checker combining morphological analysis with grammar rules.
///
/// # Example
///
/// ```no_run
/// use mce_grammar::finnish::FinnishGrammarChecker;
/// use mce_grammar::GrammarChecker;
///
/// // Load VFST dictionary bytes (e.g., from a file)
/// let dict_bytes: &[u8] = &[]; // placeholder
/// // let checker = FinnishGrammarChecker::new(dict_bytes).unwrap();
/// // let errors = checker.check("koira koira juoksee.");
/// ```
pub struct FinnishGrammarChecker {
    analyzer: Option<FinnishAnalyzer>,
    disambiguator: ViterbiDisambiguator,
    rules: Vec<Box<dyn GrammarRule>>,
}

impl FinnishGrammarChecker {
    /// Create a grammar checker with a VFST dictionary for full analysis.
    ///
    /// Loads the `FinnishAnalyzer` from raw VFST bytes and registers all
    /// default grammar rules (repeated word, capitalization, agreement).
    ///
    /// # Errors
    ///
    /// Returns an error if the VFST data is malformed.
    pub fn new(mor_vfst: &[u8]) -> Result<Self, mce_fst::VfstError> {
        let analyzer = FinnishAnalyzer::from_bytes(mor_vfst)?;
        Ok(Self {
            analyzer: Some(analyzer),
            disambiguator: ViterbiDisambiguator::with_finnish_defaults_and_emission(),
            rules: default_rules(),
        })
    }

    /// Create a grammar checker without a morphological analyzer.
    ///
    /// Only rules that do not require analysis will produce meaningful
    /// results (e.g., `RepeatedWordRule`, basic `CapitalizationRule`).
    /// The agreement rule will be registered but will not fire because
    /// tokens will lack analysis attributes.
    pub fn without_analyzer() -> Self {
        Self {
            analyzer: None,
            disambiguator: ViterbiDisambiguator::with_finnish_defaults(),
            rules: default_rules(),
        }
    }

    /// Create a grammar checker with custom rules.
    pub fn with_rules(
        mor_vfst: &[u8],
        rules: Vec<Box<dyn GrammarRule>>,
    ) -> Result<Self, mce_fst::VfstError> {
        let analyzer = FinnishAnalyzer::from_bytes(mor_vfst)?;
        Ok(Self {
            analyzer: Some(analyzer),
            disambiguator: ViterbiDisambiguator::with_finnish_defaults_and_emission(),
            rules,
        })
    }

    /// Create a grammar checker without analyzer but with custom rules.
    pub fn without_analyzer_with_rules(rules: Vec<Box<dyn GrammarRule>>) -> Self {
        Self {
            analyzer: None,
            disambiguator: ViterbiDisambiguator::with_finnish_defaults(),
            rules,
        }
    }

    /// Tokenize text into annotated tokens with byte offsets.
    ///
    /// Each token carries its surface form, byte offset range, and whether
    /// it is a word token. Morphological analysis is not yet attached.
    fn tokenize_with_offsets(text: &str) -> Vec<AnnotatedToken> {
        let chars: Vec<char> = text.chars().collect();
        let text_len = chars.len();
        let mut tokens = Vec::new();
        let mut char_pos = 0;

        while char_pos < text_len {
            let (token_type, token_len) = next_token(&chars, text_len, char_pos);

            if token_len == 0 {
                break;
            }

            let token_text: String = chars[char_pos..char_pos + token_len].iter().collect();

            // Compute byte offset from char position.
            let byte_start = char_offset_to_byte(text, &chars, char_pos);
            let byte_end = char_offset_to_byte(text, &chars, char_pos + token_len);

            let is_word = token_type == TokenType::Word;
            tokens.push(AnnotatedToken {
                text: token_text,
                start: byte_start,
                end: byte_end,
                is_word,
                analysis: None,
            });

            char_pos += token_len;
        }

        tokens
    }

    /// Run the analysis + disambiguation pipeline and attach results to tokens.
    fn analyze_tokens(&self, tokens: &mut [AnnotatedToken]) {
        let analyzer = match &self.analyzer {
            Some(a) => a,
            None => return,
        };

        // Collect word tokens' indices and their analyses.
        let word_indices: Vec<usize> = tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_word)
            .map(|(i, _)| i)
            .collect();

        if word_indices.is_empty() {
            return;
        }

        // Analyze each word.
        let word_analyses: Vec<Vec<mce_core::analysis::Analysis>> = word_indices
            .iter()
            .map(|&i| {
                let chars: Vec<char> = tokens[i].text.chars().collect();
                let word_len = chars.len();
                analyzer.analyze(&chars, word_len)
            })
            .collect();

        // Disambiguate.
        let word_refs: Vec<&str> = word_indices
            .iter()
            .map(|&i| tokens[i].text.as_str())
            .collect();
        let disambiguated = self
            .disambiguator
            .disambiguate_with_words(&word_refs, &word_analyses);

        // Attach disambiguated analysis to each word token.
        for (slot, &token_idx) in word_indices.iter().enumerate() {
            if slot < disambiguated.len() {
                tokens[token_idx].analysis = Some(disambiguated[slot].clone());
            }
        }
    }
}

impl GrammarChecker for FinnishGrammarChecker {
    fn check(&self, text: &str) -> Vec<GrammarError> {
        if text.is_empty() {
            return Vec::new();
        }

        // Step 1: Tokenize with byte offsets.
        let mut tokens = Self::tokenize_with_offsets(text);

        // Step 2-3: Analyze and disambiguate.
        self.analyze_tokens(&mut tokens);

        // Step 4: Run all rules.
        let mut errors = Vec::new();
        for rule in &self.rules {
            errors.extend(rule.check(&tokens));
        }

        // Sort errors by position for deterministic output.
        errors.sort_by_key(|e| (e.start, e.end));
        errors
    }
}

/// Build the default set of Finnish grammar rules.
fn default_rules() -> Vec<Box<dyn GrammarRule>> {
    vec![
        // --- Original 10 rules ---
        Box::new(RepeatedWordRule::new()),
        Box::new(CapitalizationRule::new()),
        Box::new(AgreementRule::new()),
        Box::new(DoubleSpaceRule::new()),
        Box::new(QuotationMarkRule::new()),
        Box::new(CommaBeforeConjunctionRule::new()),
        Box::new(CompoundSpacingRule::new()),
        Box::new(NumberAgreementRule::new()),
        Box::new(NegationAgreementRule::new()),
        Box::new(DoubleNegationRule::new()),
        // --- New 11 rules ---
        Box::new(MissingSpaceAfterPunctuationRule::new()),
        Box::new(ExtraSpaceBeforePunctuationRule::new()),
        Box::new(PartitiveObjectRule::new()),
        Box::new(SubjectVerbAgreementRule::new()),
        Box::new(PostpositionCaseRule::new()),
        Box::new(ComparativePartitiveRule::new()),
        Box::new(MissingMainVerbRule::new()),
        Box::new(SentenceInitialLowercaseRule::new()),
        Box::new(ExcessiveExclamationRule::new()),
        Box::new(CommaInSubordinateRule::new()),
        Box::new(PossessiveSuffixRule::new()),
    ]
}

/// Convert a character offset to a byte offset in the original string.
///
/// This handles multi-byte UTF-8 characters correctly.
fn char_offset_to_byte(text: &str, chars: &[char], char_offset: usize) -> usize {
    // Sum the byte lengths of all characters before char_offset.
    chars[..char_offset]
        .iter()
        .map(|c| c.len_utf8())
        .sum::<usize>()
        .min(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Tokenization with offsets
    // -----------------------------------------------------------------------

    #[test]
    fn tokenize_simple_sentence() {
        let tokens = FinnishGrammarChecker::tokenize_with_offsets("Koira juoksee.");
        let words: Vec<&str> = tokens
            .iter()
            .filter(|t| t.is_word)
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(words, vec!["Koira", "juoksee"]);
    }

    #[test]
    fn tokenize_byte_offsets_ascii() {
        let tokens = FinnishGrammarChecker::tokenize_with_offsets("Koira juoksee.");
        // "Koira" = bytes 0..5
        let koira = &tokens[0];
        assert_eq!(koira.text, "Koira");
        assert_eq!(koira.start, 0);
        assert_eq!(koira.end, 5);
        assert!(koira.is_word);
    }

    #[test]
    fn tokenize_byte_offsets_unicode() {
        // "Äiti" starts at byte 0 (Ä is 2 bytes in UTF-8).
        let tokens = FinnishGrammarChecker::tokenize_with_offsets("\u{00C4}iti puhuu.");
        let first = &tokens[0];
        assert_eq!(first.text, "\u{00C4}iti");
        assert_eq!(first.start, 0);
        // Ä = 2 bytes, i = 1, t = 1, i = 1 → end = 5
        assert_eq!(first.end, 5);
    }

    #[test]
    fn tokenize_empty() {
        let tokens = FinnishGrammarChecker::tokenize_with_offsets("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn tokenize_preserves_punctuation_tokens() {
        let tokens = FinnishGrammarChecker::tokenize_with_offsets("Hei! Moi.");
        let non_words: Vec<&str> = tokens
            .iter()
            .filter(|t| !t.is_word)
            .map(|t| t.text.as_str())
            .collect();
        // Should include at least "!" and "." as non-word tokens.
        assert!(non_words.contains(&"!"));
        assert!(non_words.contains(&"."));
    }

    // -----------------------------------------------------------------------
    // Without analyzer — only lightweight rules fire
    // -----------------------------------------------------------------------

    #[test]
    fn without_analyzer_detects_repeated_word() {
        let checker = FinnishGrammarChecker::without_analyzer();
        let errors = checker.check("Koira koira koira juoksee.");
        // "koira" repeated: 2nd and 3rd occurrence detected.
        let repeated: Vec<&GrammarError> = errors
            .iter()
            .filter(|e| e.code == "REPEATED_WORD")
            .collect();
        assert_eq!(repeated.len(), 2);
    }

    #[test]
    fn without_analyzer_detects_capitalization() {
        let checker = FinnishGrammarChecker::without_analyzer();
        let errors = checker.check("koira juoksee.");
        // First word "koira" should be capitalized.
        let cap_errors: Vec<&GrammarError> = errors
            .iter()
            .filter(|e| e.code == "CAPITALIZATION_ERROR")
            .collect();
        assert_eq!(cap_errors.len(), 1);
        assert_eq!(cap_errors[0].suggestions, vec!["Koira"]);
    }

    #[test]
    fn without_analyzer_capitalization_after_period() {
        let checker = FinnishGrammarChecker::without_analyzer();
        let errors = checker.check("Koira juoksee. kissa nukkuu.");
        let cap_errors: Vec<&GrammarError> = errors
            .iter()
            .filter(|e| e.code == "CAPITALIZATION_ERROR")
            .collect();
        assert_eq!(cap_errors.len(), 1);
        assert!(cap_errors[0].message.contains("kissa"));
    }

    #[test]
    fn without_analyzer_no_agreement_errors() {
        // Without analysis, agreement rule cannot fire.
        let checker = FinnishGrammarChecker::without_analyzer();
        let errors = checker.check("Koira juoksevat.");
        let agreement_errors: Vec<&GrammarError> = errors
            .iter()
            .filter(|e| e.code == "AGREEMENT_ERROR")
            .collect();
        assert!(agreement_errors.is_empty());
    }

    #[test]
    fn empty_text() {
        let checker = FinnishGrammarChecker::without_analyzer();
        let errors = checker.check("");
        assert!(errors.is_empty());
    }

    #[test]
    fn whitespace_only() {
        let checker = FinnishGrammarChecker::without_analyzer();
        let errors = checker.check("   ");
        // DoubleSpaceRule fires on 3 consecutive spaces.
        let double_space_errors: Vec<&GrammarError> =
            errors.iter().filter(|e| e.code == "DOUBLE_SPACE").collect();
        assert_eq!(double_space_errors.len(), 1);
        // No other error types should fire.
        let other_errors: Vec<&GrammarError> =
            errors.iter().filter(|e| e.code != "DOUBLE_SPACE").collect();
        assert!(other_errors.is_empty());
    }

    #[test]
    fn correct_simple_sentence() {
        let checker = FinnishGrammarChecker::without_analyzer();
        let errors = checker.check("Koira juoksee.");
        assert!(errors.is_empty());
    }

    #[test]
    fn multiple_errors_sorted() {
        let checker = FinnishGrammarChecker::without_analyzer();
        // lowercase start + repeated word
        let errors = checker.check("koira koira juoksee.");
        assert!(errors.len() >= 2);
        // Errors should be sorted by position.
        for i in 1..errors.len() {
            assert!(errors[i].start >= errors[i - 1].start);
        }
    }

    // -----------------------------------------------------------------------
    // Custom rules
    // -----------------------------------------------------------------------

    #[test]
    fn custom_rules_only_repeated_word() {
        let rules: Vec<Box<dyn GrammarRule>> = vec![Box::new(RepeatedWordRule::new())];
        let checker = FinnishGrammarChecker::without_analyzer_with_rules(rules);
        // Lowercase start should NOT trigger because capitalization rule is not registered.
        let errors = checker.check("koira koira juoksee.");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "REPEATED_WORD");
    }

    // -----------------------------------------------------------------------
    // char_offset_to_byte helper
    // -----------------------------------------------------------------------

    #[test]
    fn char_offset_to_byte_ascii() {
        let text = "hello";
        let chars: Vec<char> = text.chars().collect();
        assert_eq!(char_offset_to_byte(text, &chars, 0), 0);
        assert_eq!(char_offset_to_byte(text, &chars, 3), 3);
        assert_eq!(char_offset_to_byte(text, &chars, 5), 5);
    }

    #[test]
    fn char_offset_to_byte_unicode() {
        let text = "\u{00C4}iti"; // "Äiti" — Ä is 2 bytes
        let chars: Vec<char> = text.chars().collect();
        assert_eq!(char_offset_to_byte(text, &chars, 0), 0); // before Ä
        assert_eq!(char_offset_to_byte(text, &chars, 1), 2); // after Ä (2 bytes)
        assert_eq!(char_offset_to_byte(text, &chars, 2), 3); // after i
        assert_eq!(char_offset_to_byte(text, &chars, 4), 5); // end
    }

    // -----------------------------------------------------------------------
    // default_rules
    // -----------------------------------------------------------------------

    #[test]
    fn default_rules_has_twenty_one_rules() {
        let rules = default_rules();
        assert_eq!(rules.len(), 21);
        let ids: Vec<&str> = rules.iter().map(|r| r.id()).collect();
        // Original 10
        assert!(ids.contains(&"REPEATED_WORD"));
        assert!(ids.contains(&"CAPITALIZATION_ERROR"));
        assert!(ids.contains(&"AGREEMENT_ERROR"));
        assert!(ids.contains(&"DOUBLE_SPACE"));
        assert!(ids.contains(&"QUOTATION_MARK_ERROR"));
        assert!(ids.contains(&"COMMA_BEFORE_CONJUNCTION"));
        assert!(ids.contains(&"COMPOUND_SPACING"));
        assert!(ids.contains(&"NUMBER_AGREEMENT"));
        assert!(ids.contains(&"NEGATION_AGREEMENT"));
        assert!(ids.contains(&"DOUBLE_NEGATION"));
        // New 11
        assert!(ids.contains(&"MISSING_SPACE_AFTER_PUNCT"));
        assert!(ids.contains(&"EXTRA_SPACE_BEFORE_PUNCT"));
        assert!(ids.contains(&"PARTITIVE_OBJECT"));
        assert!(ids.contains(&"SUBJECT_VERB_PERSON"));
        assert!(ids.contains(&"POSTPOSITION_CASE"));
        assert!(ids.contains(&"COMPARATIVE_PARTITIVE"));
        assert!(ids.contains(&"MISSING_MAIN_VERB"));
        assert!(ids.contains(&"SENTENCE_INITIAL_LOWERCASE"));
        assert!(ids.contains(&"EXCESSIVE_EXCLAMATION"));
        assert!(ids.contains(&"COMMA_BEFORE_RELATIVE"));
        assert!(ids.contains(&"POSSESSIVE_REDUNDANCY"));
    }
}
