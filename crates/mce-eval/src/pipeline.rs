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

use mce_comonad::cg::{CgRule, apply_cg_rules, finnish_disambiguation_rules};
use mce_core::analysis::{ATTR_BASEFORM, ATTR_CLASS, Analysis};
use mce_core::token::TokenType;
use mce_disambig::ViterbiDisambiguator;
use mce_disambig::bigram::EmissionScorer;
use mce_disambig::corpus::{build_model_from_conllu, extract_emission_priors};
use mce_disambig::cs::SparseDisambiguator;
use mce_disambig::suffix_tagger::SuffixTagger;
use mce_fi::morphology::{Analyzer, FinnishAnalyzer};
use mce_tokenizer::next_token;

use crate::conllu::ConlluSentence;
use crate::lemma_dict::{LemmaDict, strip_suffix};
use crate::metrics::{EvalResults, TokenResult};
use crate::pos_map::mce_to_upos;

/// Evaluation pipeline configuration.
pub struct EvalPipeline {
    analyzer: FinnishAnalyzer,
    disambiguator: ViterbiDisambiguator,
    cg_rules: Vec<Box<dyn CgRule>>,
    lemma_dict: Option<LemmaDict>,
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
            lemma_dict: None,
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
            lemma_dict: None,
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
            lemma_dict: None,
        })
    }

    /// Set the suffix tagger for emission scoring.
    ///
    /// When present, the suffix tagger's per-UPOS log-probabilities are
    /// added to each reading's emission score in the Viterbi lattice.
    pub fn set_suffix_tagger(&mut self, tagger: SuffixTagger) {
        self.disambiguator.set_suffix_tagger(tagger);
    }

    /// Set the lemma dictionary for dictionary-enhanced lemmatization.
    ///
    /// When present, the dictionary's (form, UPOS) -> lemma mapping is
    /// preferred over the FST baseform. Case normalization is also applied:
    /// non-PROPN lemmas are lowercased.
    pub fn set_lemma_dict(&mut self, dict: LemmaDict) {
        self.lemma_dict = Some(dict);
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

    /// Resolve the best lemma for a token, applying dictionary lookup and
    /// case normalization.
    ///
    /// Priority:
    /// 1. Dictionary (form, UPOS) -> lemma (if dict is loaded and has entry)
    /// 2. FST baseform (from analysis)
    /// 3. Lowercased surface form (last resort)
    ///
    /// Case normalization: if UPOS is not PROPN, lowercase the lemma.
    fn resolve_lemma(&self, form: &str, upos: &str, fst_baseform: &str) -> String {
        if let Some(ref dict) = self.lemma_dict {
            dict.resolve_lemma(form, upos, fst_baseform)
        } else {
            fst_baseform.to_string()
        }
    }

    /// Resolve lemma for an OOV word (no FST analysis found).
    ///
    /// Priority:
    /// 1. Dictionary (form, UPOS) -> lemma
    /// 2. Suffix stripping (strip Finnish inflectional suffixes)
    /// 3. Lowercased surface form (last resort)
    ///
    /// Case normalization: if UPOS is not PROPN, lowercase the lemma.
    fn resolve_lemma_oov(&self, form: &str, upos: &str, fst_baseform: &str) -> String {
        // Check dictionary first.
        if let Some(ref dict) = self.lemma_dict {
            if let Some(dict_lemma) = dict.lookup(form, upos) {
                return if upos != "PROPN" {
                    dict_lemma.to_lowercase()
                } else {
                    dict_lemma.to_string()
                };
            }
        }

        // OOV: try suffix stripping.
        let stripped = strip_suffix(&form.to_lowercase(), upos);
        if upos != "PROPN" {
            stripped
        } else {
            // For PROPN, preserve original casing pattern if possible.
            fst_baseform.to_string()
        }
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

        let mut words: Vec<String> = Vec::new();
        let mut word_analyses: Vec<Vec<Analysis>> = Vec::new();
        let mut has_analysis: Vec<bool> = Vec::new();
        let mut is_oov: Vec<bool> = Vec::new();

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
                is_oov.push(true);
            } else {
                has_analysis.push(true);
                is_oov.push(false);
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
                let prev2 = if i > 1 {
                    Some(words[i - 2].as_str())
                } else {
                    None
                };
                let next2 = if i + 2 < n {
                    Some(words[i + 2].as_str())
                } else {
                    None
                };
                let log_probs =
                    tagger.emission_scores_ext(&words[i], prev, next, prev2, next2, i, n);

                let classes = tagger.classes();
                let best_idx = log_probs
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(idx, _)| idx)
                    .unwrap_or(0);
                let pred_upos = classes[best_idx].clone();

                // Find the best FST analysis whose UPOS matches the suffix tagger
                // prediction. Use its lemma as fallback for the dictionary.
                let fst_baseform = word_analyses[i]
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
                    });

                let pred_lemma = if is_oov[i] {
                    self.resolve_lemma_oov(&token.form, &pred_upos, fst_baseform)
                } else {
                    self.resolve_lemma(&token.form, &pred_upos, fst_baseform)
                };

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

        if !self.cg_rules.is_empty() {
            word_analyses = apply_cg_rules(&word_analyses, &self.cg_rules);
        }

        let word_refs: Vec<&str> = words.iter().map(|s| s.as_str()).collect();
        let best = self
            .disambiguator
            .disambiguate_with_words(&word_refs, &word_analyses);

        for (i, token) in eval_tokens.iter().enumerate() {
            let (pred_upos, pred_lemma) = if i < best.len() && has_analysis[i] {
                let analysis = &best[i];
                let upos = mce_to_upos(analysis, &token.form);
                let fst_baseform = analysis.get(ATTR_BASEFORM).unwrap_or("");
                let lemma = if is_oov[i] {
                    self.resolve_lemma_oov(&token.form, upos, fst_baseform)
                } else {
                    self.resolve_lemma(&token.form, upos, fst_baseform)
                };
                (upos.to_string(), lemma)
            } else {
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

            if !self.cg_rules.is_empty() {
                mce_analyses = apply_cg_rules(&mce_analyses, &self.cg_rules);
            }

            let word_refs: Vec<&str> = mce_words.iter().map(|s| s.as_str()).collect();
            let best = self
                .disambiguator
                .disambiguate_with_words(&word_refs, &mce_analyses);

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

                let gold_token = gold_tokens[gold_idx];
                let forms_match = mce_word.to_lowercase() == gold_token.form.to_lowercase();

                if forms_match {
                    let (pred_upos, pred_lemma) = if mce_idx < best.len() {
                        let analysis = &best[mce_idx];
                        let upos = mce_to_upos(analysis, mce_word);
                        let fst_baseform = analysis.get(ATTR_BASEFORM).unwrap_or("");
                        let lemma = self.resolve_lemma(mce_word, upos, fst_baseform);
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

    #[test]
    fn token_result_incorrect_upos() {
        let mut r = EvalResults::new();
        r.add(&TokenResult {
            form: "iso".to_string(),
            gold_upos: "ADJ".to_string(),
            pred_upos: "NOUN".to_string(),
            gold_lemma: "iso".to_string(),
            pred_lemma: "iso".to_string(),
        });
        assert_eq!(r.upos_correct, 0);
        assert_eq!(r.total, 1);
        // Lemma still correct even though UPOS is wrong.
        assert_eq!(r.lemma_correct, 1);
    }

    #[test]
    fn token_result_unknown_pred() {
        let mut r = EvalResults::new();
        r.add(&TokenResult {
            form: "xyzzy".to_string(),
            gold_upos: "NOUN".to_string(),
            pred_upos: "X".to_string(),
            gold_lemma: "xyzzy".to_string(),
            pred_lemma: String::new(),
        });
        assert_eq!(r.upos_correct, 0);
        assert_eq!(r.unknown_count, 1);
        assert_eq!(r.total, 1);
    }

    #[test]
    fn punct_filtered_from_gold_sentence() {
        // Verify PUNCT tokens are removed when skip_punct is true.
        let s = make_gold_sentence();
        let non_punct: Vec<_> = s
            .tokens
            .iter()
            .filter(|t| t.upos != "PUNCT" && t.upos != "SYM")
            .collect();
        assert_eq!(non_punct.len(), 2);
        assert_eq!(non_punct[0].form, "Koira");
        assert_eq!(non_punct[1].form, "juoksee");
    }

    #[test]
    fn multiple_sentences_accumulate_results() {
        let mut r = EvalResults::new();
        // Sentence 1: 2 correct
        r.add(&TokenResult {
            form: "koira".to_string(),
            gold_upos: "NOUN".to_string(),
            pred_upos: "NOUN".to_string(),
            gold_lemma: "koira".to_string(),
            pred_lemma: "koira".to_string(),
        });
        r.add(&TokenResult {
            form: "juoksee".to_string(),
            gold_upos: "VERB".to_string(),
            pred_upos: "VERB".to_string(),
            gold_lemma: "juosta".to_string(),
            pred_lemma: "juosta".to_string(),
        });
        // Sentence 2: 1 correct, 1 wrong
        r.add(&TokenResult {
            form: "iso".to_string(),
            gold_upos: "ADJ".to_string(),
            pred_upos: "NOUN".to_string(),
            gold_lemma: "iso".to_string(),
            pred_lemma: "iso".to_string(),
        });
        r.add(&TokenResult {
            form: "kissa".to_string(),
            gold_upos: "NOUN".to_string(),
            pred_upos: "NOUN".to_string(),
            gold_lemma: "kissa".to_string(),
            pred_lemma: "kissa".to_string(),
        });

        assert_eq!(r.total, 4);
        assert_eq!(r.upos_correct, 3);
        assert_eq!(r.lemma_correct, 4);
        assert!((r.upos_accuracy() - 0.75).abs() < 1e-10);
        assert!((r.lemma_accuracy() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn remaining_gold_counted_as_miss() {
        // Simulate the alignment logic from evaluate_end_to_end:
        // When MCE produces fewer tokens than gold, remaining gold tokens
        // get pred_upos "X" and empty pred_lemma.
        let mut r = EvalResults::new();

        // One matched token
        r.add(&TokenResult {
            form: "koira".to_string(),
            gold_upos: "NOUN".to_string(),
            pred_upos: "NOUN".to_string(),
            gold_lemma: "koira".to_string(),
            pred_lemma: "koira".to_string(),
        });

        // Two remaining gold tokens counted as misses
        r.add(&TokenResult {
            form: "juoksee".to_string(),
            gold_upos: "VERB".to_string(),
            pred_upos: "X".to_string(),
            gold_lemma: "juosta".to_string(),
            pred_lemma: String::new(),
        });
        r.add(&TokenResult {
            form: "nopeasti".to_string(),
            gold_upos: "ADV".to_string(),
            pred_upos: "X".to_string(),
            gold_lemma: "nopeasti".to_string(),
            pred_lemma: String::new(),
        });

        assert_eq!(r.total, 3);
        assert_eq!(r.upos_correct, 1);
        assert_eq!(r.unknown_count, 2);
        assert!((r.coverage() - 1.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn empty_sentence_skipped() {
        // Verify that an all-PUNCT sentence yields no evaluation tokens.
        let s = ConlluSentence {
            sent_id: "punct.1".to_string(),
            text: "...".to_string(),
            tokens: vec![
                ConlluToken {
                    id: 1,
                    form: ".".to_string(),
                    lemma: ".".to_string(),
                    upos: "PUNCT".to_string(),
                    xpos: "Punct".to_string(),
                    feats: "_".to_string(),
                },
                ConlluToken {
                    id: 2,
                    form: ".".to_string(),
                    lemma: ".".to_string(),
                    upos: "PUNCT".to_string(),
                    xpos: "Punct".to_string(),
                    feats: "_".to_string(),
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
        };
        let non_punct: Vec<_> = s
            .tokens
            .iter()
            .filter(|t| t.upos != "PUNCT" && t.upos != "SYM")
            .collect();
        assert!(
            non_punct.is_empty(),
            "All-PUNCT sentence should have no eval tokens"
        );
    }

    #[test]
    fn sym_tokens_also_filtered() {
        // SYM tokens should also be excluded when skip_punct is true.
        let tokens = vec![
            ConlluToken {
                id: 1,
                form: "koira".to_string(),
                lemma: "koira".to_string(),
                upos: "NOUN".to_string(),
                xpos: "N".to_string(),
                feats: "_".to_string(),
            },
            ConlluToken {
                id: 2,
                form: "%".to_string(),
                lemma: "%".to_string(),
                upos: "SYM".to_string(),
                xpos: "Sym".to_string(),
                feats: "_".to_string(),
            },
        ];
        let non_punct_sym: Vec<_> = tokens
            .iter()
            .filter(|t| t.upos != "PUNCT" && t.upos != "SYM")
            .collect();
        assert_eq!(non_punct_sym.len(), 1);
        assert_eq!(non_punct_sym[0].form, "koira");
    }
}
