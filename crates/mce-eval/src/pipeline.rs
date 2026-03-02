//! Evaluation pipeline: run MCE on gold-standard sentences.
//!
//! Feeds each CoNLL-U sentence through the MCE pipeline:
//! 1. Tokenize (mce-tokenizer)
//! 2. Analyze each token (mce-fi FinnishAnalyzer)
//! 3. CG-lite pruning (mce-comonad CG rules prune unlikely readings)
//! 4. Disambiguate (mce-disambig ViterbiDisambiguator)
//! 5. Map predicted class to UPOS (pos_map)
//!
//! For evaluation, we use gold tokenization (the tokens from the CoNLL-U file)
//! rather than MCE's tokenizer. This isolates POS tagging errors from
//! tokenization errors.

use mce_comonad::cg::{apply_cg_rules, finnish_disambiguation_rules, CgRule};
use mce_core::analysis::{Analysis, ATTR_BASEFORM, ATTR_CLASS};
use mce_core::token::TokenType;
use mce_disambig::bigram::EmissionScorer;
use mce_disambig::corpus::{build_model_from_conllu, extract_emission_priors};
use mce_disambig::cs::SparseDisambiguator;
use mce_disambig::suffix_tagger::SuffixTagger;
use mce_disambig::ViterbiDisambiguator;
use mce_fi::morphology::{Analyzer, FinnishAnalyzer};
use mce_tokenizer::next_token;

use crate::conllu::ConlluSentence;
use crate::metrics::{EvalResults, TokenResult};
use crate::pos_map::mce_to_upos;

/// Evaluation pipeline configuration.
pub struct EvalPipeline {
    analyzer: FinnishAnalyzer,
    disambiguator: ViterbiDisambiguator,
    cg_rules: Vec<Box<dyn CgRule>>,
}

