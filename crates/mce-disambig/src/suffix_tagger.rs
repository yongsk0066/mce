//! Suffix-based logistic regression POS tagger.
//!
//! A lightweight POS tagger that uses character suffix/prefix features and
//! word-level context to predict UPOS tags. The model is a sparse logistic
//! regression classifier trained offline (Python/sklearn) and loaded as a
//! compact binary file at runtime.
//!
//! # Architecture
//!
//! The tagger operates in two phases:
//!
//! 1. **Feature extraction**: For each word, extract ~20-30 sparse features
//!    (suffixes 1-6, prefixes 1-4, word shape, context suffixes, etc.).
//!
//! 2. **Sparse dot product + softmax**: Compute class logits via sparse
//!    matrix-vector multiply, then log-softmax to get per-tag probabilities.
//!
//! # Integration with MCE pipeline
//!
//! The tagger provides emission log-probabilities that feed into the Viterbi
//! lattice. It does **not** replace the FST analysis; instead it re-ranks
//! FST-generated candidates. The pipeline is:
//!
//! 1. CG-lite rules (high-precision filtering)
//! 2. SuffixTagger emission scoring (this module)
//! 3. Viterbi bigram decoding (global sequence optimization)
//!
//! # Binary model format
//!
//! ```text
//! [4 bytes: magic "MCET"]
//! [4 bytes: version (u32 LE)]
//! [4 bytes: n_features (u32 LE)]
//! [4 bytes: n_classes (u32 LE)]
//! [class names: (u16 LE len, UTF-8 bytes) x n_classes]
//! [feature names: (u16 LE len, UTF-8 bytes, u32 LE index) x n_features]
//! [intercepts: f32 LE x n_classes]
//! [scale: f32 LE]
//! [weights: i8 x (n_classes * n_features), row-major]
//! ```

use std::collections::HashMap;

/// Magic bytes identifying a suffix tagger model file.
const MODEL_MAGIC: &[u8; 4] = b"MCET";

/// Current binary format version.
const MODEL_VERSION: u32 = 1;

// ─────────────────────────────────────────────────────────────────
// Feature extraction
// ─────────────────────────────────────────────────────────────────

/// Punctuation characters recognized for feature extraction.
const PUNCT_CHARS: &str = ".,;:!?\"'()[]{}";

/// Extended punctuation (includes dashes).
const PUNCT_CHARS_EXTENDED: &str = ".,;:!?\"'()[]{}–—";

/// Configuration for feature extraction from word surface forms.
#[derive(Debug, Clone)]
pub struct FeatureConfig {
    /// Maximum suffix length to extract (1..=max_suffix_len). Default: 6.
    pub max_suffix_len: usize,
    /// Maximum prefix length to extract (1..=max_prefix_len). Default: 4.
    pub max_prefix_len: usize,
    /// Whether to extract context features from neighboring words. Default: true.
    pub use_context: bool,
    /// Maximum word length for which we include the word form as a feature. Default: 6.
    pub max_word_form_len: usize,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            max_suffix_len: 6,
            max_prefix_len: 4,
            use_context: true,
            max_word_form_len: 6,
        }
    }
}

/// Compute a compressed word shape.
///
/// Maps each character to a category (X=upper, x=lower, d=digit, other=self),
/// then deduplicates consecutive identical categories.
///
/// Examples:
/// - "Hello" -> "Xx"
/// - "123" -> "d"
/// - "U.S.A." -> "X.X.X."
fn compressed_shape(word: &str) -> String {
    let mut result = String::with_capacity(word.len().min(20));
    let mut prev: Option<char> = None;

    for c in word.chars().take(20) {
        let mapped = if c.is_uppercase() {
            'X'
        } else if c.is_lowercase() {
            'x'
        } else if c.is_ascii_digit() {
            'd'
        } else {
            c
        };
        if prev != Some(mapped) {
            result.push(mapped);
            prev = Some(mapped);
        }
    }
    result
}

