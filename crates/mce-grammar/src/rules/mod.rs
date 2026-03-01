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

mod agreement;
mod capitalization;
mod repeated_word;

pub use agreement::AgreementRule;
pub use capitalization::CapitalizationRule;
pub use repeated_word::RepeatedWordRule;
