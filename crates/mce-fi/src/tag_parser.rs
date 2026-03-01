// Adapted from corevoikko (voikko-fi/morphology/tag_parser.rs)
//
// Pure functions for parsing FST output tags.
//
// The Finnish VFST transducer produces output strings containing bracketed tags
// interleaved with surface characters, e.g.:
//   [Ln][Xp]koira[X]koira[Sn][Ny]
//
// This module extracts structured morphological information from these strings
// without requiring an FST transducer, making the functions unit-testable.

use mce_core::character::{simple_lower, simple_upper};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum output buffer size. Matches C++ BUFFER_SIZE.
pub(crate) const BUFFER_SIZE: usize = 2000;

/// Maximum number of analyses to collect per word.
pub(crate) const MAX_ANALYSIS_COUNT: usize = 100;

// ---------------------------------------------------------------------------
// Attribute value maps (tag code -> Finnish morphological term)
// ---------------------------------------------------------------------------

/// Look up a word class from its short FST code.
pub(crate) fn lookup_class(code: &str) -> Option<&'static str> {
    match code {
        "n" => Some("nimisana"),
        "l" => Some("laatusana"),
        "nl" => Some("nimisana_laatusana"),
        "h" => Some("huudahdussana"),
        "ee" => Some("etunimi"),
        "es" => Some("sukunimi"),
        "ep" => Some("paikannimi"),
        "em" => Some("nimi"),
        "t" => Some("teonsana"),
        "a" => Some("lyhenne"),
        "s" => Some("seikkasana"),
        "u" | "ur" => Some("lukusana"),
        "r" => Some("asemosana"),
        "c" => Some("sidesana"),
        "d" => Some("suhdesana"),
        "k" => Some("kieltosana"),
        "p" => Some("etuliite"),
        _ => None,
    }
}

/// Look up a case (sijamuoto) from its short FST code.
pub(crate) fn lookup_sijamuoto(code: &str) -> Option<&'static str> {
    match code {
        "n" => Some("nimento"),
        "g" => Some("omanto"),
        "p" => Some("osanto"),
        "es" => Some("olento"),
        "tr" => Some("tulento"),
        "ine" => Some("sisaolento"),
        "ela" => Some("sisaeronto"),
        "ill" => Some("sisatulento"),
        "ade" => Some("ulkoolento"),
        "abl" => Some("ulkoeronto"),
        "all" => Some("ulkotulento"),
        "ab" => Some("vajanto"),
        "ko" => Some("seuranto"),
        "in" => Some("keinonto"),
        "sti" => Some("kerrontosti"),
        "ak" => Some("kohdanto"),
        _ => None,
    }
}

/// Look up a comparison degree.
pub(crate) fn lookup_comparison(code: &str) -> Option<&'static str> {
    match code {
        "c" => Some("comparative"),
        "s" => Some("superlative"),
        _ => None,
    }
}

/// Look up a verb mood.
pub(crate) fn lookup_mood(code: &str) -> Option<&'static str> {
    match code {
        "n1" => Some("A-infinitive"),
        "n2" => Some("E-infinitive"),
        "n3" => Some("MA-infinitive"),
        "n4" => Some("MINEN-infinitive"),
        "n5" => Some("MAINEN-infinitive"),
        "t" => Some("indicative"),
        "e" => Some("conditional"),
        "k" => Some("imperative"),
        "m" => Some("potential"),
        _ => None,
    }
}

/// Look up a number (singular/plural).
pub(crate) fn lookup_number(code: &str) -> Option<&'static str> {
    match code {
        "y" => Some("singular"),
        "m" => Some("plural"),
        _ => None,
    }
}

/// Look up a person.
pub(crate) fn lookup_person(code: &str) -> Option<&'static str> {
    match code {
        "1" => Some("1"),
        "2" => Some("2"),
        "3" => Some("3"),
        "4" => Some("4"),
        _ => None,
    }
}

/// Look up a tense.
pub(crate) fn lookup_tense(code: &str) -> Option<&'static str> {
    match code {
        "p" => Some("present_simple"),
        "i" => Some("past_imperfective"),
        _ => None,
    }
}

/// Look up a focus particle.
pub(crate) fn lookup_focus(code: &str) -> Option<&'static str> {
    match code {
        "kin" => Some("kin"),
        "kaan" => Some("kaan"),
        _ => None,
    }
}

/// Look up a possessive suffix.
pub(crate) fn lookup_possessive(code: &str) -> Option<&'static str> {
    match code {
        "1y" => Some("1s"),
        "2y" => Some("2s"),
        "1m" => Some("1p"),
        "2m" => Some("2p"),
        "3" => Some("3"),
        _ => None,
    }
}

/// Look up a negative value.
pub(crate) fn lookup_negative(code: &str) -> Option<&'static str> {
    match code {
        "t" => Some("true"),
        "f" => Some("false"),
        "b" => Some("both"),
        _ => None,
    }
}

