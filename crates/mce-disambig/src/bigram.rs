//! Bigram transition model for POS-based disambiguation.
//!
//! Provides a POS bigram model that scores transitions between consecutive
//! morphological analyses. Weights are stored as log-probabilities in a hash
//! map keyed by `(prev_class, curr_class)` pairs.
//!
//! Finnish-specific defaults encode common POS transition patterns derived
//! from linguistic knowledge of Finnish word order:
//! - Finnish is relatively free word order but SVO is the default
//! - Adjectives/numerals precede nouns
//! - Verbs follow subjects (nouns/pronouns)
//! - Postpositions follow nouns
//! - Negation verbs precede main verbs
//!
//! # Corpus-weight infrastructure
//!
//! [`BigramModel::from_counts`] builds a model from raw bigram counts
//! (e.g. extracted from UD Finnish-TDT). Counts are converted to
//! log-probabilities with add-one (Laplace) smoothing.
//!
//! # Emission scoring
//!
//! [`EmissionScorer`] adjusts per-reading scores based on morphological
//! features: baseform match, analysis simplicity (STRUCTURE length),
//! and configurable bonuses.

use std::collections::HashMap;

use mce_core::analysis::{Analysis, ATTR_BASEFORM, ATTR_CLASS, ATTR_STRUCTURE};

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
    /// The weights are approximate log-probabilities derived from linguistic
    /// knowledge of Finnish sentence structure. Finnish POS classes used:
    ///
    /// - `nimisana` (noun)
    /// - `teonsana` (verb)
    /// - `laatusana` (adjective)
    /// - `seikkasana` (adverb)
    /// - `lukusana` (numeral)
    /// - `asemosana` (pronoun)
    /// - `sidesana` (conjunction)
    /// - `suhdesana` (adposition — pre/postposition)
    /// - `kieltosana` (negation verb)
    /// - `huudahdussana` (interjection)
    ///
    /// These serve as a reasonable baseline until corpus-derived weights
    /// are available via [`BigramModel::from_counts`].
    pub fn finnish_defaults() -> Self {
        let mut model = Self::new(-3.0);

        // ─────────────────────────────────────────────────────────
        // 1. Subject–verb patterns (SVO default, but flexible)
        // ─────────────────────────────────────────────────────────
        model.set_weight("nimisana", "teonsana", -0.3); // NOUN -> VERB (subject-verb)
        model.set_weight("teonsana", "nimisana", -0.5); // VERB -> NOUN (verb-object)
        model.set_weight("asemosana", "teonsana", -0.3); // PRON -> VERB (hän juoksee)
        model.set_weight("teonsana", "asemosana", -0.9); // VERB -> PRON (näkee hänet)

        // ─────────────────────────────────────────────────────────
        // 2. Modifier–head patterns
        // ─────────────────────────────────────────────────────────
        model.set_weight("laatusana", "nimisana", -0.2); // ADJ -> NOUN (iso koira)
        model.set_weight("lukusana", "nimisana", -0.4); // NUM -> NOUN (kuusi koiraa)
        model.set_weight("asemosana", "nimisana", -0.6); // PRON -> NOUN (minun koira)
        model.set_weight("nimisana", "nimisana", -0.8); // NOUN -> NOUN (compound/apposition)
        model.set_weight("nimisana", "laatusana", -1.5); // NOUN -> ADJ (less common in Finnish)
        model.set_weight("lukusana", "laatusana", -1.2); // NUM -> ADJ (rare)

        // ─────────────────────────────────────────────────────────
        // 3. Adverb patterns
        // ─────────────────────────────────────────────────────────
        model.set_weight("seikkasana", "teonsana", -0.5); // ADV -> VERB (nopeasti juoksee)
        model.set_weight("teonsana", "seikkasana", -0.6); // VERB -> ADV (juoksee nopeasti)
        model.set_weight("seikkasana", "laatusana", -0.7); // ADV -> ADJ (erittäin iso)
        model.set_weight("seikkasana", "nimisana", -0.8); // ADV -> NOUN (myös koira)
        model.set_weight("seikkasana", "seikkasana", -1.0); // ADV -> ADV (hyvin nopeasti)
        model.set_weight("seikkasana", "asemosana", -1.2); // ADV -> PRON (myös hän)
        model.set_weight("nimisana", "seikkasana", -1.0); // NOUN -> ADV
        model.set_weight("laatusana", "seikkasana", -1.3); // ADJ -> ADV (rare)

        // ─────────────────────────────────────────────────────────
        // 4. Verb–verb and predicate patterns
        // ─────────────────────────────────────────────────────────
        model.set_weight("teonsana", "teonsana", -0.7); // VERB -> VERB (auxiliary chains: on tehnyt)
        model.set_weight("teonsana", "laatusana", -0.7); // VERB -> ADJ (predicate adj: on iso)
        model.set_weight("laatusana", "teonsana", -0.9); // ADJ -> VERB (iso juoksee — less common)

        // ─────────────────────────────────────────────────────────
        // 5. Numeral patterns
        // ─────────────────────────────────────────────────────────
        model.set_weight("lukusana", "lukusana", -1.0); // NUM -> NUM (compound numbers)
        model.set_weight("nimisana", "lukusana", -1.5); // NOUN -> NUM (rare)
        model.set_weight("lukusana", "teonsana", -0.8); // NUM -> VERB (kuusi kasvaa — num subject)
        model.set_weight("teonsana", "lukusana", -1.3); // VERB -> NUM (rare)

        // ─────────────────────────────────────────────────────────
        // 6. Conjunction patterns (sidesana connects clauses)
        // ─────────────────────────────────────────────────────────
        model.set_weight("sidesana", "nimisana", -0.4); // CONJ -> NOUN (ja koira)
        model.set_weight("sidesana", "asemosana", -0.5); // CONJ -> PRON (ja hän)
        model.set_weight("sidesana", "teonsana", -0.5); // CONJ -> VERB (ja juoksee)
        model.set_weight("sidesana", "laatusana", -0.6); // CONJ -> ADJ (ja iso)
        model.set_weight("sidesana", "seikkasana", -0.8); // CONJ -> ADV (ja nopeasti)
        model.set_weight("sidesana", "lukusana", -0.9); // CONJ -> NUM (ja kuusi)
        model.set_weight("nimisana", "sidesana", -0.6); // NOUN -> CONJ (koira ja)
        model.set_weight("teonsana", "sidesana", -0.7); // VERB -> CONJ (juoksee ja)
        model.set_weight("laatusana", "sidesana", -0.8); // ADJ -> CONJ (iso ja)
        model.set_weight("seikkasana", "sidesana", -1.0); // ADV -> CONJ
        model.set_weight("lukusana", "sidesana", -1.0); // NUM -> CONJ

        // ─────────────────────────────────────────────────────────
        // 7. Adposition patterns (suhdesana — mostly postpositions)
        // ─────────────────────────────────────────────────────────
        model.set_weight("nimisana", "suhdesana", -0.5); // NOUN -> ADPOS (talon edessä)
        model.set_weight("suhdesana", "nimisana", -0.5); // ADPOS -> NOUN (preposition case)
        model.set_weight("asemosana", "suhdesana", -0.6); // PRON -> ADPOS (hänen luonaan)
        model.set_weight("suhdesana", "teonsana", -0.8); // ADPOS -> VERB (edessä seisoo)
        model.set_weight("suhdesana", "asemosana", -1.0); // ADPOS -> PRON (rare)

        // ─────────────────────────────────────────────────────────
        // 8. Negation patterns (kieltosana — ei, en, et, ...)
        // ─────────────────────────────────────────────────────────
        model.set_weight("kieltosana", "teonsana", -0.2); // NEG -> VERB (ei juokse — very strong)
        model.set_weight("kieltosana", "laatusana", -1.0); // NEG -> ADJ (rare, e.g. ei iso)
        model.set_weight("kieltosana", "nimisana", -1.5); // NEG -> NOUN (rare)
        model.set_weight("nimisana", "kieltosana", -0.7); // NOUN -> NEG (koira ei)
        model.set_weight("asemosana", "kieltosana", -0.6); // PRON -> NEG (hän ei)
        model.set_weight("seikkasana", "kieltosana", -0.9); // ADV -> NEG (usein ei)
        model.set_weight("sidesana", "kieltosana", -0.7); // CONJ -> NEG (ja ei — eikä)

        // ─────────────────────────────────────────────────────────
        // 9. Interjection patterns (huudahdussana)
        // ─────────────────────────────────────────────────────────
        model.set_weight("huudahdussana", "nimisana", -1.0); // INTJ -> NOUN (voi koira)
        model.set_weight("huudahdussana", "teonsana", -1.0); // INTJ -> VERB
        model.set_weight("huudahdussana", "asemosana", -1.0); // INTJ -> PRON
        model.set_weight("huudahdussana", "sidesana", -1.5); // INTJ -> CONJ

        // ─────────────────────────────────────────────────────────
        // 10. Pronoun additional patterns
        // ─────────────────────────────────────────────────────────
        model.set_weight("asemosana", "asemosana", -1.5); // PRON -> PRON (rare)
        model.set_weight("asemosana", "laatusana", -0.8); // PRON -> ADJ (se iso...)
        model.set_weight("asemosana", "seikkasana", -1.0); // PRON -> ADV
        model.set_weight("asemosana", "sidesana", -1.2); // PRON -> CONJ
        model.set_weight("laatusana", "asemosana", -1.5); // ADJ -> PRON (rare)
        model.set_weight("laatusana", "laatusana", -1.2); // ADJ -> ADJ (iso punainen)

        model
    }

    /// Build a bigram model from raw transition counts.
    ///
    /// Converts raw bigram counts (e.g. extracted from a UD treebank) into
    /// log-probabilities using Laplace (add-one) smoothing.
    ///
    /// The smoothing ensures that unseen bigrams receive a non-zero probability.
    /// The formula is: `log(count(a,b) + 1) - log(count(a,*) + V)` where V is
    /// the vocabulary size (number of distinct POS tags observed).
    ///
    /// # Arguments
    ///
    /// * `transitions` - Slice of `((prev_class, curr_class), count)` pairs.
    ///
    /// # Example
    ///
    /// ```
    /// use mce_disambig::bigram::BigramModel;
    ///
    /// let counts = vec![
    ///     (("nimisana", "teonsana"), 150),
    ///     (("teonsana", "nimisana"), 120),
    ///     (("laatusana", "nimisana"), 80),
    /// ];
    /// let model = BigramModel::from_counts(&counts);
    /// assert!(model.get_weight("nimisana", "teonsana") > model.get_weight("laatusana", "nimisana"));
    /// ```
    pub fn from_counts(transitions: &[((&str, &str), u64)]) -> Self {
        // Collect all distinct POS tags for vocabulary size.
        let mut tags: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for &((prev, curr), _) in transitions {
            tags.insert(prev);
            tags.insert(curr);
        }
        let vocab_size = tags.len().max(1) as f64;

        // Sum counts per prev_class for normalization.
        let mut prev_totals: HashMap<String, u64> = HashMap::new();
        for &((prev, _), count) in transitions {
            *prev_totals.entry(prev.to_string()).or_insert(0) += count;
        }

        // Compute log-probabilities with Laplace smoothing.
        let mut model = Self::new(0.0);

        for &((prev, curr), count) in transitions {
            let total = prev_totals[prev];
            // P(curr | prev) = (count + 1) / (total + V)
            let log_prob = ((count as f64) + 1.0).ln() - ((total as f64) + vocab_size).ln();
            model.set_weight(prev, curr, log_prob);
        }

        // Default weight: log(1 / (avg_total + V)) — smoothed probability for
        // unseen bigrams from an "average" predecessor.
        let avg_total = if prev_totals.is_empty() {
            0.0
        } else {
            prev_totals.values().sum::<u64>() as f64 / prev_totals.len() as f64
        };
        model.default_weight = (1.0_f64).ln() - (avg_total + vocab_size).ln();

        model
    }

    /// Build a bigram model from a `HashMap` of transition counts.
    ///
    /// Like [`from_counts`](Self::from_counts) but accepts owned `String`
    /// keys in a `HashMap`, which is the natural output of corpus extraction
    /// tools such as [`corpus::parse_conllu_pos_bigrams`](crate::corpus::parse_conllu_pos_bigrams).
    ///
    /// # Example
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use mce_disambig::bigram::BigramModel;
    ///
    /// let mut counts = HashMap::new();
    /// counts.insert(("NOUN".to_string(), "VERB".to_string()), 150);
    /// counts.insert(("VERB".to_string(), "NOUN".to_string()), 120);
    /// let model = BigramModel::from_counts_map(&counts);
    /// assert!(model.num_weights() > 0);
    /// ```
    pub fn from_counts_map(counts: &HashMap<(String, String), u64>) -> Self {
        let transitions: Vec<((&str, &str), u64)> = counts
            .iter()
            .map(|((prev, curr), &count)| ((prev.as_str(), curr.as_str()), count))
            .collect();
        Self::from_counts(&transitions)
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

// ─────────────────────────────────────────────────────────────────────────────
// Emission scoring
// ─────────────────────────────────────────────────────────────────────────────

/// Emission scorer that adjusts per-reading scores based on morphological features.
///
/// The emission score supplements the Viterbi lattice emission (which may come
/// from FST weights) with linguistically motivated heuristics:
///
/// - **Baseform match bonus**: When the surface form equals the baseform,
///   the analysis is more likely (no inflection = simpler reading).
/// - **Structure length penalty**: Shorter STRUCTURE values suggest simpler
///   morphological composition, which is generally preferred.
/// - **Word-level POS priors**: When corpus-derived P(UPOS|word) priors are
///   available, analyses whose POS class matches a high-probability UPOS for
///   the given word receive a bonus. This is the strongest disambiguation
///   signal for unambiguous or low-ambiguity words.
///
/// Scores are additive adjustments (in log-probability space).
#[derive(Debug, Clone)]
pub struct EmissionScorer {
    /// Bonus added when the surface form exactly matches BASEFORM.
    pub baseform_match_bonus: f64,
    /// Penalty per character of STRUCTURE length beyond 1.
    /// Applied as: `-(len - 1) * penalty` (only when len > 1).
    pub structure_length_penalty: f64,
    /// Corpus-derived P(UPOS | word_form) priors.
    ///
    /// Outer key: lowercased word form.
    /// Inner key: UPOS tag (e.g. "NOUN", "VERB").
    /// Inner value: probability (0.0..1.0).
    word_pos_priors: HashMap<String, HashMap<String, f64>>,
    /// Weight applied to the log-prior: `prior_weight * ln(P(UPOS|word))`.
    /// Higher values make the prior more influential.
    pub prior_weight: f64,
    /// Penalty applied when the analysis maps to a UPOS category not seen
    /// in the prior distribution for this word. Should be negative.
    pub prior_unseen_penalty: f64,
}

impl EmissionScorer {
    /// Create an emission scorer with the given parameters.
    pub fn new(baseform_match_bonus: f64, structure_length_penalty: f64) -> Self {
        Self {
            baseform_match_bonus,
            structure_length_penalty,
            word_pos_priors: HashMap::new(),
            prior_weight: 0.0,
            prior_unseen_penalty: 0.0,
        }
    }

    /// Create an emission scorer with reasonable Finnish defaults.
    ///
    /// - Baseform match bonus: +0.5 (moderate preference for uninflected forms)
    /// - Structure length penalty: 0.1 per extra character (mild preference for
    ///   simpler analyses)
    /// - No word-POS priors (must be set separately via [`set_word_pos_priors`]).
    pub fn finnish_defaults() -> Self {
        Self {
            baseform_match_bonus: 0.5,
            structure_length_penalty: 0.1,
            word_pos_priors: HashMap::new(),
            prior_weight: 2.0,
            prior_unseen_penalty: -3.0,
        }
    }

    /// Set word-level POS priors from corpus data.
    ///
    /// The priors map lowercased word forms to their UPOS probability
    /// distributions: `{ "koira" => { "NOUN" => 1.0 }, "kuusi" => { "NOUN" => 0.67, "NUM" => 0.33 } }`.
    ///
    /// These are typically extracted from a CoNLL-U treebank via
    /// [`corpus::extract_emission_priors`](crate::corpus::extract_emission_priors).
    pub fn set_word_pos_priors(&mut self, priors: HashMap<String, HashMap<String, f64>>) {
        self.word_pos_priors = priors;
    }

    /// Check whether word-level POS priors have been loaded.
    pub fn has_word_pos_priors(&self) -> bool {
        !self.word_pos_priors.is_empty()
    }

    /// Number of word forms with POS priors.
    pub fn num_word_priors(&self) -> usize {
        self.word_pos_priors.len()
    }

    /// Score an analysis for a given surface word.
    ///
    /// Returns an additive adjustment to the emission log-probability.
    /// A positive value boosts the reading; negative penalizes it.
    ///
    /// When word-level POS priors are available, the score includes a
    /// bonus proportional to `prior_weight * ln(P(UPOS|word))` for matching
    /// POS categories, or `prior_unseen_penalty` for categories not observed
    /// in the training data.
    ///
    /// # Arguments
    ///
    /// * `word` - The surface form of the word being analyzed.
    /// * `analysis` - The candidate morphological analysis.
    pub fn score(&self, word: &str, analysis: &Analysis) -> f64 {
        let mut adjustment = 0.0;

        // Baseform match: if BASEFORM == surface form, the word is likely
        // in its base (uninflected) form, which is a common reading.
        if let Some(baseform) = analysis.get(ATTR_BASEFORM) {
            if baseform == word {
                adjustment += self.baseform_match_bonus;
            }
        }

        // Structure length penalty: shorter STRUCTURE means simpler
        // morphological decomposition. STRUCTURE encodes the morpheme
        // pattern (e.g., "=pp" for a simple word, "=pp=pp" for a compound).
        if let Some(structure) = analysis.get(ATTR_STRUCTURE) {
            let len = structure.len();
            if len > 1 {
                adjustment -= (len as f64 - 1.0) * self.structure_length_penalty;
            }
        }

        // Word-level POS prior: if we have corpus-derived P(UPOS|word),
        // boost analyses whose CLASS maps to a high-probability UPOS.
        if !self.word_pos_priors.is_empty() {
            let lower = word
                .chars()
                .map(|c| c.to_lowercase().next().unwrap_or(c))
                .collect::<String>();

            if let Some(prior_map) = self.word_pos_priors.get(&lower) {
                let predicted_upos = class_to_upos_category(analysis.get(ATTR_CLASS).unwrap_or(""));

                if let Some(&prob) = prior_map.get(predicted_upos) {
                    // Bonus: prior_weight * ln(P(UPOS|word))
                    // For prob=1.0 this gives 0 bonus; for prob=0.5 gives -0.69*weight.
                    adjustment += self.prior_weight * prob.ln();
                } else {
                    // This POS category was never seen for this word in training data.
                    adjustment += self.prior_unseen_penalty;
                }
            }
        }

        adjustment
    }
}

/// Map an MCE CLASS value to a UPOS category string for emission prior scoring.
///
/// This is a lightweight mapping used inside [`EmissionScorer`] to match
/// analysis CLASS values against the UPOS tags in the corpus-derived priors.
/// It intentionally mirrors the basic mapping in `mce-eval::pos_map` but
/// lives here to avoid a circular dependency between `mce-disambig` and `mce-eval`.
pub fn class_to_upos_category(class: &str) -> &'static str {
    match class {
        "nimisana" | "lyhenne" => "NOUN",
        "nimisana_laatusana" => "ADJ",
        "teonsana" => "VERB",
        "laatusana" => "ADJ",
        "seikkasana" => "ADV",
        "asemosana" => "PRON",
        "lukusana" => "NUM",
        "etunimi" | "sukunimi" | "paikannimi" => "PROPN",
        "kieltosana" => "AUX",
        "suhdesana" => "ADP",
        "sidesana" => "CCONJ",
        "huudahdussana" => "INTJ",
        _ => "X",
    }
}

