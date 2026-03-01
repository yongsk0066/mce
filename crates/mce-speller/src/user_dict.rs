//! User dictionary for custom words that bypass spell checking.
//!
//! Stores user-added words in a [`HashSet`] and supports persistence via
//! a simple newline-delimited text file. Words in the user dictionary are
//! always considered correctly spelled, and can also appear as spelling
//! suggestions.

use std::collections::HashSet;
use std::io;
use std::path::Path;

/// A user-managed dictionary of custom words.
///
/// Words added here are treated as correctly spelled by the spell checker,
/// regardless of whether they appear in the main dictionary or pass
/// morphological validation.
///
/// # Persistence
///
/// The dictionary can be saved to and loaded from a newline-delimited text
/// file using [`save_to_file`](Self::save_to_file) and
/// [`load_from_file`](Self::load_from_file). Each line in the file is one
/// word. Empty lines and leading/trailing whitespace are ignored on load.
#[derive(Debug, Clone)]
pub struct UserDictionary {
    words: HashSet<String>,
}

impl UserDictionary {
    /// Create a new, empty user dictionary.
    pub fn new() -> Self {
        Self {
            words: HashSet::new(),
        }
    }

    /// Add a word to the user dictionary.
    ///
    /// The word is stored as-is (no lowercasing). If the word is already
    /// present, this is a no-op.
    pub fn add_word(&mut self, word: &str) {
        if !word.is_empty() {
            self.words.insert(word.to_string());
        }
    }

    /// Remove a word from the user dictionary.
    ///
    /// Returns `true` if the word was present and removed, `false` if it
    /// was not in the dictionary.
    pub fn remove_word(&mut self, word: &str) -> bool {
        self.words.remove(word)
    }

    /// Check whether a word is in the user dictionary.
    pub fn contains(&self, word: &str) -> bool {
        self.words.contains(word)
    }

    /// Return the number of words in the user dictionary.
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// Check whether the user dictionary is empty.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Return an iterator over all words in the user dictionary.
    pub fn words(&self) -> impl Iterator<Item = &str> {
        self.words.iter().map(|s| s.as_str())
    }

    /// Save the user dictionary to a newline-delimited text file.
    ///
    /// Words are written in sorted order for deterministic output.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the file cannot be created or written to.
    pub fn save_to_file(&self, path: &Path) -> io::Result<()> {
        let mut words: Vec<&str> = self.words.iter().map(|s| s.as_str()).collect();
        words.sort();
        let content = words.join("\n");
        std::fs::write(path, content)
    }

    /// Load the user dictionary from a newline-delimited text file.
    ///
    /// Each non-empty line (after trimming whitespace) becomes a word in the
    /// dictionary. If the file does not exist, returns an empty dictionary.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the file exists but cannot be read.
    pub fn load_from_file(path: &Path) -> io::Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }

        let content = std::fs::read_to_string(path)?;
        let mut dict = Self::new();

        for line in content.lines() {
            let word = line.trim();
            if !word.is_empty() {
                dict.add_word(word);
            }
        }

        Ok(dict)
    }

    /// Create a user dictionary from an iterator of words.
    pub fn from_words<I, S>(words: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut dict = Self::new();
        for word in words {
            dict.add_word(word.as_ref());
        }
        dict
    }
}

impl Default for UserDictionary {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn new_dict_is_empty() {
        let dict = UserDictionary::new();
        assert!(dict.is_empty());
        assert_eq!(dict.len(), 0);
    }

    #[test]
    fn add_and_contains() {
        let mut dict = UserDictionary::new();
        assert!(!dict.contains("MCE"));
        dict.add_word("MCE");
        assert!(dict.contains("MCE"));
        assert_eq!(dict.len(), 1);
    }

    #[test]
    fn add_duplicate_is_noop() {
        let mut dict = UserDictionary::new();
        dict.add_word("hello");
        dict.add_word("hello");
        assert_eq!(dict.len(), 1);
    }

    #[test]
    fn add_empty_string_is_noop() {
        let mut dict = UserDictionary::new();
        dict.add_word("");
        assert!(dict.is_empty());
    }

    #[test]
    fn remove_word() {
        let mut dict = UserDictionary::new();
        dict.add_word("test");
        assert!(dict.remove_word("test"));
        assert!(!dict.contains("test"));
        assert!(dict.is_empty());
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut dict = UserDictionary::new();
        assert!(!dict.remove_word("nope"));
    }

    #[test]
    fn words_iterator() {
        let mut dict = UserDictionary::new();
        dict.add_word("alpha");
        dict.add_word("beta");

        let mut words: Vec<&str> = dict.words().collect();
        words.sort();
        assert_eq!(words, vec!["alpha", "beta"]);
    }

    #[test]
    fn from_words() {
        let dict = UserDictionary::from_words(["one", "two", "three"]);
        assert_eq!(dict.len(), 3);
        assert!(dict.contains("one"));
        assert!(dict.contains("two"));
        assert!(dict.contains("three"));
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("mce_user_dict_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_dict.txt");

        let mut dict = UserDictionary::new();
        dict.add_word("koira");
        dict.add_word("kissa");
        dict.add_word("MCE");

        dict.save_to_file(&path).unwrap();

        let loaded = UserDictionary::load_from_file(&path).unwrap();
        assert_eq!(loaded.len(), 3);
        assert!(loaded.contains("koira"));
        assert!(loaded.contains("kissa"));
        assert!(loaded.contains("MCE"));

        // Clean up.
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn load_from_nonexistent_file_returns_empty() {
        let path = Path::new("/tmp/mce_nonexistent_dict_file_12345.txt");
        let dict = UserDictionary::load_from_file(path).unwrap();
        assert!(dict.is_empty());
    }

    #[test]
    fn load_ignores_empty_lines_and_whitespace() {
        let dir = std::env::temp_dir().join("mce_user_dict_ws_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ws_dict.txt");

        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "  hello  ").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "world").unwrap();
        writeln!(file, "   ").unwrap();
        writeln!(file, "test").unwrap();
        drop(file);

        let loaded = UserDictionary::load_from_file(&path).unwrap();
        assert_eq!(loaded.len(), 3);
        assert!(loaded.contains("hello"));
        assert!(loaded.contains("world"));
        assert!(loaded.contains("test"));

        // Clean up.
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn save_produces_sorted_output() {
        let dir = std::env::temp_dir().join("mce_user_dict_sort_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sorted_dict.txt");

        let mut dict = UserDictionary::new();
        dict.add_word("zebra");
        dict.add_word("alpha");
        dict.add_word("mango");

        dict.save_to_file(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines, vec!["alpha", "mango", "zebra"]);

        // Clean up.
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn default_is_empty() {
        let dict = UserDictionary::default();
        assert!(dict.is_empty());
    }

    #[test]
    fn unicode_words() {
        let mut dict = UserDictionary::new();
        dict.add_word("p\u{00E4}\u{00E4}kaupunki"); // pääkaupunki
        dict.add_word("\u{00E4}\u{00E4}ni"); // ääni
        assert!(dict.contains("p\u{00E4}\u{00E4}kaupunki"));
        assert!(dict.contains("\u{00E4}\u{00E4}ni"));
    }
}
