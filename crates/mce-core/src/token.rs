// Adapted from corevoikko (voikko-core/token.rs + enums.rs)

/// Token type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    None,
    Word,
    Punctuation,
    Whitespace,
    Unknown,
}

/// Sentence boundary type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SentenceType {
    None,
    Possible,
    Probable,
}

/// A text token produced by the tokenizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub token_type: TokenType,
    pub text: String,
    pub token_len: usize,
    pub pos: usize,
}

impl Token {
    pub fn new(token_type: TokenType, text: impl Into<String>, pos: usize) -> Self {
        let text = text.into();
        let token_len = text.chars().count();
        Self {
            token_type,
            text,
            token_len,
            pos,
        }
    }

    pub fn none() -> Self {
        Self {
            token_type: TokenType::None,
            text: String::new(),
            token_len: 0,
            pos: 0,
        }
    }
}

impl Default for Token {
    fn default() -> Self {
        Self::none()
    }
}

/// Result of sentence boundary detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sentence {
    pub sentence_type: SentenceType,
    pub sentence_len: usize,
}

impl Sentence {
    pub fn new(sentence_type: SentenceType, sentence_len: usize) -> Self {
        Self {
            sentence_type,
            sentence_len,
        }
    }

    pub fn none() -> Self {
        Self {
            sentence_type: SentenceType::None,
            sentence_len: 0,
        }
    }
}

impl Default for Sentence {
    fn default() -> Self {
        Self::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_new() {
        let tok = Token::new(TokenType::Word, "koira", 0);
        assert_eq!(tok.token_type, TokenType::Word);
        assert_eq!(tok.text, "koira");
        assert_eq!(tok.token_len, 5);
    }

    #[test]
    fn token_unicode_length() {
        let tok = Token::new(TokenType::Word, "\u{00E4}iti", 0);
        assert_eq!(tok.token_len, 4);
    }

    #[test]
    fn sentence_default() {
        let s = Sentence::default();
        assert_eq!(s.sentence_type, SentenceType::None);
    }
}
