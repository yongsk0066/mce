//! Finnish compound word analyzer using [`FinnishAnalyzer`] as dictionary lookup.
//!
//! Bridges the M3 [`CompoundAnalyzer`] (pushdown transducer) with the real VFST
//! dictionary via [`FinnishAnalyzer`]. A word is considered "in the dictionary"
//! if it yields at least one valid morphological analysis.
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

use mce_core::compound::{CompoundAnalyzer, CompoundSplit};
use mce_fst::VfstError;

use crate::morphology::{Analyzer, FinnishAnalyzer};

/// Type alias for the boxed dictionary predicate used in compound analysis.
type DictPredicate = Box<dyn Fn(&str) -> bool>;

/// Finnish compound word analyzer using [`FinnishAnalyzer`] as dictionary lookup.
///
/// Wraps a [`CompoundAnalyzer`] whose dictionary predicate delegates to
/// [`FinnishAnalyzer::analyze`]: a word is known if it has at least one valid
/// morphological analysis in the VFST dictionary.
pub struct FinnishCompoundAnalyzer {
    /// Kept alive so the `Rc` inside the closure remains valid.
    _analyzer: Rc<FinnishAnalyzer>,
    compound: CompoundAnalyzer<DictPredicate>,
}

impl FinnishCompoundAnalyzer {
    /// Create a new `FinnishCompoundAnalyzer` from raw VFST binary data (mor.vfst).
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

        let compound = CompoundAnalyzer::new(lookup);

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
}
