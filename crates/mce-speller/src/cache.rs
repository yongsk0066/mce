// Adapted from corevoikko (voikko-fi/speller/cache.rs)
//
// Hash-based spell result cache for words up to 10 characters.

use crate::{SpellResult, Speller};

/// Maximum word length that can be cached.
const MAX_CACHED_WORD_LEN: usize = 10;

/// Hash orders for each word length (1-indexed, index 0 unused).
const HASH_ORDERS: [i32; 11] = [0, 3, 5, 6, 7, 7, 7, 7, 7, 7, 7];

/// Offsets into the word storage array for each word length.
const CACHE_OFFSETS: [usize; 11] = [0, 0, 16, 80, 272, 784, 1424, 2192, 3088, 4112, 5264];

/// Offsets into the result metadata array for each word length.
const META_OFFSETS: [usize; 11] = [0, 0, 16, 48, 112, 240, 368, 496, 624, 752, 880];

/// Total number of characters in the word storage (before size_param scaling).
const BASE_WORD_COUNT: usize = 6544;

/// Total number of entries in the result metadata (before size_param scaling).
const BASE_META_COUNT: usize = 1008;

/// Simple string hashing algorithm.
fn voikko_hash(word: &[char], len: usize, order: i32) -> usize {
    let mut hash: i32 = 0;
    for &ch in &word[..len] {
        hash = (hash.wrapping_mul(37).wrapping_add(ch as i32)) % (1 << order);
    }
    hash as usize
}

/// A fixed-size, hash-based cache for spell results.
///
/// Only caches [`SpellResult::Ok`] and [`SpellResult::CapitalizeFirst`] results
/// for words up to 10 characters long. Failed results are NOT cached.
///
/// The cache uses a simple replacement strategy: hash collisions overwrite
/// silently (no chaining or LRU).
pub struct SpellerCache {
    size_param: usize,
    /// Stores word characters in a flat array, organized by word length.
    words: Vec<char>,
    /// Stores spell results: 'p' = Ok, 'i' = CapitalizeFirst.
    spell_results: Vec<u8>,
}

impl SpellerCache {
    /// Create a new cache with the given size parameter.
    ///
    /// `size_param` scales the cache: total word storage is
    /// `6544 * (1 << size_param)` characters. A value of 0 gives the base size.
    pub fn new(size_param: usize) -> Self {
        let word_count = BASE_WORD_COUNT << size_param;
        let meta_count = BASE_META_COUNT << size_param;
        Self {
            size_param,
            words: vec!['\0'; word_count],
            spell_results: vec![0; meta_count],
        }
    }

    /// Check whether a word is present in the cache.
    ///
    /// Returns `false` for words longer than 10 characters.
    pub fn is_in_cache(&self, word: &[char], wlen: usize) -> bool {
        if wlen == 0 || wlen > MAX_CACHED_WORD_LEN {
            return false;
        }
        let hash_code = voikko_hash(word, wlen, HASH_ORDERS[wlen] + self.size_param as i32);
        let cache_offset = (CACHE_OFFSETS[wlen] << self.size_param) + hash_code * wlen;

        // Compare the cached word with the input
        if cache_offset + wlen > self.words.len() {
            return false;
        }
        self.words[cache_offset..cache_offset + wlen] == word[..wlen]
    }

    /// Get the cached spell result for a word.
    ///
    /// **Precondition**: The word must be in the cache (call `is_in_cache` first).
    pub fn get_spell_result(&self, word: &[char], wlen: usize) -> SpellResult {
        let hash_code = voikko_hash(word, wlen, HASH_ORDERS[wlen] + self.size_param as i32);
        let result_offset = (META_OFFSETS[wlen] << self.size_param) + hash_code;

        if self.spell_results[result_offset] == b'i' {
            SpellResult::CapitalizeFirst
        } else {
            SpellResult::Ok
        }
    }

    /// Store a spell result in the cache.
    ///
    /// Only [`SpellResult::Ok`] and [`SpellResult::CapitalizeFirst`] are cached.
    /// Other results (Failed, CapitalizationError) are silently ignored.
    /// Words longer than 10 characters are also ignored.
    pub fn set_spell_result(&mut self, word: &[char], wlen: usize, result: SpellResult) {
        if wlen == 0
            || wlen > MAX_CACHED_WORD_LEN
            || (result != SpellResult::Ok && result != SpellResult::CapitalizeFirst)
        {
            return;
        }
        let hash_code = voikko_hash(word, wlen, HASH_ORDERS[wlen] + self.size_param as i32);
        let cache_offset = (CACHE_OFFSETS[wlen] << self.size_param) + hash_code * wlen;
        let result_offset = (META_OFFSETS[wlen] << self.size_param) + hash_code;

        // Store the word characters
        if cache_offset + wlen <= self.words.len() {
            self.words[cache_offset..cache_offset + wlen].copy_from_slice(&word[..wlen]);
        }

        // Store the result marker
        if result_offset < self.spell_results.len() {
            self.spell_results[result_offset] = if result == SpellResult::Ok {
                b'p'
            } else {
                b'i'
            };
        }
    }