/// Extract features from a single word in context.
///
/// Returns a vector of (feature_name, value) pairs. All values are 1.0
/// for binary features; numeric features use their actual value.
///
/// The feature set matches the Python prototype exactly:
/// - Suffix 1-6, prefix 1-4
/// - Word shape, capitalization, digit patterns
/// - Position (first/last/relative)
/// - Punctuation type
/// - Finnish-specific suffix patterns (case endings, verb endings)
/// - Context: previous/next word suffix-3, shape, capitalization
/// - Word form (for short words)
pub fn extract_features(
    config: &FeatureConfig,
    word: &str,
    prev_word: Option<&str>,
    next_word: Option<&str>,
    position: usize,
    sent_len: usize,
) -> Vec<String> {
    let mut features = Vec::with_capacity(32);
    let lower = word.to_lowercase();
    let lower_len = lower.chars().count();

    // ── Suffix features ──
    for n in 1..=config.max_suffix_len.min(lower_len) {
        // Get last n characters by char boundary
        let start = char_boundary_from_end(&lower, n);
        features.push(format!("suf{}={}", n, &lower[start..]));
    }

    // ── Prefix features ──
    for n in 1..=config.max_prefix_len.min(lower_len) {
        let end = char_boundary_from_start(&lower, n);
        features.push(format!("pre{}={}", n, &lower[..end]));
    }

    // ── Word properties ──
    features.push(format!("len={}", word.len().min(20)));
    features.push(format!("shape={}", compressed_shape(word)));

    if word.chars().next().is_some_and(|c| c.is_uppercase()) {
        features.push("is_upper=true".into());
    }
    if word.chars().all(|c| c.is_uppercase() || !c.is_alphabetic())
        && word.chars().any(|c| c.is_uppercase())
    {
        features.push("all_upper=True".into());
    }
    if word.chars().all(|c| c.is_lowercase() || !c.is_alphabetic())
        && word.chars().any(|c| c.is_lowercase())
    {
        features.push("all_lower=True".into());
    }
    if word.chars().any(|c| c.is_ascii_digit()) {
        features.push("has_digit=True".into());
    }
    if word.contains('-') {
        features.push("has_hyphen=True".into());
    }
    if word.chars().all(|c| c.is_ascii_digit()) && !word.is_empty() {
        features.push("is_digit=True".into());
    }

    // ── Position features ──
    if position == 0 {
        features.push("is_first=True".into());
    }
    if sent_len > 0 && position == sent_len - 1 {
        features.push("is_last=True".into());
    }
    let rel_pos = if sent_len > 1 {
        (position as f64 / (sent_len - 1) as f64 * 100.0).round() / 100.0
    } else {
        0.0
    };
    features.push(format!("rel_pos={:.2}", rel_pos));

    // ── Punctuation ──
    if word.len() == 1 && PUNCT_CHARS.contains(word) {
        features.push("is_punct=True".into());
        features.push(format!("punct_type={}", word));
    }

    // ── Finnish-specific suffix patterns ──
    if lower.ends_with("ssa") || lower.ends_with("ss\u{00E4}") {
        features.push("fi_case_iness=True".into());
    }
    if lower.ends_with("sta") || lower.ends_with("st\u{00E4}") {
        features.push("fi_case_elat=True".into());
    }
    if lower.ends_with("lla") || lower.ends_with("ll\u{00E4}") {
        features.push("fi_case_adess=True".into());
    }
    if lower.ends_with("lta") || lower.ends_with("lt\u{00E4}") {
        features.push("fi_case_ablat=True".into());
    }
    if lower.ends_with("lle") {
        features.push("fi_case_allat=True".into());
    }
    if lower.ends_with("sti") {
        features.push("fi_adv_sti=True".into());
    }
    if lower.ends_with("inen") {
        features.push("fi_adj_inen=True".into());
    }
    if lower.ends_with("inen") || lower.ends_with("llinen") {
        features.push("fi_adj_pattern=True".into());
    }

    // Verb endings
    for ending in &[
        "an",
        "en",
        "isi",
        "aa",
        "\u{00E4}\u{00E4}",
        "ee",
        "uu",
        "yy",
        "oo",
        "\u{00F6}\u{00F6}",
    ] {
        if lower.ends_with(ending) {
            features.push(format!("fi_vend_{}=True", ending));
        }
    }

    // ── Context features ──
    if config.use_context {
        if let Some(prev) = prev_word {
            let prev_lower = prev.to_lowercase();
            let prev_char_len = prev_lower.chars().count();
            if prev_char_len >= 3 {
                let start = char_boundary_from_end(&prev_lower, 3);
                features.push(format!("prev_suf3={}", &prev_lower[start..]));
            }
            features.push(format!("prev_shape={}", compressed_shape(prev)));
            if prev.chars().next().is_some_and(|c| c.is_uppercase()) {
                features.push("prev_is_upper=True".into());
            }
            if prev.len() == 1 && PUNCT_CHARS_EXTENDED.contains(prev) {
                features.push("prev_is_punct=True".into());
            }
        } else {
            features.push("prev_BOS=True".into());
        }

        if let Some(next) = next_word {
            let next_lower = next.to_lowercase();
            let next_char_len = next_lower.chars().count();
            if next_char_len >= 3 {
                let start = char_boundary_from_end(&next_lower, 3);
                features.push(format!("next_suf3={}", &next_lower[start..]));
            }
            features.push(format!("next_shape={}", compressed_shape(next)));
            if next.chars().next().is_some_and(|c| c.is_uppercase()) {
                features.push("next_is_upper=True".into());
            }
            if next.len() == 1 && PUNCT_CHARS_EXTENDED.contains(next) {
                features.push("next_is_punct=True".into());
            }
        } else {
            features.push("next_EOS=True".into());
        }
    }

    // ── Word form (for short, common words) ──
    if lower_len <= config.max_word_form_len {
        features.push(format!("word_form={}", lower));
    }

    features
}

