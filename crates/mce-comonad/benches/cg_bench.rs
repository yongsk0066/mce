//! Criterion benchmarks for CG-lite disambiguation rules.
//!
//! Measures:
//! - Full CG pipeline (62 rules) on a synthetic 5-word sentence.
//! - Individual rule application via `Zipper::extend`.
//! - Synthetic sentence construction overhead.
//!
//! Run with: cargo bench -p mce-comonad --bench cg_bench

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use mce_comonad::Zipper;
use mce_comonad::cg::{
    CgRule, ReadingSet, RemoveIfPreceded, SelectIfFollowed, apply_cg_rules,
    finnish_disambiguation_rules,
};
use mce_core::analysis::{ATTR_BASEFORM, ATTR_CLASS, ATTR_SIJAMUOTO, Analysis};

// ── Synthetic data ────────────────────────────────────────────────────

/// Build a synthetic 5-position ambiguous Finnish sentence:
/// [DET] [ADJ/NOUN] [NOUN/VERB] [ADV/NOUN] [VERB]
fn build_sentence() -> Vec<ReadingSet> {
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

// ── Benchmarks ────────────────────────────────────────────────────────

fn bench_full_cg_pipeline(c: &mut Criterion) {
    let sentence = build_sentence();
    let rules = finnish_disambiguation_rules();

    c.bench_function("cg_full_pipeline_62_rules", |b| {
        b.iter(|| {
            black_box(apply_cg_rules(&sentence, &rules));
        });
    });
}

fn bench_single_rule_extend(c: &mut Criterion) {
    let sentence = build_sentence();

    let remove_rule = RemoveIfPreceded {
        remove_class: "teonsana".into(),
        preceded_by_class: "asemosana".into(),
    };

    let select_rule = SelectIfFollowed {
        select_class: "nimisana".into(),
        followed_by_class: "teonsana".into(),
    };

    c.bench_function("cg_extend_RemoveIfPreceded", |b| {
        b.iter(|| {
            if let Some(z) = Zipper::new(sentence.clone()) {
                black_box(z.extend(|focused| remove_rule.apply(focused)));
            }
        });
    });

    c.bench_function("cg_extend_SelectIfFollowed", |b| {
        b.iter(|| {
            if let Some(z) = Zipper::new(sentence.clone()) {
                black_box(z.extend(|focused| select_rule.apply(focused)));
            }
        });
    });
}

fn bench_sentence_construction(c: &mut Criterion) {
    c.bench_function("cg_build_sentence", |b| {
        b.iter(|| {
            black_box(build_sentence());
        });
    });
}

criterion_group!(
    benches,
    bench_full_cg_pipeline,
    bench_single_rule_extend,
    bench_sentence_construction,
);
criterion_main!(benches);
