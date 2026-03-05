//! Property-based tests for comonad laws and Finnish morphophonological invariants.

use mce_comonad::Zipper;
use mce_comonad::finnish::{Grade, harmonize, morphophonological_pipeline};
use mce_comonad::writer::{
    DeletionSet, Monoid, WriterZipper, gradation_pipeline_pure, morphophonological_pipeline_pure,
};
use proptest::prelude::*;

// ============================================================================
// Strategy generators
// ============================================================================

/// Strategy for Finnish-like characters: lowercase letters including umlauts.
fn finnish_chars() -> impl Strategy<Value = char> {
    prop_oneof![
        // Common Finnish letters (a-z as individual chars via regex)
        proptest::char::range('a', 'z'),
        // Finnish-specific vowels
        Just('\u{00E4}'), // ä
        Just('\u{00F6}'), // ö
    ]
}

/// Strategy for non-empty Finnish-like strings (1..=max_len chars).
fn finnish_string(max_len: usize) -> impl Strategy<Value = String> {
    proptest::collection::vec(finnish_chars(), 1..=max_len).prop_map(|v| v.into_iter().collect())
}

/// Strategy for words with archiphonemic suffix markers (A, O, U, V).
fn finnish_word_with_suffix() -> impl Strategy<Value = String> {
    (
        finnish_string(8),
        prop_oneof![
            Just("ssA"),
            Just("llA"),
            Just("stA"),
            Just("Vn"),
            Just("ssA"),
            Just("nA"),
        ],
    )
        .prop_map(|(stem, suffix)| format!("{}{}", stem, suffix))
}

// ============================================================================
// Monoid laws for DeletionSet
// ============================================================================

proptest! {
    #[test]
    fn monoid_left_identity(positions in proptest::collection::vec(0..100usize, 0..10)) {
        let empty = DeletionSet::new();
        let mut s = DeletionSet::new();
        for p in &positions {
            s = s.combine(&DeletionSet::singleton(*p));
        }
        prop_assert_eq!(empty.combine(&s), s);
    }

    #[test]
    fn monoid_right_identity(positions in proptest::collection::vec(0..100usize, 0..10)) {
        let empty = DeletionSet::new();
        let mut s = DeletionSet::new();
        for p in &positions {
            s = s.combine(&DeletionSet::singleton(*p));
        }
        prop_assert_eq!(s.combine(&empty), s);
    }

    #[test]
    fn monoid_associativity(
        a_pos in proptest::collection::vec(0..100usize, 0..5),
        b_pos in proptest::collection::vec(0..100usize, 0..5),
        c_pos in proptest::collection::vec(0..100usize, 0..5),
    ) {
        let mut a = DeletionSet::new();
        for p in &a_pos { a = a.combine(&DeletionSet::singleton(*p)); }
        let mut b = DeletionSet::new();
        for p in &b_pos { b = b.combine(&DeletionSet::singleton(*p)); }
        let mut c = DeletionSet::new();
        for p in &c_pos { c = c.combine(&DeletionSet::singleton(*p)); }

        let lhs = a.combine(&b).combine(&c);
        let rhs = a.combine(&b.combine(&c));
        prop_assert_eq!(lhs, rhs);
    }

    #[test]
    fn monoid_combine_is_idempotent(pos in 0..100usize) {
        let a = DeletionSet::singleton(pos);
        let combined = a.combine(&a);
        prop_assert_eq!(combined.len(), 1);
        prop_assert!(combined.contains(pos));
    }
}

// ============================================================================
// Comonad laws for WriterZipper
// ============================================================================

