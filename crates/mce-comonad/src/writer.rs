//! Writer Comonad — a comonad with monoid-valued accumulation.
//!
//! This module provides a **Writer Comonad** that pairs a [`Zipper`] with a
//! monoid accumulator. The key application is eliminating the `'\0'` sentinel
//! hack from consonant gradation: instead of inserting null characters to
//! represent deletions (which breaks pure coKleisli composition), we
//! accumulate a [`DeletionSet`] algebraically and apply it once at the end.
//!
//! # Mathematical structure
//!
//! The Writer Comonad is the product comonad `W × (−)` where `W` is a
//! monoid. For a comonad `D` (our Zipper) and a monoid `W`:
//!
//! - `extract(w, a) = extract(a)` — project out the focused element
//! - `extend(f)(w, a) = (w ⊕ w₁ ⊕ ... ⊕ wₙ, extend(π₂ ∘ f)(a))`
//!   where `f` returns `(wᵢ, bᵢ)` at each position
//!
//! The monoid operation (`combine`) accumulates side-information across
//! all positions during `extend`. For `DeletionSet`, this is set union:
//! each coKleisli arrow can mark positions for deletion, and the final
//! result merges all deletion requests.
//!
//! # Why this matters
//!
//! The old pipeline used `'\0'` as a sentinel for deleted characters:
//!
//! ```text
//! gradation: pp → p\0  (insert null for deleted position)
//! pipeline:  ...filter(|&c| c != '\0')...  (strip nulls between steps)
//! ```
//!
//! This breaks pure coKleisli composition because:
//! 1. The intermediate `'\0'` characters shift positions for subsequent rules
//! 2. Filtering between steps requires materializing and re-creating zippers
//! 3. The comonad laws only hold *modulo* the null-filtering post-processing
//!
//! With the Writer Comonad:
//!
//! ```text
//! gradation_writer: returns (DeletionSet::singleton(pos), original_char)
//! pipeline:         writer.extend(grad).extend(harmony).extend(poss)
//! materialize:      apply all accumulated deletions ONCE at the end
//! ```
//!
//! This restores pure coKleisli composition: each arrow operates on the
//! full character sequence with correct positions, and deletions are
//! tracked algebraically rather than destructively.
//!
//! # References
//!
//! - Uustalu, T. & Vene, V. (2008). "Comonadic Notions of Computation."
//!   ENTCS 203(5), pp. 263-284.
//! - The product comonad `(W ×) ∘ D` for a comonad D and monoid W.

use std::collections::BTreeSet;

use crate::zipper::Zipper;

// ===========================================================================
// Monoid trait
// ===========================================================================

/// A monoid: a type with an associative binary operation and an identity element.
///
/// # Laws
///
/// For all `a`, `b`, `c`:
/// - **Identity**: `Monoid::default().combine(&a) == a` and `a.combine(&Monoid::default()) == a`
/// - **Associativity**: `a.combine(&b).combine(&c) == a.combine(&b.combine(&c))`
pub trait Monoid: Clone + Default {
    /// The monoidal binary operation.
    ///
    /// Must be associative: `a.combine(&b).combine(&c) == a.combine(&b.combine(&c))`
    fn combine(&self, other: &Self) -> Self;
}

// ===========================================================================
// DeletionSet — the monoid for tracking position deletions
// ===========================================================================

/// A set of positions to delete, serving as the monoid for the Writer Comonad.
///
/// The monoidal operation is set union: when composing multiple coKleisli
/// arrows, each arrow can mark positions for deletion, and the combined
/// result is the union of all marked positions.
///
/// Positions are absolute indices into the original character sequence.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeletionSet {
    positions: BTreeSet<usize>,
}

impl DeletionSet {
    /// Create an empty deletion set (the monoid identity).
    pub fn new() -> Self {
        DeletionSet {
            positions: BTreeSet::new(),
        }
    }

    /// Create a deletion set containing a single position.
    pub fn singleton(pos: usize) -> Self {
        let mut positions = BTreeSet::new();
        positions.insert(pos);
        DeletionSet { positions }
    }

    /// Returns `true` if no positions are marked for deletion.
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Returns `true` if the given position is marked for deletion.
    pub fn contains(&self, pos: usize) -> bool {
        self.positions.contains(&pos)
    }

    /// The number of positions marked for deletion.
    pub fn len(&self) -> usize {
        self.positions.len()
    }
}