    /// Look up a word in the cache, calling the speller on a miss.
    ///
    /// On a cache miss, the speller is invoked and the result is stored.
    pub fn spell_with_cache(
        &mut self,
        word: &[char],
        wlen: usize,
        speller: &dyn Speller,
    ) -> SpellResult {
        if self.is_in_cache(word, wlen) {
            return self.get_spell_result(word, wlen);
        }
        let result = speller.spell(word, wlen);
        self.set_spell_result(word, wlen, result);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn new_cache_has_no_entries() {
        let cache = SpellerCache::new(0);
        let word = chars("koira");
        assert!(!cache.is_in_cache(&word, word.len()));
    }

    #[test]
    fn set_and_get_ok_result() {
        let mut cache = SpellerCache::new(0);
        let word = chars("koira");
        cache.set_spell_result(&word, word.len(), SpellResult::Ok);
        assert!(cache.is_in_cache(&word, word.len()));
        assert_eq!(cache.get_spell_result(&word, word.len()), SpellResult::Ok);
    }

    #[test]
    fn set_and_get_cap_first_result() {
        let mut cache = SpellerCache::new(0);
        let word = chars("helsinki");
        cache.set_spell_result(&word, word.len(), SpellResult::CapitalizeFirst);
        assert!(cache.is_in_cache(&word, word.len()));
        assert_eq!(
            cache.get_spell_result(&word, word.len()),
            SpellResult::CapitalizeFirst
        );
    }

    #[test]
    fn failed_result_is_not_cached() {
        let mut cache = SpellerCache::new(0);
        let word = chars("xyzzy");
        cache.set_spell_result(&word, word.len(), SpellResult::Failed);
        assert!(!cache.is_in_cache(&word, word.len()));
    }

    #[test]
    fn cap_error_is_not_cached() {
        let mut cache = SpellerCache::new(0);
        let word = chars("koIra");
        cache.set_spell_result(&word, word.len(), SpellResult::CapitalizationError);
        assert!(!cache.is_in_cache(&word, word.len()));
    }

    #[test]
    fn word_longer_than_10_is_not_cached() {
        let mut cache = SpellerCache::new(0);
        let word = chars("pitkasanainen");
        assert!(word.len() > MAX_CACHED_WORD_LEN);
        cache.set_spell_result(&word, word.len(), SpellResult::Ok);
        assert!(!cache.is_in_cache(&word, word.len()));
    }

    #[test]
    fn single_char_word_is_cached() {
        let mut cache = SpellerCache::new(0);
        let word = chars("a");
        cache.set_spell_result(&word, word.len(), SpellResult::Ok);
        assert!(cache.is_in_cache(&word, word.len()));
        assert_eq!(cache.get_spell_result(&word, word.len()), SpellResult::Ok);
    }

    #[test]
    fn ten_char_word_is_cached() {
        let mut cache = SpellerCache::new(0);
        let word = chars("abcdefghij");
        assert_eq!(word.len(), 10);
        cache.set_spell_result(&word, word.len(), SpellResult::Ok);
        assert!(cache.is_in_cache(&word, word.len()));
    }

    #[test]
    fn different_words_do_not_collide_usually() {
        let mut cache = SpellerCache::new(0);
        let w1 = chars("koira");
        let w2 = chars("kissa");
        cache.set_spell_result(&w1, w1.len(), SpellResult::Ok);
        cache.set_spell_result(&w2, w2.len(), SpellResult::CapitalizeFirst);

        // At least the last-written word should be in cache
        // (hash collision may evict w1)
        assert!(cache.is_in_cache(&w2, w2.len()));
        assert_eq!(
            cache.get_spell_result(&w2, w2.len()),
            SpellResult::CapitalizeFirst
        );
    }

    #[test]
    fn larger_size_param_works() {
        let mut cache = SpellerCache::new(2);
        let word = chars("koira");
        cache.set_spell_result(&word, word.len(), SpellResult::Ok);
        assert!(cache.is_in_cache(&word, word.len()));
        assert_eq!(cache.get_spell_result(&word, word.len()), SpellResult::Ok);
    }

    #[test]
    fn spell_with_cache_calls_speller_on_miss() {
        struct OkSpeller;
        impl Speller for OkSpeller {
            fn spell(&self, _word: &[char], _word_len: usize) -> SpellResult {
                SpellResult::Ok
            }
        }

        let mut cache = SpellerCache::new(0);
        let speller = OkSpeller;
        let word = chars("koira");

        // First call: cache miss, calls speller
        let result = cache.spell_with_cache(&word, word.len(), &speller);
        assert_eq!(result, SpellResult::Ok);

        // Second call: cache hit, does not call speller
        assert!(cache.is_in_cache(&word, word.len()));
        let result = cache.spell_with_cache(&word, word.len(), &speller);
        assert_eq!(result, SpellResult::Ok);
    }

    #[test]
    fn spell_with_cache_does_not_cache_failed() {
        struct FailSpeller;
        impl Speller for FailSpeller {
            fn spell(&self, _word: &[char], _word_len: usize) -> SpellResult {
                SpellResult::Failed
            }
        }

        let mut cache = SpellerCache::new(0);
        let speller = FailSpeller;
        let word = chars("xyzzy");

        let result = cache.spell_with_cache(&word, word.len(), &speller);
        assert_eq!(result, SpellResult::Failed);
        assert!(!cache.is_in_cache(&word, word.len()));
    }

    #[test]
    fn empty_word_is_not_cached() {
        let mut cache = SpellerCache::new(0);
        cache.set_spell_result(&[], 0, SpellResult::Ok);
        assert!(!cache.is_in_cache(&[], 0));
    }

    #[test]
    fn hash_function_deterministic() {
        let word = chars("koira");
        let h1 = voikko_hash(&word, word.len(), 7);
        let h2 = voikko_hash(&word, word.len(), 7);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_function_different_words_usually_differ() {
        let w1 = chars("koira");
        let w2 = chars("kissa");
        let h1 = voikko_hash(&w1, w1.len(), 7);
        let h2 = voikko_hash(&w2, w2.len(), 7);
        // Not guaranteed but very likely for these particular words
        assert_ne!(h1, h2);
    }
}
