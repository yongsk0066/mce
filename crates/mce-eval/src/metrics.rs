//! Evaluation metrics for UPOS tagging and lemmatization.
//!
//! Computes:
//! - Overall accuracy (correct / total)
//! - Per-POS precision, recall, F1
//! - Confusion matrix (top N confusions)
//! - Lemma accuracy

use std::collections::HashMap;
use std::fmt;

/// A single prediction-vs-gold pair.
#[derive(Debug, Clone)]
pub struct TokenResult {
    /// Surface form of the word.
    pub form: String,
    /// Gold UPOS tag from the treebank.
    pub gold_upos: String,
    /// Predicted UPOS tag from MCE.
    pub pred_upos: String,
    /// Gold lemma from the treebank (if available).
    pub gold_lemma: String,
    /// Predicted lemma from MCE (baseform, if available).
    pub pred_lemma: String,
}

/// Aggregate evaluation results.
#[derive(Debug)]
pub struct EvalResults {
    /// Total tokens evaluated.
    pub total: usize,
    /// Correctly predicted UPOS tags.
    pub upos_correct: usize,
    /// Correctly predicted lemmas (case-insensitive).
    pub lemma_correct: usize,
    /// Tokens where MCE had no analysis (predicted "X").
    pub unknown_count: usize,
    /// Per-POS statistics: tag -> (true_pos, false_pos, false_neg).
    pub per_pos: HashMap<String, (usize, usize, usize)>,
    /// Confusion pairs: (gold, pred) -> count.
    pub confusions: HashMap<(String, String), usize>,
}

impl EvalResults {
    /// Create a new empty result set.
    pub fn new() -> Self {
        Self {
            total: 0,
            upos_correct: 0,
            lemma_correct: 0,
            unknown_count: 0,
            per_pos: HashMap::new(),
            confusions: HashMap::new(),
        }
    }

    /// Add a single token result.
    pub fn add(&mut self, result: &TokenResult) {
        self.total += 1;

        let correct = result.gold_upos == result.pred_upos;
        if correct {
            self.upos_correct += 1;
        }

        if result.pred_upos == "X" && result.gold_upos != "X" {
            self.unknown_count += 1;
        }

        // Lemma accuracy (case-insensitive comparison).
        if !result.gold_lemma.is_empty() && !result.pred_lemma.is_empty() {
            let gold_lower: String = result.gold_lemma.to_lowercase();
            let pred_lower: String = result.pred_lemma.to_lowercase();
            // Handle compound lemma notation: UD uses # for compounds (e.g., "viikon#loppu")
            // MCE may return the full word without #. Compare after removing #.
            let gold_clean: String = gold_lower.replace('#', "");
            let pred_clean: String = pred_lower.replace('#', "");
            if gold_clean == pred_clean {
                self.lemma_correct += 1;
            }
        }

        // Per-POS tracking.
        // True positive: gold == pred (counted for the gold tag).
        // False positive: pred is X but gold is not X (counted for pred tag).
        // False negative: gold is X but pred is not X (counted for gold tag).
        if correct {
            let entry = self
                .per_pos
                .entry(result.gold_upos.clone())
                .or_insert((0, 0, 0));
            entry.0 += 1; // TP
        } else {
            // FP for the predicted tag (it predicted this tag incorrectly).
            let fp_entry = self
                .per_pos
                .entry(result.pred_upos.clone())
                .or_insert((0, 0, 0));
            fp_entry.1 += 1; // FP

            // FN for the gold tag (it missed this tag).
            let fn_entry = self
                .per_pos
                .entry(result.gold_upos.clone())
                .or_insert((0, 0, 0));
            fn_entry.2 += 1; // FN
        }

        // Confusion matrix (only track errors).
        if !correct {
            let key = (result.gold_upos.clone(), result.pred_upos.clone());
            *self.confusions.entry(key).or_insert(0) += 1;
        }
    }

    /// Overall UPOS accuracy (0.0 to 1.0).
    pub fn upos_accuracy(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.upos_correct as f64 / self.total as f64
    }

    /// Lemma accuracy (0.0 to 1.0).
    pub fn lemma_accuracy(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.lemma_correct as f64 / self.total as f64
    }

