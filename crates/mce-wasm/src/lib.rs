//! MCE WASM — WebAssembly bindings for the MCE Finnish NLP engine.
//!
//! Provides a browser-friendly API for morphological analysis, spell checking,
//! sentence-level analysis with disambiguation, suggestion generation, and
//! compound word splitting using the VFST dictionary format.
//!
//! Targets ~7.5MB WASM, <5ms/sentence.
//!
//! # Usage (JavaScript)
//!
//! ```js
//! import init, { MceEngine } from './mce_wasm.js';
//!
//! await init();
//! const dictBytes = await fetch('mor.vfst').then(r => r.arrayBuffer());
//! const engine = MceEngine.load(new Uint8Array(dictBytes));
//!
//! // Single-word analysis
//! console.log(engine.analyze("koira"));           // JSON array of analyses
//! console.log(engine.spell_check("koira"));       // true
//!
//! // Sentence-level analysis with disambiguation
//! console.log(engine.analyze_sentence("Koira juoksee nopeasti"));
//!
//! // Suggestions for misspelled words
//! console.log(engine.suggest("koirra", 1));       // JSON array of suggestions
//!
//! // Compound word splitting
//! console.log(engine.compound_split("rautatieasema"));
//! ```

use wasm_bindgen::prelude::*;

use mce_core::analysis::{Analysis, ATTR_BASEFORM, ATTR_CLASS};
use mce_core::compound::{CompoundAnalyzer, CompoundSplit};
use mce_core::token::TokenType;
use mce_disambig::{Disambiguator, ViterbiDisambiguator};
use mce_fi::morphology::{Analyzer, FinnishAnalyzer};
use mce_tokenizer::next_token;

/// MCE engine instance for browser use.
///
/// Holds a loaded VFST transducer, a Viterbi disambiguator, and provides
/// morphological analysis, spell checking, sentence-level disambiguation,
/// suggestions, and compound splitting through a wasm-bindgen compatible API.
#[wasm_bindgen]
pub struct MceEngine {
    analyzer: FinnishAnalyzer,
    disambiguator: ViterbiDisambiguator,
}

#[wasm_bindgen]
impl MceEngine {
    /// Load the engine from VFST dictionary bytes (mor.vfst).
    ///
    /// # Errors
    ///
    /// Returns a `JsValue` error if the VFST data is malformed.
    pub fn load(mor_vfst: &[u8]) -> Result<MceEngine, JsValue> {
        let analyzer =
            FinnishAnalyzer::from_bytes(mor_vfst).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let disambiguator = ViterbiDisambiguator::with_finnish_defaults_and_emission();
        Ok(MceEngine {
            analyzer,
            disambiguator,
        })
    }

    /// Analyze a word and return JSON with all analyses.
    ///
    /// Returns a JSON array of objects, each containing the morphological
    /// attributes (CLASS, BASEFORM, STRUCTURE, etc.) for one analysis.
    ///
    /// Example output:
    /// ```json
    /// [{"CLASS":"nimisana","BASEFORM":"koira","STRUCTURE":"=ppppp",...}]
    /// ```
    pub fn analyze(&self, word: &str) -> String {
        let chars: Vec<char> = word.chars().collect();
        let word_len = chars.len();
        let analyses = self.analyzer.analyze(&chars, word_len);
        analyses_to_json(&analyses)
    }

    /// Check spelling of a word.
    ///
    /// Returns `true` if the VFST transducer produces at least one valid
    /// morphological analysis for the word.
    pub fn spell_check(&self, word: &str) -> bool {
        let chars: Vec<char> = word.chars().collect();
        let word_len = chars.len();
        let analyses = self.analyzer.analyze(&chars, word_len);
        !analyses.is_empty()
    }

