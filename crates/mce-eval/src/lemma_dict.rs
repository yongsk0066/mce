//! Dictionary-enhanced lemmatization from UD training data.
//!
//! Loads a TSV file of `(form, UPOS) -> lemma` mappings extracted from
//! the CoNLL-U training corpus. At evaluation time, the dictionary lemma
//! is preferred over the FST baseform when a match is found.
//!
//! # File Format
//!
//! Each line is tab-separated: `form<TAB>UPOS<TAB>lemma`.
//! - `form` is lowercase.
//! - Entries where `form == lemma.to_lowercase()` are omitted (identity).
//!   The lookup returns `None` for those; the caller should fall back to
//!   the lowercased surface form.
//!
//! # Usage
//!
//! ```no_run
//! use mce_eval::lemma_dict::LemmaDict;
//!
//! let dict = LemmaDict::from_file("data/lemma_dict.tsv").unwrap();
//! if let Some(lemma) = dict.lookup("juoksee", "VERB") {
//!     assert_eq!(lemma, "juosta");
//! }
//! ```

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

/// Strip Finnish inflectional suffixes from an OOV word to approximate the lemma.
///
/// This is a last-resort heuristic for words not found in the dictionary or FST.
/// Only strips if the remaining stem has at least 2 characters.
///
/// For NOUN/ADJ/PROPN: strips case suffixes (longest match first).
/// For VERB: strips person/number suffixes.
pub fn strip_suffix(form: &str, upos: &str) -> String {
    let lower = form.to_lowercase();

    let suffixes: &[&str] = match upos {
        "NOUN" | "ADJ" | "PROPN" => &[
            // Longest first to avoid partial matches.
            "ssa", "ssä", "sta", "stä", // inessive, elative (3 chars)
            "lle", "lta", "ltä", "lla", "llä", // allative, ablative, adessive (3 chars)
            "tta", "ttä", // abessive/partitive (3 chars)
            "ksi", "ine", // translative, comitative (3 chars)
            "na", "nä", // essive (2 chars)
            "aa", "ää", // partitive long vowel (2 chars)
            "ia", "iä", // partitive (2 chars)
            "ta", "tä", // partitive (2 chars)
            "a", "ä", // partitive short (1 char)
            "n", // genitive (1 char)
            "t", // plural nominative (1 char)
        ],
        "VERB" => &[
            "mme", // 1pl (3 chars)
            "tte", // 2pl (3 chars)
            "vat", "vät", // 3pl (3 chars)
            "an",  // passive (2 chars)
            "ee",  // 3sg long vowel (2 chars)
            "aa", "ää", // 3sg long vowel (2 chars)
            "n",  // 1sg (1 char)
            "t",  // 2sg (1 char)
        ],
        _ => return lower,
    };

    for suffix in suffixes {
        if let Some(stem) = lower.strip_suffix(suffix) {
            if stem.chars().count() >= 2 {
                return stem.to_string();
            }
        }
    }

    lower
}

/// A dictionary mapping `(lowercase_form, UPOS)` to the most frequent lemma.
pub struct LemmaDict {
    /// Key: `(lowercase_form, UPOS)`, Value: `lemma`.
    entries: HashMap<(String, String), String>,
}

