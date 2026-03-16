//! Crate-level error type for `mce-grammar`.
//!
//! Wraps internal errors (e.g., [`mce_fi::FiError`]) so that downstream
//! consumers do not need to depend on `mce-fst` directly.

use thiserror::Error;

/// Error type for `mce-grammar` initialization (e.g., loading a VFST dictionary).
///
/// Named `InitError` to avoid confusion with [`super::GrammarError`], which
/// represents a detected grammar mistake in user text.
#[derive(Debug, Error)]
pub enum InitError {
    /// An error from the Finnish language module.
    #[error("{0}")]
    Fi(#[from] mce_fi::FiError),
}