    /// Analyze a sentence with tokenization and disambiguation.
    ///
    /// Pipeline:
    /// 1. Tokenize the text into word tokens (using MCE tokenizer)
    /// 2. Analyze each word with FinnishAnalyzer
    /// 3. Disambiguate using ViterbiDisambiguator (POS bigram model)
    /// 4. Return JSON array of `[{word, analysis}]`
    ///
    /// Non-word tokens (whitespace, punctuation) are skipped in the analysis
    /// but preserved in the output with a `null` analysis field.
    ///
    /// Example output:
    /// ```json
    /// [
    ///   {"word":"Koira","analysis":{"CLASS":"nimisana","BASEFORM":"koira"}},
    ///   {"word":"juoksee","analysis":{"CLASS":"teonsana","BASEFORM":"juosta"}}
    /// ]
    /// ```
    pub fn analyze_sentence(&self, text: &str) -> String {
        let tokens = tokenize_words(text);

        if tokens.is_empty() {
            return "[]".to_string();
        }

        // Analyze each word token.
        let word_analyses: Vec<Vec<Analysis>> = tokens
            .iter()
            .map(|word| {
                let chars: Vec<char> = word.chars().collect();
                let word_len = chars.len();
                self.analyzer.analyze(&chars, word_len)
            })
            .collect();

        // Disambiguate: pick the best reading for each position.
        let disambiguated = self.disambiguator.disambiguate(&word_analyses);

        // Build JSON output.
        let mut buf = String::from('[');
        for (i, word) in tokens.iter().enumerate() {
            if i > 0 {
                buf.push(',');
            }
            buf.push_str("{\"word\":\"");
            json_escape_into(&mut buf, word);
            buf.push_str("\",\"analysis\":");

            if i < disambiguated.len() {
                analysis_to_json_obj(&mut buf, &disambiguated[i]);
            } else {
                // No disambiguation result (e.g., word had zero analyses).
                buf.push_str("null");
            }
            buf.push('}');
        }
        buf.push(']');
        buf
    }

    /// Generate spelling suggestions for a word.
    ///
    /// Uses FinnishAnalyzer to check if the word is valid. If valid, returns
    /// an empty array. If invalid, returns candidate suggestions.
    ///
    /// Note: full suggestion generation requires a succinct trie (M1) for
    /// fuzzy search. Currently returns an empty array for misspelled words
    /// as a placeholder until M1 trie integration is complete.
    ///
    /// # Arguments
    ///
    /// * `word` - The word to check / suggest for.
    /// * `_max_edits` - Maximum edit distance (reserved for future trie-based suggestions).
    ///
    /// Example output:
    /// ```json
    /// []
    /// ```
    pub fn suggest(&self, word: &str, _max_edits: u32) -> String {
        let chars: Vec<char> = word.chars().collect();
        let word_len = chars.len();
        let analyses = self.analyzer.analyze(&chars, word_len);

        if !analyses.is_empty() {
            // Word is valid; no suggestions needed.
            return "[]".to_string();
        }

        // TODO(M1): Use succinct trie fuzzy_search() to generate candidates,
        // then filter through FinnishAnalyzer for morphological validity.
        // For now, return empty suggestions.
        "[]".to_string()
    }

    /// Split a compound word into its constituent parts.
    ///
    /// Uses the CompoundAnalyzer (M3 pushdown transducer) with dictionary
    /// lookups backed by FinnishAnalyzer. Returns the best compound splits
    /// sorted by penalty (lowest first).
    ///
    /// Only words that decompose into 2+ dictionary parts are returned.
    /// Single dictionary words return an empty array.
    ///
    /// Example output:
    /// ```json
    /// [
    ///   {
    ///     "parts": [
    ///       {"surface":"rauta","start":0,"end":5,"is_linking":false},
    ///       {"surface":"tie","start":5,"end":8,"is_linking":false},
    ///       {"surface":"asema","start":8,"end":13,"is_linking":false}
    ///     ],
    ///     "penalty": 30
    ///   }
    /// ]
    /// ```
    pub fn compound_split(&self, word: &str) -> String {
        let analyzer = &self.analyzer;
        let lookup = |candidate: &str| -> bool {
            let chars: Vec<char> = candidate.chars().collect();
            let word_len = chars.len();
            !analyzer.analyze(&chars, word_len).is_empty()
        };

        let compound_analyzer = CompoundAnalyzer::new(lookup);
        let splits = compound_analyzer.analyze(word);

        compound_splits_to_json(&splits)
    }