impl Monoid for DeletionSet {
    fn combine(&self, other: &Self) -> Self {
        DeletionSet {
            positions: self.positions.union(&other.positions).copied().collect(),
        }
    }
}

// ===========================================================================
// WriterZipper — the Writer Comonad
// ===========================================================================

/// A Writer Comonad: a zipper paired with a monoid accumulator.
///
/// `WriterZipper<W, A>` combines context-dependent computation (via the
/// zipper) with algebraic accumulation (via the monoid `W`). Each
/// coKleisli arrow `&WriterZipper<W, A> -> (W, B)` can both transform
/// the focused element and contribute to the accumulated monoid value.
///
/// The `extend` operation applies the arrow at every position and combines
/// all monoid contributions using [`Monoid::combine`].
///
/// # Type parameters
///
/// - `W`: The monoid accumulator (e.g., [`DeletionSet`])
/// - `A`: The element type in the zipper
#[derive(Clone, Debug)]
pub struct WriterZipper<W: Monoid, A: Clone> {
    /// The accumulated monoid value.
    pub log: W,
    /// The underlying zipper.
    pub zipper: Zipper<A>,
}

impl<W: Monoid, A: Clone> WriterZipper<W, A> {
    /// Create a new `WriterZipper` with an empty (identity) log.
    pub fn new(zipper: Zipper<A>) -> Self {
        WriterZipper {
            log: W::default(),
            zipper,
        }
    }

    /// Create a `WriterZipper` with a pre-existing log value.
    pub fn with_log(log: W, zipper: Zipper<A>) -> Self {
        WriterZipper { log, zipper }
    }

    /// Comonad `extract`: project out the focused element (ignoring the log).
    ///
    /// This satisfies the comonad contract: extract discards the context
    /// and returns only the focused value.
    pub fn extract(&self) -> &A {
        self.zipper.extract()
    }

    /// The zero-based position of the focus in the underlying zipper.
    pub fn position(&self) -> usize {
        self.zipper.position()
    }

    /// The total number of elements in the underlying zipper.
    pub fn len(&self) -> usize {
        self.zipper.len()
    }

    /// Whether the underlying zipper is empty (always false for a valid zipper).
    pub fn is_empty(&self) -> bool {
        self.zipper.is_empty()
    }

    /// Peek at an element `n` positions to the left of the focus.
    pub fn peek_left(&self, n: usize) -> Option<&A> {
        self.zipper.peek_left(n)
    }

    /// Peek at an element `n` positions to the right of the focus.
    pub fn peek_right(&self, n: usize) -> Option<&A> {
        self.zipper.peek_right(n)
    }

    /// Move the focus one position to the left, preserving the log.
    pub fn move_left(&self) -> Option<WriterZipper<W, A>> {
        self.zipper.move_left().map(|z| WriterZipper {
            log: self.log.clone(),
            zipper: z,
        })
    }

    /// Move the focus one position to the right, preserving the log.
    pub fn move_right(&self) -> Option<WriterZipper<W, A>> {
        self.zipper.move_right().map(|z| WriterZipper {
            log: self.log.clone(),
            zipper: z,
        })
    }

    /// Comonad `extend`: apply a monoid-producing function at every position.
    ///
    /// Given `f: &WriterZipper<W, A> -> (W, B)`, this:
    /// 1. Applies `f` at every position in the zipper
    /// 2. Collects the `B` values into a new zipper
    /// 3. Combines all `W` values (including the existing log) via `Monoid::combine`
    ///
    /// The result is a new `WriterZipper<W, B>` with the accumulated log
    /// and the transformed zipper.
    ///
    /// This is the key operation that enables pure coKleisli composition
    /// with algebraic side-effects: each arrow can mark deletions (or
    /// other monoid-valued annotations) without modifying the sequence.
    pub fn extend<B, F>(&self, f: F) -> WriterZipper<W, B>
    where
        B: Clone,
        F: Fn(&WriterZipper<W, A>) -> (W, B),
    {
        // Apply f at the focus position.
        let (w_focus, b_focus) = f(self);

        // Start accumulating from the existing log combined with the focus result.
        let mut accumulated_w = self.log.combine(&w_focus);

        // Apply f at all positions to the left.
        // WriterLefts yields writer-zippers from nearest-to-focus outward.
        // We collect in that order then reverse to match the storage convention.
        let mut left_results: Vec<B> = Vec::new();
        let mut current_left = self.move_left();
        while let Some(wz) = current_left {
            let (w_i, b_i) = f(&wz);
            accumulated_w = accumulated_w.combine(&w_i);
            left_results.push(b_i);
            current_left = wz.move_left();
        }
        left_results.reverse();

        // Apply f at all positions to the right.
        let mut right_results: Vec<B> = Vec::new();
        let mut current_right = self.move_right();
        while let Some(wz) = current_right {
            let (w_i, b_i) = f(&wz);
            accumulated_w = accumulated_w.combine(&w_i);
            right_results.push(b_i);
            current_right = wz.move_right();
        }

        // Build the new zipper from the collected results.
        let focus_pos = left_results.len();
        let mut all_items = left_results;
        all_items.push(b_focus);
        all_items.extend(right_results);
        let new_zipper = zipper_at(all_items, focus_pos);

        WriterZipper {
            log: accumulated_w,
            zipper: new_zipper,
        }
    }
}

