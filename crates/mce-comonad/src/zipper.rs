//! ListZipper comonad — a focused, navigable sequence.
//!
//! A `Zipper<T>` represents a non-empty list with a distinguished "focus"
//! element, plus left and right context. This is the canonical 1-D comonad
//! for context-dependent computations.
//!
//! # Comonad operations
//!
//! - [`Zipper::extract`] returns a reference to the focused element.
//! - [`Zipper::extend`] applies a context-dependent function at every position,
//!   producing a new zipper of results. This is the key primitive for expressing
//!   morphophonological rules: each rule is a function `&Zipper<T> -> U` that
//!   inspects the local context (via `peek_left`, `peek_right`) and computes an
//!   output for the focused position. `extend` lifts this local rule into a
//!   global transformation over the entire sequence.
//!
//! # Comonad laws
//!
//! The implementation satisfies the standard comonad laws:
//!
//! 1. **Left identity (L1)**: `z.extend(Zipper::extract) == z`
//!    (extending with extract recovers the original zipper)
//! 2. **Right identity (L2)**: `z.extend(f).extract() == f(&z)`
//!    (extracting after extend yields the same as direct application)
//! 3. **Associativity (L3')**: `z.extend(g).extend(f) == z.extend(|w| f(w.extend(g)))`
//!    (sequential extends equal a single extend of the composed arrow)

/// A list zipper: a non-empty sequence with a distinguished focus element.
///
/// `left` is stored in **reverse** order so that the nearest left neighbor is
/// at the end of the `Vec`, making push/pop O(1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zipper<T> {
    /// Elements to the left of the focus, stored in reverse order.
    /// `left.last()` is the immediate left neighbor of `focus`.
    left: Vec<T>,
    /// The currently focused element.
    focus: T,
    /// Elements to the right of the focus, stored in natural order.
    /// `right.first()` is the immediate right neighbor of `focus`.
    right: Vec<T>,
}

impl<T> Zipper<T> {
    /// Create a new zipper focused on the first element of `items`.
    ///
    /// Returns `None` if the input is empty.
    pub fn new(mut items: Vec<T>) -> Option<Self> {
        if items.is_empty() {
            return None;
        }
        let focus = items.remove(0);
        Some(Zipper {
            left: Vec::new(),
            focus,
            right: items,
        })
    }

    /// Comonad `extract`: returns a reference to the focused element.
    pub fn extract(&self) -> &T {
        &self.focus
    }

    /// The number of elements in the zipper.
    pub fn len(&self) -> usize {
        self.left.len() + 1 + self.right.len()
    }

    /// Returns `true` if the zipper contains exactly one element.
    ///
    /// A zipper is always non-empty, so this is never "is the zipper empty?"
    /// but rather "does it contain a single element?"
    pub fn is_empty(&self) -> bool {
        // A zipper always has at least one element (focus), so this is
        // always false. Provided to satisfy the clippy `len_without_is_empty` lint.
        false
    }

    /// The zero-based index of the current focus within the full sequence.
    pub fn position(&self) -> usize {
        self.left.len()
    }

    /// Look at the element `n` positions to the left of the focus (1-indexed).
    ///
    /// `peek_left(1)` returns the immediate left neighbor.
    /// Returns `None` if `n` is out of range or zero.
    pub fn peek_left(&self, n: usize) -> Option<&T> {
        if n == 0 {
            return None;
        }
        let idx = self.left.len().checked_sub(n)?;
        self.left.get(idx)
    }

    /// Look at the element `n` positions to the right of the focus (1-indexed).
    ///
    /// `peek_right(1)` returns the immediate right neighbor.
    /// Returns `None` if `n` is out of range or zero.
    pub fn peek_right(&self, n: usize) -> Option<&T> {
        if n == 0 {
            return None;
        }
        self.right.get(n - 1)
    }
}

