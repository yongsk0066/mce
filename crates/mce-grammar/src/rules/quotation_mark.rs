//! Quotation mark matching.
//!
//! Finnish uses several quotation styles:
//! - ASCII straight quotes: "..."
//! - Typographic quotes: \u{201C}...\u{201D} (left/right double)
//! - Guillemets: \u{00BB}...\u{00AB} (Finnish style: >>...<<)
//!
//! This rule checks that opening quotation marks have corresponding
//! closing marks.

use crate::{AnnotatedToken, GrammarError, GrammarRule};

/// Detects unmatched quotation marks.
///
/// Scans through all tokens (word and non-word) looking for quotation
/// mark characters. Reports an error if the count is odd (unmatched)
/// or if typographic/guillemet pairs are mismatched.
///
/// # Error code
///
/// `QUOTATION_MARK_ERROR`
pub struct QuotationMarkRule;

impl QuotationMarkRule {
    /// Create a new quotation mark rule.
    pub fn new() -> Self {
        Self
    }
}

impl Default for QuotationMarkRule {
    fn default() -> Self {
        Self::new()
    }
}

/// State for tracking paired quotation marks.
#[derive(Debug)]
struct QuoteState {
    /// Position (start byte offset) of the opening quote.
    open_pos: usize,
    /// The kind of opening quote.
    kind: QuoteKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteKind {
    /// ASCII straight double quote: "
    Straight,
    /// Typographic left double: \u{201C}
    TypographicLeft,
    /// Guillemet right-pointing (opening in Finnish): \u{00BB}
    GuilRight,
}

impl GrammarRule for QuotationMarkRule {
    fn id(&self) -> &'static str {
        "QUOTATION_MARK_ERROR"
    }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();
        let mut stack: Vec<QuoteState> = Vec::new();

        for token in tokens {
            for ch in token.text.chars() {
                match ch {
                    '"' => {
                        // ASCII straight quote toggles open/close.
                        if let Some(pos) = stack.iter().rposition(|s| s.kind == QuoteKind::Straight)
                        {
                            stack.remove(pos);
                        } else {
                            stack.push(QuoteState {
                                open_pos: token.start,
                                kind: QuoteKind::Straight,
                            });
                        }
                    }
                    '\u{201C}' => {
                        // Left double quotation mark — opening.
                        stack.push(QuoteState {
                            open_pos: token.start,
                            kind: QuoteKind::TypographicLeft,
                        });
                    }
                    '\u{201D}' => {
                        // Right double quotation mark — closing.
                        if let Some(pos) = stack
                            .iter()
                            .rposition(|s| s.kind == QuoteKind::TypographicLeft)
                        {
                            stack.remove(pos);
                        } else {
                            errors.push(GrammarError::new(
                                token.start,
                                token.end,
                                "QUOTATION_MARK_ERROR",
                                "Closing \u{201D} without matching opening \u{201C}".to_string(),
                            ));
                        }
                    }
                    '\u{00BB}' => {
                        // Right-pointing guillemet — opening in Finnish.
                        stack.push(QuoteState {
                            open_pos: token.start,
                            kind: QuoteKind::GuilRight,
                        });
                    }
                    '\u{00AB}' => {
                        // Left-pointing guillemet — closing in Finnish.
                        if let Some(pos) =
                            stack.iter().rposition(|s| s.kind == QuoteKind::GuilRight)
                        {
                            stack.remove(pos);
                        } else {
                            errors.push(GrammarError::new(
                                token.start,
                                token.end,
                                "QUOTATION_MARK_ERROR",
                                "Closing \u{00AB} without matching opening \u{00BB}".to_string(),
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }

        for unmatched in &stack {
            let label = match unmatched.kind {
                QuoteKind::Straight => "\"",
                QuoteKind::TypographicLeft => "\u{201C}",
                QuoteKind::GuilRight => "\u{00BB}",
            };
            errors.push(GrammarError::new(
                unmatched.open_pos,
                unmatched.open_pos + label.len(),
                "QUOTATION_MARK_ERROR",
                format!("Unmatched opening quotation mark: {}", label),
            ));
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, start: usize, end: usize) -> AnnotatedToken {
        AnnotatedToken::word(text, start, end, None)
    }

    fn non_word(text: &str, start: usize, end: usize) -> AnnotatedToken {
        AnnotatedToken::non_word(text, start, end)
    }

    // --- Positive detections ---

    #[test]
    fn detects_unmatched_straight_quote() {
        let rule = QuotationMarkRule::new();
        // Only one " present.
        let tokens = vec![
            word("Hän", 0, 4),
            non_word(" ", 4, 5),
            word("sanoi", 5, 10),
            non_word(" \"", 10, 12),
            word("hei", 12, 15),
        ];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "QUOTATION_MARK_ERROR");
    }

    #[test]
    fn detects_unmatched_typographic_left() {
        let rule = QuotationMarkRule::new();
        let tokens = vec![non_word("\u{201C}", 0, 3), word("hei", 3, 6)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("Unmatched"));
    }

    #[test]
    fn detects_orphan_typographic_right() {
        let rule = QuotationMarkRule::new();
        let tokens = vec![word("hei", 0, 3), non_word("\u{201D}", 3, 6)];
        let errors = rule.check(&tokens);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("without matching"));
    }

    // --- No false positives ---

    #[test]
    fn no_error_for_matched_straight_quotes() {
        let rule = QuotationMarkRule::new();
        let tokens = vec![
            non_word("\"", 0, 1),
            word("hei", 1, 4),
            non_word("\"", 4, 5),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_matched_typographic_quotes() {
        let rule = QuotationMarkRule::new();
        let tokens = vec![
            non_word("\u{201C}", 0, 3),
            word("hei", 3, 6),
            non_word("\u{201D}", 6, 9),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_matched_guillemets() {
        let rule = QuotationMarkRule::new();
        let tokens = vec![
            non_word("\u{00BB}", 0, 2),
            word("hei", 2, 5),
            non_word("\u{00AB}", 5, 7),
        ];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_empty_input() {
        let rule = QuotationMarkRule::new();
        let errors = rule.check(&[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn no_error_for_text_without_quotes() {
        let rule = QuotationMarkRule::new();
        let tokens = vec![word("Koira", 0, 5), word("juoksee", 6, 13)];
        let errors = rule.check(&tokens);
        assert!(errors.is_empty());
    }

    #[test]
    fn rule_id() {
        let rule = QuotationMarkRule::new();
        assert_eq!(rule.id(), "QUOTATION_MARK_ERROR");
    }

    #[test]
    fn default_trait() {
        let rule = QuotationMarkRule::default();
        assert_eq!(rule.id(), "QUOTATION_MARK_ERROR");
    }
}
