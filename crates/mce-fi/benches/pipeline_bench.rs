//! Criterion benchmarks for the full Finnish analysis pipeline.
//!
//! Measures:
//! - `FinnishAnalyzer::analyze()` for a single word (FST traversal).
//! - Full pipeline: analyze + CG + disambiguate for a sentence.
//!
//! **Requires `data/mor.vfst`** — benchmarks are skipped if the file is absent.
//!
//! Run with: cargo bench -p mce-fi --bench pipeline_bench

use std::hint::black_box;
use std::path::PathBuf;

use criterion::{Criterion, criterion_group, criterion_main};
use mce_comonad::cg::{apply_cg_rules, finnish_disambiguation_rules};
use mce_disambig::{Disambiguator, ViterbiDisambiguator};
use mce_fi::morphology::{Analyzer, FinnishAnalyzer};

// ── Data loading ──────────────────────────────────────────────────────

/// Resolve the path to `mor.vfst` relative to the workspace `data/` directory.
fn vfst_path() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/mce-fi/
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("../../data/mor.vfst")
}

/// Try to load the FinnishAnalyzer. Returns None if the data file is missing.
fn try_load_analyzer() -> Option<FinnishAnalyzer> {
    let path = vfst_path();
    let data = std::fs::read(&path).ok()?;
    FinnishAnalyzer::from_bytes(&data).ok()
}

// ── Benchmarks ────────────────────────────────────────────────────────

fn bench_analyze_single_word(c: &mut Criterion) {
    let analyzer = match try_load_analyzer() {
        Some(a) => a,
        None => {
            eprintln!("SKIP: data/mor.vfst not found — skipping pipeline benchmarks");
            return;
        }
    };

    let words: &[&str] = &["koira", "talo", "juoksee", "kauniin", "yhdysvaltalainen"];

    for &word in words {
        let chars: Vec<char> = word.chars().collect();
        let len = chars.len();

        c.bench_function(&format!("analyze_{}", word), |b| {
            b.iter(|| {
                black_box(analyzer.analyze(&chars, len));
            });
        });
    }
}

fn bench_full_pipeline_sentence(c: &mut Criterion) {
    let analyzer = match try_load_analyzer() {
        Some(a) => a,
        None => return, // already printed skip message above if both run
    };

    let cg_rules = finnish_disambiguation_rules();
    let disambiguator = ViterbiDisambiguator::with_finnish_defaults();

    // "Iso koira juoksee nopeasti pihalla"
    let sentence_words: &[&str] = &["iso", "koira", "juoksee", "nopeasti", "pihalla"];

    c.bench_function("full_pipeline_5w_analyze_cg_viterbi", |b| {
        b.iter(|| {
            // Step 1: Analyze each word
            let word_analyses: Vec<Vec<_>> = sentence_words
                .iter()
                .map(|w| {
                    let chars: Vec<char> = w.chars().collect();
                    analyzer.analyze(&chars, chars.len())
                })
                .collect();

            // Step 2: CG disambiguation
            let after_cg = apply_cg_rules(&word_analyses, &cg_rules);

            // Step 3: Viterbi disambiguation
            black_box(disambiguator.disambiguate(&after_cg));
        });
    });
}

criterion_group!(
    benches,
    bench_analyze_single_word,
    bench_full_pipeline_sentence,
);
criterion_main!(benches);