impl<T: Clone> Zipper<T> {
    /// Move the focus one position to the left.
    ///
    /// Returns `None` if the focus is already at the leftmost position.
    pub fn move_left(&self) -> Option<Zipper<T>> {
        let new_focus = self.left.last()?.clone();
        let mut new_left = self.left.clone();
        new_left.pop();
        let mut new_right = Vec::with_capacity(1 + self.right.len());
        new_right.push(self.focus.clone());
        new_right.extend(self.right.iter().cloned());
        Some(Zipper {
            left: new_left,
            focus: new_focus,
            right: new_right,
        })
    }

    /// Move the focus one position to the right.
    ///
    /// Returns `None` if the focus is already at the rightmost position.
    pub fn move_right(&self) -> Option<Zipper<T>> {
        if self.right.is_empty() {
            return None;
        }
        let mut new_right = self.right.clone();
        let new_focus = new_right.remove(0);
        let mut new_left = self.left.clone();
        new_left.push(self.focus.clone());
        Some(Zipper {
            left: new_left,
            focus: new_focus,
            right: new_right,
        })
    }

    /// Collect all elements back into a `Vec` in their natural order.
    pub fn to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.len());
        // left is reversed, so iterate forward
        result.extend(self.left.iter().cloned());
        result.push(self.focus.clone());
        result.extend(self.right.iter().cloned());
        result
    }

    /// Comonad `extend`: apply a context-dependent function at every position.
    ///
    /// Given `f: &Zipper<T> -> U`, `extend` slides the focus across every
    /// position in the sequence, applies `f` at each position, and collects
    /// the results into a new `Zipper<U>`.
    ///
    /// This is the key combinator for morphophonological rule application:
    /// a rule that inspects the local context via `peek_left`/`peek_right`
    /// and produces a transformed element can be extended over the full word.
    pub fn extend<U, F>(&self, f: F) -> Zipper<U>
    where
        F: Fn(&Zipper<T>) -> U,
    {
        let focus_result = f(self);

        // Collect left results nearest-to-focus outward, then reverse
        // to match `self.left` storage convention (farthest first).
        let mut left_results: Vec<U> = Lefts::new(self).map(|z| f(&z)).collect();
        left_results.reverse();

        let right_results: Vec<U> = Rights::new(self).map(|z| f(&z)).collect();

        Zipper {
            left: left_results,
            focus: focus_result,
            right: right_results,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal iterators for walking the zipper leftward / rightward.
// ---------------------------------------------------------------------------

/// Iterator that yields successive left-shifted zippers.
struct Lefts<T: Clone> {
    current: Option<Zipper<T>>,
}

impl<T: Clone> Lefts<T> {
    fn new(start: &Zipper<T>) -> Self {
        Lefts {
            current: start.move_left(),
        }
    }
}

impl<T: Clone> Iterator for Lefts<T> {
    type Item = Zipper<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let zipper = self.current.take()?;
        self.current = zipper.move_left();
        Some(zipper)
    }
}

/// Iterator that yields successive right-shifted zippers.
struct Rights<T: Clone> {
    current: Option<Zipper<T>>,
}

impl<T: Clone> Rights<T> {
    fn new(start: &Zipper<T>) -> Self {
        Rights {
            current: start.move_right(),
        }
    }
}

impl<T: Clone> Iterator for Rights<T> {
    type Item = Zipper<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let zipper = self.current.take()?;
        self.current = zipper.move_right();
        Some(zipper)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Construction ---------------------------------------------------------

    #[test]
    fn new_from_empty_returns_none() {
        let z: Option<Zipper<i32>> = Zipper::new(vec![]);
        assert!(z.is_none());
    }

    #[test]
    fn new_single_element() {
        let z = Zipper::new(vec![42]).unwrap();
        assert_eq!(*z.extract(), 42);
        assert_eq!(z.len(), 1);
        assert_eq!(z.position(), 0);
    }

    #[test]
    fn new_multiple_elements_focuses_first() {
        let z = Zipper::new(vec![1, 2, 3]).unwrap();
        assert_eq!(*z.extract(), 1);
        assert_eq!(z.position(), 0);
        assert_eq!(z.len(), 3);
    }

    // -- Navigation -----------------------------------------------------------

    #[test]
    fn move_right_then_left_roundtrip() {
        let z = Zipper::new(vec![1, 2, 3]).unwrap();
        let z1 = z.move_right().unwrap();
        assert_eq!(*z1.extract(), 2);
        assert_eq!(z1.position(), 1);

        let z2 = z1.move_left().unwrap();
        assert_eq!(*z2.extract(), 1);
        assert_eq!(z2.position(), 0);
        assert_eq!(z, z2);
    }

    #[test]
    fn move_left_at_start_returns_none() {
        let z = Zipper::new(vec![1, 2, 3]).unwrap();
        assert!(z.move_left().is_none());
    }

    #[test]
    fn move_right_at_end_returns_none() {
        let z = Zipper::new(vec![1]).unwrap();
        assert!(z.move_right().is_none());
    }

    #[test]
    fn walk_to_end() {
        let z = Zipper::new(vec![10, 20, 30]).unwrap();
        let z1 = z.move_right().unwrap();
        let z2 = z1.move_right().unwrap();
        assert_eq!(*z2.extract(), 30);
        assert_eq!(z2.position(), 2);
        assert!(z2.move_right().is_none());
    }

    // -- Peek -----------------------------------------------------------------

    #[test]
    fn peek_left_and_right() {
        let z = Zipper::new(vec![1, 2, 3, 4, 5]).unwrap();
        let z2 = z.move_right().unwrap().move_right().unwrap(); // focus = 3
        assert_eq!(z2.peek_left(1), Some(&2));
        assert_eq!(z2.peek_left(2), Some(&1));
        assert_eq!(z2.peek_left(3), None);
        assert_eq!(z2.peek_right(1), Some(&4));
        assert_eq!(z2.peek_right(2), Some(&5));
        assert_eq!(z2.peek_right(3), None);
    }

    #[test]
    fn peek_zero_returns_none() {
        let z = Zipper::new(vec![1, 2, 3]).unwrap();
        assert_eq!(z.peek_left(0), None);
        assert_eq!(z.peek_right(0), None);
    }

    // -- to_vec ---------------------------------------------------------------

    #[test]
    fn to_vec_preserves_order() {
        let original = vec![1, 2, 3, 4, 5];
        let z = Zipper::new(original.clone()).unwrap();
        assert_eq!(z.to_vec(), original);

        // Also works after moving focus
        let z2 = z.move_right().unwrap().move_right().unwrap();
        assert_eq!(z2.to_vec(), original);
    }

    // -- Comonad law: left identity -------------------------------------------
    // extend(extract) == identity
    //
    // That is: if we extend with a function that just extracts the focus,
    // the resulting zipper should be equal to the original.

    #[test]
    fn comonad_left_identity() {
        let z = Zipper::new(vec![10, 20, 30, 40]).unwrap();
        let extended = z.extend(|w| w.extract().clone());
        assert_eq!(extended, z);
    }

    #[test]
    fn comonad_left_identity_after_move() {
        let z = Zipper::new(vec![10, 20, 30, 40])
            .unwrap()
            .move_right()
            .unwrap();
        let extended = z.extend(|w| w.extract().clone());
        assert_eq!(extended, z);
    }

    // -- Comonad law: right identity ------------------------------------------
    // extract(extend(f)) == f(z)
    //
    // That is: extracting from the extended zipper at the focus position
    // yields the same result as applying f directly to the original zipper.

    #[test]
    fn comonad_right_identity() {
        let z = Zipper::new(vec![1, 2, 3, 4, 5]).unwrap();
        let f = |w: &Zipper<i32>| {
            let left = w.peek_left(1).copied().unwrap_or(0);
            let right = w.peek_right(1).copied().unwrap_or(0);
            left + w.extract() + right
        };
        let extended = z.extend(&f);
        assert_eq!(*extended.extract(), f(&z));
    }

    #[test]
    fn comonad_right_identity_at_various_positions() {
        let z = Zipper::new(vec![1, 2, 3, 4]).unwrap();
        let f = |w: &Zipper<i32>| w.extract() * 2;

        // Check at every position
        let mut current = Some(z);
        while let Some(z) = current {
            let extended = z.extend(&f);
            assert_eq!(*extended.extract(), f(&z));
            current = z.move_right();
        }
    }

    // -- extend with context-dependent functions ------------------------------

    #[test]
    fn extend_doubles_each_element() {
        let z = Zipper::new(vec![1, 2, 3]).unwrap();
        let doubled = z.extend(|w| w.extract() * 2);
        assert_eq!(doubled.to_vec(), vec![2, 4, 6]);
    }

    #[test]
    fn extend_sum_with_neighbors() {
        // Each position sums itself with its left and right neighbors (0 if absent)
        let z = Zipper::new(vec![1, 2, 3, 4]).unwrap();
        let result = z.extend(|w| {
            let l = w.peek_left(1).copied().unwrap_or(0);
            let r = w.peek_right(1).copied().unwrap_or(0);
            l + w.extract() + r
        });
        // pos 0: 0 + 1 + 2 = 3
        // pos 1: 1 + 2 + 3 = 6
        // pos 2: 2 + 3 + 4 = 9
        // pos 3: 3 + 4 + 0 = 7
        assert_eq!(result.to_vec(), vec![3, 6, 9, 7]);
    }

    #[test]
    fn extend_preserves_position() {
        let z = Zipper::new(vec![1, 2, 3]).unwrap().move_right().unwrap(); // focus = 2
        let result = z.extend(|w| w.extract() * 10);
        assert_eq!(result.position(), 1); // position preserved
        assert_eq!(*result.extract(), 20);
        assert_eq!(result.to_vec(), vec![10, 20, 30]);
    }

    // -- Finnish vowel harmony example ----------------------------------------

    /// Determine whether a Finnish character is a back vowel.
    fn is_back_vowel(c: char) -> bool {
        matches!(c, 'a' | 'o' | 'u')
    }

    /// Determine whether a Finnish character is a front vowel.
    fn is_front_vowel(c: char) -> bool {
        matches!(c, '\u{00E4}' | '\u{00F6}' | 'y')
    }

    /// Determine vowel harmony class of a character sequence by scanning
    /// leftward from the focus position.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Harmony {
        Back,
        Front,
        Neutral,
    }

    /// A simple vowel harmony rule expressed as a coKleisli arrow.
    ///
    /// Given a zipper of characters, determine the harmony class
    /// visible at the current focus by scanning left context.
    fn detect_harmony(z: &Zipper<char>) -> Harmony {
        // Scan leftward to find the nearest back or front vowel.
        for i in 0..z.position() {
            if let Some(&c) = z.peek_left(i + 1) {
                if is_back_vowel(c) {
                    return Harmony::Back;
                }
                if is_front_vowel(c) {
                    return Harmony::Front;
                }
            }
        }
        // Also check the focus itself.
        let c = *z.extract();
        if is_back_vowel(c) {
            return Harmony::Back;
        }
        if is_front_vowel(c) {
            return Harmony::Front;
        }
        Harmony::Neutral
    }

    /// Apply vowel harmony: replace archiphonemic 'A' with 'a' (back) or
    /// '\u{00E4}' (front), and 'O' with 'o' or '\u{00F6}'.
    fn apply_harmony(z: &Zipper<char>) -> char {
        let c = *z.extract();
        match c {
            'A' => {
                // Scan left for back/front context
                let harmony = detect_harmony(z);
                match harmony {
                    Harmony::Back | Harmony::Neutral => 'a',
                    Harmony::Front => '\u{00E4}',
                }
            }
            'O' => {
                let harmony = detect_harmony(z);
                match harmony {
                    Harmony::Back | Harmony::Neutral => 'o',
                    Harmony::Front => '\u{00F6}',
                }
            }
            _ => c,
        }
    }

    #[test]
    fn vowel_harmony_back() {
        // "talo" + archiphonemic suffix "-ssA" -> "talossa"
        let input: Vec<char> = "talossA".chars().collect();
        let z = Zipper::new(input).unwrap();
        let result = z.extend(apply_harmony);
        let word: String = result.to_vec().into_iter().collect();
        assert_eq!(word, "talossa");
    }

    #[test]
    fn vowel_harmony_front() {
        // "m\u{00E4}ki" (maki with front vowels) + "-ssA" -> "m\u{00E4}kiss\u{00E4}"
        let input: Vec<char> = "m\u{00E4}kissA".chars().collect();
        let z = Zipper::new(input).unwrap();
        let result = z.extend(apply_harmony);
        let word: String = result.to_vec().into_iter().collect();
        assert_eq!(word, "m\u{00E4}kiss\u{00E4}");
    }

    #[test]
    fn vowel_harmony_multiple_archiphonemes() {
        // "p\u{00F6}yt\u{00E4}" + "-llA" + "-An" -> "p\u{00F6}yd\u{00E4}ll\u{00E4}\u{00E4}n"
        // Simplified: just test "pOytAllA"
        let input: Vec<char> = "p\u{00F6}ytAllA".chars().collect();
        let z = Zipper::new(input).unwrap();
        let result = z.extend(apply_harmony);
        let word: String = result.to_vec().into_iter().collect();
        // \u{00F6} is front vowel, so A -> \u{00E4}
        assert_eq!(word, "p\u{00F6}yt\u{00E4}ll\u{00E4}");
    }

    // -- Comonad law: associativity (L3') --------------------------------------
    // extend(f) ∘ extend(g) == extend(f ∘ extend(g))
    //
    // That is: composing two extended functions sequentially must be the
    // same as extending the composition of f with extend(g).

    #[test]
    fn comonad_associativity_l3() {
        // f = sum of focus with immediate neighbors
        let f = |w: &Zipper<i32>| -> i32 {
            let l = w.peek_left(1).copied().unwrap_or(0);
            let r = w.peek_right(1).copied().unwrap_or(0);
            l + w.extract() + r
        };
        // g = double the focus
        let g = |w: &Zipper<i32>| -> i32 { w.extract() * 2 };

        // LHS: extend(g) then extend(f)  —  i.e., extend(f)(extend(g)(w))
        // RHS: extend(λw'. f(extend(g, w')))  —  a single extend of the composition

        let inputs: Vec<Vec<i32>> = vec![
            vec![1, 2, 3, 4, 5],
            vec![10, 20, 30],
            vec![42],
            vec![1, 2],
            vec![-3, 0, 7, -1, 4, 2],
        ];

        for input in inputs {
            let z = Zipper::new(input.clone()).unwrap();

            // LHS: two sequential extends
            let lhs = z.extend(&g).extend(&f);

            // RHS: single extend with composed function
            let rhs = z.extend(|w: &Zipper<i32>| {
                // Build the g-extended zipper from w, then apply f to it
                let extended_g = w.extend(&g);
                f(&extended_g)
            });

            assert_eq!(
                lhs.to_vec(),
                rhs.to_vec(),
                "L3' associativity failed for input {:?}",
                input
            );
        }
    }

    #[test]
    fn comonad_associativity_l3_at_various_positions() {
        // Verify L3' holds regardless of the initial focus position.
        let f = |w: &Zipper<i32>| -> i32 {
            let l = w.peek_left(1).copied().unwrap_or(0);
            let r = w.peek_right(1).copied().unwrap_or(0);
            l + w.extract() + r
        };
        let g = |w: &Zipper<i32>| -> i32 { w.extract() * 2 };

        let z_start = Zipper::new(vec![3, 1, 4, 1, 5]).unwrap();

        let mut current = Some(z_start);
        while let Some(z) = current {
            let lhs = z.extend(&g).extend(&f);
            let rhs = z.extend(|w: &Zipper<i32>| {
                let extended_g = w.extend(&g);
                f(&extended_g)
            });
            assert_eq!(
                lhs.to_vec(),
                rhs.to_vec(),
                "L3' failed at position {}",
                z.position()
            );
            current = z.move_right();
        }
    }

    #[test]
    fn comonad_associativity_l3_with_different_arrow_types() {
        // Use asymmetric arrows to stress-test: g looks right, f looks left.
        let f = |w: &Zipper<i32>| -> i32 {
            // Sum of focus and two left neighbors
            let l1 = w.peek_left(1).copied().unwrap_or(0);
            let l2 = w.peek_left(2).copied().unwrap_or(0);
            w.extract() + l1 + l2
        };
        let g = |w: &Zipper<i32>| -> i32 {
            // Focus minus right neighbor
            let r = w.peek_right(1).copied().unwrap_or(0);
            w.extract() - r
        };

        let z = Zipper::new(vec![5, 3, 8, 2, 7, 1]).unwrap();

        let lhs = z.extend(&g).extend(&f);
        let rhs = z.extend(|w: &Zipper<i32>| {
            let extended_g = w.extend(&g);
            f(&extended_g)
        });

        assert_eq!(
            lhs.to_vec(),
            rhs.to_vec(),
            "L3' failed with asymmetric arrows"
        );
    }

    // -- extend composition (associativity-like property) ---------------------

    #[test]
    fn extend_composition() {
        // Verify: z.extend(f).extend(g).to_vec()
        // produces a valid result with g operating on the f-transformed zipper.
        let z = Zipper::new(vec![1, 2, 3, 4, 5]).unwrap();

        let f = |w: &Zipper<i32>| -> i32 { w.extract() + 1 };
        let g = |w: &Zipper<i32>| -> i32 {
            let l = w.peek_left(1).copied().unwrap_or(0);
            let r = w.peek_right(1).copied().unwrap_or(0);
            l + w.extract() + r
        };

        // First add 1 to each, then sum with neighbors
        let step1 = z.extend(f);
        assert_eq!(step1.to_vec(), vec![2, 3, 4, 5, 6]);

        let step2 = step1.extend(g);
        // pos 0: 0 + 2 + 3 = 5
        // pos 1: 2 + 3 + 4 = 9
        // pos 2: 3 + 4 + 5 = 12
        // pos 3: 4 + 5 + 6 = 15
        // pos 4: 5 + 6 + 0 = 11
        assert_eq!(step2.to_vec(), vec![5, 9, 12, 15, 11]);
    }

    // -- Single-element edge cases --------------------------------------------

    #[test]
    fn single_element_extend() {
        let z = Zipper::new(vec![42]).unwrap();
        let result = z.extend(|w| w.extract() * 3);
        assert_eq!(result.to_vec(), vec![126]);
        assert_eq!(result.position(), 0);
    }

    #[test]
    fn single_element_navigation() {
        let z = Zipper::new(vec![42]).unwrap();
        assert!(z.move_left().is_none());
        assert!(z.move_right().is_none());
        assert_eq!(z.peek_left(1), None);
        assert_eq!(z.peek_right(1), None);
    }

    // -- is_empty always false ------------------------------------------------

    #[test]
    fn is_empty_always_false() {
        let z = Zipper::new(vec![1]).unwrap();
        assert!(!z.is_empty());
        let z2 = Zipper::new(vec![1, 2, 3]).unwrap();
        assert!(!z2.is_empty());
    }
}