    /// Coverage: proportion of tokens where MCE produced an analysis.
    pub fn coverage(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.total - self.unknown_count) as f64 / self.total as f64
    }

    /// Per-POS precision for a given tag.
    pub fn precision(&self, tag: &str) -> f64 {
        let (tp, fp, _fn) = self.per_pos.get(tag).copied().unwrap_or((0, 0, 0));
        let denom = tp + fp;
        if denom == 0 {
            0.0
        } else {
            tp as f64 / denom as f64
        }
    }

    /// Per-POS recall for a given tag.
    pub fn recall(&self, tag: &str) -> f64 {
        let (tp, _fp, fn_count) = self.per_pos.get(tag).copied().unwrap_or((0, 0, 0));
        let denom = tp + fn_count;
        if denom == 0 {
            0.0
        } else {
            tp as f64 / denom as f64
        }
    }

    /// Per-POS F1 for a given tag.
    pub fn f1(&self, tag: &str) -> f64 {
        let p = self.precision(tag);
        let r = self.recall(tag);
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }

    /// Top N confusion pairs, sorted by count (descending).
    pub fn top_confusions(&self, n: usize) -> Vec<((String, String), usize)> {
        let mut pairs: Vec<_> = self
            .confusions
            .iter()
            .map(|(k, &v)| (k.clone(), v))
            .collect();
        pairs.sort_by(|a, b| b.1.cmp(&a.1));
        pairs.truncate(n);
        pairs
    }

    /// All POS tags that appear in the results (sorted).
    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self.per_pos.keys().cloned().collect();
        tags.sort();
        tags
    }

    /// Support (total gold occurrences) for a given tag.
    pub fn support(&self, tag: &str) -> usize {
        let (tp, _fp, fn_count) = self.per_pos.get(tag).copied().unwrap_or((0, 0, 0));
        tp + fn_count
    }
}

impl Default for EvalResults {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EvalResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== MCE UPOS Evaluation Results ===")?;
        writeln!(f)?;

        // Overall metrics.
        writeln!(
            f,
            "UPOS Accuracy:   {:.2}% ({}/{})",
            self.upos_accuracy() * 100.0,
            self.upos_correct,
            self.total,
        )?;
        writeln!(
            f,
            "Lemma Accuracy:  {:.2}% ({}/{})",
            self.lemma_accuracy() * 100.0,
            self.lemma_correct,
            self.total,
        )?;
        writeln!(
            f,
            "Coverage:        {:.2}% ({}/{})",
            self.coverage() * 100.0,
            self.total - self.unknown_count,
            self.total,
        )?;
        writeln!(f)?;

        // Per-POS table.
        writeln!(
            f,
            "{:<8} {:>8} {:>8} {:>8} {:>8}",
            "POS", "Prec", "Recall", "F1", "Support"
        )?;
        writeln!(f, "{}", "-".repeat(44))?;

        let tags = self.all_tags();
        for tag in &tags {
            writeln!(
                f,
                "{:<8} {:>7.2}% {:>7.2}% {:>7.2}% {:>8}",
                tag,
                self.precision(tag) * 100.0,
                self.recall(tag) * 100.0,
                self.f1(tag) * 100.0,
                self.support(tag),
            )?;
        }
        writeln!(f)?;

