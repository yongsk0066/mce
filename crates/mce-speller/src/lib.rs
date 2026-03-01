//! MCE Speller — 맞춤법 검사 및 추천 인프라.
//!
//! Adapted from corevoikko (voikko-fi/speller/, voikko-fi/suggestion/).

pub mod cache;
pub mod pipeline;
pub mod status;
pub mod user_dict;

/// Internal spell-checker result type.
///
/// Adapted from corevoikko (voikko-core/enums.rs, SpellResult).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpellResult {
    /// Word is correctly spelled.
    Ok,
    /// Word is correct if the first letter is capitalized.
    CapitalizeFirst,
    /// Word has a capitalization error.
    CapitalizationError,
    /// Word is misspelled.
    Failed,
}

/// Trait for spell checkers.
///
/// All implementations take a word as a `char` slice (for random-access
/// indexing into character positions) and return a [`SpellResult`].
///
/// Adapted from corevoikko (voikko-fi/speller/mod.rs, Speller trait).
pub trait Speller {
    /// Check whether the given word is correct (or would be correct
    /// with different capitalization).
    ///
    /// - `word`: the word to check (char slice)
    /// - `word_len`: the number of characters to consider
    fn spell(&self, word: &[char], word_len: usize) -> SpellResult;
}