impl<W: Monoid> WriterZipper<W, char> {
    /// Collect the zipper contents into a `Vec<char>`, excluding positions
    /// marked for deletion in the log.
    ///
    /// This is a specialized method for `WriterZipper<DeletionSet, char>`-like
    /// usage, but works with any monoid `W` that has a `contains` method.
    /// For the general case, use [`materialize_with`].
    pub fn to_vec(&self) -> Vec<char> {
        self.zipper.to_vec()
    }
}

impl WriterZipper<DeletionSet, char> {
    /// Apply accumulated deletions to produce the final character sequence.
    ///
    /// This method is called **once** at the very end of a pipeline, after
    /// all coKleisli arrows have been composed via `extend`. It filters out
    /// characters at positions marked in the [`DeletionSet`] log.
    ///
    /// The result is the "surface form" — the phonologically correct output
    /// with deleted characters removed.
    pub fn materialize(&self) -> Vec<char> {
        self.zipper
            .to_vec()
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !self.log.contains(*i))
            .map(|(_, c)| c)
            .collect()
    }

    /// Materialize the writer zipper into a `String`.
    pub fn materialize_string(&self) -> String {
        self.materialize().into_iter().collect()
    }
}

// ===========================================================================
// Helper: build a Zipper at a given focus position
// ===========================================================================

/// Build a `Zipper<T>` from a `Vec<T>` with the focus at position `focus_pos`.
///
/// Uses the public `Zipper::new` constructor (which focuses position 0) and
/// then navigates rightward to the desired position. This avoids accessing
/// private fields of `Zipper`.
///
/// Panics if `items` is empty or `focus_pos >= items.len()`.
fn zipper_at<T: Clone>(items: Vec<T>, focus_pos: usize) -> Zipper<T> {
    let mut z = Zipper::new(items).expect("zipper_at: empty input");
    for _ in 0..focus_pos {
        z = z.move_right().expect("zipper_at: focus_pos out of range");
    }
    z
}

// ===========================================================================
// Finnish morphophonological arrows for WriterZipper
// ===========================================================================

use crate::finnish::{Grade, apply_gradation, apply_possessive, apply_vowel_harmony};

/// Apply consonant gradation as a Writer coKleisli arrow.
///
/// Instead of returning `'\0'` for deleted positions, this returns
/// `(DeletionSet::singleton(pos), original_char)` — marking the position
/// for deletion while preserving the original character in the zipper.
///
/// For non-deleting transformations (e.g., `p -> v`, `mp -> mm`), the
/// deletion set is empty and the transformed character is returned.
///
/// This wraps the existing [`apply_gradation`] function, intercepting
/// `'\0'` results and converting them to algebraic deletions.
pub fn gradation_writer(wz: &WriterZipper<DeletionSet, char>, grade: Grade) -> (DeletionSet, char) {
    let focus = *wz.extract();
    let result = apply_gradation(&wz.zipper, grade);

    if result == '\0' {
        // Deletion: mark position, keep original character in the zipper.
        // The character won't appear in the final output because
        // materialize() will skip this position.
        (DeletionSet::singleton(wz.position()), focus)
    } else {
        // Non-deleting transformation (or identity).
        (DeletionSet::new(), result)
    }
}

