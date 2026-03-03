// Integration tests for FinnishCompoundAnalyzer with real VFST dictionary.
//
// These tests require the MCE_DICT_PATH environment variable pointing to the
// directory containing mor.vfst (e.g., data/).
//
// Run with:
//   MCE_DICT_PATH=data cargo test -p mce-fi -- --ignored

use mce_fi::compound::FinnishCompoundAnalyzer;

fn load_compound_analyzer() -> FinnishCompoundAnalyzer {
    let dict_dir =
        std::env::var("MCE_DICT_PATH").expect("MCE_DICT_PATH must be set for integration tests");
    let mor_path = std::path::Path::new(&dict_dir).join("mor.vfst");
    let data = std::fs::read(&mor_path).expect("Failed to read mor.vfst");
    FinnishCompoundAnalyzer::from_bytes(&data).expect("Failed to create compound analyzer")
}

// ---------------------------------------------------------------------------
// rautatieasema: classic three-part Finnish compound
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn rautatieasema_splits() {
    let analyzer = load_compound_analyzer();
    let splits = analyzer.analyze("rautatieasema");

    assert!(
        !splits.is_empty(),
        "rautatieasema should have compound splits"
    );

    // Collect all sets of word parts across splits.
    let all_word_parts: Vec<Vec<&str>> = splits
        .iter()
        .map(|s| s.word_parts().iter().map(|p| p.surface.as_str()).collect())
        .collect();

    // Should contain rauta+tie+asema or rautatie+asema (or both).
    let has_three_part = all_word_parts
        .iter()
        .any(|wp| wp == &["rauta", "tie", "asema"]);
    let has_two_part = all_word_parts.iter().any(|wp| wp == &["rautatie", "asema"]);

    assert!(
        has_three_part || has_two_part,
        "rautatieasema should split as rauta+tie+asema or rautatie+asema, got: {:?}",
        all_word_parts
    );
}

#[test]
#[ignore]
fn rautatieasema_is_compound() {
    let analyzer = load_compound_analyzer();
    assert!(
        analyzer.is_compound("rautatieasema"),
        "rautatieasema should be recognized as a compound"
    );
}

// ---------------------------------------------------------------------------
// kissanpentu: compound with linking element -n-
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn kissanpentu_splits_with_linking() {
    let analyzer = load_compound_analyzer();
    let splits = analyzer.analyze("kissanpentu");

    assert!(
        !splits.is_empty(),
        "kissanpentu should have compound splits"
    );

    let best = &splits[0];
    let word_parts: Vec<&str> = best
        .word_parts()
        .iter()
        .map(|p| p.surface.as_str())
        .collect();
    // The compound may split as ["kissan", "pentu"] (kissan is genitive of kissa,
    // a valid word form) or ["kissa", "pentu"] with linking -n-.
    // Both are linguistically valid.
    let valid = word_parts == vec!["kissa", "pentu"] || word_parts == vec!["kissan", "pentu"];
    assert!(
        valid,
        "kissanpentu should split as kissa+pentu or kissan+pentu, got: {:?}",
        word_parts
    );

    // If split as kissa+n+pentu, verify linking element.
    // If split as kissan+pentu, no linking element needed.
    if word_parts == vec!["kissa", "pentu"] {
        let linking: Vec<&str> = best
            .parts
            .iter()
            .filter(|p| p.is_linking)
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(linking, vec!["n"], "should have linking element 'n'");
    }
}

// ---------------------------------------------------------------------------
// koira: single word, not a compound
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn koira_not_compound() {
    let analyzer = load_compound_analyzer();
    let splits = analyzer.analyze("koira");

    assert!(
        splits.is_empty(),
        "koira is a single word, should have no compound splits"
    );

    assert!(
        !analyzer.is_compound("koira"),
        "koira should not be recognized as a compound"
    );
}

// ---------------------------------------------------------------------------
// maa-alue: hyphenated compound
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn maa_alue_hyphenated_compound() {
    let analyzer = load_compound_analyzer();
    let splits = analyzer.analyze("maa-alue");

    assert!(!splits.is_empty(), "maa-alue should have compound splits");

    let best = &splits[0];
    let word_parts: Vec<&str> = best
        .word_parts()
        .iter()
        .map(|p| p.surface.as_str())
        .collect();
    assert_eq!(
        word_parts,
        vec!["maa", "alue"],
        "maa-alue should split as maa + alue"
    );

    // The hyphen should be a linking element.
    let has_hyphen_link = best.parts.iter().any(|p| p.is_linking && p.surface == "-");
    assert!(has_hyphen_link, "maa-alue should have hyphen as linking");
}

// ---------------------------------------------------------------------------
// xyzqwerty: nonsense word, no splits
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn xyzqwerty_no_splits() {
    let analyzer = load_compound_analyzer();
    let splits = analyzer.analyze("xyzqwerty");

    assert!(
        splits.is_empty(),
        "xyzqwerty should have no compound splits"
    );

    assert!(
        !analyzer.is_compound("xyzqwerty"),
        "xyzqwerty should not be a compound"
    );
}

// ---------------------------------------------------------------------------
// Additional: verify penalty ordering
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn splits_sorted_by_penalty() {
    let analyzer = load_compound_analyzer();
    let splits = analyzer.analyze("rautatieasema");

    if splits.len() >= 2 {
        for window in splits.windows(2) {
            assert!(
                window[0].penalty <= window[1].penalty,
                "splits should be sorted by penalty (ascending): {} > {}",
                window[0].penalty,
                window[1].penalty
            );
        }
    }
}