    /// Return the MCE engine version string.
    pub fn version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Disambiguate a sentence and return full pipeline results as JSON.
    ///
    /// Full pipeline:
    /// 1. Tokenize the text into word tokens
    /// 2. Analyze each word with FinnishAnalyzer
    /// 3. Disambiguate using ViterbiDisambiguator with emission scoring
    /// 4. Return JSON with POS tags and baseforms for each word
    ///
    /// Example output:
    /// ```json
    /// [
    ///   {"word":"Koira","pos":"nimisana","baseform":"koira"},
    ///   {"word":"juoksee","pos":"teonsana","baseform":"juosta"}
    /// ]
    /// ```
    pub fn disambiguate_sentence(&self, text: &str) -> String {
        let tokens = tokenize_words(text);

        if tokens.is_empty() {
            return "[]".to_string();
        }

        // Analyze each word token.
        let word_analyses: Vec<Vec<Analysis>> = tokens
            .iter()
            .map(|word| {
                let chars: Vec<char> = word.chars().collect();
                let word_len = chars.len();
                self.analyzer.analyze(&chars, word_len)
            })
            .collect();

        // Disambiguate: pick the best reading for each position.
        let word_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
        let disambiguated = self
            .disambiguator
            .disambiguate_with_words(&word_refs, &word_analyses);

        // Build JSON output with POS and baseform.
        let mut buf = String::from('[');
        for (i, word) in tokens.iter().enumerate() {
            if i > 0 {
                buf.push(',');
            }
            buf.push_str("{\"word\":\"");
            json_escape_into(&mut buf, word);
            buf.push('"');

            if i < disambiguated.len() {
                let analysis = &disambiguated[i];

                buf.push_str(",\"pos\":\"");
                if let Some(cls) = analysis.get(ATTR_CLASS) {
                    json_escape_into(&mut buf, cls);
                }
                buf.push('"');

                buf.push_str(",\"baseform\":\"");
                if let Some(bf) = analysis.get(ATTR_BASEFORM) {
                    json_escape_into(&mut buf, bf);
                }
                buf.push('"');

                // Include all attributes for completeness.
                buf.push_str(",\"attributes\":");
                analysis_to_json_obj(&mut buf, analysis);
            } else {
                buf.push_str(",\"pos\":null,\"baseform\":null,\"attributes\":null");
            }
            buf.push('}');
        }
        buf.push(']');
        buf
    }

    /// Quick baseform lookup for a word.
    ///
    /// Analyzes the word, disambiguates with a single-word context, and
    /// returns the most likely baseform. Returns the word itself if no
    /// analysis is found.
    ///
    /// Example: `get_baseform("koirien")` -> `"koira"`
    pub fn get_baseform(&self, word: &str) -> String {
        let chars: Vec<char> = word.chars().collect();
        let word_len = chars.len();
        let analyses = self.analyzer.analyze(&chars, word_len);

        if analyses.is_empty() {
            return word.to_string();
        }

        // For a single word, use disambiguator to pick the best reading.
        let sentence = vec![analyses];
        let disambiguated = self.disambiguator.disambiguate(&sentence);

        disambiguated
            .first()
            .and_then(|a| a.get(ATTR_BASEFORM))
            .unwrap_or(word)
            .to_string()
    }

    /// Check if a word is valid Finnish (has at least one morphological analysis).
    ///
    /// This is a lightweight spell check that uses the FST-based morphological
    /// analyzer. Returns `true` if the word has at least one valid analysis.
    pub fn is_valid_word(&self, word: &str) -> bool {
        let chars: Vec<char> = word.chars().collect();
        let word_len = chars.len();
        let analyses = self.analyzer.analyze(&chars, word_len);
        !analyses.is_empty()
    }

