//! M3: Compound Word Analyzer — Pushdown Transducer for compound splitting.
//!
//! Finnish compound words are formed by concatenating stems, sometimes with
//! linking elements (e.g., `-n-` genitive, `-en-`). The compound structure is
//! inherently context-free (binary branching tree), so a pushdown mechanism is
//! required for structural analysis. Here, the call stack serves as the
//! implicit pushdown stack via recursive descent.
//!
//! # Examples
//!
//! ```
//! use mce_core::compound::{CompoundAnalyzer, CompoundSplit};
//!
//! let dict = |w: &str| matches!(w, "rauta" | "tie" | "asema" | "kissa" | "pentu");
//! let analyzer = CompoundAnalyzer::new(dict);
//! let splits = analyzer.analyze("rautatieasema");
//! assert!(!splits.is_empty());
//! assert_eq!(splits[0].parts.len(), 3); // rauta + tie + asema
//! ```

/// A single part of a compound word split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompoundPart {
    /// Surface form of this part as it appears in the original word.
    pub surface: String,
    /// Byte offset of the start of this part in the original word.
    pub start: usize,
    /// Byte offset of the end of this part in the original word.
    pub end: usize,
    /// True if this part is a linking element (e.g., "n", "en") rather than a
    /// dictionary word.
    pub is_linking: bool,
}

/// One way to split a compound word into parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompoundSplit {
    /// The constituent parts of the compound word, in order.
    pub parts: Vec<CompoundPart>,
    /// Penalty score (lower is better). Fewer parts and known dictionary words
    /// yield lower penalties.
    pub penalty: u32,
}

impl CompoundSplit {
    /// Return only the dictionary word parts (excluding linking elements).
    pub fn word_parts(&self) -> Vec<&CompoundPart> {
        self.parts.iter().filter(|p| !p.is_linking).collect()
    }
}

/// Finnish linking elements that can appear between compound parts.
///
/// These are remnants of genitive or other case forms used to join compound
/// constituents. Ordered by length (longest first) for greedy matching.
///
/// Common Finnish linking morphemes:
/// - `"en"`: genitive plural or nen-stem (hevos**en**kenkä)
/// - `"n"`: genitive singular (kissa**n**pentu)
/// - `"s"`: sometimes appears between parts (työ**s**kentely — though usually
///   not a compound; used in e.g. kirjoitu**s**pöytä-type words)
/// - `"i"`, `"o"`, `"u"`: rarer linking vowels
/// - `""` (zero): direct concatenation (jää**kaappi**)
const LINKING_ELEMENTS: &[&str] = &["en", "n", "s", "i", "o", "u"];

/// Penalty constants for scoring compound splits.
const PENALTY_PER_PART: u32 = 10;
const PENALTY_LINKING: u32 = 5;
const PENALTY_SHORT_PART: u32 = 20;

/// Stem reconstruction function type.
///
/// Given a surface stem (left part after stripping a linking morpheme) and the
/// linking morpheme itself, returns possible dictionary forms to try.
///
/// For example, in Finnish, when the linking morpheme is `"en"` and the surface
/// stem is `"hevos"`, the reconstructor should return `["hevonen"]` because the
/// nen-stem word `hevonen` has the compound stem `hevos-`.
///
/// Return an empty vector if no reconstruction is applicable.
pub type StemReconstructor = Box<dyn Fn(&str, &str) -> Vec<String>>;

/// Compound word analyzer using recursive descent (pushdown transducer).
///
/// The analyzer is generic over the dictionary lookup function, allowing it to
/// work with any dictionary backend (succinct trie, FST, hash set, etc.).
///
/// # Type Parameter
///
/// - `F`: A function `&str -> bool` that returns `true` if the given string is
///   a known dictionary word.
pub struct CompoundAnalyzer<F: Fn(&str) -> bool> {
    lookup: F,
    /// Minimum length (in bytes) for a compound part to be considered valid.
    /// Default: 3.
    min_part_len: usize,
    /// Maximum number of word parts (excluding linking elements) in a compound.
    /// Default: 5.
    max_parts: usize,
    /// Optional stem reconstruction function for language-specific stem→dict
    /// form recovery. When a linking morpheme is stripped from the left part,
    /// this function is called to produce candidate dictionary forms.
    stem_reconstructor: Option<StemReconstructor>,
}

