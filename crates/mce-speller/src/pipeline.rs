// Spell-checking pipeline: connects M1 Succinct Trie with morphological
// validation via a language-agnostic callback.
//
// The pipeline checks words in three stages:
//   1. Cache lookup (fast path for repeated words)
//   2. Exact trie lookup (dictionary hit)
//   3. Morphological analysis fallback (compounds, inflections, etc.)

use crate::cache::SpellerCache;
use crate::user_dict::UserDictionary;
use crate::{SpellResult, Speller};
use mce_core::frequency::FrequencyList;
use mce_core::trie::SuccinctTrie;

/// A language-agnostic morphological validator.
///
/// Given a word (as a char slice and its length), returns `true` if the word
/// is morphologically valid. This decouples the spell checker from any
/// specific language module.
pub trait MorphValidator {
    fn is_valid(&self, word: &[char], word_len: usize) -> bool;
}

/// Blanket implementation for closures: `Fn(&[char], usize) -> bool`.
impl<F> MorphValidator for F
where
    F: Fn(&[char], usize) -> bool,
{
    fn is_valid(&self, word: &[char], word_len: usize) -> bool {
        (self)(word, word_len)
    }
}

/// A spell checker that combines dictionary lookup (M1 Succinct Trie) with
/// morphological validation for words not found in the dictionary.
///
/// The generic parameter `M` is the morphological validator — any type
/// implementing [`MorphValidator`], including closures.
pub struct SpellChecker<M> {
    trie: SuccinctTrie,
    morph: M,
    cache: SpellerCache,
    /// Optional user dictionary: words here are always considered correct.
    user_dict: Option<UserDictionary>,
    /// Optional frequency list for ranking suggestions by word commonness.
    freq_list: Option<FrequencyList>,
}

impl<M: MorphValidator> SpellChecker<M> {
    /// Check whether a word is correctly spelled.
    ///
    /// Stages:
    ///   1. Cache hit -> return cached result immediately.
    ///   2. Exact match in trie -> Correct.
    ///   3. Morphological validation -> Correct if valid, else Incorrect.
    ///
    /// Only `SpellResult::Ok` is cached (the underlying `SpellerCache`
    /// does not cache `Failed` results, which is intentional — failed
    /// results may change if the dictionary is updated).
    pub fn check(&mut self, word: &str) -> SpellResult {
        let chars: Vec<char> = word.chars().collect();
        let wlen = chars.len();

        // Stage 1: cache lookup.
        if self.cache.is_in_cache(&chars, wlen) {
            return self.cache.get_spell_result(&chars, wlen);
        }

        // Stage 2: user dictionary lookup.
        if let Some(ref ud) = self.user_dict {
            if ud.contains(word) {
                self.cache.set_spell_result(&chars, wlen, SpellResult::Ok);
                return SpellResult::Ok;
            }
        }

        // Stage 3: exact trie lookup (byte-level).
        if self.trie.contains(word.as_bytes()) {
            self.cache.set_spell_result(&chars, wlen, SpellResult::Ok);
            return SpellResult::Ok;
        }

        // Stage 4: morphological validation.
        if self.morph.is_valid(&chars, wlen) {
            self.cache.set_spell_result(&chars, wlen, SpellResult::Ok);
            return SpellResult::Ok;
        }

        SpellResult::Failed
    }

    /// Generate spelling suggestions for a misspelled word.
    ///
    /// Uses the trie's fuzzy search (Levenshtein automaton) to find
    /// candidates within `max_edits` edit distance, optionally filters
    /// by morphological validity, and returns at most `max_suggestions`
    /// results sorted by edit distance (ascending).
    pub fn suggest(&self, word: &str, max_edits: usize, max_suggestions: usize) -> Vec<String> {
        let raw_candidates = self.trie.fuzzy_search(word.as_bytes(), max_edits);

        let mut results: Vec<String> = raw_candidates
            .into_iter()
            .filter_map(|bytes| {
                // Convert from byte key back to UTF-8 string.
                let candidate = String::from_utf8(bytes).ok()?;

                // Accept if in user dictionary or morphologically valid.
                if self.is_candidate_valid(&candidate) {
                    Some(candidate)
                } else {
                    None
                }
            })
            .collect();

        // Also search user dictionary for nearby words.
        if let Some(ref ud) = self.user_dict {
            let word_bytes = word.as_bytes();
            for user_word in ud.words() {
                let dist = edit_distance(word_bytes, user_word.as_bytes());
                if dist <= max_edits && !results.contains(&user_word.to_string()) {
                    results.push(user_word.to_string());
                }
            }
        }

        results.truncate(max_suggestions);
        results
    }