impl Default for EmissionScorer {
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
        a.set(ATTR_BASEFORM, baseform);
        a
    }

    fn make_full_analysis(class: &str, baseform: &str, structure: &str) -> Analysis {
        let mut a = make_analysis(class, baseform);
        a.set(ATTR_STRUCTURE, structure);
        a
    }

    // ─────────────────────────────────────────────────────────────
    // BigramModel: basic tests
    // ─────────────────────────────────────────────────────────────

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

    // ─────────────────────────────────────────────────────────────
    // Finnish defaults: expanded coverage
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn finnish_defaults_has_many_transitions() {
        let model = BigramModel::finnish_defaults();
        // We define at least 50 transitions now.
        assert!(
            model.num_weights() >= 50,
            "Expected >= 50 weights, got {}",
            model.num_weights()
        );
    }

    #[test]
    fn default_model_is_finnish() {
        let model = BigramModel::default();
        assert!(model.num_weights() >= 50);
    }

    #[test]
    fn finnish_defaults_noun_verb_better_than_default() {
        let model = BigramModel::finnish_defaults();
        let noun_verb = model.get_weight("nimisana", "teonsana");
        assert!(noun_verb > model.default_weight());
    }

    #[test]
    fn finnish_defaults_adj_noun_better_than_default() {
        let model = BigramModel::finnish_defaults();
        let adj_noun = model.get_weight("laatusana", "nimisana");
        assert!(adj_noun > model.default_weight());
    }

    #[test]
    fn finnish_defaults_negation_verb_is_strong() {
        let model = BigramModel::finnish_defaults();
        // NEG -> VERB is the strongest transition for negation
        let neg_verb = model.get_weight("kieltosana", "teonsana");
        let neg_noun = model.get_weight("kieltosana", "nimisana");
        assert!(
            neg_verb > neg_noun,
            "NEG->VERB should be stronger than NEG->NOUN"
        );
    }

    #[test]
    fn finnish_defaults_conj_starts_new_clause() {
        let model = BigramModel::finnish_defaults();
        let conj_noun = model.get_weight("sidesana", "nimisana");
        let conj_verb = model.get_weight("sidesana", "teonsana");
        let conj_pron = model.get_weight("sidesana", "asemosana");
        // All should be better than default
        assert!(conj_noun > model.default_weight());
        assert!(conj_verb > model.default_weight());
        assert!(conj_pron > model.default_weight());
    }

    #[test]
    fn finnish_defaults_postposition_follows_noun() {
        let model = BigramModel::finnish_defaults();
        let noun_adpos = model.get_weight("nimisana", "suhdesana");
        let pron_adpos = model.get_weight("asemosana", "suhdesana");
        assert!(noun_adpos > model.default_weight());
        assert!(pron_adpos > model.default_weight());
    }

    #[test]
    fn finnish_defaults_adj_precedes_noun_preferred() {
        let model = BigramModel::finnish_defaults();
        // ADJ->NOUN should be better than NOUN->ADJ in Finnish
        let adj_noun = model.get_weight("laatusana", "nimisana");
        let noun_adj = model.get_weight("nimisana", "laatusana");
        assert!(
            adj_noun > noun_adj,
            "ADJ->NOUN ({}) should be stronger than NOUN->ADJ ({})",
            adj_noun,
            noun_adj
        );
    }

    #[test]
    fn finnish_defaults_verb_verb_auxiliary_chain() {
        let model = BigramModel::finnish_defaults();
        let verb_verb = model.get_weight("teonsana", "teonsana");
        assert!(verb_verb > model.default_weight());
    }

    #[test]
    fn finnish_defaults_all_ten_pos_classes_covered() {
        let model = BigramModel::finnish_defaults();
        let classes = [
            "nimisana",
            "teonsana",
            "laatusana",
            "seikkasana",
            "lukusana",
            "asemosana",
            "sidesana",
            "suhdesana",
            "kieltosana",
            "huudahdussana",
        ];
        // Every class should appear as either prev or curr in at least one weight
        for class in &classes {
            let has_weight = (0..model.num_weights()).any(|_| {
                // Check both directions
                classes
                    .iter()
                    .any(|other| model.get_weight(class, other) != model.default_weight())
                    || classes
                        .iter()
                        .any(|other| model.get_weight(other, class) != model.default_weight())
            });
            assert!(
                has_weight,
                "POS class '{}' should appear in at least one transition",
                class
            );
        }
    }

    // ─────────────────────────────────────────────────────────────
    // from_counts: corpus-weight infrastructure
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn from_counts_simple() {
        let counts = vec![
            (("nimisana", "teonsana"), 100),
            (("teonsana", "nimisana"), 80),
            (("laatusana", "nimisana"), 50),
        ];
        let model = BigramModel::from_counts(&counts);

        // Higher counts should produce higher weights
        let nt = model.get_weight("nimisana", "teonsana");
        let ln = model.get_weight("laatusana", "nimisana");
        assert!(
            nt > ln,
            "nimisana->teonsana ({}) should be higher than laatusana->nimisana ({})",
            nt,
            ln
        );
    }

    #[test]
    fn from_counts_preserves_relative_order() {
        let counts = vec![(("A", "B"), 500), (("A", "C"), 100), (("A", "D"), 10)];
        let model = BigramModel::from_counts(&counts);

        let ab = model.get_weight("A", "B");
        let ac = model.get_weight("A", "C");
        let ad = model.get_weight("A", "D");

        assert!(ab > ac, "A->B should be higher than A->C");
        assert!(ac > ad, "A->C should be higher than A->D");
    }

    #[test]
    fn from_counts_all_weights_are_negative() {
        // Log-probabilities should be negative (probabilities < 1).
        let counts = vec![(("A", "B"), 100), (("A", "C"), 50), (("B", "A"), 80)];
        let model = BigramModel::from_counts(&counts);

        let ab = model.get_weight("A", "B");
        let ac = model.get_weight("A", "C");
        let ba = model.get_weight("B", "A");
        assert!(ab < 0.0, "log-prob should be negative, got {}", ab);
        assert!(ac < 0.0, "log-prob should be negative, got {}", ac);
        assert!(ba < 0.0, "log-prob should be negative, got {}", ba);
    }

    #[test]
    fn from_counts_unseen_bigram_gets_default() {
        let counts = vec![(("A", "B"), 100), (("B", "A"), 80)];
        let model = BigramModel::from_counts(&counts);

        let unseen = model.get_weight("A", "C");
        let seen = model.get_weight("A", "B");
        assert!(
            seen > unseen,
            "Seen bigram ({}) should be better than unseen ({})",
            seen,
            unseen
        );
    }

    #[test]
    fn from_counts_empty_input() {
        let counts: Vec<((&str, &str), u64)> = vec![];
        let model = BigramModel::from_counts(&counts);
        assert_eq!(model.num_weights(), 0);
    }

    #[test]
    fn from_counts_single_transition() {
        let counts = vec![(("nimisana", "teonsana"), 42)];
        let model = BigramModel::from_counts(&counts);
        assert_eq!(model.num_weights(), 1);
        let w = model.get_weight("nimisana", "teonsana");
        assert!(w < 0.0);
        assert!(w > model.default_weight());
    }

    // ─────────────────────────────────────────────────────────────
    // EmissionScorer
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn emission_scorer_baseform_match_bonus() {
        let scorer = EmissionScorer::finnish_defaults();

        let a = make_analysis("nimisana", "koira");
        let score_match = scorer.score("koira", &a);
        let score_no_match = scorer.score("koiran", &a);

        assert!(
            score_match > score_no_match,
            "Baseform match ({}) should score higher than non-match ({})",
            score_match,
            score_no_match
        );
    }

    #[test]
    fn emission_scorer_exact_baseform_bonus_value() {
        let scorer = EmissionScorer::new(1.0, 0.0);

        let a = make_analysis("nimisana", "koira");
        let score = scorer.score("koira", &a);
        assert!(
            (score - 1.0).abs() < f64::EPSILON,
            "Expected bonus of 1.0, got {}",
            score
        );
    }

    #[test]
    fn emission_scorer_structure_penalty() {
        let scorer = EmissionScorer::new(0.0, 0.1);

        let simple = make_full_analysis("nimisana", "koira", "=pp");
        let compound = make_full_analysis("nimisana", "kissanpentu", "=pp=pp");

        let score_simple = scorer.score("koira", &simple);
        let score_compound = scorer.score("kissanpentu", &compound);

        assert!(
            score_simple > score_compound,
            "Simpler structure ({}) should score higher than compound ({})",
            score_simple,
            score_compound
        );
    }

    #[test]
    fn emission_scorer_structure_penalty_value() {
        let scorer = EmissionScorer::new(0.0, 0.1);

        // Structure "=pp" has length 3, so penalty = -(3-1) * 0.1 = -0.2
        let a = make_full_analysis("nimisana", "koira", "=pp");
        let score = scorer.score("koira", &a);
        assert!(
            (score - (-0.2)).abs() < f64::EPSILON,
            "Expected -0.2, got {}",
            score
        );
    }

    #[test]
    fn emission_scorer_no_baseform_no_structure() {
        let scorer = EmissionScorer::finnish_defaults();

        // Analysis with only CLASS, no BASEFORM or STRUCTURE
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "nimisana");
        let score = scorer.score("koira", &a);
        assert!(
            score.abs() < f64::EPSILON,
            "No BASEFORM or STRUCTURE should yield 0.0, got {}",
            score
        );
    }

    #[test]
    fn emission_scorer_combined_bonus_and_penalty() {
        let scorer = EmissionScorer::new(1.0, 0.1);

        // Baseform matches + structure "=pp" (len=3 -> penalty -0.2)
        let a = make_full_analysis("nimisana", "koira", "=pp");
        let score = scorer.score("koira", &a);
        // bonus(1.0) + penalty(-0.2) = 0.8
        assert!(
            (score - 0.8).abs() < f64::EPSILON,
            "Expected 0.8, got {}",
            score
        );
    }

    #[test]
    fn emission_scorer_default_is_finnish() {
        let scorer = EmissionScorer::default();
        assert!((scorer.baseform_match_bonus - 0.5).abs() < f64::EPSILON);
        assert!((scorer.structure_length_penalty - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn emission_scorer_zero_penalty_for_single_char_structure() {
        let scorer = EmissionScorer::new(0.0, 0.1);

        // Structure "p" has length 1, so penalty = 0
        let a = make_full_analysis("nimisana", "koira", "p");
        let score = scorer.score("koira", &a);
        assert!(
            score.abs() < f64::EPSILON,
            "Single-char structure should have zero penalty, got {}",
            score
        );
    }

    // ─────────────────────────────────────────────────────────────
    // Integration: disambiguation improves with better weights
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn disambiguation_improves_with_negation_weights() {
        // Without negation weights, "ei juokse" might not prefer NEG->VERB.
        // With negation weights, it should strongly prefer NEG->VERB.
        let model_with = BigramModel::finnish_defaults();
        let model_without = BigramModel::new(-3.0);

        let neg = make_analysis("kieltosana", "ei");
        let verb = make_analysis("teonsana", "juosta");

        let score_with = model_with.score(&neg, &verb);
        let score_without = model_without.score(&neg, &verb);

        assert!(
            score_with > score_without,
            "Finnish defaults ({}) should score NEG->VERB higher than empty model ({})",
            score_with,
            score_without
        );
    }

    #[test]
    fn disambiguation_adj_noun_vs_noun_noun() {
        // "iso koira" should prefer ADJ->NOUN over NOUN->NOUN
        let model = BigramModel::finnish_defaults();

        let adj_noun = model.get_weight("laatusana", "nimisana");
        let noun_noun = model.get_weight("nimisana", "nimisana");

        assert!(
            adj_noun > noun_noun,
            "ADJ->NOUN ({}) should be preferred over NOUN->NOUN ({})",
            adj_noun,
            noun_noun
        );
    }

    #[test]
    fn corpus_model_matches_pattern_of_defaults() {
        // Simulate a small Finnish-like corpus and verify the from_counts model
        // produces a similar pattern to finnish_defaults (NOUN->VERB > NOUN->NOUN).
        let counts = vec![
            (("nimisana", "teonsana"), 200),
            (("nimisana", "nimisana"), 50),
            (("teonsana", "nimisana"), 180),
            (("laatusana", "nimisana"), 120),
        ];
        let model = BigramModel::from_counts(&counts);

        let noun_verb = model.get_weight("nimisana", "teonsana");
        let noun_noun = model.get_weight("nimisana", "nimisana");
        assert!(
            noun_verb > noun_noun,
            "Corpus model: NOUN->VERB ({}) should be higher than NOUN->NOUN ({})",
            noun_verb,
            noun_noun
        );
    }

    // ─────────────────────────────────────────────────────────────
    // Word-level POS priors (EmissionScorer)
    // ─────────────────────────────────────────────────────────────

    fn make_scorer_with_priors() -> EmissionScorer {
        let mut scorer = EmissionScorer::new(0.0, 0.0);
        scorer.prior_weight = 2.0;
        scorer.prior_unseen_penalty = -3.0;

        let mut priors = HashMap::new();
        // "koira" is always NOUN
        let mut koira_priors = HashMap::new();
        koira_priors.insert("NOUN".to_string(), 1.0);
        priors.insert("koira".to_string(), koira_priors);

        // "kuusi" is ambiguous: 2/3 NOUN, 1/3 NUM
        let mut kuusi_priors = HashMap::new();
        kuusi_priors.insert("NOUN".to_string(), 2.0 / 3.0);
        kuusi_priors.insert("NUM".to_string(), 1.0 / 3.0);
        priors.insert("kuusi".to_string(), kuusi_priors);

        scorer.set_word_pos_priors(priors);
        scorer
    }

    #[test]
    fn emission_prior_unambiguous_word_boosts_correct_pos() {
        let scorer = make_scorer_with_priors();

        let noun = make_analysis("nimisana", "koira");
        let verb = make_analysis("teonsana", "koirata");

        let noun_score = scorer.score("koira", &noun);
        let verb_score = scorer.score("koira", &verb);

        // NOUN should get a bonus (P=1.0 -> ln(1.0)=0.0) while
        // VERB should get the unseen penalty (-3.0).
        assert!(
            noun_score > verb_score,
            "NOUN ({}) should score higher than VERB ({}) for 'koira'",
            noun_score,
            verb_score
        );
    }

    #[test]
    fn emission_prior_ambiguous_word_prefers_majority() {
        let scorer = make_scorer_with_priors();

        let noun = make_analysis("nimisana", "kuusi");
        let num = make_analysis("lukusana", "kuusi");

        let noun_score = scorer.score("kuusi", &noun);
        let num_score = scorer.score("kuusi", &num);

        // NOUN (P=2/3) should score higher than NUM (P=1/3)
        assert!(
            noun_score > num_score,
            "NOUN ({}) should score higher than NUM ({}) for 'kuusi'",
            noun_score,
            num_score
        );
    }

    #[test]
    fn emission_prior_unseen_word_no_effect() {
        let scorer = make_scorer_with_priors();

        let noun = make_analysis("nimisana", "talo");
        let verb = make_analysis("teonsana", "talo");

        let noun_score = scorer.score("talo", &noun);
        let verb_score = scorer.score("talo", &verb);

        // "talo" is not in the priors, so both should get 0.0 adjustment
        assert!(
            (noun_score - verb_score).abs() < f64::EPSILON,
            "Unseen word should not be affected by priors: NOUN={}, VERB={}",
            noun_score,
            verb_score
        );
    }

    #[test]
    fn emission_prior_case_insensitive() {
        let scorer = make_scorer_with_priors();

        // "Koira" (capitalized) should match "koira" in priors
        let noun = make_analysis("nimisana", "koira");
        let verb = make_analysis("teonsana", "koirata");

        let noun_score = scorer.score("Koira", &noun);
        let verb_score = scorer.score("Koira", &verb);

        assert!(
            noun_score > verb_score,
            "Case-insensitive lookup: NOUN ({}) should beat VERB ({}) for 'Koira'",
            noun_score,
            verb_score
        );
    }

    #[test]
    fn emission_prior_unseen_pos_penalty() {
        let scorer = make_scorer_with_priors();

        // "koira" only has NOUN in priors; ADJ should get unseen penalty
        let adj = make_analysis("laatusana", "koira");
        let score = scorer.score("koira", &adj);

        assert!(
            score < 0.0,
            "Unseen POS for a known word should receive a penalty, got {}",
            score
        );
        assert!(
            (score - (-3.0)).abs() < f64::EPSILON,
            "Expected penalty of -3.0, got {}",
            score
        );
    }

    #[test]
    fn emission_prior_perfect_prior_gives_zero_bonus() {
        let scorer = make_scorer_with_priors();

        // "koira" NOUN has P=1.0, ln(1.0) = 0.0, so bonus = 2.0 * 0.0 = 0.0
        let noun = make_analysis("nimisana", "koira");
        let score = scorer.score("koira", &noun);

        assert!(
            score.abs() < f64::EPSILON,
            "P=1.0 should give zero bonus (ln(1)=0), got {}",
            score
        );
    }

    #[test]
    fn emission_prior_has_word_pos_priors() {
        let mut scorer = EmissionScorer::new(0.0, 0.0);
        assert!(!scorer.has_word_pos_priors());
        assert_eq!(scorer.num_word_priors(), 0);

        let mut priors = HashMap::new();
        priors.insert("test".to_string(), HashMap::new());
        scorer.set_word_pos_priors(priors);
        assert!(scorer.has_word_pos_priors());
        assert_eq!(scorer.num_word_priors(), 1);
    }

    #[test]
    fn class_to_upos_category_mappings() {
        assert_eq!(class_to_upos_category("nimisana"), "NOUN");
        assert_eq!(class_to_upos_category("teonsana"), "VERB");
        assert_eq!(class_to_upos_category("laatusana"), "ADJ");
        assert_eq!(class_to_upos_category("seikkasana"), "ADV");
        assert_eq!(class_to_upos_category("asemosana"), "PRON");
        assert_eq!(class_to_upos_category("lukusana"), "NUM");
        assert_eq!(class_to_upos_category("etunimi"), "PROPN");
        assert_eq!(class_to_upos_category("sukunimi"), "PROPN");
        assert_eq!(class_to_upos_category("paikannimi"), "PROPN");
        assert_eq!(class_to_upos_category("kieltosana"), "AUX");
        assert_eq!(class_to_upos_category("suhdesana"), "ADP");
        assert_eq!(class_to_upos_category("sidesana"), "CCONJ");
        assert_eq!(class_to_upos_category("huudahdussana"), "INTJ");
        assert_eq!(class_to_upos_category("nimisana_laatusana"), "ADJ");
        assert_eq!(class_to_upos_category("lyhenne"), "NOUN");
        assert_eq!(class_to_upos_category("unknown"), "X");
    }
}
