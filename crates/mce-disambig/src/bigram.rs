//! Bigram transition model for POS-based disambiguation.
//!
//! Provides a simple POS bigram model that scores transitions between
//! consecutive morphological analyses. Weights are stored as log-probabilities
//! in a hash map keyed by `(prev_class, curr_class)` pairs.
//!
//! Finnish-specific defaults encode common POS transition patterns:
//! - NOUN -> VERB (subject-verb) is very common
//! - VERB -> NOUN (verb-object) is common
//! - DET  -> NOUN (determiner-noun) is common
//! - ADJ  -> NOUN (adjective-noun) is common
//! - NOUN -> NOUN (compounds) is moderately common

use std::collections::HashMap;

use mce_core::analysis::{Analysis, ATTR_CLASS};

/// A bigram transition model that scores POS tag pairs.
///
/// Uses `ATTR_CLASS` from each [`Analysis`] to look up transition weights.
/// Falls back to a configurable default weight for unseen bigrams.
#[derive(Debug, Clone)]
pub struct BigramModel {
    /// Transition weights keyed by (prev_class, curr_class).
    /// Values are log-probabilities (higher = more likely).
    weights: HashMap<(String, String), f64>,
    /// Default weight for unseen POS bigrams.
    default_weight: f64,
}

impl BigramModel {
    /// Create an empty bigram model with the given default weight.
    pub fn new(default_weight: f64) -> Self {
        Self {
            weights: HashMap::new(),
            default_weight,
        }
    }

    /// Create a bigram model pre-loaded with Finnish POS transition defaults.
    ///
    /// The weights are approximate log-probabilities derived from common
    /// Finnish sentence patterns. These serve as a reasonable baseline
    /// until corpus-derived or TT-core weights are available.
    pub fn finnish_defaults() -> Self {
        let mut model = Self::new(-3.0);

        // Very common transitions (Finnish SVO/SOV flexible order)
        model.set_weight("nimisana", "teonsana", -0.3); // NOUN -> VERB (subject-verb)
        model.set_weight("teonsana", "nimisana", -0.5); // VERB -> NOUN (verb-object)
        model.set_weight("laatusana", "nimisana", -0.2); // ADJ -> NOUN (modifier-head)
        model.set_weight("nimisana", "nimisana", -0.8); // NOUN -> NOUN (compounds, apposition)

        // Common transitions
        model.set_weight("teonsana", "laatusana", -0.7); // VERB -> ADJ (predicate adj)
        model.set_weight("teonsana", "seikkasana", -0.6); // VERB -> ADV (adverbial)
        model.set_weight("seikkasana", "teonsana", -0.5); // ADV -> VERB
        model.set_weight("seikkasana", "nimisana", -0.8); // ADV -> NOUN (adverb before NP)
        model.set_weight("seikkasana", "laatusana", -0.7); // ADV -> ADJ (degree modifier)
        model.set_weight("seikkasana", "seikkasana", -1.0); // ADV -> ADV
        model.set_weight("nimisana", "laatusana", -1.5); // NOUN -> ADJ (less common)
        model.set_weight("nimisana", "seikkasana", -1.0); // NOUN -> ADV

        // Numeral transitions
        model.set_weight("lukusana", "nimisana", -0.4); // NUM -> NOUN (e.g., "kuusi koiraa")
        model.set_weight("nimisana", "lukusana", -1.5); // NOUN -> NUM (less common)
        model.set_weight("lukusana", "lukusana", -1.0); // NUM -> NUM (compound numbers)

        // Pronoun transitions
        model.set_weight("asemosana", "teonsana", -0.3); // PRON -> VERB
        model.set_weight("asemosana", "nimisana", -0.8); // PRON -> NOUN
        model.set_weight("teonsana", "asemosana", -0.9); // VERB -> PRON

        // Conjunction transitions
        model.set_weight("sidesana", "nimisana", -0.4); // CONJ -> NOUN
        model.set_weight("sidesana", "teonsana", -0.5); // CONJ -> VERB
        model.set_weight("nimisana", "sidesana", -0.6); // NOUN -> CONJ
        model.set_weight("teonsana", "sidesana", -0.7); // VERB -> CONJ

        // Postposition/preposition transitions
        model.set_weight("nimisana", "suhdesana", -0.5); // NOUN -> ADPOS
        model.set_weight("suhdesana", "nimisana", -0.5); // ADPOS -> NOUN

        model
    }

    /// Set the transition weight for a specific POS bigram.
    pub fn set_weight(&mut self, prev_class: &str, curr_class: &str, weight: f64) {
        self.weights
            .insert((prev_class.to_string(), curr_class.to_string()), weight);
    }

    /// Get the transition weight for a POS bigram, or the default if unseen.
    pub fn get_weight(&self, prev_class: &str, curr_class: &str) -> f64 {
        self.weights
            .get(&(prev_class.to_string(), curr_class.to_string()))
            .copied()
            .unwrap_or(self.default_weight)
    }

