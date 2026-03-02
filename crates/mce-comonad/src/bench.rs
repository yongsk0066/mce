//! Per-rule latency benchmarking for coKleisli arrows and CG rules.
//!
//! Measures individual rule execution times using `std::time::Instant`
//! (no external benchmark dependencies, WASM-compatible design).
//!
//! # Usage
//!
//! ```no_run
//! use mce_comonad::bench::{bench_cokleisli_arrows, bench_cg_rules};
//!
//! let morpho_results = bench_cokleisli_arrows(10_000);
//! for r in &morpho_results {
//!     println!("{}: {:.2} us +/- {:.2} us", r.rule_name, r.mean_us, r.std_us);
//! }
//!
//! let cg_results = bench_cg_rules(10_000);
//! for r in &cg_results {
//!     println!("{}: {:.2} us +/- {:.2} us", r.rule_name, r.mean_us, r.std_us);
//! }
//! ```

use std::time::Instant;

use mce_core::analysis::{Analysis, ATTR_BASEFORM, ATTR_CLASS, ATTR_SIJAMUOTO};

use crate::cg::{apply_cg_rules, finnish_disambiguation_rules, CgRule, ReadingSet};
use crate::finnish::{
    apply_gradation, apply_possessive, apply_vowel_harmony, gradate, harmonize,
    morphophonological_pipeline, Grade,
};
use crate::zipper::Zipper;

// ---------------------------------------------------------------------------
// Result type
// ---------------------------------------------------------------------------

/// Result of benchmarking a single rule.
#[derive(Debug, Clone)]
pub struct RuleBenchmark {
    /// Human-readable name of the rule being benchmarked.
    pub rule_name: String,
    /// Mean execution time in microseconds.
    pub mean_us: f64,
    /// Standard deviation of execution time in microseconds.
    pub std_us: f64,
    /// Number of iterations used to compute statistics.
    pub iterations: usize,
    /// Example input used for the benchmark.
    pub input: String,
    /// Example output produced by the rule.
    pub output: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run a closure `iterations` times, collecting per-iteration durations
/// in nanoseconds. Returns (mean_us, std_us).
fn measure_stats<F: FnMut()>(iterations: usize, warmup: usize, mut f: F) -> (f64, f64) {
    // Warmup phase.
    for _ in 0..warmup {
        f();
    }

    // Measurement phase: collect each iteration's duration.
    let mut durations_ns = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        f();
        let elapsed = start.elapsed();
        durations_ns.push(elapsed.as_nanos() as f64);
    }

    // Compute mean in microseconds.
    let sum: f64 = durations_ns.iter().sum();
    let mean_ns = sum / iterations as f64;
    let mean_us = mean_ns / 1_000.0;

    // Compute standard deviation in microseconds.
    let variance: f64 = durations_ns
        .iter()
        .map(|&d| {
            let diff = d - mean_ns;
            diff * diff
        })
        .sum::<f64>()
        / iterations as f64;
    let std_us = variance.sqrt() / 1_000.0;

    (mean_us, std_us)
}

// ---------------------------------------------------------------------------
// Morphophonological (coKleisli arrow) benchmarks
// ---------------------------------------------------------------------------

/// Benchmark data: (label, input_word, grade, expected_output).
const GRADATION_CASES: &[(&str, &str, bool, &str)] = &[
    ("pp->p (kaappi)", "kaappi", true, "kaapi"),
    ("tt->t (matto)", "matto", true, "mato"),
    ("kk->k (kukka)", "kukka", true, "kuka"),
    ("p->v (tapa)", "tapa", true, "tava"),
    ("t->d (katu)", "katu", true, "kadu"),
    ("nk->ng (kenka)", "kenka", true, "kenga"),
    ("nt->nn (ranta)", "ranta", true, "ranna"),
    ("mp->mm (kampa)", "kampa", true, "kamma"),
    ("lt->ll (kulta)", "kulta", true, "kulla"),
    ("rt->rr (parta)", "parta", true, "parra"),
    ("no gradation (talo)", "talo", true, "talo"),
];