/// Find the byte index `n` characters from the end of a string.
fn char_boundary_from_end(s: &str, n: usize) -> usize {
    let char_count = s.chars().count();
    if n >= char_count {
        return 0;
    }
    s.char_indices()
        .nth(char_count - n)
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Find the byte index `n` characters from the start of a string.
fn char_boundary_from_start(s: &str, n: usize) -> usize {
    s.char_indices().nth(n).map(|(i, _)| i).unwrap_or(s.len())
}

// ─────────────────────────────────────────────────────────────────
// Logistic regression model
// ─────────────────────────────────────────────────────────────────

/// Error type for suffix tagger operations.
#[derive(Debug)]
pub enum SuffixTaggerError {
    /// Model binary data is too short or truncated.
    TruncatedData { expected: usize, got: usize },
    /// Invalid magic bytes in model header.
    InvalidMagic([u8; 4]),
    /// Unsupported model format version.
    UnsupportedVersion(u32),
    /// Invalid UTF-8 in string data.
    InvalidUtf8(std::string::FromUtf8Error),
}

impl std::fmt::Display for SuffixTaggerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SuffixTaggerError::TruncatedData { expected, got } => {
                write!(
                    f,
                    "truncated model data: expected {} bytes, got {}",
                    expected, got
                )
            }
            SuffixTaggerError::InvalidMagic(magic) => {
                write!(f, "invalid model magic: {:?}", magic)
            }
            SuffixTaggerError::UnsupportedVersion(v) => {
                write!(f, "unsupported model version: {}", v)
            }
            SuffixTaggerError::InvalidUtf8(e) => {
                write!(f, "invalid UTF-8 in model: {}", e)
            }
        }
    }
}

impl std::error::Error for SuffixTaggerError {}

/// Suffix-based logistic regression POS tagger.
///
/// Stores a sparse logistic regression model with INT8-quantized weights.
/// Feature extraction produces sparse binary features; inference is a
/// sparse dot product followed by log-softmax.
pub struct SuffixTagger {
    /// Feature name -> feature index in the weight matrix.
    feature_vocab: HashMap<String, u32>,
    /// INT8 weight matrix, row-major: `weights[class * n_features + feature]`.
    weights: Vec<i8>,
    /// Scale factor to convert INT8 weights back to f64: `real_weight = int8_weight * scale`.
    scale: f32,
    /// Per-class intercept (bias) terms.
    intercepts: Vec<f32>,
    /// Number of features in the model.
    n_features: u32,
    /// Number of output classes (UPOS tags).
    n_classes: u32,
    /// Ordered class labels (UPOS tag names).
    classes: Vec<String>,
    /// Feature extraction configuration.
    config: FeatureConfig,
}

impl SuffixTagger {
    /// Load a suffix tagger model from its binary representation.
    ///
    /// See the module-level documentation for the binary format specification.
    ///
    /// Returns `Err` if the data is truncated, has invalid magic bytes,
    /// or uses an unsupported format version.
    pub fn from_bytes(data: &[u8]) -> Result<Self, SuffixTaggerError> {
        // ── Header (16 bytes) ──
        if data.len() < 16 {
            return Err(SuffixTaggerError::TruncatedData {
                expected: 16,
                got: data.len(),
            });
        }

        let magic: [u8; 4] = data[0..4].try_into().unwrap();
        if &magic != MODEL_MAGIC {
            return Err(SuffixTaggerError::InvalidMagic(magic));
        }

        let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
        if version != MODEL_VERSION {
            return Err(SuffixTaggerError::UnsupportedVersion(version));
        }

        let n_features = u32::from_le_bytes(data[8..12].try_into().unwrap());
        let n_classes = u32::from_le_bytes(data[12..16].try_into().unwrap());
        let mut cursor = 16;

        // ── Class names ──
        let mut classes = Vec::with_capacity(n_classes as usize);
        for _ in 0..n_classes {
            let (name, new_cursor) = read_length_prefixed_string(data, cursor)?;
            classes.push(name);
            cursor = new_cursor;
        }

        // ── Feature vocabulary ──
        let mut feature_vocab = HashMap::with_capacity(n_features as usize);
        for _ in 0..n_features {
            let (name, new_cursor) = read_length_prefixed_string(data, cursor)?;
            if new_cursor + 4 > data.len() {
                return Err(SuffixTaggerError::TruncatedData {
                    expected: new_cursor + 4,
                    got: data.len(),
                });
            }
            let idx = u32::from_le_bytes(data[new_cursor..new_cursor + 4].try_into().unwrap());
            feature_vocab.insert(name, idx);
            cursor = new_cursor + 4;
        }

        // ── Intercepts ──
        let intercept_bytes = n_classes as usize * 4;
        if cursor + intercept_bytes > data.len() {
            return Err(SuffixTaggerError::TruncatedData {
                expected: cursor + intercept_bytes,
                got: data.len(),
            });
        }
        let mut intercepts = Vec::with_capacity(n_classes as usize);
        for i in 0..n_classes as usize {
            let offset = cursor + i * 4;
            let val = f32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
            intercepts.push(val);
        }
        cursor += intercept_bytes;

        // ── Scale ──
        if cursor + 4 > data.len() {
            return Err(SuffixTaggerError::TruncatedData {
                expected: cursor + 4,
                got: data.len(),
            });
        }
        let scale = f32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap());
        cursor += 4;