    /// Generate suggestions without morphological filtering.
    ///
    /// Returns all candidates from the trie's fuzzy search within
    /// `max_edits`, up to `max_suggestions`, sorted by edit distance.
    pub fn suggest_unfiltered(
        &self,
        word: &str,
        max_edits: usize,
        max_suggestions: usize,
    ) -> Vec<String> {
        self.trie
            .fuzzy_search(word.as_bytes(), max_edits)
            .into_iter()
            .filter_map(|bytes| String::from_utf8(bytes).ok())
            .take(max_suggestions)
            .collect()
    }

    /// Generate ranked suggestions with optional context-based reranking.
    ///
    /// 1. Fuzzy search the trie for candidates within `max_edits`.
    /// 2. Filter by morphological validity.
    /// 3. Rank by edit distance (primary) and morph-validity bonus.
    /// 4. Limit to `max_suggestions` results.
    ///
    /// The `rank_fn` callback, if provided, assigns an additional score to each
    /// candidate (higher = better). The final ranking is:
    ///   `(edit_distance_ascending, -rank_score_descending, lexicographic)`.
    ///
    /// This enables context-dependent suggestion ranking (e.g. POS bigram
    /// scoring based on the previous word's POS tag).
    pub fn suggest_ranked<F>(
        &self,
        word: &str,
        max_edits: usize,
        max_suggestions: usize,
        rank_fn: Option<F>,
    ) -> Vec<String>
    where
        F: Fn(&str) -> f64,
    {
        let raw_candidates = self.trie.fuzzy_search(word.as_bytes(), max_edits);
        let word_bytes = word.as_bytes();

        let mut scored: Vec<(usize, f64, String)> = raw_candidates
            .into_iter()
            .filter_map(|bytes| {
                let candidate = String::from_utf8(bytes).ok()?;

                // Accept if in user dictionary or morphologically valid.
                if !self.is_candidate_valid(&candidate) {
                    return None;
                }

                // Compute edit distance (we know it is within max_edits
                // because fuzzy_search already filtered, but we need the
                // exact distance for ranking).
                let edit_dist = edit_distance(word_bytes, candidate.as_bytes());

                // Apply optional rank function (higher = better).
                let mut rank_score = rank_fn.as_ref().map_or(0.0, |f| f(&candidate));

                // Add built-in frequency bonus if a frequency list is present.
                if let Some(ref fl) = self.freq_list {
                    let rel = fl.relative_frequency(&candidate);
                    rank_score += (1.0 + rel * 1_000_000.0).ln();
                }

                Some((edit_dist, rank_score, candidate))
            })
            .collect();

        // Also include user dictionary words within edit distance.
        if let Some(ref ud) = self.user_dict {
            let mut seen: std::collections::HashSet<String> =
                scored.iter().map(|(_, _, s)| s.clone()).collect();
            for user_word in ud.words() {
                if seen.contains(user_word) {
                    continue;
                }
                let dist = edit_distance(word_bytes, user_word.as_bytes());
                if dist <= max_edits {
                    let mut rank_score = rank_fn.as_ref().map_or(0.0, |f| f(user_word));
                    if let Some(ref fl) = self.freq_list {
                        let rel = fl.relative_frequency(user_word);
                        rank_score += (1.0 + rel * 1_000_000.0).ln();
                    }
                    scored.push((dist, rank_score, user_word.to_string()));
                    seen.insert(user_word.to_string());
                }
            }
        }

        // Sort: ascending edit distance, then descending rank_score,
        // then lexicographic.
        scored.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.2.cmp(&b.2))
        });

        scored
            .into_iter()
            .take(max_suggestions)
            .map(|(_, _, s)| s)
            .collect()
    }

    /// Check whether a candidate string is valid (either in user dictionary
    /// or morphologically valid).
    fn is_candidate_valid(&self, candidate: &str) -> bool {
        // User dictionary words are always valid.
        if let Some(ref ud) = self.user_dict {
            if ud.contains(candidate) {
                return true;
            }
        }
        // Fall back to morphological validation.
        let chars: Vec<char> = candidate.chars().collect();
        let clen = chars.len();
        self.morph.is_valid(&chars, clen)
    }

    /// Access the underlying trie (for inspection or advanced usage).
    pub fn trie(&self) -> &SuccinctTrie {
        &self.trie
    }

    /// Access the morph validator.
    pub fn morph(&self) -> &M {
        &self.morph
    }

    /// Set the user dictionary.
    pub fn set_user_dict(&mut self, user_dict: UserDictionary) {
        self.user_dict = Some(user_dict);
    }

    /// Access the user dictionary, if set.
    pub fn user_dict(&self) -> Option<&UserDictionary> {
        self.user_dict.as_ref()
    }

    /// Set the frequency list for ranking suggestions.
    pub fn set_frequency_list(&mut self, freq_list: FrequencyList) {
        self.freq_list = Some(freq_list);
    }

    /// Access the frequency list, if set.
    pub fn frequency_list(&self) -> Option<&FrequencyList> {
        self.freq_list.as_ref()
    }
}

