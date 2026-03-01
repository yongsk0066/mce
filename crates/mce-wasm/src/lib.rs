//! MCE WASM — WebAssembly bindings for the MCE Finnish NLP engine.
//!
//! Provides a browser-friendly API for morphological analysis and spell checking
//! using the VFST dictionary format. Targets ~7.5MB WASM, <5ms/sentence.
//!
//! # Usage (JavaScript)
//!
//! ```js
//! import init, { MceEngine } from './mce_wasm.js';
//!
//! await init();
//! const dictBytes = await fetch('mor.vfst').then(r => r.arrayBuffer());
//! const engine = MceEngine.load(new Uint8Array(dictBytes));
//! console.log(engine.analyze("koira"));   // JSON array of analyses
//! console.log(engine.spell_check("koira")); // true
//! ```

use wasm_bindgen::prelude::*;

use mce_core::analysis::Analysis;
use mce_fi::morphology::{Analyzer, FinnishAnalyzer};

/// MCE engine instance for browser use.
///
/// Holds a loaded VFST transducer and provides morphological analysis
/// and spell checking through a wasm-bindgen compatible API.
#[wasm_bindgen]
pub struct MceEngine {
    analyzer: FinnishAnalyzer,
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
        Ok(MceEngine { analyzer })
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

    /// Return the MCE engine version string.
    pub fn version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

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
            json_escape_into(&mut buf, key);
            buf.push_str("\":\"");
            json_escape_into(&mut buf, value);
            buf.push('"');
        }
        buf.push('}');
    }
    buf.push(']');
    buf
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
}
