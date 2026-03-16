// Adapted from corevoikko (voikko-fi/tokenizer)

//! MCE Tokenizer -- text tokenization and sentence detection.
//!
//! Language-agnostic tokenizer that handles URL/email detection, word boundary
//! detection, and sentence boundary heuristics. The tokenizer classifies text
//! into word, whitespace, punctuation, and unknown tokens. The sentence
//! detector identifies probable and possible sentence boundaries based on
//! punctuation patterns.

use mce_core::character::{CharType, get_char_type};
use mce_core::token::{SentenceType, TokenType};

/// Check whether a character is a quotation mark commonly used in Finnish text.
///
/// Matches `"` (ASCII double quote), `\u{00BB}` (RIGHT-POINTING DOUBLE ANGLE
/// QUOTATION MARK), and `\u{201D}` (RIGHT DOUBLE QUOTATION MARK).
fn is_quotation_mark(c: char) -> bool {
    matches!(
        c,
        '"' | '\u{00BB}' // » RIGHT-POINTING DOUBLE ANGLE QUOTATION MARK
            | '\u{201D}' // RIGHT DOUBLE QUOTATION MARK
    )
}

/// Callback type for spell-checking a word (used in sentence detection for
/// abbreviation recognition). Returns `true` if the word is a valid word
/// in the dictionary.
type SpellCheckFn<'a> = Option<&'a dyn Fn(&[char]) -> bool>;

// ============================================================================
// URL / Email detection
// ============================================================================

/// Check whether the characters form a valid email-address character in the
/// "unknown" character class (characters that are not letter/digit/whitespace/
/// punctuation according to Voikko's classification).
fn is_email_unknown_char(c: char) -> bool {
    matches!(
        c,
        '#' | '$' | '%' | '*' | '+' | '=' | '^' | '_' | '`' | '|' | '~'
    )
}

/// Check whether a punctuation character is allowed in email addresses.
fn is_email_punctuation_char(c: char) -> bool {
    matches!(c, '!' | '&' | '\'' | '-' | '/' | '?' | '{' | '}' | '.')
}

/// Check whether an "unknown" character is allowed in HTTP URLs.
fn is_url_unknown_char(c: char) -> bool {
    matches!(c, '=' | '#' | '%')
}

/// Try to find a URL (http:// or https://) or email address starting at the
/// beginning of `text`. Returns the length of the URL/email token, or 0 if
/// none was found.
fn find_url_or_email(text: &[char]) -> usize {
    let textlen = text.len();

    // Min length 12: shortest real HTTP URL is ~12 chars (e.g. http://a.b.c).
    let is_http = textlen >= 12 && starts_with_chars(text, &['h', 't', 't', 'p', ':', '/', '/']);
    let is_https =
        textlen >= 12 && starts_with_chars(text, &['h', 't', 't', 'p', 's', ':', '/', '/']);

    if !is_http && !is_https {
        return find_email(text);
    }

    let start = if is_https { 8 } else { 7 };
    for i in start..textlen {
        match get_char_type(text[i]) {
            CharType::Whitespace => return i,
            CharType::Unknown => {
                if !is_url_unknown_char(text[i]) {
                    return i;
                }
            }
            CharType::Digit | CharType::Letter => {}
            CharType::Punctuation => {
                // A dot at end-of-text or before whitespace terminates the URL
                // (the dot is not part of the URL).
                if text[i] == '.'
                    && (i + 1 == textlen || get_char_type(text[i + 1]) == CharType::Whitespace)
                {
                    return i;
                }
                // All other punctuation is allowed inside URLs.
            }
        }
    }
    textlen
}

/// Try to find an email address at the start of `text`.
/// Returns the length of the email token, or 0 if none was found.
fn find_email(text: &[char]) -> usize {
    let textlen = text.len();
    if textlen < 6 {
        return 0;
    }

    let mut found_at = false;
    let mut found_dot = false;

    for i in 0..textlen {
        match get_char_type(text[i]) {
            CharType::Whitespace => {
                if found_at && found_dot {
                    return i;
                }
                return 0;
            }
            CharType::Unknown => {
                if text[i] == '@' {
                    if found_at {
                        return 0;
                    }
                    found_at = true;
                } else if !is_email_unknown_char(text[i]) {
                    if found_at && found_dot {
                        return i;
                    }
                    return 0;
                }
            }
            CharType::Digit | CharType::Letter => {}
            CharType::Punctuation => {
                if text[i] == '.' && found_at {
                    if i + 1 == textlen || get_char_type(text[i + 1]) == CharType::Whitespace {
                        if found_dot {
                            return i;
                        }
                        return 0;
                    }
                    found_dot = true;
                } else if !is_email_punctuation_char(text[i]) {
                    if found_at && found_dot {
                        return i;
                    }
                    return 0;
                }
            }
        }
    }

    if found_at && found_dot {
        return textlen;
    }
    0
}

/// Check whether `text` starts with exactly the characters in `prefix`.
fn starts_with_chars(text: &[char], prefix: &[char]) -> bool {
    if text.len() < prefix.len() {
        return false;
    }
    text[..prefix.len()] == *prefix
}

// ============================================================================
// Word length detection
// ============================================================================