    /// Score the transition between two analyses using their CLASS attribute.
    ///
    /// If either analysis lacks the CLASS attribute, returns the default weight.
    pub fn score(&self, prev: &Analysis, curr: &Analysis) -> f64 {
        let prev_class = prev.get(ATTR_CLASS).unwrap_or("");
        let curr_class = curr.get(ATTR_CLASS).unwrap_or("");

        if prev_class.is_empty() || curr_class.is_empty() {
            return self.default_weight;
        }

        self.get_weight(prev_class, curr_class)
    }

    /// Convert this model into a boxed [`TransitionFn`](crate::viterbi::TransitionFn).
    ///
    /// Useful for passing directly to the Viterbi decoder.
    pub fn as_transition_fn(&self) -> Box<crate::viterbi::TransitionFn> {
        let weights = self.weights.clone();
        let default = self.default_weight;
        Box::new(move |prev: &Analysis, curr: &Analysis| {
            let prev_class = prev.get(ATTR_CLASS).unwrap_or("");
            let curr_class = curr.get(ATTR_CLASS).unwrap_or("");

            if prev_class.is_empty() || curr_class.is_empty() {
                return default;
            }

            weights
                .get(&(prev_class.to_string(), curr_class.to_string()))
                .copied()
                .unwrap_or(default)
        })
    }

    /// Number of explicitly defined bigram weights.
    pub fn num_weights(&self) -> usize {
        self.weights.len()
    }

    /// The default weight for unseen bigrams.
    pub fn default_weight(&self) -> f64 {
        self.default_weight
    }
}

impl Default for BigramModel {
    fn default() -> Self {
        Self::finnish_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_analysis(class: &str, baseform: &str) -> Analysis {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, class);
        a.set("BASEFORM", baseform);
        a
    }

    #[test]
    fn empty_model_uses_default() {
        let model = BigramModel::new(-5.0);
        let a = make_analysis("nimisana", "koira");
        let b = make_analysis("teonsana", "juosta");
        assert!((model.score(&a, &b) - (-5.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn set_and_get_weight() {
        let mut model = BigramModel::new(-5.0);
        model.set_weight("nimisana", "teonsana", -0.3);
        assert!((model.get_weight("nimisana", "teonsana") - (-0.3)).abs() < f64::EPSILON);
        // Unseen bigram still returns default
        assert!((model.get_weight("teonsana", "nimisana") - (-5.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn score_uses_class_attr() {
        let mut model = BigramModel::new(-5.0);
        model.set_weight("nimisana", "teonsana", -0.3);

        let noun = make_analysis("nimisana", "koira");
        let verb = make_analysis("teonsana", "juosta");
        assert!((model.score(&noun, &verb) - (-0.3)).abs() < f64::EPSILON);
    }

    #[test]
    fn missing_class_returns_default() {
        let model = BigramModel::new(-5.0);
        let with_class = make_analysis("nimisana", "koira");
        let without_class = Analysis::new(); // no CLASS attribute
        assert!((model.score(&with_class, &without_class) - (-5.0)).abs() < f64::EPSILON);
        assert!((model.score(&without_class, &with_class) - (-5.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn finnish_defaults_has_common_transitions() {
        let model = BigramModel::finnish_defaults();
        assert!(model.num_weights() > 0);

        // NOUN -> VERB should be better than default
        let noun_verb = model.get_weight("nimisana", "teonsana");
        assert!(noun_verb > model.default_weight());

        // ADJ -> NOUN should be even better
        let adj_noun = model.get_weight("laatusana", "nimisana");
        assert!(adj_noun > model.default_weight());
    }

    #[test]
    fn as_transition_fn_works() {
        let mut model = BigramModel::new(-5.0);
        model.set_weight("nimisana", "teonsana", -0.3);

        let tf = model.as_transition_fn();

        let noun = make_analysis("nimisana", "koira");
        let verb = make_analysis("teonsana", "juosta");
        assert!((tf(&noun, &verb) - (-0.3)).abs() < f64::EPSILON);

        // Unseen transition
        assert!((tf(&verb, &noun) - (-5.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn overwrite_weight() {
        let mut model = BigramModel::new(-5.0);
        model.set_weight("nimisana", "teonsana", -0.3);
        model.set_weight("nimisana", "teonsana", -0.1);
        assert!((model.get_weight("nimisana", "teonsana") - (-0.1)).abs() < f64::EPSILON);
    }

    #[test]
    fn num_weights() {
        let mut model = BigramModel::new(-5.0);
        assert_eq!(model.num_weights(), 0);
        model.set_weight("A", "B", -1.0);
        assert_eq!(model.num_weights(), 1);
        model.set_weight("A", "B", -2.0); // overwrite, not new
        assert_eq!(model.num_weights(), 1);
        model.set_weight("B", "A", -1.0);
        assert_eq!(model.num_weights(), 2);
    }

    #[test]
    fn default_model_is_finnish() {
        let model = BigramModel::default();
        // Should have Finnish-specific weights
        assert!(model.num_weights() > 10);
    }
}