impl LemmaDict {
    /// Load a lemma dictionary from a TSV file.
    ///
    /// Each line: `form<TAB>UPOS<TAB>lemma`.
    pub fn from_file(path: impl AsRef<Path>) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        Ok(Self::parse(&content))
    }

    /// Parse dictionary from a string (for testing or embedded data).
    pub fn parse(content: &str) -> Self {
        let mut entries = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            if parts.len() == 3 {
                entries.insert(
                    (parts[0].to_string(), parts[1].to_string()),
                    parts[2].to_string(),
                );
            }
        }
        LemmaDict { entries }
    }

    /// Create an empty dictionary (no-op).
    pub fn empty() -> Self {
        LemmaDict {
            entries: HashMap::new(),
        }
    }

    /// Look up the best lemma for a given `(form, UPOS)` pair.
    ///
    /// The form is matched case-insensitively (the dictionary stores
    /// lowercase forms). Returns `None` if no entry is found; the
    /// caller should fall back to FST baseform or surface form.
    pub fn lookup(&self, form: &str, upos: &str) -> Option<&str> {
        let key = (form.to_lowercase(), upos.to_string());
        self.entries.get(&key).map(|s| s.as_str())
    }

    /// Number of entries in the dictionary.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the dictionary is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Resolve the best lemma for a token, applying case normalization.
    ///
    /// Strategy:
    /// 1. If the dictionary has a (form, upos) entry, use it.
    /// 2. Otherwise, use the provided FST baseform.
    /// 3. Apply case normalization: if UPOS is not PROPN, lowercase the lemma.
    pub fn resolve_lemma(&self, form: &str, upos: &str, fst_baseform: &str) -> String {
        let raw = if let Some(dict_lemma) = self.lookup(form, upos) {
            dict_lemma.to_string()
        } else {
            fst_baseform.to_string()
        };

        // Case normalization: non-PROPN lemmas should be lowercase.
        if upos != "PROPN" {
            raw.to_lowercase()
        } else {
            raw
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_TSV: &str = "\
juoksee\tVERB\tjuosta
kävelyreitti\tNOUN\tkävely#reitti
jäällä\tNOUN\tjää
helsinkiin\tNOUN\tHelsinki
";

    #[test]
    fn parse_tsv() {
        let dict = LemmaDict::parse(SAMPLE_TSV);
        assert_eq!(dict.len(), 4);
    }

    #[test]
    fn lookup_found() {
        let dict = LemmaDict::parse(SAMPLE_TSV);
        assert_eq!(dict.lookup("juoksee", "VERB"), Some("juosta"));
        assert_eq!(dict.lookup("jäällä", "NOUN"), Some("jää"));
    }

    #[test]
    fn lookup_case_insensitive() {
        let dict = LemmaDict::parse(SAMPLE_TSV);
        assert_eq!(dict.lookup("Juoksee", "VERB"), Some("juosta"));
        assert_eq!(dict.lookup("JÄÄLLÄ", "NOUN"), Some("jää"));
    }

    #[test]
    fn lookup_not_found() {
        let dict = LemmaDict::parse(SAMPLE_TSV);
        assert_eq!(dict.lookup("koira", "NOUN"), None);
        assert_eq!(dict.lookup("juoksee", "NOUN"), None); // wrong UPOS
    }

    #[test]
    fn resolve_lemma_dict_hit() {
        let dict = LemmaDict::parse(SAMPLE_TSV);
        // Dict says juoksee -> juosta, FST says something else.
        let lemma = dict.resolve_lemma("juoksee", "VERB", "juoksea");
        assert_eq!(lemma, "juosta");
    }

    #[test]
    fn resolve_lemma_dict_miss() {
        let dict = LemmaDict::parse(SAMPLE_TSV);
        // Not in dict, use FST baseform (lowercased since NOUN != PROPN).
        let lemma = dict.resolve_lemma("koiraa", "NOUN", "Koira");
        assert_eq!(lemma, "koira");
    }

    #[test]
    fn resolve_lemma_propn_preserves_case() {
        let dict = LemmaDict::parse(SAMPLE_TSV);
        // PROPN should preserve case of FST baseform.
        let lemma = dict.resolve_lemma("Helsingin", "PROPN", "Helsinki");
        assert_eq!(lemma, "Helsinki");
    }

    #[test]
    fn resolve_lemma_non_propn_lowercases() {
        let dict = LemmaDict::parse(SAMPLE_TSV);
        // Non-PROPN: dict returns "Helsinki" but we lowercase it.
        let lemma = dict.resolve_lemma("helsinkiin", "NOUN", "helsinki");
        // Dict has "Helsinki" but NOUN -> lowercase.
        assert_eq!(lemma, "helsinki");
    }

    #[test]
    fn empty_dict() {
        let dict = LemmaDict::empty();
        assert!(dict.is_empty());
        assert_eq!(dict.lookup("koira", "NOUN"), None);
    }

    // --- strip_suffix tests ---

    #[test]
    fn strip_noun_inessive() {
        assert_eq!(strip_suffix("talossa", "NOUN"), "talo");
        assert_eq!(strip_suffix("metsässä", "NOUN"), "metsä");
    }

    #[test]
    fn strip_noun_elative() {
        assert_eq!(strip_suffix("talosta", "NOUN"), "talo");
        assert_eq!(strip_suffix("metsästä", "NOUN"), "metsä");
    }

    #[test]
    fn strip_noun_allative() {
        assert_eq!(strip_suffix("talolle", "NOUN"), "talo");
    }

    #[test]
    fn strip_noun_ablative() {
        assert_eq!(strip_suffix("talolta", "NOUN"), "talo");
        assert_eq!(strip_suffix("metsältä", "NOUN"), "metsä");
    }

    #[test]
    fn strip_noun_adessive() {
        assert_eq!(strip_suffix("talolla", "NOUN"), "talo");
        assert_eq!(strip_suffix("metsällä", "NOUN"), "metsä");
    }

    #[test]
    fn strip_noun_essive() {
        assert_eq!(strip_suffix("talona", "NOUN"), "talo");
        assert_eq!(strip_suffix("metsänä", "NOUN"), "metsä");
    }

    #[test]
    fn strip_noun_translative() {
        assert_eq!(strip_suffix("taloksi", "NOUN"), "talo");
    }

    #[test]
    fn strip_noun_genitive() {
        assert_eq!(strip_suffix("talon", "NOUN"), "talo");
    }

    #[test]
    fn strip_noun_partitive() {
        assert_eq!(strip_suffix("taloa", "NOUN"), "talo");
    }

    #[test]
    fn strip_noun_plural_nominative() {
        assert_eq!(strip_suffix("talot", "NOUN"), "talo");
    }

    #[test]
    fn strip_noun_minimum_stem() {
        // Stem must be >= 2 chars. "an" -> strip "n" -> "a" (1 char) -> no strip.
        assert_eq!(strip_suffix("an", "NOUN"), "an");
        // "ot" -> strip "t" -> "o" (1 char) -> no strip.
        assert_eq!(strip_suffix("ot", "NOUN"), "ot");
    }

    #[test]
    fn strip_verb_3pl() {
        assert_eq!(strip_suffix("juoksevat", "VERB"), "juokse");
        assert_eq!(strip_suffix("menevät", "VERB"), "mene");
    }

    #[test]
    fn strip_verb_1pl() {
        assert_eq!(strip_suffix("juoksemme", "VERB"), "juokse");
    }

    #[test]
    fn strip_verb_2pl() {
        assert_eq!(strip_suffix("juoksette", "VERB"), "juokse");
    }

    #[test]
    fn strip_verb_1sg() {
        assert_eq!(strip_suffix("juoksen", "VERB"), "juokse");
    }

    #[test]
    fn strip_verb_2sg() {
        assert_eq!(strip_suffix("juokset", "VERB"), "juokse");
    }

    #[test]
    fn strip_adj_works() {
        assert_eq!(strip_suffix("isossa", "ADJ"), "iso");
    }

    #[test]
    fn strip_other_upos_passthrough() {
        // Non NOUN/ADJ/PROPN/VERB should return as-is.
        assert_eq!(strip_suffix("quickly", "ADV"), "quickly");
        assert_eq!(strip_suffix("ja", "CCONJ"), "ja");
    }

    #[test]
    fn strip_longest_suffix_first() {
        // "talossa" should match "ssa" (3 chars), not "a" (1 char).
        assert_eq!(strip_suffix("talossa", "NOUN"), "talo");
    }

    #[test]
    fn resolve_lemma_oov_no_stripping_in_dict() {
        let dict = LemmaDict::empty();
        // LemmaDict::resolve_lemma does NOT do suffix stripping;
        // stripping is handled by the pipeline for OOV words.
        let lemma = dict.resolve_lemma("talossa", "NOUN", "talossa");
        assert_eq!(lemma, "talossa");
    }

    #[test]
    fn resolve_lemma_fst_hit_no_stripping() {
        let dict = LemmaDict::empty();
        // FST provided a real baseform.
        let lemma = dict.resolve_lemma("talossa", "NOUN", "talo");
        assert_eq!(lemma, "talo");
    }

    #[test]
    fn strip_suffix_used_for_oov() {
        // Verify that strip_suffix correctly handles what the pipeline would pass.
        assert_eq!(strip_suffix("talossa", "NOUN"), "talo");
        assert_eq!(strip_suffix("metsästä", "NOUN"), "metsä");
        assert_eq!(strip_suffix("juoksevat", "VERB"), "juokse");
    }

    // --- from_tsv / parse edge case tests ---

    #[test]
    fn parse_skips_malformed_lines_no_tab() {
        let tsv = "juoksee\tVERB\tjuosta\nmalformed_no_tabs\nkissa\tNOUN\tkissa\n";
        let dict = LemmaDict::parse(tsv);
        // Only two valid lines; the malformed line (no tab -> less than 3 parts) is skipped.
        assert_eq!(dict.len(), 2);
        assert_eq!(dict.lookup("juoksee", "VERB"), Some("juosta"));
        assert_eq!(dict.lookup("kissa", "NOUN"), Some("kissa"));
    }

    #[test]
    fn parse_skips_empty_lines_in_middle() {
        let tsv = "juoksee\tVERB\tjuosta\n\n\nkissa\tNOUN\tkissa\n";
        let dict = LemmaDict::parse(tsv);
        assert_eq!(dict.len(), 2);
    }

    #[test]
    fn parse_handles_whitespace_only_lines() {
        let tsv = "juoksee\tVERB\tjuosta\n   \n\t\nkissa\tNOUN\tkissa\n";
        let dict = LemmaDict::parse(tsv);
        assert_eq!(dict.len(), 2);
    }

    #[test]
    fn parse_line_with_only_one_tab() {
        // Only two fields: should be skipped (needs 3).
        let tsv = "juoksee\tVERB\n";
        let dict = LemmaDict::parse(tsv);
        assert_eq!(dict.len(), 0);
    }

    #[test]
    fn parse_lemma_with_tabs_in_value() {
        // splitn(3, '\t') means the third field captures everything after the second tab,
        // including any additional tabs.
        let tsv = "compound\tNOUN\tsome\textra\ttabs\n";
        let dict = LemmaDict::parse(tsv);
        assert_eq!(dict.len(), 1);
        // The lemma should be "some\textra\ttabs" since splitn(3, '\t') stops at 3 parts.
        assert_eq!(dict.lookup("compound", "NOUN"), Some("some\textra\ttabs"));
    }

    #[test]
    fn parse_empty_string() {
        let dict = LemmaDict::parse("");
        assert!(dict.is_empty());
        assert_eq!(dict.len(), 0);
    }

    #[test]
    fn lookup_returns_none_for_wrong_upos() {
        let dict = LemmaDict::parse(SAMPLE_TSV);
        // "juoksee" exists as VERB but not as NOUN.
        assert_eq!(dict.lookup("juoksee", "NOUN"), None);
        // "jäällä" exists as NOUN but not as VERB.
        assert_eq!(dict.lookup("jäällä", "VERB"), None);
    }

    #[test]
    fn lookup_case_insensitive_mixed_case() {
        let dict = LemmaDict::parse(SAMPLE_TSV);
        assert_eq!(dict.lookup("JuOkSeE", "VERB"), Some("juosta"));
        assert_eq!(dict.lookup("HELSINKIIN", "NOUN"), Some("Helsinki"));
    }

    #[test]
    fn resolve_lemma_propn_dict_hit_preserves_case() {
        // When dict has a hit for PROPN, the case should be preserved (not lowercased).
        let tsv = "helsingin\tPROPN\tHelsinki\n";
        let dict = LemmaDict::parse(tsv);
        let lemma = dict.resolve_lemma("helsingin", "PROPN", "helsinki");
        assert_eq!(lemma, "Helsinki");
    }

    #[test]
    fn resolve_lemma_empty_fst_baseform() {
        let dict = LemmaDict::empty();
        // When FST baseform is empty, resolve_lemma returns empty string (lowercased).
        let lemma = dict.resolve_lemma("koira", "NOUN", "");
        assert_eq!(lemma, "");
    }

    #[test]
    fn strip_suffix_verb_passive() {
        assert_eq!(strip_suffix("juostaan", "VERB"), "juosta");
    }

    #[test]
    fn strip_suffix_verb_3sg_long_vowel() {
        assert_eq!(strip_suffix("juoksee", "VERB"), "juoks");
    }

    #[test]
    fn strip_suffix_propn_treated_as_noun() {
        // PROPN uses the same suffix table as NOUN/ADJ.
        // "helsinkiin" -> strip "n" -> "helsinkii" (only "n" matches, not "in").
        assert_eq!(strip_suffix("helsinkiin", "PROPN"), "helsinkii");
        // "tampereella" -> strip "lla" -> "tamperee".
        assert_eq!(strip_suffix("tampereella", "PROPN"), "tamperee");
        // "suomessa" -> strip "ssa" -> "suome".
        assert_eq!(strip_suffix("suomessa", "PROPN"), "suome");
    }

    #[test]
    fn strip_noun_abessive() {
        assert_eq!(strip_suffix("talotta", "NOUN"), "talo");
        assert_eq!(strip_suffix("metsättä", "NOUN"), "metsä");
    }

    #[test]
    fn strip_noun_comitative() {
        assert_eq!(strip_suffix("taloine", "NOUN"), "talo");
    }

    #[test]
    fn strip_noun_partitive_ia() {
        assert_eq!(strip_suffix("koiria", "NOUN"), "koir");
    }
}