        // ── Weight matrix ──
        let weight_count = n_classes as usize * n_features as usize;
        if cursor + weight_count > data.len() {
            return Err(SuffixTaggerError::TruncatedData {
                expected: cursor + weight_count,
                got: data.len(),
            });
        }
        let weights: Vec<i8> = data[cursor..cursor + weight_count]
            .iter()
            .map(|&b| b as i8)
            .collect();

        Ok(Self {
            feature_vocab,
            weights,
            scale,
            intercepts,
            n_features,
            n_classes,
            classes,
            config: FeatureConfig::default(),
        })
    }

    /// Create a suffix tagger with explicit parameters (for testing).
    ///
    /// This bypasses the binary loading path and allows constructing a model
    /// directly with known weights.
    pub fn from_parts(
        feature_vocab: HashMap<String, u32>,
        weights: Vec<i8>,
        scale: f32,
        intercepts: Vec<f32>,
        n_features: u32,
        n_classes: u32,
        classes: Vec<String>,
    ) -> Self {
        Self {
            feature_vocab,
            weights,
            scale,
            intercepts,
            n_features,
            n_classes,
            classes,
            config: FeatureConfig::default(),
        }
    }

    /// Get the index of a UPOS class label, or `None` if not in the model.
    pub fn class_index(&self, upos: &str) -> Option<usize> {
        self.classes.iter().position(|c| c == upos)
    }

    /// Get the class labels.
    pub fn classes(&self) -> &[String] {
        &self.classes
    }

    /// Get the number of features in the model.
    pub fn n_features(&self) -> u32 {
        self.n_features
    }

    /// Get the number of classes in the model.
    pub fn n_classes(&self) -> u32 {
        self.n_classes
    }

    /// Compute emission log-probabilities for a word in context.
    ///
    /// Returns a vector of log-probabilities, one per class, in the same
    /// order as [`Self::classes()`]. These can be used directly as emission
    /// scores in the Viterbi lattice.
    ///
    /// # Arguments
    ///
    /// * `word` - The surface form of the current word.
    /// * `prev` - Surface form of the previous word, or `None` for sentence start.
    /// * `next` - Surface form of the next word, or `None` for sentence end.
    /// * `position` - Zero-based word position in the sentence.
    /// * `sent_len` - Total number of words in the sentence.
    pub fn emission_scores(
        &self,
        word: &str,
        prev: Option<&str>,
        next: Option<&str>,
        position: usize,
        sent_len: usize,
    ) -> Vec<f64> {
        let features = extract_features(&self.config, word, prev, next, position, sent_len);
        self.compute_log_probs(&features)
    }

    /// Compute emission log-probabilities for a pre-extracted feature set.
    ///
    /// This is the core inference routine: sparse dot product + log-softmax.
    pub fn compute_log_probs(&self, features: &[String]) -> Vec<f64> {
        let nc = self.n_classes as usize;
        let nf = self.n_features as usize;
        let scale = self.scale as f64;

        let mut scores = vec![0.0f64; nc];

        // Add intercepts.
        for (c, intercept) in self.intercepts.iter().enumerate() {
            scores[c] = *intercept as f64;
        }

        // Sparse dot product: only iterate over active (present) features.
        for feat_name in features {
            if let Some(&idx) = self.feature_vocab.get(feat_name) {
                let idx = idx as usize;
                if idx < nf {
                    for (c, score) in scores.iter_mut().enumerate() {
                        let w = self.weights[c * nf + idx];
                        *score += (w as f64) * scale;
                    }
                }
            }
        }

        // Log-softmax.
        log_softmax_in_place(&mut scores);
        scores
    }

    /// Tag a sentence (greedy per-word prediction, without Viterbi).
    ///
    /// Returns a UPOS tag for each word. This ignores transition scores
    /// and just picks the most probable tag for each word independently.
    /// Useful for testing and baseline evaluation.
    pub fn tag_sentence(&self, words: &[&str]) -> Vec<String> {
        let n = words.len();
        words
            .iter()
            .enumerate()
            .map(|(i, word)| {
                let prev = if i > 0 { Some(words[i - 1]) } else { None };
                let next = if i + 1 < n { Some(words[i + 1]) } else { None };
                let log_probs = self.emission_scores(word, prev, next, i, n);
                let best_idx = log_probs
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(idx, _)| idx)
                    .unwrap_or(0);
                self.classes[best_idx].clone()
            })
            .collect()
    }

    /// Get emission log-probabilities as a map from UPOS tag to log-probability.
    ///
    /// Convenience method for integration with existing code that uses
    /// string-keyed maps.
    pub fn emission_scores_map(
        &self,
        word: &str,
        prev: Option<&str>,
        next: Option<&str>,
        position: usize,
        sent_len: usize,
    ) -> HashMap<String, f64> {
        let log_probs = self.emission_scores(word, prev, next, position, sent_len);
        self.classes.iter().cloned().zip(log_probs).collect()
    }
}