/// Benchmark each coKleisli arrow individually.
///
/// Tests consonant gradation (11 patterns), vowel harmony, possessive
/// suffix copying, and the full morphophonological pipeline.
///
/// # Arguments
///
/// * `iterations` - Number of timed iterations per rule.
///
/// # Returns
///
/// A vector of [`RuleBenchmark`] results, one per rule tested.
pub fn bench_cokleisli_arrows(iterations: usize) -> Vec<RuleBenchmark> {
    let warmup = iterations / 10;
    let mut results = Vec::new();

    // -----------------------------------------------------------------------
    // 1. Individual consonant gradation patterns
    // -----------------------------------------------------------------------
    for &(label, input, is_weak, expected) in GRADATION_CASES {
        let grade = if is_weak { Grade::Weak } else { Grade::Strong };
        let output = gradate(input, grade);
        debug_assert_eq!(output, expected, "gradation sanity check for {}", label);

        let (mean_us, std_us) = measure_stats(iterations, warmup, || {
            let _ = gradate(input, grade);
        });

        results.push(RuleBenchmark {
            rule_name: format!("gradation {}", label),
            mean_us,
            std_us,
            iterations,
            input: input.to_string(),
            output,
        });
    }

    // -----------------------------------------------------------------------
    // 2. Vowel harmony
    // -----------------------------------------------------------------------
    let harmony_cases: &[(&str, &str, &str)] = &[
        ("back (talossA)", "talossA", "talossa"),
        (
            "front (p\u{00F6}yd\u{00E4}llA)",
            "p\u{00F6}yd\u{00E4}llA",
            "p\u{00F6}yd\u{00E4}ll\u{00E4}",
        ),
        ("neutral (kissAlle)", "kissAlle", "kiss\u{00E4}lle"),
        ("no archiphoneme (auto)", "auto", "auto"),
    ];

    for &(label, input, expected) in harmony_cases {
        let output = harmonize(input);
        debug_assert_eq!(output, expected, "harmony sanity check for {}", label);

        let (mean_us, std_us) = measure_stats(iterations, warmup, || {
            let _ = harmonize(input);
        });

        results.push(RuleBenchmark {
            rule_name: format!("harmony {}", label),
            mean_us,
            std_us,
            iterations,
            input: input.to_string(),
            output,
        });
    }

    // -----------------------------------------------------------------------
    // 3. Possessive suffix vowel copying
    // -----------------------------------------------------------------------
    let possessive_cases: &[(&str, &str, &str)] = &[
        ("V copy (taloVn)", "taloVn", "taloon"),
        ("V copy (k\u{00E4}siVn)", "k\u{00E4}siVn", "k\u{00E4}siin"),
        ("no V (koira)", "koira", "koira"),
    ];

    for &(label, input, expected) in possessive_cases {
        let output = crate::finnish::apply_possessive_to_word(input);
        debug_assert_eq!(output, expected, "possessive sanity check for {}", label);

        let (mean_us, std_us) = measure_stats(iterations, warmup, || {
            let _ = crate::finnish::apply_possessive_to_word(input);
        });

        results.push(RuleBenchmark {
            rule_name: format!("possessive {}", label),
            mean_us,
            std_us,
            iterations,
            input: input.to_string(),
            output,
        });
    }

    // -----------------------------------------------------------------------
    // 4. Full morphophonological pipeline
    // -----------------------------------------------------------------------
    let pipeline_cases: &[(&str, &str, &str)] = &[
        ("rantAssA (weak)", "rantAssA", "rannassa"),
        (
            "p\u{00F6}yt\u{00E4}llA (weak)",
            "p\u{00F6}yt\u{00E4}llA",
            "p\u{00F6}yd\u{00E4}ll\u{00E4}",
        ),
        ("taloVn (weak)", "taloVn", "taloon"),
        ("kaapissA (weak)", "kaapissA", "kaavissa"),
    ];

    for &(label, input, expected) in pipeline_cases {
        let output = morphophonological_pipeline(input, Grade::Weak);
        debug_assert_eq!(output, expected, "pipeline sanity check for {}", label);

        let (mean_us, std_us) = measure_stats(iterations, warmup, || {
            let _ = morphophonological_pipeline(input, Grade::Weak);
        });

        results.push(RuleBenchmark {
            rule_name: format!("pipeline {}", label),
            mean_us,
            std_us,
            iterations,
            input: input.to_string(),
            output,
        });
    }

    // -----------------------------------------------------------------------
    // 5. Bare coKleisli arrow (extend only, no alloc)
    // -----------------------------------------------------------------------
    // Measure the raw `extend` cost for a single gradation arrow on "kaappi".
    {
        let chars: Vec<char> = "kaappi".chars().collect();
        let z = Zipper::new(chars).unwrap();

        let (mean_us, std_us) = measure_stats(iterations, warmup, || {
            let _ = z.extend(|zi| apply_gradation(zi, Grade::Weak));
        });

        results.push(RuleBenchmark {
            rule_name: "extend_only gradation (kaappi)".to_string(),
            mean_us,
            std_us,
            iterations,
            input: "kaappi".to_string(),
            output: "kaapi".to_string(),
        });
    }

    // Measure the raw `extend` cost for vowel harmony arrow on "talossA".
    {
        let chars: Vec<char> = "talossA".chars().collect();
        let z = Zipper::new(chars).unwrap();

        let (mean_us, std_us) = measure_stats(iterations, warmup, || {
            let _ = z.extend(apply_vowel_harmony);
        });

        results.push(RuleBenchmark {
            rule_name: "extend_only harmony (talossA)".to_string(),
            mean_us,
            std_us,
            iterations,
            input: "talossA".to_string(),
            output: "talossa".to_string(),
        });
    }

    // Measure the raw `extend` cost for possessive arrow on "taloVn".
    {
        let chars: Vec<char> = "taloVn".chars().collect();
        let z = Zipper::new(chars).unwrap();

        let (mean_us, std_us) = measure_stats(iterations, warmup, || {
            let _ = z.extend(apply_possessive);
        });

        results.push(RuleBenchmark {
            rule_name: "extend_only possessive (taloVn)".to_string(),
            mean_us,
            std_us,
            iterations,
            input: "taloVn".to_string(),
            output: "taloon".to_string(),
        });
    }

    results
}

