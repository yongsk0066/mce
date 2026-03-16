// Adapted from corevoikko (voikko-fi/suggestion/status.rs)
//
// Suggestion status tracking: abort conditions, cost budget, deduplication.

use std::collections::HashSet;

/// A suggestion candidate with its computed priority.
///
/// Lower priority values indicate better suggestions.
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub word: String,
    /// Priority of the suggestion (lower is better).
    pub priority: i32,
}

/// Tracks the state of suggestion generation: found suggestions,
/// cost budget, and abort conditions.
///
/// Every spell-check attempt during suggestion generation increments
/// `current_cost` by 1 via `charge()`. The strategy sets `max_cost`
/// to control the computational budget (e.g., 800 for typing, 2000
/// for OCR).
pub struct SuggestionStatus<'a> {
    word: &'a [char],
    max_suggestions: usize,
    /// Budget limit; abort when exceeded (doubled if no suggestions found yet).
    max_cost: usize,
    current_cost: usize,
    suggestions: Vec<Suggestion>,
    /// Deduplication set for suggestion strings.
    seen: HashSet<String>,
}

impl<'a> SuggestionStatus<'a> {
    /// Create a new suggestion status for the given word.
    pub fn new(word: &'a [char], max_suggestions: usize) -> Self {
        Self {
            word,
            max_suggestions,
            max_cost: 0,
            current_cost: 0,
            suggestions: Vec::with_capacity(max_suggestions),
            seen: HashSet::new(),
        }
    }

    /// Returns `true` if no more suggestions should be generated.
    ///
    /// Abort conditions:
    /// - Maximum suggestion count reached.
    /// - Cost budget exceeded (doubled if no suggestions found yet).
    pub fn should_abort(&self) -> bool {
        if self.suggestions.len() >= self.max_suggestions {
            return true;
        }
        if self.current_cost < self.max_cost {
            return false;
        }
        // If no suggestions have been found, allow the search to take
        // twice as long as usual.
        if self.suggestions.is_empty() && self.current_cost < 2 * self.max_cost {
            return false;
        }
        true
    }

    /// Increment the cost counter by one unit (one morphological analysis
    /// or equivalent operation).
    pub fn charge(&mut self) {
        self.current_cost += 1;
    }

    /// Set the maximum computational cost.
    pub fn set_max_cost(&mut self, max_cost: usize) {
        self.max_cost = max_cost;
    }

    /// Add a new suggestion with the given base priority.
    ///
    /// The final priority is `priority * (suggestion_count + 5)`, which
    /// penalizes later-found suggestions to bias toward strategies executed
    /// earlier in the pipeline.
    ///
    /// Duplicate suggestions (same string) are silently ignored.
    pub fn add_suggestion(&mut self, suggestion: String, priority: i32) {
        if self.suggestions.len() >= self.max_suggestions {
            return;
        }
        if !self.seen.insert(suggestion.clone()) {
            return; // duplicate
        }
        let final_priority = priority * (self.suggestions.len() as i32 + 5);
        self.suggestions.push(Suggestion {
            word: suggestion,
            priority: final_priority,
        });
    }

    /// Sort suggestions by priority (ascending -- lower priority is better).
    pub fn sort_suggestions(&mut self) {
        self.suggestions.sort_by_key(|s| s.priority);
    }

    /// Return the current suggestion count.
    pub fn suggestion_count(&self) -> usize {
        self.suggestions.len()
    }

    /// Return the maximum number of suggestions.
    pub fn max_suggestion_count(&self) -> usize {
        self.max_suggestions
    }

    /// Return the word being checked.
    pub fn word(&self) -> &[char] {
        self.word
    }

    /// Return the word length.
    pub fn word_len(&self) -> usize {
        self.word.len()
    }

    /// Consume the status and return the collected suggestions.
    pub fn into_suggestions(self) -> Vec<Suggestion> {
        self.suggestions
    }