// ─────────────────────────────────────────────────────────────────
// Numerical utilities
// ─────────────────────────────────────────────────────────────────

/// Compute log-softmax in-place: `scores[i] = log(exp(scores[i]) / sum(exp(scores)))`.
///
/// Uses the numerically stable form: `log_softmax(x_i) = x_i - log(sum(exp(x_j)))`.
fn log_softmax_in_place(scores: &mut [f64]) {
    if scores.is_empty() {
        return;
    }

    // Find max for numerical stability.
    let max_score = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    // Compute log-sum-exp.
    let log_sum_exp: f64 = scores
        .iter()
        .map(|&s| (s - max_score).exp())
        .sum::<f64>()
        .ln()
        + max_score;

    // Subtract to get log-softmax.
    for s in scores.iter_mut() {
        *s -= log_sum_exp;
    }
}

// ─────────────────────────────────────────────────────────────────
// Binary format helpers
// ─────────────────────────────────────────────────────────────────

/// Read a length-prefixed UTF-8 string from binary data.
///
/// Format: `[u16 LE length][UTF-8 bytes]`.
/// Returns `(string, new_cursor_position)`.
fn read_length_prefixed_string(
    data: &[u8],
    cursor: usize,
) -> Result<(String, usize), SuffixTaggerError> {
    if cursor + 2 > data.len() {
        return Err(SuffixTaggerError::TruncatedData {
            expected: cursor + 2,
            got: data.len(),
        });
    }
    let len = u16::from_le_bytes(data[cursor..cursor + 2].try_into().unwrap()) as usize;
    let str_start = cursor + 2;
    let str_end = str_start + len;
    if str_end > data.len() {
        return Err(SuffixTaggerError::TruncatedData {
            expected: str_end,
            got: data.len(),
        });
    }
    let s = String::from_utf8(data[str_start..str_end].to_vec())
        .map_err(SuffixTaggerError::InvalidUtf8)?;
    Ok((s, str_end))
}

/// Serialize a suffix tagger model to binary format.
///
/// This is the inverse of [`SuffixTagger::from_bytes`], useful for testing
/// the round-trip serialization.
pub fn serialize_model(tagger: &SuffixTagger) -> Vec<u8> {
    let mut buf = Vec::new();

    // Header.
    buf.extend_from_slice(MODEL_MAGIC);
    buf.extend_from_slice(&MODEL_VERSION.to_le_bytes());
    buf.extend_from_slice(&tagger.n_features.to_le_bytes());
    buf.extend_from_slice(&tagger.n_classes.to_le_bytes());

    // Class names.
    for class in &tagger.classes {
        let bytes = class.as_bytes();
        buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(bytes);
    }

    // Feature vocabulary: sort by index to ensure deterministic output.
    let mut feat_entries: Vec<(&String, &u32)> = tagger.feature_vocab.iter().collect();
    feat_entries.sort_by_key(|(_, idx)| **idx);
    for (name, idx) in feat_entries {
        let bytes = name.as_bytes();
        buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(bytes);
        buf.extend_from_slice(&idx.to_le_bytes());
    }

    // Intercepts.
    for &intercept in &tagger.intercepts {
        buf.extend_from_slice(&intercept.to_le_bytes());
    }

    // Scale.
    buf.extend_from_slice(&tagger.scale.to_le_bytes());

    // Weights.
    for &w in &tagger.weights {
        buf.push(w as u8);
    }

    buf
}