proptest! {
    #[test]
    fn comonad_left_identity(values in proptest::collection::vec(1..100i32, 1..20)) {
        let z = Zipper::new(values.clone()).unwrap();
        let wz = WriterZipper::<DeletionSet, i32>::new(z);

        // extend(extract) should be identity
        let extended = wz.extend(|w| (DeletionSet::new(), *w.extract()));
        prop_assert_eq!(extended.zipper.to_vec(), values);
        prop_assert!(extended.log.is_empty());
    }

    #[test]
    fn comonad_right_identity(values in proptest::collection::vec(1..100i32, 1..10)) {
        let z = Zipper::new(values).unwrap();
        let wz = WriterZipper::<DeletionSet, i32>::new(z);

        let f = |w: &WriterZipper<DeletionSet, i32>| -> (DeletionSet, i32) {
            (DeletionSet::new(), w.extract() * 2)
        };

        // extract(extend(f)) == f(wz)
        let extended = wz.extend(&f);
        let (_, direct) = f(&wz);
        prop_assert_eq!(*extended.extract(), direct);
    }
}

// ============================================================================
// Vowel harmony idempotence
// ============================================================================

proptest! {
    #[test]
    fn vowel_harmony_idempotent(word in finnish_word_with_suffix()) {
        // Applying harmony twice should give the same result as once.
        let once = harmonize(&word);
        let twice = harmonize(&once);
        prop_assert_eq!(once, twice,
            "Vowel harmony not idempotent for input: {}", word);
    }

    #[test]
    fn vowel_harmony_preserves_length(word in finnish_word_with_suffix()) {
        let result = harmonize(&word);
        // Vowel harmony only substitutes characters, never adds or removes.
        prop_assert_eq!(
            word.chars().count(),
            result.chars().count(),
            "Vowel harmony changed length for: {}", word
        );
    }
}

// ============================================================================
// Writer pipeline equivalence with legacy pipeline
// ============================================================================

proptest! {
    #[test]
    fn pure_pipeline_matches_legacy(
        word in finnish_word_with_suffix(),
        grade in prop_oneof![Just(Grade::Strong), Just(Grade::Weak)],
    ) {
        let old = morphophonological_pipeline(&word, grade);
        let new = morphophonological_pipeline_pure(&word, grade);
        prop_assert_eq!(old, new,
            "Pipeline mismatch for word={}, grade={:?}", word, grade);
    }
}

// ============================================================================
// Gradation pipeline properties
// ============================================================================

proptest! {
    #[test]
    fn gradation_never_increases_length(word in finnish_string(15)) {
        let chars: Vec<char> = word.chars().collect();
        let result = gradation_pipeline_pure(&chars, Grade::Weak);
        // Gradation can only reduce length (geminate weakening deletes a char)
        // or keep it the same.
        prop_assert!(
            result.len() <= chars.len(),
            "Gradation increased length for: {} ({} -> {})",
            word, chars.len(), result.len()
        );
    }

    #[test]
    fn gradation_weak_never_longer_than_strong(word in finnish_string(15)) {
        let chars: Vec<char> = word.chars().collect();
        let weak = gradation_pipeline_pure(&chars, Grade::Weak);
        let strong = gradation_pipeline_pure(&chars, Grade::Strong);
        // Weak grade can delete geminates, so its output is at most
        // as long as the strong grade output.
        prop_assert!(
            weak.len() <= strong.len(),
            "Weak gradation longer than strong for: {} (weak={}, strong={})",
            word, weak.len(), strong.len()
        );
    }
}

// ============================================================================
// WriterZipper materialize correctness
// ============================================================================

proptest! {
    #[test]
    fn materialize_deletes_exactly_marked(
        chars in proptest::collection::vec(proptest::char::range('a', 'z'), 1..=20),
        delete_indices in proptest::collection::vec(0..20usize, 0..5),
    ) {
        let z = Zipper::new(chars.clone()).unwrap();
        let mut log = DeletionSet::new();
        for &i in &delete_indices {
            if i < chars.len() {
                log = log.combine(&DeletionSet::singleton(i));
            }
        }
        let wz = WriterZipper::with_log(log.clone(), z);
        let result = wz.materialize();

        // Every non-deleted character should be in the result.
        let expected: Vec<char> = chars.iter().enumerate()
            .filter(|(i, _)| !log.contains(*i))
            .map(|(_, &c)| c)
            .collect();
        prop_assert_eq!(result, expected);
    }
}
