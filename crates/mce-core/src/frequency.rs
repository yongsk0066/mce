//! Word frequency list for ranking spelling suggestions.
//!
//! Parses word frequencies from CoNLL-U format (Universal Dependencies)
//! and provides lookup by word form. Used to rank spell-check suggestions
//! so that more common words appear first among equal-distance candidates.

use std::collections::HashMap;

/// Word frequency list for ranking suggestions.
///
/// Stores lowercased word form frequencies extracted from a corpus.
/// Supports lookup by word form, relative frequency, and rank.
///
/// # Serialization
///
/// Use [`to_bytes`](Self::to_bytes) / [`from_bytes`](Self::from_bytes) for
/// compact binary serialization to avoid re-parsing CoNLL-U on every load.
pub struct FrequencyList {
    /// word_form (lowercased) -> count
    frequencies: HashMap<String, u32>,
    /// Total token count (sum of all frequencies).
    total: u32,
    /// Cached rank list: sorted (word, count) pairs, descending by count.
    /// Lazily built on first `rank()` call.
    ranked: Vec<(String, u32)>,
}

impl FrequencyList {
    /// Build a frequency list from CoNLL-U format text.
    ///
    /// CoNLL-U format: tab-separated fields, 10 columns per token line.
    /// Column 2 (index 1) is the word form. Lines starting with `#` are
    /// comments; empty lines are sentence boundaries.
    ///
    /// Multi-word tokens (ID containing `-`) and empty nodes (ID containing
    /// `.`) are skipped. Only regular tokens are counted.
    ///
    /// All word forms are lowercased before counting.
    pub fn from_conllu(content: &str) -> Self {
        let mut frequencies: HashMap<String, u32> = HashMap::new();
        let mut total: u32 = 0;

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines.
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 10 {
                continue;
            }

            let id = fields[0];

            // Skip multi-word tokens (e.g., "1-2") and empty nodes (e.g., "1.1").
            if id.contains('-') || id.contains('.') {
                continue;
            }

            // Column 2 (index 1) is the word form.
            let word_form = fields[1].to_lowercase();

            // Skip punctuation-only tokens (UPOS = PUNCT in column 4).
            let upos = fields[3];
            if upos == "PUNCT" || upos == "SYM" {
                continue;
            }

            *frequencies.entry(word_form).or_insert(0) += 1;
            total = total.saturating_add(1);
        }

