//! Corpus-based bigram extraction from CoNLL-U format.
//!
//! Provides utilities to extract POS bigram counts and word-form emission
//! priors from CoNLL-U treebank data (e.g. UD Finnish-TDT). The extracted
//! statistics can be fed into [`BigramModel::from_counts`] or used to
//! initialize an [`EmissionScorer`].
//!
//! # CoNLL-U format
//!
//! Each token is a tab-separated line with 10 columns:
//!
//! ```text
//! ID  FORM  LEMMA  UPOS  XPOS  FEATS  HEAD  DEPREL  DEPS  MISC
//! ```
//!
//! Sentence boundaries are marked by blank lines. Comment lines start
//! with `#`. Multi-word tokens have range IDs (e.g. `1-2`) and are
//! skipped for POS extraction since their sub-tokens carry the actual
//! UPOS tags.
//!
//! # Example
//!
//! ```
//! use mce_disambig::corpus::{parse_conllu_pos_bigrams, build_model_from_conllu};
//!
//! let conllu = "\
//! # sent_id = s1
//! # text = Koira juoksee.
//! 1\tKoira\tkoira\tNOUN\tN\tCase=Nom|Number=Sing\t2\tnsubj\t_\t_
//! 2\tjuoksee\tjuosta\tVERB\tV\tMood=Ind|Number=Sing|Person=3\t0\troot\t_\tSpaceAfter=No
//! 3\t.\t.\tPUNCT\tPunct\t_\t2\tpunct\t_\t_
//!
//! ";
//!
//! let bigrams = parse_conllu_pos_bigrams(conllu);
//! assert_eq!(bigrams[&("<BOS>".to_string(), "NOUN".to_string())], 1);
//! assert_eq!(bigrams[&("NOUN".to_string(), "VERB".to_string())], 1);
//! assert_eq!(bigrams[&("PUNCT".to_string(), "<EOS>".to_string())], 1);
//!
//! let model = build_model_from_conllu(conllu);
//! assert!(model.num_weights() > 0);
//! ```

use std::collections::HashMap;

use crate::bigram::BigramModel;

/// Sentinel token marking the beginning of a sentence.
pub const BOS: &str = "<BOS>";
/// Sentinel token marking the end of a sentence.
pub const EOS: &str = "<EOS>";

/// Parse CoNLL-U content and extract UPOS bigram counts.
///
/// Iterates over sentences in the input, collecting consecutive POS tag
/// pairs. Each sentence is padded with [`BOS`] at the start and [`EOS`]
/// at the end.
///
/// Multi-word token lines (those with range IDs like `1-2`) are skipped
/// since their sub-tokens carry the actual UPOS tags.
///
/// # Arguments
///
/// * `conllu_content` - A string containing CoNLL-U formatted data.
///
/// # Returns
///
/// A map from `(prev_pos, curr_pos)` pairs to their observed counts.
pub fn parse_conllu_pos_bigrams(conllu_content: &str) -> HashMap<(String, String), u64> {
    let mut bigrams: HashMap<(String, String), u64> = HashMap::new();
    let mut current_tags: Vec<String> = Vec::new();

    for line in conllu_content.lines() {
        let line = line.trim();

        if line.starts_with('#') {
            continue;
        }

        if line.is_empty() {
            if !current_tags.is_empty() {
                flush_sentence(&current_tags, &mut bigrams);
                current_tags.clear();
            }
            continue;
        }

        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() < 4 {
            continue;
        }

        let id = columns[0];

        // Skip multi-word tokens (range IDs like "1-2").
        if id.contains('-') {
            continue;
        }

        // Skip empty nodes (decimal IDs like "1.1").
        if id.contains('.') {
            continue;
        }

        let upos = columns[3];
        if upos != "_" {
            current_tags.push(upos.to_string());
        }
    }

    // Handle final sentence if file doesn't end with a blank line.
    if !current_tags.is_empty() {
        flush_sentence(&current_tags, &mut bigrams);
    }

    bigrams
}

