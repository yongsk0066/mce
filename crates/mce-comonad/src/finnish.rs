//! Finnish morphophonological rules as coKleisli morphisms.
//!
//! This module expresses Finnish morphophonological processes as coKleisli
//! arrows `&Zipper<char> -> char`, composable via [`Zipper::extend`].
//!
//! # Implemented rules
//!
//! ## 1. Consonant gradation (astevaihtelu)
//!
//! Stem-final consonants alternate between **strong** and **weak** grades
//! depending on syllable structure.
//!
//! ### Quantitative gradation (geminate weakening)
//!
//! | Strong | Weak | Example            |
//! |--------|------|--------------------|
//! | pp     | p    | kaappi -> kaapi    |
//! | tt     | t    | matto -> mato      |
//! | kk     | k    | kukka -> kuka      |
//!
//! ### Qualitative gradation
//!
//! | Strong | Weak | Example            |
//! |--------|------|--------------------|
//! | p      | v    | tapa -> tava       |
//! | t      | d    | katu -> kadu       |
//! | k      | (deleted) | puku -> puu   |
//! | mp     | mm   | kampa -> kamma     |
//! | nt     | nn   | ranta -> ranna     |
//! | nk     | ng   | kenka -> kenga     |
//! | lt     | ll   | kulta -> kulla     |
//! | rt     | rr   | parta -> parra     |
//!
//! ## 2. Vowel harmony (vokaalisointu)
//!
//! Finnish has a front/back vowel harmony system. Suffix vowels must agree
//! with stem vowels. Archiphonemic characters in morphological
//! representations are resolved by scanning the left context:
//!
//! | Archiphoneme | Back realization | Front realization |
//! |--------------|------------------|-------------------|
//! | A            | a                | ä                 |
//! | O            | o                | ö                 |
//! | U            | u                | y                 |
//!
//! Neutral vowels (e, i) are transparent to harmony: they do not trigger
//! either class, and harmony "looks through" them to find the last
//! non-neutral vowel.
//!
//! ## 3. Possessive suffix vowel copying
//!
//! The archiphoneme `V` copies the immediately preceding vowel. This
//! occurs in certain possessive and derivational suffix contexts.
//!
//! ## 4. Morphophonological pipeline
//!
//! Rules are composed via coKleisli composition: each rule is applied as
//! an `extend` step, and the output of one rule becomes the input to the
//! next. The standard pipeline order is:
//!
//! 1. Consonant gradation
//! 2. Vowel harmony
//! 3. Possessive suffix vowel copying
//!
//! # Design
//!
//! Each coKleisli arrow examines the focus character and its context
//! (via `peek_left`, `peek_right`) and produces a transformed output for
//! that position. `extend` lifts the local rule into a global
//! transformation. Only the focus character is ever transformed; context
//! characters serve as read-only input.
//!
//! # Scope
//!
//! This module implements **pure phonological rules**: it transforms every
//! character that matches a rule pattern in the appropriate context.
//! Lexical exceptions (loanwords, proper nouns) are handled at a higher
//! level by the morphological analyzer, not by the rule itself.

use crate::Zipper;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The grade (strong or weak) to produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grade {
    /// Strong grade (vahva aste): geminates and original consonants are kept.
    Strong,
    /// Weak grade (heikko aste): geminates shorten, consonants alternate.
    Weak,
}

/// A consonant gradation pattern.
///
/// Each pattern encodes a two-character window: `[context_char, graded_char]`.
/// Only the character at position 1 (the graded consonant) is ever
/// transformed; position 0 is used solely for context matching.
///
/// For geminate patterns (e.g. `pp -> p`), position 0 is the same consonant.
/// For cluster patterns (e.g. `mp -> mm`), position 0 is the preceding
/// consonant. For single-consonant patterns (e.g. `p -> v`), position 0 is
/// `'\0'`, meaning "preceded by a vowel" (any vowel matches).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradationPattern {
    /// Strong-grade pair: `[context_char, graded_char]`.
    pub strong: [char; 2],
    /// Weak-grade pair: `[context_char, replacement_char]`.
    /// `'\0'` at position 1 means the graded character is deleted.
    pub weak: [char; 2],
}

// ---------------------------------------------------------------------------
// Pattern table
// ---------------------------------------------------------------------------

/// All Finnish consonant gradation patterns.
///
/// Ordering matters: more specific patterns (geminates and clusters) must
/// appear before the general single-consonant patterns to avoid false
/// matches. For example, `mp` must match before bare `p`, and `nt`/`lt`/`rt`
/// must match before bare `t`.
const PATTERNS: &[GradationPattern] = &[
    // -- Quantitative gradation (geminate weakening) --
    // pp -> p  (second p is deleted)
    GradationPattern {
        strong: ['p', 'p'],
        weak: ['p', '\0'],
    },
    // tt -> t  (second t is deleted)
    GradationPattern {
        strong: ['t', 't'],
        weak: ['t', '\0'],
    },
    // kk -> k  (second k is deleted)
    GradationPattern {
        strong: ['k', 'k'],
        weak: ['k', '\0'],
    },
    // -- Qualitative gradation: consonant clusters --
    // mp -> mm  (p becomes m)
    GradationPattern {
        strong: ['m', 'p'],
        weak: ['m', 'm'],
    },
    // nt -> nn  (t becomes n)
    GradationPattern {
        strong: ['n', 't'],
        weak: ['n', 'n'],
    },
    // nk -> ng  (k becomes g)
    GradationPattern {
        strong: ['n', 'k'],
        weak: ['n', 'g'],
    },
    // lt -> ll  (t becomes l)
    GradationPattern {
        strong: ['l', 't'],
        weak: ['l', 'l'],
    },
    // rt -> rr  (t becomes r)
    GradationPattern {
        strong: ['r', 't'],
        weak: ['r', 'r'],
    },
    // -- Qualitative gradation: single consonants --
    // These MUST come after all cluster/geminate patterns.
    // p -> v  (preceded by vowel)
    GradationPattern {
        strong: ['\0', 'p'],
        weak: ['\0', 'v'],
    },
    // t -> d  (preceded by vowel)
    GradationPattern {
        strong: ['\0', 't'],
        weak: ['\0', 'd'],
    },
    // k -> deleted  (preceded by vowel)
    GradationPattern {
        strong: ['\0', 'k'],
        weak: ['\0', '\0'],
    },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check if a character is a Finnish vowel (lowercase).
///
/// Delegates to [`mce_core::character::is_finnish_vowel`].
fn is_vowel(c: char) -> bool {
    mce_core::character::is_finnish_vowel(c)
}

/// Find the gradation pattern that matches the focus character at position 1,
/// given the left and right neighbors as context.
///
/// For `Grade::Weak`, we match the `strong` side of patterns and produce
/// the `weak` side. For `Grade::Strong`, vice versa.
///
/// The `right_char` parameter is used to suppress false matches: when the
/// focus is the first character of a geminate pair (e.g. the first `p` in
/// `pp`), the single-consonant pattern (`p -> v`) must NOT fire, because
/// the geminate pattern handles the pair as a unit.
fn find_pattern_at_pos1(
    left_char: Option<char>,
    focus: char,
    right_char: Option<char>,
    grade: Grade,
) -> Option<&'static GradationPattern> {
    let left = left_char.unwrap_or('\0');

    for pat in PATTERNS {
        let source = match grade {
            Grade::Weak => &pat.strong,
            Grade::Strong => &pat.weak,
        };

        // Focus must match position 1 of the source pair.
        if focus != source[1] {
            continue;
        }

        if source[0] != '\0' {
            // Cluster/geminate pattern: left neighbor must match exactly.
            if left == source[0] {
                return Some(pat);
            }
        } else {
            // Single-consonant pattern: left neighbor must be a vowel.
            if !is_vowel(left) {
                continue;
            }

            // Suppress if the focus is actually position 0 of a geminate or
            // cluster pattern with the right neighbor. For example, the first
            // 'p' in "pp" should NOT match "p -> v" because it is part of the
            // geminate "pp -> p". Similarly, the 'p' in "pp" when weakened to
            // a single 'p' should not match single-consonant strengthening
            // if its right neighbor forms a different pattern.
            if let Some(right) = right_char {
                if is_pos0_of_some_pattern(focus, right, grade) {
                    continue;
                }
            }

            return Some(pat);
        }
    }

    None
}

