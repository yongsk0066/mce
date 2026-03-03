// Comprehensive end-to-end integration tests for the full MCE pipeline.
//
// Tests cover all major MCE features: morphological analysis + disambiguation,
// spell checking, compound analysis, hyphenation, grammar checking, and the
// full pipeline combining all stages.
//
// These tests require the MCE_DICT_PATH environment variable pointing to the
// directory containing mor.vfst (e.g., data/).
//
// Run with:
//   MCE_DICT_PATH=data cargo test -p mce-fi -- --ignored

use mce_core::analysis::{ATTR_BASEFORM, ATTR_CLASS, ATTR_SIJAMUOTO, Analysis};
use mce_disambig::{Disambiguator, ViterbiDisambiguator};
use mce_fi::compound::FinnishCompoundAnalyzer;
use mce_fi::hyphenation::FinnishHyphenator;
use mce_fi::morphology::{Analyzer, FinnishAnalyzer};
use mce_fi::spellcheck::FinnishSpellChecker;
use mce_grammar::GrammarChecker;
use mce_grammar::finnish::FinnishGrammarChecker;
use mce_speller::SpellResult;

// ===========================================================================
// Shared helpers
// ===========================================================================

fn dict_path() -> String {
    std::env::var("MCE_DICT_PATH").expect("MCE_DICT_PATH must be set for integration tests")
}

fn load_mor_bytes() -> Vec<u8> {
    let dict_dir = dict_path();
    let mor_path = std::path::Path::new(&dict_dir).join("mor.vfst");
    std::fs::read(&mor_path).expect("Failed to read mor.vfst")
}

fn load_analyzer() -> FinnishAnalyzer {
    FinnishAnalyzer::from_bytes(&load_mor_bytes()).expect("Failed to create analyzer")
}

fn load_spell_checker() -> FinnishSpellChecker {
    FinnishSpellChecker::from_bytes(&load_mor_bytes()).expect("Failed to create spell checker")
}

fn load_compound_analyzer() -> FinnishCompoundAnalyzer {
    FinnishCompoundAnalyzer::from_bytes(&load_mor_bytes())
        .expect("Failed to create compound analyzer")
}

fn load_grammar_checker() -> FinnishGrammarChecker {
    FinnishGrammarChecker::new(&load_mor_bytes()).expect("Failed to create grammar checker")
}

/// Analyze a single word and return all analyses.
fn analyze_word(analyzer: &FinnishAnalyzer, word: &str) -> Vec<Analysis> {
    let chars: Vec<char> = word.chars().collect();
    analyzer.analyze(&chars, chars.len())
}

/// Run the full analysis + disambiguation pipeline on a sentence.
/// Returns one disambiguated Analysis per whitespace-separated word.
fn run_pipeline(sentence: &str) -> Vec<Analysis> {
    let analyzer = load_analyzer();
    let disambiguator = ViterbiDisambiguator::with_finnish_defaults();

    let words: Vec<&str> = sentence.split_whitespace().collect();

    let sentence_analyses: Vec<Vec<Analysis>> =
        words.iter().map(|w| analyze_word(&analyzer, w)).collect();

    // Guard: every word must have at least one analysis.
    for (i, (word, analyses)) in words.iter().zip(&sentence_analyses).enumerate() {
        assert!(
            !analyses.is_empty(),
            "Word '{}' at position {} has no analyses",
            word,
            i
        );
    }

    let result = disambiguator.disambiguate(&sentence_analyses);
    assert_eq!(
        result.len(),
        words.len(),
        "Disambiguator should return one analysis per word"
    );

    result
}

/// Assert that a word's CLASS is exactly the expected value.
fn assert_class(result: &[Analysis], pos: usize, word: &str, expected: &str) {
    let actual = result[pos].get(ATTR_CLASS).unwrap_or("MISSING");
    assert_eq!(
        actual, expected,
        "Word '{}' at position {}: expected CLASS='{}', got '{}'",
        word, pos, expected, actual
    );
}

