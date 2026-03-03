//! Integration tests for mce-fst using real VFST dictionary files from corevoikko.
//!
//! These tests are `#[ignore]` by default so they don't fail in CI without dictionaries.
//!
//! To run:
//! ```sh
//! MCE_DICT_PATH=/path/to/corevoikko/voikko-fi/vvfst cargo test -p mce-fst -- --ignored
//! ```

use std::path::PathBuf;

use mce_fst::Transducer;
use mce_fst::unweighted::UnweightedTransducer;

/// Returns the dictionary path from the `MCE_DICT_PATH` environment variable.
fn dict_path() -> Option<PathBuf> {
    std::env::var("MCE_DICT_PATH").ok().map(PathBuf::from)
}

/// Loads `mor.vfst` as an `UnweightedTransducer`.
fn load_mor() -> UnweightedTransducer {
    let path = dict_path().expect("Set MCE_DICT_PATH to run integration tests");
    let mor_path = path.join("mor.vfst");
    let data = std::fs::read(&mor_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", mor_path.display()));
    UnweightedTransducer::from_bytes(&data)
        .unwrap_or_else(|e| panic!("Failed to load mor.vfst: {e}"))
}

/// Loads `autocorr.vfst` as an `UnweightedTransducer`.
fn load_autocorr() -> UnweightedTransducer {
    let path = dict_path().expect("Set MCE_DICT_PATH to run integration tests");
    let ac_path = path.join("autocorr.vfst");
    let data = std::fs::read(&ac_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", ac_path.display()));
    UnweightedTransducer::from_bytes(&data)
        .unwrap_or_else(|e| panic!("Failed to load autocorr.vfst: {e}"))
}

/// Collects all FST outputs for a given input word.
fn analyze(transducer: &UnweightedTransducer, word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let mut config = transducer.new_config(2000);
    transducer.prepare(&mut config, &chars);

    let mut results = Vec::new();
    let mut output = String::new();
    while transducer.next(&mut config, &mut output) {
        results.push(output.clone());
    }
    results
}

// ---------------------------------------------------------------------------
// Test: Load mor.vfst and verify structural properties
// ---------------------------------------------------------------------------
#[test]
#[ignore]
fn load_mor_vfst() {
    let t = load_mor();
    let symbols = t.symbols();

    // mor.vfst is ~4MB, should have a substantial number of symbols
    assert!(
        symbols.symbol_strings.len() > 100,
        "Expected >100 symbols, got {}",
        symbols.symbol_strings.len()
    );

    // Should have flag diacritics (Finnish morphology uses them)
    assert!(
        t.flag_feature_count() > 0,
        "Expected flag diacritics in mor.vfst"
    );

    // first_normal_char should be set (flags come before normal chars)
    assert!(
        symbols.first_normal_char > 1,
        "Expected first_normal_char > 1 (flags precede normal chars), got {}",
        symbols.first_normal_char
    );
}

// ---------------------------------------------------------------------------
// Test: Analyze "koira" (dog) — a simple noun
// ---------------------------------------------------------------------------
#[test]
#[ignore]
fn analyze_koira() {
    let t = load_mor();
    let results = analyze(&t, "koira");

    assert!(
        !results.is_empty(),
        "Expected at least one analysis for 'koira', got none"
    );

    // At least one output should contain the lemma "koira"
    let has_koira = results.iter().any(|r| r.contains("koira"));
    assert!(
        has_koira,
        "Expected an analysis containing 'koira', got: {results:?}"
    );
}

// ---------------------------------------------------------------------------
// Test: Analyze "kissoja" (cats, partitive plural)
// ---------------------------------------------------------------------------
#[test]
#[ignore]
fn analyze_kissoja() {
    let t = load_mor();
    let results = analyze(&t, "kissoja");

    assert!(
        !results.is_empty(),
        "Expected at least one analysis for 'kissoja', got none"
    );

    // The output should contain morphological tags (VFST uses bracket notation like [Ln])
    let has_tags = results.iter().any(|r| r.contains('['));
    assert!(
        has_tags,
        "Expected morphological tags in output for 'kissoja', got: {results:?}"
    );

    // Should reference the base form "kissa"
    let has_kissa = results.iter().any(|r| r.contains("kissa"));
    assert!(
        has_kissa,
        "Expected base form 'kissa' in analysis of 'kissoja', got: {results:?}"
    );
}

// ---------------------------------------------------------------------------
// Test: Analyze a nonsense word — should produce no output
// ---------------------------------------------------------------------------
#[test]
#[ignore]
fn analyze_nonword() {
    let t = load_mor();
    let results = analyze(&t, "asdfghjkl");

    assert!(
        results.is_empty(),
        "Expected no analysis for 'asdfghjkl', got: {results:?}"
    );
}

// ---------------------------------------------------------------------------
// Test: Analyze "kissanpentu" (kitten, compound word: kissa + pentu)
// ---------------------------------------------------------------------------
#[test]
#[ignore]
fn analyze_compound_word() {
    let t = load_mor();
    let results = analyze(&t, "kissanpentu");

    assert!(
        !results.is_empty(),
        "Expected at least one analysis for 'kissanpentu' (compound word), got none"
    );

    // Compound word analysis should reference both components
    let has_kissa = results.iter().any(|r| r.contains("kissa"));
    let has_pentu = results.iter().any(|r| r.contains("pentu"));
    assert!(
        has_kissa && has_pentu,
        "Expected compound components 'kissa' and 'pentu' in analysis of 'kissanpentu', got: {results:?}"
    );
}

// ---------------------------------------------------------------------------
// Test: Load autocorr.vfst — verify it loads successfully
// ---------------------------------------------------------------------------
#[test]
#[ignore]
fn load_autocorr_vfst() {
    let t = load_autocorr();
    let symbols = t.symbols();

    // autocorr.vfst is ~11KB, should have some symbols
    assert!(
        symbols.symbol_strings.len() > 10,
        "Expected >10 symbols in autocorr.vfst, got {}",
        symbols.symbol_strings.len()
    );
}