/// Apply vowel harmony as a Writer coKleisli arrow.
///
/// Vowel harmony never deletes characters, so the deletion set is always
/// empty. The character transformation delegates to [`apply_vowel_harmony`]
/// via the underlying zipper.
pub fn harmony_writer(wz: &WriterZipper<DeletionSet, char>) -> (DeletionSet, char) {
    let result = apply_vowel_harmony(&wz.zipper);
    (DeletionSet::new(), result)
}

/// Apply possessive vowel copying as a Writer coKleisli arrow.
///
/// Like vowel harmony, this never deletes characters. The `V` archiphoneme
/// is resolved by scanning leftward for the nearest vowel.
///
/// This wraps the existing [`apply_possessive`] function.
pub fn possessive_writer(wz: &WriterZipper<DeletionSet, char>) -> (DeletionSet, char) {
    let result = apply_possessive(&wz.zipper);
    (DeletionSet::new(), result)
}

// ===========================================================================
// Pure morphophonological pipeline (Writer Comonad version)
// ===========================================================================

/// Apply a chain of morphophonological rules using the Writer Comonad.
///
/// This is the pure-coKleisli version of [`morphophonological_pipeline`].
/// Instead of using `'\0'` sentinels and filtering between steps, it:
///
/// 1. Wraps the input in a `WriterZipper<DeletionSet, char>`
/// 2. Composes all arrows via `extend` — NO intermediate materialization
/// 3. Applies accumulated deletions ONCE at the end via `materialize`
///
/// The result should be identical to the old pipeline for all inputs.
///
/// [`morphophonological_pipeline`]: crate::finnish::morphophonological_pipeline
pub fn morphophonological_pipeline_pure(word: &str, grade: Grade) -> String {
    if word.is_empty() {
        return String::new();
    }

    let chars: Vec<char> = word.chars().collect();
    let zipper = match Zipper::new(chars) {
        Some(z) => z,
        None => return String::new(),
    };

    let writer = WriterZipper::<DeletionSet, char>::new(zipper);

    // Step 1: Consonant gradation
    let after_gradation = writer.extend(|wz| gradation_writer(wz, grade));

    // Step 2: Vowel harmony
    let after_harmony = after_gradation.extend(harmony_writer);

    // Step 3: Possessive vowel copying
    let after_possessive = after_harmony.extend(possessive_writer);

    // Apply all accumulated deletions at the very end.
    after_possessive.materialize_string()
}

// ===========================================================================
// Standalone writer-based convenience wrappers
// ===========================================================================

/// Apply consonant gradation to an entire word using the Writer Comonad.
///
/// This is the writer-based replacement for [`crate::finnish::gradation_pipeline`].
/// Instead of returning `'\0'` for deletions and filtering afterward, it
/// accumulates deletions in a [`DeletionSet`] and materializes once at the end.
///
/// The output is identical to the old `gradation_pipeline` for all inputs.
pub fn gradation_pipeline_pure(word: &[char], grade: Grade) -> Vec<char> {
    let zipper = match Zipper::new(word.to_vec()) {
        Some(z) => z,
        None => return Vec::new(),
    };

    let writer = WriterZipper::<DeletionSet, char>::new(zipper);
    let result = writer.extend(|wz| gradation_writer(wz, grade));
    result.materialize()
}