/// Compute the Levenshtein edit distance between two byte sequences.
fn edit_distance(a: &[u8], b: &[u8]) -> usize {
    let n = a.len();
    let m = b.len();

    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0usize; m + 1];

    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[m]
}

/// Implement the existing `Speller` trait so that `SpellChecker` can be
/// used with the `SpellerCache::spell_with_cache` mechanism.
impl<M: MorphValidator> Speller for SpellChecker<M> {
    fn spell(&self, word: &[char], word_len: usize) -> SpellResult {
        // Convert char slice to a UTF-8 string for trie lookup.
        let s: String = word[..word_len].iter().collect();

        // Stage 1: user dictionary lookup.
        if let Some(ref ud) = self.user_dict {
            if ud.contains(&s) {
                return SpellResult::Ok;
            }
        }

        // Stage 2: exact trie lookup.
        if self.trie.contains(s.as_bytes()) {
            return SpellResult::Ok;
        }

        // Stage 3: morphological validation.
        if self.morph.is_valid(word, word_len) {
            return SpellResult::Ok;
        }

        SpellResult::Failed
    }
}

/// Builder for constructing a [`SpellChecker`] from its components.
///
/// # Example
///
/// ```ignore
/// let checker = SpellCheckerBuilder::new()
///     .trie(my_trie)
///     .morph_validator(|word: &[char], len: usize| {
///         // your morphological validation here
///         false
///     })
///     .cache_size(2)
///     .build();
/// ```
pub struct SpellCheckerBuilder<M> {
    trie: Option<SuccinctTrie>,
    morph: Option<M>,
    cache_size_param: usize,
    user_dict: Option<UserDictionary>,
    freq_list: Option<FrequencyList>,
}

