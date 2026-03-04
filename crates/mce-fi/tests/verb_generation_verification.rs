// Integration test: FST reverse verification for verb generator.
//
// For each verb lemma from lemma_dict.tsv, generate all conjugated forms
// using MorphGenerator, then analyze each generated form with FinnishAnalyzer.
// If the analyzer's BASEFORM does not match the original lemma, that is a
// generator mismatch.
//
// Run with:
//   MCE_DICT_PATH=data cargo test -p mce-fi --test verb_generation_verification -- --ignored --nocapture

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use mce_core::analysis::ATTR_BASEFORM;
use mce_fi::generator::{MorphGenerator, VerbNumber, VerbPerson, VerbPolarity, VerbTense};
use mce_fi::morphology::{Analyzer, FinnishAnalyzer};

fn load_analyzer() -> FinnishAnalyzer {
    let dict_dir =
        std::env::var("MCE_DICT_PATH").expect("MCE_DICT_PATH must be set for integration tests");
    let mor_path = std::path::Path::new(&dict_dir).join("mor.vfst");
    let data = fs::read(&mor_path).expect("Failed to read mor.vfst");
    FinnishAnalyzer::from_bytes(&data).expect("Failed to create analyzer")
}

/// Load unique verb lemmas from lemma_dict.tsv.
/// Format: surface\tPOS\tlemma
fn load_verb_lemmas() -> BTreeSet<String> {
    let dict_dir =
        std::env::var("MCE_DICT_PATH").expect("MCE_DICT_PATH must be set for integration tests");
    let dict_path = std::path::Path::new(&dict_dir).join("lemma_dict.tsv");
    let content = fs::read_to_string(&dict_path).expect("Failed to read lemma_dict.tsv");

    let mut verbs = BTreeSet::new();
    for line in content.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 && parts[1] == "VERB" {
            let lemma = parts[2].to_string();
            // Skip compound lemmas (contain #) and lemmas with special chars
            if !lemma.contains('#') && !lemma.contains('\'') && !lemma.contains(':') {
                verbs.insert(lemma);
            }
        }
    }
    verbs
}

/// Classification of a mismatch.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum MismatchType {
    /// Generator produced a form, but the analyzer finds zero analyses.
    NoAnalysis,
    /// Analyzer returns analyses but none have BASEFORM matching the lemma.
    WrongBaseform,
    /// Generator could not classify the verb (returns None).
    GeneratorReject,
}

impl std::fmt::Display for MismatchType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MismatchType::NoAnalysis => write!(f, "NO_ANALYSIS"),
            MismatchType::WrongBaseform => write!(f, "WRONG_BASEFORM"),
            MismatchType::GeneratorReject => write!(f, "GENERATOR_REJECT"),
        }
    }
}

struct Mismatch {
    lemma: String,
    tense: String,
    person: String,
    polarity: String,
    generated_form: String,
    mismatch_type: MismatchType,
    analyzer_baseforms: Vec<String>,
}

impl std::fmt::Display for Mismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let baseforms = if self.analyzer_baseforms.is_empty() {
            "(none)".to_string()
        } else {
            self.analyzer_baseforms.join(", ")
        };
        write!(
            f,
            "{}: {} {} {} -> \"{}\" [{}] analyzer_baseforms=[{}]",
            self.mismatch_type,
            self.lemma,
            self.tense,
            self.person,
            self.generated_form,
            self.polarity,
            baseforms,
        )
    }
}