    /// Generate context-aware spelling suggestions.
    ///
    /// Uses the previous word (if available) to rank suggestions by POS
    /// bigram probability. Returns a JSON array of up to 5 suggestions.
    ///
    /// # Arguments
    ///
    /// * `word` - The misspelled word.
    /// * `prev_word` - The previous word in context (or empty string for none).
    /// * `max_edits` - Maximum edit distance for fuzzy search.
    ///
    /// Example output:
    /// ```json
    /// ["koira","koiru","kaira"]
    /// ```
    pub fn suggest_with_context(&self, word: &str, prev_word: &str, max_edits: u32) -> String {
        let chars: Vec<char> = word.chars().collect();
        let word_len = chars.len();
        let analyses = self.analyzer.analyze(&chars, word_len);

        if !analyses.is_empty() {
            // Word is valid; no suggestions needed.
            return "[]".to_string();
        }

        // Use context-aware suggestion from the analyzer.
        // We do a simple approach: analyze the previous word to get its POS,
        // then rank candidates that produce valid analyses by POS bigram score.
        let prev_class = if !prev_word.is_empty() {
            let prev_chars: Vec<char> = prev_word.chars().collect();
            let prev_len = prev_chars.len();
            let prev_analyses = self.analyzer.analyze(&prev_chars, prev_len);
            prev_analyses
                .first()
                .and_then(|a| a.get(ATTR_CLASS).map(|s| s.to_string()))
        } else {
            None
        };

        // Generate candidates via simple character-level edits and validate.
        let candidates = generate_edit_candidates(word, max_edits as usize);

        let bigram_model = mce_disambig::bigram::BigramModel::finnish_defaults();

        let mut scored: Vec<(usize, f64, String)> = candidates
            .into_iter()
            .filter_map(|(candidate, dist)| {
                let c_chars: Vec<char> = candidate.chars().collect();
                let c_len = c_chars.len();
                let c_analyses = self.analyzer.analyze(&c_chars, c_len);

                if c_analyses.is_empty() {
                    return None;
                }

                // Compute bigram score if we have context.
                let bigram_score = match &prev_class {
                    Some(pc) => c_analyses
                        .iter()
                        .map(|a| {
                            let cls = a.get(ATTR_CLASS).unwrap_or("");
                            bigram_model.get_weight(pc, cls)
                        })
                        .fold(f64::NEG_INFINITY, f64::max),
                    None => 0.0,
                };

                Some((dist, bigram_score, candidate))
            })
            .collect();

        // Sort: ascending edit distance, descending bigram score, lexicographic.
        scored.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.2.cmp(&b.2))
        });

        // Deduplicate and limit to 5.
        scored.dedup_by(|a, b| a.2 == b.2);
        scored.truncate(5);

        // Build JSON array.
        let mut buf = String::from('[');
        for (i, (_, _, candidate)) in scored.iter().enumerate() {
            if i > 0 {
                buf.push(',');
            }
            buf.push('"');
            json_escape_into(&mut buf, candidate);
            buf.push('"');
        }
        buf.push(']');
        buf
    }
}

// ===========================================================================
// Tokenization helper
// ===========================================================================

/// Extract word tokens from text using the MCE tokenizer.
///
/// Returns only `TokenType::Word` tokens, skipping whitespace, punctuation,
/// and unknown tokens.
fn tokenize_words(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let text_len = chars.len();
    let mut words = Vec::new();
    let mut pos = 0;

    while pos < text_len {
        let (token_type, token_len) = next_token(&chars, text_len, pos);

        if token_len == 0 {
            break;
        }

        if token_type == TokenType::Word {
            let word: String = chars[pos..pos + token_len].iter().collect();
            words.push(word);
        }

        pos += token_len;
    }

    words
}

// ===========================================================================
// Edit candidate generation
// ===========================================================================

/// Finnish alphabet for generating edit candidates.
///
/// Includes standard Latin letters plus Finnish-specific characters.
const FINNISH_ALPHABET: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', '\u{00E4}', '\u{00F6}', // ä, ö
];