/// Look up a participle type.
pub(crate) fn lookup_participle(code: &str) -> Option<&'static str> {
    match code {
        "v" => Some("present_active"),
        "a" => Some("present_passive"),
        "u" => Some("past_active"),
        "t" => Some("past_passive"),
        "m" => Some("agent"),
        "e" => Some("negation"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helper: extract a tag code between positions j+2..i in fst_output
// ---------------------------------------------------------------------------

fn extract_tag_code(fst_output: &[char], tag_start: usize, tag_end: usize) -> String {
    fst_output[tag_start + 2..tag_end].iter().collect()
}

// ---------------------------------------------------------------------------
// parse_structure
// ---------------------------------------------------------------------------

/// Build the STRUCTURE attribute string from FST output.
///
/// The STRUCTURE string encodes the expected case for each character position:
/// - `=` -- compound boundary
/// - `i` / `j` -- uppercase expected (j for abbreviations)
/// - `p` / `q` -- lowercase expected (q for abbreviations)
/// - `-` -- literal hyphen
/// - `:` -- literal colon
pub(crate) fn parse_structure(fst_output: &[char], wlen: usize) -> String {
    let output_len = fst_output.len();
    let mut structure: Vec<char> = Vec::with_capacity(wlen * 2 + 1);
    structure.push('=');

    let mut chars_missing = wlen;
    let mut chars_seen: usize = 0;
    let mut chars_from_default: usize = 0;
    let mut default_title_case = false;
    let mut is_abbr = false;

    let mut i = 0;
    while i < output_len {
        if fst_output[i] == '[' && i + 2 < output_len {
            // Check for [Bx] boundary tags (not [Bh])
            if i + 3 < output_len
                && fst_output[i + 1] == 'B'
                && fst_output[i + 2] != 'h'
                && fst_output[i + 3] == ']'
            {
                if i == 1 {
                    structure.push('=');
                }
                if chars_seen > chars_from_default {
                    create_default_structure(
                        chars_seen - chars_from_default,
                        &mut default_title_case,
                        &mut structure,
                        is_abbr,
                    );
                    decrease_chars_missing(&mut chars_missing, chars_seen, chars_from_default);
                }
                if i != 1
                    && i + 5 < output_len
                    && !structure.is_empty()
                    && *structure.last().unwrap() != '='
                {
                    structure.push('=');
                }
                i += 3;
                chars_seen = 0;
                chars_from_default = 0;
            }
            // Check for [Xx] tags
            else if i + 3 < output_len && fst_output[i + 1] == 'X' && fst_output[i + 3] == ']' {
                if fst_output[i + 2] == 'r' {
                    // [Xr]...[X] -- explicit structure override
                    default_title_case = false;
                    i += 4;
                    while i < output_len && fst_output[i] != '[' && chars_missing > 0 {
                        structure.push(fst_output[i]);
                        if fst_output[i] != '=' {
                            chars_from_default += 1;
                            if fst_output[i] != '-' {
                                chars_missing -= 1;
                            }
                        }
                        i += 1;
                    }
                    i += 2; // skip X]
                } else {
                    // [Xp]...[X] or [Xs]...[X] or [Xj]...[X] -- skip content
                    i += 4;
                    while i < output_len && fst_output[i] != '[' {
                        i += 1;
                    }
                    i += 2; // skip X]
                }
            }
            // Check for [Lx...] class tags
            else if i + 3 < output_len && fst_output[i + 1] == 'L' {
                if fst_output[i + 2] == 'e' {
                    default_title_case = true;
                    is_abbr = false;
                    i += 4;
                } else if fst_output[i + 2] == 'n' && fst_output[i + 3] == 'l' {
                    is_abbr = false;
                    i += 4;
                } else if fst_output[i + 2] == 'a' {
                    is_abbr = true;
                    i += 3;
                } else if fst_output[i + 2] == 'u'
                    && i + 5 < output_len
                    && (fst_output[i + 3] == 'r' || fst_output[i + 4].is_ascii_digit())
                {
                    is_abbr = true;
                    i += 3;
                    if i < output_len && fst_output[i] == 'r' {
                        i += 1;
                    }
                } else {
                    is_abbr = false;
                    i += 3;
                }
            } else {
                // Skip any other tag entirely
                while i < output_len && fst_output[i] != ']' {
                    i += 1;
                }
            }
        } else if fst_output[i] == '-' {
            if chars_seen > chars_from_default {
                create_default_structure(
                    chars_seen - chars_from_default,
                    &mut default_title_case,
                    &mut structure,
                    is_abbr,
                );
                decrease_chars_missing(&mut chars_missing, chars_seen, chars_from_default);
                structure.push('-');
                chars_seen = 0;
                chars_from_default = 0;
            } else if i != 0 {
                if chars_seen == chars_from_default {
                    structure.push('-');
                } else {
                    chars_seen += 1;
                }
            }
            chars_missing = chars_missing.saturating_sub(1);
            if structure.len() == 1 {
                structure[0] = '-';
            }
        } else if fst_output[i] == ':' {
            if is_abbr {
                if chars_seen > chars_from_default {
                    create_default_structure(
                        chars_seen - chars_from_default,
                        &mut default_title_case,
                        &mut structure,
                        is_abbr,
                    );
                    decrease_chars_missing(&mut chars_missing, chars_seen, chars_from_default);
                    chars_seen = 0;
                    chars_from_default = 0;
                }
                is_abbr = false;
            }
            structure.push(':');
            chars_missing = chars_missing.saturating_sub(1);
        } else {
            chars_seen += 1;
        }
        i += 1;
    }

    // Fill remaining chars
    create_default_structure(
        chars_missing,
        &mut default_title_case,
        &mut structure,
        is_abbr,
    );

    structure.iter().collect()
}

fn create_default_structure(
    count: usize,
    default_title_case: &mut bool,
    structure: &mut Vec<char>,
    is_abbr: bool,
) {
    for _ in 0..count {
        if *default_title_case {
            structure.push(if is_abbr { 'j' } else { 'i' });
            *default_title_case = false;
        } else {
            structure.push(if is_abbr { 'q' } else { 'p' });
        }
    }
}

fn decrease_chars_missing(chars_missing: &mut usize, chars_seen: usize, chars_from_default: usize) {
    let consumed = chars_seen.saturating_sub(chars_from_default);
    if consumed <= *chars_missing {
        *chars_missing -= consumed;
    } else {
        *chars_missing = 0;
    }
}

// ---------------------------------------------------------------------------
// is_valid_analysis
// ---------------------------------------------------------------------------

/// Validate compound word structure in FST output.
pub(crate) fn is_valid_analysis(fst_output: &[char]) -> bool {
    let len = fst_output.len();
    let mut before_last_char: char = '\0';
    let mut last_char: char = '\0';
    let mut boundary_passed = false;
    let mut hyphen_present = false;
    let mut hyphen_unconditionally_allowed = false;
    let mut hyphen_unconditionally_allowed_just_set = false;
    let mut hyphen_required = false;
    let mut required_hyphen_missing = false;
    let mut starts_with_proper_noun = false;
    let mut ends_with_non_ica_noun = false;

    let mut i = 0;
    while i < len {
        if fst_output[i] == '[' {
            if i + 2 >= len {
                return false;
            }
            if i + 3 < len {
                if fst_output[i + 1] == 'I' {
                    if starts_with(fst_output, i + 2, "sf") {
                        hyphen_unconditionally_allowed = true;
                        hyphen_unconditionally_allowed_just_set = true;
                    } else if starts_with(fst_output, i + 2, "cu") {
                        boundary_passed = false;
                        hyphen_unconditionally_allowed = true;
                        hyphen_required = true;
                    } else if starts_with(fst_output, i + 2, "ca") {
                        required_hyphen_missing = false;
                        ends_with_non_ica_noun = false;
                    }
                } else if fst_output[i + 1] == 'L' {
                    if fst_output[i + 2] == 'e' {
                        starts_with_proper_noun = true;
                        ends_with_non_ica_noun = false;
                    } else if fst_output[i + 2] == 'n' {
                        ends_with_non_ica_noun = true;
                    }
                } else if starts_with(fst_output, i + 1, "Dg") {
                    starts_with_proper_noun = false;
                }
            }

            if fst_output[i + 1] == 'X' {
                while i + 3 < len {
                    i += 1;
                    if fst_output[i] == '[' && fst_output[i + 1] == 'X' && fst_output[i + 2] == ']'
                    {
                        i += 2;
                        break;
                    }
                }
            } else if starts_with(fst_output, i + 1, "Bh") {
                i += 3;
                boundary_passed = true;
                hyphen_present = false;
                if required_hyphen_missing {
                    return false;
                }
                if hyphen_required {
                    required_hyphen_missing = true;
                }
            } else {
                i += 1;
                while i < len && fst_output[i] != ']' {
                    i += 1;
                }
            }
        } else if fst_output[i] == '-' {
            starts_with_proper_noun = false;
            ends_with_non_ica_noun = false;
            if i + 5 < len && starts_with(fst_output, i + 1, "[Bh]") {
                boundary_passed = true;
                hyphen_present = true;
                i += 4;
            }
        } else {
            // Regular character
            if boundary_passed {
                if last_char == '\0' || (before_last_char == 'i' && last_char == 's') {
                    hyphen_unconditionally_allowed = true;
                }
                if hyphen_required && hyphen_present {
                    hyphen_required = false;
                }
                if !(hyphen_unconditionally_allowed && hyphen_present) {
                    last_char = simple_lower(last_char);
                    let lc_next = simple_lower(fst_output[i]);
                    let need_hyphen = (last_char == lc_next && is_finnish_vowel(last_char))
                        || last_char.is_ascii_digit();
                    if need_hyphen != hyphen_present {
                        return false;
                    }
                }
                boundary_passed = false;
                if hyphen_unconditionally_allowed_just_set {
                    hyphen_unconditionally_allowed_just_set = false;
                } else {
                    hyphen_unconditionally_allowed = false;
                }
            }
            before_last_char = last_char;
            last_char = fst_output[i];
        }
        i += 1;
    }

    !required_hyphen_missing && (!starts_with_proper_noun || !ends_with_non_ica_noun)
}

/// Check if `slice[offset..]` starts with the given pattern.
pub(crate) fn starts_with(slice: &[char], offset: usize, pattern: &str) -> bool {
    if offset >= slice.len() {
        return false;
    }
    let remaining = &slice[offset..];
    let mut pattern_len = 0;
    for (sc, pc) in remaining.iter().zip(pattern.chars()) {
        if *sc != pc {
            return false;
        }
        pattern_len += 1;
    }
    pattern_len == pattern.chars().count()
}

/// Finnish vowel check for compound word validation.
fn is_finnish_vowel(c: char) -> bool {
    matches!(
        c,
        'a' | 'e' | 'i' | 'o' | 'u' | 'y' | '\u{00E4}' | '\u{00F6}'
    )
}

// ---------------------------------------------------------------------------
// parse_baseform
// ---------------------------------------------------------------------------

/// Extract the base form of a word from FST output.
pub(crate) fn parse_baseform(fst_output: &[char], structure: &[char]) -> Option<String> {
    let fst_len = fst_output.len();
    let structure_len = structure.len();
    let mut baseform: Vec<char> = Vec::with_capacity(fst_len + 1);
    let mut latest_xp_start_in_fst: usize = 0;
    let mut latest_xp_start_in_baseform: usize = 0;
    let mut hyphens_in_latest_xp: usize = 0;
    let mut structure_pos: usize = 0;
    let mut is_in_xp = false;
    let mut is_in_xr = false;
    let mut is_in_tag = false;
    let mut ignore_next_de = false;
    let mut is_de = false;
    let mut class_tag_seen = false;

    let mut i = 0;
    while i < fst_len {
        if fst_output[i] == '[' {
            if i + 2 >= fst_len {
                return None;
            }
            if fst_output[i + 1] == 'X' {
                if fst_output[i + 2] == ']' {
                    is_in_xp = false;
                    is_in_xr = false;
                    i += 2;
                } else if i + 6 < fst_len && fst_output[i + 3] == ']' {
                    match fst_output[i + 2] {
                        'p' | 'j' => {
                            i += 3;
                            is_in_xp = true;
                            latest_xp_start_in_fst = i + 1;
                            latest_xp_start_in_baseform = baseform.len();
                            hyphens_in_latest_xp = 0;
                        }
                        'r' | 's' => {
                            i += 3;
                            is_in_xr = true;
                        }
                        _ => {
                            i += 3;
                            is_in_xr = true;
                        }
                    }
                }
            } else if !class_tag_seen && i + 6 < fst_len && starts_with(fst_output, i + 1, "Lu]") {
                i += 3;
                class_tag_seen = true;
                if let Some(numeral_bf) = parse_numeral_baseform(&fst_output[i + 1..], &baseform) {
                    return Some(numeral_bf);
                }
            } else if starts_with(fst_output, i + 1, "De]") {
                is_de = !ignore_next_de;
                i += 3;
            } else {
                if fst_output[i + 1] == 'L' {
                    class_tag_seen = true;
                    is_de = false;
                    ignore_next_de = i + 3 >= fst_len
                        || (fst_output[i + 2] != 'l' && !starts_with(fst_output, i + 2, "nl"));
                }
                is_in_tag = true;
            }
        } else if is_in_xr {
            // Skip characters inside [Xr]...[X]
        } else if is_in_tag {
            if fst_output[i] == ']' {
                is_in_tag = false;
            }
        } else if is_in_xp {
            if fst_output[i] == '-' {
                hyphens_in_latest_xp += 1;
            }
        } else {
            // Regular character outside tags
            let mut next_char = fst_output[i];

            if next_char == '-' {
                if hyphens_in_latest_xp > 0 {
                    hyphens_in_latest_xp -= 1;
                } else {
                    if is_de
                        && latest_xp_start_in_fst != 0
                        && !(i >= 2 && fst_output[i - 2] == 'i' && fst_output[i - 1] == 's')
                    {
                        let mut j = i;
                        while j + 4 < fst_len {
                            if starts_with(fst_output, j, "[Lep]") {
                                baseform.truncate(latest_xp_start_in_baseform);
                                let mut k = latest_xp_start_in_fst;
                                let mut first = true;
                                while k < fst_len && fst_output[k] != '[' {
                                    if fst_output[k] != '=' {
                                        if first {
                                            baseform.push(simple_upper(fst_output[k]));
                                            first = false;
                                        } else {
                                            baseform.push(fst_output[k]);
                                        }
                                    }
                                    k += 1;
                                }
                                break;
                            }
                            j += 1;
                        }
                    }
                    latest_xp_start_in_fst = 0;
                }
                is_de = false;
            }

            // Apply structure-based capitalization
            while structure_pos < structure_len {
                let pattern_char = structure[structure_pos];
                structure_pos += 1;
                if pattern_char != '=' {
                    if pattern_char == 'i' || pattern_char == 'j' {
                        next_char = simple_upper(next_char);
                    }
                    break;
                }
            }
            baseform.push(next_char);
        }
        i += 1;
    }

    if latest_xp_start_in_fst != 0 {
        baseform.truncate(latest_xp_start_in_baseform);
        let mut k = latest_xp_start_in_fst;
        while k < fst_len && fst_output[k] != '[' {
            if fst_output[k] != '=' {
                baseform.push(fst_output[k]);
            }
            k += 1;
        }
    }

    if baseform.is_empty() {
        None
    } else {
        Some(baseform.iter().collect())
    }
}

fn parse_numeral_baseform(fst_output: &[char], prefix: &[char]) -> Option<String> {
    let fst_len = fst_output.len();
    let mut baseform: Vec<char> = prefix.to_vec();
    let mut is_in_xp = false;
    let mut is_in_xr = false;
    let mut is_in_tag = false;
    let mut is_in_digit_sequence = false;
    let mut xp_passed = false;

    let mut i = 0;
    while i < fst_len {
        if i == 0 && (fst_output[i].is_ascii_digit() || fst_output[i] == '-') {
            is_in_digit_sequence = true;
        }
        if fst_output[i] == '[' {
            if is_in_digit_sequence {
                is_in_digit_sequence = false;
                xp_passed = true;
            }
            if i + 2 >= fst_len {
                return None;
            }
            if i + 6 < fst_len
                && (starts_with(fst_output, i, "[Xp]") || starts_with(fst_output, i, "[Xj]"))
            {
                i += 3;
                is_in_xp = true;
            } else if i + 6 < fst_len && starts_with(fst_output, i, "[Xr]") {
                i += 3;
                is_in_xr = true;
            } else if i + 4 == fst_len && starts_with(fst_output, i, "[Bc]") {
                return None;
            } else if i + 6 < fst_len && starts_with(fst_output, i, "[Bc]") {
                i += 3;
                xp_passed = false;
            } else if i + 6 < fst_len
                && (starts_with(fst_output, i, "[Ln]")
                    || starts_with(fst_output, i, "[Ll]")
                    || starts_with(fst_output, i, "[Lnl]"))
            {
                return None;
            } else if starts_with(fst_output, i, "[X]") {
                if is_in_xp {
                    is_in_xp = false;
                    xp_passed = true;
                }
                is_in_xr = false;
                i += 2;
            } else {
                is_in_tag = true;
            }
        } else if is_in_xr {
            // skip
        } else if is_in_tag {
            if fst_output[i] == ']' {
                is_in_tag = false;
            }
        } else if is_in_xp || is_in_digit_sequence || !xp_passed || fst_output[i] == '-' {
            baseform.push(fst_output[i]);
        }
        i += 1;
    }

    Some(baseform.iter().collect())
}

// ---------------------------------------------------------------------------
// parse_basic_attributes
// ---------------------------------------------------------------------------

/// Result of parsing basic morphological attributes from FST output.
#[derive(Debug, Clone, Default)]
pub(crate) struct BasicAttributes {
    pub class: Option<&'static str>,
    pub sijamuoto: Option<&'static str>,
    pub number: Option<&'static str>,
    pub person: Option<&'static str>,
    pub mood: Option<&'static str>,
    pub tense: Option<&'static str>,
    pub focus: Option<&'static str>,
    pub possessive: Option<&'static str>,
    pub negative: Option<&'static str>,
    pub comparison: Option<&'static str>,
    pub participle: Option<&'static str>,
    pub kysymysliite: bool,
    pub require_following_verb: Option<&'static str>,
    pub malaga_vapaa_jalkiosa: bool,
    pub possible_geographical_name: bool,
}

/// Parse morphological attributes from FST output by scanning tags backwards.
pub(crate) fn parse_basic_attributes(fst_output: &[char]) -> BasicAttributes {
    let fst_len = fst_output.len();
    let mut attrs = BasicAttributes::default();
    let mut convert_nimi_laatusana_to_laatusana = false;
    let mut bc_passed = false;
    let mut class_set = false;

    if fst_len < 3 {
        return attrs;
    }

    let mut i = fst_len - 1;
    while i >= 2 {
        if fst_output[i] == ']' {
            let mut j = i;
            while j >= 1 {
                j -= 1;
                if fst_output[j] == '[' {
                    let tag_char = fst_output[j + 1];
                    let code = extract_tag_code(fst_output, j, i);

                    match tag_char {
                        'L' => {
                            if !class_set || fst_output[j + 2] == ']' {
                                if code == "nl" {
                                    let comp = attrs.comparison;
                                    if convert_nimi_laatusana_to_laatusana
                                        || matches!(comp, Some("comparative") | Some("superlative"))
                                        || (fst_len >= 4 && starts_with(fst_output, 0, "[Lu]"))
                                    {
                                        attrs.class = Some("laatusana");
                                    } else {
                                        attrs.class = Some("nimisana_laatusana");
                                    }
                                } else if let Some(cls) = lookup_class(&code) {
                                    attrs.class = Some(cls);
                                }
                                class_set = true;
                            }
                        }
                        'N' => {
                            if attrs.number.is_none() {
                                let skip =
                                    matches!(attrs.class, Some("etuliite") | Some("seikkasana"));
                                if !skip {
                                    attrs.number = lookup_number(&code);
                                }
                            }
                        }
                        'P' => {
                            if attrs.person.is_none() {
                                attrs.person = lookup_person(&code);
                            }
                        }
                        'S' => {
                            if attrs.sijamuoto.is_none() {
                                let skip =
                                    matches!(attrs.class, Some("etuliite") | Some("seikkasana"));
                                if !skip {
                                    attrs.sijamuoto = lookup_sijamuoto(&code);
                                    if code == "sti" {
                                        convert_nimi_laatusana_to_laatusana = true;
                                    }
                                }
                            }
                        }
                        'T' => {
                            if attrs.class.is_none() && attrs.mood.is_none() {
                                attrs.mood = lookup_mood(&code);
                            }
                        }
                        'A' => {
                            if attrs.tense.is_none() {
                                attrs.tense = lookup_tense(&code);
                            }
                        }
                        'F' => {
                            if code == "ko" {
                                attrs.kysymysliite = true;
                            } else if attrs.focus.is_none() {
                                attrs.focus = lookup_focus(&code);
                            }
                        }
                        'O' => {
                            if attrs.possessive.is_none() {
                                attrs.possessive = lookup_possessive(&code);
                            }
                        }
                        'C' => {
                            if attrs.class.is_none() && attrs.comparison.is_none() {
                                attrs.comparison = lookup_comparison(&code);
                            }
                        }
                        'E' => {
                            if attrs.negative.is_none() {
                                attrs.negative = lookup_negative(&code);
                            }
                        }
                        'R' => {
                            if !bc_passed && attrs.participle.is_none() {
                                let skip = matches!(
                                    attrs.class,
                                    Some(c) if c != "laatusana" && {
                                        !starts_with(
                                            fst_output,
                                            fst_len.saturating_sub(4),
                                            "[Ln]",
                                        )
                                    }
                                );
                                if !skip {
                                    attrs.participle = lookup_participle(&code);
                                }
                            }
                        }
                        'I' => {
                            add_info_flag(&mut attrs, &code, fst_output, j);
                        }
                        'B' => {
                            if j >= 5 && fst_output[j + 2] == 'c' {
                                if !class_set
                                    && attrs.class.is_none()
                                    && (fst_output[j - 1] == '-'
                                        || (j >= 5 && starts_with(fst_output, j - 5, "-[Bh]")))
                                {
                                    attrs.class = Some("etuliite");
                                    class_set = true;
                                }
                                bc_passed = true;
                            }
                        }
                        _ => {}
                    }
                    break;
                }
            }
            if j < 3 {
                return attrs;
            }
            i = j;
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }

    attrs
}

fn add_info_flag(attrs: &mut BasicAttributes, code: &str, fst_output: &[char], tag_pos: usize) {
    if code == "vj" {
        if !fst_output.is_empty() && fst_output[0] != '-' {
            attrs.malaga_vapaa_jalkiosa = true;
        }
    } else if code == "ca" {
        let suffix = &fst_output[tag_pos..];
        let has_bc = suffix.windows(4).any(|w| w == ['[', 'B', 'c', ']']);
        let has_ll = suffix.windows(4).any(|w| w == ['[', 'L', 'l', ']']);
        if !has_bc && !has_ll && matches!(attrs.class, None | Some("nimisana")) {
            attrs.possible_geographical_name = true;
        }
    } else if code == "ra" {
        let dominated_by_infinitive = matches!(
            attrs.mood,
            Some("E-infinitive") | Some("MINEN-infinitive") | Some("MA-infinitive")
        );
        if !dominated_by_infinitive && matches!(attrs.class, None | Some("teonsana")) {
            attrs.require_following_verb = Some("A-infinitive");
        }
    } else if code == "rm" {
        let dominated_by_infinitive = matches!(
            attrs.mood,
            Some("E-infinitive") | Some("MINEN-infinitive") | Some("MA-infinitive")
        );
        if !dominated_by_infinitive && matches!(attrs.class, None | Some("teonsana")) {
            attrs.require_following_verb = Some("MA-infinitive");
        }
    }
}

// ---------------------------------------------------------------------------
// parse_debug_attributes (WORDBASES / WORDIDS)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub(crate) struct DebugAttributes {
    pub wordbases: Option<String>,
    pub wordids: Option<String>,
}

/// Parse WORDBASES and WORDIDS attributes from FST output.
pub(crate) fn parse_debug_attributes(fst_output: &[char]) -> DebugAttributes {
    let fst_len = fst_output.len();
    let mut word_ids: Vec<char> = Vec::with_capacity(2 * fst_len + 1);
    let mut word_bases: Vec<char> = Vec::with_capacity(2 * fst_len + 1);
    let mut xs_buffer: Vec<char> = Vec::with_capacity(fst_len);
    let mut xp_buffer: Vec<char> = Vec::with_capacity(fst_len);
    let mut id_pos_last: usize = 0;
    let mut base_pos_last: usize = 0;
    let mut in_xs = false;
    let mut in_xp = false;
    let mut in_xj = false;
    let mut in_x_other = false;
    let mut in_content = false;
    let mut in_tag = false;
    let mut any_xs = false;

    let mut i = 0;
    while i < fst_len {
        if starts_with(fst_output, i, "[L") || starts_with(fst_output, i, "-[B") {
            in_content = false;
            in_tag = true;
            debug_content_end(
                &mut word_ids,
                &mut word_bases,
                &mut xs_buffer,
                &mut xp_buffer,
            );
            if fst_output[i] == '-' {
                word_ids.push('+');
                word_ids.push('-');
                word_bases.push('+');
                word_bases.push('-');
                i += 1;
            }
        } else if fst_output[i] == '[' && i + 2 < fst_len {
            if fst_output[i + 1] == 'X' {
                match fst_output[i + 2] {
                    's' => {
                        in_xs = true;
                        any_xs = true;
                        xs_buffer.clear();
                        i += 3;
                    }
                    'p' => {
                        in_xp = true;
                        xp_buffer.clear();
                        i += 3;
                    }
                    'j' => {
                        if in_content {
                            debug_content_end(
                                &mut word_ids,
                                &mut word_bases,
                                &mut xs_buffer,
                                &mut xp_buffer,
                            );
                            id_pos_last = word_ids.len();
                            base_pos_last = word_bases.len();
                        }
                        in_xj = true;
                        xp_buffer.clear();
                        i += 3;
                    }
                    ']' => {
                        in_xs = false;
                        in_xp = false;
                        in_xj = false;
                        in_x_other = false;
                        i += 2;
                    }
                    _ => {
                        in_x_other = true;
                        i += 3;
                    }
                }
            } else {
                in_tag = true;
            }
        } else if fst_output[i] == ']' {
            in_tag = false;
        } else if in_tag || in_x_other {
            // skip
        } else if in_xs {
            xs_buffer.push(fst_output[i]);
        } else if in_xp {
            xp_buffer.push(fst_output[i]);
        } else if in_xj {
            if xp_buffer.is_empty() {
                xp_buffer.push('+');
            }
            xp_buffer.push(fst_output[i]);
        } else {
            if !in_content {
                word_ids.push('+');
                word_bases.push('+');
                id_pos_last = word_ids.len();
                base_pos_last = word_bases.len();
                in_content = true;
            }
            word_ids.push(fst_output[i]);
            word_bases.push(fst_output[i]);
        }
        i += 1;
    }

    // Handle trailing [Xj] content
    if !xp_buffer.is_empty() {
        word_bases.truncate(base_pos_last);
        word_ids.truncate(id_pos_last);
        let plus = if !word_bases.is_empty()
            && !xp_buffer.is_empty()
            && xp_buffer[0] == '+'
            && *word_bases.last().unwrap() == '+'
        {
            1
        } else {
            0
        };
        if plus > 0 {
            word_bases.pop();
            word_ids.pop();
        }
        for &c in &xp_buffer {
            if c != '=' {
                word_bases.push(c);
                word_ids.push(c);
            }
        }
        word_bases.push('(');
        word_bases.extend_from_slice(&xp_buffer);
        word_bases.push(')');
    }
    if !xs_buffer.is_empty() {
        word_ids.push('(');
        word_ids.push('w');
        word_ids.extend_from_slice(&xs_buffer);
        word_ids.push(')');
    }

    DebugAttributes {
        wordbases: Some(word_bases.iter().collect()),
        wordids: if any_xs {
            Some(word_ids.iter().collect())
        } else {
            None
        },
    }
}

fn debug_content_end(
    word_ids: &mut Vec<char>,
    word_bases: &mut Vec<char>,
    xs_buffer: &mut Vec<char>,
    xp_buffer: &mut Vec<char>,
) {
    if !xs_buffer.is_empty() {
        word_ids.push('(');
        word_ids.push('w');
        word_ids.extend_from_slice(xs_buffer);
        word_ids.push(')');
        xs_buffer.clear();
    }
    if !xp_buffer.is_empty() {
        word_bases.push('(');
        word_bases.extend_from_slice(xp_buffer);
        word_bases.push(')');
        xp_buffer.clear();
    }
}

// ---------------------------------------------------------------------------
// fix_structure
// ---------------------------------------------------------------------------

/// Apply post-processing fixes to the STRUCTURE string based on derivation tags.
pub(crate) fn fix_structure(structure: &mut [char], fst_output: &[char]) {
    let fst_len = fst_output.len();
    let mut is_de = false;
    let mut hyphen_count: usize = 0;

    let mut j = 0;
    while j < fst_len {
        if j + 3 < fst_len && fst_output[j] == '[' {
            if fst_output[j + 1] == 'D' {
                if fst_output[j + 2] == 'g' {
                    let mut hyphens_in_structure = 0;
                    for ch in structure.iter_mut() {
                        if *ch == 'i' {
                            if hyphens_in_structure == hyphen_count {
                                *ch = 'p';
                            }
                        } else if *ch == '-' {
                            hyphens_in_structure += 1;
                        }
                    }
                } else if fst_output[j + 2] == 'e' {
                    is_de = true;
                }
            } else if starts_with(fst_output, j + 1, "Ln]") {
                is_de = false;
            }
        } else if fst_output[j] == '-' {
            hyphen_count += 1;
            if is_de {
                let mut to_upper = j == fst_len - 1;
                let mut k = j + 1;
                while !to_upper && k + 4 < fst_len {
                    if starts_with(fst_output, k, "[Lep]") {
                        to_upper = true;
                    }
                    k += 1;
                }
                if to_upper {
                    for ch in structure.iter_mut() {
                        if *ch == 'i' || *ch == 'p' {
                            *ch = 'i';
                            return;
                        }
                    }
                }
            }
        }
        j += 1;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    // -- parse_structure tests --

    #[test]
    fn structure_simple_noun() {
        let fst = chars("[Ln][Xp]koira[X]koira[Sn][Ny]");
        let result = parse_structure(&fst, 5);
        assert_eq!(result, "=ppppp");
    }

    #[test]
    fn structure_proper_noun() {
        let fst = chars("[Lep][Xp]Helsinki[X]Helsinki[Sn][Ny]");
        let result = parse_structure(&fst, 8);
        assert_eq!(result, "=ippppppp");
    }

    #[test]
    fn structure_abbreviation() {
        let fst = chars("[La][Xp]EU[X]EU[Sn][Ny]");
        let result = parse_structure(&fst, 2);
        assert_eq!(result, "=qq");
    }

    #[test]
    fn structure_with_hyphen() {
        let fst = chars("-[Bh][Ln][Xp]koira[X]koiran[Sg][Ny]");
        let result = parse_structure(&fst, 7);
        assert_eq!(result, "-pppppp");
    }

    #[test]
    fn structure_with_colon() {
        let fst = chars("[La][Xp]EU[X]EU:n[Sg][Ny]");
        let result = parse_structure(&fst, 4);
        assert_eq!(result, "=qq:p");
    }

    // -- is_valid_analysis tests --

    #[test]
    fn valid_simple_analysis() {
        let fst = chars("[Ln][Xp]koira[X]koira[Sn][Ny]");
        assert!(is_valid_analysis(&fst));
    }

    #[test]
    fn valid_compound_with_required_hyphen() {
        let fst = chars("[Ln][Xp]maa[X]maa-[Bh][Ln][Xp]alue[X]alue[Sn][Ny]");
        assert!(is_valid_analysis(&fst));
    }

    #[test]
    fn reject_missing_required_hyphen() {
        let fst = chars("[Ln][Xp]maa[X]maa[Bh][Ln][Xp]alue[X]alue[Sn][Ny]");
        assert!(!is_valid_analysis(&fst));
    }

    #[test]
    fn valid_compound_no_hyphen_different_chars() {
        let fst = chars("[Ln][Xp]koira[X]koira[Bh][Ln][Xp]koti[X]koti[Sn][Ny]");
        assert!(is_valid_analysis(&fst));
    }

    // -- parse_baseform tests --

    #[test]
    fn baseform_simple_noun() {
        let fst = chars("[Ln][Xp]koira[X]koira[Sn][Ny]");
        let structure = chars("=ppppp");
        let result = parse_baseform(&fst, &structure);
        assert_eq!(result.as_deref(), Some("koira"));
    }

    #[test]
    fn baseform_proper_noun() {
        let fst = chars("[Lep][Xp]Helsinki[X]Helsingin[Sg][Ny]");
        let structure = chars("=ipppppppp");
        let result = parse_baseform(&fst, &structure);
        assert_eq!(result.as_deref(), Some("Helsinki"));
    }

    // -- parse_basic_attributes tests --

    #[test]
    fn basic_attrs_noun_nominative_singular() {
        let fst = chars("[Ln][Xp]koira[X]koira[Sn][Ny]");
        let attrs = parse_basic_attributes(&fst);
        assert_eq!(attrs.class, Some("nimisana"));
        assert_eq!(attrs.sijamuoto, Some("nimento"));
        assert_eq!(attrs.number, Some("singular"));
    }

    #[test]
    fn basic_attrs_verb() {
        let fst = chars("[Lt][Xp]juosta[X]juoksen[Tt][Ap][P1][Ny]");
        let attrs = parse_basic_attributes(&fst);
        assert_eq!(attrs.class, Some("teonsana"));
        assert_eq!(attrs.mood, Some("indicative"));
        assert_eq!(attrs.tense, Some("present_simple"));
        assert_eq!(attrs.person, Some("1"));
        assert_eq!(attrs.number, Some("singular"));
    }

    #[test]
    fn basic_attrs_adjective_comparative() {
        let fst = chars("[Ll][Xp]suuri[X]suurempi[Cc][Sn][Ny]");
        let attrs = parse_basic_attributes(&fst);
        assert_eq!(attrs.class, Some("laatusana"));
        assert_eq!(attrs.comparison, Some("comparative"));
    }

    #[test]
    fn basic_attrs_question_clitic() {
        let fst = chars("[Ln][Xp]koira[X]koirako[Sn][Ny][Fko]");
        let attrs = parse_basic_attributes(&fst);
        assert!(attrs.kysymysliite);
    }

    #[test]
    fn basic_attrs_possessive() {
        let fst = chars("[Ln][Xp]koira[X]koirani[Sn][Ny][O1y]");
        let attrs = parse_basic_attributes(&fst);
        assert_eq!(attrs.possessive, Some("1s"));
    }

    #[test]
    fn basic_attrs_participle() {
        let fst = chars("[Lt][Xp]juosta[X]juostu[Rt][Sn][Ny]");
        let attrs = parse_basic_attributes(&fst);
        assert_eq!(attrs.participle, Some("past_passive"));
    }

    #[test]
    fn basic_attrs_focus_kin() {
        let fst = chars("[Ln][Xp]koira[X]koirakin[Sn][Ny][Fkin]");
        let attrs = parse_basic_attributes(&fst);
        assert_eq!(attrs.focus, Some("kin"));
    }

    #[test]
    fn basic_attrs_negative() {
        let fst = chars("[Lt][Xp]olla[X]ole[Et][Tt][Ap][P2][Ny]");
        let attrs = parse_basic_attributes(&fst);
        assert_eq!(attrs.negative, Some("true"));
    }

    // -- lookup function tests --

    #[test]
    fn lookup_class_values() {
        assert_eq!(lookup_class("n"), Some("nimisana"));
        assert_eq!(lookup_class("t"), Some("teonsana"));
        assert_eq!(lookup_class("l"), Some("laatusana"));
        assert_eq!(lookup_class("u"), Some("lukusana"));
        assert_eq!(lookup_class("ur"), Some("lukusana"));
        assert_eq!(lookup_class("ee"), Some("etunimi"));
        assert_eq!(lookup_class("xyz"), None);
    }

    #[test]
    fn lookup_sijamuoto_values() {
        assert_eq!(lookup_sijamuoto("n"), Some("nimento"));
        assert_eq!(lookup_sijamuoto("g"), Some("omanto"));
        assert_eq!(lookup_sijamuoto("sti"), Some("kerrontosti"));
        assert_eq!(lookup_sijamuoto("xyz"), None);
    }

    #[test]
    fn lookup_mood_values() {
        assert_eq!(lookup_mood("n1"), Some("A-infinitive"));
        assert_eq!(lookup_mood("t"), Some("indicative"));
        assert_eq!(lookup_mood("e"), Some("conditional"));
        assert_eq!(lookup_mood("xyz"), None);
    }

    // -- parse_debug_attributes tests --

    #[test]
    fn debug_attrs_simple_word() {
        let fst = chars("[Ln][Xp]koira[X]koira[Sn][Ny]");
        let debug = parse_debug_attributes(&fst);
        assert!(debug.wordbases.is_some());
        let wb = debug.wordbases.unwrap();
        assert!(
            wb.contains("koira"),
            "wordbases should contain 'koira': {wb}"
        );
    }

    #[test]
    fn debug_attrs_with_xs() {
        let fst = chars("[Ln][Xs]DOG[X][Xp]koira[X]koira[Sn][Ny]");
        let debug = parse_debug_attributes(&fst);
        assert!(debug.wordids.is_some());
        let wi = debug.wordids.unwrap();
        assert!(wi.contains("DOG"), "wordids should contain 'DOG': {wi}");
    }

    #[test]
    fn structure_compound_rautatieasema() {
        let fst = chars(
            "[Ln][Xp]rauta[X]raut[Sn][Ny]a[Bh][Bc][Ln][Ica][Xp]tie[X]tie[Sn][Ny][Bh][Bc][Ln][Xp]asema[X]asem[Sn][Ny]a",
        );
        let result = parse_structure(&fst, 13);
        assert_eq!(result, "=ppppp=ppp=ppppp");
    }

    #[test]
    fn structure_compound_koirakoti() {
        let fst = chars("[Ln][Xp]koira[X]koira[Sn][Ny][Bh][Bc][Ln][Xp]koti[X]koti[Sn][Ny]");
        let result = parse_structure(&fst, 9);
        assert_eq!(result, "=ppppp=pppp");
    }

    // -- fix_structure tests --

    #[test]
    fn fix_structure_dg_lowercases() {
        let fst = chars("[Dg][Le][Xp]Helsinki[X]helsinki[Sn][Ny]");
        let mut structure: Vec<char> = "=ipppppppp".chars().collect();
        fix_structure(&mut structure, &fst);
        assert_eq!(structure[1], 'p');
    }
}
