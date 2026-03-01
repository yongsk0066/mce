// Adapted from corevoikko (voikko-core/analysis.rs)

use std::collections::HashMap;

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
}