/// Compute the length of a "word" token starting at the beginning of `text`.
///
/// The `ignore_dot` flag controls whether a trailing dot is considered part of
/// the word (used by the sentence detector to include dots in word tokens).
fn word_length(text: &[char], ignore_dot: bool) -> usize {
    let textlen = text.len();

    let url_length = find_url_or_email(text);
    if url_length != 0 {
        return url_length;
    }

    let adot: usize = if ignore_dot { 1 } else { 0 };
    let mut wlen: usize = 0;
    let mut processing_number = false;
    let mut seen_letters = false;

    while wlen < textlen {
        match get_char_type(text[wlen]) {
            CharType::Letter => {
                processing_number = false;
                seen_letters = true;
                wlen += 1;
            }
            CharType::Digit => {
                processing_number = true;
                wlen += 1;
            }
            CharType::Whitespace | CharType::Unknown => {
                return wlen;
            }
            CharType::Punctuation => {
                match text[wlen] {
                    // Apostrophe, right single quotation mark, colon:
                    // continue if followed by a letter.
                    '\'' | '\u{2019}' | ':' => {
                        if wlen + 1 == textlen {
                            return wlen;
                        }
                        if get_char_type(text[wlen + 1]) == CharType::Letter {
                            wlen += 1;
                        } else {
                            return wlen;
                        }
                    }

                    // Hyphen variants and soft hyphen.
                    '-' | '\u{00AD}' | '\u{2010}' | '\u{2011}' => {
                        if wlen + 1 == textlen {
                            return wlen + 1;
                        }
                        if is_quotation_mark(text[wlen + 1]) {
                            return wlen + 1;
                        }
                        match get_char_type(text[wlen + 1]) {
                            CharType::Letter | CharType::Digit => {
                                wlen += 1;
                            }
                            CharType::Whitespace | CharType::Unknown => {
                                return wlen + 1;
                            }
                            CharType::Punctuation => {
                                if text[wlen + 1] == ',' {
                                    return wlen + 1;
                                }
                                return wlen;
                            }
                        }
                    }

                    // Dot: include if followed by a letter; include followed
                    // by a digit only if no letters have been seen yet
                    // (e.g. "1.2.3" is one token, but "abc.1" is not).
                    '.' => {
                        if wlen + 1 == textlen {
                            return wlen + adot;
                        }
                        match get_char_type(text[wlen + 1]) {
                            CharType::Letter => {
                                wlen += 1;
                            }
                            CharType::Digit => {
                                if seen_letters {
                                    return wlen + adot;
                                }
                                wlen += 1;
                            }
                            CharType::Whitespace | CharType::Unknown | CharType::Punctuation => {
                                return wlen + adot;
                            }
                        }
                    }

                    // Comma: only include when processing a number and
                    // followed by a digit (e.g. "1,234").
                    ',' => {
                        if !processing_number {
                            return wlen;
                        }
                        if wlen + 1 == textlen {
                            return wlen;
                        }
                        if get_char_type(text[wlen + 1]) == CharType::Digit {
                            wlen += 1;
                        } else {
                            return wlen;
                        }
                    }

                    // Any other punctuation ends the word.
                    _ => {
                        return wlen;
                    }
                }
            }
        }
    }
    textlen
}

// ============================================================================
// Public tokenizer API
// ============================================================================

/// Find the next token starting at position `pos` in the text.
///
/// Returns `(TokenType, token_length)`. The caller advances `pos` by
/// `token_length` to process subsequent tokens.
///
/// The `ignore_dot` parameter controls whether trailing dots are considered
/// part of word tokens. This is normally `false`; the sentence detector sets
/// it to `false` internally (overriding any caller preference) so that dots
/// are always separate punctuation tokens during sentence detection.
pub fn next_token(text: &[char], text_len: usize, pos: usize) -> (TokenType, usize) {
    next_token_with_options(text, text_len, pos, false)
}

/// Find the next token starting at position `pos`, with explicit
/// `ignore_dot` control.
pub fn next_token_with_options(
    text: &[char],
    text_len: usize,
    pos: usize,
    ignore_dot: bool,
) -> (TokenType, usize) {
    let remaining = text_len.saturating_sub(pos);
    if remaining == 0 {
        return (TokenType::None, 0);
    }

    let slice = &text[pos..pos + remaining];

    match get_char_type(slice[0]) {
        CharType::Letter | CharType::Digit => {
            let wlen = word_length(slice, ignore_dot);
            (TokenType::Word, wlen)
        }
        CharType::Whitespace => {
            let mut i = 1;
            while i < remaining && get_char_type(slice[i]) == CharType::Whitespace {
                i += 1;
            }
            (TokenType::Whitespace, i)
        }
        CharType::Punctuation => {
            // Hyphen at the start: if followed by a word, treat as word.
            if matches!(slice[0], '-' | '\u{2010}' | '\u{2011}') {
                if remaining == 1 {
                    return (TokenType::Punctuation, 1);
                }
                let wlen = word_length(&slice[1..], ignore_dot);
                if wlen == 0 {
                    return (TokenType::Punctuation, 1);
                }
                return (TokenType::Word, wlen + 1);
            }

            // Ellipsis: three consecutive dots.
            if remaining >= 3 && slice[0] == '.' && slice[1] == '.' && slice[2] == '.' {
                return (TokenType::Punctuation, 3);
            }

            (TokenType::Punctuation, 1)
        }
        CharType::Unknown => (TokenType::Unknown, 1),
    }
}

// ============================================================================
// Sentence detection
// ============================================================================

/// Check whether a word ending with a dot can be interpreted as a single word
/// (i.e. the dot is part of the word, not a sentence-ending period).
///
/// This is a simplified version that recognizes initials (e.g. "K.") and
/// ordinal numbers / dates (e.g. "24." or "1.2.").
///
/// If a `spell_check` callback is provided, it will be called to check for
/// abbreviations.
fn dot_part_of_word(text: &[char], spell_check: SpellCheckFn<'_>) -> bool {
    let len = text.len();
    if len < 2 {
        return false;
    }

    // Initials: single uppercase letter followed by dot (e.g. "K.")
    if len == 2 && mce_core::character::is_upper(text[0]) {
        return true;
    }

    // Ordinal numbers and dates: everything before the trailing dot is
    // digits, dots, or hyphens.
    let mut only_numbers_or_dots = true;
    for &ch in &text[..len - 1] {
        if ch != '.' && ch != '-' && !ch.is_ascii_digit() {
            only_numbers_or_dots = false;
            break;
        }
    }
    if only_numbers_or_dots {
        return true;
    }

    // Abbreviations: use the spell checker if available.
    if let Some(check) = spell_check {
        if check(text) {
            return true;
        }
    }

    false
}