impl EvalPipeline {
    /// Create a pipeline from raw VFST dictionary bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, mce_fst::VfstError> {
        let analyzer = FinnishAnalyzer::from_bytes(data)?;
        let disambiguator = ViterbiDisambiguator::with_finnish_defaults_and_emission();
        let cg_rules = finnish_disambiguation_rules();
        Ok(Self {
            analyzer,
            disambiguator,
            cg_rules,
        })
    }

    /// Create a pipeline with corpus-trained bigram weights and emission priors.
    ///
    /// Uses the provided CoNLL-U training data to:
    /// 1. Build a bigram model with real corpus transition statistics.
    /// 2. Extract word-level P(UPOS|word) emission priors and configure them
    ///    in the emission scorer. These priors are the strongest disambiguation
    ///    signal for unambiguous or low-ambiguity words.
    pub fn from_bytes_with_corpus(
        data: &[u8],
        train_conllu: &str,
    ) -> Result<Self, mce_fst::VfstError> {
        let analyzer = FinnishAnalyzer::from_bytes(data)?;
        let corpus_model = build_model_from_conllu(train_conllu);

        // Extract word-level POS priors from training data.
        let emission_priors = extract_emission_priors(train_conllu);
        let prior_count = emission_priors.len();

        let mut emission_scorer = EmissionScorer::finnish_defaults();
        emission_scorer.set_word_pos_priors(emission_priors);

        eprintln!("Emission priors: {} word forms loaded.", prior_count,);

        let mut disambiguator = ViterbiDisambiguator::new(corpus_model);
        disambiguator.set_emission_scorer(emission_scorer);
        let cg_rules = finnish_disambiguation_rules();
        Ok(Self {
            analyzer,
            disambiguator,
            cg_rules,
        })
    }

    /// Create a pipeline with corpus-trained bigrams but NO emission priors.
    ///
    /// This is for the "CS-only" experiment where we want to test whether
    /// CS scoring can replace emission priors entirely.
    pub fn from_bytes_with_corpus_no_emission(
        data: &[u8],
        train_conllu: &str,
    ) -> Result<Self, mce_fst::VfstError> {
        let analyzer = FinnishAnalyzer::from_bytes(data)?;
        let corpus_model = build_model_from_conllu(train_conllu);

        let disambiguator = ViterbiDisambiguator::new(corpus_model);
        let cg_rules = finnish_disambiguation_rules();
        Ok(Self {
            analyzer,
            disambiguator,
            cg_rules,
        })
    }

    /// Set the suffix tagger for emission scoring.
    ///
    /// When present, the suffix tagger's per-UPOS log-probabilities are
    /// added to each reading's emission score in the Viterbi lattice.
    pub fn set_suffix_tagger(&mut self, tagger: SuffixTagger) {
        self.disambiguator.set_suffix_tagger(tagger);
    }

    /// Enable the Compressed Sensing (FISTA) scorer on this pipeline.
    ///
    /// Creates a `SparseDisambiguator` with the given number of measurements
    /// and L1 regularization parameter (lambda), and configures it on the
    /// internal `ViterbiDisambiguator`.
    pub fn enable_cs(&mut self, num_measurements: usize, lambda: f64) {
        let cs_scorer = SparseDisambiguator::new(num_measurements, lambda);
        self.disambiguator.set_cs_scorer(cs_scorer);
    }

    /// Evaluate a set of CoNLL-U sentences, returning aggregate results.
    ///
    /// Skips punctuation tokens (UPOS = "PUNCT") and symbol tokens ("SYM")
    /// by default, since MCE's tokenizer treats them separately and they are
    /// not part of morphological analysis.
    pub fn evaluate(&self, sentences: &[ConlluSentence]) -> EvalResults {
        self.evaluate_filtered(sentences, true)
    }

    /// Evaluate with option to include/exclude punctuation.
    pub fn evaluate_filtered(&self, sentences: &[ConlluSentence], skip_punct: bool) -> EvalResults {
        let mut results = EvalResults::new();

        for sentence in sentences {
            self.evaluate_sentence(sentence, skip_punct, &mut results);
        }

        results
    }

    /// Evaluate a single sentence using gold tokenization.
    ///
    /// Uses gold tokens from the CoNLL-U file (not MCE tokenizer) to
    /// isolate POS tagging accuracy from tokenization accuracy.
    fn evaluate_sentence(
        &self,
        sentence: &ConlluSentence,
        skip_punct: bool,
        results: &mut EvalResults,
    ) {
        // Filter tokens: skip punctuation if requested.
        let eval_tokens: Vec<_> = sentence
            .tokens
            .iter()
            .filter(|t| {
                if skip_punct {
                    t.upos != "PUNCT" && t.upos != "SYM"
                } else {
                    true
                }
            })
            .collect();

        if eval_tokens.is_empty() {
            return;
        }

        // Analyze each gold token with MCE.
        let mut words: Vec<String> = Vec::new();
        let mut word_analyses: Vec<Vec<Analysis>> = Vec::new();
        let mut has_analysis: Vec<bool> = Vec::new();

        for token in &eval_tokens {
            let chars: Vec<char> = token.form.chars().collect();
            let analyses = self.analyzer.analyze(&chars, chars.len());
            words.push(token.form.clone());
            if analyses.is_empty() {
                // Create fallback analysis so disambiguator doesn't bail out.
                // If the word starts with uppercase (and is not sentence-initial or
                // we can't tell), guess PROPN; otherwise default to NOUN.
                let mut fallback = Analysis::new();
                let first_char = token.form.chars().next().unwrap_or('a');
                if first_char.is_uppercase() {
                    fallback.set(ATTR_CLASS, "etunimi"); // → PROPN
                } else {
                    fallback.set(ATTR_CLASS, "nimisana"); // → NOUN
                }
                let lower = token.form.to_lowercase();
                fallback.set(ATTR_BASEFORM, lower);
                word_analyses.push(vec![fallback]);
                has_analysis.push(true); // fallback analysis counts as a prediction
            } else {
                has_analysis.push(true);
                word_analyses.push(analyses);
            }
        }

        // Apply CG rules to prune candidate readings before disambiguation.
        // CG rules remove unlikely readings based on local context, which
        // reduces ambiguity for the Viterbi disambiguator.
        if !self.cg_rules.is_empty() {
            word_analyses = apply_cg_rules(&word_analyses, &self.cg_rules);
        }

        // When a suffix tagger is available, use it as the primary UPOS predictor.
        // The FST analysis is used for lemma extraction, and the suffix tagger
        // provides the UPOS tag. This hybrid approach combines the suffix tagger's
        // superior UPOS accuracy with the FST's morphological analysis.
        if let Some(tagger) = self.disambiguator.suffix_tagger() {
            let n = words.len();
            for (i, token) in eval_tokens.iter().enumerate() {
                let prev = if i > 0 {
                    Some(words[i - 1].as_str())
                } else {
                    None
                };
                let next = if i + 1 < n {
                    Some(words[i + 1].as_str())
                } else {
                    None
                };
                let log_probs = tagger.emission_scores(&words[i], prev, next, i, n);

                // Find the best UPOS tag from the suffix tagger.
                let classes = tagger.classes();
                let best_idx = log_probs
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(idx, _)| idx)
                    .unwrap_or(0);
                let pred_upos = classes[best_idx].clone();

                // Find the best FST analysis whose UPOS matches the suffix tagger
                // prediction. Use its lemma.
                let pred_lemma = word_analyses[i]
                    .iter()
                    .find(|a| {
                        let mapped = mce_to_upos(a, &token.form);
                        mapped == pred_upos
                    })
                    .and_then(|a| a.get(ATTR_BASEFORM))
                    .unwrap_or_else(|| {
                        // No FST analysis matches: use the first analysis's lemma
                        // or fallback to lowercased surface.
                        word_analyses[i]
                            .first()
                            .and_then(|a| a.get(ATTR_BASEFORM))
                            .unwrap_or(&token.form)
                    })
                    .to_string();

                results.add(&TokenResult {
                    form: token.form.clone(),
                    gold_upos: token.upos.clone(),
                    pred_upos,
                    gold_lemma: token.lemma.clone(),
                    pred_lemma,
                });
            }
            return;
        }

        // Apply CG rules to prune candidate readings before disambiguation.
        // CG rules remove unlikely readings based on local context, which
        // reduces ambiguity for the Viterbi disambiguator.
        if !self.cg_rules.is_empty() {
            word_analyses = apply_cg_rules(&word_analyses, &self.cg_rules);
        }

        // Disambiguate.
        let word_refs: Vec<&str> = words.iter().map(|s| s.as_str()).collect();
        let best = self
            .disambiguator
            .disambiguate_with_words(&word_refs, &word_analyses);

        // Compare predictions to gold.
        for (i, token) in eval_tokens.iter().enumerate() {
            let (pred_upos, pred_lemma) = if i < best.len() && has_analysis[i] {
                let analysis = &best[i];
                let upos = mce_to_upos(analysis, &token.form);
                let lemma = analysis.get(ATTR_BASEFORM).unwrap_or("").to_string();
                (upos.to_string(), lemma)
            } else {
                // No analysis available.
                ("X".to_string(), String::new())
            };

            results.add(&TokenResult {
                form: token.form.clone(),
                gold_upos: token.upos.clone(),
                pred_upos,
                gold_lemma: token.lemma.clone(),
                pred_lemma,
            });
        }
    }

    /// Evaluate using MCE's own tokenizer (for measuring end-to-end accuracy).
    ///
    /// This tokenizes the raw sentence text with MCE's tokenizer, aligns
    /// the resulting tokens with gold tokens, and evaluates POS accuracy.
    /// Alignment mismatches are counted as errors.
    pub fn evaluate_end_to_end(&self, sentences: &[ConlluSentence]) -> EvalResults {
        let mut results = EvalResults::new();

        for sentence in sentences {
            if sentence.text.is_empty() {
                continue;
            }

            // Tokenize with MCE.
            let chars: Vec<char> = sentence.text.chars().collect();
            let text_len = chars.len();
            let mut pos = 0;
            let mut mce_words: Vec<String> = Vec::new();
            let mut mce_analyses: Vec<Vec<Analysis>> = Vec::new();

            while pos < text_len {
                let (token_type, token_len) = next_token(&chars, text_len, pos);
                if token_len == 0 {
                    break;
                }
                if token_type == TokenType::Word {
                    let word: String = chars[pos..pos + token_len].iter().collect();
                    let word_chars: Vec<char> = word.chars().collect();
                    let analyses = self.analyzer.analyze(&word_chars, word_chars.len());
                    mce_words.push(word);
                    mce_analyses.push(analyses);
                }
                pos += token_len;
            }

            // Apply CG rules to prune candidate readings before disambiguation.
            if !self.cg_rules.is_empty() {
                mce_analyses = apply_cg_rules(&mce_analyses, &self.cg_rules);
            }

            // Disambiguate.
            let word_refs: Vec<&str> = mce_words.iter().map(|s| s.as_str()).collect();
            let best = self
                .disambiguator
                .disambiguate_with_words(&word_refs, &mce_analyses);

            // Build gold tokens (non-punct).
            let gold_tokens: Vec<_> = sentence
                .tokens
                .iter()
                .filter(|t| t.upos != "PUNCT" && t.upos != "SYM")
                .collect();

            // Simple alignment: match by surface form position.
            let mut gold_idx = 0;
            for (mce_idx, mce_word) in mce_words.iter().enumerate() {
                if gold_idx >= gold_tokens.len() {
                    break;
                }

                // Try to align: find the gold token matching this MCE word.
                let gold_token = gold_tokens[gold_idx];
                let forms_match = mce_word.to_lowercase() == gold_token.form.to_lowercase();

                if forms_match {
                    let (pred_upos, pred_lemma) = if mce_idx < best.len() {
                        let analysis = &best[mce_idx];
                        let upos = mce_to_upos(analysis, mce_word);
                        let lemma = analysis.get(ATTR_BASEFORM).unwrap_or("").to_string();
                        (upos.to_string(), lemma)
                    } else {
                        ("X".to_string(), String::new())
                    };

                    results.add(&TokenResult {
                        form: mce_word.clone(),
                        gold_upos: gold_token.upos.clone(),
                        pred_upos,
                        gold_lemma: gold_token.lemma.clone(),
                        pred_lemma,
                    });
                    gold_idx += 1;
                }
                // If forms don't match, skip this MCE token (alignment error).
            }

            // Any remaining gold tokens are counted as misses.
            while gold_idx < gold_tokens.len() {
                let gold_token = gold_tokens[gold_idx];
                results.add(&TokenResult {
                    form: gold_token.form.clone(),
                    gold_upos: gold_token.upos.clone(),
                    pred_upos: "X".to_string(),
                    gold_lemma: gold_token.lemma.clone(),
                    pred_lemma: String::new(),
                });
                gold_idx += 1;
            }
        }

        results
    }
}