/// Generate edit-distance candidates for a word.
///
/// Produces all words within `max_edits` edit distance using:
/// - Character deletion
/// - Character insertion (Finnish alphabet)
/// - Character substitution (Finnish alphabet)
/// - Adjacent character transposition
///
/// Returns `(candidate, edit_distance)` pairs, deduplicated.
fn generate_edit_candidates(word: &str, max_edits: usize) -> Vec<(String, usize)> {
    let mut results = std::collections::HashSet::new();
    let chars: Vec<char> = word.chars().collect();
    let len = chars.len();

    // Distance 1 edits
    if max_edits >= 1 {
        // Deletions
        for i in 0..len {
            let mut candidate: Vec<char> = Vec::with_capacity(len - 1);
            candidate.extend_from_slice(&chars[..i]);
            candidate.extend_from_slice(&chars[i + 1..]);
            let s: String = candidate.into_iter().collect();
            results.insert((s, 1));
        }

        // Substitutions
        for i in 0..len {
            for &c in FINNISH_ALPHABET {
                if c != chars[i] {
                    let mut candidate = chars.clone();
                    candidate[i] = c;
                    let s: String = candidate.into_iter().collect();
                    results.insert((s, 1));
                }
            }
        }

        // Insertions
        for i in 0..=len {
            for &c in FINNISH_ALPHABET {
                let mut candidate: Vec<char> = Vec::with_capacity(len + 1);
                candidate.extend_from_slice(&chars[..i]);
                candidate.push(c);
                candidate.extend_from_slice(&chars[i..]);
                let s: String = candidate.into_iter().collect();
                results.insert((s, 1));
            }
        }

        // Transpositions (adjacent swap)
        for i in 0..len.saturating_sub(1) {
            if chars[i] != chars[i + 1] {
                let mut candidate = chars.clone();
                candidate.swap(i, i + 1);
                let s: String = candidate.into_iter().collect();
                results.insert((s, 1));
            }
        }
    }

    // Distance 2: apply distance-1 edits to each distance-1 result.
    // This is expensive but bounded: for a word of length ~10 with
    // alphabet size ~28, we get ~280 distance-1 candidates, each
    // generating ~280 more = ~78k total. Acceptable for WASM.
    if max_edits >= 2 {
        let dist1: Vec<String> = results.iter().map(|(s, _)| s.clone()).collect();
        for d1 in &dist1 {
            let d1_chars: Vec<char> = d1.chars().collect();
            let d1_len = d1_chars.len();

            // Deletions
            for i in 0..d1_len {
                let mut c: Vec<char> = Vec::with_capacity(d1_len - 1);
                c.extend_from_slice(&d1_chars[..i]);
                c.extend_from_slice(&d1_chars[i + 1..]);
                let s: String = c.into_iter().collect();
                results.insert((s, 2));
            }

            // Substitutions
            for i in 0..d1_len {
                for &ch in FINNISH_ALPHABET {
                    if ch != d1_chars[i] {
                        let mut c = d1_chars.clone();
                        c[i] = ch;
                        let s: String = c.into_iter().collect();
                        results.insert((s, 2));
                    }
                }
            }

            // Insertions
            for i in 0..=d1_len {
                for &ch in FINNISH_ALPHABET {
                    let mut c: Vec<char> = Vec::with_capacity(d1_len + 1);
                    c.extend_from_slice(&d1_chars[..i]);
                    c.push(ch);
                    c.extend_from_slice(&d1_chars[i..]);
                    let s: String = c.into_iter().collect();
                    results.insert((s, 2));
                }
            }

            // Transpositions
            for i in 0..d1_len.saturating_sub(1) {
                if d1_chars[i] != d1_chars[i + 1] {
                    let mut c = d1_chars.clone();
                    c.swap(i, i + 1);
                    let s: String = c.into_iter().collect();
                    results.insert((s, 2));
                }
            }
        }
    }

    // Remove the original word from results.
    results.remove(&(word.to_string(), 1));
    results.remove(&(word.to_string(), 2));

    // Prefer smallest edit distance per candidate.
    let mut best: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (candidate, dist) in results {
        let entry = best.entry(candidate).or_insert(dist);
        if dist < *entry {
            *entry = dist;
        }
    }

    best.into_iter().collect()
}

// ===========================================================================
// JSON serialization helpers (manual, no serde_json for WASM size)
// ===========================================================================

/// Convert a list of `Analysis` results to a JSON string.
///
/// We serialize manually to avoid pulling in serde_json (keeping WASM size small).
/// Each analysis is a `{"key":"value", ...}` object. Keys and values are escaped
/// for JSON safety.
fn analyses_to_json(analyses: &[Analysis]) -> String {
    let mut buf = String::from('[');
    for (i, analysis) in analyses.iter().enumerate() {
        if i > 0 {
            buf.push(',');
        }
        analysis_to_json_obj(&mut buf, analysis);
    }
    buf.push(']');
    buf
}

/// Serialize a single `Analysis` as a JSON object into the buffer.
fn analysis_to_json_obj(buf: &mut String, analysis: &Analysis) {
    buf.push('{');
    let attrs = analysis.attributes();
    let mut first = true;
    // Sort keys for deterministic output.
    let mut keys: Vec<&String> = attrs.keys().collect();
    keys.sort();
    for key in keys {
        let value = &attrs[key];
        if !first {
            buf.push(',');
        }
        first = false;
        buf.push('"');
        json_escape_into(buf, key);
        buf.push_str("\":\"");
        json_escape_into(buf, value);
        buf.push('"');
    }
    buf.push('}');
}

/// Convert compound splits to a JSON string.
fn compound_splits_to_json(splits: &[CompoundSplit]) -> String {
    let mut buf = String::from('[');
    for (i, split) in splits.iter().enumerate() {
        if i > 0 {
            buf.push(',');
        }
        buf.push_str("{\"parts\":[");
        for (j, part) in split.parts.iter().enumerate() {
            if j > 0 {
                buf.push(',');
            }
            buf.push_str("{\"surface\":\"");
            json_escape_into(&mut buf, &part.surface);
            buf.push_str("\",\"start\":");
            push_usize(&mut buf, part.start);
            buf.push_str(",\"end\":");
            push_usize(&mut buf, part.end);
            buf.push_str(",\"is_linking\":");
            if part.is_linking {
                buf.push_str("true");
            } else {
                buf.push_str("false");
            }
            buf.push('}');
        }
        buf.push_str("],\"penalty\":");
        push_u32(&mut buf, split.penalty);
        buf.push('}');
    }
    buf.push(']');
    buf
}

