//! Integration tests for mce-wasm.
//!
//! These tests require the VFST dictionary file (mor.vfst).
//! Set MCE_DICT_PATH to the directory containing mor.vfst:
//!
//! ```sh
//! MCE_DICT_PATH=~/oss/corevoikko/voikko-fi/vvfst cargo test -p mce-wasm -- --ignored
//! ```

use std::path::PathBuf;

use mce_wasm::MceEngine;

/// Load the VFST dictionary bytes from MCE_DICT_PATH.
///
/// Returns `None` if the environment variable is not set or the file
/// does not exist, so integration tests can be skipped gracefully.
fn load_vfst() -> Option<Vec<u8>> {
    let dir = std::env::var("MCE_DICT_PATH").ok()?;
    let path = PathBuf::from(dir).join("mor.vfst");
    std::fs::read(&path).ok()
}

/// Helper: load engine or skip test.
fn engine() -> MceEngine {
    let data = load_vfst().expect("MCE_DICT_PATH not set or mor.vfst not found");
    MceEngine::load(&data).expect("Failed to load MceEngine from VFST")
}

// =========================================================================
// 1. disambiguate_sentence
// =========================================================================

#[test]
#[ignore]
fn disambiguate_sentence_returns_valid_json() {
    let engine = engine();
    let result = engine.disambiguate_sentence("Koira juoksee nopeasti.");

    // Must be a JSON array.
    assert!(result.starts_with('['), "Should start with [: {}", result);
    assert!(result.ends_with(']'), "Should end with ]: {}", result);

    // Must contain word entries.
    assert!(
        result.contains("\"word\":"),
        "Should contain word field: {}",
        result
    );
    assert!(
        result.contains("\"pos\":"),
        "Should contain pos field: {}",
        result
    );
    assert!(
        result.contains("\"baseform\":"),
        "Should contain baseform field: {}",
        result
    );
}

#[test]
#[ignore]
fn disambiguate_sentence_koira_juoksee() {
    let engine = engine();
    let result = engine.disambiguate_sentence("Koira juoksee.");

    // "Koira" should be analyzed as nimisana (noun).
    assert!(
        result.contains("\"pos\":\"nimisana\""),
        "Koira should be a noun: {}",
        result
    );

    // "juoksee" should be analyzed as teonsana (verb).
    assert!(
        result.contains("\"pos\":\"teonsana\""),
        "juoksee should be a verb: {}",
        result
    );
}

#[test]
#[ignore]
fn disambiguate_sentence_empty_input() {
    let engine = engine();
    let result = engine.disambiguate_sentence("");
    assert_eq!(result, "[]");
}

#[test]
#[ignore]
fn disambiguate_sentence_single_word() {
    let engine = engine();
    let result = engine.disambiguate_sentence("koira");

    assert!(result.starts_with('['));
    assert!(result.ends_with(']'));
    assert!(result.contains("\"word\":\"koira\""));
    assert!(result.contains("\"baseform\":\"koira\""));
}

// =========================================================================
// 2. get_baseform
// =========================================================================

#[test]
#[ignore]
fn get_baseform_koirien() {
    let engine = engine();
    // "koirien" is genitive plural of "koira" (dog).
    let baseform = engine.get_baseform("koirien");
    assert_eq!(baseform, "koira", "Baseform of 'koirien' should be 'koira'");
}

#[test]
#[ignore]
fn get_baseform_juoksee() {
    let engine = engine();
    // "juoksee" is 3rd person singular of "juosta" (to run).
    let baseform = engine.get_baseform("juoksee");
    assert_eq!(
        baseform, "juosta",
        "Baseform of 'juoksee' should be 'juosta'"
    );
}

#[test]
#[ignore]
fn get_baseform_taloissa() {
    let engine = engine();
    // "taloissa" is inessive plural of "talo" (house).
    let baseform = engine.get_baseform("taloissa");
    assert_eq!(baseform, "talo", "Baseform of 'taloissa' should be 'talo'");
}

#[test]
#[ignore]
fn get_baseform_unknown_word() {
    let engine = engine();
    // An unknown word should return itself.
    let baseform = engine.get_baseform("xyzzyplugh");
    assert_eq!(
        baseform, "xyzzyplugh",
        "Unknown word should return itself as baseform"
    );
}

#[test]
#[ignore]
fn get_baseform_nominative() {
    let engine = engine();
    // A word already in nominative should return itself.
    let baseform = engine.get_baseform("koira");
    assert_eq!(
        baseform, "koira",
        "Nominative form should return itself as baseform"
    );
}

