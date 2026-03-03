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
}
