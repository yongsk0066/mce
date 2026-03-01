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

mod agreement;
mod capitalization;
mod comma_before_conjunction;
mod compound_spacing;
mod double_negation;
mod double_space;
mod negation_agreement;
mod number_agreement;
mod quotation_mark;
mod repeated_word;

pub use agreement::AgreementRule;
pub use capitalization::CapitalizationRule;
pub use comma_before_conjunction::CommaBeforeConjunctionRule;
pub use compound_spacing::CompoundSpacingRule;
pub use double_negation::DoubleNegationRule;
pub use double_space::DoubleSpaceRule;
pub use negation_agreement::NegationAgreementRule;
pub use number_agreement::NumberAgreementRule;
pub use quotation_mark::QuotationMarkRule;
pub use repeated_word::RepeatedWordRule;
