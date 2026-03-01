// Integration tests for FinnishAnalyzer with real VFST dictionary.
//
// These tests require the MCE_DICT_PATH environment variable pointing to the
// directory containing mor.vfst (e.g., ~/oss/corevoikko/voikko-fi/vvfst/).
//
// Run with:
//   MCE_DICT_PATH=~/oss/corevoikko/voikko-fi/vvfst cargo test -p mce-fi -- --ignored

use mce_core::analysis::{
    ATTR_BASEFORM, ATTR_CLASS, ATTR_FSTOUTPUT, ATTR_NUMBER, ATTR_SIJAMUOTO, ATTR_STRUCTURE,
};
use mce_fi::morphology::{Analyzer, FinnishAnalyzer};

fn load_analyzer() -> FinnishAnalyzer {
    let dict_dir =
        std::env::var("MCE_DICT_PATH").expect("MCE_DICT_PATH must be set for integration tests");
    let mor_path = std::path::Path::new(&dict_dir).join("mor.vfst");
    let data = std::fs::read(&mor_path).expect("Failed to read mor.vfst");
    FinnishAnalyzer::from_bytes(&data).expect("Failed to create analyzer")
}

#[test]
#[ignore]
fn analyze_koira() {
    let analyzer = load_analyzer();
    let word: Vec<char> = "koira".chars().collect();
    let results = analyzer.analyze(&word, word.len());

    assert!(!results.is_empty(), "koira should have analyses");

    let has_nimisana = results
        .iter()
        .any(|a| a.get(ATTR_CLASS) == Some("nimisana"));
    assert!(has_nimisana, "koira should be a noun (nimisana)");

    let nominative = results
        .iter()
        .find(|a| a.get(ATTR_SIJAMUOTO) == Some("nimento"));
    assert!(nominative.is_some(), "koira should have nominative form");

    if let Some(a) = nominative {
        assert_eq!(a.get(ATTR_NUMBER), Some("singular"));
        assert_eq!(a.get(ATTR_BASEFORM), Some("koira"));
    }
}

#[test]
#[ignore]
fn analyze_koirien() {
    let analyzer = load_analyzer();
    let word: Vec<char> = "koirien".chars().collect();
    let results = analyzer.analyze(&word, word.len());

    assert!(!results.is_empty(), "koirien should have analyses");

    let genitive_plural = results
        .iter()
        .find(|a| a.get(ATTR_SIJAMUOTO) == Some("omanto") && a.get(ATTR_NUMBER) == Some("plural"));
    assert!(
        genitive_plural.is_some(),
        "koirien should have genitive plural"
    );

    if let Some(a) = genitive_plural {
        assert_eq!(a.get(ATTR_BASEFORM), Some("koira"));
    }
}

#[test]
#[ignore]
fn analyze_juoksen() {
    let analyzer = load_analyzer();
    let word: Vec<char> = "juoksen".chars().collect();
    let results = analyzer.analyze(&word, word.len());

    assert!(!results.is_empty(), "juoksen should have analyses");

    let verb = results
        .iter()
        .find(|a| a.get(ATTR_CLASS) == Some("teonsana"));
    assert!(verb.is_some(), "juoksen should be a verb");
}

#[test]
#[ignore]
fn analyze_compound_kissanpentu() {
    let analyzer = load_analyzer();
    let word: Vec<char> = "kissanpentu".chars().collect();
    let results = analyzer.analyze(&word, word.len());

    assert!(
        !results.is_empty(),
        "kissanpentu should have analyses as a compound word"
    );

    // Check STRUCTURE has compound boundary marker
    let has_compound = results
        .iter()
        .any(|a| a.get(ATTR_STRUCTURE).map_or(false, |s| s.contains("=p")));
    assert!(has_compound, "kissanpentu should have compound structure");
}

#[test]
#[ignore]
fn analyze_nonword_returns_empty() {
    let analyzer = load_analyzer();
    let word: Vec<char> = "xyzqwerty".chars().collect();
    let results = analyzer.analyze(&word, word.len());

    assert!(
        results.is_empty(),
        "nonword should have no analyses, got {}",
        results.len()
    );
}

#[test]
#[ignore]
fn analyze_full_morphology_has_fstoutput() {
    let analyzer = load_analyzer();
    let word: Vec<char> = "koira".chars().collect();
    let results = analyzer.analyze_full(&word, word.len(), true);

    assert!(!results.is_empty());
    let first = &results[0];
    assert!(
        first.get(ATTR_FSTOUTPUT).is_some(),
        "full morphology should set FSTOUTPUT"
    );
}

#[test]
#[ignore]
fn analyze_partial_morphology_no_fstoutput() {
    let analyzer = load_analyzer();
    let word: Vec<char> = "koira".chars().collect();
    let results = analyzer.analyze_full(&word, word.len(), false);

    assert!(!results.is_empty());
    let first = &results[0];
    assert!(
        first.get(ATTR_FSTOUTPUT).is_none(),
        "partial morphology should not set FSTOUTPUT"
    );
}
