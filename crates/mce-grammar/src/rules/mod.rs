//! Grammar rules for Finnish text.
//!
//! Each rule implements [`GrammarRule`](crate::GrammarRule) and operates on
//! a sequence of [`AnnotatedToken`](crate::AnnotatedToken)s.
//!
//! # Available Rules
//!
//! | Rule | Code | Description |
//! |------|------|-------------|
//! | [`RepeatedWordRule`] | `REPEATED_WORD` | Consecutive identical words |
//! | [`CapitalizationRule`] | `CAPITALIZATION_ERROR` | Sentence-initial and proper noun capitalization |
//! | [`AgreementRule`] | `AGREEMENT_ERROR` | Subject-verb number agreement |
//! | [`DoubleSpaceRule`] | `DOUBLE_SPACE` | Multiple consecutive spaces |
//! | [`QuotationMarkRule`] | `QUOTATION_MARK_ERROR` | Unmatched quotation marks |
//! | [`CommaBeforeConjunctionRule`] | `COMMA_BEFORE_CONJUNCTION` | Missing comma before subordinating conjunctions |
//! | [`CompoundSpacingRule`] | `COMPOUND_SPACING` | Compound words incorrectly split with a space |
//! | [`NumberAgreementRule`] | `NUMBER_AGREEMENT` | Numeral-noun case/number agreement |
//! | [`NegationAgreementRule`] | `NEGATION_AGREEMENT` | Negation verb person/number agreement |
//! | [`DoubleNegationRule`] | `DOUBLE_NEGATION` | Non-standard double negation |
//! | [`MissingSpaceAfterPunctuationRule`] | `MISSING_SPACE_AFTER_PUNCT` | No space after comma/period/semicolon |
//! | [`ExtraSpaceBeforePunctuationRule`] | `EXTRA_SPACE_BEFORE_PUNCT` | Space before comma/period (error in Finnish) |
//! | [`PartitiveObjectRule`] | `PARTITIVE_OBJECT` | Negated sentences require partitive object |
//! | [`SubjectVerbAgreementRule`] | `SUBJECT_VERB_PERSON` | Subject-verb person agreement via pronouns |
//! | [`PostpositionCaseRule`] | `POSTPOSITION_CASE` | Postpositions require specific cases |
//! | [`ComparativePartitiveRule`] | `COMPARATIVE_PARTITIVE` | Comparative + partitive without "kuin" |
//! | [`MissingMainVerbRule`] | `MISSING_MAIN_VERB` | Sentence without a finite verb |
//! | [`SentenceInitialLowercaseRule`] | `SENTENCE_INITIAL_LOWERCASE` | Lowercase after colon/semicolon |
//! | [`ExcessiveExclamationRule`] | `EXCESSIVE_EXCLAMATION` | Multiple exclamation/question marks |
//! | [`CommaInSubordinateRule`] | `COMMA_BEFORE_RELATIVE` | Missing comma before relative pronouns |
//! | [`PossessiveSuffixRule`] | `POSSESSIVE_REDUNDANCY` | Redundant possessive pronoun + suffix |

mod agreement;
mod capitalization;
mod comma_before_conjunction;
mod comma_in_subordinate;
mod comparative_partitive;
mod compound_spacing;
mod double_negation;
mod double_space;
mod excessive_exclamation;
mod extra_space_before_punctuation;
mod missing_main_verb;
mod missing_space_after_punctuation;
mod negation_agreement;
mod number_agreement;
mod partitive_object;
mod possessive_suffix;
mod postposition_case;
mod quotation_mark;
mod repeated_word;
mod sentence_initial_lowercase;
mod subject_verb_agreement;

pub use agreement::AgreementRule;
pub use capitalization::CapitalizationRule;
pub use comma_before_conjunction::CommaBeforeConjunctionRule;
pub use comma_in_subordinate::CommaInSubordinateRule;
pub use comparative_partitive::ComparativePartitiveRule;
pub use compound_spacing::CompoundSpacingRule;
pub use double_negation::DoubleNegationRule;
pub use double_space::DoubleSpaceRule;
pub use excessive_exclamation::ExcessiveExclamationRule;
pub use extra_space_before_punctuation::ExtraSpaceBeforePunctuationRule;
pub use missing_main_verb::MissingMainVerbRule;
pub use missing_space_after_punctuation::MissingSpaceAfterPunctuationRule;
pub use negation_agreement::NegationAgreementRule;
pub use number_agreement::NumberAgreementRule;
pub use partitive_object::PartitiveObjectRule;
pub use possessive_suffix::PossessiveSuffixRule;
pub use postposition_case::PostpositionCaseRule;
pub use quotation_mark::QuotationMarkRule;
pub use repeated_word::RepeatedWordRule;
pub use sentence_initial_lowercase::SentenceInitialLowercaseRule;
pub use subject_verb_agreement::SubjectVerbAgreementRule;