impl<F: Fn(&str) -> bool> CompoundAnalyzer<F> {
    /// Create a new compound analyzer with default settings.
    ///
    /// - `min_part_len`: 3
    /// - `max_parts`: 5
    /// - `stem_reconstructor`: None
    pub fn new(lookup: F) -> Self {
        Self {
            lookup,
            min_part_len: 3,
            max_parts: 5,
            stem_reconstructor: None,
        }
    }

    /// Set the minimum byte length for a compound part.
    pub fn with_min_part_len(mut self, len: usize) -> Self {
        self.min_part_len = len;
        self
    }

    /// Set the maximum number of word parts (excluding linking elements).
    pub fn with_max_parts(mut self, max: usize) -> Self {
        self.max_parts = max;
        self
    }

    /// Set a stem reconstruction function for language-specific linking
    /// morpheme handling.
    ///
    /// The function receives `(surface_stem, linking_morpheme)` and returns
    /// candidate dictionary forms. For example, Finnish `("hevos", "en")`
    /// should yield `["hevonen"]`.
    pub fn with_stem_reconstructor(mut self, f: StemReconstructor) -> Self {
        self.stem_reconstructor = Some(f);
        self
    }

    /// Analyze a word and return all valid compound splits, sorted by penalty
    /// (lowest first).
    ///
    /// Returns an empty vector if the word has no valid compound split (i.e.,
    /// it is a single dictionary word or not decomposable).
    ///
    /// A "compound split" requires at least two dictionary word parts. A word
    /// that is itself in the dictionary is not returned as a compound split.
    pub fn analyze(&self, word: &str) -> Vec<CompoundSplit> {
        if word.is_empty() {
            return Vec::new();
        }

        // Handle hyphenated compounds: split at hyphens first, then analyze
        // each segment.
        if word.contains('-') {
            return self.analyze_hyphenated(word);
        }

        let mut results: Vec<CompoundSplit> = Vec::new();
        let mut path: Vec<CompoundPart> = Vec::new();

        self.recurse(word, 0, &mut path, &mut results);

        // Only keep splits with at least 2 word parts.
        results.retain(|s| s.word_parts().len() >= 2);

        results.sort_by_key(|s| s.penalty);
        results
    }

    /// Analyze a hyphenated compound by splitting at hyphens first.
    ///
    /// Each hyphen-separated segment is checked against the dictionary. If all
    /// segments are known words, a compound split is produced. Segments that
    /// are themselves compound words are further decomposed.
    fn analyze_hyphenated(&self, word: &str) -> Vec<CompoundSplit> {
        let segments: Vec<&str> = word.split('-').collect();

        if segments.len() < 2 {
            return Vec::new();
        }

        // Check that we do not exceed max_parts.
        if segments.len() > self.max_parts {
            return Vec::new();
        }

        // Check that no segment is empty (consecutive hyphens or leading/trailing).
        if segments.iter().any(|s| s.is_empty()) {
            return Vec::new();
        }

        // Try treating each segment as a single dictionary word.
        let all_known = segments.iter().all(|s| (self.lookup)(s));

        let mut results: Vec<CompoundSplit> = Vec::new();

        if all_known {
            let mut parts = Vec::new();
            let mut byte_offset = 0;

            for (i, seg) in segments.iter().enumerate() {
                if i > 0 {
                    // Account for the hyphen.
                    parts.push(CompoundPart {
                        surface: "-".to_string(),
                        start: byte_offset,
                        end: byte_offset + 1,
                        is_linking: true,
                    });
                    byte_offset += 1;
                }

                parts.push(CompoundPart {
                    surface: (*seg).to_string(),
                    start: byte_offset,
                    end: byte_offset + seg.len(),
                    is_linking: false,
                });
                byte_offset += seg.len();
            }

            let penalty = compute_penalty(&parts);
            results.push(CompoundSplit { parts, penalty });
        }

        results.sort_by_key(|s| s.penalty);
        results
    }