/// Flush a completed sentence's POS tags into the bigram counts.
fn flush_sentence(tags: &[String], bigrams: &mut HashMap<(String, String), u64>) {
    if tags.is_empty() {
        return;
    }

    *bigrams
        .entry((BOS.to_string(), tags[0].clone()))
        .or_insert(0) += 1;

    for window in tags.windows(2) {
        *bigrams
            .entry((window[0].clone(), window[1].clone()))
            .or_insert(0) += 1;
    }

    *bigrams
        .entry((tags[tags.len() - 1].clone(), EOS.to_string()))
        .or_insert(0) += 1;
}

/// Build a [`BigramModel`] from CoNLL-U content.
///
/// This is a convenience function that extracts POS bigram counts via
/// [`parse_conllu_pos_bigrams`] and then converts them to a bigram model
/// using [`BigramModel::from_counts_map`].
///
/// # Arguments
///
/// * `conllu_content` - A string containing CoNLL-U formatted data.
///
/// # Returns
///
/// A [`BigramModel`] with log-probability weights derived from the corpus.
pub fn build_model_from_conllu(conllu_content: &str) -> BigramModel {
    let bigrams = parse_conllu_pos_bigrams(conllu_content);
    BigramModel::from_counts_map(&bigrams)
}

/// Extract emission priors P(UPOS | word_form) from CoNLL-U content.
///
/// For each word form in the corpus, computes the empirical probability
/// distribution over UPOS tags. This can be used to initialize or improve
/// an [`EmissionScorer`](crate::bigram::EmissionScorer).
///
/// # Arguments
///
/// * `conllu_content` - A string containing CoNLL-U formatted data.
///
/// # Returns
///
/// A map from lowercased word form to a map of `{ POS tag -> probability }`.
/// Probabilities for each word sum to 1.0.
pub fn extract_emission_priors(conllu_content: &str) -> HashMap<String, HashMap<String, f64>> {
    let mut counts: HashMap<String, HashMap<String, u64>> = HashMap::new();

    for line in conllu_content.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() < 4 {
            continue;
        }

        let id = columns[0];
        if id.contains('-') || id.contains('.') {
            continue;
        }

        let form = columns[1].to_lowercase();
        let upos = columns[3];

        if upos == "_" {
            continue;
        }

        *counts
            .entry(form)
            .or_default()
            .entry(upos.to_string())
            .or_insert(0) += 1;
    }

    let mut result: HashMap<String, HashMap<String, f64>> = HashMap::new();
    for (word, pos_counts) in &counts {
        let total: u64 = pos_counts.values().sum();
        if total == 0 {
            continue;
        }
        let mut probs: HashMap<String, f64> = HashMap::new();
        for (pos, &count) in pos_counts {
            probs.insert(pos.clone(), count as f64 / total as f64);
        }
        result.insert(word.clone(), probs);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─────────────────────────────────────────────────────────────
    // Test CoNLL-U data
    // ─────────────────────────────────────────────────────────────

    const SINGLE_SENTENCE: &str = "\
# sent_id = s1
# text = Koira juoksee.
1\tKoira\tkoira\tNOUN\tN\tCase=Nom|Number=Sing\t2\tnsubj\t_\t_
2\tjuoksee\tjuosta\tVERB\tV\tMood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin|Voice=Act\t0\troot\t_\tSpaceAfter=No
3\t.\t.\tPUNCT\tPunct\t_\t2\tpunct\t_\t_

";

    const TWO_SENTENCES: &str = "\
# sent_id = s1
# text = Koira juoksee.
1\tKoira\tkoira\tNOUN\tN\tCase=Nom|Number=Sing\t2\tnsubj\t_\t_
2\tjuoksee\tjuosta\tVERB\tV\tMood=Ind\t0\troot\t_\tSpaceAfter=No
3\t.\t.\tPUNCT\tPunct\t_\t2\tpunct\t_\t_

# sent_id = s2
# text = Iso kissa nukkuu.
1\tIso\tiso\tADJ\tA\tCase=Nom|Degree=Pos|Number=Sing\t2\tamod\t_\t_
2\tkissa\tkissa\tNOUN\tN\tCase=Nom|Number=Sing\t3\tnsubj\t_\t_
3\tnukkuu\tnukkua\tVERB\tV\tMood=Ind\t0\troot\t_\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t3\tpunct\t_\t_

";

    const WITH_MWT: &str = "\
# sent_id = s1
# text = Meninkö kotiin?
1-2\tMeninkö\t_\t_\t_\t_\t_\t_\t_\t_
1\tMenin\tmennä\tVERB\tV\tMood=Ind|Number=Sing|Person=1|Tense=Past\t0\troot\t_\t_
2\tkö\tkö\tPART\tPcle\t_\t1\tdiscourse\t_\t_
3\tkotiin\tkoti\tNOUN\tN\tCase=Ill|Number=Sing\t1\tnmod\t_\tSpaceAfter=No
4\t?\t?\tPUNCT\tPunct\t_\t1\tpunct\t_\t_

";

    // ─────────────────────────────────────────────────────────────
    // parse_conllu_pos_bigrams
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn bigrams_single_sentence() {
        let bigrams = parse_conllu_pos_bigrams(SINGLE_SENTENCE);

        // BOS -> NOUN -> VERB -> PUNCT -> EOS
        assert_eq!(bigrams[&(BOS.to_string(), "NOUN".to_string())], 1);
        assert_eq!(bigrams[&("NOUN".to_string(), "VERB".to_string())], 1);
        assert_eq!(bigrams[&("VERB".to_string(), "PUNCT".to_string())], 1);
        assert_eq!(bigrams[&("PUNCT".to_string(), EOS.to_string())], 1);

        // Total: 4 unique bigrams
        assert_eq!(bigrams.len(), 4);
    }

    #[test]
    fn bigrams_two_sentences() {
        let bigrams = parse_conllu_pos_bigrams(TWO_SENTENCES);

        // Sentence 1: BOS->NOUN, NOUN->VERB, VERB->PUNCT, PUNCT->EOS
        // Sentence 2: BOS->ADJ, ADJ->NOUN, NOUN->VERB, VERB->PUNCT, PUNCT->EOS
        assert_eq!(bigrams[&(BOS.to_string(), "NOUN".to_string())], 1);
        assert_eq!(bigrams[&(BOS.to_string(), "ADJ".to_string())], 1);
        assert_eq!(bigrams[&("ADJ".to_string(), "NOUN".to_string())], 1);
        // NOUN->VERB appears in both sentences
        assert_eq!(bigrams[&("NOUN".to_string(), "VERB".to_string())], 2);
        // VERB->PUNCT appears in both sentences
        assert_eq!(bigrams[&("VERB".to_string(), "PUNCT".to_string())], 2);
        // PUNCT->EOS appears in both sentences
        assert_eq!(bigrams[&("PUNCT".to_string(), EOS.to_string())], 2);
    }

    #[test]
    fn bigrams_mwt_skipped() {
        let bigrams = parse_conllu_pos_bigrams(WITH_MWT);

        // The MWT line "1-2 Meninkö" should be skipped.
        // Actual tokens: VERB(1), PART(2), NOUN(3), PUNCT(4)
        // Bigrams: BOS->VERB, VERB->PART, PART->NOUN, NOUN->PUNCT, PUNCT->EOS
        assert_eq!(bigrams[&(BOS.to_string(), "VERB".to_string())], 1);
        assert_eq!(bigrams[&("VERB".to_string(), "PART".to_string())], 1);
        assert_eq!(bigrams[&("PART".to_string(), "NOUN".to_string())], 1);
        assert_eq!(bigrams[&("NOUN".to_string(), "PUNCT".to_string())], 1);
        assert_eq!(bigrams[&("PUNCT".to_string(), EOS.to_string())], 1);
        assert_eq!(bigrams.len(), 5);
    }

    #[test]
    fn bigrams_empty_input() {
        let bigrams = parse_conllu_pos_bigrams("");
        assert!(bigrams.is_empty());
    }

    #[test]
    fn bigrams_only_comments() {
        let bigrams = parse_conllu_pos_bigrams("# comment\n# another comment\n");
        assert!(bigrams.is_empty());
    }

    #[test]
    fn bigrams_only_blank_lines() {
        let bigrams = parse_conllu_pos_bigrams("\n\n\n");
        assert!(bigrams.is_empty());
    }

    #[test]
    fn bigrams_no_trailing_newline() {
        // File that doesn't end with a blank line should still work.
        let conllu = "\
# sent_id = s1
1\tKoira\tkoira\tNOUN\tN\t_\t0\troot\t_\t_";
        let bigrams = parse_conllu_pos_bigrams(conllu);
        assert_eq!(bigrams[&(BOS.to_string(), "NOUN".to_string())], 1);
        assert_eq!(bigrams[&("NOUN".to_string(), EOS.to_string())], 1);
        assert_eq!(bigrams.len(), 2);
    }

    #[test]
    fn bigrams_single_token_sentence() {
        let conllu = "1\tKoira\tkoira\tNOUN\tN\t_\t0\troot\t_\t_\n\n";
        let bigrams = parse_conllu_pos_bigrams(conllu);
        // BOS -> NOUN -> EOS
        assert_eq!(bigrams[&(BOS.to_string(), "NOUN".to_string())], 1);
        assert_eq!(bigrams[&("NOUN".to_string(), EOS.to_string())], 1);
        assert_eq!(bigrams.len(), 2);
    }

    #[test]
    fn bigrams_empty_node_skipped() {
        // Empty nodes have decimal IDs like "1.1"
        let conllu = "\
1\tKoira\tkoira\tNOUN\tN\t_\t0\troot\t_\t_
1.1\tkoira\tkoira\tNOUN\tN\t_\t_\t_\t_\t_
2\tjuoksee\tjuosta\tVERB\tV\t_\t1\tnsubj\t_\t_

";
        let bigrams = parse_conllu_pos_bigrams(conllu);
        // Empty node 1.1 should be skipped: BOS->NOUN, NOUN->VERB, VERB->EOS
        assert_eq!(bigrams[&(BOS.to_string(), "NOUN".to_string())], 1);
        assert_eq!(bigrams[&("NOUN".to_string(), "VERB".to_string())], 1);
        assert_eq!(bigrams[&("VERB".to_string(), EOS.to_string())], 1);
        assert_eq!(bigrams.len(), 3);
    }

    #[test]
    fn bigrams_underscore_upos_skipped() {
        // If UPOS is "_" (unspecified), skip the token.
        let conllu = "\
1\tKoira\tkoira\tNOUN\tN\t_\t0\troot\t_\t_
2\tx\tx\t_\t_\t_\t_\t_\t_\t_
3\tjuoksee\tjuosta\tVERB\tV\t_\t1\tnsubj\t_\t_

";
        let bigrams = parse_conllu_pos_bigrams(conllu);
        // Token 2 has UPOS="_", so it's skipped: BOS->NOUN, NOUN->VERB, VERB->EOS
        assert_eq!(bigrams[&(BOS.to_string(), "NOUN".to_string())], 1);
        assert_eq!(bigrams[&("NOUN".to_string(), "VERB".to_string())], 1);
        assert_eq!(bigrams[&("VERB".to_string(), EOS.to_string())], 1);
        assert_eq!(bigrams.len(), 3);
    }

    // ─────────────────────────────────────────────────────────────
    // build_model_from_conllu
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn model_from_single_sentence() {
        let model = build_model_from_conllu(SINGLE_SENTENCE);
        // Should have weights derived from BOS->NOUN, NOUN->VERB, VERB->PUNCT, PUNCT->EOS
        assert!(model.num_weights() > 0);
    }

    #[test]
    fn model_from_two_sentences_reflects_counts() {
        let model = build_model_from_conllu(TWO_SENTENCES);

        // NOUN->VERB appears twice, ADJ->NOUN appears once.
        // So NOUN->VERB should have higher weight.
        let noun_verb = model.get_weight("NOUN", "VERB");
        let adj_noun = model.get_weight("ADJ", "NOUN");
        assert!(
            noun_verb > adj_noun,
            "NOUN->VERB ({}) should be higher than ADJ->NOUN ({}) due to count 2 vs 1",
            noun_verb,
            adj_noun
        );
    }

    #[test]
    fn model_from_empty_input() {
        let model = build_model_from_conllu("");
        assert_eq!(model.num_weights(), 0);
    }

    #[test]
    fn model_weights_are_negative() {
        let model = build_model_from_conllu(TWO_SENTENCES);
        // All log-probabilities should be negative.
        let noun_verb = model.get_weight("NOUN", "VERB");
        assert!(
            noun_verb < 0.0,
            "Expected negative log-prob, got {}",
            noun_verb
        );
    }

    // ─────────────────────────────────────────────────────────────
    // extract_emission_priors
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn emission_priors_single_sentence() {
        let priors = extract_emission_priors(SINGLE_SENTENCE);

        // "koira" -> NOUN with probability 1.0
        let koira_priors = &priors["koira"];
        assert_eq!(koira_priors.len(), 1);
        assert!((koira_priors["NOUN"] - 1.0).abs() < f64::EPSILON);

        // "juoksee" -> VERB with probability 1.0
        let juoksee_priors = &priors["juoksee"];
        assert_eq!(juoksee_priors.len(), 1);
        assert!((juoksee_priors["VERB"] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn emission_priors_case_insensitive() {
        // "Koira" and "koira" should be merged.
        let conllu = "\
1\tKoira\tkoira\tNOUN\tN\t_\t0\troot\t_\t_

1\tkoira\tkoira\tNOUN\tN\t_\t0\troot\t_\t_

";
        let priors = extract_emission_priors(conllu);
        // Both "Koira" and "koira" should be lowered to "koira"
        assert_eq!(priors["koira"]["NOUN"], 1.0);
    }

    #[test]
    fn emission_priors_ambiguous_word() {
        // "kuusi" can be NOUN or NUM
        let conllu = "\
1\tkuusi\tkuusi\tNOUN\tN\t_\t0\troot\t_\t_

1\tkuusi\tkuusi\tNOUN\tN\t_\t0\troot\t_\t_

1\tkuusi\tkuusi\tNUM\tNum\t_\t0\troot\t_\t_

";
        let priors = extract_emission_priors(conllu);
        let kuusi = &priors["kuusi"];
        assert_eq!(kuusi.len(), 2);
        // 2/3 NOUN, 1/3 NUM
        assert!((kuusi["NOUN"] - 2.0 / 3.0).abs() < f64::EPSILON);
        assert!((kuusi["NUM"] - 1.0 / 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn emission_priors_mwt_skipped() {
        let priors = extract_emission_priors(WITH_MWT);
        // The MWT surface "meninkö" should NOT appear as a word form.
        assert!(!priors.contains_key("meninkö"));
        // Sub-tokens should appear.
        assert!(priors.contains_key("menin"));
        assert!(priors.contains_key("kö"));
    }

    #[test]
    fn emission_priors_empty_input() {
        let priors = extract_emission_priors("");
        assert!(priors.is_empty());
    }

    #[test]
    fn emission_priors_probabilities_sum_to_one() {
        let priors = extract_emission_priors(TWO_SENTENCES);
        for (word, pos_probs) in &priors {
            let sum: f64 = pos_probs.values().sum();
            assert!(
                (sum - 1.0).abs() < 1e-10,
                "Probabilities for '{}' should sum to 1.0, got {}",
                word,
                sum
            );
        }
    }

    #[test]
    fn emission_priors_underscore_upos_skipped() {
        let conllu = "\
1\tKoira\tkoira\tNOUN\tN\t_\t0\troot\t_\t_
2\tx\tx\t_\t_\t_\t_\t_\t_\t_

";
        let priors = extract_emission_priors(conllu);
        assert!(priors.contains_key("koira"));
        // "x" with UPOS="_" should be skipped entirely
        assert!(!priors.contains_key("x"));
    }
}