// ─────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Feature extraction tests ──

    #[test]
    fn extract_suffix_features() {
        let config = FeatureConfig::default();
        let features = extract_features(&config, "juoksee", None, None, 0, 1);
        assert!(features.contains(&"suf1=e".to_string()));
        assert!(features.contains(&"suf2=ee".to_string()));
        assert!(features.contains(&"suf3=see".to_string()));
        assert!(features.contains(&"suf4=ksee".to_string()));
        assert!(features.contains(&"suf5=oksee".to_string()));
        assert!(features.contains(&"suf6=uoksee".to_string())); // last 6 of "juoksee"
    }

    #[test]
    fn extract_prefix_features() {
        let config = FeatureConfig::default();
        let features = extract_features(&config, "juoksee", None, None, 0, 1);
        assert!(features.contains(&"pre1=j".to_string()));
        assert!(features.contains(&"pre2=ju".to_string()));
        assert!(features.contains(&"pre3=juo".to_string()));
        assert!(features.contains(&"pre4=juok".to_string()));
    }

    #[test]
    fn extract_capitalization_feature() {
        let config = FeatureConfig::default();
        let features = extract_features(&config, "Helsinki", None, None, 0, 1);
        assert!(features.contains(&"is_upper=true".to_string()));

        let features_lower = extract_features(&config, "koira", None, None, 0, 1);
        assert!(!features_lower.contains(&"is_upper=true".to_string()));
    }

    #[test]
    fn extract_context_features_prev() {
        let config = FeatureConfig::default();
        let features = extract_features(&config, "juoksee", Some("koira"), None, 1, 3);
        assert!(features.contains(&"prev_suf3=ira".to_string()));
        assert!(features.contains(&"prev_shape=x".to_string()));
        assert!(!features.contains(&"prev_BOS=True".to_string()));
    }

    #[test]
    fn extract_context_features_bos() {
        let config = FeatureConfig::default();
        let features = extract_features(&config, "Koira", None, Some("juoksee"), 0, 2);
        assert!(features.contains(&"prev_BOS=True".to_string()));
        assert!(features.contains(&"next_suf3=see".to_string()));
    }

    #[test]
    fn extract_context_features_eos() {
        let config = FeatureConfig::default();
        let features = extract_features(&config, "juoksee", Some("Koira"), None, 1, 2);
        assert!(features.contains(&"next_EOS=True".to_string()));
    }

    #[test]
    fn extract_word_form_short() {
        let config = FeatureConfig::default();
        let features = extract_features(&config, "ja", None, None, 0, 1);
        assert!(features.contains(&"word_form=ja".to_string()));
    }

    #[test]
    fn extract_word_form_long_excluded() {
        let config = FeatureConfig::default();
        let features = extract_features(&config, "nopeasti", None, None, 0, 1);
        // "nopeasti" has 8 chars, above the 6-char threshold
        assert!(!features.iter().any(|f| f.starts_with("word_form=")));
    }

    #[test]
    fn extract_position_features() {
        let config = FeatureConfig::default();
        let features = extract_features(&config, "koira", None, None, 0, 5);
        assert!(features.contains(&"is_first=True".to_string()));
        assert!(!features.contains(&"is_last=True".to_string()));

        let features_last = extract_features(&config, ".", None, None, 4, 5);
        assert!(!features_last.contains(&"is_first=True".to_string()));
        assert!(features_last.contains(&"is_last=True".to_string()));
    }

    #[test]
    fn extract_finnish_suffix_patterns() {
        let config = FeatureConfig::default();
        let features = extract_features(&config, "nopeasti", None, None, 0, 1);
        assert!(features.contains(&"fi_adv_sti=True".to_string()));

        let features_adj = extract_features(&config, "kaunis", None, None, 0, 1);
        assert!(!features_adj.contains(&"fi_adv_sti=True".to_string()));

        let features_inen = extract_features(&config, "punainen", None, None, 0, 1);
        assert!(features_inen.contains(&"fi_adj_inen=True".to_string()));
    }

    #[test]
    fn extract_punctuation_features() {
        let config = FeatureConfig::default();
        let features = extract_features(&config, ".", None, None, 0, 1);
        assert!(features.contains(&"is_punct=True".to_string()));
        assert!(features.contains(&"punct_type=.".to_string()));
    }

    #[test]
    fn extract_finnish_case_endings() {
        let config = FeatureConfig::default();

        let f_iness = extract_features(&config, "talossa", None, None, 0, 1);
        assert!(f_iness.contains(&"fi_case_iness=True".to_string()));

        let f_elat = extract_features(&config, "talosta", None, None, 0, 1);
        assert!(f_elat.contains(&"fi_case_elat=True".to_string()));

        let f_adess = extract_features(&config, "pihalla", None, None, 0, 1);
        assert!(f_adess.contains(&"fi_case_adess=True".to_string()));

        let f_ablat = extract_features(&config, "pihalta", None, None, 0, 1);
        assert!(f_ablat.contains(&"fi_case_ablat=True".to_string()));

        let f_allat = extract_features(&config, "pihalle", None, None, 0, 1);
        assert!(f_allat.contains(&"fi_case_allat=True".to_string()));
    }

    #[test]
    fn extract_digit_features() {
        let config = FeatureConfig::default();
        let features = extract_features(&config, "123", None, None, 0, 1);
        assert!(features.contains(&"is_digit=True".to_string()));
        assert!(features.contains(&"has_digit=True".to_string()));
    }

    #[test]
    fn extract_hyphen_feature() {
        let config = FeatureConfig::default();
        let features = extract_features(&config, "puna-valkoinen", None, None, 0, 1);
        assert!(features.contains(&"has_hyphen=True".to_string()));
    }

    #[test]
    fn extract_features_unicode_safe() {
        let config = FeatureConfig::default();
        // Finnish word with umlauts: "tytöllä" (girl + adessive)
        let features = extract_features(&config, "tyt\u{00F6}ll\u{00E4}", None, None, 0, 1);
        assert!(features.contains(&"fi_case_adess=True".to_string()));
        // The suffix should be "llä", not a byte slice
        assert!(features.contains(&"suf3=ll\u{00E4}".to_string()));
    }

    #[test]
    fn extract_no_context_when_disabled() {
        let config = FeatureConfig {
            use_context: false,
            ..Default::default()
        };
        let features = extract_features(&config, "juoksee", Some("koira"), Some("pihalla"), 1, 3);
        assert!(!features.iter().any(|f| f.starts_with("prev_")));
        assert!(!features.iter().any(|f| f.starts_with("next_")));
        assert!(!features.iter().any(|f| f.starts_with("prev_BOS")));
        assert!(!features.iter().any(|f| f.starts_with("next_EOS")));
    }

    // ── Word shape tests ──

    #[test]
    fn compressed_shape_basic() {
        assert_eq!(compressed_shape("Hello"), "Xx");
        assert_eq!(compressed_shape("hello"), "x");
        assert_eq!(compressed_shape("123"), "d");
        assert_eq!(compressed_shape("HELLO"), "X");
        assert_eq!(compressed_shape("a1b"), "xdx");
    }

    // ── Log-softmax tests ──

    #[test]
    fn log_softmax_uniform() {
        let mut scores = vec![0.0, 0.0, 0.0];
        log_softmax_in_place(&mut scores);
        let expected = -(3.0f64.ln());
        for &s in &scores {
            assert!((s - expected).abs() < 1e-10);
        }
    }

    #[test]
    fn log_softmax_probabilities_sum_to_one() {
        let mut scores = vec![1.0, 2.0, 3.0, 0.5, -1.0];
        log_softmax_in_place(&mut scores);
        let sum: f64 = scores.iter().map(|s| s.exp()).sum();
        assert!((sum - 1.0).abs() < 1e-10);
    }

    #[test]
    fn log_softmax_preserves_ordering() {
        let mut scores = vec![1.0, 3.0, 2.0];
        log_softmax_in_place(&mut scores);
        assert!(scores[1] > scores[2]);
        assert!(scores[2] > scores[0]);
    }

    #[test]
    fn log_softmax_empty() {
        let mut scores: Vec<f64> = Vec::new();
        log_softmax_in_place(&mut scores);
        assert!(scores.is_empty());
    }

    // ── Model construction and inference tests ──

    fn make_test_model() -> SuffixTagger {
        // 2 classes (NOUN, VERB), 3 features (suf1=a, suf1=e, word_form=on)
        let mut feature_vocab = HashMap::new();
        feature_vocab.insert("suf1=a".to_string(), 0);
        feature_vocab.insert("suf1=e".to_string(), 1);
        feature_vocab.insert("word_form=on".to_string(), 2);

        // Weight matrix (2 classes x 3 features):
        // NOUN: suf1=a -> +5, suf1=e -> -3, word_form=on -> -2
        // VERB: suf1=a -> -3, suf1=e -> +5, word_form=on -> -2
        let weights: Vec<i8> = vec![
            5, -3, -2, // NOUN
            -3, 5, -2, // VERB
        ];

        SuffixTagger::from_parts(
            feature_vocab,
            weights,
            1.0,            // scale = 1.0 (no quantization)
            vec![0.0, 0.0], // no bias
            3,              // n_features
            2,              // n_classes
            vec!["NOUN".to_string(), "VERB".to_string()],
        )
    }

    #[test]
    fn model_class_index() {
        let model = make_test_model();
        assert_eq!(model.class_index("NOUN"), Some(0));
        assert_eq!(model.class_index("VERB"), Some(1));
        assert_eq!(model.class_index("ADJ"), None);
    }

    #[test]
    fn model_inference_noun_suffix() {
        let model = make_test_model();
        // Word ending in "a" should favor NOUN.
        let features = vec!["suf1=a".to_string()];
        let log_probs = model.compute_log_probs(&features);
        assert_eq!(log_probs.len(), 2);
        // NOUN (index 0) should have higher log-prob than VERB (index 1).
        assert!(
            log_probs[0] > log_probs[1],
            "NOUN log-prob {} should be > VERB log-prob {}",
            log_probs[0],
            log_probs[1]
        );
    }

    #[test]
    fn model_inference_verb_suffix() {
        let model = make_test_model();
        // Word ending in "e" should favor VERB.
        let features = vec!["suf1=e".to_string()];
        let log_probs = model.compute_log_probs(&features);
        assert!(
            log_probs[1] > log_probs[0],
            "VERB log-prob {} should be > NOUN log-prob {}",
            log_probs[1],
            log_probs[0]
        );
    }

    #[test]
    fn model_inference_unknown_feature() {
        let model = make_test_model();
        // Unknown features should be ignored; log-probs should be uniform.
        let features = vec!["suf1=xyz".to_string(), "unknown_feat".to_string()];
        let log_probs = model.compute_log_probs(&features);
        // With zero weights + zero bias, log-softmax should give -ln(2) for both.
        let expected = -(2.0f64.ln());
        for &lp in &log_probs {
            assert!(
                (lp - expected).abs() < 1e-10,
                "Expected {}, got {}",
                expected,
                lp
            );
        }
    }

    #[test]
    fn model_tag_sentence() {
        let model = make_test_model();
        // "koira juoksee" -- "a" suffix -> NOUN, "e" suffix -> VERB
        // Note: the full feature extraction will extract many features,
        // but only suf1=a and suf1=e are in our tiny vocab.
        let tags = model.tag_sentence(&["koira", "juoksee"]);
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0], "NOUN");
        assert_eq!(tags[1], "VERB");
    }

    #[test]
    fn model_emission_scores_map() {
        let model = make_test_model();
        let map = model.emission_scores_map("koira", None, None, 0, 1);
        assert!(map.contains_key("NOUN"));
        assert!(map.contains_key("VERB"));
        assert!(map["NOUN"] > map["VERB"]);
    }

    // ── Binary serialization round-trip ──

    #[test]
    fn binary_round_trip() {
        let original = make_test_model();
        let bytes = serialize_model(&original);
        let restored = SuffixTagger::from_bytes(&bytes).expect("deserialization should succeed");

        assert_eq!(restored.n_features, original.n_features);
        assert_eq!(restored.n_classes, original.n_classes);
        assert_eq!(restored.classes, original.classes);
        assert_eq!(restored.weights, original.weights);
        assert!((restored.scale - original.scale).abs() < f32::EPSILON);
        assert_eq!(restored.intercepts.len(), original.intercepts.len());
        for (a, b) in restored.intercepts.iter().zip(original.intercepts.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
        assert_eq!(restored.feature_vocab.len(), original.feature_vocab.len());
        for (key, &val) in &original.feature_vocab {
            assert_eq!(
                restored.feature_vocab.get(key),
                Some(&val),
                "Feature '{}' mismatch",
                key
            );
        }
    }

    #[test]
    fn binary_invalid_magic() {
        let mut bytes = serialize_model(&make_test_model());
        bytes[0] = b'X'; // corrupt magic
        let result = SuffixTagger::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn binary_truncated() {
        let bytes = serialize_model(&make_test_model());
        // Truncate to just the header.
        let result = SuffixTagger::from_bytes(&bytes[..16]);
        assert!(result.is_err());
    }

    #[test]
    fn binary_unsupported_version() {
        let mut bytes = serialize_model(&make_test_model());
        // Set version to 99.
        bytes[4..8].copy_from_slice(&99u32.to_le_bytes());
        let result = SuffixTagger::from_bytes(&bytes);
        assert!(result.is_err());
    }

    // ── Integration: emission scores feed into disambiguation ──

    #[test]
    fn emission_scores_vector_length() {
        let model = make_test_model();
        let scores = model.emission_scores("koira", None, None, 0, 1);
        assert_eq!(scores.len(), model.n_classes as usize);
    }

    #[test]
    fn emission_scores_are_log_probabilities() {
        let model = make_test_model();
        let scores = model.emission_scores("koira", None, None, 0, 1);
        // All log-probabilities should be <= 0.
        for &s in &scores {
            assert!(s <= 0.0, "log-prob {} should be <= 0.0", s);
        }
        // exp(log-probs) should sum to ~1.
        let sum: f64 = scores.iter().map(|s| s.exp()).sum();
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "Probabilities should sum to 1.0, got {}",
            sum
        );
    }

    // ── Char boundary helpers ──

    #[test]
    fn char_boundary_from_end_ascii() {
        assert_eq!(char_boundary_from_end("hello", 3), 2); // "llo" starts at byte 2
        assert_eq!(char_boundary_from_end("hello", 5), 0); // full string
        assert_eq!(char_boundary_from_end("hello", 6), 0); // more than length -> 0
    }

    #[test]
    fn char_boundary_from_end_unicode() {
        // "tyttö" = 't', 'y', 't', 't', 'ö'
        // ö is 2 bytes in UTF-8, so byte length = 6, char count = 5
        let s = "tytt\u{00F6}";
        let idx = char_boundary_from_end(s, 2); // "tö" starts at char index 3
        assert_eq!(&s[idx..], "t\u{00F6}");
    }

    #[test]
    fn char_boundary_from_start_ascii() {
        assert_eq!(char_boundary_from_start("hello", 3), 3);
        assert_eq!(char_boundary_from_start("hello", 0), 0);
    }

    #[test]
    fn char_boundary_from_start_unicode() {
        let s = "\u{00E4}\u{00F6}y"; // "äöy"
        assert_eq!(char_boundary_from_start(s, 1), 2); // ä is 2 bytes
        assert_eq!(char_boundary_from_start(s, 2), 4); // ä + ö = 4 bytes
    }
}
