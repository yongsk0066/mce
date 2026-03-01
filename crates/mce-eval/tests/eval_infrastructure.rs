//! Integration test for the evaluation infrastructure.
//!
//! Tests the full pipeline: CoNLL-U parsing -> metrics computation -> reporting.
//! Does NOT require MCE_DICT_PATH since it uses mock predictions.

use mce_eval::conllu::parse_conllu;
use mce_eval::metrics::{EvalResults, TokenResult};
use mce_eval::pos_map::{mce_class_to_upos, mce_to_upos};

use mce_core::analysis::Analysis;

// ---------------------------------------------------------------------------
// Hand-crafted CoNLL-U test data (4 Finnish sentences)
// ---------------------------------------------------------------------------

const TEST_CONLLU: &str = "\
# sent_id = eval.1
# text = Koira juoksee nopeasti.
1\tKoira\tkoira\tNOUN\tN\tCase=Nom|Number=Sing\t2\tnsubj\t2:nsubj\t_
2\tjuoksee\tjuosta\tVERB\tV\tMood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin|Voice=Act\t0\troot\t0:root\t_
3\tnopeasti\tnopeasti\tADV\tAdv\t_\t2\tadvmod\t2:advmod\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t2\tpunct\t2:punct\t_

# sent_id = eval.2
# text = Iso kissa nukkuu sohvalla.
1\tIso\tiso\tADJ\tA\tCase=Nom|Degree=Pos|Number=Sing\t2\tamod\t2:amod\t_
2\tkissa\tkissa\tNOUN\tN\tCase=Nom|Number=Sing\t3\tnsubj\t3:nsubj\t_
3\tnukkuu\tnukkua\tVERB\tV\tMood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin|Voice=Act\t0\troot\t0:root\t_
4\tsohvalla\tsohva\tNOUN\tN\tCase=Ade|Number=Sing\t3\tobl\t3:obl\tSpaceAfter=No
5\t.\t.\tPUNCT\tPunct\t_\t3\tpunct\t3:punct\t_

# sent_id = eval.3
# text = Helsinki on kaunis kaupunki.
1\tHelsinki\tHelsinki\tPROPN\tN\tCase=Nom|Number=Sing\t4\tnsubj:cop\t4:nsubj:cop\t_
2\ton\tolla\tAUX\tV\tMood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin|Voice=Act\t4\tcop\t4:cop\t_
3\tkaunis\tkaunis\tADJ\tA\tCase=Nom|Degree=Pos|Number=Sing\t4\tamod\t4:amod\t_
4\tkaupunki\tkaupunki\tNOUN\tN\tCase=Nom|Number=Sing\t0\troot\t0:root\tSpaceAfter=No
5\t.\t.\tPUNCT\tPunct\t_\t4\tpunct\t4:punct\t_

# sent_id = eval.4
# text = Kolme kissaa ja koira.
1\tKolme\tkolme\tNUM\tNum\tCase=Nom|Number=Sing|NumType=Card\t2\tnummod\t2:nummod\t_
2\tkissaa\tkissa\tNOUN\tN\tCase=Par|Number=Sing\t0\troot\t0:root\t_
3\tja\tja\tCCONJ\tC\t_\t4\tcc\t4:cc\t_
4\tkoira\tkoira\tNOUN\tN\tCase=Nom|Number=Sing\t2\tconj\t2:conj\tSpaceAfter=No
5\t.\t.\tPUNCT\tPunct\t_\t2\tpunct\t2:punct\t_
";

// ---------------------------------------------------------------------------
// CoNLL-U parsing tests
// ---------------------------------------------------------------------------

#[test]
fn parse_conllu_four_sentences() {
    let sentences = parse_conllu(TEST_CONLLU);
    assert_eq!(sentences.len(), 4, "Should parse 4 sentences");

    // Sentence 1: Koira juoksee nopeasti.
    assert_eq!(sentences[0].sent_id, "eval.1");
    assert_eq!(sentences[0].tokens.len(), 4);
    assert_eq!(sentences[0].tokens[0].form, "Koira");
    assert_eq!(sentences[0].tokens[0].upos, "NOUN");
    assert_eq!(sentences[0].tokens[0].lemma, "koira");

    // Sentence 2: Iso kissa nukkuu sohvalla.
    assert_eq!(sentences[1].sent_id, "eval.2");
    assert_eq!(sentences[1].tokens.len(), 5);

    // Sentence 3: Helsinki on kaunis kaupunki.
    assert_eq!(sentences[2].tokens[0].upos, "PROPN");
    assert_eq!(sentences[2].tokens[1].upos, "AUX");

    // Sentence 4: Kolme kissaa ja koira.
    assert_eq!(sentences[3].tokens[0].upos, "NUM");
    assert_eq!(sentences[3].tokens[2].upos, "CCONJ");
}

