//! MCE Finnish — 핀란드어 언어 모듈.
//!
//! 핀란드어 고유의 음운 상수, 형태론 규칙, 사전 로딩 등을 담당.

/// Finnish vowels (lowercase): a, e, i, o, u, y, ä, ö.
pub const VOWELS: &[char] = &['a', 'e', 'i', 'o', 'u', 'y', '\u{00E4}', '\u{00F6}'];

/// Finnish consonants (lowercase).
pub const CONSONANTS: &[char] = &[
    'b', 'c', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'm', 'n', 'p', 'q', 'r', 's', 't', 'v', 'w',
    'x', 'z', '\u{0161}', '\u{017E}',
];

/// Check if a character is a Finnish vowel (case-insensitive).
pub fn is_vowel(c: char) -> bool {
    let lower = mce_core::character::simple_lower(c);
    VOWELS.contains(&lower)
}

/// Check if a character is a Finnish consonant (case-insensitive).
pub fn is_consonant(c: char) -> bool {
    let lower = mce_core::character::simple_lower(c);
    CONSONANTS.contains(&lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finnish_vowels() {
        assert!(is_vowel('a'));
        assert!(is_vowel('A'));
        assert!(is_vowel('\u{00E4}')); // ä
        assert!(is_vowel('\u{00C4}')); // Ä
        assert!(!is_vowel('k'));
    }

    #[test]
    fn finnish_consonants() {
        assert!(is_consonant('k'));
        assert!(is_consonant('K'));
        assert!(!is_consonant('a'));
    }
}