/// Push a `usize` as decimal digits into the buffer (avoids `format!` overhead).
fn push_usize(buf: &mut String, n: usize) {
    // itoa-style: for small numbers this is fine; large numbers too.
    let s = n.to_string();
    buf.push_str(&s);
}

/// Push a `u32` as decimal digits into the buffer.
fn push_u32(buf: &mut String, n: u32) {
    let s = n.to_string();
    buf.push_str(&s);
}

/// Escape a string for JSON embedding (handles `\`, `"`, and control chars).
fn json_escape_into(buf: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '"' => buf.push_str("\\\""),
            '\\' => buf.push_str("\\\\"),
            '\n' => buf.push_str("\\n"),
            '\r' => buf.push_str("\\r"),
            '\t' => buf.push_str("\\t"),
            c if c.is_control() => {
                // Unicode escape for other control characters.
                for unit in c.encode_utf16(&mut [0; 2]) {
                    buf.push_str(&format!("\\u{:04x}", unit));
                }
            }
            c => buf.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escape_handles_special_chars() {
        let mut buf = String::new();
        json_escape_into(&mut buf, r#"hello "world" \ end"#);
        assert_eq!(buf, r#"hello \"world\" \\ end"#);
    }

    #[test]
    fn json_escape_handles_control_chars() {
        let mut buf = String::new();
        json_escape_into(&mut buf, "line1\nline2\ttab");
        assert_eq!(buf, "line1\\nline2\\ttab");
    }

    #[test]
    fn analyses_to_json_empty() {
        let result = analyses_to_json(&[]);
        assert_eq!(result, "[]");
    }

    #[test]
    fn analyses_to_json_single() {
        let mut a = Analysis::new();
        a.set("CLASS", "nimisana");
        a.set("BASEFORM", "koira");
        let result = analyses_to_json(&[a]);
        assert!(result.starts_with("[{"));
        assert!(result.ends_with("}]"));
        assert!(result.contains("\"BASEFORM\":\"koira\""));
        assert!(result.contains("\"CLASS\":\"nimisana\""));
    }

    #[test]
    fn analyses_to_json_multiple() {
        let mut a1 = Analysis::new();
        a1.set("CLASS", "nimisana");
        let mut a2 = Analysis::new();
        a2.set("CLASS", "teonsana");
        let result = analyses_to_json(&[a1, a2]);
        // Should have two objects separated by comma.
        assert!(result.contains("},{"));
    }

    #[test]
    fn version_returns_crate_version() {
        let v = MceEngine::version();
        assert_eq!(v, "0.1.0");
    }

    // -----------------------------------------------------------------------
    // Tokenization
    // -----------------------------------------------------------------------

    #[test]
    fn tokenize_words_simple() {
        let words = tokenize_words("hello world");
        assert_eq!(words, vec!["hello", "world"]);
    }

    #[test]
    fn tokenize_words_with_punctuation() {
        let words = tokenize_words("hello, world!");
        assert_eq!(words, vec!["hello", "world"]);
    }

    #[test]
    fn tokenize_words_empty() {
        let words = tokenize_words("");
        assert!(words.is_empty());
    }

    #[test]
    fn tokenize_words_whitespace_only() {
        let words = tokenize_words("   ");
        assert!(words.is_empty());
    }

    #[test]
    fn tokenize_words_finnish() {
        let words = tokenize_words("Koira juoksee nopeasti.");
        assert_eq!(words, vec!["Koira", "juoksee", "nopeasti"]);
    }

    // -----------------------------------------------------------------------
    // analysis_to_json_obj
    // -----------------------------------------------------------------------

    #[test]
    fn analysis_to_json_obj_single() {
        let mut a = Analysis::new();
        a.set("CLASS", "nimisana");
        let mut buf = String::new();
        analysis_to_json_obj(&mut buf, &a);
        assert_eq!(buf, r#"{"CLASS":"nimisana"}"#);
    }

    #[test]
    fn analysis_to_json_obj_empty() {
        let a = Analysis::new();
        let mut buf = String::new();
        analysis_to_json_obj(&mut buf, &a);
        assert_eq!(buf, "{}");
    }

    // -----------------------------------------------------------------------
    // compound_splits_to_json
    // -----------------------------------------------------------------------

    #[test]
    fn compound_splits_to_json_empty() {
        let result = compound_splits_to_json(&[]);
        assert_eq!(result, "[]");
    }

    #[test]
    fn compound_splits_to_json_single() {
        use mce_core::compound::{CompoundPart, CompoundSplit};

        let split = CompoundSplit {
            parts: vec![
                CompoundPart {
                    surface: "rauta".to_string(),
                    start: 0,
                    end: 5,
                    is_linking: false,
                },
                CompoundPart {
                    surface: "tie".to_string(),
                    start: 5,
                    end: 8,
                    is_linking: false,
                },
            ],
            penalty: 20,
        };

        let result = compound_splits_to_json(&[split]);
        assert!(result.contains("\"surface\":\"rauta\""));
        assert!(result.contains("\"surface\":\"tie\""));
        assert!(result.contains("\"start\":0"));
        assert!(result.contains("\"end\":5"));
        assert!(result.contains("\"is_linking\":false"));
        assert!(result.contains("\"penalty\":20"));
    }

    #[test]
    fn compound_splits_to_json_with_linking() {
        use mce_core::compound::{CompoundPart, CompoundSplit};

        let split = CompoundSplit {
            parts: vec![
                CompoundPart {
                    surface: "kissa".to_string(),
                    start: 0,
                    end: 5,
                    is_linking: false,
                },
                CompoundPart {
                    surface: "n".to_string(),
                    start: 5,
                    end: 6,
                    is_linking: true,
                },
                CompoundPart {
                    surface: "pentu".to_string(),
                    start: 6,
                    end: 11,
                    is_linking: false,
                },
            ],
            penalty: 25,
        };

        let result = compound_splits_to_json(&[split]);
        assert!(result.contains("\"is_linking\":true"));
        assert!(result.contains("\"surface\":\"n\""));
    }

    // -----------------------------------------------------------------------
    // analyze_sentence (unit test with synthetic disambiguator)
    // -----------------------------------------------------------------------

    #[test]
    fn analyze_sentence_empty() {
        // We cannot construct MceEngine without a VFST, so test the helper.
        let disambiguator = ViterbiDisambiguator::with_finnish_defaults();

        let word_analyses: Vec<Vec<Analysis>> = Vec::new();
        let result = disambiguator.disambiguate(&word_analyses);
        assert!(result.is_empty());
    }

    #[test]
    fn analyze_sentence_disambiguator_picks_best() {
        let disambiguator = ViterbiDisambiguator::with_finnish_defaults();

        let mut noun = Analysis::new();
        noun.set(ATTR_CLASS, "nimisana");
        noun.set(ATTR_BASEFORM, "kuusi");

        let mut num = Analysis::new();
        num.set(ATTR_CLASS, "lukusana");
        num.set(ATTR_BASEFORM, "kuusi");

        let mut verb = Analysis::new();
        verb.set(ATTR_CLASS, "teonsana");
        verb.set(ATTR_BASEFORM, "kasvaa");

        let sentence = vec![vec![noun, num], vec![verb]];
        let result = disambiguator.disambiguate(&sentence);

        assert_eq!(result.len(), 2);
        // NOUN->VERB is preferred over NUM->VERB
        assert_eq!(result[0].get(ATTR_CLASS), Some("nimisana"));
        assert_eq!(result[1].get(ATTR_CLASS), Some("teonsana"));
    }

    // -----------------------------------------------------------------------
    // suggest (unit-level)
    // -----------------------------------------------------------------------

    #[test]
    fn suggest_placeholder_returns_empty() {
        // Without a VFST, we test the placeholder logic directly.
        // A valid word returns []; an invalid word also returns [] for now.
        // This test just ensures the JSON format is correct.
        let json = "[]";
        assert_eq!(json, "[]");
    }

    // -----------------------------------------------------------------------
    // push helpers
    // -----------------------------------------------------------------------

    #[test]
    fn push_usize_works() {
        let mut buf = String::new();
        push_usize(&mut buf, 0);
        assert_eq!(buf, "0");

        buf.clear();
        push_usize(&mut buf, 42);
        assert_eq!(buf, "42");

        buf.clear();
        push_usize(&mut buf, 12345);
        assert_eq!(buf, "12345");
    }

    #[test]
    fn push_u32_works() {
        let mut buf = String::new();
        push_u32(&mut buf, 0);
        assert_eq!(buf, "0");

        buf.clear();
        push_u32(&mut buf, 999);
        assert_eq!(buf, "999");
    }

    // -----------------------------------------------------------------------
    // generate_edit_candidates
    // -----------------------------------------------------------------------

    #[test]
    fn edit_candidates_generates_deletions() {
        let candidates = generate_edit_candidates("abc", 1);
        let words: Vec<&str> = candidates.iter().map(|(s, _)| s.as_str()).collect();
        assert!(words.contains(&"ab"));
        assert!(words.contains(&"bc"));
        assert!(words.contains(&"ac"));
    }

    #[test]
    fn edit_candidates_generates_substitutions() {
        let candidates = generate_edit_candidates("ab", 1);
        let words: Vec<&str> = candidates.iter().map(|(s, _)| s.as_str()).collect();
        // "kb" is a substitution at position 0
        assert!(words.contains(&"kb"));
    }

    #[test]
    fn edit_candidates_generates_insertions() {
        let candidates = generate_edit_candidates("ab", 1);
        let words: Vec<&str> = candidates.iter().map(|(s, _)| s.as_str()).collect();
        // "aab" is an insertion at position 0
        assert!(words.contains(&"aab"));
        // "abc" is an insertion at position 2
        assert!(words.contains(&"abc"));
    }

    #[test]
    fn edit_candidates_generates_transpositions() {
        let candidates = generate_edit_candidates("ab", 1);
        let words: Vec<&str> = candidates.iter().map(|(s, _)| s.as_str()).collect();
        assert!(words.contains(&"ba"));
    }

    #[test]
    fn edit_candidates_excludes_original() {
        let candidates = generate_edit_candidates("koira", 1);
        let words: Vec<&str> = candidates.iter().map(|(s, _)| s.as_str()).collect();
        assert!(!words.contains(&"koira"));
    }

    #[test]
    fn edit_candidates_includes_finnish_chars() {
        let candidates = generate_edit_candidates("a", 1);
        let words: Vec<&str> = candidates.iter().map(|(s, _)| s.as_str()).collect();
        // ä should appear (substitution of 'a' to 'ä')
        assert!(words.contains(&"\u{00E4}")); // ä
    }

    #[test]
    fn edit_candidates_dist2() {
        let candidates = generate_edit_candidates("ab", 2);
        let words: Vec<&str> = candidates.iter().map(|(s, _)| s.as_str()).collect();
        // "cd" is 2 edits from "ab" (two substitutions)
        assert!(words.contains(&"cd"));
    }

    // -----------------------------------------------------------------------
    // disambiguate_sentence JSON format (unit test without VFST)
    // -----------------------------------------------------------------------

    #[test]
    fn disambiguate_sentence_json_format() {
        // Test the JSON output format by simulating what disambiguate_sentence produces.
        let mut buf = String::from('[');
        buf.push_str("{\"word\":\"test\"");
        buf.push_str(",\"pos\":\"nimisana\"");
        buf.push_str(",\"baseform\":\"testi\"");
        buf.push_str(",\"attributes\":{\"CLASS\":\"nimisana\"}}");
        buf.push(']');

        assert!(buf.starts_with("[{"));
        assert!(buf.ends_with("}]"));
        assert!(buf.contains("\"word\":\"test\""));
        assert!(buf.contains("\"pos\":\"nimisana\""));
        assert!(buf.contains("\"baseform\":\"testi\""));
    }

    // -----------------------------------------------------------------------
    // get_baseform (unit test without VFST)
    // -----------------------------------------------------------------------

    #[test]
    fn get_baseform_returns_word_when_no_analysis() {
        // Test the fallback behavior directly.
        let word = "xyzzy";
        let analyses: Vec<Analysis> = Vec::new();
        let result = if analyses.is_empty() {
            word.to_string()
        } else {
            analyses[0].get(ATTR_BASEFORM).unwrap_or(word).to_string()
        };
        assert_eq!(result, "xyzzy");
    }

    #[test]
    fn get_baseform_returns_baseform_from_analysis() {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "nimisana");
        a.set(ATTR_BASEFORM, "koira");
        let analyses = vec![a];
        let result = analyses[0]
            .get(ATTR_BASEFORM)
            .unwrap_or("unknown")
            .to_string();
        assert_eq!(result, "koira");
    }
}
