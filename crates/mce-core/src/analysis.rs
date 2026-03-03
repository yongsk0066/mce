// Adapted from corevoikko (voikko-core/analysis.rs)

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Attribute key constants
// Origin: Analysis.hpp:44-66 (voikko_mor_analysis::Key enum)
// ---------------------------------------------------------------------------

pub const ATTR_BASEFORM: &str = "BASEFORM";
pub const ATTR_CLASS: &str = "CLASS";
pub const ATTR_COMPARISON: &str = "COMPARISON";
pub const ATTR_FOCUS: &str = "FOCUS";
pub const ATTR_FSTOUTPUT: &str = "FSTOUTPUT";
pub const ATTR_KYSYMYSLIITE: &str = "KYSYMYSLIITE";
pub const ATTR_MALAGA_VAPAA_JALKIOSA: &str = "MALAGA_VAPAA_JALKIOSA";
pub const ATTR_MOOD: &str = "MOOD";
pub const ATTR_NEGATIVE: &str = "NEGATIVE";
pub const ATTR_NUMBER: &str = "NUMBER";
pub const ATTR_PARTICIPLE: &str = "PARTICIPLE";
pub const ATTR_PERSON: &str = "PERSON";
pub const ATTR_POSSESSIVE: &str = "POSSESSIVE";
pub const ATTR_POSSIBLE_GEOGRAPHICAL_NAME: &str = "POSSIBLE_GEOGRAPHICAL_NAME";
pub const ATTR_REQUIRE_FOLLOWING_VERB: &str = "REQUIRE_FOLLOWING_VERB";
pub const ATTR_SIJAMUOTO: &str = "SIJAMUOTO";
pub const ATTR_STRUCTURE: &str = "STRUCTURE";
pub const ATTR_TENSE: &str = "TENSE";
pub const ATTR_WEIGHT: &str = "WEIGHT";
pub const ATTR_WORDBASES: &str = "WORDBASES";
pub const ATTR_WORDIDS: &str = "WORDIDS";

/// Maximum word length for morphological analysis.
pub const MAX_WORD_CHARS: usize = 255;

/// Result of morphological analysis: a set of key-value attribute pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    attributes: HashMap<String, String>,
}

impl Analysis {
    pub fn new() -> Self {
        Self {
            attributes: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.attributes.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.attributes.get(key).map(String::as_str)
    }

    pub fn remove(&mut self, key: &str) {
        self.attributes.remove(key);
    }

    pub fn keys(&self) -> Vec<&str> {
        self.attributes.keys().map(String::as_str).collect()
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.attributes.contains_key(key)
    }

    pub fn attributes(&self) -> &HashMap<String, String> {
        &self.attributes
    }

    pub fn len(&self) -> usize {
        self.attributes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.attributes.is_empty()
    }
}

impl Default for Analysis {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_analysis_is_empty() {
        let a = Analysis::new();
        assert!(a.is_empty());
        assert_eq!(a.len(), 0);
    }

    #[test]
    fn set_and_get() {
        let mut a = Analysis::new();
        a.set("BASEFORM", "koira");
        assert_eq!(a.get("BASEFORM"), Some("koira"));
        assert_eq!(a.get("CLASS"), None);
    }

    #[test]
    fn set_replaces_existing() {
        let mut a = Analysis::new();
        a.set("BASEFORM", "koira");
        a.set("BASEFORM", "kissa");
        assert_eq!(a.get("BASEFORM"), Some("kissa"));
        assert_eq!(a.len(), 1);
    }

    #[test]
    fn remove_attribute() {
        let mut a = Analysis::new();
        a.set("CLASS", "nimisana");
        a.remove("CLASS");
        assert!(a.is_empty());
    }

    #[test]
    fn keys_returns_all_keys() {
        let mut a = Analysis::new();
        a.set("BASEFORM", "koira");
        a.set("CLASS", "nimisana");
        a.set("NUMBER", "singular");
        let mut keys = a.keys();
        keys.sort();
        assert_eq!(keys, vec!["BASEFORM", "CLASS", "NUMBER"]);
    }

    #[test]
    fn contains_key_present_and_absent() {
        let mut a = Analysis::new();
        a.set("BASEFORM", "koira");
        assert!(a.contains_key("BASEFORM"));
        assert!(!a.contains_key("CLASS"));
    }

    #[test]
    fn attributes_returns_inner_map() {
        let mut a = Analysis::new();
        a.set("BASEFORM", "koira");
        a.set("CLASS", "nimisana");
        let attrs = a.attributes();
        assert_eq!(attrs.len(), 2);
        assert_eq!(attrs.get("BASEFORM").map(String::as_str), Some("koira"));
        assert_eq!(attrs.get("CLASS").map(String::as_str), Some("nimisana"));
    }

    #[test]
    fn remove_nonexistent_key_is_noop() {
        let mut a = Analysis::new();
        a.set("BASEFORM", "koira");
        a.remove("NONEXISTENT");
        // The existing entry should remain intact.
        assert_eq!(a.len(), 1);
        assert_eq!(a.get("BASEFORM"), Some("koira"));
    }
}
