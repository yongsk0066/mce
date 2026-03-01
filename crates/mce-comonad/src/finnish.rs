//! Finnish consonant gradation (astevaihtelu) as coKleisli morphisms.
//!
//! Consonant gradation is a morphophonological process in Finnish where
//! stem-final consonants alternate between **strong** and **weak** grades
//! depending on syllable structure. This module expresses gradation rules
//! as coKleisli arrows `&Zipper<char> -> char`, composable via
//! [`Zipper::extend`].
//!
//! # Gradation patterns
//!
//! ## Quantitative gradation (geminate weakening)
//!
//! | Strong | Weak | Example            |
//! |--------|------|--------------------|
//! | pp     | p    | kaappi -> kaapi    |
//! | tt     | t    | matto -> mato      |
//! | kk     | k    | kukka -> kuka      |
//!
//! ## Qualitative gradation
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
//! # Design
//!
//! The main coKleisli arrow [`apply_gradation`] examines the focus character
//! and its left neighbor to determine whether a gradation pattern applies,
//! then returns the appropriately graded output character. Only the second
//! character of a two-character pattern (position 1) is ever transformed;
//! the first character (position 0) serves as context only and passes
//! through unchanged. This ensures that `extend` applies cleanly: each
//! character position produces exactly one output character (or `'\0'` for
//! deletions), with no double-counting.
//!
//! # Scope
//!
//! This module implements **pure phonological rules**: it transforms every
//! character that matches a gradation pattern in the appropriate context.
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
fn is_vowel(c: char) -> bool {
    matches!(
        c,
        'a' | 'e' | 'i' | 'o' | 'u' | 'y' | '\u{00E4}' | '\u{00F6}'
    )
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

    // Check if the focus matches position 1 of some gradation pattern.
    if let Some(pat) = find_pattern_at_pos1(left, focus, right, grade) {
        let target = match grade {
            Grade::Weak => &pat.weak,
            Grade::Strong => &pat.strong,
        };
        return target[1];
    }

    // No pattern matches; return focus unchanged.
    focus
}

// ---------------------------------------------------------------------------
// Pipeline helper
// ---------------------------------------------------------------------------

/// Apply consonant gradation to an entire word.
///
/// Creates a [`Zipper`] from the word's characters, extends it with
/// [`apply_gradation`], collects the result, and filters out any null
/// characters (which represent deletions, e.g. `k -> (nothing)`).
///
/// Returns an empty `Vec` if the input is empty.
pub fn gradation_pipeline(word: &[char], grade: Grade) -> Vec<char> {
    let z = match Zipper::new(word.to_vec()) {
        Some(z) => z,
        None => return Vec::new(),
    };

    let result = z.extend(|zi| apply_gradation(zi, grade));
    result.to_vec().into_iter().filter(|&c| c != '\0').collect()
}

/// Convenience wrapper: apply gradation to a `&str` and return a `String`.
pub fn gradate(word: &str, grade: Grade) -> String {
    let chars: Vec<char> = word.chars().collect();
    gradation_pipeline(&chars, grade).into_iter().collect()
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
}