        // Top confusions.
        let top = self.top_confusions(15);
        if !top.is_empty() {
            writeln!(f, "Top Confusions (gold -> pred):")?;
            for ((gold, pred), count) in &top {
                writeln!(f, "  {:<8} -> {:<8}  {}", gold, pred, count)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(
        form: &str,
        gold_upos: &str,
        pred_upos: &str,
        gold_lemma: &str,
        pred_lemma: &str,
    ) -> TokenResult {
        TokenResult {
            form: form.to_string(),
            gold_upos: gold_upos.to_string(),
            pred_upos: pred_upos.to_string(),
            gold_lemma: gold_lemma.to_string(),
            pred_lemma: pred_lemma.to_string(),
        }
    }

    #[test]
    fn empty_results() {
        let r = EvalResults::new();
        assert_eq!(r.total, 0);
        assert_eq!(r.upos_accuracy(), 0.0);
        assert_eq!(r.lemma_accuracy(), 0.0);
    }

    #[test]
    fn perfect_accuracy() {
        let mut r = EvalResults::new();
        r.add(&make_result("koira", "NOUN", "NOUN", "koira", "koira"));
        r.add(&make_result("juoksee", "VERB", "VERB", "juosta", "juosta"));

        assert_eq!(r.total, 2);
        assert_eq!(r.upos_correct, 2);
        assert_eq!(r.upos_accuracy(), 1.0);
        assert_eq!(r.lemma_accuracy(), 1.0);
    }

    #[test]
    fn partial_accuracy() {
        let mut r = EvalResults::new();
        r.add(&make_result("koira", "NOUN", "NOUN", "koira", "koira"));
        r.add(&make_result("iso", "ADJ", "NOUN", "iso", "iso")); // wrong UPOS
        r.add(&make_result("juoksee", "VERB", "VERB", "juosta", "juosta"));

        assert_eq!(r.total, 3);
        assert_eq!(r.upos_correct, 2);
        assert!((r.upos_accuracy() - 2.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn precision_recall_f1() {
        let mut r = EvalResults::new();
        // NOUN: 2 correct, 1 FP (pred NOUN but gold ADJ)
        r.add(&make_result("koira", "NOUN", "NOUN", "", ""));
        r.add(&make_result("kissa", "NOUN", "NOUN", "", ""));
        r.add(&make_result("iso", "ADJ", "NOUN", "", "")); // FP for NOUN, FN for ADJ

        // NOUN: TP=2, FP=1, FN=0 -> P=2/3, R=2/2=1.0
        assert!((r.precision("NOUN") - 2.0 / 3.0).abs() < 1e-10);
        assert!((r.recall("NOUN") - 1.0).abs() < 1e-10);

        // ADJ: TP=0, FP=0, FN=1 -> P=0, R=0
        assert_eq!(r.precision("ADJ"), 0.0);
        assert_eq!(r.recall("ADJ"), 0.0);
    }

    #[test]
    fn confusion_tracking() {
        let mut r = EvalResults::new();
        r.add(&make_result("iso", "ADJ", "NOUN", "", ""));
        r.add(&make_result("pieni", "ADJ", "NOUN", "", ""));
        r.add(&make_result("koira", "NOUN", "VERB", "", ""));

        let top = r.top_confusions(10);
        assert_eq!(top.len(), 2);
        // ADJ->NOUN should be first (count=2)
        assert_eq!(top[0].0, ("ADJ".to_string(), "NOUN".to_string()));
        assert_eq!(top[0].1, 2);
    }

    #[test]
    fn unknown_tracking() {
        let mut r = EvalResults::new();
        r.add(&make_result("foo", "NOUN", "X", "", ""));
        r.add(&make_result("bar", "VERB", "VERB", "", ""));
        assert_eq!(r.unknown_count, 1);
        assert!((r.coverage() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn lemma_compound_comparison() {
        let mut r = EvalResults::new();
        // UD notation uses # for compounds, MCE returns the full word.
        r.add(&make_result(
            "rautatieasema",
            "NOUN",
            "NOUN",
            "rauta#tie#asema",
            "rautatieasema",
        ));
        assert_eq!(r.lemma_correct, 1);
    }

    #[test]
    fn display_does_not_panic() {
        let mut r = EvalResults::new();
        r.add(&make_result("koira", "NOUN", "NOUN", "koira", "koira"));
        r.add(&make_result("iso", "ADJ", "NOUN", "iso", "iso"));
        let output = format!("{}", r);
        assert!(output.contains("UPOS Accuracy"));
        assert!(output.contains("Lemma Accuracy"));
    }

    #[test]
    fn support_counts_gold_occurrences() {
        let mut r = EvalResults::new();
        // 3 gold NOUN tokens: 2 correct, 1 missed (gold=NOUN, pred=ADJ)
        r.add(&make_result("koira", "NOUN", "NOUN", "", ""));
        r.add(&make_result("kissa", "NOUN", "NOUN", "", ""));
        r.add(&make_result("talo", "NOUN", "ADJ", "", "")); // FN for NOUN
        // 1 gold VERB token: correct
        r.add(&make_result("juoksee", "VERB", "VERB", "", ""));
        // 1 gold ADJ token: correct + 1 FP from above
        r.add(&make_result("iso", "ADJ", "ADJ", "", ""));

        // support(NOUN) = TP + FN = 2 + 1 = 3
        assert_eq!(r.support("NOUN"), 3);
        // support(VERB) = TP + FN = 1 + 0 = 1
        assert_eq!(r.support("VERB"), 1);
        // support(ADJ) = TP + FN = 1 + 0 = 1
        // Note: the FP on ADJ (from talo NOUN->ADJ) does not affect support.
        assert_eq!(r.support("ADJ"), 1);
    }

    #[test]
    fn support_unknown_tag_is_zero() {
        let r = EvalResults::new();
        assert_eq!(r.support("NONEXISTENT"), 0);
    }

    #[test]
    fn all_tags_sorted() {
        let mut r = EvalResults::new();
        r.add(&make_result("a", "VERB", "VERB", "", ""));
        r.add(&make_result("b", "ADJ", "ADJ", "", ""));
        r.add(&make_result("c", "NOUN", "NOUN", "", ""));
        let tags = r.all_tags();
        assert_eq!(tags, vec!["ADJ", "NOUN", "VERB"]);
    }

    #[test]
    fn f1_calculation() {
        let mut r = EvalResults::new();
        // NOUN: TP=2, FP=1, FN=1 -> P=2/3, R=2/3, F1=2/3
        r.add(&make_result("a", "NOUN", "NOUN", "", ""));
        r.add(&make_result("b", "NOUN", "NOUN", "", ""));
        r.add(&make_result("c", "ADJ", "NOUN", "", "")); // FP for NOUN
        r.add(&make_result("d", "NOUN", "VERB", "", "")); // FN for NOUN
        r.add(&make_result("e", "VERB", "VERB", "", ""));

        let p = r.precision("NOUN");
        let rc = r.recall("NOUN");
        let f = r.f1("NOUN");
        assert!((p - 2.0 / 3.0).abs() < 1e-10);
        assert!((rc - 2.0 / 3.0).abs() < 1e-10);
        assert!((f - 2.0 / 3.0).abs() < 1e-10); // 2 * (2/3) * (2/3) / (2/3 + 2/3) = 2/3
    }

    #[test]
    fn f1_zero_when_no_tp() {
        let mut r = EvalResults::new();
        // ADJ: TP=0, FP=0, FN=1
        r.add(&make_result("iso", "ADJ", "NOUN", "", ""));
        assert_eq!(r.f1("ADJ"), 0.0);
    }

    #[test]
    fn lemma_case_insensitive_comparison() {
        let mut r = EvalResults::new();
        // Gold "Helsinki", pred "helsinki" -> should match (case-insensitive).
        r.add(&make_result(
            "Helsinki", "PROPN", "PROPN", "Helsinki", "helsinki",
        ));
        assert_eq!(r.lemma_correct, 1);
    }

    #[test]
    fn lemma_empty_gold_not_counted() {
        let mut r = EvalResults::new();
        // When gold_lemma is empty, lemma comparison is skipped.
        r.add(&make_result("foo", "NOUN", "NOUN", "", "koira"));
        assert_eq!(r.lemma_correct, 0);
    }

    #[test]
    fn lemma_empty_pred_not_counted() {
        let mut r = EvalResults::new();
        // When pred_lemma is empty, lemma comparison is skipped.
        r.add(&make_result("foo", "NOUN", "NOUN", "koira", ""));
        assert_eq!(r.lemma_correct, 0);
    }

    #[test]
    fn top_confusions_truncated() {
        let mut r = EvalResults::new();
        // Create 3 different confusion types.
        r.add(&make_result("a", "ADJ", "NOUN", "", ""));
        r.add(&make_result("b", "ADJ", "NOUN", "", ""));
        r.add(&make_result("c", "VERB", "NOUN", "", ""));
        r.add(&make_result("d", "PROPN", "NOUN", "", ""));

        // top_confusions(2) should return only the top 2.
        let top = r.top_confusions(2);
        assert_eq!(top.len(), 2);
        // ADJ->NOUN should be first (count=2).
        assert_eq!(top[0].0, ("ADJ".to_string(), "NOUN".to_string()));
        assert_eq!(top[0].1, 2);
    }

    #[test]
    fn top_confusions_empty_when_all_correct() {
        let mut r = EvalResults::new();
        r.add(&make_result("koira", "NOUN", "NOUN", "", ""));
        r.add(&make_result("juoksee", "VERB", "VERB", "", ""));
        let top = r.top_confusions(10);
        assert!(top.is_empty());
    }

    #[test]
    fn default_is_same_as_new() {
        let r1 = EvalResults::new();
        let r2 = EvalResults::default();
        assert_eq!(r1.total, r2.total);
        assert_eq!(r1.upos_correct, r2.upos_correct);
        assert_eq!(r1.lemma_correct, r2.lemma_correct);
        assert_eq!(r1.unknown_count, r2.unknown_count);
    }

    #[test]
    fn x_gold_and_x_pred_not_unknown() {
        let mut r = EvalResults::new();
        // When gold is also X, it's not counted as "unknown" — the condition
        // only triggers when pred==X and gold!=X.
        r.add(&make_result("???", "X", "X", "", ""));
        assert_eq!(r.unknown_count, 0);
        assert_eq!(r.upos_correct, 1);
    }

    #[test]
    fn coverage_full_when_no_unknowns() {
        let mut r = EvalResults::new();
        r.add(&make_result("a", "NOUN", "NOUN", "", ""));
        r.add(&make_result("b", "VERB", "ADJ", "", "")); // wrong but not unknown
        assert!((r.coverage() - 1.0).abs() < 1e-10);
    }
}