#[test]
#[ignore]
fn verb_generation_fst_verification() {
    let analyzer = load_analyzer();
    let generator = MorphGenerator::new();
    let verb_lemmas = load_verb_lemmas();

    let tenses = [
        (
            VerbTense::Present,
            VerbPolarity::Affirmative,
            "present",
            "aff",
        ),
        (VerbTense::Past, VerbPolarity::Affirmative, "past", "aff"),
        (
            VerbTense::Conditional,
            VerbPolarity::Affirmative,
            "conditional",
            "aff",
        ),
        (
            VerbTense::Present,
            VerbPolarity::Negative,
            "neg_present",
            "neg",
        ),
    ];

    let persons = [
        (VerbPerson::First, VerbNumber::Singular, "1sg"),
        (VerbPerson::Second, VerbNumber::Singular, "2sg"),
        (VerbPerson::Third, VerbNumber::Singular, "3sg"),
        (VerbPerson::First, VerbNumber::Plural, "1pl"),
        (VerbPerson::Second, VerbNumber::Plural, "2pl"),
        (VerbPerson::Third, VerbNumber::Plural, "3pl"),
    ];

    let mut mismatches: Vec<Mismatch> = Vec::new();
    let mut total_forms = 0u64;
    let mut total_ok = 0u64;
    let mut total_generator_reject = 0u64;
    let mut verbs_tested = 0u64;
    let mut verbs_with_issues: BTreeSet<String> = BTreeSet::new();

    // Mismatch counts by category
    let mut by_type: BTreeMap<MismatchType, u64> = BTreeMap::new();
    let mut by_tense: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_person: BTreeMap<String, u64> = BTreeMap::new();

    let total_verbs = verb_lemmas.len();

    for lemma in &verb_lemmas {
        verbs_tested += 1;

        // First, check if the generator can even classify this verb.
        let paradigm = generator.generate_verb_paradigm(lemma);
        if paradigm.is_none() {
            total_generator_reject += 1;
            // Record one mismatch entry for the entire verb.
            mismatches.push(Mismatch {
                lemma: lemma.clone(),
                tense: "*".to_string(),
                person: "*".to_string(),
                polarity: "*".to_string(),
                generated_form: "(rejected)".to_string(),
                mismatch_type: MismatchType::GeneratorReject,
                analyzer_baseforms: vec![],
            });
            verbs_with_issues.insert(lemma.clone());
            continue;
        }

        for (tense, polarity, tense_label, pol_label) in &tenses {
            for (person, number, person_label) in &persons {
                total_forms += 1;

                let generated = generator.generate_verb(lemma, *tense, *person, *number, *polarity);

                let form = match generated {
                    Some(f) => f,
                    None => {
                        // Should not happen if paradigm was Some, but handle.
                        continue;
                    }
                };

                // For negative forms, the generated form is "aux stem".
                // The analyzer would analyze the main verb part only.
                // Split and analyze the main verb (last word).
                let word_to_analyze = if *polarity == VerbPolarity::Negative {
                    // "en puhu" -> analyze "puhu"
                    form.split_whitespace().last().unwrap_or(&form).to_string()
                } else {
                    form.clone()
                };

                let chars: Vec<char> = word_to_analyze.chars().collect();
                let analyses = analyzer.analyze(&chars, chars.len());

                if analyses.is_empty() {
                    mismatches.push(Mismatch {
                        lemma: lemma.clone(),
                        tense: tense_label.to_string(),
                        person: person_label.to_string(),
                        polarity: pol_label.to_string(),
                        generated_form: form.clone(),
                        mismatch_type: MismatchType::NoAnalysis,
                        analyzer_baseforms: vec![],
                    });
                    *by_type.entry(MismatchType::NoAnalysis).or_insert(0) += 1;
                    *by_tense.entry(tense_label.to_string()).or_insert(0) += 1;
                    *by_person.entry(person_label.to_string()).or_insert(0) += 1;
                    verbs_with_issues.insert(lemma.clone());
                    continue;
                }

                // Check if any analysis has a matching baseform.
                let analyzer_baseforms: Vec<String> = analyses
                    .iter()
                    .filter_map(|a| a.get(ATTR_BASEFORM).map(|s| s.to_string()))
                    .collect();

                let matches = analyzer_baseforms.iter().any(|bf| bf == lemma);

                if matches {
                    total_ok += 1;
                } else {
                    mismatches.push(Mismatch {
                        lemma: lemma.clone(),
                        tense: tense_label.to_string(),
                        person: person_label.to_string(),
                        polarity: pol_label.to_string(),
                        generated_form: form.clone(),
                        mismatch_type: MismatchType::WrongBaseform,
                        analyzer_baseforms: analyzer_baseforms.clone(),
                    });
                    *by_type.entry(MismatchType::WrongBaseform).or_insert(0) += 1;
                    *by_tense.entry(tense_label.to_string()).or_insert(0) += 1;
                    *by_person.entry(person_label.to_string()).or_insert(0) += 1;
                    verbs_with_issues.insert(lemma.clone());
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Print summary report
    // -----------------------------------------------------------------------

    println!("\n============================================================");
    println!("  VERB GENERATION FST REVERSE VERIFICATION REPORT");
    println!("============================================================\n");
    println!("Total verb lemmas in dict:   {}", total_verbs);
    println!("Verbs tested:                {}", verbs_tested);
    println!("Generator rejected (no type):{}", total_generator_reject);
    println!("Total forms generated:       {}", total_forms);
    println!("Forms OK (baseform match):   {}", total_ok);
    println!("Total mismatches:            {}", mismatches.len());
    println!(
        "Verbs with at least 1 issue: {} / {} ({:.1}%)",
        verbs_with_issues.len(),
        verbs_tested,
        verbs_with_issues.len() as f64 / verbs_tested as f64 * 100.0
    );

    println!("\n--- Mismatches by type ---");
    for (t, count) in &by_type {
        println!("  {}: {}", t, count);
    }

    println!("\n--- Mismatches by tense (excl. GeneratorReject) ---");
    for (t, count) in &by_tense {
        println!("  {}: {}", t, count);
    }

    println!("\n--- Mismatches by person (excl. GeneratorReject) ---");
    for (p, count) in &by_person {
        println!("  {}: {}", p, count);
    }

    // Classify rejected verbs by ending pattern
    let rejected_verbs: Vec<&String> = mismatches
        .iter()
        .filter(|m| m.mismatch_type == MismatchType::GeneratorReject)
        .map(|m| &m.lemma)
        .collect();
    if !rejected_verbs.is_empty() {
        println!("\n--- Generator-rejected verb endings ---");
        let mut ending_counts: BTreeMap<String, u64> = BTreeMap::new();
        for v in &rejected_verbs {
            let ending = if v.len() >= 3 {
                v[v.len() - 3..].to_string()
            } else {
                v.to_string()
            };
            *ending_counts.entry(ending).or_insert(0) += 1;
        }
        let mut ending_vec: Vec<(String, u64)> = ending_counts.into_iter().collect();
        ending_vec.sort_by(|a, b| b.1.cmp(&a.1));
        for (ending, count) in ending_vec.iter().take(30) {
            println!("  ...{}: {}", ending, count);
        }
    }

    // Print specific mismatches (WRONG_BASEFORM and NO_ANALYSIS, not Generator-rejected)
    let analysis_mismatches: Vec<&Mismatch> = mismatches
        .iter()
        .filter(|m| m.mismatch_type != MismatchType::GeneratorReject)
        .collect();
    if !analysis_mismatches.is_empty() {
        // Group by lemma for readability
        let mut by_lemma: BTreeMap<&str, Vec<&Mismatch>> = BTreeMap::new();
        for m in &analysis_mismatches {
            by_lemma.entry(&m.lemma).or_default().push(m);
        }

        println!("\n--- Detailed mismatches (grouped by lemma) ---\n");
        let max_show = 100; // Max lemmas to show in detail
        for (i, (lemma, items)) in by_lemma.iter().enumerate() {
            if i >= max_show {
                println!(
                    "  ... and {} more lemmas with mismatches (truncated)",
                    by_lemma.len() - max_show
                );
                break;
            }
            println!("  [{}] {} ({} mismatches):", i + 1, lemma, items.len());
            for m in items {
                println!("      {}", m);
            }
        }
    }

    // Print some OK examples for sanity
    println!("\n--- Sample rejected verbs (first 30) ---");
    for (i, v) in rejected_verbs.iter().take(30).enumerate() {
        println!("  {}: {}", i + 1, v);
    }

    println!("\n============================================================");
    println!("  END OF REPORT");
    println!("============================================================\n");
}