// ---------------------------------------------------------------------------
// CG rule benchmarks
// ---------------------------------------------------------------------------

/// Build a synthetic ambiguous Finnish sentence for CG benchmarking.
///
/// Returns a 5-position sentence with realistic ambiguity:
/// \[DET\] \[ADJ/NOUN\] \[NOUN/VERB\] \[ADV/NOUN\] \[VERB\]
fn build_cg_test_sentence() -> Vec<ReadingSet> {
    let mut det = Analysis::new();
    det.set(ATTR_CLASS, "asemosana");
    det.set(ATTR_BASEFORM, "se");

    let mut adj = Analysis::new();
    adj.set(ATTR_CLASS, "laatusana");
    adj.set(ATTR_BASEFORM, "suuri");

    let mut noun1 = Analysis::new();
    noun1.set(ATTR_CLASS, "nimisana");
    noun1.set(ATTR_BASEFORM, "suuri");

    let mut noun2 = Analysis::new();
    noun2.set(ATTR_CLASS, "nimisana");
    noun2.set(ATTR_BASEFORM, "koira");
    noun2.set(ATTR_SIJAMUOTO, "nimento");

    let mut verb1 = Analysis::new();
    verb1.set(ATTR_CLASS, "teonsana");
    verb1.set(ATTR_BASEFORM, "koira");

    let mut adv = Analysis::new();
    adv.set(ATTR_CLASS, "seikkasana");
    adv.set(ATTR_BASEFORM, "nopeasti");

    let mut noun3 = Analysis::new();
    noun3.set(ATTR_CLASS, "nimisana");
    noun3.set(ATTR_BASEFORM, "nopeasti");

    let mut verb2 = Analysis::new();
    verb2.set(ATTR_CLASS, "teonsana");
    verb2.set(ATTR_BASEFORM, "juosta");

    vec![
        vec![det],          // position 0: DET
        vec![adj, noun1],   // position 1: ADJ/NOUN
        vec![noun2, verb1], // position 2: NOUN/VERB
        vec![adv, noun3],   // position 3: ADV/NOUN
        vec![verb2],        // position 4: VERB
    ]
}