// ---------------------------------------------------------------------------
// POS mapping tests
// ---------------------------------------------------------------------------

#[test]
fn pos_mapping_round_trip() {
    // Verify all MCE classes map to valid UPOS tags.
    let classes = vec![
        ("nimisana", "NOUN"),
        ("laatusana", "ADJ"),
        ("teonsana", "VERB"),
        ("seikkasana", "ADV"),
        ("asemosana", "PRON"),
        ("lukusana", "NUM"),
        ("sidesana", "CCONJ"),
        ("suhdesana", "ADP"),
        ("huudahdussana", "INTJ"),
        ("kieltosana", "AUX"),
        ("etunimi", "PROPN"),
        ("sukunimi", "PROPN"),
        ("paikannimi", "PROPN"),
        ("nimisana_laatusana", "NOUN"),
        ("lyhenne", "NOUN"),
    ];

    for (class, expected_upos) in &classes {
        let mut a = Analysis::new();
        a.set("CLASS", *class);
        let upos = mce_class_to_upos(&a);
        assert_eq!(
            upos, *expected_upos,
            "CLASS '{}' should map to UPOS '{}'",
            class, expected_upos
        );
    }
}

#[test]
fn conjunction_disambiguation() {
    let mut a = Analysis::new();
    a.set("CLASS", "sidesana");

    // Coordinating conjunctions
    assert_eq!(mce_to_upos(&a, "ja"), "CCONJ");
    assert_eq!(mce_to_upos(&a, "tai"), "CCONJ");
    assert_eq!(mce_to_upos(&a, "mutta"), "CCONJ");
    assert_eq!(mce_to_upos(&a, "sekä"), "CCONJ");
    assert_eq!(mce_to_upos(&a, "Ja"), "CCONJ"); // case-insensitive

    // Subordinating conjunctions
    assert_eq!(mce_to_upos(&a, "että"), "SCONJ");
    assert_eq!(mce_to_upos(&a, "kun"), "SCONJ");
    assert_eq!(mce_to_upos(&a, "jos"), "SCONJ");
    assert_eq!(mce_to_upos(&a, "koska"), "SCONJ");
    assert_eq!(mce_to_upos(&a, "vaikka"), "SCONJ");
}

// ---------------------------------------------------------------------------
// Metrics computation tests
// ---------------------------------------------------------------------------

fn make_token_result(
    form: &str,
    gold: &str,
    pred: &str,
    gold_lemma: &str,
    pred_lemma: &str,
) -> TokenResult {
    TokenResult {
        form: form.to_string(),
        gold_upos: gold.to_string(),
        pred_upos: pred.to_string(),
        gold_lemma: gold_lemma.to_string(),
        pred_lemma: pred_lemma.to_string(),
    }
}

#[test]
fn metrics_full_scenario() {
    let mut results = EvalResults::new();

    // Sentence 1: "Koira juoksee nopeasti" — all correct
    results.add(&make_token_result(
        "Koira", "NOUN", "NOUN", "koira", "koira",
    ));
    results.add(&make_token_result(
        "juoksee", "VERB", "VERB", "juosta", "juosta",
    ));
    results.add(&make_token_result(
        "nopeasti", "ADV", "ADV", "nopeasti", "nopeasti",
    ));

    // Sentence 2: "iso kissa nukkuu" — 1 error (iso tagged as NOUN)
    results.add(&make_token_result("iso", "ADJ", "NOUN", "iso", "iso")); // error
    results.add(&make_token_result(
        "kissa", "NOUN", "NOUN", "kissa", "kissa",
    ));
    results.add(&make_token_result(
        "nukkuu", "VERB", "VERB", "nukkua", "nukkua",
    ));

    // Sentence 3: "Helsinki on kaunis kaupunki" — 1 unknown
    results.add(&make_token_result("Helsinki", "PROPN", "X", "Helsinki", "")); // unknown
    results.add(&make_token_result("on", "AUX", "AUX", "olla", "olla"));
    results.add(&make_token_result(
        "kaunis", "ADJ", "ADJ", "kaunis", "kaunis",
    ));
    results.add(&make_token_result(
        "kaupunki", "NOUN", "NOUN", "kaupunki", "kaupunki",
    ));

    // Total: 10 tokens, 8 correct UPOS, 1 error (ADJ->NOUN), 1 unknown (PROPN->X)
    assert_eq!(results.total, 10);
    assert_eq!(results.upos_correct, 8);
    assert!((results.upos_accuracy() - 0.8).abs() < 1e-10);

    // Unknown tracking
    assert_eq!(results.unknown_count, 1);
    assert!((results.coverage() - 0.9).abs() < 1e-10);

    // Per-POS: NOUN should have P=3/4 (3 TP + 1 FP from iso), R=3/3=1.0
    assert!((results.precision("NOUN") - 0.75).abs() < 1e-10);
    assert!((results.recall("NOUN") - 1.0).abs() < 1e-10);

    // ADJ: TP=1, FP=0, FN=1 -> P=1.0, R=0.5
    assert!((results.precision("ADJ") - 1.0).abs() < 1e-10);
    assert!((results.recall("ADJ") - 0.5).abs() < 1e-10);

    // Confusions: ADJ->NOUN (1), PROPN->X (1)
    let top = results.top_confusions(5);
    assert_eq!(top.len(), 2);

    // Display should work
    let output = format!("{}", results);
    assert!(output.contains("80.00%"), "UPOS accuracy should be 80%");
    assert!(output.contains("Coverage"));
}

