// Finnish hyphenation engine.
//
// Implements syllable-based hyphenation for Finnish words. Finnish syllabification
// follows regular phonotactic rules:
//
//   1. Each syllable has exactly one vowel nucleus (a vowel, long vowel, or diphthong).
//   2. Consonants between nuclei: at most one onset consonant goes with the following
//      syllable; the rest attach as coda to the preceding syllable.
//      Exception: certain foreign-origin onset clusters (pr, pl, tr, kr, kl, ...) stay together.
//   3. Diphthongs are indivisible: ai, ei, oi, ui, yi, äi, öi, au, eu, ou, iu, äy, öy,
//      ie, uo, yö.
//   4. Long vowels (aa, ee, ii, oo, uu, yy, ää, öö) are indivisible.
//   5. Hyphenation points cannot leave a single character alone at the start or end.
//
// This module is purely algorithmic and does not require a dictionary or FST.
// For compound-aware hyphenation (splitting at morpheme boundaries), callers
// should perform compound analysis first and then hyphenate each component.

use mce_core::character::simple_lower;

use crate::is_vowel;

/// Finnish diphthongs — vowel pairs that form a single syllable nucleus.
///
/// These 16 diphthongs should not be split when syllabifying.
/// Source: Finnish phonology reference (Karlsson 1983; Suomi, Toivanen & Ylitalo 2008).
const DIPHTHONGS: &[[char; 2]] = &[
    // i-final
    ['a', 'i'],
    ['e', 'i'],
    ['o', 'i'],
    ['u', 'i'],
    ['y', 'i'],
    ['\u{00E4}', 'i'], // äi
    ['\u{00F6}', 'i'], // öi
    // u-final
    ['a', 'u'],
    ['e', 'u'],
    ['o', 'u'],
    ['i', 'u'],
    // y-final
    ['\u{00E4}', 'y'], // äy
    ['\u{00F6}', 'y'], // öy
    // opening diphthongs
    ['i', 'e'],
    ['u', 'o'],
    ['y', '\u{00F6}'], // yö
];

/// Foreign-origin onset clusters that stay together before a vowel.
///
/// In native Finnish, onsets are at most one consonant, but loanwords
/// may have these clusters at word-initial position. We keep them together
/// so that e.g. "stra-te-gi-a" is not split as "st-ra-te-gi-a".
///
/// Note: these only apply word-initially. Between syllables within a word,
/// the native single-consonant onset rule is used.
#[cfg(test)]
const FOREIGN_ONSETS: &[&[char]] = &[
    &['p', 'r'],
    &['p', 'l'],
    &['t', 'r'],
    &['k', 'r'],
    &['k', 'l'],
    &['k', 'n'],
    &['g', 'r'],
    &['g', 'l'],
    &['b', 'r'],
    &['b', 'l'],
    &['f', 'r'],
    &['f', 'l'],
    &['s', 'k'],
    &['s', 'p'],
    &['s', 't'],
    &['s', 'n'],
    &['s', 'l'],
    &['s', 'm'],
    &['s', 'v'],
    &['s', 'k', 'r'],
    &['s', 'p', 'r'],
    &['s', 't', 'r'],
];

/// Finnish hyphenation engine.
///
/// Provides pure rule-based syllable hyphenation for Finnish words. Does not
/// require a dictionary — it operates on phonotactic patterns alone.
///
/// # Example
///
/// ```
/// use mce_fi::hyphenation::FinnishHyphenator;
///
/// let h = FinnishHyphenator::new();
/// assert_eq!(h.hyphenate_word("koulu"), "kou-lu");
/// assert_eq!(h.hyphenate_word("kartta"), "kart-ta");
/// ```
pub struct FinnishHyphenator {
    /// Minimum number of characters that must remain on either side of a
    /// hyphenation point. Prevents single-character fragments at line breaks.
    min_fragment: usize,
}

impl Default for FinnishHyphenator {
    fn default() -> Self {
        Self::new()
    }
}

impl FinnishHyphenator {
    /// Create a new `FinnishHyphenator` with default settings.
    ///
    /// The default minimum fragment size is 2, meaning a break will not be
    /// placed if it would leave fewer than 2 characters on either side.
    pub fn new() -> Self {
        Self { min_fragment: 2 }
    }

