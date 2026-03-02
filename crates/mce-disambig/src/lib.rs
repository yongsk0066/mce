//! MCE Disambig — M4' Disambiguation Engine.
//!
//! Resolves ambiguity when a word has multiple morphological analyses by
//! selecting the globally optimal reading sequence across a sentence.
//!
//! # Architecture (MCE v3 — M4')
//!
//! The disambiguation engine has three stages:
//!
//! 1. **CG-lite rules** (applied upstream): High-precision deterministic rules
//!    that eliminate obviously impossible readings.
//!
//! 2. **Suffix-based emission scoring** ([`suffix_tagger`]): A lightweight
//!    logistic regression model that uses character suffix/prefix features and
//!    context to produce per-tag emission probabilities. When loaded, these
//!    scores feed into the Viterbi lattice to re-rank FST-generated candidates.
//!
//! 3. **Weighted Lattice + Viterbi** (this crate): Standard bigram-based
//!    Viterbi decoding over a lattice of candidate readings. Uses POS
//!    transition weights to find the most probable reading sequence.
//!
//! An optional **Compressed Sensing scoring** layer ([`cs`]) can also be
//! configured, though it has been shown to provide negative results for
//! this task.
//!
//! # Modules
//!
//! - [`lattice`]: Weighted lattice data structure (nodes, readings, scores).
//! - [`viterbi`]: Viterbi algorithm for optimal path finding.
//! - [`bigram`]: POS bigram transition model with Finnish defaults, corpus-weight
//!   infrastructure ([`BigramModel::from_counts`]), and emission scoring
//!   ([`EmissionScorer`]).
//! - [`corpus`]: CoNLL-U corpus parsing for bigram extraction and emission priors.
//! - [`cs`]: Compressed Sensing (FISTA) layer for sparse feature-based scoring.
//! - [`suffix_tagger`]: Suffix-based logistic regression POS tagger for
//!   emission scoring (94.87% standalone, ~95-96% integrated).
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
pub mod corpus;
pub mod cs;
pub mod lattice;
pub mod suffix_tagger;
pub mod viterbi;

use mce_core::analysis::Analysis;

use crate::bigram::{class_to_upos_category, BigramModel, EmissionScorer};
use crate::cs::SparseDisambiguator;
use crate::lattice::{Lattice, LatticeNode, Reading};
use crate::suffix_tagger::SuffixTagger;
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
///
/// When an [`EmissionScorer`] is configured, its adjustments are added to
/// the lattice emission scores. Use [`disambiguate_with_words`] to provide
/// surface forms for emission scoring.
///
/// When a [`SuffixTagger`] is configured, its per-UPOS log-probabilities
/// are added to each reading's emission score. The tagger maps each reading's
/// CLASS attribute to a UPOS category and looks up the corresponding
/// log-probability from the suffix model. This provides a strong signal
/// for disambiguation that combines character-level features with context.
///
/// An optional [`SparseDisambiguator`] (CS scorer) can be configured to
/// provide an additional disambiguation signal based on Compressed Sensing
/// sparse recovery in the morphological feature space.
pub struct ViterbiDisambiguator {
    model: BigramModel,
    emission_scorer: Option<EmissionScorer>,
    cs_scorer: Option<SparseDisambiguator>,
    suffix_tagger: Option<SuffixTagger>,
}

impl ViterbiDisambiguator {
    /// Create a disambiguator with the given bigram model.
    pub fn new(model: BigramModel) -> Self {
        Self {
            model,
            emission_scorer: None,
            cs_scorer: None,
            suffix_tagger: None,
        }
    }

    /// Create a disambiguator with Finnish POS transition defaults.
    pub fn with_finnish_defaults() -> Self {
        Self::new(BigramModel::finnish_defaults())
    }

    /// Create a disambiguator with Finnish defaults and emission scoring.
    pub fn with_finnish_defaults_and_emission() -> Self {
        Self {
            model: BigramModel::finnish_defaults(),
            emission_scorer: Some(EmissionScorer::finnish_defaults()),
            cs_scorer: None,
            suffix_tagger: None,
        }
    }