    /// Recursive descent: try all valid splits from `pos` onward.
    ///
    /// The call stack acts as the pushdown stack, tracking the current nesting
    /// depth of compound constituents.
    fn recurse(
        &self,
        word: &str,
        pos: usize,
        path: &mut Vec<CompoundPart>,
        results: &mut Vec<CompoundSplit>,
    ) {
        let remaining = &word[pos..];

        if remaining.is_empty() {
            // We have consumed the entire word.
            let penalty = compute_penalty(path);
            results.push(CompoundSplit {
                parts: path.clone(),
                penalty,
            });
            return;
        }

        // Count current word parts (non-linking) to enforce max_parts.
        let word_part_count = path.iter().filter(|p| !p.is_linking).count();
        if word_part_count >= self.max_parts {
            return;
        }

        // Try matching dictionary words of increasing length starting from `pos`.
        let byte_len = remaining.len();

        for end_byte in self.min_part_len..=byte_len {
            // Only split on char boundaries.
            if !remaining.is_char_boundary(end_byte) {
                continue;
            }

            let candidate = &remaining[..end_byte];
            let next_pos = pos + end_byte;
            let at_end = next_pos == word.len();

            // --- Strategy 1: Direct dictionary match (zero linking) ---
            if (self.lookup)(candidate) {
                path.push(CompoundPart {
                    surface: candidate.to_string(),
                    start: pos,
                    end: next_pos,
                    is_linking: false,
                });

                if at_end {
                    self.recurse(word, next_pos, path, results);
                } else {
                    // Try continuing directly (no linking element).
                    self.recurse(word, next_pos, path, results);

                    // Try linking elements *after* this word, before the next part.
                    let after = &word[next_pos..];
                    for &link in LINKING_ELEMENTS {
                        if after.starts_with(link) {
                            let link_end = next_pos + link.len();
                            // Make sure there is still room for another word part after.
                            if link_end + self.min_part_len <= word.len() {
                                path.push(CompoundPart {
                                    surface: link.to_string(),
                                    start: next_pos,
                                    end: link_end,
                                    is_linking: true,
                                });
                                self.recurse(word, link_end, path, results);
                                path.pop(); // backtrack linking element
                            }
                        }
                    }
                }

                path.pop(); // backtrack dictionary word
            }

            // --- Strategy 2: Linking morpheme fused into left part ---
            //
            // The surface candidate ends with a linking morpheme that must be
            // stripped to recover the dictionary stem. For example:
            //   "hevosen" → strip "en" → stem "hevos" → reconstruct "hevonen"
            //
            // This strategy only applies when the candidate is NOT the last
            // part (linking morphemes only appear between parts).
            if !at_end {
                for &link in LINKING_ELEMENTS {
                    if candidate.len() > link.len() && candidate.ends_with(link) {
                        let stem = &candidate[..candidate.len() - link.len()];

                        // The bare stem must meet minimum length.
                        if stem.len() < self.min_part_len {
                            continue;
                        }

                        // Check if the bare stem itself is a dictionary word.
                        let stem_is_word = (self.lookup)(stem);

                        // Try stem reconstruction via the callback.
                        let reconstructed_match = self
                            .stem_reconstructor
                            .as_ref()
                            .map(|f| f(stem, link).iter().any(|form| (self.lookup)(form)))
                            .unwrap_or(false);

                        if stem_is_word || reconstructed_match {
                            // Record the stem as the word part and the linking
                            // morpheme as a separate linking element.
                            let stem_end = pos + stem.len();
                            let link_end = next_pos;

                            path.push(CompoundPart {
                                surface: stem.to_string(),
                                start: pos,
                                end: stem_end,
                                is_linking: false,
                            });
                            path.push(CompoundPart {
                                surface: link.to_string(),
                                start: stem_end,
                                end: link_end,
                                is_linking: true,
                            });

                            self.recurse(word, link_end, path, results);

                            path.pop(); // backtrack linking element
                            path.pop(); // backtrack stem
                        }
                    }
                }
            }
        }
    }
}

