//! MCE Grammar — Finnish grammar checking.
//!
//! Detects grammar errors in Finnish text by combining tokenization,
//! morphological analysis, and disambiguation with a set of modular
//! grammar rules.
//!
//! # Architecture
//!
//! The grammar checker pipeline:
//! 1. Tokenize text into word tokens (preserving byte offsets)
//! 2. Analyze each word with [`FinnishAnalyzer`](mce_fi::morphology::FinnishAnalyzer)
//! 3. Disambiguate using [`ViterbiDisambiguator`](mce_disambig::ViterbiDisambiguator)
//! 4. Run each [`GrammarRule`] against the annotated token stream
//! 5. Collect and return all [`GrammarError`]s
//!
//! # Rules
//!
//! - [`rules::repeated_word`]: Consecutive identical words ("koira koira")
//! - [`rules::capitalization`]: Sentence-initial capitalization and proper noun casing
//! - [`rules::agreement`]: Subject-verb number agreement
//! - [`rules::double_space`]: Multiple consecutive spaces between words
//! - [`rules::quotation_mark`]: Unmatched quotation marks (straight, typographic, guillemets)
//! - [`rules::comma_before_conjunction`]: Missing comma before subordinating conjunctions
//! - [`rules::compound_spacing`]: Compound words incorrectly split with a space
//! - [`rules::number_agreement`]: Numeral-noun case/number agreement
//! - [`rules::negation_agreement`]: Negation verb person/number agreement with pronouns
//! - [`rules::double_negation`]: Non-standard double negation detection
//! - [`rules::missing_space_after_punctuation`]: No space after comma/period/semicolon
//! - [`rules::extra_space_before_punctuation`]: Space before comma/period (error in Finnish)
//! - [`rules::partitive_object`]: Negated sentences require partitive object
//! - [`rules::subject_verb_agreement`]: Subject-verb person agreement via pronouns
//! - [`rules::postposition_case`]: Postpositions require specific cases (genitive)
//! - [`rules::comparative_partitive`]: Comparative + partitive without "kuin"
//! - [`rules::missing_main_verb`]: Sentence without a finite verb (heuristic)
//! - [`rules::sentence_initial_lowercase`]: Lowercase after colon/semicolon
//! - [`rules::excessive_exclamation`]: Multiple exclamation/question marks (!!!, ???)
//! - [`rules::comma_in_subordinate`]: Missing comma before relative pronouns (joka, etc.)
//! - [`rules::possessive_suffix`]: Redundant possessive pronoun + suffix
//!
//! # Example
//!
//! ```
//! use mce_grammar::{GrammarChecker, GrammarError};
//! use mce_grammar::rules::{RepeatedWordRule, CapitalizationRule, AgreementRule};
//! use mce_grammar::finnish::FinnishGrammarChecker;
//!
//! // Without a VFST dictionary, use the lightweight checker with
//! // only rules that don't require morphological analysis.
//! let rules: Vec<Box<dyn mce_grammar::GrammarRule>> = vec![
//!     Box::new(RepeatedWordRule::new()),
//! ];
//! let errors = rules[0].check(&[
//!     mce_grammar::AnnotatedToken::word("koira", 0, 5, None),
//!     mce_grammar::AnnotatedToken::word("koira", 6, 11, None),
//! ]);
//! assert_eq!(errors.len(), 1);
//! assert_eq!(errors[0].code, "REPEATED_WORD");
//! ```

pub mod error;
pub mod finnish;
pub mod rules;

pub use error::InitError;

use mce_core::analysis::Analysis;

/// A grammar error detected in text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrammarError {
    /// Byte offset of the error start in the original text.
    pub start: usize,
    /// Byte offset of the error end.
    pub end: usize,
    /// Error code (e.g., "AGREEMENT_ERROR", "REPEATED_WORD").
    pub code: &'static str,
    /// Human-readable description.
    pub message: String,
    /// Suggested corrections (may be empty).
    pub suggestions: Vec<String>,
}

impl GrammarError {
    /// Create a new grammar error.
    pub fn new(start: usize, end: usize, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            start,
            end,
            code,
            message: message.into(),
            suggestions: Vec::new(),
        }
    }

    /// Create a grammar error with suggestions.
    pub fn with_suggestions(
        start: usize,
        end: usize,
        code: &'static str,
        message: impl Into<String>,
        suggestions: Vec<String>,
    ) -> Self {
        Self {
            start,
            end,
            code,
            message: message.into(),
            suggestions,
        }
    }
}