    /// Return a reference to the collected suggestions.
    pub fn suggestions(&self) -> &[Suggestion] {
        &self.suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn new_status_has_zero_cost_and_no_suggestions() {
        let word = chars("koira");
        let status = SuggestionStatus::new(&word, 5);
        assert_eq!(status.suggestion_count(), 0);
        assert_eq!(status.max_suggestion_count(), 5);
        assert_eq!(status.word_len(), 5);
    }

    #[test]
    fn should_abort_when_max_suggestions_reached() {
        let word = chars("ab");
        let mut status = SuggestionStatus::new(&word, 2);
        status.set_max_cost(1000);
        status.add_suggestion("a".to_string(), 1);
        assert!(!status.should_abort());
        status.add_suggestion("b".to_string(), 1);
        assert!(status.should_abort());
    }

    #[test]
    fn should_not_abort_when_cost_below_max() {
        let word = chars("abc");
        let mut status = SuggestionStatus::new(&word, 5);
        status.set_max_cost(100);
        for _ in 0..99 {
            status.charge();
        }
        assert!(!status.should_abort());
        status.charge(); // cost == max_cost, no suggestions -> still OK (doubled budget)
        assert!(!status.should_abort());
    }

    #[test]
    fn should_abort_at_doubled_budget_with_no_suggestions() {
        let word = chars("abc");
        let mut status = SuggestionStatus::new(&word, 5);
        status.set_max_cost(10);
        for _ in 0..20 {
            status.charge();
        }
        // cost == 2 * max_cost, no suggestions -> abort
        assert!(status.should_abort());
    }

    #[test]
    fn should_abort_at_max_cost_with_suggestions() {
        let word = chars("abc");
        let mut status = SuggestionStatus::new(&word, 5);
        status.set_max_cost(10);
        status.add_suggestion("test".to_string(), 1);
        for _ in 0..10 {
            status.charge();
        }
        // cost == max_cost, has suggestions -> abort
        assert!(status.should_abort());
    }

    #[test]
    fn add_suggestion_computes_final_priority() {
        let word = chars("abc");
        let mut status = SuggestionStatus::new(&word, 5);
        status.set_max_cost(1000);
        status.add_suggestion("first".to_string(), 10);
        // final_priority = 10 * (0 + 5) = 50
        assert_eq!(status.suggestions()[0].priority, 50);

        status.add_suggestion("second".to_string(), 10);
        // final_priority = 10 * (1 + 5) = 60
        assert_eq!(status.suggestions()[1].priority, 60);
    }

    #[test]
    fn duplicate_suggestions_are_ignored() {
        let word = chars("abc");
        let mut status = SuggestionStatus::new(&word, 5);
        status.set_max_cost(1000);
        status.add_suggestion("test".to_string(), 1);
        status.add_suggestion("test".to_string(), 2);
        assert_eq!(status.suggestion_count(), 1);
    }

    #[test]
    fn sort_suggestions_by_priority() {
        let word = chars("abc");
        let mut status = SuggestionStatus::new(&word, 5);
        status.set_max_cost(1000);
        status.add_suggestion("low".to_string(), 1);
        status.add_suggestion("high".to_string(), 100);
        status.add_suggestion("mid".to_string(), 10);
        status.sort_suggestions();
        assert_eq!(status.suggestions()[0].word, "low");
        assert_eq!(status.suggestions()[1].word, "mid");
        assert_eq!(status.suggestions()[2].word, "high");
    }

    #[test]
    fn excess_suggestions_are_dropped() {
        let word = chars("abc");
        let mut status = SuggestionStatus::new(&word, 2);
        status.set_max_cost(1000);
        status.add_suggestion("a".to_string(), 1);
        status.add_suggestion("b".to_string(), 1);
        status.add_suggestion("c".to_string(), 1);
        assert_eq!(status.suggestion_count(), 2);
    }

    #[test]
    fn word_returns_original_slice() {
        let word = chars("testi");
        let status = SuggestionStatus::new(&word, 5);
        assert_eq!(status.word(), &word[..]);
    }

    #[test]
    fn into_suggestions_consumes_status() {
        let word = chars("abc");
        let mut status = SuggestionStatus::new(&word, 5);
        status.set_max_cost(1000);
        status.add_suggestion("test".to_string(), 1);
        let suggestions = status.into_suggestions();
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].word, "test");
    }
}
