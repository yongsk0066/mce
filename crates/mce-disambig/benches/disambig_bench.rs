//! Criterion benchmarks for the Viterbi disambiguator.
//!
//! Measures:
//! - `ViterbiDisambiguator::disambiguate()` on a synthetic 5-word sentence.
//! - `disambiguate_with_words()` with emission scoring.
//! - Unambiguous (fast-path) sentence throughput.
//!
//! Run with: cargo bench -p mce-disambig --bench disambig_bench

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use mce_core::analysis::{ATTR_BASEFORM, ATTR_CLASS, Analysis};
use mce_disambig::{Disambiguator, ViterbiDisambiguator};

// ── Synthetic data ────────────────────────────────────────────────────

fn make_analysis(class: &str, baseform: &str) -> Analysis {
    let mut a = Analysis::new();
    a.set(ATTR_CLASS, class);
    a.set(ATTR_BASEFORM, baseform);
    a
}

/// Ambiguous 5-word sentence:
/// "Iso koira juoksee nopeasti pihalla"
/// Positions 0, 3 are ambiguous (2 readings each).
fn build_ambiguous_sentence() -> Vec<Vec<Analysis>> {
    vec![
        vec![
            make_analysis("laatusana", "iso"),   // ADJ
            make_analysis("nimisana", "iso"),     // NOUN (rare)
        ],
        vec![
            make_analysis("nimisana", "koira"),   // NOUN
            make_analysis("teonsana", "koira"),   // VERB (unlikely)
        ],
        vec![make_analysis("teonsana", "juosta")], // VERB
        vec![
            make_analysis("seikkasana", "nopeasti"), // ADV
            make_analysis("laatusana", "nopea"),     // ADJ
        ],
        vec![make_analysis("nimisana", "piha")], // NOUN
    ]
}

/// Unambiguous 5-word sentence (1 reading per position).
fn build_unambiguous_sentence() -> Vec<Vec<Analysis>> {
    vec![
        vec![make_analysis("laatusana", "iso")],
        vec![make_analysis("nimisana", "koira")],
        vec![make_analysis("teonsana", "juosta")],
        vec![make_analysis("seikkasana", "nopeasti")],
        vec![make_analysis("nimisana", "piha")],
    ]
}

// ── Benchmarks ────────────────────────────────────────────────────────

fn bench_viterbi_disambiguate(c: &mut Criterion) {
    let disambiguator = ViterbiDisambiguator::with_finnish_defaults();
    let sentence = build_ambiguous_sentence();

    c.bench_function("viterbi_disambiguate_5w_ambiguous", |b| {
        b.iter(|| {
            black_box(disambiguator.disambiguate(&sentence));
        });
    });
}

fn bench_viterbi_disambiguate_with_words(c: &mut Criterion) {
    let disambiguator = ViterbiDisambiguator::with_finnish_defaults_and_emission();
    let sentence = build_ambiguous_sentence();
    let words = ["iso", "koira", "juoksee", "nopeasti", "pihalla"];

    c.bench_function("viterbi_disambiguate_with_words_5w", |b| {
        b.iter(|| {
            black_box(disambiguator.disambiguate_with_words(&words, &sentence));
        });
    });
}

fn bench_viterbi_unambiguous_fast_path(c: &mut Criterion) {
    let disambiguator = ViterbiDisambiguator::with_finnish_defaults();
    let sentence = build_unambiguous_sentence();

    c.bench_function("viterbi_unambiguous_fast_path_5w", |b| {
        b.iter(|| {
            black_box(disambiguator.disambiguate(&sentence));
        });
    });
}

criterion_group!(
    benches,
    bench_viterbi_disambiguate,
    bench_viterbi_disambiguate_with_words,
    bench_viterbi_unambiguous_fast_path,
);
criterion_main!(benches);