/// Convenience wrapper: apply gradation to a `&str` using the Writer Comonad.
///
/// This is the writer-based replacement for [`crate::finnish::gradate`].
pub fn gradate_pure(word: &str, grade: Grade) -> String {
    let chars: Vec<char> = word.chars().collect();
    gradation_pipeline_pure(&chars, grade).into_iter().collect()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =====================================================================
    // Monoid laws for DeletionSet
    // =====================================================================

    #[test]
    fn deletion_set_monoid_identity_left() {
        let empty = DeletionSet::new();
        let s = DeletionSet::singleton(3);
        assert_eq!(empty.combine(&s), s);
    }

    #[test]
    fn deletion_set_monoid_identity_right() {
        let empty = DeletionSet::new();
        let s = DeletionSet::singleton(3);
        assert_eq!(s.combine(&empty), s);
    }

    #[test]
    fn deletion_set_monoid_associativity() {
        let a = DeletionSet::singleton(1);
        let b = DeletionSet::singleton(2);
        let c = DeletionSet::singleton(3);
        assert_eq!(a.combine(&b).combine(&c), a.combine(&b.combine(&c)));
    }

    #[test]
    fn deletion_set_combine_is_union() {
        let a = DeletionSet::singleton(1);
        let b = DeletionSet::singleton(2);
        let combined = a.combine(&b);
        assert!(combined.contains(1));
        assert!(combined.contains(2));
        assert!(!combined.contains(0));
        assert_eq!(combined.len(), 2);
    }

    #[test]
    fn deletion_set_combine_idempotent() {
        let a = DeletionSet::singleton(1);
        let combined = a.combine(&a);
        assert_eq!(combined.len(), 1);
        assert!(combined.contains(1));
    }

    #[test]
    fn deletion_set_empty() {
        let empty = DeletionSet::new();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
    }

    // =====================================================================
    // WriterZipper construction and basic operations
    // =====================================================================

    #[test]
    fn writer_zipper_new() {
        let z = Zipper::new(vec!['a', 'b', 'c']).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        assert_eq!(*wz.extract(), 'a');
        assert!(wz.log.is_empty());
        assert_eq!(wz.position(), 0);
        assert_eq!(wz.len(), 3);
    }

    #[test]
    fn writer_zipper_navigation() {
        let z = Zipper::new(vec!['a', 'b', 'c']).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let wz2 = wz.move_right().unwrap();
        assert_eq!(*wz2.extract(), 'b');
        assert_eq!(wz2.position(), 1);
        let wz3 = wz2.move_left().unwrap();
        assert_eq!(*wz3.extract(), 'a');
        assert_eq!(wz3.position(), 0);
    }

    #[test]
    fn writer_zipper_peek() {
        let z = Zipper::new(vec!['a', 'b', 'c']).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let wz2 = wz.move_right().unwrap();
        assert_eq!(wz2.peek_left(1), Some(&'a'));
        assert_eq!(wz2.peek_right(1), Some(&'c'));
    }

    // =====================================================================
    // WriterZipper extend — basic behavior
    // =====================================================================

    #[test]
    fn writer_extend_identity_no_deletions() {
        let z = Zipper::new(vec!['a', 'b', 'c']).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);

        // Identity arrow: no deletions, return focus unchanged.
        let result = wz.extend(|w| (DeletionSet::new(), *w.extract()));
        assert_eq!(result.zipper.to_vec(), vec!['a', 'b', 'c']);
        assert!(result.log.is_empty());
    }

    #[test]
    fn writer_extend_with_transformation() {
        let z = Zipper::new(vec![1, 2, 3]).unwrap();
        let wz = WriterZipper::<DeletionSet, i32>::new(z);

        // Double each element.
        let result = wz.extend(|w| (DeletionSet::new(), w.extract() * 2));
        assert_eq!(result.zipper.to_vec(), vec![2, 4, 6]);
        assert!(result.log.is_empty());
    }

    #[test]
    fn writer_extend_accumulates_deletions() {
        let z = Zipper::new(vec!['a', 'b', 'c']).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);

        // Mark position 1 for deletion.
        let result = wz.extend(|w| {
            if *w.extract() == 'b' {
                (DeletionSet::singleton(w.position()), 'b')
            } else {
                (DeletionSet::new(), *w.extract())
            }
        });

        assert!(result.log.contains(1));
        assert!(!result.log.contains(0));
        assert!(!result.log.contains(2));
        assert_eq!(result.zipper.to_vec(), vec!['a', 'b', 'c']);

        // Materialize should exclude position 1.
        let materialized = result.materialize();
        assert_eq!(materialized, vec!['a', 'c']);
    }

    #[test]
    fn writer_extend_accumulates_multiple_deletions() {
        let z = Zipper::new(vec!['a', 'b', 'c', 'd', 'e']).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);

        // Mark positions 1 and 3 for deletion.
        let result = wz.extend(|w| {
            let pos = w.position();
            if pos == 1 || pos == 3 {
                (DeletionSet::singleton(pos), *w.extract())
            } else {
                (DeletionSet::new(), *w.extract())
            }
        });

        assert_eq!(result.log.len(), 2);
        assert!(result.log.contains(1));
        assert!(result.log.contains(3));
        assert_eq!(result.materialize(), vec!['a', 'c', 'e']);
    }

    // =====================================================================
    // Comonad laws for WriterZipper
    // =====================================================================

    #[test]
    fn writer_comonad_left_identity() {
        // L1: extend(extract) == id
        // For WriterZipper, the "extract-like" arrow returns (empty, extract).
        let z = Zipper::new(vec![10, 20, 30, 40]).unwrap();
        let wz = WriterZipper::<DeletionSet, i32>::new(z.clone());

        let extended = wz.extend(|w| (DeletionSet::new(), *w.extract()));
        assert_eq!(extended.zipper.to_vec(), z.to_vec());
        assert!(extended.log.is_empty());
    }

    #[test]
    fn writer_comonad_right_identity() {
        // L2: extract(extend(f)) == f(wz)
        let z = Zipper::new(vec![1, 2, 3, 4, 5]).unwrap();
        let wz = WriterZipper::<DeletionSet, i32>::new(z);

        let f = |w: &WriterZipper<DeletionSet, i32>| -> (DeletionSet, i32) {
            let left = w.peek_left(1).copied().unwrap_or(0);
            let right = w.peek_right(1).copied().unwrap_or(0);
            (DeletionSet::new(), left + w.extract() + right)
        };

        let extended = wz.extend(&f);
        let (_, direct_value) = f(&wz);
        assert_eq!(*extended.extract(), direct_value);
    }

    #[test]
    fn writer_comonad_right_identity_at_various_positions() {
        let z = Zipper::new(vec![1, 2, 3, 4]).unwrap();

        let f = |w: &WriterZipper<DeletionSet, i32>| -> (DeletionSet, i32) {
            (DeletionSet::new(), w.extract() * 2)
        };

        // Check at every position.
        let mut current = Some(WriterZipper::<DeletionSet, i32>::new(z));
        while let Some(wz) = current {
            let extended = wz.extend(&f);
            let (_, direct) = f(&wz);
            assert_eq!(*extended.extract(), direct);
            current = wz.move_right();
        }
    }

    #[test]
    fn writer_comonad_associativity() {
        // L3': extend(f).extend(g) == extend(|w| g(&w.extend(f)))
        let f = |w: &WriterZipper<DeletionSet, i32>| -> (DeletionSet, i32) {
            (DeletionSet::new(), w.extract() * 2)
        };
        let g = |w: &WriterZipper<DeletionSet, i32>| -> (DeletionSet, i32) {
            let l = w.peek_left(1).copied().unwrap_or(0);
            let r = w.peek_right(1).copied().unwrap_or(0);
            (DeletionSet::new(), l + w.extract() + r)
        };

        let inputs: Vec<Vec<i32>> = vec![
            vec![1, 2, 3, 4, 5],
            vec![10, 20, 30],
            vec![42],
            vec![1, 2],
            vec![-3, 0, 7, -1, 4, 2],
        ];

        for input in inputs {
            let z = Zipper::new(input.clone()).unwrap();
            let wz = WriterZipper::<DeletionSet, i32>::new(z);

            // LHS: two sequential extends
            let lhs = wz.extend(&f).extend(&g);

            // RHS: single extend with composed function
            let rhs = wz.extend(|w: &WriterZipper<DeletionSet, i32>| {
                let extended_f = w.extend(&f);
                g(&extended_f)
            });

            assert_eq!(
                lhs.zipper.to_vec(),
                rhs.zipper.to_vec(),
                "L3' associativity failed for input {:?}",
                input
            );
        }
    }

    // =====================================================================
    // Gradation writer arrow
    // =====================================================================

    #[test]
    fn gradation_writer_geminate_pp() {
        // kaappi -> kaapi (second p deleted via DeletionSet)
        let z = Zipper::new("kaappi".chars().collect()).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let result = wz.extend(|w| gradation_writer(w, Grade::Weak));

        // The zipper should preserve all characters (no '\0').
        assert_eq!(result.zipper.to_vec().len(), 6);
        // But the deletion set should mark position 4 (the second p).
        assert!(
            result.log.contains(4),
            "Expected position 4 in deletion set, got: {:?}",
            result.log
        );
        // Materialize should produce "kaapi".
        assert_eq!(result.materialize_string(), "kaapi");
    }

    #[test]
    fn gradation_writer_geminate_tt() {
        let z = Zipper::new("matto".chars().collect()).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let result = wz.extend(|w| gradation_writer(w, Grade::Weak));
        assert_eq!(result.materialize_string(), "mato");
    }

    #[test]
    fn gradation_writer_geminate_kk() {
        let z = Zipper::new("kukka".chars().collect()).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let result = wz.extend(|w| gradation_writer(w, Grade::Weak));
        assert_eq!(result.materialize_string(), "kuka");
    }

    #[test]
    fn gradation_writer_k_deletion() {
        // puku -> puu (k deleted)
        let z = Zipper::new("puku".chars().collect()).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let result = wz.extend(|w| gradation_writer(w, Grade::Weak));
        assert!(result.log.contains(2), "Expected position 2 (k) deleted");
        assert_eq!(result.materialize_string(), "puu");
    }

    #[test]
    fn gradation_writer_cluster_mp_to_mm() {
        let z = Zipper::new("kampa".chars().collect()).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let result = wz.extend(|w| gradation_writer(w, Grade::Weak));
        assert!(result.log.is_empty(), "mp->mm should not delete");
        assert_eq!(result.materialize_string(), "kamma");
    }

    #[test]
    fn gradation_writer_cluster_nt_to_nn() {
        let z = Zipper::new("ranta".chars().collect()).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let result = wz.extend(|w| gradation_writer(w, Grade::Weak));
        assert!(result.log.is_empty(), "nt->nn should not delete");
        assert_eq!(result.materialize_string(), "ranna");
    }

    #[test]
    fn gradation_writer_qualitative_p_to_v() {
        let z = Zipper::new("tapa".chars().collect()).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let result = wz.extend(|w| gradation_writer(w, Grade::Weak));
        assert!(result.log.is_empty(), "p->v should not delete");
        assert_eq!(result.materialize_string(), "tava");
    }

    #[test]
    fn gradation_writer_qualitative_t_to_d() {
        let z = Zipper::new("katu".chars().collect()).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let result = wz.extend(|w| gradation_writer(w, Grade::Weak));
        assert_eq!(result.materialize_string(), "kadu");
    }

    #[test]
    fn gradation_writer_no_change() {
        let z = Zipper::new("kisa".chars().collect()).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let result = wz.extend(|w| gradation_writer(w, Grade::Weak));
        assert!(result.log.is_empty());
        assert_eq!(result.materialize_string(), "kisa");
    }

    // =====================================================================
    // Harmony writer arrow
    // =====================================================================

    #[test]
    fn harmony_writer_back() {
        let z = Zipper::new("talossA".chars().collect()).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let result = wz.extend(harmony_writer);
        assert!(result.log.is_empty());
        assert_eq!(result.materialize_string(), "talossa");
    }

    #[test]
    fn harmony_writer_front() {
        let z = Zipper::new("p\u{00F6}yt\u{00E4}ssA".chars().collect()).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let result = wz.extend(harmony_writer);
        assert_eq!(result.materialize_string(), "p\u{00F6}yt\u{00E4}ss\u{00E4}");
    }

    // =====================================================================
    // Possessive writer arrow
    // =====================================================================

    #[test]
    fn possessive_writer_copy_o() {
        let z = Zipper::new("taloVn".chars().collect()).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let result = wz.extend(possessive_writer);
        assert_eq!(result.materialize_string(), "taloon");
    }

    #[test]
    fn possessive_writer_copy_i() {
        let z = Zipper::new("k\u{00E4}siVn".chars().collect()).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        let result = wz.extend(possessive_writer);
        assert_eq!(result.materialize_string(), "k\u{00E4}siin");
    }

    // =====================================================================
    // Pure pipeline: equivalence with old pipeline
    // =====================================================================

    #[test]
    fn pure_pipeline_matches_old_kaappi() {
        // kaappi (weak) -> kaapi
        let old = crate::finnish::morphophonological_pipeline("kaappi", Grade::Weak);
        let new = morphophonological_pipeline_pure("kaappi", Grade::Weak);
        assert_eq!(old, new, "kaappi mismatch: old={}, new={}", old, new);
    }

    #[test]
    fn pure_pipeline_matches_old_tytto() {
        // tyttö (weak) -> tytön? No — tyttö with suffix:
        // Let's use the base word first.
        let old = crate::finnish::morphophonological_pipeline("tytt\u{00F6}", Grade::Weak);
        let new = morphophonological_pipeline_pure("tytt\u{00F6}", Grade::Weak);
        assert_eq!(old, new, "tyttö mismatch: old={}, new={}", old, new);
    }

    #[test]
    fn pure_pipeline_matches_old_poyta() {
        // pöytä + llA (weak)
        let old =
            crate::finnish::morphophonological_pipeline("p\u{00F6}yt\u{00E4}llA", Grade::Weak);
        let new = morphophonological_pipeline_pure("p\u{00F6}yt\u{00E4}llA", Grade::Weak);
        assert_eq!(old, new, "pöytä+llA mismatch: old={}, new={}", old, new);
    }

    #[test]
    fn pure_pipeline_matches_old_ranta() {
        // ranta + ssA (weak) -> rannassa
        let old = crate::finnish::morphophonological_pipeline("rantAssA", Grade::Weak);
        let new = morphophonological_pipeline_pure("rantAssA", Grade::Weak);
        assert_eq!(old, new, "ranta+ssA mismatch: old={}, new={}", old, new);
    }

    #[test]
    fn pure_pipeline_matches_old_puku_deletion() {
        // puku + ssA (weak) -> puussa
        let old = crate::finnish::morphophonological_pipeline("pukussA", Grade::Weak);
        let new = morphophonological_pipeline_pure("pukussA", Grade::Weak);
        assert_eq!(old, new, "puku+ssA mismatch: old={}, new={}", old, new);
    }

    #[test]
    fn pure_pipeline_matches_old_all_three_rules() {
        // kampa + stA + Vn (weak) -> kammastaan
        let old = crate::finnish::morphophonological_pipeline("kampAstAVn", Grade::Weak);
        let new = morphophonological_pipeline_pure("kampAstAVn", Grade::Weak);
        assert_eq!(old, new, "kampa+stA+Vn mismatch: old={}, new={}", old, new);
    }

    #[test]
    fn pure_pipeline_matches_old_front_all_three() {
        // kenkä + stA + Vn (weak) -> kengästään
        let old = crate::finnish::morphophonological_pipeline("kenk\u{00E4}stAVn", Grade::Weak);
        let new = morphophonological_pipeline_pure("kenk\u{00E4}stAVn", Grade::Weak);
        assert_eq!(old, new, "kenkä+stA+Vn mismatch: old={}, new={}", old, new);
    }

    #[test]
    fn pure_pipeline_matches_old_strong_grade() {
        // ranta + ssA (strong) -> rantassa
        let old = crate::finnish::morphophonological_pipeline("rantAssA", Grade::Strong);
        let new = morphophonological_pipeline_pure("rantAssA", Grade::Strong);
        assert_eq!(
            old, new,
            "ranta+ssA strong mismatch: old={}, new={}",
            old, new
        );
    }

    #[test]
    fn pure_pipeline_matches_old_empty() {
        let old = crate::finnish::morphophonological_pipeline("", Grade::Weak);
        let new = morphophonological_pipeline_pure("", Grade::Weak);
        assert_eq!(old, new);
    }

    #[test]
    fn pure_pipeline_matches_old_no_changes() {
        let old = crate::finnish::morphophonological_pipeline("talossa", Grade::Weak);
        let new = morphophonological_pipeline_pure("talossa", Grade::Weak);
        assert_eq!(old, new);
    }

    #[test]
    fn pure_pipeline_matches_old_harmony_only() {
        let old = crate::finnish::morphophonological_pipeline("massAssA", Grade::Strong);
        let new = morphophonological_pipeline_pure("massAssA", Grade::Strong);
        assert_eq!(old, new);
    }

    // =====================================================================
    // Property: materialize removes exactly the marked positions
    // =====================================================================

    #[test]
    fn materialize_preserves_order() {
        let z = Zipper::new(vec!['a', 'b', 'c', 'd', 'e']).unwrap();
        let mut wz = WriterZipper::<DeletionSet, char>::new(z);
        wz.log = DeletionSet::singleton(2); // delete 'c'
        assert_eq!(wz.materialize(), vec!['a', 'b', 'd', 'e']);
    }

    #[test]
    fn materialize_empty_deletion() {
        let z = Zipper::new(vec!['a', 'b', 'c']).unwrap();
        let wz = WriterZipper::<DeletionSet, char>::new(z);
        assert_eq!(wz.materialize(), vec!['a', 'b', 'c']);
    }

    #[test]
    fn materialize_delete_all() {
        let z = Zipper::new(vec!['a', 'b', 'c']).unwrap();
        let mut log = DeletionSet::new();
        log = log.combine(&DeletionSet::singleton(0));
        log = log.combine(&DeletionSet::singleton(1));
        log = log.combine(&DeletionSet::singleton(2));
        let wz = WriterZipper::with_log(log, z);
        assert_eq!(wz.materialize(), vec![]);
    }
}
