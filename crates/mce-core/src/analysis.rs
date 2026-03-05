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

/// Number of known attribute keys.
const ATTR_COUNT: usize = 21;

/// Enum index for each known attribute key.
/// The discriminant value is the array index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
enum AttrKey {
    Baseform = 0,
    Class = 1,
    Comparison = 2,
    Focus = 3,
    Fstoutput = 4,
    Kysymysliite = 5,
    MalagaVapaaJalkiosa = 6,
    Mood = 7,
    Negative = 8,
    Number = 9,
    Participle = 10,
    Person = 11,
    Possessive = 12,
    PossibleGeographicalName = 13,
    RequireFollowingVerb = 14,
    Sijamuoto = 15,
    Structure = 16,
    Tense = 17,
    Weight = 18,
    Wordbases = 19,
    Wordids = 20,
}

/// All AttrKey variants in index order, for iteration.
const ALL_KEYS: [AttrKey; ATTR_COUNT] = [
    AttrKey::Baseform,
    AttrKey::Class,
    AttrKey::Comparison,
    AttrKey::Focus,
    AttrKey::Fstoutput,
    AttrKey::Kysymysliite,
    AttrKey::MalagaVapaaJalkiosa,
    AttrKey::Mood,
    AttrKey::Negative,
    AttrKey::Number,
    AttrKey::Participle,
    AttrKey::Person,
    AttrKey::Possessive,
    AttrKey::PossibleGeographicalName,
    AttrKey::RequireFollowingVerb,
    AttrKey::Sijamuoto,
    AttrKey::Structure,
    AttrKey::Tense,
    AttrKey::Weight,
    AttrKey::Wordbases,
    AttrKey::Wordids,
];

/// String name for each AttrKey variant (matches the ATTR_* constants).
const KEY_NAMES: [&str; ATTR_COUNT] = [
    "BASEFORM",
    "CLASS",
    "COMPARISON",
    "FOCUS",
    "FSTOUTPUT",
    "KYSYMYSLIITE",
    "MALAGA_VAPAA_JALKIOSA",
    "MOOD",
    "NEGATIVE",
    "NUMBER",
    "PARTICIPLE",
    "PERSON",
    "POSSESSIVE",
    "POSSIBLE_GEOGRAPHICAL_NAME",
    "REQUIRE_FOLLOWING_VERB",
    "SIJAMUOTO",
    "STRUCTURE",
    "TENSE",
    "WEIGHT",
    "WORDBASES",
    "WORDIDS",
];

impl AttrKey {
    /// Convert a string key to an AttrKey.
    /// Returns None for unrecognized keys.
    #[inline]
    fn from_str(s: &str) -> Option<Self> {
        // Hot path: check the most frequently accessed keys first.
        // Based on codebase analysis: CLASS, BASEFORM, SIJAMUOTO are the top 3.
        match s {
            "CLASS" => Some(AttrKey::Class),
            "BASEFORM" => Some(AttrKey::Baseform),
            "SIJAMUOTO" => Some(AttrKey::Sijamuoto),
            "NUMBER" => Some(AttrKey::Number),
            "MOOD" => Some(AttrKey::Mood),
            "PERSON" => Some(AttrKey::Person),
            "TENSE" => Some(AttrKey::Tense),
            "PARTICIPLE" => Some(AttrKey::Participle),
            "STRUCTURE" => Some(AttrKey::Structure),
            "POSSESSIVE" => Some(AttrKey::Possessive),
            "COMPARISON" => Some(AttrKey::Comparison),
            "NEGATIVE" => Some(AttrKey::Negative),
            "WEIGHT" => Some(AttrKey::Weight),
            "FOCUS" => Some(AttrKey::Focus),
            "FSTOUTPUT" => Some(AttrKey::Fstoutput),
            "KYSYMYSLIITE" => Some(AttrKey::Kysymysliite),
            "MALAGA_VAPAA_JALKIOSA" => Some(AttrKey::MalagaVapaaJalkiosa),
            "POSSIBLE_GEOGRAPHICAL_NAME" => Some(AttrKey::PossibleGeographicalName),
            "REQUIRE_FOLLOWING_VERB" => Some(AttrKey::RequireFollowingVerb),
            "WORDBASES" => Some(AttrKey::Wordbases),
            "WORDIDS" => Some(AttrKey::Wordids),
            _ => None,
        }
    }

    #[inline]
    fn index(self) -> usize {
        self as u8 as usize
    }

    #[inline]
    fn name(self) -> &'static str {
        KEY_NAMES[self.index()]
    }
}

/// Result of morphological analysis: a set of key-value attribute pairs.
///
/// Internally stored as a fixed-size array of 21 optional values, indexed by
/// `AttrKey`. This avoids HashMap allocation and hashing overhead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    slots: [Option<String>; ATTR_COUNT],
}

impl Analysis {
    pub fn new() -> Self {
        Self {
            slots: Default::default(),
        }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key_str: String = key.into();
        if let Some(ak) = AttrKey::from_str(&key_str) {
            self.slots[ak.index()] = Some(value.into());
        }
        // Unknown keys are silently ignored (matches previous behavior where
        // callers always used known ATTR_* constants).
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        AttrKey::from_str(key)
            .and_then(|ak| self.slots[ak.index()].as_deref())
    }

    pub fn remove(&mut self, key: &str) {
        if let Some(ak) = AttrKey::from_str(key) {
            self.slots[ak.index()] = None;
        }
    }

    pub fn keys(&self) -> Vec<&str> {
        ALL_KEYS
            .iter()
            .filter(|ak| self.slots[ak.index()].is_some())
            .map(|ak| ak.name())
            .collect()
    }

    pub fn contains_key(&self, key: &str) -> bool {
        AttrKey::from_str(key)
            .is_some_and(|ak| self.slots[ak.index()].is_some())
    }

    /// Returns a HashMap view of the attributes for backward compatibility.
    pub fn attributes(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for ak in &ALL_KEYS {
            if let Some(val) = &self.slots[ak.index()] {
                map.insert(ak.name().to_string(), val.clone());
            }
        }
        map
    }

    /// Iterate over (key_name, value) pairs without allocating a HashMap.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &str)> + '_ {
        ALL_KEYS.iter().filter_map(|ak| {
            self.slots[ak.index()]
                .as_deref()
                .map(|v| (ak.name(), v))
        })
    }

    pub fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_none())
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
    fn attributes_returns_map() {
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
        assert_eq!(a.len(), 1);
        assert_eq!(a.get("BASEFORM"), Some("koira"));
    }

    #[test]
    fn iter_yields_all_set_attrs() {
        let mut a = Analysis::new();
        a.set("BASEFORM", "koira");
        a.set("CLASS", "nimisana");
        let pairs: Vec<_> = a.iter().collect();
        assert_eq!(pairs.len(), 2);
        assert!(pairs.contains(&("BASEFORM", "koira")));
        assert!(pairs.contains(&("CLASS", "nimisana")));
    }

    #[test]
    fn clone_produces_equal_analysis() {
        let mut a = Analysis::new();
        a.set("BASEFORM", "koira");
        a.set("CLASS", "nimisana");
        a.set("NUMBER", "singular");
        let b = a.clone();
        assert_eq!(a, b);
        assert_eq!(b.get("BASEFORM"), Some("koira"));
    }

    #[test]
    fn unknown_key_is_silent_noop() {
        let mut a = Analysis::new();
        a.set("UNKNOWN_KEY", "value");
        assert!(a.is_empty());
        assert_eq!(a.get("UNKNOWN_KEY"), None);
    }
}
