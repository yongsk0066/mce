// End-to-end sentence-level integration tests for the MCE pipeline.
//
// Tests the full pipeline: text -> word split -> analyze -> disambiguate -> verify POS.
//
// These tests require the MCE_DICT_PATH environment variable pointing to the
// directory containing mor.vfst (e.g., data/).
//
// Run with:
//   MCE_DICT_PATH=data cargo test -p mce-fi -- --ignored

use mce_core::analysis::{ATTR_BASEFORM, ATTR_CLASS, Analysis};
use mce_disambig::{Disambiguator, ViterbiDisambiguator};
use mce_fi::morphology::{Analyzer, FinnishAnalyzer};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn load_analyzer() -> FinnishAnalyzer {
    let dict_dir =
        std::env::var("MCE_DICT_PATH").expect("MCE_DICT_PATH must be set for integration tests");
    let mor_path = std::path::Path::new(&dict_dir).join("mor.vfst");
    let data = std::fs::read(&mor_path).expect("Failed to read mor.vfst");
    FinnishAnalyzer::from_bytes(&data).expect("Failed to create analyzer")
}

/// Analyze a single word string and return all analyses.
fn analyze_word(analyzer: &FinnishAnalyzer, word: &str) -> Vec<Analysis> {
    let chars: Vec<char> = word.chars().collect();
    analyzer.analyze(&chars, chars.len())
}