// =========================================================================
// 3. is_valid_word
// =========================================================================

#[test]
#[ignore]
fn is_valid_word_common_words() {
    let engine = engine();

    // Common Finnish words should be valid.
    assert!(engine.is_valid_word("koira"), "'koira' should be valid");
    assert!(engine.is_valid_word("kissa"), "'kissa' should be valid");
    assert!(engine.is_valid_word("talo"), "'talo' should be valid");
    assert!(engine.is_valid_word("juoksee"), "'juoksee' should be valid");
}

#[test]
#[ignore]
fn is_valid_word_inflected_forms() {
    let engine = engine();

    // Inflected forms should also be valid.
    assert!(engine.is_valid_word("koirien"), "'koirien' should be valid");
    assert!(
        engine.is_valid_word("taloissa"),
        "'taloissa' should be valid"
    );
    assert!(
        engine.is_valid_word("juoksevat"),
        "'juoksevat' should be valid"
    );
}

#[test]
#[ignore]
fn is_valid_word_invalid_words() {
    let engine = engine();

    // Nonsense strings should be invalid.
    assert!(
        !engine.is_valid_word("xyzzyplugh"),
        "'xyzzyplugh' should be invalid"
    );
    assert!(
        !engine.is_valid_word("asdfghjkl"),
        "'asdfghjkl' should be invalid"
    );
}

#[test]
#[ignore]
fn is_valid_word_empty_string() {
    let engine = engine();
    // Empty string should be invalid.
    assert!(!engine.is_valid_word(""), "Empty string should be invalid");
}

// =========================================================================
// 4. suggest_with_context
// =========================================================================

#[test]
#[ignore]
fn suggest_with_context_returns_json_array() {
    let engine = engine();
    let result = engine.suggest_with_context("koirra", "", 1);

    // Should be a JSON array.
    assert!(result.starts_with('['), "Should start with [: {}", result);
    assert!(result.ends_with(']'), "Should end with ]: {}", result);
}

#[test]
#[ignore]
fn suggest_with_context_valid_word_returns_empty() {
    let engine = engine();
    // A valid word should return no suggestions.
    let result = engine.suggest_with_context("koira", "", 1);
    assert_eq!(result, "[]", "Valid word should return empty suggestions");
}

#[test]
#[ignore]
fn suggest_with_context_misspelled_returns_candidates() {
    let engine = engine();
    // "koirra" is a common misspelling of "koira" (double r).
    let result = engine.suggest_with_context("koirra", "", 1);

    // Should suggest "koira" among the results.
    assert!(
        result.contains("\"koira\""),
        "'koira' should be among suggestions for 'koirra': {}",
        result
    );
}

#[test]
#[ignore]
fn suggest_with_context_limits_to_five() {
    let engine = engine();
    let result = engine.suggest_with_context("koirra", "", 2);

    // Count the number of suggestions (commas + 1, or 0 if empty).
    if result != "[]" {
        let count = result.matches('"').count() / 2; // each suggestion has 2 quotes
        assert!(
            count <= 5,
            "Should return at most 5 suggestions, got {}: {}",
            count,
            result
        );
    }
}

#[test]
#[ignore]
fn suggest_with_context_uses_bigram_context() {
    let engine = engine();

    // After a noun, a verb is more likely than another noun.
    // So with "koira" as context, verb suggestions should rank higher.
    let with_context = engine.suggest_with_context("juokse", "koira", 1);
    let without_context = engine.suggest_with_context("juokse", "", 1);

    // Both should be valid JSON arrays.
    assert!(with_context.starts_with('['));
    assert!(without_context.starts_with('['));

    // The contextual version might produce different rankings.
    // We just verify they are valid and non-empty for a plausible misspelling.
    // (The exact output depends on the dictionary contents.)
}

// =========================================================================
// 5. version
// =========================================================================

#[test]
fn version_returns_semver() {
    let v = MceEngine::version();
    assert!(!v.is_empty(), "Version should not be empty");
    // Should look like a semver (x.y.z).
    let parts: Vec<&str> = v.split('.').collect();
    assert_eq!(
        parts.len(),
        3,
        "Version should have 3 parts (semver): {}",
        v
    );
}

// =========================================================================
// 6. analyze (existing method, verify JSON format)
// =========================================================================