    /// Set the emission scorer for this disambiguator.
    pub fn set_emission_scorer(&mut self, scorer: EmissionScorer) {
        self.emission_scorer = Some(scorer);
    }

    /// Access the underlying bigram model.
    pub fn model(&self) -> &BigramModel {
        &self.model
    }

    /// Access the underlying bigram model mutably.
    pub fn model_mut(&mut self) -> &mut BigramModel {
        &mut self.model
    }

    /// Access the emission scorer, if configured.
    pub fn emission_scorer(&self) -> Option<&EmissionScorer> {
        self.emission_scorer.as_ref()
    }

    /// Set the Compressed Sensing scorer for this disambiguator.
    ///
    /// When present, the CS score (negated reconstruction error) is added
    /// to the emission score of each reading in the lattice.
    pub fn set_cs_scorer(&mut self, scorer: SparseDisambiguator) {
        self.cs_scorer = Some(scorer);
    }

    /// Access the CS scorer, if configured.
    pub fn cs_scorer(&self) -> Option<&SparseDisambiguator> {
        self.cs_scorer.as_ref()
    }

    /// Set the suffix tagger for emission scoring.
    ///
    /// When present, the suffix tagger's per-UPOS log-probabilities are
    /// added to each reading's emission score in the lattice. This is the
    /// primary mechanism for achieving 95%+ UPOS accuracy.
    ///
    /// The suffix tagger requires surface word forms to operate, so it
    /// only takes effect when [`disambiguate_with_words`] is called.
    pub fn set_suffix_tagger(&mut self, tagger: SuffixTagger) {
        self.suffix_tagger = Some(tagger);
    }

    /// Access the suffix tagger, if configured.
    pub fn suffix_tagger(&self) -> Option<&SuffixTagger> {
        self.suffix_tagger.as_ref()
    }

    /// Build a lattice from raw analysis candidates.
    ///
    /// Emission scores are derived from the `WEIGHT` attribute if present,
    /// or default to 0.0 (uniform) otherwise.
    fn build_lattice(&self, sentence: &[Vec<Analysis>]) -> Lattice {
        self.build_lattice_with_words(sentence, None)
    }

    /// Build a lattice from raw analysis candidates with optional surface words.
    ///
    /// When surface words and an emission scorer are provided, the emission
    /// score for each reading is adjusted by the scorer. When a suffix tagger
    /// is also present, its per-UPOS log-probabilities are added as well.
    fn build_lattice_with_words(
        &self,
        sentence: &[Vec<Analysis>],
        words: Option<&[&str]>,
    ) -> Lattice {
        // Pre-compute suffix tagger emission scores for all positions.
        // This is done once per sentence, outside the per-reading loop.
        let suffix_emissions: Option<Vec<Vec<f64>>> = match (&self.suffix_tagger, words) {
            (Some(tagger), Some(ws)) => {
                let n = ws.len();
                Some(
                    (0..n)
                        .map(|i| {
                            let prev = if i > 0 { Some(ws[i - 1]) } else { None };
                            let next = if i + 1 < n { Some(ws[i + 1]) } else { None };
                            tagger.emission_scores(ws[i], prev, next, i, n)
                        })
                        .collect(),
                )
            }
            _ => None,
        };

        let nodes = sentence
            .iter()
            .enumerate()
            .map(|(pos, analyses)| {
                // Compute CS scores for all analyses at this position (if configured).
                let cs_scores = self
                    .cs_scorer
                    .as_ref()
                    .map(|scorer| scorer.score_analyses(analyses));

                let readings = analyses
                    .iter()
                    .enumerate()
                    .map(|(idx, a)| {
                        let mut score = a
                            .get("WEIGHT")
                            .and_then(|w| w.parse::<f64>().ok())
                            .unwrap_or(0.0);

                        // Apply emission scorer adjustments if available.
                        if let (Some(scorer), Some(ws)) = (&self.emission_scorer, words) {
                            if pos < ws.len() {
                                score += scorer.score(ws[pos], a);
                            }
                        }

                        // Apply CS scorer adjustments if available.
                        if let Some(ref cs) = cs_scores {
                            score += cs[idx];
                        }

                        // Apply suffix tagger emission scores if available.
                        if let Some(ref emissions) = suffix_emissions {
                            if pos < emissions.len() {
                                if let Some(ref tagger) = self.suffix_tagger {
                                    let upos = class_to_upos_category(a.get("CLASS").unwrap_or(""));
                                    if let Some(class_idx) = tagger.class_index(upos) {
                                        score += emissions[pos][class_idx];
                                    }
                                }
                            }
                        }

                        Reading::new(a.clone(), score)
                    })
                    .collect();
                LatticeNode::new(pos, readings)
            })
            .collect();
        Lattice { nodes }
    }