    /// Create a hyphenator with a custom minimum fragment size.
    ///
    /// `min_fragment` controls the minimum number of characters on each side
    /// of a hyphenation point. Setting this to 1 allows single-character
    /// fragments (ugly but valid); setting it to 3 or more is more conservative.
    pub fn with_min_fragment(min_fragment: usize) -> Self {
        Self {
            min_fragment: min_fragment.max(1),
        }
    }

    /// Find valid hyphenation points in a Finnish word.
    ///
    /// Returns a vector of **char offsets** (0-based) where a hyphen may be
    /// inserted. Each offset indicates a break *before* the character at that
    /// position. For example, for "kou-lu" the returned offset is `[3]`
    /// (break before the 'l').
    pub fn hyphenate(&self, word: &str) -> Vec<usize> {
        let chars: Vec<char> = word.chars().collect();
        let len = chars.len();

        if len < self.min_fragment * 2 {
            return Vec::new();
        }

        let lower: Vec<char> = chars.iter().map(|&c| simple_lower(c)).collect();
        let syllables = syllabify_chars(&lower);

        if syllables.len() <= 1 {
            return Vec::new();
        }

        let mut points = Vec::new();
        let mut offset = 0;
        for (i, syl) in syllables.iter().enumerate() {
            if i > 0 {
                let chars_before = offset;
                let chars_after = len - offset;
                if chars_before >= self.min_fragment && chars_after >= self.min_fragment {
                    points.push(offset);
                }
            }
            offset += syl.len();
        }

        points
    }

    /// Return the word with soft hyphens (`-`) inserted at valid break points.
    pub fn hyphenate_word(&self, word: &str) -> String {
        let points = self.hyphenate(word);
        if points.is_empty() {
            return word.to_string();
        }

        let chars: Vec<char> = word.chars().collect();
        let mut result = String::with_capacity(word.len() + points.len());
        let mut pi = 0; // index into points

        for (i, &ch) in chars.iter().enumerate() {
            if pi < points.len() && i == points[pi] {
                result.push('-');
                pi += 1;
            }
            result.push(ch);
        }

        result
    }