/// Find the next sentence boundary starting at position `pos` in the text.
///
/// Returns `(SentenceType, sentence_length)`. The `sentence_length` measures
/// from `pos` to the end of the sentence (including trailing whitespace up
/// to but not including the next sentence's first token).
///
/// This version uses heuristic-only abbreviation detection (initials and
/// ordinal numbers). Use `next_sentence_with_spell_check` for a custom
/// callback-based abbreviation detection.
pub fn next_sentence(text: &[char], text_len: usize, pos: usize) -> (SentenceType, usize) {
    next_sentence_with_spell_check(text, text_len, pos, None)
}

/// Find the next sentence boundary with an optional spell-checker callback
/// for abbreviation detection.
pub fn next_sentence_with_spell_check(
    text: &[char],
    text_len: usize,
    pos: usize,
    spell_check: SpellCheckFn<'_>,
) -> (SentenceType, usize) {
    let remaining = text_len.saturating_sub(pos);
    if remaining == 0 {
        return (SentenceType::None, 0);
    }

    let slice = &text[pos..pos + remaining];

    let mut slen: usize = 0;
    let mut previous_token_start: usize = 0;
    let mut previous_token_type = TokenType::None;
    let mut end_found = false;
    let mut in_quotation = false;
    let mut end_dotword = false;
    let mut possible_end_punctuation = false;

    loop {
        if slen >= remaining {
            break;
        }

        // Tokenize with ignore_dot = false, so dots are always separate
        // punctuation tokens.
        let (token, tokenlen) = next_token_with_options(slice, remaining, slen, false);

        if token == TokenType::None {
            break;
        }

        if end_found && !in_quotation {
            if token != TokenType::Whitespace {
                // Sentence boundary found. Determine type.
                let stype = if end_dotword
                    || possible_end_punctuation
                    || (previous_token_type != TokenType::Whitespace && token == TokenType::Word)
                {
                    SentenceType::Possible
                } else {
                    SentenceType::Probable
                };
                return (stype, slen);
            }
        } else if token == TokenType::Punctuation {
            let punct = slice[slen];

            if punct == '!' || punct == '?' {
                end_found = true;
                if in_quotation {
                    possible_end_punctuation = true;
                }
            } else if (punct == '.' && tokenlen == 3) || punct == '\u{2026}' {
                // Ellipsis (... or U+2026)
                end_found = true;
                possible_end_punctuation = true;
            } else if punct == '.' {
                end_found = true;
                if slen != 0
                    && previous_token_type == TokenType::Word
                    && dot_part_of_word(&slice[previous_token_start..slen + 1], spell_check)
                {
                    end_dotword = true;
                }
            } else if punct == ':' {
                end_found = true;
                possible_end_punctuation = true;
            } else if is_quotation_mark(punct) || punct == '\u{201C}' {
                in_quotation = !in_quotation;
                if !in_quotation && slen + 1 < remaining && slice[slen + 1] == ',' {
                    // Comma immediately after ending quote suggests the
                    // sentence most likely did not end here.
                    end_found = false;
                    possible_end_punctuation = false;
                }
            }
        }

        previous_token_start = slen;
        previous_token_type = token;
        slen += tokenlen;
    }

    (SentenceType::None, remaining)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Helper ---------------------------------------------------------------

    /// Convenience: convert a &str to Vec<char> and call next_token at pos=0.
    fn tok(s: &str) -> (TokenType, usize) {
        let chars: Vec<char> = s.chars().collect();
        next_token(&chars, chars.len(), 0)
    }

    /// Convenience: tokenize an entire string into (type, text) pairs.
    fn tokenize_all(s: &str) -> Vec<(TokenType, String)> {
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len();
        let mut pos = 0;
        let mut result = Vec::new();
        while pos < len {
            let (tt, tlen) = next_token(&chars, len, pos);
            if tt == TokenType::None {
                break;
            }
            let text: String = chars[pos..pos + tlen].iter().collect();
            result.push((tt, text));
            pos += tlen;
        }
        result
    }

    /// Convenience: find the next sentence boundary for a string.
    fn sent(s: &str) -> (SentenceType, usize) {
        let chars: Vec<char> = s.chars().collect();
        next_sentence(&chars, chars.len(), 0)
    }

    // =========================================================================
    // Tokenizer tests
    // =========================================================================

    // -- Empty and trivial inputs ---

    #[test]
    fn empty_text_returns_none() {
        assert_eq!(tok(""), (TokenType::None, 0));
    }

    #[test]
    fn single_letter() {
        assert_eq!(tok("a"), (TokenType::Word, 1));
    }

    #[test]
    fn single_digit() {
        assert_eq!(tok("5"), (TokenType::Word, 1));
    }

    #[test]
    fn single_space() {
        assert_eq!(tok(" "), (TokenType::Whitespace, 1));
    }

    #[test]
    fn single_punctuation() {
        assert_eq!(tok("."), (TokenType::Punctuation, 1));
    }

    #[test]
    fn single_unknown() {
        assert_eq!(tok("@"), (TokenType::Unknown, 1));
    }

    // -- Simple words ---

    #[test]
    fn simple_word() {
        assert_eq!(tok("koira"), (TokenType::Word, 5));
    }

    #[test]
    fn word_with_finnish_chars() {
        // "äiti" — 4 chars
        assert_eq!(tok("\u{00E4}iti"), (TokenType::Word, 4));
    }

    #[test]
    fn digits_form_word() {
        assert_eq!(tok("123"), (TokenType::Word, 3));
    }

    #[test]
    fn mixed_letters_and_digits() {
        assert_eq!(tok("abc123"), (TokenType::Word, 6));
    }

    // -- Multiple tokens ---

    #[test]
    fn two_words_separated_by_space() {
        let tokens = tokenize_all("koira kissa");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], (TokenType::Word, "koira".to_string()));
        assert_eq!(tokens[1], (TokenType::Whitespace, " ".to_string()));
        assert_eq!(tokens[2], (TokenType::Word, "kissa".to_string()));
    }

    #[test]
    fn word_followed_by_period() {
        let tokens = tokenize_all("koira.");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], (TokenType::Word, "koira".to_string()));
        assert_eq!(tokens[1], (TokenType::Punctuation, ".".to_string()));
    }

    #[test]
    fn word_with_comma() {
        let tokens = tokenize_all("koira,");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], (TokenType::Word, "koira".to_string()));
        assert_eq!(tokens[1], (TokenType::Punctuation, ",".to_string()));
    }

    // -- Whitespace sequences ---

    #[test]
    fn multiple_whitespace_chars() {
        let tokens = tokenize_all("a  b");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], (TokenType::Word, "a".to_string()));
        assert_eq!(tokens[1], (TokenType::Whitespace, "  ".to_string()));
        assert_eq!(tokens[2], (TokenType::Word, "b".to_string()));
    }

    #[test]
    fn only_whitespace() {
        let tokens = tokenize_all("   ");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], (TokenType::Whitespace, "   ".to_string()));
    }

    // -- Hyphen within words ---

    #[test]
    fn hyphen_within_word() {
        // "abc-def" should be one word.
        assert_eq!(tok("abc-def"), (TokenType::Word, 7));
    }

    #[test]
    fn hyphen_at_start_followed_by_word() {
        // "-koira" → Word (hyphen + letters = one word token)
        assert_eq!(tok("-koira"), (TokenType::Word, 6));
    }

    #[test]
    fn hyphen_at_start_alone() {
        assert_eq!(tok("-"), (TokenType::Punctuation, 1));
    }

    #[test]
    fn hyphen_at_start_followed_by_space() {
        let tokens = tokenize_all("- koira");
        assert_eq!(tokens[0], (TokenType::Punctuation, "-".to_string()));
    }

    #[test]
    fn soft_hyphen_within_word() {
        // "koi\u{00AD}ra" — soft hyphen is part of the word.
        // This is 6 characters: k, o, i, U+00AD, r, a.
        assert_eq!(tok("koi\u{00AD}ra"), (TokenType::Word, 6));
    }

    #[test]
    fn unicode_hyphen_within_word() {
        // HYPHEN (U+2010) within a word
        assert_eq!(tok("abc\u{2010}def"), (TokenType::Word, 7));
    }

    #[test]
    fn non_breaking_hyphen_within_word() {
        // NON-BREAKING HYPHEN (U+2011) within a word
        assert_eq!(tok("abc\u{2011}def"), (TokenType::Word, 7));
    }

    #[test]
    fn trailing_hyphen_at_end_of_text() {
        // "koira-" at end of text: hyphen is included in the word.
        assert_eq!(tok("koira-"), (TokenType::Word, 6));
    }

    #[test]
    fn trailing_hyphen_before_space() {
        // "koira- " — hyphen included, space is separate.
        let tokens = tokenize_all("koira- ");
        assert_eq!(tokens[0], (TokenType::Word, "koira-".to_string()));
        assert_eq!(tokens[1], (TokenType::Whitespace, " ".to_string()));
    }

    #[test]
    fn hyphen_before_comma() {
        // "koira-," — hyphen included but comma is not.
        let tokens = tokenize_all("koira-,");
        assert_eq!(tokens[0], (TokenType::Word, "koira-".to_string()));
        assert_eq!(tokens[1], (TokenType::Punctuation, ",".to_string()));
    }

    #[test]
    fn hyphen_before_finnish_quotation() {
        // Hyphen followed by a Finnish quotation mark: hyphen included in word.
        let tokens = tokenize_all("koira-\u{201D}");
        assert_eq!(tokens[0], (TokenType::Word, "koira-".to_string()));
        assert_eq!(tokens[1], (TokenType::Punctuation, "\u{201D}".to_string()));
    }

    // -- Apostrophe and colon in words ---

    #[test]
    fn apostrophe_within_word() {
        // "it's" — apostrophe followed by letter stays in word.
        assert_eq!(tok("it's"), (TokenType::Word, 4));
    }

    #[test]
    fn apostrophe_at_end() {
        // "it'" — apostrophe at end is not part of the word.
        let tokens = tokenize_all("it'");
        assert_eq!(tokens[0], (TokenType::Word, "it".to_string()));
        assert_eq!(tokens[1], (TokenType::Punctuation, "'".to_string()));
    }

    #[test]
    fn right_single_quotation_mark_within_word() {
        // U+2019 RIGHT SINGLE QUOTATION MARK acts like apostrophe.
        assert_eq!(tok("it\u{2019}s"), (TokenType::Word, 4));
    }

    #[test]
    fn colon_within_word() {
        // "klo:12" — colon not followed by letter, so colon ends the word.
        // Actually: "klo" then ":" then "12"
        let tokens = tokenize_all("klo:12");
        assert_eq!(tokens[0], (TokenType::Word, "klo".to_string()));
        assert_eq!(tokens[1], (TokenType::Punctuation, ":".to_string()));
        assert_eq!(tokens[2], (TokenType::Word, "12".to_string()));
    }

    #[test]
    fn colon_followed_by_letter() {
        // "a:b" — colon followed by letter stays in word.
        assert_eq!(tok("a:b"), (TokenType::Word, 3));
    }

    // -- Dots in words ---

    #[test]
    fn dot_followed_by_letter() {
        // "e.g" — dot followed by letter stays in word.
        assert_eq!(tok("e.g"), (TokenType::Word, 3));
    }

    #[test]
    fn dot_in_number() {
        // "1.2.3" — dots between digits (no letters seen) form one token.
        assert_eq!(tok("1.2.3"), (TokenType::Word, 5));
    }

    #[test]
    fn dot_after_letters_before_digit() {
        // "abc.1" — after letters, dot before digit ends the word.
        let tokens = tokenize_all("abc.1");
        assert_eq!(tokens[0], (TokenType::Word, "abc".to_string()));
        assert_eq!(tokens[1], (TokenType::Punctuation, ".".to_string()));
        assert_eq!(tokens[2], (TokenType::Word, "1".to_string()));
    }

    #[test]
    fn dot_at_end_of_text_not_in_word() {
        // "koira." — dot at end is not part of the word (ignore_dot=false).
        let tokens = tokenize_all("koira.");
        assert_eq!(tokens[0], (TokenType::Word, "koira".to_string()));
        assert_eq!(tokens[1], (TokenType::Punctuation, ".".to_string()));
    }

    #[test]
    fn dot_at_end_with_ignore_dot() {
        // With ignore_dot=true, trailing dot is included in word.
        let chars: Vec<char> = "koira.".chars().collect();
        let (tt, tlen) = next_token_with_options(&chars, chars.len(), 0, true);
        assert_eq!(tt, TokenType::Word);
        assert_eq!(tlen, 6);
    }

    // -- Comma in numbers ---

    #[test]
    fn comma_in_number() {
        // "1,234" — comma between digits in number context.
        assert_eq!(tok("1,234"), (TokenType::Word, 5));
    }

    #[test]
    fn comma_not_in_number() {
        // "abc,def" — comma after letters ends the word.
        let tokens = tokenize_all("abc,def");
        assert_eq!(tokens[0], (TokenType::Word, "abc".to_string()));
        assert_eq!(tokens[1], (TokenType::Punctuation, ",".to_string()));
        assert_eq!(tokens[2], (TokenType::Word, "def".to_string()));
    }

    // -- Ellipsis ---

    #[test]
    fn ellipsis_three_dots() {
        let tokens = tokenize_all("koira...");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], (TokenType::Word, "koira".to_string()));
        assert_eq!(tokens[1], (TokenType::Punctuation, "...".to_string()));
    }

    #[test]
    fn two_dots_not_ellipsis() {
        // Two dots: first is one punctuation, second is another.
        let tokens = tokenize_all("..");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], (TokenType::Punctuation, ".".to_string()));
        assert_eq!(tokens[1], (TokenType::Punctuation, ".".to_string()));
    }

    // -- Unknown characters ---

    #[test]
    fn at_sign_is_unknown() {
        let tokens = tokenize_all("a@b");
        assert_eq!(tokens[0], (TokenType::Word, "a".to_string()));
        assert_eq!(tokens[1], (TokenType::Unknown, "@".to_string()));
        assert_eq!(tokens[2], (TokenType::Word, "b".to_string()));
    }

    #[test]
    fn hash_is_unknown() {
        assert_eq!(tok("#"), (TokenType::Unknown, 1));
    }

    // -- URL detection ---

    #[test]
    fn http_url_basic() {
        assert_eq!(tok("http://example.com"), (TokenType::Word, 18));
    }

    #[test]
    fn https_url_basic() {
        assert_eq!(tok("https://example.com"), (TokenType::Word, 19));
    }

    #[test]
    fn http_url_with_path() {
        // "http://example.com/path" is 23 characters.
        assert_eq!(tok("http://example.com/path"), (TokenType::Word, 23));
    }

    #[test]
    fn http_url_ending_with_dot_before_space() {
        // Trailing dot before space is NOT part of URL.
        let tokens = tokenize_all("http://example.com. next");
        assert_eq!(
            tokens[0],
            (TokenType::Word, "http://example.com".to_string())
        );
        assert_eq!(tokens[1], (TokenType::Punctuation, ".".to_string()));
    }

    #[test]
    fn http_url_ending_at_eof_with_dot() {
        // Trailing dot at end of text is NOT part of URL.
        let tokens = tokenize_all("http://example.com.");
        assert_eq!(
            tokens[0],
            (TokenType::Word, "http://example.com".to_string())
        );
        assert_eq!(tokens[1], (TokenType::Punctuation, ".".to_string()));
    }

    #[test]
    fn http_url_in_sentence() {
        let tokens = tokenize_all("see http://example.com here");
        assert_eq!(tokens[0], (TokenType::Word, "see".to_string()));
        assert_eq!(tokens[1], (TokenType::Whitespace, " ".to_string()));
        assert_eq!(
            tokens[2],
            (TokenType::Word, "http://example.com".to_string())
        );
    }

    #[test]
    fn too_short_for_url() {
        // "http://a" is only 8 chars — too short for URL (< 12).
        // It should not be recognized as URL but as separate tokens.
        let tokens = tokenize_all("http://a");
        assert!(tokens.len() > 1); // Not a single word token
    }

    // -- Email detection ---

    #[test]
    fn simple_email() {
        assert_eq!(tok("foo@bar.com"), (TokenType::Word, 11));
    }

    #[test]
    fn email_in_sentence() {
        let tokens = tokenize_all("send foo@bar.com mail");
        assert_eq!(tokens[0], (TokenType::Word, "send".to_string()));
        assert_eq!(tokens[1], (TokenType::Whitespace, " ".to_string()));
        assert_eq!(tokens[2], (TokenType::Word, "foo@bar.com".to_string()));
    }

    #[test]
    fn email_ending_with_dot_before_space() {
        // "foo@bar.com. " — trailing dot is not part of email.
        let tokens = tokenize_all("foo@bar.com. next");
        assert_eq!(tokens[0], (TokenType::Word, "foo@bar.com".to_string()));
        assert_eq!(tokens[1], (TokenType::Punctuation, ".".to_string()));
    }

    #[test]
    fn not_email_no_at() {
        // No @ sign, not an email.
        let tokens = tokenize_all("foo.bar.com");
        assert_eq!(tokens[0], (TokenType::Word, "foo.bar.com".to_string()));
    }

    #[test]
    fn not_email_no_dot_after_at() {
        // @ but no dot after it — not a complete email.
        let tokens = tokenize_all("foo@bar ");
        // This will be split at @ since no dot was found after @.
        assert_eq!(tokens[0], (TokenType::Word, "foo".to_string()));
    }

    #[test]
    fn double_at_not_email() {
        // Two @ signs — not a valid email.
        let tokens = tokenize_all("foo@@bar.com");
        assert_eq!(tokens[0], (TokenType::Word, "foo".to_string()));
    }

    #[test]
    fn too_short_for_email() {
        // Less than 6 chars cannot be an email.
        let tokens = tokenize_all("a@b.c");
        // 5 chars — too short for email detection.
        assert_eq!(tokens[0], (TokenType::Word, "a".to_string()));
    }

    // -- Finnish quotation marks ---

    #[test]
    fn finnish_quotation_mark_is_punctuation() {
        assert_eq!(tok("\u{00BB}"), (TokenType::Punctuation, 1)); // >>
        assert_eq!(tok("\u{201D}"), (TokenType::Punctuation, 1)); // "
        assert_eq!(tok("\""), (TokenType::Punctuation, 1)); // "
    }

    // -- En-dash and em-dash ---

    #[test]
    fn en_dash_is_punctuation() {
        assert_eq!(tok("\u{2013}"), (TokenType::Punctuation, 1));
    }

    #[test]
    fn em_dash_is_punctuation() {
        assert_eq!(tok("\u{2014}"), (TokenType::Punctuation, 1));
    }

    // -- Unicode HYPHEN (U+2010) at start ---

    #[test]
    fn unicode_hyphen_at_start_with_word() {
        assert_eq!(tok("\u{2010}koira"), (TokenType::Word, 6));
    }

    #[test]
    fn non_breaking_hyphen_at_start_with_word() {
        assert_eq!(tok("\u{2011}koira"), (TokenType::Word, 6));
    }

    // -- Horizontal ellipsis ---

    #[test]
    fn horizontal_ellipsis_char() {
        // U+2026 HORIZONTAL ELLIPSIS
        assert_eq!(tok("\u{2026}"), (TokenType::Punctuation, 1));
    }

    // -- Left double quotation mark ---

    #[test]
    fn left_double_quotation_mark() {
        // U+201C LEFT DOUBLE QUOTATION MARK — classified as punctuation.
        assert_eq!(tok("\u{201C}"), (TokenType::Punctuation, 1));
    }

    // =========================================================================
    // Sentence detection tests
    // =========================================================================

    #[test]
    fn empty_text_sentence_none() {
        assert_eq!(sent(""), (SentenceType::None, 0));
    }

    #[test]
    fn no_sentence_ending() {
        // Text without sentence-ending punctuation.
        let (stype, slen) = sent("koira kissa");
        assert_eq!(stype, SentenceType::None);
        assert_eq!(slen, 11);
    }

    #[test]
    fn simple_sentence_with_period() {
        // "Koira juoksi. Kissa nukkui."
        // The first sentence ends at the period. The boundary is after the
        // whitespace, just before "Kissa".
        let s = "Koira juoksi. Kissa nukkui.";
        let chars: Vec<char> = s.chars().collect();
        let (stype, slen) = next_sentence(&chars, chars.len(), 0);
        assert_eq!(stype, SentenceType::Probable);
        // sentence_len should point to just before "Kissa"
        let sentence_text: String = chars[..slen].iter().collect();
        assert_eq!(sentence_text, "Koira juoksi. ");
    }

    #[test]
    fn sentence_with_exclamation() {
        let s = "Hei! Miten menee?";
        let chars: Vec<char> = s.chars().collect();
        let (stype, slen) = next_sentence(&chars, chars.len(), 0);
        assert_eq!(stype, SentenceType::Probable);
        let sentence_text: String = chars[..slen].iter().collect();
        assert_eq!(sentence_text, "Hei! ");
    }

    #[test]
    fn sentence_with_question_mark() {
        let s = "Miten menee? Hyvin.";
        let chars: Vec<char> = s.chars().collect();
        let (stype, slen) = next_sentence(&chars, chars.len(), 0);
        assert_eq!(stype, SentenceType::Probable);
        let sentence_text: String = chars[..slen].iter().collect();
        assert_eq!(sentence_text, "Miten menee? ");
    }

    #[test]
    fn sentence_with_ellipsis() {
        // Ellipsis (three dots) produces a Possible sentence boundary.
        let s = "Koira... Kissa.";
        let chars: Vec<char> = s.chars().collect();
        let (stype, slen) = next_sentence(&chars, chars.len(), 0);
        assert_eq!(stype, SentenceType::Possible);
        let sentence_text: String = chars[..slen].iter().collect();
        assert_eq!(sentence_text, "Koira... ");
    }

    #[test]
    fn sentence_with_colon() {
        // Colon produces a Possible sentence boundary.
        let s = "Huom: kissa.";
        let chars: Vec<char> = s.chars().collect();
        let (stype, slen) = next_sentence(&chars, chars.len(), 0);
        assert_eq!(stype, SentenceType::Possible);
        let sentence_text: String = chars[..slen].iter().collect();
        assert_eq!(sentence_text, "Huom: ");
    }

    #[test]
    fn ordinal_number_not_sentence_end() {
        // "24. joulukuuta" — the dot after "24" is part of the ordinal number,
        // so it produces a Possible boundary (dotword), not Probable.
        let s = "24. joulukuuta oli.";
        let chars: Vec<char> = s.chars().collect();
        let (stype, slen) = next_sentence(&chars, chars.len(), 0);
        assert_eq!(stype, SentenceType::Possible);
        let sentence_text: String = chars[..slen].iter().collect();
        assert_eq!(sentence_text, "24. ");
    }

    #[test]
    fn initial_not_sentence_end() {
        // "K. Korhonen" — the dot after "K" is part of an initial,
        // so it produces a Possible boundary (dotword).
        let s = "K. Korhonen meni.";
        let chars: Vec<char> = s.chars().collect();
        let (stype, slen) = next_sentence(&chars, chars.len(), 0);
        assert_eq!(stype, SentenceType::Possible);
        let sentence_text: String = chars[..slen].iter().collect();
        assert_eq!(sentence_text, "K. ");
    }

    #[test]
    fn single_sentence_no_boundary() {
        // Only one sentence with period at end but no following text.
        let s = "Koira juoksi.";
        let (stype, slen) = sent(s);
        assert_eq!(stype, SentenceType::None);
        assert_eq!(slen, 13);
    }

    #[test]
    fn sentence_with_quotation_marks() {
        // Quotation marks: the ! inside quotes sets end_found and
        // possible_end_punctuation while in_quotation is true. When the
        // closing quote toggles in_quotation off, end_found is still true.
        // After the whitespace following the closing quote, the next
        // non-whitespace token triggers a Possible sentence boundary
        // (because possible_end_punctuation is true).
        let s = "H\u{00E4}n sanoi \"kyll\u{00E4}!\" ja l\u{00E4}hti.";
        let chars: Vec<char> = s.chars().collect();
        let (stype, _slen) = next_sentence(&chars, chars.len(), 0);
        assert_eq!(stype, SentenceType::Possible);
    }

    #[test]
    fn quotation_followed_by_comma_not_sentence_end() {
        // Comma after closing quote means the sentence didn't end.
        let s = "H\u{00E4}n sanoi \"kyll\u{00E4}!\", ja l\u{00E4}hti. Sitten.";
        let chars: Vec<char> = s.chars().collect();
        let (stype, slen) = next_sentence(&chars, chars.len(), 0);
        // The period after "lahti" should be the sentence end.
        assert_eq!(stype, SentenceType::Probable);
        let sentence_text: String = chars[..slen].iter().collect();
        assert!(sentence_text.contains("l\u{00E4}hti."));
    }

    #[test]
    fn consecutive_sentences() {
        // Parse two consecutive sentences.
        let s = "Ensimm\u{00E4}inen. Toinen.";
        let chars: Vec<char> = s.chars().collect();

        let (stype1, slen1) = next_sentence(&chars, chars.len(), 0);
        assert_eq!(stype1, SentenceType::Probable);

        // Parse the second sentence.
        let (stype2, slen2) = next_sentence(&chars, chars.len(), slen1);
        // Second sentence ends at end of text, so None.
        assert_eq!(stype2, SentenceType::None);
        assert_eq!(slen2, chars.len() - slen1);
    }

    #[test]
    fn no_space_between_sentences_none() {
        // "Koira.Kissa" — the tokenizer treats "Koira.Kissa" as a single
        // word token because the dot is followed by a letter. So no
        // sentence boundary is detected.
        let s = "Koira.Kissa";
        let chars: Vec<char> = s.chars().collect();
        let (stype, slen) = next_sentence(&chars, chars.len(), 0);
        assert_eq!(stype, SentenceType::None);
        assert_eq!(slen, 11);
    }

    #[test]
    fn unicode_ellipsis_sentence_boundary() {
        // U+2026 HORIZONTAL ELLIPSIS creates a Possible boundary.
        let s = "Koira\u{2026} Kissa";
        let chars: Vec<char> = s.chars().collect();
        let (stype, _slen) = next_sentence(&chars, chars.len(), 0);
        assert_eq!(stype, SentenceType::Possible);
    }

    // =========================================================================
    // Integration: tokenize then sentence-detect
    // =========================================================================

    #[test]
    fn full_paragraph_tokenization() {
        let s = "Koira juoksi nopeasti. Kissa nukkui rauhassa!";
        let tokens = tokenize_all(s);

        // Verify word tokens.
        let words: Vec<&str> = tokens
            .iter()
            .filter(|(tt, _)| *tt == TokenType::Word)
            .map(|(_, text)| text.as_str())
            .collect();
        assert_eq!(
            words,
            &["Koira", "juoksi", "nopeasti", "Kissa", "nukkui", "rauhassa"]
        );

        // Verify punctuation tokens.
        let punct: Vec<&str> = tokens
            .iter()
            .filter(|(tt, _)| *tt == TokenType::Punctuation)
            .map(|(_, text)| text.as_str())
            .collect();
        assert_eq!(punct, &[".", "!"]);
    }

    #[test]
    fn url_in_sentence_tokenization() {
        let s = "Visit http://example.com/page for info.";
        let tokens = tokenize_all(s);
        let words: Vec<&str> = tokens
            .iter()
            .filter(|(tt, _)| *tt == TokenType::Word)
            .map(|(_, text)| text.as_str())
            .collect();
        assert!(words.contains(&"http://example.com/page"));
    }

    #[test]
    fn email_in_sentence_tokenization() {
        let s = "Email foo@bar.com for info.";
        let tokens = tokenize_all(s);
        let words: Vec<&str> = tokens
            .iter()
            .filter(|(tt, _)| *tt == TokenType::Word)
            .map(|(_, text)| text.as_str())
            .collect();
        assert!(words.contains(&"foo@bar.com"));
    }

    // =========================================================================
    // Edge cases in word_length
    // =========================================================================

    #[test]
    fn word_length_url_trumps_normal() {
        // URL detection takes priority over normal word scanning.
        // "http://example.com/test" is 23 characters.
        let chars: Vec<char> = "http://example.com/test".chars().collect();
        let wlen = word_length(&chars, false);
        assert_eq!(wlen, 23);
    }

    #[test]
    fn word_length_dot_between_digits() {
        // "192.168.1.1" — dots between digits form one word (IP address).
        let chars: Vec<char> = "192.168.1.1".chars().collect();
        let wlen = word_length(&chars, false);
        assert_eq!(wlen, 11);
    }

    #[test]
    fn number_with_comma_and_dot() {
        // "1,234.56" — comma in number context, then dot with digit.
        let chars: Vec<char> = "1,234.56".chars().collect();
        let wlen = word_length(&chars, false);
        // "1,234" is one token (comma in number context), then ".56" —
        // but after comma, processing_number is true, then digits continue,
        // then dot before digit with no letters seen: continues.
        assert_eq!(wlen, 8);
    }

    // =========================================================================
    // Regression / boundary tests
    // =========================================================================

    #[test]
    fn pos_beyond_text_returns_none() {
        let chars: Vec<char> = "abc".chars().collect();
        let (tt, tlen) = next_token(&chars, chars.len(), 5);
        assert_eq!(tt, TokenType::None);
        assert_eq!(tlen, 0);
    }

    #[test]
    fn pos_at_end_returns_none() {
        let chars: Vec<char> = "abc".chars().collect();
        let (tt, tlen) = next_token(&chars, chars.len(), 3);
        assert_eq!(tt, TokenType::None);
        assert_eq!(tlen, 0);
    }

    #[test]
    fn sentence_from_middle() {
        // Sentence detection starting from a position in the middle.
        let s = "Ensimm\u{00E4}inen. Toinen. Kolmas.";
        let chars: Vec<char> = s.chars().collect();

        let (_, slen1) = next_sentence(&chars, chars.len(), 0);
        let (stype2, slen2) = next_sentence(&chars, chars.len(), slen1);
        assert_eq!(stype2, SentenceType::Probable);

        let sentence_text: String = chars[slen1..slen1 + slen2].iter().collect();
        assert_eq!(sentence_text, "Toinen. ");
    }

    #[test]
    fn complex_sentence_with_mixed_punctuation() {
        let s = "Hei! Miten menee? Hyvin, kiitos. Ja sinulla?";
        let chars: Vec<char> = s.chars().collect();
        let mut pos = 0;
        let mut sentence_count = 0;
        while pos < chars.len() {
            let (stype, slen) = next_sentence(&chars, chars.len(), pos);
            if stype == SentenceType::None {
                break;
            }
            sentence_count += 1;
            pos += slen;
        }
        // We should find at least 3 sentence boundaries (Hei!, Miten menee?, Hyvin kiitos.)
        assert!(sentence_count >= 3);
    }

    // =========================================================================
    // Spell-check callback tests
    // =========================================================================

    #[test]
    fn spell_check_callback_abbreviation_detection() {
        // Without spell checker: "esim. talo" splits at "esim." because
        // it doesn't match the heuristic checks (not an initial, not a number).
        let s = "esim. talo on korkea.";
        let chars: Vec<char> = s.chars().collect();
        let (stype_no_spell, slen_no_spell) = next_sentence(&chars, chars.len(), 0);
        // Without speller: "esim." is followed by " " then "talo" => Probable
        assert_eq!(stype_no_spell, SentenceType::Probable);
        let first_text: String = chars[..slen_no_spell].iter().collect();
        assert_eq!(first_text, "esim. ");

        // With spell checker: "esim." is a known abbreviation, so the dot
        // is part of the word and the sentence boundary is Possible (dotword).
        let check_fn = |word: &[char]| -> bool {
            let s: String = word.iter().collect();
            // "esim." is a known abbreviation
            s == "esim."
        };
        let (stype_with_spell, _) =
            next_sentence_with_spell_check(&chars, chars.len(), 0, Some(&check_fn));
        assert_eq!(stype_with_spell, SentenceType::Possible);
    }

    #[test]
    fn spell_check_callback_non_abbreviation() {
        // "koira." is not a known abbreviation, so speller returns false
        // and the dot is treated as sentence-ending.
        let s = "koira. kissa.";
        let chars: Vec<char> = s.chars().collect();
        let check_fn = |_word: &[char]| -> bool { false };
        let (stype, slen) = next_sentence_with_spell_check(&chars, chars.len(), 0, Some(&check_fn));
        assert_eq!(stype, SentenceType::Probable);
        let first_text: String = chars[..slen].iter().collect();
        assert_eq!(first_text, "koira. ");
    }
}
