// Adapted from corevoikko (voikko-fst/symbols.rs)

use crate::VfstError;
use crate::flags::{FlagDiacriticParser, OpFeatureValue};
use hashbrown::HashMap;

/// Parsed symbol table from a VFST binary file.
pub struct SymbolTable {
    pub symbol_strings: Vec<String>,
    pub symbol_lengths: Vec<usize>,
    pub char_to_symbol: HashMap<char, u16>,
    pub symbol_to_diacritic: Vec<OpFeatureValue>,
    pub first_normal_char: u16,
    pub first_multi_char: u16,
    pub flag_feature_count: u16,
}

/// Parse the symbol table from VFST binary data at the given offset.
/// Returns (table, byte offset after the symbol table).
pub fn parse_symbol_table(data: &[u8], offset: usize) -> Result<(SymbolTable, usize), VfstError> {
    if offset + 2 > data.len() {
        return Err(VfstError::TooShort {
            expected: offset + 2,
            actual: data.len(),
        });
    }

    let symbol_count = u16::from_le_bytes([data[offset], data[offset + 1]]);
    let mut pos = offset + 2;

    let mut symbol_strings = Vec::with_capacity(symbol_count as usize);
    let mut symbol_lengths = Vec::with_capacity(symbol_count as usize);
    let mut char_to_symbol = HashMap::new();
    let mut symbol_to_diacritic = Vec::new();
    let mut first_normal_char: u16 = 0;
    let mut first_multi_char: u16 = 0;
    let mut flag_parser = FlagDiacriticParser::new();

    for i in 0..symbol_count {
        let str_start = pos;
        while pos < data.len() && data[pos] != 0 {
            pos += 1;
        }
        if pos >= data.len() {
            return Err(VfstError::InvalidSymbolTable(
                "unterminated symbol string".to_string(),
            ));
        }

        let symbol_bytes = &data[str_start..pos];
        pos += 1;

        if i == 0 {
            symbol_strings.push(String::new());
            symbol_lengths.push(0);
            symbol_to_diacritic.push(OpFeatureValue::default());
        } else {
            let symbol_str = std::str::from_utf8(symbol_bytes).map_err(|_| {
                VfstError::InvalidSymbolTable(format!("invalid UTF-8 in symbol {i}"))
            })?;
            let char_len = symbol_str.chars().count();

            symbol_strings.push(symbol_str.to_string());
            symbol_lengths.push(char_len);

            if first_normal_char == 0 {
                if symbol_str.starts_with('@') {
                    let ofv = flag_parser.parse(symbol_str)?;
                    symbol_to_diacritic.push(ofv);
                } else {
                    first_normal_char = i;
                }
            } else if first_multi_char == 0 && symbol_str.starts_with('[') {
                first_multi_char = i;
            }

            if first_normal_char > 0 && first_multi_char == 0 {
                if let Some(ch) = symbol_str.chars().next() {
                    char_to_symbol.insert(ch, i);
                }
            }
        }
    }

    if first_multi_char == 0 && first_normal_char > 0 {
        first_multi_char = symbol_count;
    }

    Ok((
        SymbolTable {
            symbol_strings,
            symbol_lengths,
            char_to_symbol,
            symbol_to_diacritic,
            first_normal_char,
            first_multi_char,
            flag_feature_count: flag_parser.feature_count(),
        },
        pos,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_symbol_table(symbols: &[&str]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&(symbols.len() as u16).to_le_bytes());
        for s in symbols {
            buf.extend_from_slice(s.as_bytes());
            buf.push(0);
        }
        buf
    }

    #[test]
    fn parse_simple() {
        let data = make_symbol_table(&["", "a", "b"]);
        let (t, _) = parse_symbol_table(&data, 0).unwrap();
        assert_eq!(t.symbol_strings.len(), 3);
        assert_eq!(t.first_normal_char, 1);
        assert_eq!(*t.char_to_symbol.get(&'a').unwrap(), 1);
    }

    #[test]
    fn parse_with_flags() {
        let data = make_symbol_table(&["", "@P.CASE.NOM@", "@C.NUM@", "a", "b", "[Ln]"]);
        let (t, _) = parse_symbol_table(&data, 0).unwrap();
        assert_eq!(t.first_normal_char, 3);
        assert_eq!(t.first_multi_char, 5);
        assert_eq!(t.flag_feature_count, 2);
    }

    #[test]
    fn parse_utf8() {
        let data = make_symbol_table(&["", "\u{00e4}", "\u{00f6}"]);
        let (t, _) = parse_symbol_table(&data, 0).unwrap();
        assert_eq!(*t.char_to_symbol.get(&'\u{00e4}').unwrap(), 1);
    }
}