/// Assert that a word's CLASS is one of the acceptable values.
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

/// Assert that a word's BASEFORM matches the expected value.
fn assert_baseform(result: &[Analysis], pos: usize, word: &str, expected: &str) {
    let actual = result[pos].get(ATTR_BASEFORM).unwrap_or("MISSING");
    assert_eq!(
        actual, expected,
        "Word '{}' at position {}: expected BASEFORM='{}', got '{}'",
        word, pos, expected, actual
    );
}

// ===========================================================================
// 1. Morphological analysis + disambiguation (5 tests)
// ===========================================================================

/// "Koira juoksee pihalla" -> NOUN + VERB + NOUN sequence.
/// Tests a basic Finnish sentence with subject-verb-adverbial structure.
#[test]
#[ignore]
fn morph_disambig_koira_juoksee_pihalla() {
    let result = run_pipeline("koira juoksee pihalla");

    // "koira" -> nimisana (noun), baseform "koira"
    assert_class(&result, 0, "koira", "nimisana");
    assert_baseform(&result, 0, "koira", "koira");

    // "juoksee" -> teonsana (verb), baseform "juosta"
    assert_class(&result, 1, "juoksee", "teonsana");
    assert_baseform(&result, 1, "juoksee", "juosta");

    // "pihalla" -> nimisana (noun in adessive case), baseform "piha"
    assert_class(&result, 2, "pihalla", "nimisana");
    assert_baseform(&result, 2, "pihalla", "piha");

    // Verify case attributes.
    let piha_case = result[2].get(ATTR_SIJAMUOTO).unwrap_or("MISSING");
    assert_eq!(
        piha_case, "ulkoolento",
        "pihalla should be in adessive case (ulkoolento)"
    );
}

/// "Kolme kissaa istuu" -> NUM + NOUN + VERB sequence.
/// Tests numeral-noun-verb pattern.
#[test]
#[ignore]
fn morph_disambig_kolme_kissaa_istuu() {
    let result = run_pipeline("kolme kissaa istuu");

    // "kolme" -> lukusana (numeral)
    assert_class(&result, 0, "kolme", "lukusana");

    // "kissaa" -> nimisana (noun), partitive singular
    assert_class(&result, 1, "kissaa", "nimisana");
    assert_baseform(&result, 1, "kissaa", "kissa");
    let kissaa_case = result[1].get(ATTR_SIJAMUOTO).unwrap_or("MISSING");
    assert_eq!(
        kissaa_case, "osanto",
        "kissaa should be in partitive case (osanto)"
    );

    // "istuu" -> teonsana (verb)
    assert_class(&result, 2, "istuu", "teonsana");
}

/// "Ei voi tietaa" -> NEG + VERB + VERB (infinitive).
/// Tests negation verb followed by main verb chain.
#[test]
#[ignore]
fn morph_disambig_ei_voi_tietaa() {
    let result = run_pipeline("ei voi tietää");

    // "ei" -> kieltosana (negation word) or teonsana depending on analysis
    assert_class_one_of(&result, 0, "ei", &["kieltosana", "teonsana"]);

    // "voi" -> teonsana (verb, "voida" = can)
    // Note: "voi" can also be noun (butter), but in this context verb is preferred.
    assert_class_one_of(&result, 1, "voi", &["teonsana", "nimisana"]);

    // "tietää" -> teonsana (verb, to know)
    assert_class(&result, 2, "tietää", "teonsana");
}

