//! MCE Disambig — M4' Disambiguation Engine.
//!
//! Resolves ambiguity when a word has multiple morphological analyses by
//! selecting the globally optimal reading sequence across a sentence.
//!
//! # Architecture (MCE v3 — M4')
//!
//! The disambiguation engine has two layers:
//!
//! 1. **Weighted Lattice + Viterbi** (this crate): Standard bigram-based
//!    Viterbi decoding over a lattice of candidate readings. Uses POS
//!    transition weights to find the most probable reading sequence.
//!
//! 2. **Compressed Sensing uniqueness verification** (future): For
//!    morphologically rich word forms, CS can prove that the analysis
//!    is uniquely determined without needing Viterbi. When the RIP
//!    condition is satisfied, CS returns immediately; otherwise, Viterbi
//!    serves as the fallback.
//!
//! # Modules
//!
//! - [`lattice`]: Weighted lattice data structure (nodes, readings, scores).
//! - [`viterbi`]: Viterbi algorithm for optimal path finding.
//! - [`bigram`]: POS bigram transition model with Finnish defaults.
//!
//! # Example
//!
//! ```
//! use mce_core::analysis::Analysis;
//! use mce_disambig::{Disambiguator, ViterbiDisambiguator};
//! use mce_disambig::bigram::BigramModel;
//!
//! // Build a sentence with ambiguous words.
//! let mut noun = Analysis::new();
//! noun.set("CLASS", "nimisana");
//! noun.set("BASEFORM", "kuusi");
//!
//! let mut num = Analysis::new();
//! num.set("CLASS", "lukusana");
//! num.set("BASEFORM", "kuusi");
//!
//! let mut verb = Analysis::new();
//! verb.set("CLASS", "teonsana");
//! verb.set("BASEFORM", "kasvaa");
//!
//! let sentence = vec![
//!     vec![noun, num],    // "kuusi" — spruce or six?
//!     vec![verb],         // "kasvaa" — grows
//! ];
//!
//! let disambiguator = ViterbiDisambiguator::with_finnish_defaults();
//! let result = disambiguator.disambiguate(&sentence);
//! assert_eq!(result.len(), 2);
//! assert_eq!(result[0].get("CLASS"), Some("nimisana")); // noun wins (NOUN->VERB common)
//! ```

pub mod bigram;
pub mod lattice;
pub mod viterbi;

use mce_core::analysis::Analysis;

use crate::bigram::BigramModel;
use crate::lattice::{Lattice, LatticeNode, Reading};
use crate::viterbi::TransitionFn;

/// Trait for disambiguation strategies.
///
/// A disambiguator takes a sentence represented as a slice of reading
/// lists (one list per word position) and returns one [`Analysis`] per
/// position — the best reading according to its strategy.
pub trait Disambiguator {
    /// Disambiguate a sentence.
    ///
    /// # Arguments
    ///
    /// * `sentence` - A slice where each element is a `Vec<Analysis>`
    ///   containing the candidate readings for one word position.
    ///
    /// # Returns
    ///
    /// A vector of [`Analysis`], one per word position, representing the
    /// best reading sequence. Returns an empty vector if the sentence is
    /// empty or contains a position with zero candidates.
    fn disambiguate(&self, sentence: &[Vec<Analysis>]) -> Vec<Analysis>;
}

/// Viterbi-based disambiguator using POS bigram transitions.
///
/// Wraps the Viterbi algorithm with a [`BigramModel`] for transition scoring.
/// Each candidate reading receives a uniform emission score unless the
/// analysis contains a `WEIGHT` attribute (from the FST), in which case
/// that value is used.
pub struct ViterbiDisambiguator {
    model: BigramModel,
}

impl ViterbiDisambiguator {
    /// Create a disambiguator with the given bigram model.
    pub fn new(model: BigramModel) -> Self {
        Self { model }
    }

    /// Create a disambiguator with Finnish POS transition defaults.
    pub fn with_finnish_defaults() -> Self {
        Self::new(BigramModel::finnish_defaults())
    }

    /// Access the underlying bigram model.
    pub fn model(&self) -> &BigramModel {
        &self.model
    }

    /// Access the underlying bigram model mutably.
    pub fn model_mut(&mut self) -> &mut BigramModel {
        &mut self.model
    }

