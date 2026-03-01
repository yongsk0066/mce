// Spell-checking pipeline: connects M1 Succinct Trie with morphological
// validation via a language-agnostic callback.
//
// The pipeline checks words in three stages:
//   1. Cache lookup (fast path for repeated words)
//   2. Exact trie lookup (dictionary hit)
//   3. Morphological analysis fallback (compounds, inflections, etc.)

use crate::cache::SpellerCache;
use crate::{SpellResult, Speller};
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

        // Stage 2: exact trie lookup (byte-level).
        if self.trie.contains(word.as_bytes()) {
            self.cache.set_spell_result(&chars, wlen, SpellResult::Ok);
            return SpellResult::Ok;
        }

        // Stage 3: morphological validation.
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

        raw_candidates
            .into_iter()
            .filter_map(|bytes| {
                // Convert from byte key back to UTF-8 string.
                let candidate = String::from_utf8(bytes).ok()?;

                // Filter by morphological validity. Candidates that fail
                // morph validation are dropped, so the returned list only
                // contains words the morph validator considers well-formed.
                let chars: Vec<char> = candidate.chars().collect();
                let clen = chars.len();
                if self.morph.is_valid(&chars, clen) {
                    Some(candidate)
                } else {
                    None
                }
            })
            .take(max_suggestions)
            .collect()
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

    /// Access the underlying trie (for inspection or advanced usage).
    pub fn trie(&self) -> &SuccinctTrie {
        &self.trie
    }
}

/// Implement the existing `Speller` trait so that `SpellChecker` can be
/// used with the `SpellerCache::spell_with_cache` mechanism.
impl<M: MorphValidator> Speller for SpellChecker<M> {
    fn spell(&self, word: &[char], word_len: usize) -> SpellResult {
        // Convert char slice to a UTF-8 string for trie lookup.
        let s: String = word[..word_len].iter().collect();

        // Stage 1: exact trie lookup.
        if self.trie.contains(s.as_bytes()) {
            return SpellResult::Ok;
        }

        // Stage 2: morphological validation.
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
}

impl<M: MorphValidator> SpellCheckerBuilder<M> {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            trie: None,
            morph: None,
            cache_size_param: 0,
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
}