        let mut ranked: Vec<(String, u32)> =
            frequencies.iter().map(|(w, &c)| (w.clone(), c)).collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        Self {
            frequencies,
            total,
            ranked,
        }
    }

    /// Create an empty frequency list.
    pub fn empty() -> Self {
        Self {
            frequencies: HashMap::new(),
            total: 0,
            ranked: Vec::new(),
        }
    }

    /// Get the absolute frequency of a word form (lowercased).
    ///
    /// Returns 0 for unknown words.
    pub fn frequency(&self, word: &str) -> u32 {
        let lower = word.to_lowercase();
        self.frequencies.get(&lower).copied().unwrap_or(0)
    }

    /// Get the relative frequency (0.0 to 1.0) of a word form.
    ///
    /// Returns 0.0 for unknown words or if the corpus is empty.
    pub fn relative_frequency(&self, word: &str) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        let count = self.frequency(word);
        count as f64 / self.total as f64
    }

    /// Get the rank of a word form (1 = most common).
    ///
    /// Returns `None` for unknown words.
    pub fn rank(&self, word: &str) -> Option<usize> {
        let lower = word.to_lowercase();
        self.ranked
            .iter()
            .position(|(w, _)| w == &lower)
            .map(|pos| pos + 1) // 1-indexed
    }

    /// Number of unique word forms in the frequency list.
    pub fn len(&self) -> usize {
        self.frequencies.len()
    }

    /// Check if the frequency list is empty.
    pub fn is_empty(&self) -> bool {
        self.frequencies.is_empty()
    }

    /// Total number of tokens counted.
    pub fn total(&self) -> u32 {
        self.total
    }

    /// Get the top N most frequent words with their counts.
    pub fn top_n(&self, n: usize) -> Vec<(&str, u32)> {
        self.ranked
            .iter()
            .take(n)
            .map(|(w, c)| (w.as_str(), *c))
            .collect()
    }

    /// Serialize the frequency list to a compact binary format.
    ///
    /// Format:
    /// - 4 bytes: total (u32 LE)
    /// - 4 bytes: number of entries (u32 LE)
    /// - For each entry:
    ///   - 2 bytes: word length (u16 LE)
    ///   - N bytes: word (UTF-8)
    ///   - 4 bytes: count (u32 LE)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        buf.extend_from_slice(&self.total.to_le_bytes());
        buf.extend_from_slice(&(self.frequencies.len() as u32).to_le_bytes());

        for (word, &count) in &self.frequencies {
            let word_bytes = word.as_bytes();
            buf.extend_from_slice(&(word_bytes.len() as u16).to_le_bytes());
            buf.extend_from_slice(word_bytes);
            buf.extend_from_slice(&count.to_le_bytes());
        }

        buf
    }

    /// Deserialize a frequency list from the compact binary format
    /// produced by [`to_bytes`](Self::to_bytes).
    ///
    /// Returns `None` if the data is malformed.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        let total = u32::from_le_bytes(data[0..4].try_into().ok()?);
        let num_entries = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;

        let mut frequencies = HashMap::with_capacity(num_entries);
        let mut offset = 8;

        for _ in 0..num_entries {
            if offset + 2 > data.len() {
                return None;
            }
            let word_len = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
            offset += 2;

            if offset + word_len > data.len() {
                return None;
            }
            let word = std::str::from_utf8(&data[offset..offset + word_len]).ok()?;
            offset += word_len;

            if offset + 4 > data.len() {
                return None;
            }
            let count = u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?);
            offset += 4;

            frequencies.insert(word.to_string(), count);
        }

        let mut ranked: Vec<(String, u32)> =
            frequencies.iter().map(|(w, &c)| (w.clone(), c)).collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        Some(Self {
            frequencies,
            total,
            ranked,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal CoNLL-U snippet for testing.
    const TEST_CONLLU: &str = "\
# sent_id = test.1
# text = Koira juoksee nopeasti.
1\tKoira\tkoira\tNOUN\tN\tCase=Nom|Number=Sing\t2\tnsubj\t2:nsubj\t_
2\tjuoksee\tjuosta\tVERB\tV\tMood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin|Voice=Act\t0\troot\t0:root\t_
3\tnopeasti\tnopeasti\tADV\tAdv\t_\t2\tadvmod\t2:advmod\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t2\tpunct\t2:punct\t_

# sent_id = test.2
# text = Koira on iso.
1\tKoira\tkoira\tNOUN\tN\tCase=Nom|Number=Sing\t3\tnsubj\t3:nsubj\t_
2\ton\tolla\tAUX\tV\tMood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin|Voice=Act\t3\tcop\t3:cop\t_
3\tiso\tiso\tADJ\tA\tCase=Nom|Degree=Pos|Number=Sing\t0\troot\t0:root\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t3\tpunct\t3:punct\t_

# sent_id = test.3
# text = Ja koira juoksee.
1\tJa\tja\tCCONJ\tC\t_\t3\tcc\t3:cc\t_
2\tkoira\tkoira\tNOUN\tN\tCase=Nom|Number=Sing\t3\tnsubj\t3:nsubj\t_
3\tjuoksee\tjuosta\tVERB\tV\tMood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin|Voice=Act\t0\troot\t0:root\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t3\tpunct\t3:punct\t_
";

    #[test]
    fn from_conllu_parses_correctly() {
        let fl = FrequencyList::from_conllu(TEST_CONLLU);
        // "koira" appears 3 times (Koira + Koira + koira, all lowercased)
        assert_eq!(fl.frequency("koira"), 3);
        assert_eq!(fl.frequency("Koira"), 3); // case-insensitive lookup
        // "juoksee" appears 2 times
        assert_eq!(fl.frequency("juoksee"), 2);
        // "on" appears 1 time
        assert_eq!(fl.frequency("on"), 1);
        // "iso" appears 1 time
        assert_eq!(fl.frequency("iso"), 1);
        // "ja" appears 1 time
        assert_eq!(fl.frequency("ja"), 1);
        // Punctuation is excluded
        assert_eq!(fl.frequency("."), 0);
    }

    #[test]
    fn from_conllu_total_count() {
        let fl = FrequencyList::from_conllu(TEST_CONLLU);
        // 3 sentences, 3+3+3 = 9 non-PUNCT tokens
        assert_eq!(fl.total(), 9);
    }

    #[test]
    fn frequency_returns_zero_for_unknown() {
        let fl = FrequencyList::from_conllu(TEST_CONLLU);
        assert_eq!(fl.frequency("xyzzy"), 0);
        assert_eq!(fl.frequency(""), 0);
    }

    #[test]
    fn relative_frequency_sums_approximately() {
        let fl = FrequencyList::from_conllu(TEST_CONLLU);
        let sum: f64 = fl
            .ranked
            .iter()
            .map(|(w, _)| fl.relative_frequency(w))
            .sum();
        // Should sum to approximately 1.0.
        assert!((sum - 1.0).abs() < 1e-9, "sum = {sum}");
    }

    #[test]
    fn relative_frequency_empty_corpus() {
        let fl = FrequencyList::empty();
        assert_eq!(fl.relative_frequency("anything"), 0.0);
    }

    #[test]
    fn rank_ordering() {
        let fl = FrequencyList::from_conllu(TEST_CONLLU);
        // "koira" (3) should be rank 1
        assert_eq!(fl.rank("koira"), Some(1));
        // "juoksee" (2) should be rank 2
        assert_eq!(fl.rank("juoksee"), Some(2));
        // Single-occurrence words have higher rank numbers
        let on_rank = fl.rank("on").unwrap();
        assert!(on_rank > 2);
    }

    #[test]
    fn rank_unknown_returns_none() {
        let fl = FrequencyList::from_conllu(TEST_CONLLU);
        assert_eq!(fl.rank("xyzzy"), None);
    }

    #[test]
    fn len_and_is_empty() {
        let fl = FrequencyList::from_conllu(TEST_CONLLU);
        assert!(!fl.is_empty());
        // unique words: koira, juoksee, nopeasti, on, iso, ja = 6
        assert_eq!(fl.len(), 6);

        let empty = FrequencyList::empty();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn top_n_returns_most_frequent() {
        let fl = FrequencyList::from_conllu(TEST_CONLLU);
        let top3 = fl.top_n(3);
        assert_eq!(top3.len(), 3);
        assert_eq!(top3[0].0, "koira");
        assert_eq!(top3[0].1, 3);
        assert_eq!(top3[1].0, "juoksee");
        assert_eq!(top3[1].1, 2);
    }

    #[test]
    fn serialization_roundtrip() {
        let fl = FrequencyList::from_conllu(TEST_CONLLU);
        let bytes = fl.to_bytes();
        let fl2 = FrequencyList::from_bytes(&bytes).expect("deserialization failed");

        assert_eq!(fl2.total(), fl.total());
        assert_eq!(fl2.len(), fl.len());
        assert_eq!(fl2.frequency("koira"), 3);
        assert_eq!(fl2.frequency("juoksee"), 2);
        assert_eq!(fl2.frequency("on"), 1);
        assert_eq!(fl2.rank("koira"), Some(1));
    }

    #[test]
    fn from_bytes_rejects_malformed() {
        assert!(FrequencyList::from_bytes(&[]).is_none());
        assert!(FrequencyList::from_bytes(&[0, 0, 0]).is_none());
        // Claims 1 entry but data is too short.
        assert!(FrequencyList::from_bytes(&[1, 0, 0, 0, 1, 0, 0, 0]).is_none());
    }

    #[test]
    fn top_n_zero_returns_empty() {
        let fl = FrequencyList::from_conllu(TEST_CONLLU);
        let top0 = fl.top_n(0);
        assert!(top0.is_empty());
    }

    #[test]
    fn top_n_exceeds_length() {
        let fl = FrequencyList::from_conllu(TEST_CONLLU);
        // There are 6 unique words; requesting 100 should return all 6.
        let top100 = fl.top_n(100);
        assert_eq!(top100.len(), 6);
    }

    #[test]
    fn skips_sym_upos() {
        let conllu = "\
# sent_id = sym.1
# text = 5 % alennus
1\t5\t5\tNUM\tNum\tNumType=Card\t3\tnummod\t3:nummod\t_
2\t%\t%\tSYM\tSym\t_\t1\tnmod\t1:nmod\t_
3\talennus\talennus\tNOUN\tN\tCase=Nom|Number=Sing\t0\troot\t0:root\t_
";
        let fl = FrequencyList::from_conllu(conllu);
        // SYM token "%" should be skipped.
        assert_eq!(fl.frequency("%"), 0);
        // Other tokens should be counted.
        assert_eq!(fl.frequency("5"), 1);
        assert_eq!(fl.frequency("alennus"), 1);
        assert_eq!(fl.total(), 2);
    }

    #[test]
    fn skips_multiword_tokens() {
        let conllu = "\
# sent_id = mwt.1
# text = Menenpä kotiin.
1-2\tMenenpä\t_\t_\t_\t_\t_\t_\t_\t_
1\tMenen\tmennä\tVERB\tV\tMood=Ind|Number=Sing|Person=1|Tense=Pres|VerbForm=Fin|Voice=Act\t0\troot\t0:root\t_
2\tpä\tpä\tPART\tPcle\t_\t1\tdiscourse\t1:discourse\t_
3\tkotiin\tkoti\tNOUN\tN\tCase=Ill|Number=Sing\t1\tobl\t1:obl\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t1\tpunct\t1:punct\t_
";
        let fl = FrequencyList::from_conllu(conllu);
        // "menenpä" (the MWT line) should NOT be counted.
        assert_eq!(fl.frequency("menenpä"), 0);
        // The actual sub-tokens should be counted.
        assert_eq!(fl.frequency("menen"), 1);
        assert_eq!(fl.frequency("pä"), 1);
        assert_eq!(fl.frequency("kotiin"), 1);
        assert_eq!(fl.total(), 3);
    }
}