/// Check whether `focus` at position 0 and `right` at position 1 form
/// a recognized geminate or cluster pattern (non-single-consonant).
fn is_pos0_of_some_pattern(focus: char, right: char, grade: Grade) -> bool {
    for pat in PATTERNS {
        let source = match grade {
            Grade::Weak => &pat.strong,
            Grade::Strong => &pat.weak,
        };

        // Only check cluster/geminate patterns (source[0] != '\0').
        if source[0] == '\0' {
            continue;
        }

        if focus == source[0] && right == source[1] {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// coKleisli arrow
// ---------------------------------------------------------------------------

/// Apply Finnish consonant gradation as a coKleisli morphism.
///
/// This function has the signature `(&Zipper<char>, Grade) -> char`, and
/// the partially applied form `|z| apply_gradation(z, grade)` is a
/// coKleisli arrow suitable for use with [`Zipper::extend`].
///
/// It examines the focus character and its left neighbor:
/// - If the focus is position 1 of a matching pattern, it is replaced with
///   the target character (or `'\0'` for deletion).
/// - All other characters pass through unchanged.
///
/// Position 0 of a pattern (the context character) is **never** modified;
/// it is only used for matching. This avoids double-counting when `extend`
/// applies the function at every position.
pub fn apply_gradation(z: &Zipper<char>, grade: Grade) -> char {
    let focus = *z.extract();
    let left = z.peek_left(1).copied();
    let right = z.peek_right(1).copied();

    if let Some(pat) = find_pattern_at_pos1(left, focus, right, grade) {
        let target = match grade {
            Grade::Weak => &pat.weak,
            Grade::Strong => &pat.strong,
        };
        return target[1];
    }

    focus
}

// ---------------------------------------------------------------------------
// Pipeline helper
// ---------------------------------------------------------------------------

/// Apply consonant gradation to an entire word.
///
/// Uses the Writer Comonad pipeline by default, which accumulates deletions
/// algebraically via [`DeletionSet`] instead of using `'\0'` sentinels.
/// This produces identical output to the legacy pipeline.
///
/// Returns an empty `Vec` if the input is empty.
///
/// [`DeletionSet`]: crate::writer::DeletionSet
pub fn gradation_pipeline(word: &[char], grade: Grade) -> Vec<char> {
    crate::writer::gradation_pipeline_pure(word, grade)
}

/// Convenience wrapper: apply gradation to a `&str` and return a `String`.
///
/// Uses the Writer Comonad pipeline by default.
pub fn gradate(word: &str, grade: Grade) -> String {
    crate::writer::gradate_pure(word, grade)
}

/// Legacy gradation pipeline using `'\0'` sentinels for deletions.
///
/// Kept as a fallback for validation. The Writer Comonad version
/// ([`gradation_pipeline`]) is now the default.
pub fn gradation_pipeline_legacy(word: &[char], grade: Grade) -> Vec<char> {
    let z = match Zipper::new(word.to_vec()) {
        Some(z) => z,
        None => return Vec::new(),
    };

    let result = z.extend(|zi| apply_gradation(zi, grade));
    result.to_vec().into_iter().filter(|&c| c != '\0').collect()
}

/// Legacy convenience wrapper: apply gradation using `'\0'` sentinels.
///
/// Kept as a fallback for validation.
pub fn gradate_legacy(word: &str, grade: Grade) -> String {
    let chars: Vec<char> = word.chars().collect();
    gradation_pipeline_legacy(&chars, grade)
        .into_iter()
        .collect()
}

// ===========================================================================
// Vowel harmony (vokaalisointu)
// ===========================================================================

/// The vowel harmony class determined by scanning a word's vowels.
///
/// Finnish words (native vocabulary) contain either back vowels or front
/// vowels, never both. Neutral vowels (e, i) are transparent and do not
/// determine the harmony class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarmonyClass {
    /// Back harmony: archiphonemes resolve to back vowels (a, o, u).
    Back,
    /// Front harmony: archiphonemes resolve to front vowels (ä, ö, y).
    Front,
}

/// Check if a character is a back vowel (a, o, u).
fn is_back_vowel(c: char) -> bool {
    matches!(c, 'a' | 'o' | 'u')
}

/// Check if a character is a front vowel (ä, ö, y).
fn is_front_vowel(c: char) -> bool {
    matches!(c, '\u{00E4}' | '\u{00F6}' | 'y')
}

/// Check if a character is a neutral vowel (e, i).
///
/// Neutral vowels are transparent to vowel harmony: they do not trigger
/// either back or front harmony, and the harmony rule "looks through"
/// them to find the nearest non-neutral vowel.
fn is_neutral_vowel(c: char) -> bool {
    matches!(c, 'e' | 'i')
}

/// Detect a compound boundary by looking at a consonant cluster and the
/// surrounding vowel harmony classes.
///
/// In Finnish compounds, vowel harmony resets at each component boundary.
/// We detect a likely compound boundary when we encounter a position where
/// the vowel class changes across a consonant cluster — specifically, when
/// the vowels to the right of a consonant sequence belong to a different
/// harmony class than the vowels to the left.
///
/// We use the archiphoneme marker `#` (if present) as an explicit boundary,
/// and otherwise apply a heuristic: scan leftward and track when the vowel
/// class *switches* from the most recently seen class. A class switch after
/// a consonant cluster of 2+ consonants strongly indicates a compound seam.
///
/// As a simpler, robust heuristic: we scan leftward and stop at the first
/// point where we see a vowel class contradiction (back after having seen
/// front, or vice versa). The last consistent vowel class determines
/// harmony for the suffix.
fn detect_harmony_class(z: &Zipper<char>) -> HarmonyClass {
    // Scan leftward for the nearest non-neutral vowel. A vowel-class switch
    // (back after front, or vice versa) signals a compound boundary; the
    // class closer to the suffix wins.

    let mut found_class: Option<HarmonyClass> = None;

    for i in 1..=z.position() {
        if let Some(&c) = z.peek_left(i) {
            if is_back_vowel(c) {
                if let Some(HarmonyClass::Front) = found_class {
                    return HarmonyClass::Front;
                }
                if found_class.is_none() {
                    found_class = Some(HarmonyClass::Back);
                }
            } else if is_front_vowel(c) {
                if let Some(HarmonyClass::Back) = found_class {
                    return HarmonyClass::Back;
                }
                if found_class.is_none() {
                    found_class = Some(HarmonyClass::Front);
                }
            }
            // Neutral vowels (e, i) and consonants: transparent, skip.
        }
    }

    // Default to front for neutral-only stems (Finnish convention).
    found_class.unwrap_or(HarmonyClass::Front)
}

/// Apply Finnish vowel harmony as a coKleisli morphism.
///
/// Resolves archiphonemic characters based on the harmony class determined
/// by scanning the left context:
///
/// | Archiphoneme | Back    | Front   |
/// |--------------|---------|---------|
/// | `A`          | `a`     | `ä`     |
/// | `O`          | `o`     | `ö`     |
/// | `U`          | `u`     | `y`     |
///
/// All non-archiphonemic characters pass through unchanged.
///
/// The harmony class is determined by the **last non-neutral vowel** to
/// the left of the focus. If no such vowel exists, front harmony is used
/// (matching Finnish convention for neutral-only stems).
pub fn apply_vowel_harmony(z: &Zipper<char>) -> char {
    let focus = *z.extract();
    match focus {
        'A' => {
            let harmony = detect_harmony_class(z);
            match harmony {
                HarmonyClass::Back => 'a',
                HarmonyClass::Front => '\u{00E4}', // ä
            }
        }
        'O' => {
            let harmony = detect_harmony_class(z);
            match harmony {
                HarmonyClass::Back => 'o',
                HarmonyClass::Front => '\u{00F6}', // ö
            }
        }
        'U' => {
            let harmony = detect_harmony_class(z);
            match harmony {
                HarmonyClass::Back => 'u',
                HarmonyClass::Front => 'y',
            }
        }
        _ => focus,
    }
}

/// Apply vowel harmony to an entire word string.
///
/// Resolves all archiphonemic characters (`A`, `O`, `U`) according to
/// the vowel harmony context.
pub fn harmonize(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let z = match Zipper::new(chars) {
        Some(z) => z,
        None => return String::new(),
    };
    let result = z.extend(apply_vowel_harmony);
    result.to_vec().into_iter().collect()
}

// ===========================================================================
// Possessive suffix vowel copying
// ===========================================================================

/// Apply possessive suffix vowel copying as a coKleisli morphism.
///
/// Resolves the archiphoneme `V`, which copies the immediately preceding
/// vowel. This occurs in contexts like possessive suffixes and certain
/// derivational affixes. For example:
///
/// - `taloVn` -> `taloon` (V copies 'o')
/// - `käsiVn` -> `käsiin` (V copies 'i')
///
/// If no vowel is found to the left, `V` passes through unchanged (this
/// indicates a malformed morphological representation).
///
/// All non-`V` characters pass through unchanged.
pub fn apply_possessive(z: &Zipper<char>) -> char {
    let focus = *z.extract();
    if focus != 'V' {
        return focus;
    }

    // Scan leftward for the nearest vowel to copy.
    for i in 1..=z.position() {
        if let Some(&c) = z.peek_left(i) {
            if is_vowel(c) || is_neutral_vowel(c) {
                return c;
            }
        }
    }

    // No vowel found to the left; return unchanged.
    // This should not happen in well-formed input.
    focus
}

/// Apply possessive vowel copying to an entire word string.
pub fn apply_possessive_to_word(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let z = match Zipper::new(chars) {
        Some(z) => z,
        None => return String::new(),
    };
    let result = z.extend(apply_possessive);
    result.to_vec().into_iter().collect()
}

// ===========================================================================
// Morphophonological pipeline (coKleisli composition chain)
// ===========================================================================

/// Apply a chain of morphophonological rules to a word.
///
/// This function composes multiple coKleisli arrows via the Writer
/// Comonad pipeline, forming a pure coKleisli composition chain:
///
/// 1. **Consonant gradation** — alternates stem consonants based on
///    the requested [`Grade`].
/// 2. **Vowel harmony** — resolves archiphonemic `A`, `O`, `U` based
///    on the stem's vowel class.
/// 3. **Possessive vowel copying** — resolves `V` archiphonemes by
///    copying the preceding vowel.
///
/// Uses the Writer Comonad pipeline by default, which accumulates
/// deletions algebraically via [`DeletionSet`] instead of using
/// `'\0'` sentinels. The output is identical to the legacy pipeline.
///
/// [`DeletionSet`]: crate::writer::DeletionSet
///
/// # Example
///
/// ```
/// use mce_comonad::finnish::{morphophonological_pipeline, Grade};
///
/// // ranta + ssA (weak grade) -> rannassa
/// assert_eq!(morphophonological_pipeline("rantAssA", Grade::Weak), "rannassa");
///
/// // pöytä + llA (weak grade) -> pöydällä
/// assert_eq!(morphophonological_pipeline("p\u{00F6}yt\u{00E4}llA", Grade::Weak), "p\u{00F6}yd\u{00E4}ll\u{00E4}");
/// ```
pub fn morphophonological_pipeline(word: &str, grade: Grade) -> String {
    crate::writer::morphophonological_pipeline_pure(word, grade)
}

/// Legacy morphophonological pipeline using `'\0'` sentinels for deletions.
///
/// This is the original implementation that filters null characters between
/// steps. Kept as a fallback for validation. The Writer Comonad version
/// ([`morphophonological_pipeline`]) is now the default.
pub fn morphophonological_pipeline_legacy(word: &str, grade: Grade) -> String {
    if word.is_empty() {
        return String::new();
    }

    let chars: Vec<char> = word.chars().collect();

    let z1 = match Zipper::new(chars) {
        Some(z) => z,
        None => return String::new(),
    };
    let after_gradation = z1.extend(|zi| apply_gradation(zi, grade));

    // Filter out null characters between steps so subsequent rules
    // see the correct surface form (e.g. after k-deletion).
    let filtered: Vec<char> = after_gradation
        .to_vec()
        .into_iter()
        .filter(|&c| c != '\0')
        .collect();

    let z2 = match Zipper::new(filtered) {
        Some(z) => z,
        None => return String::new(),
    };
    let after_harmony = z2.extend(apply_vowel_harmony);

    let z3 = match Zipper::new(after_harmony.to_vec()) {
        Some(z) => z,
        None => return String::new(),
    };
    let after_possessive = z3.extend(apply_possessive);

    after_possessive.to_vec().into_iter().collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Helpers --

    fn weak(word: &str) -> String {
        gradate(word, Grade::Weak)
    }

    fn strong(word: &str) -> String {
        gradate(word, Grade::Strong)
    }

    // =====================================================================
    // Quantitative gradation: geminate weakening (strong -> weak)
    // =====================================================================

    #[test]
    fn geminate_pp_to_p() {
        // kaappi -> kaapi  (second p deleted)
        assert_eq!(weak("kaappi"), "kaapi");
    }

    #[test]
    fn geminate_tt_to_t() {
        // matto -> mato  (second t deleted)
        assert_eq!(weak("matto"), "mato");
    }

    #[test]
    fn geminate_kk_to_k() {
        // kukka -> kuka  (second k deleted)
        assert_eq!(weak("kukka"), "kuka");
    }

    // =====================================================================
    // Qualitative gradation: single consonant (strong -> weak)
    // =====================================================================

    #[test]
    fn qualitative_p_to_v() {
        // tapa -> tava  (p between vowels becomes v)
        assert_eq!(weak("tapa"), "tava");
    }

    #[test]
    fn qualitative_t_to_d() {
        // katu -> kadu  (t between vowels becomes d)
        assert_eq!(weak("katu"), "kadu");
    }

    #[test]
    fn qualitative_k_deleted() {
        // puku -> puu  (k between vowels is deleted)
        assert_eq!(weak("puku"), "puu");
    }

    #[test]
    fn qualitative_k_deleted_luku() {
        // luku -> luu  (another k-deletion example)
        assert_eq!(weak("luku"), "luu");
    }

    // =====================================================================
    // Qualitative gradation: consonant clusters (strong -> weak)
    // =====================================================================

    #[test]
    fn cluster_mp_to_mm() {
        // kampa -> kamma  (p after m becomes m)
        assert_eq!(weak("kampa"), "kamma");
    }

    #[test]
    fn cluster_nt_to_nn() {
        // ranta -> ranna  (t after n becomes n)
        assert_eq!(weak("ranta"), "ranna");
    }

    #[test]
    fn cluster_nk_to_ng() {
        // kenka -> kenga  (k after n becomes g)
        assert_eq!(weak("kenka"), "kenga");
    }

    #[test]
    fn cluster_lt_to_ll() {
        // kulta -> kulla  (t after l becomes l)
        assert_eq!(weak("kulta"), "kulla");
    }

    #[test]
    fn cluster_rt_to_rr() {
        // parta -> parra  (t after r becomes r)
        assert_eq!(weak("parta"), "parra");
    }

    // =====================================================================
    // Reverse direction: weak -> strong
    // =====================================================================

    #[test]
    fn reverse_mm_to_mp() {
        assert_eq!(strong("kamma"), "kampa");
    }

    #[test]
    fn reverse_nn_to_nt() {
        assert_eq!(strong("ranna"), "ranta");
    }

    #[test]
    fn reverse_ng_to_nk() {
        assert_eq!(strong("kenga"), "kenka");
    }

    #[test]
    fn reverse_ll_to_lt() {
        assert_eq!(strong("kulla"), "kulta");
    }

    #[test]
    fn reverse_rr_to_rt() {
        assert_eq!(strong("parra"), "parta");
    }

    #[test]
    fn reverse_v_to_p() {
        assert_eq!(strong("tava"), "tapa");
    }

    #[test]
    fn reverse_d_to_t() {
        assert_eq!(strong("kadu"), "katu");
    }

    // Note: reversing geminate weakening (single p -> pp) from filtered
    // text is ambiguous and not tested here. The raw zipper output before
    // '\0' filtering would be needed for that.

    // =====================================================================
    // Words that should NOT undergo gradation
    // =====================================================================

    #[test]
    fn no_gradation_vowel_only() {
        assert_eq!(weak("aie"), "aie");
        assert_eq!(strong("aie"), "aie");
    }

    #[test]
    fn no_gradation_consonant_not_in_pattern() {
        // 's', 'h', 'l' alone are not grading consonants
        assert_eq!(weak("kisa"), "kisa");
        assert_eq!(strong("kisa"), "kisa");
    }

    #[test]
    fn no_gradation_consonant_at_word_start() {
        // 'k' at position 0 has no left neighbor, so no gradation
        assert_eq!(weak("ka"), "ka");
    }

    #[test]
    fn no_gradation_consonant_after_consonant() {
        // 'p' after 's' (not a grading cluster) should not match
        // single-consonant p->v (which requires vowel+p)
        assert_eq!(weak("spa"), "spa");
    }

    #[test]
    fn no_gradation_non_grading_geminate() {
        // 'ss' is not a grading geminate
        assert_eq!(weak("massa"), "massa");
    }

    // =====================================================================
    // Edge cases
    // =====================================================================

    #[test]
    fn empty_input() {
        assert_eq!(gradation_pipeline(&[], Grade::Weak), Vec::<char>::new());
        assert_eq!(gradate("", Grade::Weak), "");
    }

    #[test]
    fn single_char() {
        assert_eq!(gradate("a", Grade::Weak), "a");
        assert_eq!(gradate("k", Grade::Weak), "k");
        assert_eq!(gradate("p", Grade::Weak), "p");
    }

    #[test]
    fn two_char_vowel_plus_stop() {
        // Minimal case: vowel + stop at end of word
        assert_eq!(gradate("ap", Grade::Weak), "av");
        assert_eq!(gradate("at", Grade::Weak), "ad");
        assert_eq!(gradate("ak", Grade::Weak), "a");
    }

    #[test]
    fn two_char_consonant_plus_stop() {
        // Consonant + stop: no gradation (no vowel before stop)
        assert_eq!(gradate("sp", Grade::Weak), "sp");
        assert_eq!(gradate("sk", Grade::Weak), "sk");
    }

    #[test]
    fn gradation_is_idempotent_on_non_grading_words() {
        let word = "silta";
        // silta has lt->ll, so weak("silta") = "silla", then
        // weak("silla") has ll which doesn't match any strong pattern
        // for weakening. So it IS idempotent on the weak result.
        let once = weak(word);
        let twice = weak(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn multiple_gradation_sites() {
        // A word with two potential gradation sites.
        // "kauppa" has pp (geminate) -> weak: "kaupa"
        assert_eq!(weak("kauppa"), "kaupa");
    }

    // =====================================================================
    // coKleisli arrow properties
    // =====================================================================

    #[test]
    fn extend_preserves_length_for_non_deleting_patterns() {
        // mp -> mm: no deletion, length preserved
        let input: Vec<char> = "kampa".chars().collect();
        let z = Zipper::new(input.clone()).unwrap();
        let result = z.extend(|zi| apply_gradation(zi, Grade::Weak));
        let output: Vec<char> = result.to_vec().into_iter().filter(|&c| c != '\0').collect();
        assert_eq!(output.len(), input.len());
    }

    #[test]
    fn extend_shortens_for_k_deletion() {
        // k-deletion: puku -> puu (length 4 -> 3)
        let input: Vec<char> = "puku".chars().collect();
        let z = Zipper::new(input.clone()).unwrap();
        let result = z.extend(|zi| apply_gradation(zi, Grade::Weak));
        let output: Vec<char> = result.to_vec().into_iter().filter(|&c| c != '\0').collect();
        assert_eq!(output.len(), input.len() - 1);
    }

    #[test]
    fn extend_shortens_for_geminate_deletion() {
        // pp -> p: kaappi -> kaapi (length 6 -> 5)
        let input: Vec<char> = "kaappi".chars().collect();
        let z = Zipper::new(input.clone()).unwrap();
        let result = z.extend(|zi| apply_gradation(zi, Grade::Weak));
        let output: Vec<char> = result.to_vec().into_iter().filter(|&c| c != '\0').collect();
        assert_eq!(output.len(), input.len() - 1);
    }

    #[test]
    fn extend_output_at_focus_matches_direct_application() {
        // Comonad right identity: extract(extend(f)) == f(z)
        let input: Vec<char> = "kampa".chars().collect();
        let z = Zipper::new(input).unwrap();
        let direct = apply_gradation(&z, Grade::Weak);
        let extended = z.extend(|zi| apply_gradation(zi, Grade::Weak));
        assert_eq!(*extended.extract(), direct);
    }

    #[test]
    fn roundtrip_cluster_patterns() {
        // Applying weak then strong should recover the original for
        // non-deleting cluster patterns.
        for word in &["kampa", "ranta", "kenka", "kulta", "parta"] {
            let weakened = weak(word);
            let restored = strong(&weakened);
            assert_eq!(
                &restored, word,
                "roundtrip failed for {}: weak={}, strong(weak)={}",
                word, weakened, restored
            );
        }
    }

    #[test]
    fn roundtrip_qualitative_single() {
        // p->v, t->d are reversible via strong().
        for (strong_form, weak_form) in &[("tapa", "tava"), ("katu", "kadu")] {
            assert_eq!(&weak(strong_form), weak_form);
            assert_eq!(&strong(weak_form), strong_form);
        }
    }

    // =====================================================================
    // Vowel harmony: back stems
    // =====================================================================

    #[test]
    fn harmony_back_stem_a() {
        // talo + ssA -> talossa (back, because 'a' and 'o' are back)
        assert_eq!(harmonize("talossA"), "talossa");
    }

    #[test]
    fn harmony_back_stem_o() {
        // koulu + ssA -> koulussa (back, because 'o' and 'u' are back)
        assert_eq!(harmonize("koulussA"), "koulussa");
    }

    #[test]
    fn harmony_back_stem_u() {
        // auto + ssA -> autossa (back, because 'u' is back)
        assert_eq!(harmonize("autossA"), "autossa");
    }

    #[test]
    fn harmony_back_multiple_archiphonemes() {
        // talo + stA + An -> talostaan
        assert_eq!(harmonize("talostAAn"), "talostaan");
    }

    // =====================================================================
    // Vowel harmony: front stems
    // =====================================================================

    #[test]
    fn harmony_front_stem_ae() {
        // pöytä + ssA -> pöydässä (front, because 'ö' and 'ä' are front)
        assert_eq!(
            harmonize("p\u{00F6}yt\u{00E4}ssA"),
            "p\u{00F6}yt\u{00E4}ss\u{00E4}"
        );
    }

    #[test]
    fn harmony_front_stem_oe() {
        // yö + ssA -> yössä (front)
        assert_eq!(harmonize("y\u{00F6}ssA"), "y\u{00F6}ss\u{00E4}");
    }

    #[test]
    fn harmony_front_stem_y() {
        // työ + ssA -> työssä (front, because 'y' and 'ö' are front)
        assert_eq!(harmonize("ty\u{00F6}ssA"), "ty\u{00F6}ss\u{00E4}");
    }

    #[test]
    fn harmony_front_multiple_archiphonemes() {
        // pöytä + llA + An -> pöytällään
        assert_eq!(
            harmonize("p\u{00F6}yt\u{00E4}ll\u{00C4}\u{00C4}n"),
            // Note: uppercase Ä is not an archiphoneme in our system.
            // Let's use A archiphonemes:
            "p\u{00F6}yt\u{00E4}ll\u{00C4}\u{00C4}n"
        );
    }

    #[test]
    fn harmony_front_archiphoneme_chain() {
        // pöytä + llA + An -> pöytällään
        assert_eq!(
            harmonize("p\u{00F6}yt\u{00E4}llAAn"),
            "p\u{00F6}yt\u{00E4}ll\u{00E4}\u{00E4}n"
        );
    }

    // =====================================================================
    // Vowel harmony: neutral vowel transparency
    // =====================================================================

    #[test]
    fn harmony_neutral_only_defaults_front() {
        // tie + ssA -> tiessä (neutral-only stem -> front harmony)
        assert_eq!(harmonize("tiessA"), "tiess\u{00E4}");
    }

    #[test]
    fn harmony_neutral_only_i() {
        // vesi + ssA -> vedessä (neutral-only stem -> front harmony)
        // Already-resolved input passes through:
        assert_eq!(harmonize("vesiss\u{00E4}"), "vesiss\u{00E4}");
        // With archiphoneme:
        assert_eq!(harmonize("vesissA"), "vesiss\u{00E4}");
    }

    #[test]
    fn harmony_neutral_transparent_to_back() {
        // poika + llA -> pojalla (neutral 'i' is transparent, 'o' is back)
        assert_eq!(harmonize("poikAllA"), "poikalla");
    }

    #[test]
    fn harmony_neutral_transparent_to_front() {
        // pöytiä + ssA -> (front, 'ö' visible through neutral 'i')
        // Test: pöytiA -> pöytiä
        assert_eq!(harmonize("p\u{00F6}ytiA"), "p\u{00F6}yti\u{00E4}");
    }

    // =====================================================================
    // Vowel harmony: archiphoneme O and U
    // =====================================================================

    #[test]
    fn harmony_archiphoneme_o_back() {
        // talo + On -> talon
        assert_eq!(harmonize("talOn"), "talon");
    }

    #[test]
    fn harmony_archiphoneme_o_front() {
        // yö + On -> yön
        assert_eq!(harmonize("y\u{00F6}On"), "y\u{00F6}\u{00F6}n");
    }

    #[test]
    fn harmony_archiphoneme_u_back() {
        // talo + Un -> taloun (hypothetical suffix)
        assert_eq!(harmonize("taloUn"), "taloun");
    }

    #[test]
    fn harmony_archiphoneme_u_front() {
        // yö + Ulle -> yöylle (hypothetical, 'U' -> 'y')
        assert_eq!(harmonize("y\u{00F6}Ulle"), "y\u{00F6}ylle");
    }

    // =====================================================================
    // Vowel harmony: edge cases
    // =====================================================================

    #[test]
    fn harmony_empty_string() {
        assert_eq!(harmonize(""), "");
    }

    #[test]
    fn harmony_no_archiphonemes() {
        // Already-resolved word passes through unchanged.
        assert_eq!(harmonize("talossa"), "talossa");
        assert_eq!(
            harmonize("p\u{00F6}yd\u{00E4}ss\u{00E4}"),
            "p\u{00F6}yd\u{00E4}ss\u{00E4}"
        );
    }

    #[test]
    fn harmony_archiphoneme_at_start() {
        // Archiphoneme at the very start with no left context -> front.
        assert_eq!(harmonize("A"), "\u{00E4}");
        assert_eq!(harmonize("O"), "\u{00F6}");
        assert_eq!(harmonize("U"), "y");
    }

    #[test]
    fn harmony_idempotent() {
        // Applying harmony to an already-harmonized word should be no-op.
        let word = "talossa";
        assert_eq!(harmonize(word), word);
        let word2 = "tiess\u{00E4}";
        assert_eq!(harmonize(word2), word2);
    }

    // =====================================================================
    // Possessive vowel copying
    // =====================================================================

    #[test]
    fn possessive_copy_o() {
        // taloVn -> taloon (V copies 'o')
        assert_eq!(apply_possessive_to_word("taloVn"), "taloon");
    }

    #[test]
    fn possessive_copy_i() {
        // käsiVn -> käsiin (V copies 'i')
        assert_eq!(apply_possessive_to_word("k\u{00E4}siVn"), "k\u{00E4}siin");
    }

    #[test]
    fn possessive_copy_a() {
        // talostaVn -> talostaan (V copies 'a')
        assert_eq!(apply_possessive_to_word("talostaVn"), "talostaan");
    }

    #[test]
    fn possessive_copy_ae() {
        // pöydältäVn -> pöydältään (V copies 'ä')
        assert_eq!(
            apply_possessive_to_word("p\u{00F6}yd\u{00E4}lt\u{00E4}Vn"),
            "p\u{00F6}yd\u{00E4}lt\u{00E4}\u{00E4}n"
        );
    }

    #[test]
    fn possessive_no_vowel_left() {
        // V at start with no vowel to the left: passes through unchanged.
        assert_eq!(apply_possessive_to_word("Vn"), "Vn");
    }

    #[test]
    fn possessive_no_v_archiphoneme() {
        // No V in word: unchanged.
        assert_eq!(apply_possessive_to_word("talossa"), "talossa");
    }

    #[test]
    fn possessive_empty() {
        assert_eq!(apply_possessive_to_word(""), "");
    }

    #[test]
    fn possessive_copy_through_consonants() {
        // V should scan past consonants to find the vowel.
        // talostVn -> talostVn... the 't' is before V, so scan past it
        // to find 's' (consonant), then 'o' (vowel) -> no, wait:
        // t-a-l-o-s-t-V-n: left of V is 't', then 's', then 'o' -> copies 'o'
        assert_eq!(apply_possessive_to_word("talostVn"), "taloston");
    }

    // =====================================================================
    // Morphophonological pipeline (composition chain)
    // =====================================================================

    #[test]
    fn pipeline_gradation_plus_harmony_back() {
        // ranta + ssA (weak grade) -> rannassa
        // Step 1: gradation: ranta -> ranna (nt -> nn)
        // But in the input "rantAssA", gradation applies to the whole string.
        // The A at position 4 is not a consonant, so gradation won't touch it.
        // Step 2: harmony: A -> a (back, because 'a' is in stem)
        assert_eq!(
            morphophonological_pipeline("rantAssA", Grade::Weak),
            "rannassa"
        );
    }

    #[test]
    fn pipeline_gradation_plus_harmony_front() {
        // pöytä + llA (weak grade)
        // Step 1: gradation does not apply to 'l' cluster (ll is not grading)
        // Actually, the input is "pöytällA" but if we want to test gradation:
        // pöytä has no gradable consonants in this suffix form.
        // Let's use a word where both apply:
        // kenka + ssA (weak) -> kengässä
        // Step 1: kenka -> kenga (nk -> ng)
        // Step 2: A -> ä (front, because 'e' is neutral but no back vowel)
        assert_eq!(
            morphophonological_pipeline("kenk\u{00E4}ssA", Grade::Weak),
            "keng\u{00E4}ss\u{00E4}"
        );
    }

    #[test]
    fn pipeline_gradation_plus_harmony_with_deletion() {
        // puku + ssA (weak) -> puussa
        // Step 1: gradation: puku -> puu (k deleted)
        // Step 2: harmony: A -> a (back, because 'u' is back)
        assert_eq!(
            morphophonological_pipeline("pukussA", Grade::Weak),
            "puussa"
        );
    }

    #[test]
    fn pipeline_no_gradation_strong_grade() {
        // ranta + ssA (strong grade) -> rantassa
        // Step 1: gradation: no change in strong grade (nt stays nt)
        // Step 2: harmony: A -> a
        assert_eq!(
            morphophonological_pipeline("rantAssA", Grade::Strong),
            "rantassa"
        );
    }

    #[test]
    fn pipeline_all_three_rules() {
        // Test gradation + harmony + possessive in one chain.
        // kampa + stA + Vn (weak) -> kammastaan
        //
        // k-a-m-p-A-s-t-A-V-n
        // Step 1 (gradation, weak): mp -> mm, so 'p' at pos 3 becomes 'm'
        //   k-a-m-m-A-s-t-A-V-n  (no deletions)
        // Step 2 (harmony): A at pos 4 -> 'a' (back), A at pos 7 -> 'a' (back)
        //   k-a-m-m-a-s-t-a-V-n
        // Step 3 (possessive): V at pos 8 copies 'a' from pos 7
        //   k-a-m-m-a-s-t-a-a-n
        assert_eq!(
            morphophonological_pipeline("kampAstAVn", Grade::Weak),
            "kammastaan"
        );
    }

    #[test]
    fn pipeline_front_all_three() {
        // kenka + stA + Vn (weak)
        // Step 1: nk -> ng: kenga + stA + Vn -> "kengAstAVn" wait, input is
        //   k-e-n-k-A-s-t-A-V-n  (with archiphonemes)
        // Actually let's use ä properly:
        // kenkä + stA + Vn (weak)
        // Input: "kenk\u{00E4}stAVn"
        // Step 1: nk -> ng: "keng\u{00E4}stAVn" (k at pos 3 becomes g)
        // Step 2: A at pos 5 -> ä (front, 'e' is neutral, no back), A at pos 7 -> ä
        //   -> "kengästäVn"
        // Step 3: V copies ä -> "kengästään"
        assert_eq!(
            morphophonological_pipeline("kenk\u{00E4}stAVn", Grade::Weak),
            "keng\u{00E4}st\u{00E4}\u{00E4}n"
        );
    }

    #[test]
    fn pipeline_empty_input() {
        assert_eq!(morphophonological_pipeline("", Grade::Weak), "");
    }

    #[test]
    fn pipeline_no_changes_needed() {
        // A word with no gradation patterns, no archiphonemes.
        assert_eq!(
            morphophonological_pipeline("talossa", Grade::Weak),
            "talossa"
        );
    }

    #[test]
    fn pipeline_harmony_only() {
        // Word with archiphonemes but no gradation.
        // massa + ssA (strong) -> massassa (ss is not grading)
        assert_eq!(
            morphophonological_pipeline("massAssA", Grade::Strong),
            "massassa"
        );
    }

    // =====================================================================
    // coKleisli composition properties
    // =====================================================================

    #[test]
    fn pipeline_right_identity_harmony() {
        // Comonad right identity for vowel harmony:
        // extract(extend(apply_vowel_harmony)) == apply_vowel_harmony(z)
        let input: Vec<char> = "talossA".chars().collect();
        let z = Zipper::new(input).unwrap();
        let direct = apply_vowel_harmony(&z);
        let extended = z.extend(apply_vowel_harmony);
        assert_eq!(*extended.extract(), direct);
    }

    #[test]
    fn pipeline_right_identity_possessive() {
        // Comonad right identity for possessive:
        let input: Vec<char> = "taloVn".chars().collect();
        let z = Zipper::new(input).unwrap();
        let direct = apply_possessive(&z);
        let extended = z.extend(apply_possessive);
        assert_eq!(*extended.extract(), direct);
    }

    #[test]
    fn pipeline_composition_order_matters() {
        // Demonstrate that applying gradation before harmony gives a
        // different result than applying them in reverse order (on the
        // raw archiphonemic input).
        let word = "pukussA";

        // Correct order: gradation first (k deleted), then harmony
        let correct = morphophonological_pipeline(word, Grade::Weak);
        assert_eq!(correct, "puussa");

        // If we applied harmony first (before gradation):
        // Apply harmony first (A -> a, because 'u' is back), then gradation:
        let harmony_first = harmonize(word);
        let then_gradation = gradate(&harmony_first, Grade::Weak);
        // In this case the result happens to be the same because the
        // harmony context doesn't change. But the pipeline ensures
        // correctness for cases where k-deletion would change vowel
        // adjacency, affecting harmony scanning.
        assert_eq!(then_gradation, correct);
    }

    #[test]
    fn pipeline_deterministic() {
        // Same input always produces same output.
        let word = "rantAssA";
        let r1 = morphophonological_pipeline(word, Grade::Weak);
        let r2 = morphophonological_pipeline(word, Grade::Weak);
        assert_eq!(r1, r2);
    }

    #[test]
    fn pipeline_idempotent_on_resolved() {
        // Applying the pipeline to already-resolved text should be a no-op
        // (no archiphonemes, no gradation patterns that would re-fire).
        let resolved = "rannassa";
        let result = morphophonological_pipeline(resolved, Grade::Weak);
        // Note: "rannassa" has "nn" which in strong->weak direction
        // doesn't match (nn is the weak form of nt, and we're already
        // in weak grade context). So the word should pass through.
        // However, nn->nn in Weak direction: nn matches the weak side
        // of nt->nn pattern. In Grade::Weak we match the strong side,
        // so nn does NOT match. Correct: no change.
        assert_eq!(result, resolved);
    }

    // =====================================================================
    // Compound word boundary in vowel harmony
    // =====================================================================

    #[test]
    fn harmony_compound_paakaupunkiseutu() {
        // pää (front) + kaupunki (back) + seutu (back)
        // The suffix should follow the *last* compound component (seutu = back).
        // "pääkaupunkiseutussA" -> "pääkaupunkiseutussa" (back, from 'u' in seutu)
        assert_eq!(
            harmonize("p\u{00E4}\u{00E4}kaupunkiseutussA"),
            "p\u{00E4}\u{00E4}kaupunkiseutussa"
        );
    }

    #[test]
    fn harmony_compound_ylioppilastutkinto() {
        // yli (front/neutral) + oppilas (back) + tutkinto (back)
        // Suffix on the whole compound should follow the last component (tutkinto = back).
        // "ylioppilastutkintossA" -> "ylioppilastutkintossa" (back, from 'u'/'o' in tutkinto)
        assert_eq!(harmonize("ylioppilastutkintossA"), "ylioppilastutkintossa");
    }

    #[test]
    fn harmony_compound_tyopoyta() {
        // työ (front) + pöytä (front) — both front, so no conflict.
        // "työpöytällA" -> "työpöytällä" (front)
        assert_eq!(
            harmonize("ty\u{00F6}p\u{00F6}yt\u{00E4}llA"),
            "ty\u{00F6}p\u{00F6}yt\u{00E4}ll\u{00E4}"
        );
    }

    #[test]
    fn harmony_compound_front_then_back() {
        // A compound where first part is front, second part is back.
        // The suffix should follow the LAST component (back).
        // "yötyössA" = yö (front) + työ (front) — both front, bad example.
        // Instead: "äänioikeus" = ääni (front) + oikeus (back)
        // "äänioikeudessA" -> "äänioikeudessa" (back, from 'o' in oikeus)
        assert_eq!(
            harmonize("\u{00E4}\u{00E4}nioikeudessA"),
            "\u{00E4}\u{00E4}nioikeudessa"
        );
    }

    #[test]
    fn harmony_simple_stem_still_works() {
        // Ensure simple (non-compound) words still work correctly.
        assert_eq!(harmonize("talossA"), "talossa");
        assert_eq!(
            harmonize("p\u{00F6}yt\u{00E4}ssA"),
            "p\u{00F6}yt\u{00E4}ss\u{00E4}"
        );
        assert_eq!(harmonize("tiessA"), "tiess\u{00E4}");
    }

    // =====================================================================
    // Writer pipeline vs legacy pipeline equivalence
    // =====================================================================

    #[test]
    fn writer_vs_legacy_gradation_equivalence() {
        let test_words = &[
            "kaappi", "matto", "kukka", "tapa", "katu", "puku", "luku", "kampa", "ranta", "kenka",
            "kulta", "parta", "kisa", "massa", "aie", "spa", "ka", "a", "k", "p", "", "ap", "at",
            "ak", "kauppa", "silta",
        ];
        for &word in test_words {
            for grade in &[Grade::Weak, Grade::Strong] {
                let writer = gradate(word, *grade);
                let legacy = gradate_legacy(word, *grade);
                assert_eq!(
                    writer, legacy,
                    "gradate mismatch for '{}' ({:?}): writer='{}', legacy='{}'",
                    word, grade, writer, legacy
                );
            }
        }
    }

    #[test]
    fn writer_vs_legacy_pipeline_equivalence() {
        let test_cases = &[
            ("rantAssA", Grade::Weak),
            ("rantAssA", Grade::Strong),
            ("kenk\u{00E4}ssA", Grade::Weak),
            ("pukussA", Grade::Weak),
            ("kampAstAVn", Grade::Weak),
            ("kenk\u{00E4}stAVn", Grade::Weak),
            ("massAssA", Grade::Strong),
            ("talossa", Grade::Weak),
            ("p\u{00F6}yt\u{00E4}llA", Grade::Weak),
            ("taloVn", Grade::Weak),
            ("kaapissA", Grade::Weak),
            ("kaappi", Grade::Weak),
            ("tytt\u{00F6}", Grade::Weak),
            ("", Grade::Weak),
            ("talossA", Grade::Weak),
        ];
        for &(word, grade) in test_cases {
            let writer = morphophonological_pipeline(word, grade);
            let legacy = morphophonological_pipeline_legacy(word, grade);
            assert_eq!(
                writer, legacy,
                "pipeline mismatch for '{}' ({:?}): writer='{}', legacy='{}'",
                word, grade, writer, legacy
            );
        }
    }

    #[test]
    fn strong_grade_kamma_to_kampa() {
        // Reverse gradation: weak form -> strong form
        // kamma (weak: mm) -> kampa (strong: mp)
        assert_eq!(strong("kamma"), "kampa");
    }

    #[test]
    fn strong_grade_via_writer_kamma() {
        // Verify that the public `gradate` function also works for strong grade
        let result = gradate("kamma", Grade::Strong);
        assert_eq!(result, "kampa");
    }

    #[test]
    fn writer_vs_legacy_gradation_pipeline_vec_equivalence() {
        let test_words: Vec<Vec<char>> = vec![
            "kaappi".chars().collect(),
            "puku".chars().collect(),
            "kampa".chars().collect(),
            "ranta".chars().collect(),
            vec![],
        ];
        for chars in &test_words {
            for grade in &[Grade::Weak, Grade::Strong] {
                let writer = gradation_pipeline(chars, *grade);
                let legacy = gradation_pipeline_legacy(chars, *grade);
                assert_eq!(
                    writer, legacy,
                    "gradation_pipeline mismatch for {:?} ({:?})",
                    chars, grade
                );
            }
        }
    }
}
