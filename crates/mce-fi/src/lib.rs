//! MCE Finnish — 핀란드어 언어 모듈.
//!
//! 핀란드어 고유의 음운 상수, 형태론 규칙, FST 기반 형태소 분석을 담당.
//!
//! - [`morphology`]: FST 출력 파싱 + 형태소 분석기 (`FinnishAnalyzer`)
//! - 음운 상수: 모음, 자음, 모음 조화

pub mod compound;
pub mod generator;
pub mod hyphenation;
pub mod morphology;
pub mod spellcheck;
mod tag_parser;

/// Finnish vowels (lowercase): a, e, i, o, u, y, ä, ö.
pub const VOWELS: &[char] = &['a', 'e', 'i', 'o', 'u', 'y', '\u{00E4}', '\u{00F6}'];

/// Finnish consonants (lowercase).
pub const CONSONANTS: &[char] = &[
    'b', 'c', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'm', 'n', 'p', 'q', 'r', 's', 't', 'v', 'w', 'x',
    'z', '\u{0161}', '\u{017E}',
];

/// Back vowels used in Finnish vowel harmony (lowercase + uppercase).
pub const BACK_VOWELS: &[char] = &['a', 'o', 'u', 'A', 'O', 'U'];

/// Front vowels corresponding to back vowels (same index order).
pub const FRONT_VOWELS: &[char] = &['\u{00E4}', '\u{00F6}', 'y', '\u{00C4}', '\u{00D6}', 'Y'];

/// Vowel pairs that may be split by a hyphen in Finnish.
pub const SPLIT_VOWELS: &[[char; 2]] = &[
    ['a', 'e'],
    ['a', 'o'],
    ['e', 'a'],
    ['e', 'o'],
    ['i', 'a'],
    ['i', 'o'],
    ['o', 'a'],
    ['o', 'e'],
    ['u', 'a'],
    ['u', 'e'],
    ['y', 'e'],
    ['e', '\u{00E4}'],
    ['e', '\u{00F6}'],
    ['i', '\u{00E4}'],
    ['i', '\u{00F6}'],
    ['y', '\u{00E4}'],
    ['\u{00E4}', 'e'],
    ['\u{00F6}', 'e'],
];

/// Check if a character is a Finnish vowel (case-insensitive).
///
/// Delegates to [`mce_core::character::is_finnish_vowel`] after lowercasing.
pub fn is_vowel(c: char) -> bool {
    mce_core::character::is_finnish_vowel(mce_core::character::simple_lower(c))
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