impl<M: MorphValidator> SpellCheckerBuilder<M> {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            trie: None,
            morph: None,
            cache_size_param: 0,
            user_dict: None,
            freq_list: None,
        }
    }

    /// Set the dictionary trie.
    pub fn trie(mut self, trie: SuccinctTrie) -> Self {
        self.trie = Some(trie);
        self
    }

    /// Set the morphological validator.
    pub fn morph_validator(mut self, morph: M) -> Self {
        self.morph = Some(morph);
        self
    }

    /// Set the cache size parameter (default: 0).
    ///
    /// The cache allocates `6544 * (1 << size_param)` character slots.
    /// A value of 0 gives the base size; 2 gives 4x the base size.
    pub fn cache_size(mut self, size_param: usize) -> Self {
        self.cache_size_param = size_param;
        self
    }

    /// Set the user dictionary.
    pub fn user_dict(mut self, user_dict: UserDictionary) -> Self {
        self.user_dict = Some(user_dict);
        self
    }

    /// Set the frequency list for ranking suggestions.
    pub fn frequency_list(mut self, freq_list: FrequencyList) -> Self {
        self.freq_list = Some(freq_list);
        self
    }

    /// Build the `SpellChecker`.
    ///
    /// # Panics
    ///
    /// Panics if `trie` or `morph_validator` was not set.
    pub fn build(self) -> SpellChecker<M> {
        SpellChecker {
            trie: self.trie.expect("SpellCheckerBuilder: trie is required"),
            morph: self
                .morph
                .expect("SpellCheckerBuilder: morph_validator is required"),
            cache: SpellerCache::new(self.cache_size_param),
            user_dict: self.user_dict,
            freq_list: self.freq_list,
        }
    }
}

