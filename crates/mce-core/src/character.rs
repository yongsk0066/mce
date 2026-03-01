// Adapted from corevoikko (voikko-core/character.rs)
// Finnish-specific parts (vowels, consonants) moved to mce-fi.

/// Character type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharType {
    Unknown,
    Letter,
    Digit,
    Whitespace,
    Punctuation,
}

/// Classify a character into letter, digit, whitespace, punctuation, or unknown.
pub fn get_char_type(c: char) -> CharType {
    let cp = c as u32;
    if (0x41..=0x5A).contains(&cp)
        || (0x61..=0x7A).contains(&cp)
        || (0xC1..=0xD6).contains(&cp)
        || (0xD8..=0xF6).contains(&cp)
        || (0x00F8..=0x02AF).contains(&cp)
        || (0x0400..=0x0481).contains(&cp)
        || (0x048A..=0x0527).contains(&cp)
        || (0x1400..=0x15C3).contains(&cp)
        || (0xFB00..=0xFB04).contains(&cp)
    {
        return CharType::Letter;
    }
    if is_whitespace(c) {
        return CharType::Whitespace;
    }
    if is_punctuation(c) {
        return CharType::Punctuation;
    }
    if c.is_ascii_digit() {
        return CharType::Digit;
    }
    CharType::Unknown
}

fn is_punctuation(c: char) -> bool {
    matches!(
        c,
        '.' | ','
            | ';'
            | '-'
            | '!'
            | '?'
            | ':'
            | '\''
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '/'
            | '&'
            | '"'
            | '\u{00AD}'
            | '\u{00BB}'
            | '\u{2010}'
            | '\u{2011}'
            | '\u{2013}'
            | '\u{2014}'
            | '\u{2019}'
            | '\u{201C}'
            | '\u{201D}'
            | '\u{2026}'
    )
}

/// Simple one-to-one lowercase mapping.
pub fn simple_lower(c: char) -> char {
    let mut iter = c.to_lowercase();
    iter.next().unwrap_or(c)
}

/// Simple one-to-one uppercase mapping.
pub fn simple_upper(c: char) -> char {
    let mut iter = c.to_uppercase();
    iter.next().unwrap_or(c)
}

pub fn is_upper(c: char) -> bool {
    c != simple_lower(c) || c == '\u{018F}'
}

pub fn is_lower(c: char) -> bool {
    c != simple_upper(c)
}

pub fn is_whitespace(c: char) -> bool {
    let cp = c as u32;
    (0x09..=0x0D).contains(&cp)
        || cp == 0x20
        || cp == 0x85
        || cp == 0xA0
        || cp == 0x1680
        || cp == 0x180E
        || (0x2000..=0x200A).contains(&cp)
        || cp == 0x2028
        || cp == 0x2029
        || cp == 0x202F
        || cp == 0x205F
        || cp == 0x3000
}

pub fn equals_ignore_case(a: &[char], b: &[char]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(&ca, &cb)| simple_lower(ca) == simple_lower(cb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_type_letters() {
        assert_eq!(get_char_type('A'), CharType::Letter);
        assert_eq!(get_char_type('z'), CharType::Letter);
        assert_eq!(get_char_type('\u{00C4}'), CharType::Letter); // Ä
    }

    #[test]
    fn char_type_digits() {
        assert_eq!(get_char_type('0'), CharType::Digit);
        assert_eq!(get_char_type('9'), CharType::Digit);
    }

    #[test]
    fn char_type_whitespace() {
        assert_eq!(get_char_type(' '), CharType::Whitespace);
        assert_eq!(get_char_type('\t'), CharType::Whitespace);
    }

    #[test]
    fn char_type_punctuation() {
        assert_eq!(get_char_type('.'), CharType::Punctuation);
        assert_eq!(get_char_type(','), CharType::Punctuation);
    }

    #[test]
    fn simple_case_conversion() {
        assert_eq!(simple_lower('A'), 'a');
        assert_eq!(simple_upper('a'), 'A');
        assert_eq!(simple_lower('\u{00C4}'), '\u{00E4}'); // Ä -> ä
    }

    #[test]
    fn equals_ignore_case_basic() {
        let a: Vec<char> = "Hello".chars().collect();
        let b: Vec<char> = "hello".chars().collect();
        assert!(equals_ignore_case(&a, &b));
    }
}