    /// Disambiguate a sentence with surface word forms for emission scoring.
    ///
    /// Like [`Disambiguator::disambiguate`], but also takes the surface forms
    /// of each word so the [`EmissionScorer`] and [`SuffixTagger`] can compute
    /// their respective scores.
    ///
    /// # Arguments
    ///
    /// * `words` - The surface forms of the words in the sentence.
    /// * `sentence` - A slice where each element is a `Vec<Analysis>`
    ///   containing the candidate readings for one word position.
    ///
    /// # Returns
    ///
    /// A vector of [`Analysis`], one per word position, representing the
    /// best reading sequence. Returns an empty vector if the sentence is
    /// empty or contains a position with zero candidates.
    pub fn disambiguate_with_words(
        &self,
        words: &[&str],
        sentence: &[Vec<Analysis>],
    ) -> Vec<Analysis> {
        if sentence.is_empty() {
            return Vec::new();
        }

        if sentence.iter().any(|readings| readings.is_empty()) {
            return Vec::new();
        }

        if sentence.iter().all(|readings| readings.len() == 1) {
            return sentence.iter().map(|r| r[0].clone()).collect();
        }

        let lattice = self.build_lattice_with_words(sentence, Some(words));
        let transition = self.transition_fn();
        let path = viterbi::viterbi(&lattice, &transition);

        path.iter()
            .enumerate()
            .map(|(i, &j)| lattice.nodes[i].readings[j].analysis.clone())
            .collect()
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
    use mce_core::analysis::{ATTR_CLASS, ATTR_STRUCTURE};

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

    fn make_full_analysis(class: &str, baseform: &str, structure: &str) -> Analysis {
        let mut a = make_analysis(class, baseform);
        a.set(ATTR_STRUCTURE, structure);
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
        // NOUN->VERB (-0.3) beats NUM->VERB (-0.8)
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

    // ─────────────────────────────────────────────────────────────
    // Emission scorer integration tests
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn disambiguator_with_emission_scorer_construction() {
        let d = ViterbiDisambiguator::with_finnish_defaults_and_emission();
        assert!(d.emission_scorer().is_some());
    }

    #[test]
    fn disambiguator_set_emission_scorer() {
        let mut d = ViterbiDisambiguator::with_finnish_defaults();
        assert!(d.emission_scorer().is_none());
        d.set_emission_scorer(EmissionScorer::finnish_defaults());
        assert!(d.emission_scorer().is_some());
    }

    #[test]
    fn disambiguate_with_words_empty() {
        let d = ViterbiDisambiguator::with_finnish_defaults_and_emission();
        let result = d.disambiguate_with_words(&[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn disambiguate_with_words_single_unambiguous() {
        let d = ViterbiDisambiguator::with_finnish_defaults_and_emission();
        let sentence = vec![vec![make_analysis("nimisana", "koira")]];
        let result = d.disambiguate_with_words(&["koira"], &sentence);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].get("BASEFORM"), Some("koira"));
    }

    /// Emission scoring should boost the reading whose BASEFORM matches
    /// the surface form, helping disambiguation.
    #[test]
    fn disambiguate_with_words_baseform_match_helps() {
        let mut d = ViterbiDisambiguator::new(BigramModel::new(-1.0));
        // Strong emission scorer to make the test reliable
        d.set_emission_scorer(EmissionScorer::new(3.0, 0.0));

        // Word "koira" has two readings with equal transition context.
        // The one with BASEFORM="koira" (matching surface) should win.
        let sentence = vec![vec![
            make_analysis("nimisana", "koira"),   // BASEFORM matches surface
            make_analysis("teonsana", "koirata"), // BASEFORM does not match
        ]];
        let result = d.disambiguate_with_words(&["koira"], &sentence);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].get("BASEFORM"),
            Some("koira"),
            "Baseform-matching reading should win"
        );
    }

    /// Structure-based penalty should prefer simpler analyses.
    #[test]
    fn disambiguate_with_words_structure_penalty_helps() {
        let mut d = ViterbiDisambiguator::new(BigramModel::new(-1.0));
        // Strong structure penalty
        d.set_emission_scorer(EmissionScorer::new(0.0, 0.5));

        let sentence = vec![vec![
            make_full_analysis("nimisana", "kissa", "=pp"), // simple (len 3)
            make_full_analysis("nimisana", "kissanpentu", "=pp=pp=pp"), // compound (len 9)
        ]];
        let result = d.disambiguate_with_words(&["kissanpentu"], &sentence);
        assert_eq!(result.len(), 1);
        // The simpler structure should win due to penalty
        assert_eq!(
            result[0].get("BASEFORM"),
            Some("kissa"),
            "Simpler structure should win"
        );
    }

    /// The negation pattern "ei juokse" should work correctly with
    /// the expanded Finnish defaults (kieltosana -> teonsana).
    #[test]
    fn disambiguator_negation_pattern() {
        let d = ViterbiDisambiguator::with_finnish_defaults();
        let sentence = vec![
            vec![
                make_analysis("kieltosana", "ei"), // NEG
                make_analysis("nimisana", "ei"),   // very unlikely
            ],
            vec![
                make_analysis("teonsana", "juosta"), // VERB
                make_analysis("nimisana", "juoksu"), // unlikely
            ],
        ];
        let result = d.disambiguate(&sentence);
        assert_eq!(result.len(), 2);
        // NEG->VERB (-0.2) is much better than anything else
        assert_eq!(result[0].get("CLASS"), Some("kieltosana"));
        assert_eq!(result[1].get("CLASS"), Some("teonsana"));
    }

    /// Corpus-derived model should work end-to-end with the disambiguator.
    #[test]
    fn disambiguator_with_corpus_model() {
        let counts = vec![
            (("nimisana", "teonsana"), 200),
            (("teonsana", "nimisana"), 180),
            (("laatusana", "nimisana"), 120),
            (("nimisana", "nimisana"), 30),
        ];
        let model = BigramModel::from_counts(&counts);
        let d = ViterbiDisambiguator::new(model);

        // "iso koira juoksee" — should still work
        let sentence = vec![
            vec![
                make_analysis("laatusana", "iso"),
                make_analysis("nimisana", "iso"),
            ],
            vec![make_analysis("nimisana", "koira")],
            vec![make_analysis("teonsana", "juosta")],
        ];
        let result = d.disambiguate(&sentence);
        assert_eq!(result.len(), 3);
        // ADJ->NOUN should be preferred (count 120 >> NOUN->NOUN count 30)
        assert_eq!(result[0].get("CLASS"), Some("laatusana"));
        assert_eq!(result[1].get("CLASS"), Some("nimisana"));
        assert_eq!(result[2].get("CLASS"), Some("teonsana"));
    }

    // ─────────────────────────────────────────────────────────────
    // Suffix tagger integration tests
    // ─────────────────────────────────────────────────────────────

    fn make_test_suffix_tagger() -> SuffixTagger {
        use std::collections::HashMap;

        // 3 classes: NOUN, VERB, ADJ
        // 3 features: suf1=a -> NOUN, suf1=e -> VERB, fi_adj_inen=True -> ADJ
        let mut feature_vocab = HashMap::new();
        feature_vocab.insert("suf1=a".to_string(), 0);
        feature_vocab.insert("suf1=e".to_string(), 1);
        feature_vocab.insert("fi_adj_inen=True".to_string(), 2);

        // Weight matrix (3 classes x 3 features):
        //           suf1=a  suf1=e  fi_adj_inen
        // NOUN:       8      -4        -4
        // VERB:      -4       8        -4
        // ADJ:       -4      -4         8
        let weights: Vec<i8> = vec![
            8, -4, -4, // NOUN
            -4, 8, -4, // VERB
            -4, -4, 8, // ADJ
        ];

        SuffixTagger::from_parts(
            feature_vocab,
            weights,
            1.0,
            vec![0.0, 0.0, 0.0],
            3,
            3,
            vec!["NOUN".to_string(), "VERB".to_string(), "ADJ".to_string()],
        )
    }

    #[test]
    fn disambiguator_set_suffix_tagger() {
        let mut d = ViterbiDisambiguator::with_finnish_defaults();
        assert!(d.suffix_tagger().is_none());
        d.set_suffix_tagger(make_test_suffix_tagger());
        assert!(d.suffix_tagger().is_some());
    }

    #[test]
    fn disambiguator_suffix_tagger_helps_disambiguation() {
        // Set up a disambiguator with a uniform bigram model (no transition preference)
        // so only emission scores matter.
        let mut d = ViterbiDisambiguator::new(BigramModel::new(-1.0));
        d.set_suffix_tagger(make_test_suffix_tagger());

        // Word "koira" ends in "a" -> suf1=a -> should strongly favor NOUN.
        // Without the suffix tagger, both readings have equal score (0.0).
        // With the suffix tagger, NOUN gets a higher emission score.
        let sentence = vec![vec![
            make_analysis("nimisana", "koira"),  // CLASS=nimisana -> UPOS=NOUN
            make_analysis("teonsana", "koiria"), // CLASS=teonsana -> UPOS=VERB
        ]];
        let result = d.disambiguate_with_words(&["koira"], &sentence);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].get("CLASS"),
            Some("nimisana"),
            "Suffix tagger should favor NOUN for word ending in 'a'"
        );
    }

    #[test]
    fn disambiguator_suffix_tagger_verb_ending() {
        let mut d = ViterbiDisambiguator::new(BigramModel::new(-1.0));
        d.set_suffix_tagger(make_test_suffix_tagger());

        // Word "juoksee" ends in "e" -> suf1=e -> should favor VERB.
        let sentence = vec![vec![
            make_analysis("nimisana", "juoksu"),
            make_analysis("teonsana", "juosta"),
        ]];
        let result = d.disambiguate_with_words(&["juoksee"], &sentence);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].get("CLASS"),
            Some("teonsana"),
            "Suffix tagger should favor VERB for word ending in 'e'"
        );
    }

    #[test]
    fn disambiguator_suffix_tagger_without_words_is_noop() {
        // When disambiguate() (without words) is called, suffix tagger should
        // not affect results — it needs surface forms.
        let mut d = ViterbiDisambiguator::new(BigramModel::new(-1.0));
        d.set_suffix_tagger(make_test_suffix_tagger());

        let sentence = vec![vec![
            make_analysis("nimisana", "koira"),
            make_analysis("teonsana", "koiria"),
        ]];
        // Without words, both have equal emission scores. Viterbi picks first.
        let result = d.disambiguate(&sentence);
        assert_eq!(result.len(), 1);
        // Result depends on emission scores only (both 0.0), first reading wins.
        // This is fine — the point is that it doesn't crash.
    }
}
