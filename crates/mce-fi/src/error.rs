//! Crate-level error type for `mce-fi`.
//!
//! Wraps internal errors (e.g., [`mce_fst::VfstError`]) so that downstream
//! consumers do not need to depend on `mce-fst` directly.

use thiserror::Error;

/// Error type for `mce-fi` operations.
#[derive(Debug, Error)]
pub enum FiError {
    /// An error from the underlying FST engine.
    #[error("FST error: {0}")]
    Fst(#[from] mce_fst::VfstError),
}
