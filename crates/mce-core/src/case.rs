// Adapted from corevoikko (voikko-core/case.rs)

use crate::character::{is_lower, is_upper, simple_lower, simple_upper};

/// Classification of character casing within a word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaseType {
    NoLetters,
    AllLower,
    FirstUpper,
    Complex,
    AllUpper,
}

/// Detect the case pattern of a character slice.
pub fn detect_case(word: &[char]) -> CaseType {
    if word.is_empty() {
        return CaseType::NoLetters;
    }

    let mut first_uc = false;
    let mut rest_lc = true;
    let mut all_uc = true;
    let mut no_letters = true;

    if is_upper(word[0]) {
        first_uc = true;
        no_letters = false;
    }
    if is_lower(word[0]) {
        all_uc = false;
        no_letters = false;
    }

    for &c in &word[1..] {
        if is_upper(c) {
            no_letters = false;
            rest_lc = false;
        }
        if is_lower(c) {
            all_uc = false;
            no_letters = false;
        }
    }

    if no_letters {
        return CaseType::NoLetters;
    }
    if all_uc {
        return CaseType::AllUpper;
    }
    if !rest_lc {
        return CaseType::Complex;
    }
    if first_uc {
        CaseType::FirstUpper
    } else {
        CaseType::AllLower
    }
}

/// Apply a case transformation to a mutable character slice.
pub fn set_case(word: &mut [char], case_type: CaseType) {
    if word.is_empty() {
        return;
    }
    match case_type {
        CaseType::NoLetters | CaseType::Complex => {}
        CaseType::AllLower => {
            for c in word.iter_mut() {
                *c = simple_lower(*c);
            }
        }
        CaseType::AllUpper => {
            for c in word.iter_mut() {
                *c = simple_upper(*c);
            }
        }
        CaseType::FirstUpper => {
            word[0] = simple_upper(word[0]);
            for c in word[1..].iter_mut() {
                *c = simple_lower(*c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn to_string(cs: &[char]) -> String {
        cs.iter().collect()
    }

    #[test]
    fn detect_all_lower() {
        assert_eq!(detect_case(&chars("koira")), CaseType::AllLower);
    }

    #[test]
    fn detect_first_upper() {
        assert_eq!(detect_case(&chars("Helsinki")), CaseType::FirstUpper);
    }

    #[test]
    fn detect_all_upper() {
        assert_eq!(detect_case(&chars("KOIRA")), CaseType::AllUpper);
    }

    #[test]
    fn set_case_roundtrip() {
        let mut w = chars("KOIRA");
        set_case(&mut w, CaseType::AllLower);
        assert_eq!(to_string(&w), "koira");
        set_case(&mut w, CaseType::FirstUpper);
        assert_eq!(to_string(&w), "Koira");
    }

    #[test]
    fn detect_no_letters_empty() {
        assert_eq!(detect_case(&[]), CaseType::NoLetters);
    }

    #[test]
    fn detect_no_letters_digits() {
        assert_eq!(detect_case(&chars("12345")), CaseType::NoLetters);
    }

    #[test]
    fn detect_complex_mixed_case() {
        // "mCE" has lower first char, upper in the middle -> Complex
        assert_eq!(detect_case(&chars("mCE")), CaseType::Complex);
        // "iPhone" is also Complex
        assert_eq!(detect_case(&chars("iPhone")), CaseType::Complex);
    }

    #[test]
    fn finnish_chars_case_detection() {
        // ä is lowercase, Ä is uppercase
        assert_eq!(detect_case(&chars("äiti")), CaseType::AllLower);
        assert_eq!(detect_case(&chars("Äiti")), CaseType::FirstUpper);
        assert_eq!(detect_case(&chars("ÄITI")), CaseType::AllUpper);
        // ö
        assert_eq!(detect_case(&chars("öljy")), CaseType::AllLower);
        assert_eq!(detect_case(&chars("Öljy")), CaseType::FirstUpper);
    }

    #[test]
    fn set_case_no_letters_and_complex_are_noop() {
        let mut w = chars("123");
        set_case(&mut w, CaseType::NoLetters);
        assert_eq!(to_string(&w), "123");

        let mut w2 = chars("mCE");
        set_case(&mut w2, CaseType::Complex);
        assert_eq!(to_string(&w2), "mCE");
    }
}
