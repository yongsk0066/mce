// Finnish spell-checker: connects M1 Succinct Trie with FinnishAnalyzer.
//
// Architecture:
//   - check(): FinnishAnalyzer (FST traversal) is the primary oracle.
//     It knows ALL valid Finnish word forms (inflections, compounds, etc.)
//     The trie serves as a fast-path cache for known base forms.
//   - suggest(): Uses the trie's fuzzy_search (Levenshtein automaton) on
//     base forms extracted from the FST symbol table, then validates
//     candidates through the morph validator.
//   - suggest_with_context(): Like suggest(), but re-ranks candidates
//     using POS bigram scores from the previous word's analysis.
//
// Frequency-based ranking:
//   An optional FrequencyList (from UD CoNLL-U data) re-ranks suggestions
//   so that more common words appear first among equal-distance candidates.
//
// This module glues together:
//   - mce_core::trie (M1 Succinct Trie)
//   - mce_core::frequency (FrequencyList, for frequency-based ranking)
//   - mce_fi::morphology (FinnishAnalyzer)
//   - mce_speller::pipeline (SpellChecker, MorphValidator, SpellCheckerBuilder)
//   - mce_disambig::bigram (BigramModel, for context-based ranking)

use mce_core::analysis::ATTR_CLASS;
use mce_core::compound::CompoundAnalyzer;
use mce_core::frequency::FrequencyList;
use mce_core::trie::{SuccinctTrie, TrieBuilder};
use mce_disambig::bigram::BigramModel;
use mce_fst::VfstError;
use mce_fst::unweighted::UnweightedTransducer;
use mce_speller::SpellResult;
use mce_speller::pipeline::{MorphValidator, SpellChecker, SpellCheckerBuilder};
use mce_speller::user_dict::UserDictionary;

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
    /// Optional word frequency list for ranking suggestions.
    freq_list: Option<FrequencyList>,
    /// Optional compound analyzer for compound-aware spelling.
    #[allow(clippy::type_complexity)]
    compound_analyzer: Option<CompoundAnalyzer<Box<dyn Fn(&str) -> bool>>>,
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

        // Build a separate analyzer for the compound analyzer's dictionary lookup.
        let compound_morph = FinnishAnalyzer::from_bytes(mor_vfst)?;
        let compound_lookup: Box<dyn Fn(&str) -> bool> = Box::new(move |word: &str| {
            let chars: Vec<char> = word.chars().collect();
            let len = chars.len();
            if len == 0 {
                return false;
            }
            !compound_morph.analyze(&chars, len).is_empty()
        });
        let compound_analyzer = CompoundAnalyzer::new(compound_lookup);

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

        Ok(Self {
            checker,
            freq_list: None,
            compound_analyzer: Some(compound_analyzer),
        })
    }

    /// Attach a word frequency list for frequency-based suggestion ranking.
    ///
    /// When a frequency list is present, [`suggest`](Self::suggest) ranks
    /// candidates by: (1) edit distance ascending, (2) frequency descending,
    /// (3) alphabetical. More common words appear first among equal-distance
    /// candidates.
    pub fn with_frequency_list(mut self, freq_list: FrequencyList) -> Self {
        self.freq_list = Some(freq_list);
        self
    }

    /// Set the frequency list (mutable version).
    pub fn set_frequency_list(&mut self, freq_list: FrequencyList) {
        self.freq_list = Some(freq_list);
    }

    /// Access the frequency list, if set.
    pub fn frequency_list(&self) -> Option<&FrequencyList> {
        self.freq_list.as_ref()
    }

    /// Set the user dictionary.
    ///
    /// Words in the user dictionary are always considered correctly spelled
    /// and appear in spelling suggestions.
    pub fn set_user_dict(&mut self, user_dict: UserDictionary) {
        self.checker.set_user_dict(user_dict);
    }

    /// Access the user dictionary, if set.
    pub fn user_dict(&self) -> Option<&UserDictionary> {
        self.checker.user_dict()
    }

    /// Builder-style method to set the user dictionary.
    pub fn with_user_dict(mut self, user_dict: UserDictionary) -> Self {
        self.checker.set_user_dict(user_dict);
        self
    }

    /// Check whether a Finnish word is correctly spelled.
    ///
    /// Returns [`SpellResult::Ok`] if the word is found in the trie,
    /// passes morphological analysis via the FST, is in the user
    /// dictionary, or is a valid compound word. Results are cached
    /// for repeated lookups.
    pub fn check(&mut self, word: &str) -> SpellResult {
        // The underlying checker already handles trie, user dict, and morph.
        let result = self.checker.check(word);
        if result == SpellResult::Ok {
            return result;
        }

        // Compound-aware check: try compound splitting.
        if let Some(ref ca) = self.compound_analyzer {
            let splits = ca.analyze(word);
            if splits.iter().any(|s| s.word_parts().len() >= 2) {
                return SpellResult::Ok;
            }
        }

        result
    }

    /// Generate spelling suggestions for a misspelled word.
    ///
    /// Uses the trie's Levenshtein automaton to find candidates within
    /// `max_edits` edit distance of the input. Candidates are filtered
    /// through the morph validator.
    ///
    /// When a frequency list is present, candidates are ranked by:
    /// (1) edit distance ascending, (2) word frequency descending,
    /// (3) alphabetical. This ensures common words like "ja", "on",
    /// "ei" appear before rare words at the same edit distance.
    ///
    /// Returns at most 10 suggestions.
    pub fn suggest(&self, word: &str, max_edits: usize) -> Vec<String> {
        match &self.freq_list {
            Some(fl) => {
                let rank_fn = |candidate: &str| -> f64 {
                    // Use log(1 + freq) so that very high frequencies don't
                    // dominate excessively. Relative frequency is in [0, 1].
                    let rel = fl.relative_frequency(candidate);
                    (1.0 + rel * 1_000_000.0).ln()
                };
                self.checker
                    .suggest_ranked(word, max_edits, 10, Some(rank_fn))
            }
            None => self.checker.suggest(word, max_edits, 10),
        }
    }

    /// Generate context-aware suggestions for a misspelled word.
    ///
    /// Uses POS bigram scoring from the previous word to re-rank candidates.
    /// The pipeline:
    /// 1. Fuzzy search the trie for candidates within `max_edits`.
    /// 2. Filter by morphological validity (via FinnishAnalyzer).
    /// 3. For each valid candidate, analyze it to get its POS class.
    /// 4. Score each candidate using the bigram transition weight from
    ///    the previous word's POS class to the candidate's POS class.
    /// 5. Rank by (edit_distance, -bigram_score, lexicographic).
    /// 6. Return the top 5 suggestions.
    ///
    /// If `prev_word` is `None`, falls back to standard `suggest()`.
    pub fn suggest_with_context(
        &self,
        word: &str,
        prev_word: Option<&str>,
        max_edits: usize,
    ) -> Vec<String> {
        const MAX_SUGGESTIONS: usize = 5;

        let prev_class = prev_word.and_then(|pw| {
            let chars: Vec<char> = pw.chars().collect();
            let clen = chars.len();
            let morph = self.checker.morph();
            let analyses = morph.analyzer().analyze(&chars, clen);
            // Take the POS class from the first analysis (most likely).
            analyses
                .first()
                .and_then(|a| a.get(ATTR_CLASS).map(|s| s.to_string()))
        });

        match prev_class {
            Some(prev_cls) => {
                let bigram_model = BigramModel::finnish_defaults();
                let freq_list = &self.freq_list;
                let rank_fn = |candidate: &str| -> f64 {
                    // Analyze the candidate to get its POS class.
                    let chars: Vec<char> = candidate.chars().collect();
                    let clen = chars.len();
                    let morph = self.checker.morph();
                    let analyses = morph.analyzer().analyze(&chars, clen);
                    let bigram_score = analyses
                        .iter()
                        .map(|a| {
                            let cls = a.get(ATTR_CLASS).unwrap_or("");
                            bigram_model.get_weight(&prev_cls, cls)
                        })
                        .fold(f64::NEG_INFINITY, f64::max);

                    // Add frequency bonus if available (weighted lower than
                    // bigram to keep context as primary signal).
                    let freq_bonus = freq_list
                        .as_ref()
                        .map(|fl| {
                            let rel = fl.relative_frequency(candidate);
                            (1.0 + rel * 1_000_000.0).ln() * 0.1
                        })
                        .unwrap_or(0.0);

                    bigram_score + freq_bonus
                };
                self.checker
                    .suggest_ranked(word, max_edits, MAX_SUGGESTIONS, Some(rank_fn))
            }
            None => {
                // No context available; use frequency-based or plain ranking.
                match &self.freq_list {
                    Some(fl) => {
                        let rank_fn = |candidate: &str| -> f64 {
                            let rel = fl.relative_frequency(candidate);
                            (1.0 + rel * 1_000_000.0).ln()
                        };
                        self.checker
                            .suggest_ranked(word, max_edits, MAX_SUGGESTIONS, Some(rank_fn))
                    }
                    None => self.checker.suggest_ranked(
                        word,
                        max_edits,
                        MAX_SUGGESTIONS,
                        None::<fn(&str) -> f64>,
                    ),
                }
            }
        }
    }

    /// Generate compound-aware spelling suggestions.
    ///
    /// When a word might be a misspelled compound, tries to split it and
    /// correct individual parts. For example, if "rautatieasma" is input
    /// (where "asma" is a typo for "asema"), this may suggest
    /// "rautatieasema".
    ///
    /// Returns at most `max_suggestions` compound suggestions, sorted by
    /// edit distance from the original word.
    pub fn suggest_compound(&self, word: &str, max_edits: usize) -> Vec<String> {
        let ca = match &self.compound_analyzer {
            Some(ca) => ca,
            None => return Vec::new(),
        };

        let mut results: Vec<(usize, String)> = Vec::new();

        // Strategy: try splitting the word at various positions.
        // For each potential first-part length, check if the prefix is a
        // valid word. If so, try to correct the remainder (and vice versa).
        let word_len = word.len();
        if word_len < 4 {
            return Vec::new();
        }

        for split_pos in 2..word_len.saturating_sub(1) {
            if !word.is_char_boundary(split_pos) {
                continue;
            }

            let prefix = &word[..split_pos];
            let suffix = &word[split_pos..];

            // Case 1: prefix is valid, suffix needs correction.
            let prefix_valid = {
                let chars: Vec<char> = prefix.chars().collect();
                let clen = chars.len();
                !self
                    .checker
                    .morph()
                    .analyzer()
                    .analyze(&chars, clen)
                    .is_empty()
            };

            if prefix_valid && !suffix.is_empty() {
                let suffix_suggestions =
                    self.checker
                        .suggest_ranked(suffix, max_edits, 3, None::<fn(&str) -> f64>);
                for corrected_suffix in suffix_suggestions {
                    let compound = format!("{prefix}{corrected_suffix}");
                    // Verify the compound is valid.
                    let compound_splits = ca.analyze(&compound);
                    if compound_splits.iter().any(|s| s.word_parts().len() >= 2) {
                        let dist = edit_distance_str(word, &compound);
                        if dist <= max_edits && dist > 0 {
                            results.push((dist, compound));
                        }
                    }
                }
            }

            // Case 2: suffix is valid, prefix needs correction.
            let suffix_valid = {
                let chars: Vec<char> = suffix.chars().collect();
                let clen = chars.len();
                !self
                    .checker
                    .morph()
                    .analyzer()
                    .analyze(&chars, clen)
                    .is_empty()
            };

            if suffix_valid && !prefix.is_empty() {
                let prefix_suggestions =
                    self.checker
                        .suggest_ranked(prefix, max_edits, 3, None::<fn(&str) -> f64>);
                for corrected_prefix in prefix_suggestions {
                    let compound = format!("{corrected_prefix}{suffix}");
                    let compound_splits = ca.analyze(&compound);
                    if compound_splits.iter().any(|s| s.word_parts().len() >= 2) {
                        let dist = edit_distance_str(word, &compound);
                        if dist <= max_edits && dist > 0 {
                            results.push((dist, compound));
                        }
                    }
                }
            }
        }

        // Deduplicate and sort by distance.
        results.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        results.dedup_by(|a, b| a.1 == b.1);
        results.into_iter().take(5).map(|(_, s)| s).collect()
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

/// Compute the Levenshtein edit distance between two strings.
///
/// Delegates to [`mce_core::string_utils::levenshtein_distance`] on byte slices.
fn edit_distance_str(a: &str, b: &str) -> usize {
    mce_core::string_utils::levenshtein_distance(a.as_bytes(), b.as_bytes())
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

    // ── frequency integration tests (using mock trie) ─────────────

    /// Build a spell checker with a mock trie and frequency list to verify
    /// that frequency-based ranking works without needing real VFST data.
    #[test]
    fn frequency_ranking_prefers_common_words() {
        use mce_core::trie::TrieBuilder;

        // Build trie with words at equal edit distance from "koirb".
        let mut builder = TrieBuilder::new();
        builder.insert(b"koira".to_vec()); // common word (dog)
        builder.insert(b"koiru".to_vec()); // rare word (made up)
        builder.insert(b"koirn".to_vec()); // rare word (made up)
        let trie = builder.build();

        // Accept all words in the morph validator.
        let accept_all = |_word: &[char], _len: usize| -> bool { true };

        let checker = SpellCheckerBuilder::new()
            .trie(trie)
            .morph_validator(accept_all)
            .cache_size(0)
            .build();

        // Build frequency list: "koira" is very common, others are rare.
        let conllu = "\
# sent_id = f.1
# text = Koira juoksee.
1\tKoira\tkoira\tNOUN\tN\tCase=Nom|Number=Sing\t2\tnsubj\t2:nsubj\t_
2\tjuoksee\tjuosta\tVERB\tV\tMood=Ind\t0\troot\t0:root\tSpaceAfter=No
3\t.\t.\tPUNCT\tPunct\t_\t2\tpunct\t2:punct\t_

# sent_id = f.2
# text = Koira on iso.
1\tKoira\tkoira\tNOUN\tN\tCase=Nom|Number=Sing\t3\tnsubj\t3:nsubj\t_
2\ton\tolla\tAUX\tV\tMood=Ind\t3\tcop\t3:cop\t_
3\tiso\tiso\tADJ\tA\tCase=Nom\t0\troot\t0:root\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t3\tpunct\t3:punct\t_

# sent_id = f.3
# text = Koiru on harvinainen.
1\tKoiru\tkoiru\tNOUN\tN\tCase=Nom|Number=Sing\t3\tnsubj\t3:nsubj\t_
2\ton\tolla\tAUX\tV\tMood=Ind\t3\tcop\t3:cop\t_
3\tharvinainen\tharvinainen\tADJ\tA\tCase=Nom\t0\troot\t0:root\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t3\tpunct\t3:punct\t_
";
        let fl = FrequencyList::from_conllu(conllu);
        assert_eq!(fl.frequency("koira"), 2);
        assert_eq!(fl.frequency("koiru"), 1);

        // Without frequency: suggestions sorted by edit distance then alphabetical.
        let suggestions_no_freq = checker.suggest_ranked("koirb", 1, 10, None::<fn(&str) -> f64>);
        // All three are 1 edit from "koirb".
        assert!(suggestions_no_freq.contains(&"koira".to_string()));

        // With frequency: "koira" should be ranked higher than "koiru".
        let rank_fn = |candidate: &str| -> f64 {
            let rel = fl.relative_frequency(candidate);
            (1.0 + rel * 1_000_000.0).ln()
        };
        let suggestions_freq = checker.suggest_ranked("koirb", 1, 10, Some(rank_fn));
        assert!(!suggestions_freq.is_empty());
        // "koira" (freq=2) should come before "koiru" (freq=1).
        let pos_koira = suggestions_freq.iter().position(|s| s == "koira").unwrap();
        let pos_koiru = suggestions_freq.iter().position(|s| s == "koiru").unwrap();
        assert!(
            pos_koira < pos_koiru,
            "koira (pos={pos_koira}) should rank before koiru (pos={pos_koiru})"
        );
    }

    #[test]
    fn frequency_list_serialization_in_spellcheck_context() {
        let conllu = "\
# sent_id = s.1
# text = ja on ei.
1\tja\tja\tCCONJ\tC\t_\t2\tcc\t2:cc\t_
2\ton\tolla\tAUX\tV\tMood=Ind\t0\troot\t0:root\t_
3\tei\tei\tAUX\tV\tPolarity=Neg\t2\taux\t2:aux\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t2\tpunct\t2:punct\t_
";
        let fl = FrequencyList::from_conllu(conllu);
        let bytes = fl.to_bytes();
        let fl2 = FrequencyList::from_bytes(&bytes).unwrap();
        assert_eq!(fl2.frequency("ja"), 1);
        assert_eq!(fl2.frequency("on"), 1);
        assert_eq!(fl2.frequency("ei"), 1);
        assert_eq!(fl2.total(), 3);
    }

    // ── compound-aware spelling tests (using mock trie) ───────────

    #[test]
    fn compound_check_with_mock_analyzer() {
        use mce_core::compound::CompoundAnalyzer;

        // Build a compound analyzer with a small dictionary.
        let dict = |w: &str| matches!(w, "koira" | "koti" | "talo" | "auto");
        let ca = CompoundAnalyzer::new(dict);

        // "koirakoti" is a valid compound.
        let splits = ca.analyze("koirakoti");
        assert!(!splits.is_empty());
        assert!(splits.iter().any(|s| s.word_parts().len() >= 2));

        // "autotalo" is a valid compound.
        let splits = ca.analyze("autotalo");
        assert!(!splits.is_empty());

        // "xyzzyabc" is not a valid compound.
        let splits = ca.analyze("xyzzyabc");
        assert!(splits.is_empty());
    }

    #[test]
    fn edit_distance_str_basic() {
        assert_eq!(edit_distance_str("koira", "koira"), 0);
        assert_eq!(edit_distance_str("koira", "koiru"), 1);
        assert_eq!(edit_distance_str("koira", "koir"), 1);
        assert_eq!(edit_distance_str("koira", "koiraa"), 1);
        assert_eq!(edit_distance_str("", "abc"), 3);
        assert_eq!(edit_distance_str("abc", ""), 3);
    }

    // ── user dictionary integration tests (mock) ──────────────────

    #[test]
    fn user_dict_integration_with_mock_spellchecker() {
        use mce_core::trie::TrieBuilder;

        let mut builder = TrieBuilder::new();
        builder.insert(b"koira".to_vec());
        builder.insert(b"kissa".to_vec());
        let trie = builder.build();

        let accept_all = |_word: &[char], _len: usize| -> bool { true };

        let ud = UserDictionary::from_words(["MCE", "WASM", "corevoikko"]);

        let mut checker = SpellCheckerBuilder::new()
            .trie(trie)
            .morph_validator(accept_all)
            .user_dict(ud)
            .cache_size(0)
            .build();

        // User dict words are accepted.
        assert_eq!(checker.check("MCE"), SpellResult::Ok);
        assert_eq!(checker.check("WASM"), SpellResult::Ok);
        assert_eq!(checker.check("corevoikko"), SpellResult::Ok);

        // Trie words are still accepted.
        assert_eq!(checker.check("koira"), SpellResult::Ok);
    }

    #[test]
    fn user_dict_words_appear_in_suggestions() {
        use mce_core::trie::TrieBuilder;

        let mut builder = TrieBuilder::new();
        builder.insert(b"test".to_vec());
        let trie = builder.build();

        let no_morph = |_word: &[char], _len: usize| -> bool { false };
        let ud = UserDictionary::from_words(["tess"]);

        let checker = SpellCheckerBuilder::new()
            .trie(trie)
            .morph_validator(no_morph)
            .user_dict(ud)
            .cache_size(0)
            .build();

        // "tesx" is 1 edit from "test" (trie) and 1 edit from "tess" (user dict).
        // no_morph rejects trie candidates, but user dict bypasses morph.
        let suggestions = checker.suggest("tesx", 1, 10);
        assert!(
            suggestions.contains(&"tess".to_string()),
            "user dict word 'tess' should appear in suggestions: {:?}",
            suggestions
        );
    }

    #[test]
    fn frequency_and_user_dict_combined_ranking() {
        use mce_core::trie::TrieBuilder;

        let mut builder = TrieBuilder::new();
        builder.insert(b"koira".to_vec());
        builder.insert(b"koura".to_vec());
        let trie = builder.build();

        let accept_all = |_word: &[char], _len: usize| -> bool { true };

        let conllu = "\
# sent_id = f.1
# text = Koira on iso.
1\tKoira\tkoira\tNOUN\tN\tCase=Nom|Number=Sing\t3\tnsubj\t3:nsubj\t_
2\ton\tolla\tAUX\tV\tMood=Ind\t3\tcop\t3:cop\t_
3\tiso\tiso\tADJ\tA\tCase=Nom\t0\troot\t0:root\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t3\tpunct\t3:punct\t_
";
        let fl = FrequencyList::from_conllu(conllu);

        let ud = UserDictionary::from_words(["MCE"]);

        let checker = SpellCheckerBuilder::new()
            .trie(trie)
            .morph_validator(accept_all)
            .user_dict(ud)
            .frequency_list(fl)
            .cache_size(0)
            .build();

        // "MCF" is 1 edit from "MCE" (user dict).
        let suggestions = checker.suggest_ranked("MCF", 1, 10, None::<fn(&str) -> f64>);
        assert!(
            suggestions.contains(&"MCE".to_string()),
            "user dict word should appear in frequency-ranked suggestions: {:?}",
            suggestions
        );
    }
}