/// Compute the penalty for a given split.
fn compute_penalty(parts: &[CompoundPart]) -> u32 {
    let mut penalty: u32 = 0;

    for part in parts {
        if part.is_linking {
            penalty += PENALTY_LINKING;
        } else {
            penalty += PENALTY_PER_PART;
            // Penalize very short parts (< 3 chars by character count).
            if part.surface.chars().count() < 3 {
                penalty += PENALTY_SHORT_PART;
            }
        }
    }

    penalty
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small Finnish dictionary for testing.
    fn test_dict(word: &str) -> bool {
        matches!(
            word,
            "rauta"
                | "tie"
                | "asema"
                | "kissa"
                | "pentu"
                | "koira"
                | "koti"
                | "maa"
                | "alue"
                | "lento"
                | "kone"
                | "tehdas"
                | "auto"
                | "talo"
                | "kirja"
                | "kauppa"
                | "rautatie"
                | "rautatiekin"
                | "j\u{00E4}\u{00E4}"   // jää
                | "kaappi"               // kaappi
                | "hevonen"              // hevonen (nen-stem)
                | "kenk\u{00E4}"         // kenkä
                | "kirjoitus"            // kirjoitus
                | "p\u{00F6}yt\u{00E4}"  // pöytä
                | "p\u{00E4}\u{00E4}"    // pää
                | "kaupunki"             // kaupunki
                | "ty\u{00F6}"           // työ
                | "ty\u{00F6}skentely" // työskentely (single word, not compound)
        )
    }

    /// Finnish stem reconstructor for testing.
    ///
    /// Given a bare stem and a linking morpheme, returns candidate dictionary
    /// forms. For example, `("hevos", "en")` -> `["hevonen"]`.
    ///
    /// Finnish nen-stem words: hevonen → compound stem hevos-, genitive hevosen.
    /// The compound linking form is `stem + en` where stem ends in `s`.
    /// To recover the dictionary form: strip trailing `s`, add `nen`.
    /// E.g., hevos → hevo + nen = hevonen.
    fn test_stem_reconstructor(stem: &str, link: &str) -> Vec<String> {
        let mut candidates = Vec::new();
        match link {
            "en" => {
                // nen-stem words: if stem ends in 's', replace 's' with 'nen'.
                // E.g., hevos → hevonen, nais → nainen, ihmis → ihminen
                if let Some(base) = stem.strip_suffix('s') {
                    candidates.push(format!("{base}nen"));
                }
            }
            _ => {}
        }
        candidates
    }

    // -----------------------------------------------------------------------
    // Simple two-part compounds
    // -----------------------------------------------------------------------

    #[test]
    fn two_part_compound_koirakoti() {
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("koirakoti");
        assert!(!splits.is_empty(), "should find at least one split");

        let best = &splits[0];
        let words: Vec<&str> = best
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(words, vec!["koira", "koti"]);
    }

    #[test]
    fn two_part_compound_autotalo() {
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("autotalo");
        assert!(!splits.is_empty());

        let best = &splits[0];
        let words: Vec<&str> = best
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(words, vec!["auto", "talo"]);
    }

    // -----------------------------------------------------------------------
    // Three-part compounds
    // -----------------------------------------------------------------------

    #[test]
    fn three_part_compound_rautatieasema() {
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("rautatieasema");

        // Should find "rauta + tie + asema" (3 parts).
        let three_part = splits.iter().find(|s| s.word_parts().len() == 3);
        assert!(three_part.is_some(), "should find a 3-part split");

        let tp = three_part.unwrap();
        let words: Vec<&str> = tp.word_parts().iter().map(|p| p.surface.as_str()).collect();
        assert_eq!(words, vec!["rauta", "tie", "asema"]);
    }

    #[test]
    fn three_part_compound_lentokonetehdas() {
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("lentokonetehdas");
        assert!(!splits.is_empty());

        // Should find "lento + kone + tehdas".
        let best = splits
            .iter()
            .find(|s| s.word_parts().len() == 3)
            .expect("should find a 3-part split");
        let words: Vec<&str> = best
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(words, vec!["lento", "kone", "tehdas"]);
    }

    // -----------------------------------------------------------------------
    // Hyphenated compounds
    // -----------------------------------------------------------------------

    #[test]
    fn hyphenated_compound_maa_alue() {
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("maa-alue");
        assert!(!splits.is_empty());

        let best = &splits[0];
        let word_parts: Vec<&str> = best
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(word_parts, vec!["maa", "alue"]);

        // The hyphen should be recorded as a linking element.
        assert!(best.parts.iter().any(|p| p.is_linking && p.surface == "-"));
    }

    #[test]
    fn hyphenated_three_parts() {
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("auto-kirja-kauppa");
        assert!(!splits.is_empty());

        let best = &splits[0];
        let word_parts: Vec<&str> = best
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(word_parts, vec!["auto", "kirja", "kauppa"]);
    }

    // -----------------------------------------------------------------------
    // Linking element -n- (genitive)
    // -----------------------------------------------------------------------

    #[test]
    fn linking_n_kissanpentu() {
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("kissanpentu");
        assert!(!splits.is_empty(), "should find at least one split");

        let best = &splits[0];
        let word_parts: Vec<&str> = best
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(word_parts, vec!["kissa", "pentu"]);

        // Should have the linking element "n".
        let linking: Vec<&str> = best
            .parts
            .iter()
            .filter(|p| p.is_linking)
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(linking, vec!["n"]);
    }

    // -----------------------------------------------------------------------
    // No valid split
    // -----------------------------------------------------------------------

    #[test]
    fn single_word_not_compound() {
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("koira");
        assert!(
            splits.is_empty(),
            "single dictionary word is not a compound"
        );
    }

    #[test]
    fn unknown_word_no_split() {
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("abcdefgh");
        assert!(splits.is_empty(), "unknown word should have no splits");
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn empty_string() {
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("");
        assert!(splits.is_empty());
    }

    #[test]
    fn too_short_parts_rejected() {
        // With min_part_len=3 (default), the word "ab" cannot form a part.
        let dict = |w: &str| matches!(w, "ab" | "cd");
        let analyzer = CompoundAnalyzer::new(dict);
        let splits = analyzer.analyze("abcd");
        // "ab" and "cd" are each only 2 chars, below the default min_part_len.
        assert!(splits.is_empty());
    }

    #[test]
    fn custom_min_part_len() {
        let dict = |w: &str| matches!(w, "ab" | "cd");
        let analyzer = CompoundAnalyzer::new(dict).with_min_part_len(2);
        let splits = analyzer.analyze("abcd");
        assert!(!splits.is_empty());

        let best = &splits[0];
        let words: Vec<&str> = best
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(words, vec!["ab", "cd"]);
    }

    #[test]
    fn max_parts_enforced() {
        let dict = |w: &str| matches!(w, "abc" | "def" | "ghi");
        let analyzer = CompoundAnalyzer::new(dict).with_max_parts(2);
        let splits = analyzer.analyze("abcdefghi");
        // 3-part split should be excluded because max_parts = 2.
        for s in &splits {
            assert!(s.word_parts().len() <= 2, "should not exceed max_parts=2");
        }
    }

    // -----------------------------------------------------------------------
    // Multiple valid splits with ranking
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_splits_ranked() {
        // "rautatieasema" can be split as:
        //   1. rauta + tie + asema  (3 word parts, penalty = 30)
        //   2. rautatie + asema     (2 word parts, penalty = 20)
        // The 2-part split should have lower penalty and be ranked first.
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("rautatieasema");
        assert!(
            splits.len() >= 2,
            "should find at least 2 splits, found {}",
            splits.len()
        );

        let first = &splits[0];
        let second = &splits[1];
        assert!(
            first.penalty <= second.penalty,
            "splits should be sorted by penalty"
        );

        // The 2-part split "rautatie + asema" should come first.
        let first_words: Vec<&str> = first
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(first_words, vec!["rautatie", "asema"]);
    }

    // -----------------------------------------------------------------------
    // Byte offset correctness
    // -----------------------------------------------------------------------

    #[test]
    fn byte_offsets_correct() {
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("koirakoti");
        let best = &splits[0];

        assert_eq!(best.parts[0].start, 0);
        assert_eq!(best.parts[0].end, 5); // "koira" = 5 bytes
        assert_eq!(best.parts[1].start, 5);
        assert_eq!(best.parts[1].end, 9); // "koti" = 4 bytes

        // Verify surface forms match the byte slices.
        let word = "koirakoti";
        for part in &best.parts {
            assert_eq!(&word[part.start..part.end], part.surface);
        }
    }

    #[test]
    fn byte_offsets_with_linking() {
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("kissanpentu");
        let best = &splits[0];

        // "kissa" (0..5) + "n" (5..6) + "pentu" (6..11)
        assert_eq!(best.parts.len(), 3);
        assert_eq!(best.parts[0].surface, "kissa");
        assert_eq!(best.parts[0].start, 0);
        assert_eq!(best.parts[0].end, 5);
        assert_eq!(best.parts[1].surface, "n");
        assert_eq!(best.parts[1].start, 5);
        assert_eq!(best.parts[1].end, 6);
        assert_eq!(best.parts[1].is_linking, true);
        assert_eq!(best.parts[2].surface, "pentu");
        assert_eq!(best.parts[2].start, 6);
        assert_eq!(best.parts[2].end, 11);
    }

    #[test]
    fn byte_offsets_hyphenated() {
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("maa-alue");
        let best = &splits[0];

        // "maa" (0..3) + "-" (3..4) + "alue" (4..8)
        assert_eq!(best.parts.len(), 3);
        assert_eq!(best.parts[0].surface, "maa");
        assert_eq!(best.parts[0].start, 0);
        assert_eq!(best.parts[0].end, 3);
        assert_eq!(best.parts[1].surface, "-");
        assert_eq!(best.parts[1].start, 3);
        assert_eq!(best.parts[1].end, 4);
        assert_eq!(best.parts[1].is_linking, true);
        assert_eq!(best.parts[2].surface, "alue");
        assert_eq!(best.parts[2].start, 4);
        assert_eq!(best.parts[2].end, 8);
    }

    // -----------------------------------------------------------------------
    // Unicode (Finnish characters with diacritics)
    // -----------------------------------------------------------------------

    #[test]
    fn unicode_compound() {
        let dict = |w: &str| matches!(w, "p\u{00E4}\u{00E4}" | "kaupunki");
        let analyzer = CompoundAnalyzer::new(dict).with_min_part_len(2);
        let splits = analyzer.analyze("p\u{00E4}\u{00E4}kaupunki");
        assert!(!splits.is_empty());

        let best = &splits[0];
        let words: Vec<&str> = best
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(words, vec!["p\u{00E4}\u{00E4}", "kaupunki"]);

        // Verify byte offsets are correct for multi-byte chars.
        let word = "p\u{00E4}\u{00E4}kaupunki";
        for part in &best.parts {
            assert_eq!(&word[part.start..part.end], part.surface);
        }
    }

    // -----------------------------------------------------------------------
    // word_parts helper
    // -----------------------------------------------------------------------

    #[test]
    fn word_parts_excludes_linking() {
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("kissanpentu");
        let best = &splits[0];

        let wp = best.word_parts();
        assert_eq!(wp.len(), 2);
        assert!(!wp[0].is_linking);
        assert!(!wp[1].is_linking);
    }

    // -----------------------------------------------------------------------
    // Linking morpheme tests (Finnish compound linking)
    // -----------------------------------------------------------------------

    #[test]
    fn zero_linking_jaakaappi() {
        // jääkaappi → jää + kaappi (zero linking morpheme)
        let analyzer = CompoundAnalyzer::new(test_dict).with_min_part_len(2);
        let splits = analyzer.analyze("j\u{00E4}\u{00E4}kaappi");
        assert!(!splits.is_empty(), "should find jää + kaappi");

        let best = &splits[0];
        let words: Vec<&str> = best
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(words, vec!["j\u{00E4}\u{00E4}", "kaappi"]);
    }

    #[test]
    fn en_linking_hevosenkenkä() {
        // hevosenkenkä → hevonen + kenkä (en-linking, stem hevos)
        // "hevosen" splits as stem "hevos" + linking "en", then reconstruct
        // "hevos" + "nen" = "hevonen" in dictionary.
        let reconstructor: StemReconstructor =
            Box::new(|stem, link| test_stem_reconstructor(stem, link));
        let analyzer = CompoundAnalyzer::new(test_dict).with_stem_reconstructor(reconstructor);
        let splits = analyzer.analyze("hevosenkenk\u{00E4}");
        assert!(
            !splits.is_empty(),
            "should find hevonen + kenkä via en-linking"
        );

        let best = &splits[0];
        let word_parts: Vec<&str> = best
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(word_parts, vec!["hevos", "kenk\u{00E4}"]);

        // Should have the linking element "en".
        let linking: Vec<&str> = best
            .parts
            .iter()
            .filter(|p| p.is_linking)
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(linking, vec!["en"]);
    }

    #[test]
    fn single_word_not_compound_tyoskentely() {
        // työskentely → not a compound (single word in dictionary)
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("ty\u{00F6}skentely");
        // "työskentely" is a single word; there should be no 2+ part split
        // where all parts are valid.
        assert!(
            splits.is_empty(),
            "ty\u{00F6}skentely is a single word, not a compound; got {:?}",
            splits
                .iter()
                .map(|s| s
                    .word_parts()
                    .iter()
                    .map(|p| p.surface.as_str())
                    .collect::<Vec<_>>())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn zero_linking_kirjoituspoyta() {
        // kirjoituspöytä → kirjoitus + pöytä (the "s" is part of "kirjoitus")
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("kirjoitusp\u{00F6}yt\u{00E4}");
        assert!(
            !splits.is_empty(),
            "should find kirjoitus + p\u{00F6}yt\u{00E4}"
        );

        let best = &splits[0];
        let words: Vec<&str> = best
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(words, vec!["kirjoitus", "p\u{00F6}yt\u{00E4}"]);
    }

    #[test]
    fn zero_linking_paakaupunki() {
        // pääkaupunki → pää + kaupunki (zero linking)
        let analyzer = CompoundAnalyzer::new(test_dict).with_min_part_len(2);
        let splits = analyzer.analyze("p\u{00E4}\u{00E4}kaupunki");
        assert!(
            !splits.is_empty(),
            "should find p\u{00E4}\u{00E4} + kaupunki"
        );

        let best = &splits[0];
        let words: Vec<&str> = best
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(words, vec!["p\u{00E4}\u{00E4}", "kaupunki"]);
    }

    #[test]
    fn zero_linking_rautatieasema() {
        // rautatieasema → rautatie + asema (zero linking)
        let analyzer = CompoundAnalyzer::new(test_dict);
        let splits = analyzer.analyze("rautatieasema");
        assert!(!splits.is_empty());

        // The best (lowest penalty) should be the 2-part split.
        let best = &splits[0];
        let words: Vec<&str> = best
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(words, vec!["rautatie", "asema"]);
    }

    #[test]
    fn en_linking_byte_offsets() {
        // Verify byte offsets are correct for en-linking compound.
        // "hevosenkenkä" = "hevos"(5) + "en"(2) + "kenkä"(6, ä=2 bytes)
        let reconstructor: StemReconstructor =
            Box::new(|stem, link| test_stem_reconstructor(stem, link));
        let analyzer = CompoundAnalyzer::new(test_dict).with_stem_reconstructor(reconstructor);
        let word = "hevosenkenk\u{00E4}";
        let splits = analyzer.analyze(word);
        assert!(!splits.is_empty());

        let best = &splits[0];
        // Verify that all parts' byte ranges are valid slices of the word.
        for part in &best.parts {
            assert_eq!(
                &word[part.start..part.end],
                part.surface,
                "byte offset mismatch for part {:?}",
                part
            );
        }

        // Check structure: "hevos" + "en" + "kenkä"
        assert_eq!(best.parts.len(), 3);
        assert_eq!(best.parts[0].surface, "hevos");
        assert!(!best.parts[0].is_linking);
        assert_eq!(best.parts[1].surface, "en");
        assert!(best.parts[1].is_linking);
        assert_eq!(best.parts[2].surface, "kenk\u{00E4}");
        assert!(!best.parts[2].is_linking);
    }
}