    /// Return syllables of a Finnish word as owned `String`s.
    ///
    /// This is the public API for syllabification. The syllable boundaries
    /// correspond exactly to the hyphenation points (before applying the
    /// minimum-fragment filter).
    pub fn syllabify(&self, word: &str) -> Vec<String> {
        let chars: Vec<char> = word.chars().collect();
        let lower: Vec<char> = chars.iter().map(|&c| simple_lower(c)).collect();
        let boundaries = syllabify_chars(&lower);

        let mut result = Vec::with_capacity(boundaries.len());
        let mut offset = 0;
        for syl in &boundaries {
            let end = offset + syl.len();
            result.push(chars[offset..end].iter().collect());
            offset = end;
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Core syllabification
// ---------------------------------------------------------------------------

/// True if the vowel pair (both lowercase) forms a Finnish diphthong.
fn is_diphthong(a: char, b: char) -> bool {
    DIPHTHONGS.contains(&[a, b])
}

/// True if the lowercase consonant sequence forms a valid foreign onset cluster.
#[cfg(test)]
fn is_foreign_onset(chars: &[char]) -> bool {
    FOREIGN_ONSETS.iter().any(|onset| **onset == *chars)
}

/// Syllabify a word given as a slice of **lowercase** chars.
///
/// Returns a `Vec` of char slices, each representing one syllable.
/// The concatenation of all slices equals the input.
///
/// Algorithm:
/// 1. Classify each character as vowel or consonant.
/// 2. Identify vowel nuclei (single vowels, long vowels, or diphthongs).
///    Hiatus pairs are split into separate nuclei.
/// 3. Place syllable breaks between consecutive nuclei: for consonant
///    clusters, the last consonant goes with the following syllable
///    (single-consonant onset, the native Finnish rule).
fn syllabify_chars(word: &[char]) -> Vec<&[char]> {
    let len = word.len();
    if len == 0 {
        return Vec::new();
    }

    let is_v: Vec<bool> = word.iter().map(|&c| is_vowel(c)).collect();
    let nuclei = find_nuclei(word, &is_v);
    if nuclei.is_empty() {
        // No vowels at all — return the whole thing as one "syllable".
        return vec![word];
    }

    let mut breaks: Vec<usize> = Vec::new();

    for pair in nuclei.windows(2) {
        let (_, n1_end) = pair[0]; // end of first nucleus (exclusive)
        let (n2_start, _) = pair[1]; // start of second nucleus

        let cluster_len = n2_start - n1_end;

        if cluster_len == 0 {
            // Hiatus: break between adjacent nuclei.
            breaks.push(n2_start);
        } else if cluster_len == 1 {
            // Single consonant becomes onset of next syllable.
            breaks.push(n1_end);
        } else {
            // Native Finnish rule: single-consonant onset.
            breaks.push(n2_start - 1);
        }
    }

    let mut syllables = Vec::new();
    let mut start = 0;
    for &bp in &breaks {
        if bp > start {
            syllables.push(&word[start..bp]);
        }
        start = bp;
    }
    if start < len {
        syllables.push(&word[start..len]);
    }

    syllables.retain(|s| !s.is_empty());

    syllables
}

/// Find vowel nuclei in the word.
///
/// Returns a list of `(start, end)` char-index ranges (exclusive end) for
/// each nucleus. A nucleus is a long vowel, a diphthong, or a single vowel.
/// Hiatus pairs (two vowels that are neither a long vowel nor a diphthong)
/// are split into separate nuclei.
fn find_nuclei(word: &[char], is_v: &[bool]) -> Vec<(usize, usize)> {
    let len = word.len();
    let mut nuclei = Vec::new();
    let mut i = 0;

    while i < len {
        if !is_v[i] {
            i += 1;
            continue;
        }

        let start = i;
        i += 1;

        if i < len && is_v[i] {
            let a = word[start];
            let b = word[i];

            if a == b {
                // Long vowel.
                i += 1;
                nuclei.push((start, i));
                continue;
            }

            if is_diphthong(a, b) {
                i += 1;
                nuclei.push((start, i));
                continue;
            }

            // Hiatus: first vowel alone; second starts a new nucleus.
            nuclei.push((start, start + 1));
            continue;
        }

        nuclei.push((start, i));
    }

    nuclei
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helper
    // -----------------------------------------------------------------------

    fn hyph(word: &str) -> String {
        FinnishHyphenator::new().hyphenate_word(word)
    }

    fn syls(word: &str) -> Vec<String> {
        FinnishHyphenator::new().syllabify(word)
    }

    // -----------------------------------------------------------------------
    // Syllabification tests
    // -----------------------------------------------------------------------

    #[test]
    fn syllabify_cv_cv() {
        // ta-lo
        assert_eq!(syls("talo"), vec!["ta", "lo"]);
    }

    #[test]
    fn syllabify_diphthong_cv() {
        // kou-lu
        assert_eq!(syls("koulu"), vec!["kou", "lu"]);
    }

    #[test]
    fn syllabify_cvcc_cv() {
        // kart-ta
        assert_eq!(syls("kartta"), vec!["kart", "ta"]);
    }

    #[test]
    fn syllabify_cvcc_cv_2() {
        // lamp-pu
        assert_eq!(syls("lamppu"), vec!["lamp", "pu"]);
    }

    #[test]
    fn syllabify_long_vowel() {
        // aa-mu
        assert_eq!(syls("aamu"), vec!["aa", "mu"]);
    }

    #[test]
    fn syllabify_diphthong_single() {
        // yö — single diphthong, no split
        assert_eq!(syls("yö"), vec!["yö"]);
    }

    #[test]
    fn syllabify_multiple() {
        // suo-ma-lai-nen
        assert_eq!(syls("suomalainen"), vec!["suo", "ma", "lai", "nen"]);
    }

    #[test]
    fn syllabify_front_vowels() {
        // pöy-tä
        assert_eq!(syls("pöytä"), vec!["pöy", "tä"]);
    }

    #[test]
    fn syllabify_three_consonants() {
        // rans-ka  (rs goes to coda, k to onset)
        assert_eq!(syls("ranska"), vec!["rans", "ka"]);
    }

    #[test]
    fn syllabify_single_vowel_start() {
        // is-tu-a
        assert_eq!(syls("istua"), vec!["is", "tu", "a"]);
    }

    #[test]
    fn syllabify_hiatus() {
        // lu-e-taan  (u-e is hiatus, aa is long vowel)
        assert_eq!(syls("luetaan"), vec!["lu", "e", "taan"]);
    }

    #[test]
    fn syllabify_koira() {
        // koi-ra
        assert_eq!(syls("koira"), vec!["koi", "ra"]);
    }

    #[test]
    fn syllabify_kissa() {
        // kis-sa
        assert_eq!(syls("kissa"), vec!["kis", "sa"]);
    }

    #[test]
    fn syllabify_opettaja() {
        // o-pet-ta-ja
        assert_eq!(syls("opettaja"), vec!["o", "pet", "ta", "ja"]);
    }

    #[test]
    fn syllabify_helsinki() {
        // hel-sin-ki
        assert_eq!(syls("helsinki"), vec!["hel", "sin", "ki"]);
    }

    // -----------------------------------------------------------------------
    // Hyphenation tests (with min_fragment = 2 filter)
    // -----------------------------------------------------------------------

    #[test]
    fn hyphenate_koulu() {
        assert_eq!(hyph("koulu"), "kou-lu");
    }

    #[test]
    fn hyphenate_koira() {
        assert_eq!(hyph("koira"), "koi-ra");
    }

    #[test]
    fn hyphenate_kissa() {
        assert_eq!(hyph("kissa"), "kis-sa");
    }

    #[test]
    fn hyphenate_talo() {
        assert_eq!(hyph("talo"), "ta-lo");
    }

    #[test]
    fn hyphenate_suomalainen() {
        assert_eq!(hyph("suomalainen"), "suo-ma-lai-nen");
    }

    #[test]
    fn hyphenate_kartta() {
        assert_eq!(hyph("kartta"), "kart-ta");
    }

    #[test]
    fn hyphenate_lamppu() {
        assert_eq!(hyph("lamppu"), "lamp-pu");
    }

    #[test]
    fn hyphenate_poyta() {
        assert_eq!(hyph("pöytä"), "pöy-tä");
    }

    #[test]
    fn hyphenate_yo_no_break() {
        // yö is too short to hyphenate with min_fragment=2
        assert_eq!(hyph("yö"), "yö");
    }

    #[test]
    fn hyphenate_aamu() {
        assert_eq!(hyph("aamu"), "aa-mu");
    }

    #[test]
    fn hyphenate_opettaja() {
        // o-pet-ta-ja: but 'o' alone is < min_fragment(2), so first break is suppressed
        assert_eq!(hyph("opettaja"), "opet-ta-ja");
    }

    #[test]
    fn hyphenate_istua() {
        // is-tu-a: 'a' alone at end is < min_fragment(2), so last break is suppressed
        assert_eq!(hyph("istua"), "is-tua");
    }

    #[test]
    fn hyphenate_helsinki() {
        assert_eq!(hyph("helsinki"), "hel-sin-ki");
    }

    #[test]
    fn hyphenate_ranska() {
        assert_eq!(hyph("ranska"), "rans-ka");
    }

    #[test]
    fn hyphenate_empty() {
        assert_eq!(hyph(""), "");
    }

    #[test]
    fn hyphenate_single_char() {
        assert_eq!(hyph("a"), "a");
    }

    #[test]
    fn hyphenate_two_chars() {
        // Too short for any hyphenation
        assert_eq!(hyph("ei"), "ei");
    }

    #[test]
    fn hyphenate_three_chars() {
        // "ovi" -> syllables o-vi, but 'o' is single char -> no break
        assert_eq!(hyph("ovi"), "ovi");
    }

    // -----------------------------------------------------------------------
    // Ugly mode (min_fragment = 1)
    // -----------------------------------------------------------------------

    #[test]
    fn hyphenate_ugly_opettaja() {
        let h = FinnishHyphenator::with_min_fragment(1);
        assert_eq!(h.hyphenate_word("opettaja"), "o-pet-ta-ja");
    }

    #[test]
    fn hyphenate_ugly_istua() {
        let h = FinnishHyphenator::with_min_fragment(1);
        assert_eq!(h.hyphenate_word("istua"), "is-tu-a");
    }

    // -----------------------------------------------------------------------
    // Offset-based API
    // -----------------------------------------------------------------------

    #[test]
    fn hyphenate_offsets_koulu() {
        let h = FinnishHyphenator::new();
        assert_eq!(h.hyphenate("koulu"), vec![3]);
    }

    #[test]
    fn hyphenate_offsets_suomalainen() {
        let h = FinnishHyphenator::new();
        assert_eq!(h.hyphenate("suomalainen"), vec![3, 5, 8]);
    }

    // -----------------------------------------------------------------------
    // Case insensitivity
    // -----------------------------------------------------------------------

    #[test]
    fn hyphenate_uppercase() {
        assert_eq!(hyph("Helsinki"), "Hel-sin-ki");
    }

    #[test]
    fn hyphenate_allcaps() {
        assert_eq!(hyph("KOULU"), "KOU-LU");
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn hyphenate_all_vowels() {
        // "aeiou" -> a-ei-ou: "ei" is a diphthong, "ou" is a diphthong
        // Without morphological context, the syllabifier keeps diphthongs intact.
        let h = FinnishHyphenator::with_min_fragment(1);
        assert_eq!(h.hyphenate_word("aeiou"), "a-ei-ou");
    }

    #[test]
    fn hyphenate_all_consonants() {
        // No vowels -> no syllable split
        assert_eq!(hyph("krt"), "krt");
    }

    #[test]
    fn hyphenate_luetaan() {
        // lu-e-taan: hiatus at u-e, long vowel aa stays
        // min_fragment=2: 'e' alone is filtered out in the break list
        // lu | e | taan -> break before 'e' (pos 2, chars_before=2, chars_after=5 OK)
        //                  break before 't' (pos 3, chars_before=3, chars_after=4 OK)
        assert_eq!(hyph("luetaan"), "lu-e-taan");
    }

    #[test]
    fn hyphenate_asia() {
        // a-si-a: single-char start and end filtered with min_fragment=2
        // syllables: a | si | a
        // break at 1: before=1 < 2 -> skip
        // break at 3: after=1 < 2 -> skip
        assert_eq!(hyph("asia"), "asia");
    }

    #[test]
    fn syllabify_preserves_original_case() {
        let h = FinnishHyphenator::new();
        assert_eq!(h.syllabify("Helsinki"), vec!["Hel", "sin", "ki"]);
    }

    // -----------------------------------------------------------------------
    // Foreign onset clusters
    // -----------------------------------------------------------------------

    #[test]
    fn syllabify_strategia() {
        // stra-te-gi-a
        assert_eq!(syls("strategia"), vec!["stra", "te", "gi", "a"]);
    }

    #[test]
    fn hyphenate_prinsessa() {
        // prin-ses-sa
        assert_eq!(hyph("prinsessa"), "prin-ses-sa");
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    #[test]
    fn diphthong_detection() {
        assert!(is_diphthong('a', 'i'));
        assert!(is_diphthong('u', 'o'));
        assert!(is_diphthong('y', '\u{00F6}')); // yö
        assert!(!is_diphthong('a', 'e')); // hiatus
        assert!(!is_diphthong('o', 'a')); // hiatus
    }

    #[test]
    fn onset_detection() {
        assert!(is_foreign_onset(&['p', 'r']));
        assert!(is_foreign_onset(&['s', 't', 'r']));
        assert!(!is_foreign_onset(&['r', 'k']));
        assert!(!is_foreign_onset(&['t', 'k']));
    }

    #[test]
    fn find_nuclei_basic() {
        let word: Vec<char> = "koulu".chars().collect();
        let is_v: Vec<bool> = word.iter().map(|&c| is_vowel(c)).collect();
        let nuclei = find_nuclei(&word, &is_v);
        // k-ou-l-u -> nuclei: (1,3) for "ou", (4,5) for "u"
        assert_eq!(nuclei, vec![(1, 3), (4, 5)]);
    }

    #[test]
    fn find_nuclei_long_vowel() {
        let word: Vec<char> = "aamu".chars().collect();
        let is_v: Vec<bool> = word.iter().map(|&c| is_vowel(c)).collect();
        let nuclei = find_nuclei(&word, &is_v);
        // aa-m-u -> nuclei: (0,2) for "aa", (3,4) for "u"
        assert_eq!(nuclei, vec![(0, 2), (3, 4)]);
    }

    #[test]
    fn find_nuclei_hiatus() {
        let word: Vec<char> = "aero".chars().collect();
        let is_v: Vec<bool> = word.iter().map(|&c| is_vowel(c)).collect();
        let nuclei = find_nuclei(&word, &is_v);
        // a-e-r-o -> nuclei: (0,1), (1,2), (3,4)
        assert_eq!(nuclei, vec![(0, 1), (1, 2), (3, 4)]);
    }
}