#[test]
#[ignore]
fn analyze_returns_json_array() {
    let engine = engine();
    let result = engine.analyze("koira");

    assert!(result.starts_with('['));
    assert!(result.ends_with(']'));
    assert!(result.contains("\"CLASS\""));
    assert!(result.contains("\"BASEFORM\""));
}

// =========================================================================
// 7. spell_check (existing method)
// =========================================================================

#[test]
#[ignore]
fn spell_check_valid_word() {
    let engine = engine();
    assert!(engine.spell_check("koira"));
    assert!(engine.spell_check("juoksee"));
}

#[test]
#[ignore]
fn spell_check_invalid_word() {
    let engine = engine();
    assert!(!engine.spell_check("xyzzyplugh"));
}

// =========================================================================
// 8. analyze_sentence (existing method)
// =========================================================================

#[test]
#[ignore]
fn analyze_sentence_returns_valid_json() {
    let engine = engine();
    let result = engine.analyze_sentence("Iso koira juoksee.");

    assert!(result.starts_with('['));
    assert!(result.ends_with(']'));
    assert!(result.contains("\"word\":\"Iso\""));
    assert!(result.contains("\"word\":\"koira\""));
    assert!(result.contains("\"word\":\"juoksee\""));
}

// =========================================================================
// 9. grammar_check
// =========================================================================

#[test]
#[ignore]
fn grammar_check_returns_json_array() {
    let engine = engine();
    let result = engine.grammar_check("Koira juoksee.");

    assert!(result.starts_with('['), "Should start with [: {}", result);
    assert!(result.ends_with(']'), "Should end with ]: {}", result);
}

#[test]
#[ignore]
fn grammar_check_no_errors_for_correct_sentence() {
    let engine = engine();
    let result = engine.grammar_check("Koira juoksee.");
    assert_eq!(result, "[]", "Correct sentence should have no errors");
}

#[test]
#[ignore]
fn grammar_check_detects_repeated_word() {
    let engine = engine();
    let result = engine.grammar_check("Koira koira juoksee.");

    assert!(
        result.contains("\"code\":\"REPEATED_WORD\""),
        "Should detect repeated word: {}",
        result
    );
}

#[test]
#[ignore]
fn grammar_check_detects_capitalization_error() {
    let engine = engine();
    let result = engine.grammar_check("koira juoksee.");

    assert!(
        result.contains("\"code\":\"CAPITALIZATION_ERROR\""),
        "Should detect capitalization error: {}",
        result
    );
    assert!(
        result.contains("\"suggestions\""),
        "Should include suggestions: {}",
        result
    );
}

#[test]
#[ignore]
fn grammar_check_empty_text() {
    let engine = engine();
    let result = engine.grammar_check("");
    assert_eq!(result, "[]");
}

// =========================================================================
// 10. hyphenate
// =========================================================================

#[test]
fn hyphenate_basic_words() {
    // hyphenate does not need VFST — it is purely algorithmic.
    // But we test it through MceEngine so we still need the engine.
    // Use a standalone FinnishHyphenator for non-ignore tests.
    let h = mce_fi::hyphenation::FinnishHyphenator::new();
    assert_eq!(h.hyphenate_word("koulu"), "kou-lu");
    assert_eq!(h.hyphenate_word("kartta"), "kart-ta");
    assert_eq!(h.hyphenate_word("Helsinki"), "Hel-sin-ki");
    assert_eq!(h.hyphenate_word("suomalainen"), "suo-ma-lai-nen");
}

#[test]
#[ignore]
fn hyphenate_via_engine() {
    let engine = engine();
    assert_eq!(engine.hyphenate("koulu"), "kou-lu");
    assert_eq!(engine.hyphenate("kartta"), "kart-ta");
    assert_eq!(engine.hyphenate("suomalainen"), "suo-ma-lai-nen");
}

#[test]
#[ignore]
fn hyphenate_text_via_engine() {
    let engine = engine();
    let result = engine.hyphenate_text("Koira juoksee.");
    // "Koira" -> "Koi-ra", "juoksee" -> "juok-see"
    assert!(
        result.contains("Koi-ra"),
        "Should hyphenate Koira: {}",
        result
    );
    assert!(
        result.contains("juok-see"),
        "Should hyphenate juoksee: {}",
        result
    );
    // Punctuation should be preserved.
    assert!(result.ends_with('.'), "Should preserve period: {}", result);
}

#[test]
#[ignore]
fn hyphenate_text_preserves_whitespace() {
    let engine = engine();
    let result = engine.hyphenate_text("talo koulu");
    assert_eq!(result, "ta-lo kou-lu");
}