/// Analyze a single word and return the raw UPOS prediction (for debugging).
///
/// This is a convenience function for interactive testing.
pub fn analyze_word_upos(analyzer: &FinnishAnalyzer, word: &str) -> (String, Vec<Analysis>) {
    let chars: Vec<char> = word.chars().collect();
    let analyses = analyzer.analyze(&chars, chars.len());

    let upos = if let Some(first) = analyses.first() {
        let class = first.get(ATTR_CLASS).unwrap_or("X");
        class.to_string()
    } else {
        "X".to_string()
    };

    (upos, analyses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conllu::{ConlluSentence, ConlluToken};

    fn make_gold_sentence() -> ConlluSentence {
        ConlluSentence {
            sent_id: "test.1".to_string(),
            text: "Koira juoksee.".to_string(),
            tokens: vec![
                ConlluToken {
                    id: 1,
                    form: "Koira".to_string(),
                    lemma: "koira".to_string(),
                    upos: "NOUN".to_string(),
                    xpos: "N".to_string(),
                    feats: "Case=Nom|Number=Sing".to_string(),
                },
                ConlluToken {
                    id: 2,
                    form: "juoksee".to_string(),
                    lemma: "juosta".to_string(),
                    upos: "VERB".to_string(),
                    xpos: "V".to_string(),
                    feats: "Mood=Ind|Number=Sing|Person=3".to_string(),
                },
                ConlluToken {
                    id: 3,
                    form: ".".to_string(),
                    lemma: ".".to_string(),
                    upos: "PUNCT".to_string(),
                    xpos: "Punct".to_string(),
                    feats: "_".to_string(),
                },
            ],
        }
    }

    #[test]
    fn token_result_correct() {
        let mut r = EvalResults::new();
        r.add(&TokenResult {
            form: "koira".to_string(),
            gold_upos: "NOUN".to_string(),
            pred_upos: "NOUN".to_string(),
            gold_lemma: "koira".to_string(),
            pred_lemma: "koira".to_string(),
        });
        assert_eq!(r.upos_correct, 1);
        assert_eq!(r.total, 1);
    }

    #[test]
    fn gold_sentence_structure() {
        let s = make_gold_sentence();
        assert_eq!(s.tokens.len(), 3);
        assert_eq!(s.tokens[0].upos, "NOUN");
        assert_eq!(s.tokens[2].upos, "PUNCT");
    }
}