    /// Build a lattice from raw analysis candidates.
    ///
    /// Emission scores are derived from the `WEIGHT` attribute if present,
    /// or default to 0.0 (uniform) otherwise.
    fn build_lattice(&self, sentence: &[Vec<Analysis>]) -> Lattice {
        let nodes = sentence
            .iter()
            .enumerate()
            .map(|(pos, analyses)| {
                let readings = analyses
                    .iter()
                    .map(|a| {
                        let score = a
                            .get("WEIGHT")
                            .and_then(|w| w.parse::<f64>().ok())
                            .unwrap_or(0.0);
                        Reading::new(a.clone(), score)
                    })
                    .collect();
                LatticeNode::new(pos, readings)
            })
            .collect();
        Lattice { nodes }
    }

    /// Get the transition function for this disambiguator.
    fn transition_fn(&self) -> Box<TransitionFn> {
        self.model.as_transition_fn()
    }
}

impl Disambiguator for ViterbiDisambiguator {
    fn disambiguate(&self, sentence: &[Vec<Analysis>]) -> Vec<Analysis> {
        if sentence.is_empty() {
            return Vec::new();
        }

        // Check for positions with no candidates.
        if sentence.iter().any(|readings| readings.is_empty()) {
            return Vec::new();
        }

        // Fast path: if every position has exactly one reading, no disambiguation needed.
        if sentence.iter().all(|readings| readings.len() == 1) {
            return sentence.iter().map(|r| r[0].clone()).collect();
        }

        let lattice = self.build_lattice(sentence);
        let transition = self.transition_fn();
        let path = viterbi::viterbi(&lattice, &transition);

        path.iter()
            .enumerate()
            .map(|(i, &j)| lattice.nodes[i].readings[j].analysis.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mce_core::analysis::ATTR_CLASS;

    fn make_analysis(class: &str, baseform: &str) -> Analysis {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, class);
        a.set("BASEFORM", baseform);
        a
    }

    fn make_weighted_analysis(class: &str, baseform: &str, weight: f64) -> Analysis {
        let mut a = make_analysis(class, baseform);
        a.set("WEIGHT", weight.to_string());
        a
    }

    #[test]
    fn disambiguator_empty_sentence() {
        let d = ViterbiDisambiguator::with_finnish_defaults();
        let result = d.disambiguate(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn disambiguator_single_word_single_reading() {
        let d = ViterbiDisambiguator::with_finnish_defaults();
        let sentence = vec![vec![make_analysis("nimisana", "koira")]];
        let result = d.disambiguate(&sentence);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].get("BASEFORM"), Some("koira"));
    }

    #[test]
    fn disambiguator_unambiguous_sentence() {
        let d = ViterbiDisambiguator::with_finnish_defaults();
        let sentence = vec![
            vec![make_analysis("nimisana", "koira")],
            vec![make_analysis("teonsana", "juosta")],
            vec![make_analysis("seikkasana", "nopeasti")],
        ];
        let result = d.disambiguate(&sentence);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].get("BASEFORM"), Some("koira"));
        assert_eq!(result[1].get("BASEFORM"), Some("juosta"));
        assert_eq!(result[2].get("BASEFORM"), Some("nopeasti"));
    }

    /// Core test: "kuusi kasvaa" — kuusi is ambiguous (NOUN spruce / NUM six).
    /// Bigram model should prefer NOUN->VERB over NUM->VERB.
    #[test]
    fn disambiguator_kuusi_kasvaa() {
        let d = ViterbiDisambiguator::with_finnish_defaults();
        let sentence = vec![
            vec![
                make_analysis("nimisana", "kuusi"), // NOUN: spruce
                make_analysis("lukusana", "kuusi"), // NUM: six
            ],
            vec![make_analysis("teonsana", "kasvaa")], // VERB: grows
        ];
        let result = d.disambiguate(&sentence);
        assert_eq!(result.len(), 2);
        // NOUN->VERB (-0.3) beats NUM->VERB (-3.0 default or specific)
        assert_eq!(result[0].get("CLASS"), Some("nimisana"));
        assert_eq!(result[1].get("CLASS"), Some("teonsana"));
    }

    /// "kuusi koiraa" — NUM->NOUN should win over NOUN->NOUN.
    #[test]
    fn disambiguator_kuusi_koiraa() {
        let d = ViterbiDisambiguator::with_finnish_defaults();
        let sentence = vec![
            vec![
                make_analysis("nimisana", "kuusi"), // NOUN: spruce
                make_analysis("lukusana", "kuusi"), // NUM: six
            ],
            vec![make_analysis("nimisana", "koira")], // NOUN: dog
        ];
        let result = d.disambiguate(&sentence);
        assert_eq!(result.len(), 2);
        // NUM->NOUN (-0.4) beats NOUN->NOUN (-0.8)
        assert_eq!(result[0].get("CLASS"), Some("lukusana"));
    }

    #[test]
    fn disambiguator_position_with_no_readings() {
        let d = ViterbiDisambiguator::with_finnish_defaults();
        let sentence = vec![
            vec![make_analysis("nimisana", "koira")],
            vec![], // empty!
            vec![make_analysis("teonsana", "juosta")],
        ];
        let result = d.disambiguate(&sentence);
        assert!(result.is_empty());
    }

    #[test]
    fn disambiguator_weight_attribute_used() {
        let d = ViterbiDisambiguator::with_finnish_defaults();
        let sentence = vec![vec![
            make_weighted_analysis("nimisana", "rare_noun", -5.0), // heavily penalized
            make_weighted_analysis("teonsana", "common_verb", 0.0), // normal weight
        ]];
        let result = d.disambiguate(&sentence);
        assert_eq!(result.len(), 1);
        // The verb should win because the noun has a very low weight
        assert_eq!(result[0].get("CLASS"), Some("teonsana"));
    }

    #[test]
    fn disambiguator_custom_model() {
        let mut model = BigramModel::new(-10.0);
        // Only define one transition as very favorable
        model.set_weight("X", "Y", 0.0);

        let d = ViterbiDisambiguator::new(model);
        let sentence = vec![
            vec![make_analysis("X", "a"), make_analysis("Z", "b")],
            vec![make_analysis("Y", "c"), make_analysis("W", "d")],
        ];
        let result = d.disambiguate(&sentence);
        assert_eq!(result.len(), 2);
        // X->Y (0.0) vs X->W (-10.0) vs Z->Y (-10.0) vs Z->W (-10.0)
        assert_eq!(result[0].get("CLASS"), Some("X"));
        assert_eq!(result[1].get("CLASS"), Some("Y"));
    }

    /// Longer Finnish-like sentence to verify end-to-end integration.
    ///
    /// With only POS bigrams (no morphological features), the model
    /// prefers ADJ->NOUN over ADV->NOUN at position 3->4 because
    /// ADJ->NOUN (-0.2) is a much stronger transition than ADV->NOUN (-0.8).
    /// A richer model (trigram, morphological features) would disambiguate
    /// further, but the bigram model correctly picks the path with
    /// strongest total transition score.
    #[test]
    fn disambiguator_full_sentence() {
        let d = ViterbiDisambiguator::with_finnish_defaults();

        // "Iso koira juoksee nopeasti pihalla"
        let sentence = vec![
            vec![
                make_analysis("laatusana", "iso"),
                make_analysis("nimisana", "iso"), // rare but possible
            ],
            vec![
                make_analysis("nimisana", "koira"),
                make_analysis("teonsana", "koira"), // unlikely
            ],
            vec![make_analysis("teonsana", "juosta")],
            vec![
                make_analysis("seikkasana", "nopeasti"),
                make_analysis("laatusana", "nopea"), // alternative
            ],
            vec![make_analysis("nimisana", "piha")],
        ];

        let result = d.disambiguate(&sentence);
        assert_eq!(result.len(), 5);

        // ADJ->NOUN->VERB is correct for positions 0-2
        assert_eq!(result[0].get("CLASS"), Some("laatusana")); // ADJ
        assert_eq!(result[1].get("CLASS"), Some("nimisana")); // NOUN
        assert_eq!(result[2].get("CLASS"), Some("teonsana")); // VERB

        // Position 3: ADJ wins over ADV because ADJ->NOUN (-0.2) >> ADV->NOUN (-0.8)
        // The Viterbi algorithm optimizes globally, favoring the stronger
        // ?->NOUN transition at position 4.
        assert_eq!(result[3].get("CLASS"), Some("laatusana")); // ADJ (bigram preference)
        assert_eq!(result[4].get("CLASS"), Some("nimisana")); // NOUN
    }

    #[test]
    fn disambiguator_trait_is_object_safe() {
        // Verify the trait can be used as a trait object.
        let d: Box<dyn Disambiguator> = Box::new(ViterbiDisambiguator::with_finnish_defaults());
        let sentence = vec![vec![make_analysis("nimisana", "koira")]];
        let result = d.disambiguate(&sentence);
        assert_eq!(result.len(), 1);
    }
}