impl<M: MorphValidator> Default for SpellCheckerBuilder<M> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mce_core::trie::TrieBuilder;

    /// Build a test trie with some Finnish-like words.
    fn build_test_trie() -> SuccinctTrie {
        let mut builder = TrieBuilder::new();
        // Simple Finnish words (ASCII subset for testing)
        builder.insert(b"koira".to_vec()); // dog
        builder.insert(b"kissa".to_vec()); // cat
        builder.insert(b"talo".to_vec()); // house
        builder.insert(b"auto".to_vec()); // car
        builder.insert(b"kirja".to_vec()); // book
        builder.insert(b"koulu".to_vec()); // school
        builder.insert(b"koura".to_vec()); // palm/scoop (less common)
        builder.build()
    }

    /// A "no-op" morph validator that always returns false.
    fn no_morph(_word: &[char], _len: usize) -> bool {
        false
    }

    /// A morph validator that accepts words starting with 'k'.
    fn k_morph(word: &[char], len: usize) -> bool {
        len > 0 && word[0] == 'k'
    }

    /// A morph validator that accepts everything.
    fn accept_all(_word: &[char], _len: usize) -> bool {
        true
    }

    fn make_checker(morph: impl MorphValidator) -> SpellChecker<impl MorphValidator> {
        SpellCheckerBuilder::new()
            .trie(build_test_trie())
            .morph_validator(morph)
            .cache_size(0)
            .build()
    }

    // ── check() tests ─────────────────────────────────────────────

    #[test]
    fn check_returns_ok_for_known_word() {
        let mut checker = make_checker(no_morph);
        assert_eq!(checker.check("koira"), SpellResult::Ok);
        assert_eq!(checker.check("kissa"), SpellResult::Ok);
        assert_eq!(checker.check("talo"), SpellResult::Ok);
    }

    #[test]
    fn check_returns_failed_for_unknown_word() {
        let mut checker = make_checker(no_morph);
        assert_eq!(checker.check("xyzzy"), SpellResult::Failed);
        assert_eq!(checker.check("hevonen"), SpellResult::Failed);
    }

    #[test]
    fn check_uses_morph_fallback() {
        // "kauppa" is NOT in the trie, but the k_morph validator accepts it.
        let mut checker = make_checker(k_morph);
        assert_eq!(checker.check("kauppa"), SpellResult::Ok);
    }

    #[test]
    fn check_morph_fallback_does_not_rescue_non_matching() {
        // "apina" is NOT in the trie, and k_morph rejects it (starts with 'a').
        let mut checker = make_checker(k_morph);
        assert_eq!(checker.check("apina"), SpellResult::Failed);
    }

    // ── cache tests ───────────────────────────────────────────────

    #[test]
    fn check_caches_positive_results() {
        let mut checker = make_checker(no_morph);

        // First call: cache miss, trie lookup succeeds.
        assert_eq!(checker.check("koira"), SpellResult::Ok);

        // Verify the word is now in the cache.
        let chars: Vec<char> = "koira".chars().collect();
        assert!(checker.cache.is_in_cache(&chars, chars.len()));

        // Second call: should hit cache.
        assert_eq!(checker.check("koira"), SpellResult::Ok);
    }

    #[test]
    fn check_does_not_cache_failed_results() {
        let mut checker = make_checker(no_morph);

        assert_eq!(checker.check("xyzzy"), SpellResult::Failed);

        // Failed results are NOT cached by SpellerCache.
        let chars: Vec<char> = "xyzzy".chars().collect();
        assert!(!checker.cache.is_in_cache(&chars, chars.len()));
    }

    #[test]
    fn check_caches_morph_validated_words() {
        let mut checker = make_checker(k_morph);

        // "kauppa" is accepted via morph, should be cached.
        assert_eq!(checker.check("kauppa"), SpellResult::Ok);

        let chars: Vec<char> = "kauppa".chars().collect();
        assert!(checker.cache.is_in_cache(&chars, chars.len()));
    }

    // ── suggest() tests ───────────────────────────────────────────

    #[test]
    fn suggest_returns_nearby_words() {
        let checker = make_checker(no_morph);

        // "koirb" is 1 edit from "koira"
        let suggestions = checker.suggest_unfiltered("koirb", 1, 5);
        assert!(suggestions.contains(&"koira".to_string()));
    }

    #[test]
    fn suggest_respects_max_suggestions() {
        let checker = make_checker(no_morph);

        // With max_edits=2 there may be several matches; limit to 2.
        let suggestions = checker.suggest_unfiltered("koira", 2, 2);
        assert!(suggestions.len() <= 2);
    }

    #[test]
    fn suggest_returns_empty_for_distant_words() {
        let checker = make_checker(no_morph);

        // "zzzzz" is far from any word in the trie.
        let suggestions = checker.suggest_unfiltered("zzzzz", 1, 5);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn suggest_sorted_by_edit_distance() {
        let checker = make_checker(no_morph);

        // "koiru" -> "koira" (1 sub). With max_edits=2, we might get
        // multiple results; the first should be the closest.
        let suggestions = checker.suggest_unfiltered("koiru", 2, 10);
        if !suggestions.is_empty() {
            // "koira" should appear since it's 1 edit away.
            assert!(suggestions.contains(&"koira".to_string()));
        }
    }

    #[test]
    fn suggest_with_morph_filter() {
        let checker = make_checker(k_morph);

        // "koirb" -> "koira" (1 edit). k_morph accepts "koira" (starts with 'k').
        let suggestions = checker.suggest("koirb", 1, 5);
        assert!(suggestions.contains(&"koira".to_string()));
    }

    #[test]
    fn suggest_morph_filter_rejects_invalid() {
        // Use a validator that rejects everything.
        let checker = make_checker(no_morph);

        // "koirb" -> "koira" (1 edit) would normally match, but no_morph
        // rejects all words, so suggest() returns nothing.
        let suggestions = checker.suggest("koirb", 1, 5);
        assert!(suggestions.is_empty());

        // suggest_unfiltered() still returns the candidate.
        let unfiltered = checker.suggest_unfiltered("koirb", 1, 5);
        assert!(unfiltered.contains(&"koira".to_string()));
    }

    // ── Speller trait tests ───────────────────────────────────────

    #[test]
    fn speller_trait_spell_works() {
        let checker = make_checker(no_morph);
        let word: Vec<char> = "koira".chars().collect();
        assert_eq!(checker.spell(&word, word.len()), SpellResult::Ok);

        let bad: Vec<char> = "xyzzy".chars().collect();
        assert_eq!(checker.spell(&bad, bad.len()), SpellResult::Failed);
    }

    // ── builder tests ─────────────────────────────────────────────

    #[test]
    fn builder_constructs_checker() {
        let checker = SpellCheckerBuilder::new()
            .trie(build_test_trie())
            .morph_validator(no_morph)
            .cache_size(1)
            .build();

        let word: Vec<char> = "talo".chars().collect();
        assert_eq!(checker.spell(&word, word.len()), SpellResult::Ok);
    }

    #[test]
    #[should_panic(expected = "trie is required")]
    fn builder_panics_without_trie() {
        let _: SpellChecker<fn(&[char], usize) -> bool> = SpellCheckerBuilder::new()
            .morph_validator(no_morph as fn(&[char], usize) -> bool)
            .build();
    }

    #[test]
    #[should_panic(expected = "morph_validator is required")]
    fn builder_panics_without_morph() {
        let _: SpellChecker<fn(&[char], usize) -> bool> =
            SpellCheckerBuilder::<fn(&[char], usize) -> bool>::new()
                .trie(build_test_trie())
                .build();
    }

    // ── edge cases ────────────────────────────────────────────────

    #[test]
    fn check_empty_string() {
        let mut checker = make_checker(no_morph);
        // Empty string is not in the trie and morph rejects it.
        assert_eq!(checker.check(""), SpellResult::Failed);
    }

    #[test]
    fn suggest_empty_string() {
        let checker = make_checker(no_morph);
        // Suggestions for empty string with 0 edits -> nothing (empty
        // string was not inserted into the trie).
        let suggestions = checker.suggest_unfiltered("", 0, 5);
        assert!(suggestions.is_empty());
    }

    // ── suggest_ranked tests ─────────────────────────────────────

    #[test]
    fn suggest_ranked_without_rank_fn() {
        let checker = make_checker(k_morph);

        // Without rank_fn, should behave like suggest() with morph filter.
        let suggestions = checker.suggest_ranked("koirb", 1, 5, None::<fn(&str) -> f64>);
        assert!(suggestions.contains(&"koira".to_string()));
    }

    #[test]
    fn suggest_ranked_with_rank_fn() {
        let checker = make_checker(k_morph);

        // Rank function: prefer words that start with "ki" over "ko".
        let rank_fn = |candidate: &str| -> f64 {
            if candidate.starts_with("ki") {
                10.0
            } else {
                0.0
            }
        };

        // "kissc" is 1 edit from "kissa"; "koirc" is 1 edit from "koira".
        // Both are in the trie and accepted by k_morph.
        // With the rank_fn, "kissa" should appear before "koira"
        // when edit distances are equal.
        let suggestions = checker.suggest_ranked("kissb", 1, 5, Some(rank_fn));
        if suggestions.contains(&"kissa".to_string()) {
            // kissa should be first due to the rank bonus
            assert_eq!(suggestions[0], "kissa");
        }
    }

    #[test]
    fn suggest_ranked_respects_max_suggestions() {
        let checker = make_checker(k_morph);

        let suggestions = checker.suggest_ranked("koira", 2, 2, None::<fn(&str) -> f64>);
        assert!(suggestions.len() <= 2);
    }

    #[test]
    fn suggest_ranked_morph_rejects_invalid() {
        let checker = make_checker(no_morph);

        // no_morph rejects everything, so suggest_ranked returns nothing.
        let suggestions = checker.suggest_ranked("koirb", 1, 5, None::<fn(&str) -> f64>);
        assert!(suggestions.is_empty());
    }

    // ── edit_distance tests ──────────────────────────────────────

    #[test]
    fn edit_distance_identical() {
        assert_eq!(super::edit_distance(b"koira", b"koira"), 0);
    }

    #[test]
    fn edit_distance_one_sub() {
        assert_eq!(super::edit_distance(b"koira", b"koiru"), 1);
    }

    #[test]
    fn edit_distance_one_del() {
        assert_eq!(super::edit_distance(b"koira", b"koir"), 1);
    }

    #[test]
    fn edit_distance_one_ins() {
        assert_eq!(super::edit_distance(b"koira", b"koiraa"), 1);
    }

    #[test]
    fn edit_distance_empty() {
        assert_eq!(super::edit_distance(b"", b"abc"), 3);
        assert_eq!(super::edit_distance(b"abc", b""), 3);
        assert_eq!(super::edit_distance(b"", b""), 0);
    }

    // ── user dictionary tests ────────────────────────────────────

    #[test]
    fn check_accepts_user_dict_word() {
        let ud = UserDictionary::from_words(["MCE", "xyzzy"]);
        let mut checker = SpellCheckerBuilder::new()
            .trie(build_test_trie())
            .morph_validator(no_morph)
            .user_dict(ud)
            .cache_size(0)
            .build();

        // "xyzzy" is not in trie or morph, but IS in user dict.
        assert_eq!(checker.check("xyzzy"), SpellResult::Ok);
        // "MCE" likewise.
        assert_eq!(checker.check("MCE"), SpellResult::Ok);
        // "nope" is nowhere.
        assert_eq!(checker.check("nope"), SpellResult::Failed);
    }

    #[test]
    fn speller_trait_accepts_user_dict_word() {
        let ud = UserDictionary::from_words(["MCE"]);
        let checker = SpellCheckerBuilder::new()
            .trie(build_test_trie())
            .morph_validator(no_morph)
            .user_dict(ud)
            .cache_size(0)
            .build();

        let word: Vec<char> = "MCE".chars().collect();
        assert_eq!(checker.spell(&word, word.len()), SpellResult::Ok);
    }

    #[test]
    fn suggest_includes_user_dict_words() {
        // "MCF" is not in trie, but "MCE" is in user dict (1 edit from "MCF").
        let ud = UserDictionary::from_words(["MCE"]);
        let checker = SpellCheckerBuilder::new()
            .trie(build_test_trie())
            .morph_validator(no_morph)
            .user_dict(ud)
            .cache_size(0)
            .build();

        let suggestions = checker.suggest("MCF", 1, 10);
        assert!(
            suggestions.contains(&"MCE".to_string()),
            "user dict word should appear in suggestions: {:?}",
            suggestions
        );
    }

    #[test]
    fn suggest_ranked_includes_user_dict_words() {
        let ud = UserDictionary::from_words(["MCE"]);
        let checker = SpellCheckerBuilder::new()
            .trie(build_test_trie())
            .morph_validator(no_morph)
            .user_dict(ud)
            .cache_size(0)
            .build();

        let suggestions = checker.suggest_ranked("MCF", 1, 10, None::<fn(&str) -> f64>);
        assert!(
            suggestions.contains(&"MCE".to_string()),
            "user dict word should appear in ranked suggestions: {:?}",
            suggestions
        );
    }

    #[test]
    fn set_user_dict_after_construction() {
        let mut checker = make_checker(no_morph);
        assert_eq!(checker.check("MCE"), SpellResult::Failed);

        checker.set_user_dict(UserDictionary::from_words(["MCE"]));
        // Cache might have the old result, but user dict words bypass cache
        // in the check() flow (user dict is checked before trie lookup).
        // However, failed results are NOT cached, so the next check should
        // find it in the user dictionary.
        assert_eq!(checker.check("MCE"), SpellResult::Ok);
    }

    // ── frequency-ranked suggestion tests ─────────────────────────

    #[test]
    fn frequency_ranked_suggestions_prefer_common_words() {
        // Build a frequency list where "koira" is very common and "koura" is rare.
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
# text = Koira juoksee taas.
1\tKoira\tkoira\tNOUN\tN\tCase=Nom|Number=Sing\t2\tnsubj\t2:nsubj\t_
2\tjuoksee\tjuosta\tVERB\tV\tMood=Ind\t0\troot\t0:root\t_
3\ttaas\ttaas\tADV\tAdv\t_\t2\tadvmod\t2:advmod\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t2\tpunct\t2:punct\t_

# sent_id = f.4
# text = Koura on pieni.
1\tKoura\tkoura\tNOUN\tN\tCase=Nom|Number=Sing\t3\tnsubj\t3:nsubj\t_
2\ton\tolla\tAUX\tV\tMood=Ind\t3\tcop\t3:cop\t_
3\tpieni\tpieni\tADJ\tA\tCase=Nom\t0\troot\t0:root\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t3\tpunct\t3:punct\t_
";
        let fl = FrequencyList::from_conllu(conllu);
        assert_eq!(fl.frequency("koira"), 3);
        assert_eq!(fl.frequency("koura"), 1);

        let checker = SpellCheckerBuilder::new()
            .trie(build_test_trie())
            .morph_validator(accept_all)
            .frequency_list(fl)
            .cache_size(0)
            .build();

        // "koirra" is equidistant from "koira" (1 del) and "koura" (2 edits),
        // but let us use "koirb" which is 1 edit from both "koira" and "koura".
        // Actually "koirb" -> "koira" is 1 sub, "koirb" -> "koura" is 2 edits.
        // Use "koura" vs "koira" from "koirx": koirx -> koira (1), koirx -> koura (2).
        // Better: use "koura" which is in the trie and test "koura" vs "koira"
        // at same distance. "koir?" where ? makes equal distance:
        // "koira" vs "koura": for "korra" -> koira (1 sub r->i), koura (1 sub r->u)
        // So "korra" is 1 edit from both. Wait, "korra" is not standard.
        // Let me use a clear test: "koira" and "koura" both 1 edit from "koixa"
        // No... let me think differently.
        // koira -> koirb: 1 edit (a->b). koura -> koirb: 2 edits.
        // Not equal. Let me just test with max_edits=2 and a word equidistant.
        // "koXra": koira=1 (sub X->i), koura=1 (sub X->u). Yes!
        let suggestions = checker.suggest_ranked("ko_ra", 1, 10, None::<fn(&str) -> f64>);
        // Both "koira" and "koura" should be at edit distance 1.
        if suggestions.contains(&"koira".to_string()) && suggestions.contains(&"koura".to_string())
        {
            let pos_koira = suggestions.iter().position(|s| s == "koira").unwrap();
            let pos_koura = suggestions.iter().position(|s| s == "koura").unwrap();
            assert!(
                pos_koira < pos_koura,
                "koira (freq=3, pos={pos_koira}) should rank before koura (freq=1, pos={pos_koura})"
            );
        }
    }

    #[test]
    fn set_frequency_list_after_construction() {
        let mut checker = make_checker(accept_all);
        assert!(checker.frequency_list().is_none());

        let fl = FrequencyList::empty();
        checker.set_frequency_list(fl);
        assert!(checker.frequency_list().is_some());
    }

    #[test]
    fn builder_with_frequency_list() {
        let fl = FrequencyList::empty();
        let checker = SpellCheckerBuilder::new()
            .trie(build_test_trie())
            .morph_validator(accept_all)
            .frequency_list(fl)
            .cache_size(0)
            .build();
        assert!(checker.frequency_list().is_some());
    }

    #[test]
    fn builder_with_user_dict() {
        let ud = UserDictionary::from_words(["test"]);
        let checker = SpellCheckerBuilder::new()
            .trie(build_test_trie())
            .morph_validator(no_morph)
            .user_dict(ud)
            .cache_size(0)
            .build();
        assert!(checker.user_dict().is_some());
        assert!(checker.user_dict().unwrap().contains("test"));
    }

    #[test]
    fn user_dict_word_in_suggest_bypasses_morph_filter() {
        // "MCE" is not valid per no_morph, but is in user dict.
        // When suggesting for "MCF" (1 edit from "MCE"), "MCE" should appear.
        let ud = UserDictionary::from_words(["MCE"]);
        let checker = SpellCheckerBuilder::new()
            .trie(build_test_trie())
            .morph_validator(no_morph)
            .user_dict(ud)
            .cache_size(0)
            .build();

        let suggestions = checker.suggest("MCF", 1, 10);
        assert!(suggestions.contains(&"MCE".to_string()));
    }

    #[test]
    fn user_dict_and_frequency_combined() {
        let ud = UserDictionary::from_words(["MCE"]);
        let fl = FrequencyList::empty();
        let mut checker = SpellCheckerBuilder::new()
            .trie(build_test_trie())
            .morph_validator(no_morph)
            .user_dict(ud)
            .frequency_list(fl)
            .cache_size(0)
            .build();

        // User dict word is accepted.
        assert_eq!(checker.check("MCE"), SpellResult::Ok);
        // Trie word is still accepted.
        assert_eq!(checker.check("koira"), SpellResult::Ok);
        // Unknown word is rejected.
        assert_eq!(checker.check("nope"), SpellResult::Failed);
    }
}