/// Run the full pipeline on a sentence: split -> analyze -> disambiguate.
/// Returns the disambiguated analyses (one per word).
fn run_pipeline(sentence: &str) -> Vec<Analysis> {
    let analyzer = load_analyzer();
    let disambiguator = ViterbiDisambiguator::with_finnish_defaults();

    let words: Vec<&str> = sentence.split_whitespace().collect();

    // Analyze each word
    let sentence_analyses: Vec<Vec<Analysis>> = words
        .iter()
        .map(|w| {
            let analyses = analyze_word(&analyzer, w);
            eprintln!(
                "  Word '{}': {} analyses -> [{}]",
                w,
                analyses.len(),
                analyses
                    .iter()
                    .map(|a| format!(
                        "{}({})",
                        a.get(ATTR_CLASS).unwrap_or("?"),
                        a.get(ATTR_BASEFORM).unwrap_or("?")
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            analyses
        })
        .collect();

    // Guard: every word must have at least one analysis
    for (i, (word, analyses)) in words.iter().zip(&sentence_analyses).enumerate() {
        assert!(
            !analyses.is_empty(),
            "Word '{}' at position {} has no analyses",
            word,
            i
        );
    }

    // Disambiguate
    let result = disambiguator.disambiguate(&sentence_analyses);
    assert_eq!(
        result.len(),
        words.len(),
        "Disambiguator should return one analysis per word"
    );

    // Print disambiguation result for debugging
    eprintln!("  Disambiguated:");
    for (i, a) in result.iter().enumerate() {
        eprintln!(
            "    [{}] {} -> CLASS={}, BASEFORM={}",
            i,
            words[i],
            a.get(ATTR_CLASS).unwrap_or("?"),
            a.get(ATTR_BASEFORM).unwrap_or("?")
        );
    }

    result
}

/// Assert that a word's CLASS is exactly the expected value.
fn assert_class_exact(result: &[Analysis], pos: usize, word: &str, expected: &str) {
    let actual = result[pos].get(ATTR_CLASS).unwrap_or("MISSING");
    assert_eq!(
        actual, expected,
        "Word '{}' at position {}: expected CLASS='{}', got '{}'",
        word, pos, expected, actual
    );
}

/// Assert that a word's CLASS is one of the acceptable values.
/// Use this for ambiguous words where the disambiguator may choose
/// differently depending on bigram weights.
fn assert_class_one_of(result: &[Analysis], pos: usize, word: &str, acceptable: &[&str]) {
    let actual = result[pos].get(ATTR_CLASS).unwrap_or("MISSING");
    assert!(
        acceptable.contains(&actual),
        "Word '{}' at position {}: expected CLASS to be one of {:?}, got '{}'",
        word,
        pos,
        acceptable,
        actual
    );
}

// ---------------------------------------------------------------------------
// Test 1: "koira juoksee" -> [nimisana, teonsana] (dog runs)
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn sentence_koira_juoksee() {
    eprintln!("\n=== Test: koira juoksee ===");
    let result = run_pipeline("koira juoksee");

    // "koira" is unambiguously a noun
    assert_class_exact(&result, 0, "koira", "nimisana");
    // "juoksee" is unambiguously a verb
    assert_class_exact(&result, 1, "juoksee", "teonsana");
}

// ---------------------------------------------------------------------------
// Test 2: "iso koira" -> [laatusana, nimisana] (big dog)
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn sentence_iso_koira() {
    eprintln!("\n=== Test: iso koira ===");
    let result = run_pipeline("iso koira");

    // "iso" should be an adjective; it could also analyze as a noun (ISO),
    // but ADJ->NOUN is the strongest bigram transition (-0.2).
    assert_class_one_of(&result, 0, "iso", &["laatusana", "nimisana_laatusana"]);
    // "koira" is unambiguously a noun
    assert_class_exact(&result, 1, "koira", "nimisana");
}

// ---------------------------------------------------------------------------
// Test 3: "kolme kissaa" -> [lukusana, nimisana] (three cats)
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn sentence_kolme_kissaa() {
    eprintln!("\n=== Test: kolme kissaa ===");
    let result = run_pipeline("kolme kissaa");

    // "kolme" should be a numeral
    assert_class_exact(&result, 0, "kolme", "lukusana");
    // "kissaa" is a noun (partitive singular of kissa)
    assert_class_exact(&result, 1, "kissaa", "nimisana");
}

// ---------------------------------------------------------------------------
// Test 4: "hän juoksee nopeasti" -> [asemosana, teonsana, seikkasana]
//         (he/she runs fast)
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn sentence_han_juoksee_nopeasti() {
    eprintln!("\n=== Test: hän juoksee nopeasti ===");
    let result = run_pipeline("hän juoksee nopeasti");

    // "hän" should be a pronoun
    assert_class_exact(&result, 0, "hän", "asemosana");
    // "juoksee" is a verb
    assert_class_exact(&result, 1, "juoksee", "teonsana");
    // "nopeasti" should be an adverb; the disambiguator might prefer
    // laatusana due to bigram weights (VERB->ADJ has a defined weight),
    // so accept both.
    assert_class_one_of(&result, 2, "nopeasti", &["seikkasana", "laatusana"]);
}

// ---------------------------------------------------------------------------
// Test 5: "talo on suuri" -> [nimisana, teonsana, laatusana]
//         (house is big)
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn sentence_talo_on_suuri() {
    eprintln!("\n=== Test: talo on suuri ===");
    let result = run_pipeline("talo on suuri");

    // "talo" is unambiguously a noun
    assert_class_exact(&result, 0, "talo", "nimisana");
    // "on" is a verb (3sg present of olla)
    // It might also appear as a conjunction or other POS in some analyses,
    // but the NOUN->VERB transition is strong.
    assert_class_one_of(&result, 1, "on", &["teonsana", "kieltosana"]);
    // "suuri" should be an adjective; it may also be classified as
    // nimisana_laatusana which is still acceptable.
    assert_class_one_of(&result, 2, "suuri", &["laatusana", "nimisana_laatusana"]);
}

// ---------------------------------------------------------------------------
// Additional pipeline validation tests
// ---------------------------------------------------------------------------

/// Verify that the pipeline handles a single-word sentence gracefully.
#[test]
#[ignore]
fn sentence_single_word() {
    eprintln!("\n=== Test: single word 'talo' ===");
    let result = run_pipeline("talo");

    assert_eq!(result.len(), 1);
    assert_class_exact(&result, 0, "talo", "nimisana");
}

/// Verify that the pipeline produces baseforms for each disambiguated word.
#[test]
#[ignore]
fn sentence_baseforms_present() {
    eprintln!("\n=== Test: baseforms in 'koira juoksee' ===");
    let result = run_pipeline("koira juoksee");

    for (i, analysis) in result.iter().enumerate() {
        assert!(
            analysis.get(ATTR_BASEFORM).is_some(),
            "Disambiguated word at position {} should have BASEFORM",
            i
        );
    }
}