/// Benchmark CG rule application per rule type.
///
/// Measures each of the 20 CG rule types individually, plus the full
/// 53-rule Finnish disambiguation pipeline.
///
/// # Arguments
///
/// * `iterations` - Number of timed iterations per rule.
///
/// # Returns
///
/// A vector of [`RuleBenchmark`] results, one per rule type.
pub fn bench_cg_rules(iterations: usize) -> Vec<RuleBenchmark> {
    let warmup = iterations / 10;
    let mut results = Vec::new();
    let sentence = build_cg_test_sentence();

    // -----------------------------------------------------------------------
    // 1. Individual rule types
    // -----------------------------------------------------------------------
    let rule_type_samples: Vec<(&str, Box<dyn CgRule>)> = vec![
        (
            "RemoveIfPreceded",
            Box::new(crate::cg::RemoveIfPreceded {
                remove_class: "teonsana".into(),
                preceded_by_class: "asemosana".into(),
            }),
        ),
        (
            "SelectIfFollowed",
            Box::new(crate::cg::SelectIfFollowed {
                select_class: "nimisana".into(),
                followed_by_class: "teonsana".into(),
            }),
        ),
        (
            "RemoveIfNotPreceded",
            Box::new(crate::cg::RemoveIfNotPreceded {
                remove_class: "laatusana".into(),
                not_preceded_by_class: "asemosana".into(),
            }),
        ),
        (
            "RemoveByClass",
            Box::new(crate::cg::RemoveByClass {
                remove_class: "nimisana".into(),
                context_class: "asemosana".into(),
                require_alternative: "laatusana".into(),
            }),
        ),
        (
            "SelectByBaseform",
            Box::new(crate::cg::SelectByBaseform {
                select_class: "teonsana".into(),
                preceded_by_baseform: "ei".into(),
            }),
        ),
        (
            "SelectIfNotFollowed",
            Box::new(crate::cg::SelectIfNotFollowed {
                select_class: "nimisana".into(),
                not_followed_by_class: "teonsana".into(),
            }),
        ),
        (
            "RemoveIfFollowed",
            Box::new(crate::cg::RemoveIfFollowed {
                remove_class: "nimisana".into(),
                followed_by_class: "teonsana".into(),
            }),
        ),
        (
            "SelectIfPreceded",
            Box::new(crate::cg::SelectIfPreceded {
                select_class: "teonsana".into(),
                preceded_by_class: "asemosana".into(),
            }),
        ),
        (
            "SelectByBaseformList",
            Box::new(crate::cg::SelectByBaseformList {
                select_class: "teonsana".into(),
                preceded_by_baseforms: vec![
                    "min\u{00E4}".into(),
                    "sin\u{00E4}".into(),
                    "h\u{00E4}n".into(),
                ],
            }),
        ),
        (
            "RemoveIfCase",
            Box::new(crate::cg::RemoveIfCase {
                remove_class: "teonsana".into(),
                has_case: "nimento".into(),
            }),
        ),
        (
            "SelectIfFollowedByBaseformList",
            Box::new(crate::cg::SelectIfFollowedByBaseformList {
                select_class: "nimisana".into(),
                followed_by_baseforms: vec!["juosta".into(), "olla".into()],
            }),
        ),
        (
            "RemoveByBaseformList",
            Box::new(crate::cg::RemoveByBaseformList {
                remove_class: "teonsana".into(),
                preceded_by_baseforms: vec!["se".into(), "t\u{00E4}m\u{00E4}".into()],
            }),
        ),
        (
            "RemoveIfNotFollowed",
            Box::new(crate::cg::RemoveIfNotFollowed {
                remove_class: "nimisana".into(),
                not_followed_by_class: "teonsana".into(),
            }),
        ),
        (
            "SelectIfAttr",
            Box::new(crate::cg::SelectIfAttr {
                select_class: "laatusana".into(),
                attr_name: ATTR_SIJAMUOTO.into(),
                attr_value: "nimento".into(),
            }),
        ),
        (
            "RemoveIfAttr",
            Box::new(crate::cg::RemoveIfAttr {
                remove_class: "teonsana".into(),
                attr_name: ATTR_SIJAMUOTO.into(),
                attr_value: "nimento".into(),
            }),
        ),
        (
            "SelectByCurrentBaseformList",
            Box::new(crate::cg::SelectByCurrentBaseformList {
                select_class: "teonsana".into(),
                baseforms: vec!["olla".into(), "voida".into()],
            }),
        ),
        (
            "RemoveIfSandwiched",
            Box::new(crate::cg::RemoveIfSandwiched {
                remove_class: "seikkasana".into(),
                preceded_by_class: "nimisana".into(),
                followed_by_class: "nimisana".into(),
            }),
        ),
        (
            "SelectIfSandwiched",
            Box::new(crate::cg::SelectIfSandwiched {
                select_class: "laatusana".into(),
                preceded_by_class: "nimisana".into(),
                followed_by_class: "nimisana".into(),
            }),
        ),
        (
            "RemoveAtSentenceStart",
            Box::new(crate::cg::RemoveAtSentenceStart {
                remove_class: "nimisana".into(),
            }),
        ),
        (
            "RemoveIfFollowedByBaseformList",
            Box::new(crate::cg::RemoveIfFollowedByBaseformList {
                remove_class: "nimisana".into(),
                followed_by_baseforms: vec!["juosta".into(), "olla".into()],
            }),
        ),
    ];

    for (rule_name, rule) in &rule_type_samples {
        // Apply the rule directly via Zipper::extend (single rule pass).
        let (mean_us, std_us) = measure_stats(iterations, warmup, || {
            if let Some(z) = Zipper::new(sentence.clone()) {
                let _ = z.extend(|focused| rule.apply(focused));
            }
        });

        // For output, compute the result once.
        let output = if let Some(z) = Zipper::new(sentence.clone()) {
            let result = z.extend(|focused| rule.apply(focused));
            let counts: Vec<usize> = result.to_vec().iter().map(|rs| rs.len()).collect();
            format!("readings: {:?}", counts)
        } else {
            "empty".to_string()
        };

        results.push(RuleBenchmark {
            rule_name: format!("cg_{}", rule_name),
            mean_us,
            std_us,
            iterations,
            input: "[DET] [ADJ/NOUN] [NOUN/VERB] [ADV/NOUN] [VERB]".to_string(),
            output,
        });
    }

    // -----------------------------------------------------------------------
    // 2. Full Finnish CG pipeline (53 rules)
    // -----------------------------------------------------------------------
    {
        let rules = finnish_disambiguation_rules();

        let (mean_us, std_us) = measure_stats(iterations, warmup, || {
            let _ = apply_cg_rules(&sentence, &rules);
        });

        let disambiguated = apply_cg_rules(&sentence, &rules);
        let counts: Vec<usize> = disambiguated.iter().map(|rs| rs.len()).collect();

        results.push(RuleBenchmark {
            rule_name: "cg_full_pipeline (53 rules)".to_string(),
            mean_us,
            std_us,
            iterations,
            input: "[DET] [ADJ/NOUN] [NOUN/VERB] [ADV/NOUN] [VERB]".to_string(),
            output: format!("readings: {:?}", counts),
        });
    }

    results
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_cokleisli_arrows_returns_results() {
        let results = bench_cokleisli_arrows(100);
        // Should have gradation (11) + harmony (4) + possessive (3) +
        // pipeline (4) + extend_only (3) = 25 results.
        assert!(
            results.len() >= 20,
            "expected at least 20 results, got {}",
            results.len()
        );

        for r in &results {
            assert!(r.iterations == 100);
            assert!(r.mean_us >= 0.0, "mean_us should be non-negative");
            assert!(r.std_us >= 0.0, "std_us should be non-negative");
            assert!(!r.rule_name.is_empty());
            assert!(!r.input.is_empty());
            assert!(!r.output.is_empty());
        }
    }

    #[test]
    fn bench_cg_rules_returns_results() {
        let results = bench_cg_rules(100);
        // Should have 20 individual rule types + 1 full pipeline = 21 results.
        assert_eq!(
            results.len(),
            21,
            "expected 21 CG results, got {}",
            results.len()
        );

        for r in &results {
            assert!(r.iterations == 100);
            assert!(r.mean_us >= 0.0, "mean_us should be non-negative");
            assert!(r.std_us >= 0.0, "std_us should be non-negative");
            assert!(r.rule_name.starts_with("cg_"));
        }
    }

    #[test]
    fn bench_gradation_produces_correct_output() {
        let results = bench_cokleisli_arrows(10);
        let kaappi = results
            .iter()
            .find(|r| r.rule_name.contains("kaappi"))
            .expect("should have kaappi benchmark");
        assert_eq!(kaappi.output, "kaapi");
    }

    #[test]
    fn bench_harmony_produces_correct_output() {
        let results = bench_cokleisli_arrows(10);
        let back = results
            .iter()
            .find(|r| r.rule_name.contains("back (talossA)"))
            .expect("should have back harmony benchmark");
        assert_eq!(back.output, "talossa");
    }

    #[test]
    fn bench_pipeline_produces_correct_output() {
        let results = bench_cokleisli_arrows(10);
        let pipeline = results
            .iter()
            .find(|r| r.rule_name.contains("rantAssA"))
            .expect("should have pipeline benchmark");
        assert_eq!(pipeline.output, "rannassa");
    }

    #[test]
    fn bench_cg_full_pipeline_present() {
        let results = bench_cg_rules(10);
        let full = results
            .iter()
            .find(|r| r.rule_name.contains("full_pipeline"))
            .expect("should have full CG pipeline benchmark");
        assert!(full.output.contains("readings:"));
    }

    #[test]
    fn measure_stats_basic_correctness() {
        let mut counter = 0u64;
        let (mean_us, std_us) = measure_stats(100, 10, || {
            counter += 1;
        });
        // The closure should have been called 100 (measurement) + 10 (warmup) = 110 times.
        assert_eq!(counter, 110);
        assert!(mean_us >= 0.0);
        assert!(std_us >= 0.0);
    }
}