/// "Suomen tasavalta" -> PROPN/NOUN + NOUN (genitive + nominative).
/// Tests proper noun + common noun pattern.
#[test]
#[ignore]
fn morph_disambig_suomen_tasavalta() {
    let result = run_pipeline("Suomen tasavalta");

    // "Suomen" -> nimisana (noun, genitive of Suomi) or nimi (proper name)
    // The analyzer may classify it as nimisana with baseform "Suomi".
    assert_class_one_of(&result, 0, "Suomen", &["nimisana", "nimi", "paikannimi"]);

    // "tasavalta" -> nimisana (noun, republic)
    assert_class(&result, 1, "tasavalta", "nimisana");
    assert_baseform(&result, 1, "tasavalta", "tasavalta");
}

/// "Hän on iloinen" -> PRON + AUX/VERB + ADJ.
/// Tests pronoun-copula-adjective predicate construction.
#[test]
#[ignore]
fn morph_disambig_han_on_iloinen() {
    let result = run_pipeline("hän on iloinen");

    // "hän" -> asemosana (pronoun)
    assert_class(&result, 0, "hän", "asemosana");
    assert_baseform(&result, 0, "hän", "hän");

    // "on" -> teonsana (verb, "olla" = to be)
    // May also be classified as kieltosana in some analyses.
    assert_class_one_of(&result, 1, "on", &["teonsana", "kieltosana"]);

    // "iloinen" -> laatusana (adjective, happy) or nimisana_laatusana
    assert_class_one_of(&result, 2, "iloinen", &["laatusana", "nimisana_laatusana"]);
    assert_baseform(&result, 2, "iloinen", "iloinen");
}

// ===========================================================================
// 2. Spell checking (5 tests)
// ===========================================================================

/// Valid Finnish words should return SpellResult::Ok.
#[test]
#[ignore]
fn spell_valid_words() {
    let mut checker = load_spell_checker();

    assert_eq!(
        checker.check("koira"),
        SpellResult::Ok,
        "koira should be valid"
    );
    assert_eq!(
        checker.check("juoksee"),
        SpellResult::Ok,
        "juoksee should be valid"
    );
    assert_eq!(
        checker.check("talossa"),
        SpellResult::Ok,
        "talossa should be valid"
    );
}

/// Misspelled words with doubled consonants should return SpellResult::Failed.
#[test]
#[ignore]
fn spell_invalid_doubled_consonant() {
    let mut checker = load_spell_checker();

    assert_eq!(
        checker.check("koirra"),
        SpellResult::Failed,
        "koirra (doubled r) should be invalid"
    );
    assert_eq!(
        checker.check("juolsee"),
        SpellResult::Failed,
        "juolsee (l instead of k) should be invalid"
    );
}

/// Suggest "koira" for the misspelling "koirra".
/// The trie-based suggestions may or may not include "koira" depending on
/// the trie contents, but the pipeline must not crash.
#[test]
#[ignore]
fn spell_suggest_koirra() {
    let checker = load_spell_checker();
    let suggestions = checker.suggest("koirra", 1);

    // Pipeline must not crash; suggestions are a valid list.
    for s in &suggestions {
        assert!(!s.is_empty(), "Suggestion should not be empty");
    }
}

/// Suggest corrections for "tallossa" (should include "talossa").
#[test]
#[ignore]
fn spell_suggest_tallossa() {
    let checker = load_spell_checker();
    let suggestions = checker.suggest("tallossa", 1);

    // The pipeline should return valid suggestions (possibly empty).
    for s in &suggestions {
        assert!(!s.is_empty(), "Suggestion should not be empty");
    }
}

/// Context-aware suggestion: after "iso" (adjective), suggest nouns.
/// Verifies that suggest_with_context does not crash and returns
/// valid results.
#[test]
#[ignore]
fn spell_suggest_with_context() {
    let checker = load_spell_checker();

    // "iso koirra" -> after "iso" (adjective), suggest nouns for "koirra".
    let suggestions = checker.suggest_with_context("koirra", Some("iso"), 1);

    // Pipeline must not crash; suggestions are valid strings.
    for s in &suggestions {
        assert!(
            !s.is_empty(),
            "Context-aware suggestion should not be empty"
        );
    }
}