/// A token annotated with its position and optional morphological analysis.
///
/// This is the common unit passed to grammar rules. It carries the surface
/// form, byte offsets in the original text, and (when available) the
/// disambiguated morphological analysis.
#[derive(Debug, Clone)]
pub struct AnnotatedToken {
    /// The surface form of the token.
    pub text: String,
    /// Byte offset of the token start in the original text.
    pub start: usize,
    /// Byte offset of the token end in the original text.
    pub end: usize,
    /// Whether this token is a word (as opposed to punctuation/whitespace).
    pub is_word: bool,
    /// Disambiguated morphological analysis, if available.
    pub analysis: Option<Analysis>,
}

impl AnnotatedToken {
    /// Create a word token with optional analysis.
    pub fn word(
        text: impl Into<String>,
        start: usize,
        end: usize,
        analysis: Option<Analysis>,
    ) -> Self {
        Self {
            text: text.into(),
            start,
            end,
            is_word: true,
            analysis,
        }
    }

    /// Create a non-word token (punctuation, whitespace).
    pub fn non_word(text: impl Into<String>, start: usize, end: usize) -> Self {
        Self {
            text: text.into(),
            start,
            end,
            is_word: false,
            analysis: None,
        }
    }
}

/// Trait for grammar checkers that operate on raw text.
pub trait GrammarChecker {
    /// Check text for grammar errors.
    fn check(&self, text: &str) -> Vec<GrammarError>;
}

/// Trait for individual grammar rules operating on annotated tokens.
///
/// Each rule inspects the sequence of annotated tokens and returns
/// any errors it detects. Rules are designed to be independent and
/// composable.
pub trait GrammarRule: Send + Sync {
    /// A short identifier for this rule (e.g., "REPEATED_WORD").
    fn id(&self) -> &'static str;

    /// Check a sequence of annotated tokens for grammar errors.
    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_error_new() {
        let err = GrammarError::new(0, 5, "TEST_ERROR", "test message");
        assert_eq!(err.start, 0);
        assert_eq!(err.end, 5);
        assert_eq!(err.code, "TEST_ERROR");
        assert_eq!(err.message, "test message");
        assert!(err.suggestions.is_empty());
    }

    #[test]
    fn grammar_error_with_suggestions() {
        let err = GrammarError::with_suggestions(
            0,
            5,
            "TEST_ERROR",
            "test message",
            vec!["fix1".to_string(), "fix2".to_string()],
        );
        assert_eq!(err.suggestions.len(), 2);
        assert_eq!(err.suggestions[0], "fix1");
    }

    #[test]
    fn annotated_token_word() {
        let tok = AnnotatedToken::word("koira", 0, 5, None);
        assert!(tok.is_word);
        assert_eq!(tok.text, "koira");
        assert_eq!(tok.start, 0);
        assert_eq!(tok.end, 5);
        assert!(tok.analysis.is_none());
    }

    #[test]
    fn annotated_token_word_with_analysis() {
        let mut a = Analysis::new();
        a.set("CLASS", "nimisana");
        let tok = AnnotatedToken::word("koira", 0, 5, Some(a));
        assert!(tok.is_word);
        assert!(tok.analysis.is_some());
        assert_eq!(
            tok.analysis.as_ref().unwrap().get("CLASS"),
            Some("nimisana")
        );
    }

    #[test]
    fn annotated_token_non_word() {
        let tok = AnnotatedToken::non_word(".", 5, 6);
        assert!(!tok.is_word);
        assert_eq!(tok.text, ".");
        assert!(tok.analysis.is_none());
    }

    #[test]
    fn grammar_error_equality() {
        let e1 = GrammarError::new(0, 5, "ERR", "msg");
        let e2 = GrammarError::new(0, 5, "ERR", "msg");
        assert_eq!(e1, e2);
    }

    #[test]
    fn grammar_error_clone() {
        let e1 = GrammarError::with_suggestions(0, 5, "ERR", "msg", vec!["a".into()]);
        let e2 = e1.clone();
        assert_eq!(e1, e2);
    }
}
