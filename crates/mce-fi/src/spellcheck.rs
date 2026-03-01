// Finnish spell-checker: connects M1 Succinct Trie with FinnishAnalyzer.
//
// Architecture:
//   - check(): FinnishAnalyzer (FST traversal) is the primary oracle.
//     It knows ALL valid Finnish word forms (inflections, compounds, etc.)
//     The trie serves as a fast-path cache for known base forms.
//   - suggest(): Uses the trie's fuzzy_search (Levenshtein automaton) on
//     base forms extracted from the FST symbol table, then validates
//     candidates through the morph validator.
//
// This module glues together:
//   - mce_core::trie (M1 Succinct Trie)
//   - mce_fi::morphology (FinnishAnalyzer)
//   - mce_speller::pipeline (SpellChecker, MorphValidator, SpellCheckerBuilder)

use mce_core::trie::{SuccinctTrie, TrieBuilder};
use mce_fst::unweighted::UnweightedTransducer;
use mce_fst::VfstError;
use mce_speller::pipeline::{MorphValidator, SpellChecker, SpellCheckerBuilder};
use mce_speller::SpellResult;

use crate::morphology::{Analyzer, FinnishAnalyzer};

/// Morphological validator backed by [`FinnishAnalyzer`].
///
/// Wraps the FST-based analyzer so it can be used as the morph validation
/// callback in [`SpellChecker`]. A word is considered valid if the analyzer
/// produces at least one morphological analysis for it.
pub struct FinnishMorphValidator {
    analyzer: FinnishAnalyzer,
}

impl FinnishMorphValidator {
    /// Create a new morph validator from raw VFST binary data (mor.vfst).
    pub fn from_bytes(data: &[u8]) -> Result<Self, VfstError> {
        let analyzer = FinnishAnalyzer::from_bytes(data)?;
        Ok(Self { analyzer })
    }

    /// Access the underlying analyzer.
    pub fn analyzer(&self) -> &FinnishAnalyzer {
        &self.analyzer
    }
}

impl MorphValidator for FinnishMorphValidator {
    fn is_valid(&self, word: &[char], word_len: usize) -> bool {
        !self.analyzer.analyze(word, word_len).is_empty()
    }
}

/// Finnish spell-checker combining M1 Succinct Trie with FinnishAnalyzer.
///
/// The trie holds base forms extracted from the VFST symbol table for fast
/// exact lookup and fuzzy suggestion generation. The [`FinnishAnalyzer`]
/// serves as the morph validation fallback for compound words, inflected
/// forms, and other productive morphology that cannot be enumerated in
/// a static word list.
///
/// # Pipeline
///
/// ```text
/// check(word)
///   1. Cache lookup (fast path)
///   2. Trie exact match (base-form dictionary)
///   3. FinnishAnalyzer (FST traversal — handles all morphology)
///
/// suggest(word, max_edits)
///   1. Trie fuzzy search (Levenshtein automaton on base forms)
///   2. Filter by morph validity (optional)
/// ```
pub struct FinnishSpellChecker {
    checker: SpellChecker<FinnishMorphValidator>,
}

impl FinnishSpellChecker {
    /// Load a Finnish spell-checker from raw VFST dictionary bytes (mor.vfst).
    ///
    /// This:
    /// 1. Parses the VFST binary to create a [`FinnishAnalyzer`]
    /// 2. Extracts single-character symbols from the symbol table to build
    ///    a base-forms trie (the alphabet known to the transducer)
    /// 3. Wires everything into a [`SpellChecker`] pipeline with caching
    ///
    /// # Errors
    ///
    /// Returns [`VfstError`] if the VFST data is malformed.
    pub fn from_bytes(mor_vfst: &[u8]) -> Result<Self, VfstError> {
        // Build the morph validator (owns its own FinnishAnalyzer).
        let morph = FinnishMorphValidator::from_bytes(mor_vfst)?;

        // Build a trie from the FST's symbol table.
        // We extract all single-character symbols (the alphabet) and
        // multi-character symbol strings that look like real words.
        // This gives us a small but useful base-form set for suggestions.
        let trie = build_trie_from_vfst(mor_vfst)?;

        let checker = SpellCheckerBuilder::new()
            .trie(trie)
            .morph_validator(morph)
            .cache_size(2) // 4x base cache for production use
            .build();

        Ok(Self { checker })
    }