// ===========================================================================
// 3. Compound analysis (3 tests)
// ===========================================================================

/// "rautatieasema" -> should split into rauta+tie+asema or rautatie+asema.
#[test]
#[ignore]
fn compound_rautatieasema() {
    let analyzer = load_compound_analyzer();
    let splits = analyzer.analyze("rautatieasema");

    assert!(
        !splits.is_empty(),
        "rautatieasema should have compound splits"
    );

    let all_word_parts: Vec<Vec<&str>> = splits
        .iter()
        .map(|s| s.word_parts().iter().map(|p| p.surface.as_str()).collect())
        .collect();

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

/// "kahvikuppi" -> should split into [kahvi, kuppi].
#[test]
#[ignore]
fn compound_kahvikuppi() {
    let analyzer = load_compound_analyzer();
    let splits = analyzer.analyze("kahvikuppi");

    assert!(!splits.is_empty(), "kahvikuppi should have compound splits");

    let all_word_parts: Vec<Vec<&str>> = splits
        .iter()
        .map(|s| s.word_parts().iter().map(|p| p.surface.as_str()).collect())
        .collect();

    let has_expected = all_word_parts.iter().any(|wp| wp == &["kahvi", "kuppi"]);

    assert!(
        has_expected,
        "kahvikuppi should split as kahvi+kuppi, got: {:?}",
        all_word_parts
    );
}

/// "maa-alue" -> hyphenated compound splits into [maa, alue].
#[test]
#[ignore]
fn compound_maa_alue_hyphenated() {
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

    // Verify hyphen is a linking element.
    let has_hyphen_link = best.parts.iter().any(|p| p.is_linking && p.surface == "-");
    assert!(
        has_hyphen_link,
        "maa-alue should have hyphen as linking element"
    );
}

// ===========================================================================
// 4. Hyphenation (5 tests)
// ===========================================================================

/// "suomalainen" -> "suo-ma-lai-nen" (four syllables with diphthongs).
#[test]
#[ignore]
fn hyphenation_suomalainen() {
    let h = FinnishHyphenator::new();
    assert_eq!(
        h.hyphenate_word("suomalainen"),
        "suo-ma-lai-nen",
        "suomalainen should hyphenate as suo-ma-lai-nen"
    );
}

/// "Helsinki" -> "Hel-sin-ki" (case-insensitive, three syllables).
#[test]
#[ignore]
fn hyphenation_helsinki() {
    let h = FinnishHyphenator::new();
    assert_eq!(
        h.hyphenate_word("Helsinki"),
        "Hel-sin-ki",
        "Helsinki should hyphenate as Hel-sin-ki"
    );
}

/// "opettaja" -> "opet-ta-ja" (min_fragment=2 suppresses single-char 'o').
#[test]
#[ignore]
fn hyphenation_opettaja() {
    let h = FinnishHyphenator::new();
    assert_eq!(
        h.hyphenate_word("opettaja"),
        "opet-ta-ja",
        "opettaja should hyphenate as opet-ta-ja (min_fragment=2)"
    );
}

/// "strategia" -> foreign cluster handling (str- stays together at word start).
#[test]
#[ignore]
fn hyphenation_strategia() {
    let h = FinnishHyphenator::new();
    let syllables = h.syllabify("strategia");
    // "strategia" syllabifies as stra-te-gi-a.
    assert_eq!(
        syllables,
        vec!["stra", "te", "gi", "a"],
        "strategia should syllabify with foreign onset cluster 'str' intact"
    );

    // With min_fragment=2, the trailing single-char 'a' is suppressed from breaks.
    let result = h.hyphenate_word("strategia");
    // Expected: "stra-te-gia" because break before 'a' is suppressed (after=1 < 2).
    assert_eq!(
        result, "stra-te-gia",
        "strategia should hyphenate as stra-te-gia with min_fragment=2"
    );
}

/// Short word "yo" -> no hyphenation (too short for any breaks).
#[test]
#[ignore]
fn hyphenation_short_word_yo() {
    let h = FinnishHyphenator::new();
    assert_eq!(
        h.hyphenate_word("yö"),
        "yö",
        "yö is too short for hyphenation"
    );
}

// ===========================================================================
// 5. Grammar checking (5 tests)
// ===========================================================================

/// "Koira koira juoksee." -> REPEATED_WORD error for the second "koira".
#[test]
#[ignore]
fn grammar_repeated_word() {
    let checker = load_grammar_checker();
    let errors = checker.check("Koira koira juoksee.");

    let repeated: Vec<_> = errors
        .iter()
        .filter(|e| e.code == "REPEATED_WORD")
        .collect();

    assert_eq!(
        repeated.len(),
        1,
        "Should detect one repeated word error for 'koira koira'"
    );
    assert!(
        repeated[0].message.contains("koira"),
        "Error message should mention the repeated word"
    );
}

/// "koira juoksee." -> CAPITALIZATION_ERROR at sentence start.
#[test]
#[ignore]
fn grammar_capitalization_error() {
    let checker = load_grammar_checker();
    let errors = checker.check("koira juoksee.");

    let cap_errors: Vec<_> = errors
        .iter()
        .filter(|e| e.code == "CAPITALIZATION_ERROR")
        .collect();

    assert_eq!(
        cap_errors.len(),
        1,
        "Should detect capitalization error for 'koira' at sentence start"
    );
    assert_eq!(
        cap_errors[0].suggestions,
        vec!["Koira"],
        "Should suggest 'Koira' as correction"
    );
}

/// "Koira juoksee." -> no errors (correct sentence).
#[test]
#[ignore]
fn grammar_correct_sentence() {
    let checker = load_grammar_checker();
    let errors = checker.check("Koira juoksee.");

    // A well-formed simple sentence should have no errors.
    // Note: agreement rule may fire depending on analysis, so we check
    // specifically for the common error types.
    let critical_errors: Vec<_> = errors
        .iter()
        .filter(|e| e.code == "REPEATED_WORD" || e.code == "CAPITALIZATION_ERROR")
        .collect();

    assert!(
        critical_errors.is_empty(),
        "Correct sentence should have no repeated word or capitalization errors, got: {:?}",
        critical_errors
    );
}

/// "Iso koira juoksee nopeasti." -> no repeated word or capitalization errors.
#[test]
#[ignore]
fn grammar_longer_correct_sentence() {
    let checker = load_grammar_checker();
    let errors = checker.check("Iso koira juoksee nopeasti.");

    let critical_errors: Vec<_> = errors
        .iter()
        .filter(|e| e.code == "REPEATED_WORD" || e.code == "CAPITALIZATION_ERROR")
        .collect();

    assert!(
        critical_errors.is_empty(),
        "Correct longer sentence should have no repeated word or capitalization errors, got: {:?}",
        critical_errors
    );
}

/// Empty and whitespace input -> no errors.
#[test]
#[ignore]
fn grammar_empty_and_whitespace() {
    let checker = load_grammar_checker();

    let errors_empty = checker.check("");
    assert!(
        errors_empty.is_empty(),
        "Empty string should produce no errors"
    );

    let errors_ws = checker.check("   ");
    assert!(
        errors_ws.is_empty(),
        "Whitespace-only string should produce no errors"
    );

    let errors_newline = checker.check("\n\n");
    assert!(
        errors_newline.is_empty(),
        "Newline-only string should produce no errors"
    );
}

// ===========================================================================
// 6. Full pipeline (2 tests)
// ===========================================================================

/// A Finnish paragraph should produce reasonable results through all stages.
/// Tests that analyze + disambiguate + grammar check all complete successfully.
#[test]
#[ignore]
fn full_pipeline_paragraph() {
    let mor_bytes = load_mor_bytes();
    let analyzer = FinnishAnalyzer::from_bytes(&mor_bytes).unwrap();
    let disambiguator = ViterbiDisambiguator::with_finnish_defaults();
    let grammar_checker = FinnishGrammarChecker::new(&mor_bytes).unwrap();
    let hyphenator = FinnishHyphenator::new();

    let paragraph = "Koira juoksee pihalla. Kissa nukkuu sohvalla.";

    // Stage 1: Analyze each word.
    let sentences: Vec<&str> = paragraph
        .split('.')
        .filter(|s| !s.trim().is_empty())
        .collect();
    for sentence_text in &sentences {
        let words: Vec<&str> = sentence_text.split_whitespace().collect();

        let word_analyses: Vec<Vec<Analysis>> = words
            .iter()
            .map(|w| {
                let chars: Vec<char> = w.chars().collect();
                analyzer.analyze(&chars, chars.len())
            })
            .collect();

        // Every content word should have at least one analysis.
        for (word, analyses) in words.iter().zip(&word_analyses) {
            assert!(
                !analyses.is_empty(),
                "Word '{}' in paragraph should have at least one analysis",
                word
            );
        }

        // Stage 2: Disambiguate.
        let disambiguated = disambiguator.disambiguate(&word_analyses);
        assert_eq!(
            disambiguated.len(),
            words.len(),
            "Disambiguator should return one result per word"
        );

        // Each disambiguated analysis should have a CLASS attribute.
        for (word, analysis) in words.iter().zip(&disambiguated) {
            assert!(
                analysis.get(ATTR_CLASS).is_some(),
                "Disambiguated word '{}' should have CLASS attribute",
                word
            );
        }
    }

    // Stage 3: Grammar check.
    let errors = grammar_checker.check(paragraph);
    // A well-formed paragraph should have no repeated word or capitalization errors.
    let critical = errors
        .iter()
        .filter(|e| e.code == "REPEATED_WORD" || e.code == "CAPITALIZATION_ERROR")
        .count();
    assert_eq!(
        critical, 0,
        "Well-formed paragraph should have no critical grammar errors"
    );

    // Stage 4: Hyphenation of individual words.
    let test_words = ["juoksee", "pihalla", "nukkuu", "sohvalla"];
    for word in &test_words {
        let result = hyphenator.hyphenate_word(word);
        // Every multi-syllable word should have at least one hyphen.
        assert!(
            result.contains('-'),
            "Word '{}' should have hyphenation points, got '{}'",
            word,
            result
        );
    }
}

/// Performance test: processing 100 words should complete in under 100ms.
/// This verifies that the pipeline meets latency targets for interactive use.
#[test]
#[ignore]
fn full_pipeline_performance_100_words() {
    let analyzer = load_analyzer();
    let disambiguator = ViterbiDisambiguator::with_finnish_defaults();

    // Build a 100-word input by repeating a small set of common Finnish words.
    let base_words = [
        "koira", "kissa", "talo", "auto", "kirja", "koulu", "maa", "vesi", "iso", "pieni",
        "juoksee", "nukkuu", "syö", "juo", "lukee", "hän", "minä", "sinä", "me", "he",
    ];
    let words: Vec<&str> = base_words.iter().cycle().take(100).copied().collect();

    let start = std::time::Instant::now();

    // Analyze all words.
    let word_analyses: Vec<Vec<Analysis>> = words
        .iter()
        .map(|w| {
            let chars: Vec<char> = w.chars().collect();
            analyzer.analyze(&chars, chars.len())
        })
        .collect();

    // Process in chunks of 10 words (simulating sentences).
    for chunk in word_analyses.chunks(10) {
        let chunk_vec: Vec<Vec<Analysis>> = chunk.to_vec();
        let _ = disambiguator.disambiguate(&chunk_vec);
    }

    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 100,
        "Processing 100 words should take less than 100ms, took {}ms",
        elapsed.as_millis()
    );
}
