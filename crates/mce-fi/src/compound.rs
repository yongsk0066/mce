//! Finnish compound word analyzer using [`FinnishAnalyzer`] as dictionary lookup.
//!
//! Bridges the M3 [`CompoundAnalyzer`] (pushdown transducer) with the real VFST
//! dictionary via [`FinnishAnalyzer`]. A word is considered "in the dictionary"
//! if it yields at least one valid morphological analysis.
//!
//! Includes Finnish-specific stem reconstruction for compound linking morphemes:
//! - `-en-` linking with nen-stem words (hevos**en**kenkä → hevonen + kenkä)
//! - `-n-` genitive linking (kissa**n**pentu → kissa + pentu)
//! - `-s-`, `-i-`, `-o-`, `-u-` linking
//! - Zero linking (jää**kaappi** → jää + kaappi)
//!
//! # Example
//!
//! ```no_run
//! use mce_fi::compound::FinnishCompoundAnalyzer;
//!
//! let vfst_data = std::fs::read("mor.vfst").unwrap();
//! let analyzer = FinnishCompoundAnalyzer::from_bytes(&vfst_data).unwrap();
//!
//! let splits = analyzer.analyze("rautatieasema");
//! assert!(!splits.is_empty());
//! ```

use std::rc::Rc;

use mce_core::compound::{CompoundAnalyzer, CompoundSplit, StemReconstructor};
use mce_fst::VfstError;

use crate::morphology::{Analyzer, FinnishAnalyzer};

/// Type alias for the boxed dictionary predicate used in compound analysis.
type DictPredicate = Box<dyn Fn(&str) -> bool>;

/// Finnish-specific stem reconstruction for compound linking morphemes.
///
/// Given a bare stem (left part after stripping a linking morpheme) and the
/// linking morpheme, returns candidate dictionary forms to try.
///
/// # Reconstruction rules
///
/// | Linking morpheme | Stem pattern | Reconstruction | Example |
/// |------------------|-------------|----------------|---------|
/// | `"en"` | ends in `s` | strip `s`, add `nen` | hevos → hevonen |
///
/// Finnish nen-stem words (e.g., hevonen, nainen, ihminen) have the compound
/// stem ending in `-s` (hevos-, nais-, ihmis-) and use `-en-` as the genitive
/// linking morpheme in compounds. The dictionary (nominative) form is
/// reconstructed by replacing the trailing `s` with `nen`.
fn finnish_stem_reconstructor(stem: &str, link: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    match link {
        "en" => {
            // nen-stem words: if compound stem ends in 's', replace 's' with
            // 'nen' to get the nominative dictionary form.
            // E.g., hevos → hevonen, nais → nainen, ihmis → ihminen
            if let Some(base) = stem.strip_suffix('s') {
                candidates.push(format!("{base}nen"));
            }
        }
        _ => {
            // For "n", "s", "i", "o", "u" — the bare stem itself is the
            // dictionary form (handled by the direct lookup in the analyzer).
            // No additional reconstruction needed.
        }
    }

    candidates
}

/// Finnish compound word analyzer using [`FinnishAnalyzer`] as dictionary lookup.
///
/// Wraps a [`CompoundAnalyzer`] whose dictionary predicate delegates to
/// [`FinnishAnalyzer::analyze`]: a word is known if it has at least one valid
/// morphological analysis in the VFST dictionary.
///
/// Includes Finnish-specific stem reconstruction for linking morphemes, enabling
/// analysis of compounds like `hevosenkenkä` (hevonen + kenkä).
pub struct FinnishCompoundAnalyzer {
    /// Kept alive so the `Rc` inside the closure remains valid.
    _analyzer: Rc<FinnishAnalyzer>,
    compound: CompoundAnalyzer<DictPredicate>,
}

impl FinnishCompoundAnalyzer {
    /// Create a new `FinnishCompoundAnalyzer` from raw VFST binary data (mor.vfst).
    ///
    /// This sets up the compound analyzer with:
    /// - VFST-based dictionary lookup (a word is known if it has morphological analyses)
    /// - Finnish stem reconstruction for linking morphemes (nen-stems, etc.)
    pub fn from_bytes(mor_vfst: &[u8]) -> Result<Self, VfstError> {
        let analyzer = Rc::new(FinnishAnalyzer::from_bytes(mor_vfst)?);
        let analyzer_clone = Rc::clone(&analyzer);

        let lookup: DictPredicate = Box::new(move |word: &str| {
            let chars: Vec<char> = word.chars().collect();
            let len = chars.len();
            if len == 0 {
                return false;
            }
            !analyzer_clone.analyze(&chars, len).is_empty()
        });

        let reconstructor: StemReconstructor = Box::new(finnish_stem_reconstructor);

        let compound = CompoundAnalyzer::new(lookup).with_stem_reconstructor(reconstructor);

        Ok(Self {
            _analyzer: analyzer,
            compound,
        })
    }

    /// Analyze a word and return all valid compound splits, sorted by penalty
    /// (lowest first).
    ///
    /// Returns an empty vector if the word is a single dictionary word or
    /// cannot be decomposed into compound parts.
    pub fn analyze(&self, word: &str) -> Vec<CompoundSplit> {
        self.compound.analyze(word)
    }

    /// Convenience: returns `true` if the word has at least one compound split
    /// with two or more word parts.
    pub fn is_compound(&self, word: &str) -> bool {
        let splits = self.analyze(word);
        splits.iter().any(|s| s.word_parts().len() >= 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that FinnishCompoundAnalyzer can be constructed and the closure
    /// works correctly with a trivial mock (empty VFST will fail, so we just
    /// test the type-level wiring here).
    #[test]
    fn type_wiring_compiles() {
        // This test only verifies that the types compose correctly.
        // A real test requires a VFST dictionary (see integration tests).
        fn _assert_send_not_required(_: &FinnishCompoundAnalyzer) {}
    }

    #[test]
    fn finnish_reconstructor_nen_stem() {
        // hevos + en → hevonen (strip 's', add 'nen')
        let candidates = finnish_stem_reconstructor("hevos", "en");
        assert_eq!(candidates, vec!["hevonen"]);
    }

    #[test]
    fn finnish_reconstructor_nen_stem_nainen() {
        // nais + en → nainen
        let candidates = finnish_stem_reconstructor("nais", "en");
        assert_eq!(candidates, vec!["nainen"]);
    }

    #[test]
    fn finnish_reconstructor_en_no_s_stem() {
        // If the stem does not end in 's', no nen-reconstruction is attempted.
        let candidates = finnish_stem_reconstructor("koira", "en");
        assert!(candidates.is_empty());
    }

    #[test]
    fn finnish_reconstructor_n_linking() {
        // For "n" linking, no reconstruction is needed (the bare stem is the
        // dictionary form, e.g., kissa + n).
        let candidates = finnish_stem_reconstructor("kissa", "n");
        assert!(candidates.is_empty());
    }

    #[test]
    fn finnish_reconstructor_s_linking() {
        let candidates = finnish_stem_reconstructor("kirjoitu", "s");
        assert!(candidates.is_empty());
    }
}