#[test]
fn metrics_lemma_with_compound_notation() {
    let mut results = EvalResults::new();

    // UD uses # for compounds, MCE returns full word
    results.add(&make_token_result(
        "rautatieasema",
        "NOUN",
        "NOUN",
        "rauta#tie#asema",
        "rautatieasema",
    ));
    results.add(&make_token_result(
        "viikonloppu",
        "NOUN",
        "NOUN",
        "viikon#loppu",
        "viikonloppu",
    ));

    assert_eq!(results.lemma_correct, 2);
    assert!((results.lemma_accuracy() - 1.0).abs() < 1e-10);
}

// ---------------------------------------------------------------------------
// End-to-end infrastructure test (without dictionary)
// ---------------------------------------------------------------------------

#[test]
fn evaluation_display_format() {
    let mut results = EvalResults::new();

    // Simulate a small evaluation run.
    let pairs = vec![
        ("Koira", "NOUN", "NOUN"),
        ("juoksee", "VERB", "VERB"),
        ("nopeasti", "ADV", "ADV"),
        ("iso", "ADJ", "NOUN"), // error
        ("ja", "CCONJ", "CCONJ"),
        ("Helsinki", "PROPN", "X"), // unknown
    ];

    for (form, gold, pred) in &pairs {
        results.add(&TokenResult {
            form: form.to_string(),
            gold_upos: gold.to_string(),
            pred_upos: pred.to_string(),
            gold_lemma: String::new(),
            pred_lemma: String::new(),
        });
    }

    let output = format!("{}", results);

    // Verify key sections are present
    assert!(output.contains("MCE UPOS Evaluation Results"));
    assert!(output.contains("UPOS Accuracy"));
    assert!(output.contains("Coverage"));
    assert!(output.contains("Top Confusions"));

    // Verify accuracy calculation: 4/6 = 66.67%
    assert_eq!(results.upos_correct, 4);
    assert_eq!(results.total, 6);
}

// ---------------------------------------------------------------------------
// CoNLL-U parsing edge cases
// ---------------------------------------------------------------------------

#[test]
fn conllu_with_multiword_tokens() {
    let content = "\
# sent_id = mwt.1
# text = Ellei tule.
1-2\tEllei\t_\t_\t_\t_\t_\t_\t_\t_
1\tEll\tella\tSCONJ\tC\t_\t3\tmark\t3:mark\t_
2\tei\tei\tAUX\tV\tNumber=Sing|Person=3|VerbForm=Fin|Voice=Act\t3\taux\t3:aux\t_
3\ttule\ttulla\tVERB\tV\tConnegative=Yes|Mood=Ind|Tense=Pres|VerbForm=Fin\t0\troot\t0:root\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t3\tpunct\t3:punct\t_
";
    let sentences = parse_conllu(content);
    assert_eq!(sentences.len(), 1);

    // Multi-word token line (1-2) should be skipped.
    // Should have tokens with IDs 1, 2, 3, 4.
    let ids: Vec<usize> = sentences[0].tokens.iter().map(|t| t.id).collect();
    assert_eq!(ids, vec![1, 2, 3, 4]);

    // Verify the tokens are correct sub-words, not the range token.
    assert_eq!(sentences[0].tokens[0].form, "Ell");
    assert_eq!(sentences[0].tokens[1].form, "ei");
}

#[test]
fn conllu_empty_feats() {
    let content = "\
# sent_id = feat.1
# text = Hei!
1\tHei\thei\tINTJ\tI\t_\t0\troot\t0:root\tSpaceAfter=No
2\t!\t!\tPUNCT\tPunct\t_\t1\tpunct\t1:punct\t_
";
    let sentences = parse_conllu(content);
    assert_eq!(sentences[0].tokens[0].feats, "_");
    assert_eq!(sentences[0].tokens[0].upos, "INTJ");
}