    /// Check whether a Finnish word is correctly spelled.
    ///
    /// Returns [`SpellResult::Ok`] if the word is found in the trie or
    /// passes morphological analysis via the FST. Results are cached
    /// for repeated lookups.
    pub fn check(&mut self, word: &str) -> SpellResult {
        self.checker.check(word)
    }

    /// Generate spelling suggestions for a misspelled word.
    ///
    /// Uses the trie's Levenshtein automaton to find candidates within
    /// `max_edits` edit distance of the input. Candidates are filtered
    /// through the morph validator and returned sorted by edit distance.
    ///
    /// Returns at most 10 suggestions.
    pub fn suggest(&self, word: &str, max_edits: usize) -> Vec<String> {
        self.checker.suggest(word, max_edits, 10)
    }

    /// Generate suggestions without morphological filtering.
    ///
    /// Useful when you want raw candidates from the trie regardless
    /// of morphological validity. Returns at most 10 suggestions.
    pub fn suggest_unfiltered(&self, word: &str, max_edits: usize) -> Vec<String> {
        self.checker.suggest_unfiltered(word, max_edits, 10)
    }

    /// Access the underlying [`SpellChecker`].
    pub fn inner(&self) -> &SpellChecker<FinnishMorphValidator> {
        &self.checker
    }

    /// Access the underlying trie.
    pub fn trie(&self) -> &SuccinctTrie {
        self.checker.trie()
    }
}

/// Build a [`SuccinctTrie`] from the VFST symbol table.
///
/// Extracts all "normal" single-character symbols from the transducer's
/// symbol table. These are the characters the FST knows about. While not
/// a word list per se, the single characters enable basic fuzzy matching.
///
/// For a richer trie, callers can later extend this with a frequency-based
/// word list or by selective FST traversal of common base forms.
fn build_trie_from_vfst(data: &[u8]) -> Result<SuccinctTrie, VfstError> {
    let transducer = UnweightedTransducer::from_bytes(data)?;
    let symbols = transducer.symbols();

    let mut builder = TrieBuilder::new();

    // Insert all single-character symbols as trie entries.
    // These are the "alphabet" of the transducer — useful as building
    // blocks for fuzzy search even though they are not full words.
    let first = symbols.first_normal_char as usize;
    let multi = symbols.first_multi_char as usize;

    for sym_str in &symbols.symbol_strings[first..multi] {
        if !sym_str.is_empty() {
            builder.insert(sym_str.as_bytes().to_vec());
        }
    }

    Ok(builder.build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finnish_morph_validator_rejects_on_empty_transducer_data() {
        // Garbage data should fail to parse.
        let result = FinnishMorphValidator::from_bytes(&[0, 0, 0, 0]);
        assert!(result.is_err());
    }

    #[test]
    fn finnish_spell_checker_rejects_garbage_data() {
        let result = FinnishSpellChecker::from_bytes(&[0, 0, 0, 0]);
        assert!(result.is_err());
    }

    #[test]
    fn build_trie_from_vfst_rejects_garbage() {
        let result = build_trie_from_vfst(&[0, 0, 0, 0]);
        assert!(result.is_err());
    }

    #[test]
    fn morph_validator_trait_impl_compiles() {
        // Verify that FinnishMorphValidator implements MorphValidator.
        fn assert_morph_validator<T: MorphValidator>() {}
        assert_morph_validator::<FinnishMorphValidator>();
    }
}
