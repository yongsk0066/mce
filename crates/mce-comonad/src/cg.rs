//! CG-lite (Constraint Grammar lite) disambiguation as coKleisli morphisms.
//!
//! Constraint Grammar (CG) disambiguation removes unlikely morphological
//! readings at each sentence position based on context. Each CG rule can
//! be expressed as a coKleisli arrow `&Zipper<ReadingSet> -> ReadingSet`:
//! it inspects the focused reading set plus its left/right context (the
//! neighboring reading sets) and produces a filtered reading set.
//!
//! This module provides:
//!
//! - [`ReadingSet`]: A type alias for the set of candidate analyses at one position.
//! - [`CgRule`]: A trait for individual CG rules (coKleisli arrows).
//! - Concrete rules (19 types):
//!   - Context-based: [`RemoveIfPreceded`], [`RemoveIfFollowed`],
//!     [`RemoveIfNotPreceded`], [`RemoveIfNotFollowed`],
//!     [`SelectIfFollowed`], [`SelectIfPreceded`],
//!     [`SelectIfNotFollowed`].
//!   - Baseform-based: [`SelectByBaseform`], [`SelectByBaseformList`],
//!     [`SelectByCurrentBaseformList`], [`SelectIfFollowedByBaseformList`],
//!     [`RemoveByBaseformList`], [`RemoveIfFollowedByBaseformList`].
//!   - Attribute-based: [`RemoveByClass`], [`RemoveIfCase`],
//!     [`SelectIfAttr`], [`RemoveIfAttr`].
//!   - Multi-context: [`RemoveIfSandwiched`], [`SelectIfSandwiched`].
//!   - Positional: [`RemoveAtSentenceStart`].
//! - [`finnish_disambiguation_rules`]: Pre-built Finnish CG rule set (57 rules)
//!   targeting the top UPOS confusions (ADJ/NOUN, ADV/NOUN, NOUN/PROPN,
//!   NOUN/VERB, PRON/NOUN, ADP/ADV, VERB/AUX, and more).
//! - [`apply_cg_rules`]: Applies a sequence of CG rules over a sentence,
//!   using [`Zipper::extend`] for each rule pass.
//!
//! # Safety invariant
//!
//! CG convention: a rule must never remove the last reading at any position.
//! All rule implementations enforce this by returning the original reading
//! set unchanged when filtering would leave it empty.
//!
//! # Example
//!
//! ```
//! use mce_core::analysis::{Analysis, ATTR_CLASS};
//! use mce_comonad::cg::{ReadingSet, CgRule, RemoveIfPreceded, apply_cg_rules};
//!
//! // Position 0: determiner
//! let mut det = Analysis::new();
//! det.set(ATTR_CLASS, "lukusana");
//!
//! // Position 1: ambiguous — noun or verb
//! let mut noun = Analysis::new();
//! noun.set(ATTR_CLASS, "nimisana");
//! let mut verb = Analysis::new();
//! verb.set(ATTR_CLASS, "teonsana");
//!
//! let sentence: Vec<ReadingSet> = vec![
//!     vec![det],
//!     vec![noun, verb],
//! ];
//!
//! // REMOVE VERB IF (-1 has NUM)
//! let rule = RemoveIfPreceded {
//!     remove_class: "teonsana".into(),
//!     preceded_by_class: "lukusana".into(),
//! };
//!
//! let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
//! assert_eq!(result[1].len(), 1);
//! assert_eq!(result[1][0].get(ATTR_CLASS), Some("nimisana"));
//! ```

use mce_core::analysis::{
    Analysis, ATTR_BASEFORM, ATTR_CLASS, ATTR_COMPARISON, ATTR_MOOD, ATTR_NEGATIVE,
    ATTR_PARTICIPLE, ATTR_POSSIBLE_GEOGRAPHICAL_NAME, ATTR_REQUIRE_FOLLOWING_VERB, ATTR_SIJAMUOTO,
};

use crate::zipper::Zipper;

/// A set of morphological readings (candidate analyses) at one sentence position.
pub type ReadingSet = Vec<Analysis>;

/// A CG-lite rule expressed as a coKleisli arrow.
///
/// Given a `Zipper<ReadingSet>` focused on the current position, the rule
/// inspects local context and returns a (possibly filtered) reading set.
///
/// Implementations must uphold the CG safety invariant: never return an
/// empty reading set. If filtering would remove all readings, the original
/// set must be returned unchanged.
pub trait CgRule {
    /// Apply the rule to the focused position, possibly removing readings.
    ///
    /// The zipper provides access to left/right context via `peek_left`
    /// and `peek_right`.
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet;
}

// ---------------------------------------------------------------------------
// Helper: check whether a reading set contains at least one reading
// with a given CLASS value.
// ---------------------------------------------------------------------------

/// Returns `true` if any analysis in `readings` has `ATTR_CLASS == class`.
fn has_class(readings: &[Analysis], class: &str) -> bool {
    readings.iter().any(|a| a.get(ATTR_CLASS) == Some(class))
}

/// Filter a reading set, enforcing the CG safety invariant.
///
/// If the predicate removes all readings, the original set is returned.
fn safe_filter<F>(readings: &ReadingSet, keep: F) -> ReadingSet
where
    F: Fn(&Analysis) -> bool,
{
    let filtered: ReadingSet = readings.iter().filter(|a| keep(a)).cloned().collect();
    if filtered.is_empty() {
        readings.clone()
    } else {
        filtered
    }
}

// ---------------------------------------------------------------------------
// Concrete rules
// ---------------------------------------------------------------------------

/// REMOVE reading with CLASS=X IF position -1 has CLASS=Y.
///
/// CG notation: `REMOVE (X) IF (-1 (Y))`
///
/// Example: REMOVE VERB IF (-1 DET) — if preceded by a determiner, remove
/// verb readings from the current position.
#[derive(Debug, Clone)]
pub struct RemoveIfPreceded {
    /// The CLASS value to remove from the current position.
    pub remove_class: String,
    /// The CLASS value that must be present at position -1.
    pub preceded_by_class: String,
}

impl CgRule for RemoveIfPreceded {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        // Check if position -1 has the required class.
        let preceded = z
            .peek_left(1)
            .is_some_and(|left| has_class(left, &self.preceded_by_class));

        if !preceded {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) != Some(&self.remove_class))
    }
}

/// SELECT reading with CLASS=X IF position +1 has CLASS=Y.
///
/// CG notation: `SELECT (X) IF (1 (Y))`
///
/// This is the dual of REMOVE: instead of removing readings, it selects
/// (keeps) only readings matching the target class — but only when the
/// context condition is met.
///
/// Example: SELECT VERB IF (+1 NOUN) — if followed by a noun, keep only
/// verb readings at the current position.
#[derive(Debug, Clone)]
pub struct SelectIfFollowed {
    /// The CLASS value to select (keep) at the current position.
    pub select_class: String,
    /// The CLASS value that must be present at position +1.
    pub followed_by_class: String,
}

impl CgRule for SelectIfFollowed {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        // Check if position +1 has the required class.
        let followed = z
            .peek_right(1)
            .is_some_and(|right| has_class(right, &self.followed_by_class));

        if !followed {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) == Some(&self.select_class))
    }
}

/// REMOVE reading with CLASS=X IF position -1 does NOT have CLASS=Y.
///
/// CG notation: `REMOVE (X) IF (NOT -1 (Y))`
///
/// Example: REMOVE ADJECTIVE IF (NOT -1 DET) — if NOT preceded by a
/// determiner, remove adjective readings.
#[derive(Debug, Clone)]
pub struct RemoveIfNotPreceded {
    /// The CLASS value to remove from the current position.
    pub remove_class: String,
    /// The CLASS value that must be ABSENT at position -1 for removal to fire.
    pub not_preceded_by_class: String,
}

impl CgRule for RemoveIfNotPreceded {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        // Check that position -1 does NOT have the required class.
        // If there is no left neighbor, the class is absent by definition.
        let has_required_class = z
            .peek_left(1)
            .is_some_and(|left| has_class(left, &self.not_preceded_by_class));

        if has_required_class {
            // The required class IS present, so the NOT condition fails.
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) != Some(&self.remove_class))
    }
}

// ---------------------------------------------------------------------------
// Extended rule types for Finnish disambiguation
// ---------------------------------------------------------------------------

/// REMOVE readings of a specific CLASS when the context contains a specific
/// CLASS at position -1, but only when the current position also has at
/// least one reading of an alternative CLASS. This prevents removing when
/// there is no safe fallback.
///
/// CG notation: `REMOVE (X) IF (-1 (Y)) (0 HAS Z)`
///
/// Example: REMOVE nimisana IF (-1 lukusana) IF (0 HAS lukusana)
///   — if preceded by a numeral and the current position also has a numeral
///   reading, remove noun readings (prefer "kolme" as numeral, not noun).
#[derive(Debug, Clone)]
pub struct RemoveByClass {
    /// The CLASS value to remove from the current position.
    pub remove_class: String,
    /// The CLASS value that must be present at position -1.
    pub context_class: String,
    /// The CLASS value that must also be present in the current readings
    /// (as a safe alternative).
    pub require_alternative: String,
}

impl CgRule for RemoveByClass {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        // Check context at position -1.
        let context_ok = z
            .peek_left(1)
            .is_some_and(|left| has_class(left, &self.context_class));

        if !context_ok {
            return current.clone();
        }

        // Check that the current position has the required alternative reading.
        if !has_class(current, &self.require_alternative) {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) != Some(&self.remove_class))
    }
}

/// SELECT readings with CLASS=X IF the current position is preceded by a
/// word with a specific BASEFORM.
///
/// CG notation: `SELECT (X) IF (-1 BASEFORM=Y)`
///
/// Example: SELECT teonsana IF (-1 baseform "ei")
///   — if preceded by the negation verb "ei", select verb readings.
#[derive(Debug, Clone)]
pub struct SelectByBaseform {
    /// The CLASS value to select at the current position.
    pub select_class: String,
    /// The BASEFORM value that must appear in at least one reading at position -1.
    pub preceded_by_baseform: String,
}

impl CgRule for SelectByBaseform {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        let preceded = z.peek_left(1).is_some_and(|left| {
            left.iter()
                .any(|a| a.get(ATTR_BASEFORM) == Some(&self.preceded_by_baseform))
        });

        if !preceded {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) == Some(&self.select_class))
    }
}

/// SELECT reading with CLASS=X IF position +1 does NOT have CLASS=Y.
///
/// CG notation: `SELECT (X) IF (NOT 1 (Y))`
///
/// Example: SELECT nimisana IF (NOT +1 teonsana)
///   — if NOT followed by a verb, prefer noun readings (adjective-as-noun).
#[derive(Debug, Clone)]
pub struct SelectIfNotFollowed {
    /// The CLASS value to select at the current position.
    pub select_class: String,
    /// The CLASS value that must be ABSENT at position +1 for selection to fire.
    pub not_followed_by_class: String,
}

impl CgRule for SelectIfNotFollowed {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        // Check that position +1 does NOT have the specified class.
        let has_class_right = z
            .peek_right(1)
            .is_some_and(|right| has_class(right, &self.not_followed_by_class));

        if has_class_right {
            // The class IS present at +1, so the NOT condition fails.
            return current.clone();
        }

        // Only fire if the current position actually has the select_class.
        if !has_class(current, &self.select_class) {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) == Some(&self.select_class))
    }
}

/// REMOVE reading with CLASS=X IF position +1 has CLASS=Y.
///
/// CG notation: `REMOVE (X) IF (1 (Y))`
///
/// Example: REMOVE nimisana IF (+1 teonsana)
///   — if followed by a verb, remove noun readings (prefer adjective/adverb).
#[derive(Debug, Clone)]
pub struct RemoveIfFollowed {
    /// The CLASS value to remove from the current position.
    pub remove_class: String,
    /// The CLASS value that must be present at position +1.
    pub followed_by_class: String,
}

impl CgRule for RemoveIfFollowed {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        let followed = z
            .peek_right(1)
            .is_some_and(|right| has_class(right, &self.followed_by_class));

        if !followed {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) != Some(&self.remove_class))
    }
}

/// SELECT reading with CLASS=X IF position -1 has CLASS=Y.
///
/// CG notation: `SELECT (X) IF (-1 (Y))`
///
/// Example: SELECT teonsana IF (-1 kieltosana)
///   -- if preceded by a negation verb, select verb readings.
#[derive(Debug, Clone)]
pub struct SelectIfPreceded {
    /// The CLASS value to select (keep) at the current position.
    pub select_class: String,
    /// The CLASS value that must be present at position -1.
    pub preceded_by_class: String,
}

impl CgRule for SelectIfPreceded {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        let preceded = z
            .peek_left(1)
            .is_some_and(|left| has_class(left, &self.preceded_by_class));

        if !preceded {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) == Some(&self.select_class))
    }
}

/// SELECT reading with CLASS=X IF position -1 has any BASEFORM in a list.
///
/// CG notation: `SELECT (X) IF (-1 BASEFORM IN list)`
///
/// This is a generalized version of [`SelectByBaseform`] that matches against
/// multiple baseforms. Useful for patterns like "after any personal pronoun".
#[derive(Debug, Clone)]
pub struct SelectByBaseformList {
    /// The CLASS value to select at the current position.
    pub select_class: String,
    /// Any of these BASEFORM values at position -1 triggers the rule.
    pub preceded_by_baseforms: Vec<String>,
}

impl CgRule for SelectByBaseformList {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        let preceded = z.peek_left(1).is_some_and(|left| {
            left.iter().any(|a| {
                if let Some(bf) = a.get(ATTR_BASEFORM) {
                    self.preceded_by_baseforms.iter().any(|b| b == bf)
                } else {
                    false
                }
            })
        });

        if !preceded {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) == Some(&self.select_class))
    }
}

/// REMOVE reading with CLASS=X IF the current position has a reading
/// with a specific SIJAMUOTO (case) value.
///
/// CG notation: `REMOVE (X) IF (0 HAS SIJAMUOTO=Y)`
///
/// Example: REMOVE teonsana IF (0 HAS SIJAMUOTO=sisaolento)
///   -- if the current position has an inessive case reading, remove verb.
///   Words ending in -ssa/-ssa with inessive case are nouns, not verbs.
#[derive(Debug, Clone)]
pub struct RemoveIfCase {
    /// The CLASS value to remove.
    pub remove_class: String,
    /// The SIJAMUOTO value that must appear in at least one reading.
    pub has_case: String,
}

impl CgRule for RemoveIfCase {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        // Check if any reading at this position has the specified case.
        let has_case_reading = current
            .iter()
            .any(|a| a.get(ATTR_SIJAMUOTO) == Some(self.has_case.as_str()));

        if !has_case_reading {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) != Some(&self.remove_class))
    }
}

/// SELECT reading with CLASS=X IF position +1 has any BASEFORM in a list.
///
/// CG notation: `SELECT (X) IF (1 BASEFORM IN list)`
#[derive(Debug, Clone)]
pub struct SelectIfFollowedByBaseformList {
    /// The CLASS value to select at the current position.
    pub select_class: String,
    /// Any of these BASEFORM values at position +1 triggers the rule.
    pub followed_by_baseforms: Vec<String>,
}

impl CgRule for SelectIfFollowedByBaseformList {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        let followed = z.peek_right(1).is_some_and(|right| {
            right.iter().any(|a| {
                if let Some(bf) = a.get(ATTR_BASEFORM) {
                    self.followed_by_baseforms.iter().any(|b| b == bf)
                } else {
                    false
                }
            })
        });

        if !followed {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) == Some(&self.select_class))
    }
}

/// REMOVE reading with CLASS=X IF position -1 has any BASEFORM in a list.
///
/// CG notation: `REMOVE (X) IF (-1 BASEFORM IN list)`
#[derive(Debug, Clone)]
pub struct RemoveByBaseformList {
    /// The CLASS value to remove at the current position.
    pub remove_class: String,
    /// Any of these BASEFORM values at position -1 triggers the rule.
    pub preceded_by_baseforms: Vec<String>,
}

impl CgRule for RemoveByBaseformList {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        let preceded = z.peek_left(1).is_some_and(|left| {
            left.iter().any(|a| {
                if let Some(bf) = a.get(ATTR_BASEFORM) {
                    self.preceded_by_baseforms.iter().any(|b| b == bf)
                } else {
                    false
                }
            })
        });

        if !preceded {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) != Some(&self.remove_class))
    }
}

// ---------------------------------------------------------------------------
// Extended rule types for Finnish disambiguation (Phase 2)
// ---------------------------------------------------------------------------

/// Returns `true` if any analysis in `readings` has `ATTR_BASEFORM` matching any in the list.
#[allow(dead_code)]
fn has_baseform_in(readings: &[Analysis], baseforms: &[String]) -> bool {
    readings.iter().any(|a| {
        if let Some(bf) = a.get(ATTR_BASEFORM) {
            baseforms.iter().any(|b| b == bf)
        } else {
            false
        }
    })
}

/// Returns `true` if any analysis has the given attribute with the given value.
#[allow(dead_code)]
fn has_attr(readings: &[Analysis], attr: &str, value: &str) -> bool {
    readings.iter().any(|a| a.get(attr) == Some(value))
}

/// REMOVE reading with CLASS=X IF position +1 does NOT have CLASS=Y.
///
/// CG notation: `REMOVE (X) IF (NOT 1 (Y))`
///
/// Example: REMOVE suhdesana IF (NOT +1 nimisana)
///   -- if NOT followed by a noun, the adposition reading is unlikely.
#[derive(Debug, Clone)]
pub struct RemoveIfNotFollowed {
    /// The CLASS value to remove from the current position.
    pub remove_class: String,
    /// The CLASS value that must be ABSENT at position +1 for removal to fire.
    pub not_followed_by_class: String,
}

impl CgRule for RemoveIfNotFollowed {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        // Check that position +1 does NOT have the specified class.
        let has_class_right = z
            .peek_right(1)
            .is_some_and(|right| has_class(right, &self.not_followed_by_class));

        if has_class_right {
            // The class IS present at +1, so the NOT condition fails.
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) != Some(&self.remove_class))
    }
}

/// SELECT reading with CLASS=X IF the current position has a reading with
/// a specific attribute value.
///
/// CG notation: `SELECT (X) IF (0 HAS ATTR=V)`
///
/// Example: SELECT laatusana IF (0 HAS COMPARISON=comparative)
///   -- if the word has a comparative form, prefer adjective reading.
#[derive(Debug, Clone)]
pub struct SelectIfAttr {
    /// The CLASS value to select.
    pub select_class: String,
    /// The attribute name to check.
    pub attr_name: String,
    /// The attribute value that must appear in at least one reading.
    pub attr_value: String,
}

impl CgRule for SelectIfAttr {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        // Check if any reading has the required attribute.
        let has_attr_reading = current
            .iter()
            .any(|a| a.get(&self.attr_name) == Some(self.attr_value.as_str()));

        if !has_attr_reading {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) == Some(&self.select_class))
    }
}

/// REMOVE reading with CLASS=X IF the current position has a reading with
/// a specific attribute value.
///
/// CG notation: `REMOVE (X) IF (0 HAS ATTR=V)`
///
/// Example: REMOVE nimisana IF (0 HAS POSSIBLE_GEOGRAPHICAL_NAME=true)
///   -- if geographic name flag is set, remove plain noun readings.
#[derive(Debug, Clone)]
pub struct RemoveIfAttr {
    /// The CLASS value to remove.
    pub remove_class: String,
    /// The attribute name to check.
    pub attr_name: String,
    /// The attribute value that must appear in at least one reading.
    pub attr_value: String,
}

impl CgRule for RemoveIfAttr {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        let has_attr_reading = current
            .iter()
            .any(|a| a.get(&self.attr_name) == Some(self.attr_value.as_str()));

        if !has_attr_reading {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) != Some(&self.remove_class))
    }
}

/// SELECT reading with CLASS=X IF the current position's BASEFORM is in a list.
///
/// CG notation: `SELECT (X) IF (0 BASEFORM IN list)`
///
/// Example: SELECT teonsana IF (0 BASEFORM IN {"olla", "voida"})
///   -- if the baseform is an auxiliary verb, prefer verb reading.
#[derive(Debug, Clone)]
pub struct SelectByCurrentBaseformList {
    /// The CLASS value to select.
    pub select_class: String,
    /// The BASEFORM values that trigger selection.
    pub baseforms: Vec<String>,
}

impl CgRule for SelectByCurrentBaseformList {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        // Check if any reading at this position has a matching baseform.
        let has_matching_baseform = current.iter().any(|a| {
            if let Some(bf) = a.get(ATTR_BASEFORM) {
                self.baseforms.iter().any(|b| b == bf)
            } else {
                false
            }
        });

        if !has_matching_baseform {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) == Some(&self.select_class))
    }
}

/// REMOVE reading with CLASS=X IF position -1 has CLASS=Y AND position +1
/// has CLASS=Z. A "sandwich" rule.
///
/// CG notation: `REMOVE (X) IF (-1 (Y)) (1 (Z))`
///
/// Example: REMOVE seikkasana IF (-1 nimisana) (1 nimisana)
///   -- noun-adverb-noun pattern unlikely; remove adverb.
#[derive(Debug, Clone)]
pub struct RemoveIfSandwiched {
    /// The CLASS value to remove from the current position.
    pub remove_class: String,
    /// The CLASS value that must be present at position -1.
    pub preceded_by_class: String,
    /// The CLASS value that must be present at position +1.
    pub followed_by_class: String,
}

impl CgRule for RemoveIfSandwiched {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        let preceded = z
            .peek_left(1)
            .is_some_and(|left| has_class(left, &self.preceded_by_class));

        let followed = z
            .peek_right(1)
            .is_some_and(|right| has_class(right, &self.followed_by_class));

        if !preceded || !followed {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) != Some(&self.remove_class))
    }
}

/// SELECT reading with CLASS=X IF position -1 has CLASS=Y AND position +1
/// has CLASS=Z. A "sandwich" select rule.
///
/// CG notation: `SELECT (X) IF (-1 (Y)) (1 (Z))`
///
/// Example: SELECT laatusana IF (-1 nimisana) (1 nimisana)
///   -- between two nouns, prefer adjective reading.
#[derive(Debug, Clone)]
pub struct SelectIfSandwiched {
    /// The CLASS value to select at the current position.
    pub select_class: String,
    /// The CLASS value that must be present at position -1.
    pub preceded_by_class: String,
    /// The CLASS value that must be present at position +1.
    pub followed_by_class: String,
}

impl CgRule for SelectIfSandwiched {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        let preceded = z
            .peek_left(1)
            .is_some_and(|left| has_class(left, &self.preceded_by_class));

        let followed = z
            .peek_right(1)
            .is_some_and(|right| has_class(right, &self.followed_by_class));

        if !preceded || !followed {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) == Some(&self.select_class))
    }
}

/// REMOVE reading with CLASS=X IF the current position is at sentence start
/// (position 0, no left neighbor).
///
/// CG notation: `REMOVE (X) IF (-1 BOS)`
///
/// Example: REMOVE suhdesana IF (-1 BOS)
///   -- sentence-initial adposition is unlikely.
#[derive(Debug, Clone)]
pub struct RemoveAtSentenceStart {
    /// The CLASS value to remove at sentence start.
    pub remove_class: String,
}

impl CgRule for RemoveAtSentenceStart {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        // Check if we're at sentence start (no left neighbor).
        if z.peek_left(1).is_some() {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) != Some(&self.remove_class))
    }
}

/// SELECT reading with CLASS=X IF position +1 has any BASEFORM in a list
/// AND the current position also has a reading with the given class.
///
/// CG notation: `SELECT (X) IF (1 BASEFORM IN list)`
///
/// This is similar to SelectIfFollowedByBaseformList but provided for
/// consistency.
///
/// Example: SELECT teonsana IF (+1 BASEFORM IN {"olla"})
///   -- before auxiliary "olla", prefer verb reading (participle + aux).
#[derive(Debug, Clone)]
pub struct RemoveIfFollowedByBaseformList {
    /// The CLASS value to remove from the current position.
    pub remove_class: String,
    /// Any of these BASEFORM values at position +1 triggers the rule.
    pub followed_by_baseforms: Vec<String>,
}

impl CgRule for RemoveIfFollowedByBaseformList {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        let followed = z.peek_right(1).is_some_and(|right| {
            right.iter().any(|a| {
                if let Some(bf) = a.get(ATTR_BASEFORM) {
                    self.followed_by_baseforms.iter().any(|b| b == bf)
                } else {
                    false
                }
            })
        });

        if !followed {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) != Some(&self.remove_class))
    }
}

// ---------------------------------------------------------------------------
// Extended rule types for Finnish disambiguation (Phase 3)
// ---------------------------------------------------------------------------

/// SELECT reading with CLASS=X IF the current position is at sentence start
/// (position 0, no left neighbor).
///
/// CG notation: `SELECT (X) IF (-1 BOS)`
///
/// Example: SELECT nimisana IF (-1 BOS)
///   -- sentence-initial common noun is more likely than proper noun.
#[derive(Debug, Clone)]
pub struct SelectAtSentenceStart {
    /// The CLASS value to select at sentence start.
    pub select_class: String,
}

impl CgRule for SelectAtSentenceStart {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        // Check if we're at sentence start (no left neighbor).
        if z.peek_left(1).is_some() {
            return current.clone();
        }

        // Only fire if the current position actually has the select_class.
        if !has_class(current, &self.select_class) {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) == Some(&self.select_class))
    }
}

/// REMOVE reading with CLASS=X IF the current position has a reading
/// with BASEFORM in a given list.
///
/// CG notation: `REMOVE (X) IF (0 BASEFORM IN list)`
///
/// Example: REMOVE teonsana IF (0 BASEFORM IN {"olla", "voida"})
///   -- for certain baseforms, remove a class (e.g., remove VERB when
///   the word should be AUX).
#[derive(Debug, Clone)]
pub struct RemoveByCurrentBaseformList {
    /// The CLASS value to remove.
    pub remove_class: String,
    /// The BASEFORM values that trigger removal.
    pub baseforms: Vec<String>,
}

impl CgRule for RemoveByCurrentBaseformList {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        // Check if any reading at this position has a matching baseform.
        let has_matching_baseform = current.iter().any(|a| {
            if let Some(bf) = a.get(ATTR_BASEFORM) {
                self.baseforms.iter().any(|b| b == bf)
            } else {
                false
            }
        });

        if !has_matching_baseform {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) != Some(&self.remove_class))
    }
}

/// SELECT reading with CLASS=X IF position -1 has BASEFORM in a list
/// AND position +1 has CLASS=Z.
///
/// CG notation: `SELECT (X) IF (-1 BASEFORM IN list) (1 (Z))`
///
/// Example: SELECT teonsana IF (-1 BASEFORM IN {"ei"}) (1 nimisana)
///   -- after negation and before noun, select verb (connegative + object).
#[derive(Debug, Clone)]
pub struct SelectIfPrecededByBaseformAndFollowed {
    /// The CLASS value to select at the current position.
    pub select_class: String,
    /// Any of these BASEFORM values at position -1 triggers.
    pub preceded_by_baseforms: Vec<String>,
    /// The CLASS value that must be present at position +1.
    pub followed_by_class: String,
}

impl CgRule for SelectIfPrecededByBaseformAndFollowed {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        let preceded = z.peek_left(1).is_some_and(|left| {
            left.iter().any(|a| {
                if let Some(bf) = a.get(ATTR_BASEFORM) {
                    self.preceded_by_baseforms.iter().any(|b| b == bf)
                } else {
                    false
                }
            })
        });

        let followed = z
            .peek_right(1)
            .is_some_and(|right| has_class(right, &self.followed_by_class));

        if !preceded || !followed {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) == Some(&self.select_class))
    }
}

/// REMOVE reading with CLASS=X IF position -1 has CLASS=Y AND the current
/// position has a reading with a specific attribute value.
///
/// CG notation: `REMOVE (X) IF (-1 (Y)) (0 HAS ATTR=V)`
///
/// Example: REMOVE teonsana IF (-1 seikkasana) (0 HAS SIJAMUOTO=omanto)
///   -- after adverb, if the word has genitive case, prefer noun over verb.
#[derive(Debug, Clone)]
pub struct RemoveIfPrecededAndAttr {
    /// The CLASS value to remove.
    pub remove_class: String,
    /// The CLASS value that must be present at position -1.
    pub preceded_by_class: String,
    /// The attribute name to check in the current position.
    pub attr_name: String,
    /// The attribute value that must be present.
    pub attr_value: String,
}

impl CgRule for RemoveIfPrecededAndAttr {
    fn apply(&self, z: &Zipper<ReadingSet>) -> ReadingSet {
        let current = z.extract();

        let preceded = z
            .peek_left(1)
            .is_some_and(|left| has_class(left, &self.preceded_by_class));

        if !preceded {
            return current.clone();
        }

        let has_attr_reading = current
            .iter()
            .any(|a| a.get(&self.attr_name) == Some(self.attr_value.as_str()));

        if !has_attr_reading {
            return current.clone();
        }

        safe_filter(current, |a| a.get(ATTR_CLASS) != Some(&self.remove_class))
    }
}

// ---------------------------------------------------------------------------
// Pre-built Finnish disambiguation rule set
// ---------------------------------------------------------------------------

/// Returns a vector of CG rules tuned for Finnish UPOS disambiguation.
///
/// These rules target the top confusion pairs observed in MCE evaluation
/// against the UD Finnish-TDT dev set:
///
/// 1. ADJ -> NOUN (290): laatusana/nimisana ambiguity
/// 2. ADV -> NOUN (278): seikkasana/nimisana ambiguity
/// 3. NOUN -> PROPN (218): nimisana/etunimi ambiguity
/// 4. NOUN -> VERB (207): nimisana/teonsana ambiguity
/// 5. PRON -> NOUN (180): asemosana/nimisana ambiguity
///
/// Additional patterns:
/// 6. ADP/ADV: suhdesana/seikkasana ambiguity
/// 7. VERB/AUX: teonsana/kieltosana auxiliary patterns
/// 8. NOUN/PROPN: geographical name and proper noun patterns
/// 9. Comparative/superlative ADJ disambiguation
/// 10. Sandwich patterns (X between Y and Z)
///
/// Rule ordering matters: rules are applied in sequence, each pass over
/// the full sentence. Earlier rules prune readings that later rules
/// can exploit.
///
/// The 53 rules are organized into 15 phases:
/// - Phases 1-3: High-confidence patterns (negation, pronouns, numerals)
/// - Phases 4-5: Subject-verb and adposition patterns
/// - Phases 6-7: Case-based and context patterns
/// - Phases 8-9: Adjective and adverb patterns
/// - Phases 10-11: Genitive/partitive and conjunction patterns
/// - Phases 12-15: Extended patterns (PROPN, ADP, sandwich, attribute-based)
pub fn finnish_disambiguation_rules() -> Vec<Box<dyn CgRule>> {
    vec![
        // =================================================================
        // PHASE 1: High-confidence negation & auxiliary patterns
        // =================================================================
        //
        // R1: After negation verb "ei", select verb readings.
        // Targets: NOUN->VERB confusion (207).
        // "ei voi" (cannot) -- "voi" is ambiguous nimisana(butter)/teonsana(can).
        // After "ei", the verb reading is strongly preferred.
        // -----------------------------------------------------------------
        Box::new(SelectByBaseform {
            select_class: "teonsana".into(),
            preceded_by_baseform: "ei".into(),
        }),
        // -----------------------------------------------------------------
        // R2: After personal pronoun (minä, sinä, hän, me, te, he), prefer verb.
        // Targets: NOUN->VERB confusion.
        // "minä tulen" (I come) -- after personal pronoun, verb is expected.
        // SELECT teonsana IF (-1 BASEFORM IN {minä, sinä, hän, me, te, he})
        // -----------------------------------------------------------------
        Box::new(SelectByBaseformList {
            select_class: "teonsana".into(),
            preceded_by_baseforms: vec![
                "minä".into(),
                "sinä".into(),
                "hän".into(),
                "me".into(),
                "te".into(),
                "he".into(),
            ],
        }),
        // -----------------------------------------------------------------
        // R3: After "olla" auxiliary forms, prefer verb (participle) readings.
        // Targets: ADJ->VERB confusion (participles).
        // "on tullut" (has come) -- after olla, the next word is often a
        // past participle tagged as VERB in UD, not ADJ.
        // DISABLED: Over-aggressive — after "olla", the complement can be:
        // - NOUN: "on opettaja" (is a teacher), "on asia" (is a matter)
        // - ADJ: "on suuri" (is big), "on kaunis" (is beautiful)
        // - ADV: "on hyvin" (is well)
        // SELECT teonsana removes all these valid readings.
        // SELECT teonsana IF (-1 BASEFORM "olla")
        // -----------------------------------------------------------------
        // Box::new(SelectByBaseform {
        //     select_class: "teonsana".into(),
        //     preceded_by_baseform: "olla".into(),
        // }),
        // -----------------------------------------------------------------
        // R4: After modal auxiliary (voida, saattaa, täytyä, pitää), prefer verb.
        // Targets: NOUN->VERB confusion.
        // "voi tehdä" (can do) -- after modal aux, the next word is a verb.
        // SELECT teonsana IF (-1 BASEFORM IN {voida, saattaa, täytyä, pitää, ...})
        // -----------------------------------------------------------------
        Box::new(SelectByBaseformList {
            select_class: "teonsana".into(),
            preceded_by_baseforms: vec![
                "voida".into(),
                "saattaa".into(),
                "täytyä".into(),
                "pitää".into(),
                "joutua".into(),
                "mahtaa".into(),
                "taitaa".into(),
                "aikoa".into(),
            ],
        }),
        // =================================================================
        // PHASE 2: Determiner/modifier + head noun patterns
        // =================================================================
        //
        // R5: After determiner-like pronoun (se, tämä, tuo), remove verb.
        // Targets: NOUN->VERB confusion (207).
        // "se koira" (that dog) -- after a demonstrative, verb reading is unlikely.
        // DISABLED: Over-aggressive — pronouns commonly precede verbs as
        // subjects in Finnish: "hän tulee" (s/he comes), "se on" (it is),
        // "tämä toimii" (this works). The rule was labeled as targeting
        // determiners but fires on ALL asemosana (all pronouns).
        // REMOVE teonsana IF (-1 asemosana)
        // -----------------------------------------------------------------
        // Box::new(RemoveIfPreceded {
        //     remove_class: "teonsana".into(),
        //     preceded_by_class: "asemosana".into(),
        // }),
        // -----------------------------------------------------------------
        // R6: After numeral, remove verb readings.
        // Targets: NOUN->VERB confusion (207).
        // "kolme koiraa" -- after a numeral, the next word is typically a noun.
        // REMOVE teonsana IF (-1 lukusana)
        // -----------------------------------------------------------------
        Box::new(RemoveIfPreceded {
            remove_class: "teonsana".into(),
            preceded_by_class: "lukusana".into(),
        }),
        // -----------------------------------------------------------------
        // R7: After adjective, remove verb readings.
        // Targets: NOUN->VERB confusion (207).
        // "iso koira" -- "koira" after "iso" (adj) is a noun, not verb.
        // REMOVE teonsana IF (-1 laatusana)
        // -----------------------------------------------------------------
        Box::new(RemoveIfPreceded {
            remove_class: "teonsana".into(),
            preceded_by_class: "laatusana".into(),
        }),
        // -----------------------------------------------------------------
        // R8: After adjective, remove adverb readings.
        // Targets: ADV->NOUN confusion (278).
        // After an adjective, the next word is more likely a noun than adverb.
        // DISABLED: Adverbs CAN follow adjectives in Finnish constructions.
        // E.g., "hyvä myös" or "parempi vielä". The next word after ADJ
        // is often a noun, but removing ADV is too aggressive.
        // REMOVE seikkasana IF (-1 laatusana)
        // -----------------------------------------------------------------
        // Box::new(RemoveIfPreceded {
        //     remove_class: "seikkasana".into(),
        //     preceded_by_class: "laatusana".into(),
        // }),
        // -----------------------------------------------------------------
        // R9: After numeral, remove adverb readings.
        // Targets: ADV->NOUN confusion.
        // "kolme kissaa" -- "kissaa" after numeral should be noun, not adverb.
        // DISABLED: Testing whether this hurts -- similar concern to R8.
        // REMOVE seikkasana IF (-1 lukusana)
        // -----------------------------------------------------------------
        // Box::new(RemoveIfPreceded {
        //     remove_class: "seikkasana".into(),
        //     preceded_by_class: "lukusana".into(),
        // }),
        // -----------------------------------------------------------------
        // R10: After numeral, remove pronoun readings.
        // Targets: PRON->NOUN confusion.
        // "viisi kissaa" -- after numeral, pronoun reading is unlikely.
        // DISABLED: Pronouns CAN follow numerals: "kolme toista" (thirteen),
        // "kaksi muuta" (two others). The word after a numeral can be a
        // pronoun-like quantifier.
        // REMOVE asemosana IF (-1 lukusana)
        // -----------------------------------------------------------------
        // Box::new(RemoveIfPreceded {
        //     remove_class: "asemosana".into(),
        //     preceded_by_class: "lukusana".into(),
        // }),
        // =================================================================
        // PHASE 3: Numeral disambiguation
        // =================================================================
        //
        // R11: Numeral + noun pattern: SELECT lukusana IF (+1 nimisana).
        // "kolme kissaa" -- prefer numeral before partitive noun.
        // -----------------------------------------------------------------
        Box::new(SelectIfFollowed {
            select_class: "lukusana".into(),
            followed_by_class: "nimisana".into(),
        }),
        // -----------------------------------------------------------------
        // R12: Numeral + proper noun pattern: SELECT lukusana IF (+1 etunimi/etc.).
        // "kolme Mattia" -- prefer numeral before proper noun too.
        // SELECT lukusana IF (+1 etunimi)
        // -----------------------------------------------------------------
        Box::new(SelectIfFollowed {
            select_class: "lukusana".into(),
            followed_by_class: "etunimi".into(),
        }),
        // =================================================================
        // PHASE 4: Subject-verb patterns (word before verb is likely noun)
        // =================================================================
        //
        // R13: Noun before verb pattern: SELECT nimisana IF (+1 teonsana).
        // Targets: ADJ->NOUN (290), PRON->NOUN (180), ADV->NOUN (278).
        // "koira juoksee" -- word before verb is typically the subject (noun).
        // DISABLED: Over-aggressive — removes correct ADJ/PRON/ADV readings
        // before verbs. Causes ~30 NOUN->VERB errors from error analysis R10.
        // The word before a verb can be PRON (hän tulee), ADJ (iso tulee),
        // or ADV (nopeasti tulee). SELECT nimisana discards all of these.
        // -----------------------------------------------------------------
        // Box::new(SelectIfFollowed {
        //     select_class: "nimisana".into(),
        //     followed_by_class: "teonsana".into(),
        // }),
        // -----------------------------------------------------------------
        // R14: When followed by a verb, remove pronoun readings.
        // Targets: PRON->NOUN confusion (180).
        // "talo seisoo" -- "talo" before verb should be noun, not pronoun.
        // DISABLED: Over-aggressive — pronouns ARE common before verbs
        // ("hän tulee", "se on", "kaikki tietävät"). This removes correct
        // PRON readings for subjects. Pronouns are the canonical subject
        // type before verbs.
        // REMOVE asemosana IF (+1 teonsana)
        // -----------------------------------------------------------------
        // Box::new(RemoveIfFollowed {
        //     remove_class: "asemosana".into(),
        //     followed_by_class: "teonsana".into(),
        // }),
        // -----------------------------------------------------------------
        // R15: When followed by auxiliary (kieltosana), prefer noun.
        // Targets: ADV->NOUN confusion.
        // "koira ei ..." -- noun before negation verb.
        // DISABLED: Over-aggressive — same problem as R13. The word before
        // "ei" can be a PRON ("hän ei"), ADV ("vielä ei"), ADJ, etc.
        // SELECT nimisana removes all non-noun readings incorrectly.
        // SELECT nimisana IF (+1 kieltosana)
        // -----------------------------------------------------------------
        // Box::new(SelectIfFollowed {
        //     select_class: "nimisana".into(),
        //     followed_by_class: "kieltosana".into(),
        // }),
        // =================================================================
        // PHASE 5: Adposition patterns
        // =================================================================
        //
        // R16: Before postposition/preposition, prefer noun.
        // Targets: ADV->NOUN, PRON->NOUN.
        // "talon takana" -- word before ADP is typically a noun (genitive).
        // DISABLED: Over-aggressive — pronouns also precede adpositions
        // ("hänen takana", "sen vuoksi"). SELECT nimisana incorrectly
        // removes PRON readings before ADP.
        // SELECT nimisana IF (+1 suhdesana)
        // -----------------------------------------------------------------
        // Box::new(SelectIfFollowed {
        //     select_class: "nimisana".into(),
        //     followed_by_class: "suhdesana".into(),
        // }),
        // -----------------------------------------------------------------
        // R17: After adposition, prefer noun (for the dependent).
        // "takana talon" (behind the house) or "ilman syytä" (without reason).
        // DISABLED: Over-aggressive — the word after an adposition can be
        // a verb, adjective, adverb, etc. in Finnish. Postpositions come
        // AFTER their argument, so the word AFTER a postposition is often
        // the start of a new phrase.
        // SELECT nimisana IF (-1 suhdesana)
        // -----------------------------------------------------------------
        // Box::new(SelectIfPreceded {
        //     select_class: "nimisana".into(),
        //     preceded_by_class: "suhdesana".into(),
        // }),
        // -----------------------------------------------------------------
        // R18: Adposition reading unlikely at sentence start.
        // Sentence-initial words are rarely postpositions.
        // REMOVE suhdesana IF (-1 BOS)
        // -----------------------------------------------------------------
        Box::new(RemoveAtSentenceStart {
            remove_class: "suhdesana".into(),
        }),
        // =================================================================
        // PHASE 6: Case-based disambiguation
        // =================================================================
        //
        // R19: If current position has an inessive case (sisaolento) reading,
        // remove verb readings. Words like "talossa" (-ssa) with inessive
        // are nouns, not verbs.
        // REMOVE teonsana IF (0 HAS SIJAMUOTO=sisaolento)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "sisaolento".into(),
        }),
        // -----------------------------------------------------------------
        // R20: If current position has an elative case (sisaeronto) reading,
        // remove verb readings. Words like "talosta" (-sta) are nouns.
        // REMOVE teonsana IF (0 HAS SIJAMUOTO=sisaeronto)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "sisaeronto".into(),
        }),
        // -----------------------------------------------------------------
        // R21: If current position has an adessive case (ulkoolento) reading,
        // remove verb readings. Words like "pöydällä" (-lla) are nouns.
        // REMOVE teonsana IF (0 HAS SIJAMUOTO=ulkoolento)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "ulkoolento".into(),
        }),
        // -----------------------------------------------------------------
        // R22: If current position has an ablative case (ulkoeronto) reading,
        // remove verb readings. Words like "pöydältä" (-lta) are nouns.
        // REMOVE teonsana IF (0 HAS SIJAMUOTO=ulkoeronto)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "ulkoeronto".into(),
        }),
        // -----------------------------------------------------------------
        // R23: If current position has an allative case (ulkotulento) reading,
        // remove verb readings. Words like "pöydälle" (-lle) are nouns.
        // REMOVE teonsana IF (0 HAS SIJAMUOTO=ulkotulento)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "ulkotulento".into(),
        }),
        // -----------------------------------------------------------------
        // R24: If current position has a translative case (tulento) reading,
        // remove verb readings. Words like "opettajaksi" (-ksi) are nouns.
        // REMOVE teonsana IF (0 HAS SIJAMUOTO=tulento)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "tulento".into(),
        }),
        // -----------------------------------------------------------------
        // R25: If current position has an illative case (sisatulento) reading,
        // remove verb readings. Words like "taloon" (-on/-Vn) are nouns.
        // REMOVE teonsana IF (0 HAS SIJAMUOTO=sisatulento)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "sisatulento".into(),
        }),
        // -----------------------------------------------------------------
        // R26: If current position has an essive case (olento) reading,
        // remove verb readings. Words like "opettajana" (-na) are nouns.
        // REMOVE teonsana IF (0 HAS SIJAMUOTO=olento)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "olento".into(),
        }),
        // -----------------------------------------------------------------
        // R27: If current position has an abessive case (vajanto) reading,
        // remove verb readings. Words like "syyttä" (-tta) are nouns.
        // REMOVE teonsana IF (0 HAS SIJAMUOTO=vajanto)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "vajanto".into(),
        }),
        // =================================================================
        // PHASE 7: ADV/NOUN sandwich & context patterns
        // =================================================================
        //
        // R28: Remove adverb reading when preceded by a noun.
        // Targets: ADV->NOUN confusion (278).
        // NOUN _ pattern in Finnish often means the second word is also a noun.
        // DISABLED: Over-aggressive — adverbs DO follow nouns commonly
        // ("koira nopeasti juoksee", "talo siellä"). This removes correct
        // ADV readings after nouns.
        // REMOVE seikkasana IF (-1 nimisana)
        // -----------------------------------------------------------------
        // Box::new(RemoveIfPreceded {
        //     remove_class: "seikkasana".into(),
        //     preceded_by_class: "nimisana".into(),
        // }),
        // -----------------------------------------------------------------
        // R29: After conjunction, prefer noun reading for the coordinated head.
        // "ja koira juoksee" -- after "ja", the next word is typically a
        // coordinated noun.
        // DISABLED: Over-aggressive — after conjunction, the next word can also
        // be VERB ("ja juoksee"), ADJ ("ja suuri"), ADV ("ja nopeasti").
        // SELECT nimisana removes all non-noun readings incorrectly.
        // SELECT nimisana IF (-1 sidesana)
        // -----------------------------------------------------------------
        // Box::new(SelectIfPreceded {
        //     select_class: "nimisana".into(),
        //     preceded_by_class: "sidesana".into(),
        // }),
        // =================================================================
        // PHASE 8: Adjective before noun patterns
        // =================================================================
        //
        // R30: When followed by a noun, prefer adjective over adverb.
        // "suuri talo" -- "suuri" before noun should be ADJ, not ADV.
        // SELECT laatusana IF (+1 nimisana)
        // -----------------------------------------------------------------
        Box::new(SelectIfFollowed {
            select_class: "laatusana".into(),
            followed_by_class: "nimisana".into(),
        }),
        // -----------------------------------------------------------------
        // R31: When followed by a proper noun, prefer adjective.
        // "suuri Suomi" -- adjective before proper noun.
        // DISABLED: Over-aggressive — before a proper noun, the word can be
        // another PROPN ("Matti Virtanen"), NOUN ("herra Matti"), or ADV.
        // SELECT laatusana removes all non-ADJ readings.
        // SELECT laatusana IF (+1 etunimi)
        // -----------------------------------------------------------------
        // Box::new(SelectIfFollowed {
        //     select_class: "laatusana".into(),
        //     followed_by_class: "etunimi".into(),
        // }),
        // =================================================================
        // PHASE 9: Adverb before verb/adjective patterns
        // =================================================================
        //
        // R32: When followed by an adjective, prefer adverb.
        // "erittäin suuri" -- adverb modifying adjective.
        // SELECT seikkasana IF (+1 laatusana)
        // -----------------------------------------------------------------
        Box::new(SelectIfFollowed {
            select_class: "seikkasana".into(),
            followed_by_class: "laatusana".into(),
        }),
        // -----------------------------------------------------------------
        // R33: After adverb, prefer verb reading over noun.
        // "nopeasti juoksee" -- adverb typically modifies a verb.
        // DISABLED: Over-aggressive — after adverb, the next word can also be
        // a NOUN ("paljon koiria"), ADJ ("erittäin suuri"), or another ADV.
        // SELECT teonsana removes all non-verb readings incorrectly.
        // This causes ~30 NOUN->VERB errors (error analysis R24).
        // SELECT teonsana IF (-1 seikkasana)
        // -----------------------------------------------------------------
        // Box::new(SelectIfPreceded {
        //     select_class: "teonsana".into(),
        //     preceded_by_class: "seikkasana".into(),
        // }),
        // =================================================================
        // PHASE 10: Genitive + noun patterns
        // =================================================================
        //
        // R34: If current position has a genitive (omanto) reading,
        // remove adverb readings. Genitive forms are nominal.
        // REMOVE seikkasana IF (0 HAS SIJAMUOTO=omanto)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "seikkasana".into(),
            has_case: "omanto".into(),
        }),
        // -----------------------------------------------------------------
        // R35: If current position has a partitive (osanto) reading,
        // remove verb readings. Partitive case is inherently nominal.
        // "koiraa" -- partitive is a noun form.
        // REMOVE teonsana IF (0 HAS SIJAMUOTO=osanto)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "osanto".into(),
        }),
        // =================================================================
        // PHASE 11: Post-verb object patterns
        // =================================================================
        //
        // R36: After verb, remove adverb reading when noun also exists.
        // "näkee talon" -- after verb, the object is typically a noun.
        // DISABLED: Over-aggressive — adverbs DO follow verbs commonly
        // ("juoksee nopeasti", "tuli takaisin", "meni pois"). This removes
        // correct ADV readings in verb-adverb patterns.
        // REMOVE seikkasana IF (-1 teonsana)
        // -----------------------------------------------------------------
        // Box::new(RemoveIfPreceded {
        //     remove_class: "seikkasana".into(),
        //     preceded_by_class: "teonsana".into(),
        // }),
        // -----------------------------------------------------------------
        // R37: After verb, remove pronoun reading when noun also exists.
        // "näkee talon" -- after verb, prefer noun over pronoun.
        // DISABLED: Over-aggressive — pronouns DO follow verbs commonly
        // ("näkee hänet", "teki sen", "ottaa sen"). This removes correct
        // PRON readings in object position.
        // REMOVE asemosana IF (-1 teonsana)
        // -----------------------------------------------------------------
        // Box::new(RemoveIfPreceded {
        //     remove_class: "asemosana".into(),
        //     preceded_by_class: "teonsana".into(),
        // }),
        // =================================================================
        // PHASE 12: NOUN/PROPN disambiguation
        // =================================================================
        //
        // R38: If POSSIBLE_GEOGRAPHICAL_NAME flag is set, remove plain
        // nimisana reading (prefer PROPN via pos_map).
        // REMOVE nimisana IF (0 HAS POSSIBLE_GEOGRAPHICAL_NAME=true)
        // -----------------------------------------------------------------
        Box::new(RemoveIfAttr {
            remove_class: "nimisana".into(),
            attr_name: ATTR_POSSIBLE_GEOGRAPHICAL_NAME.into(),
            attr_value: "true".into(),
        }),
        // -----------------------------------------------------------------
        // R39: If the word has both etunimi and nimisana readings, and the
        // previous word is also a proper noun or a title, prefer etunimi.
        // "Matti Virtanen" -- after a proper noun, another proper noun.
        // SELECT etunimi IF (-1 etunimi)
        // -----------------------------------------------------------------
        Box::new(SelectIfPreceded {
            select_class: "etunimi".into(),
            preceded_by_class: "etunimi".into(),
        }),
        // -----------------------------------------------------------------
        // R40: If the word has both sukunimi and nimisana readings, and
        // the previous word is a first name, prefer sukunimi.
        // "Matti Virtanen" -- surname after first name.
        // SELECT sukunimi IF (-1 etunimi)
        // -----------------------------------------------------------------
        Box::new(SelectIfPreceded {
            select_class: "sukunimi".into(),
            preceded_by_class: "etunimi".into(),
        }),
        // =================================================================
        // PHASE 13: Comparative/superlative ADJ patterns
        // =================================================================
        //
        // R41: If the word has a comparative form (COMPARISON=comparative),
        // prefer adjective reading. Comparatives are always adjectives.
        // "suurempi" (bigger) -- comparative is ADJ, not NOUN.
        // SELECT laatusana IF (0 HAS COMPARISON=comparative)
        // -----------------------------------------------------------------
        Box::new(SelectIfAttr {
            select_class: "laatusana".into(),
            attr_name: ATTR_COMPARISON.into(),
            attr_value: "comparative".into(),
        }),
        // -----------------------------------------------------------------
        // R42: If the word has a superlative form (COMPARISON=superlative),
        // prefer adjective reading.
        // "suurin" (biggest) -- superlative is ADJ, not NOUN.
        // SELECT laatusana IF (0 HAS COMPARISON=superlative)
        // -----------------------------------------------------------------
        Box::new(SelectIfAttr {
            select_class: "laatusana".into(),
            attr_name: ATTR_COMPARISON.into(),
            attr_value: "superlative".into(),
        }),
        // =================================================================
        // PHASE 14: Sandwich patterns
        // =================================================================
        //
        // R43: Adverb sandwiched between two nouns is unlikely.
        // "talo [X] talo" -- in N _ N pattern, prefer noun over adverb.
        // DISABLED: ADV between two nouns does occur in Finnish
        // ("mies vain katsoi" where both mies and katsoi might have N readings).
        // Also, since R13 (which selects N before V) is disabled, this
        // pattern fires less usefully now.
        // REMOVE seikkasana IF (-1 nimisana) (1 nimisana)
        // -----------------------------------------------------------------
        // Box::new(RemoveIfSandwiched {
        //     remove_class: "seikkasana".into(),
        //     preceded_by_class: "nimisana".into(),
        //     followed_by_class: "nimisana".into(),
        // }),
        // -----------------------------------------------------------------
        // R44: Between a noun and a verb, prefer adjective reading
        // (adnominal modifier in a relative clause or apposition).
        // "koira iso juoksee" => unlikely, but "iso" between noun/verb
        // is more likely adjective than adverb.
        // DISABLED: The SELECT is too aggressive. Between N and V, the
        // word could be another NOUN, a PRON, or an ADV. Forcing ADJ
        // reading is incorrect in many cases.
        // SELECT laatusana IF (-1 nimisana) (1 teonsana)
        // -----------------------------------------------------------------
        // Box::new(SelectIfSandwiched {
        //     select_class: "laatusana".into(),
        //     preceded_by_class: "nimisana".into(),
        //     followed_by_class: "teonsana".into(),
        // }),
        // -----------------------------------------------------------------
        // R45: Between conjunction and verb, prefer noun reading.
        // "ja koira juoksee" -- coordinated noun subject.
        // DISABLED: Between CONJ and VERB, the word can also be a PRON
        // ("ja hän tuli"), ADV ("ja nopeasti juoksi"), ADJ.
        // SELECT nimisana IF (-1 sidesana) (1 teonsana)
        // -----------------------------------------------------------------
        // Box::new(SelectIfSandwiched {
        //     select_class: "nimisana".into(),
        //     preceded_by_class: "sidesana".into(),
        //     followed_by_class: "teonsana".into(),
        // }),
        // =================================================================
        // PHASE 15: Remaining case-based and context patterns
        // =================================================================
        //
        // R46: Genitive-case reading removes adposition alternative.
        // If a word has genitive case, it's not an adposition.
        // REMOVE suhdesana IF (0 HAS SIJAMUOTO=omanto)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "suhdesana".into(),
            has_case: "omanto".into(),
        }),
        // -----------------------------------------------------------------
        // R47: Inessive-case reading removes adverb alternative.
        // If a word has inessive case, it's nominal, not adverb.
        // REMOVE seikkasana IF (0 HAS SIJAMUOTO=sisaolento)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "seikkasana".into(),
            has_case: "sisaolento".into(),
        }),
        // -----------------------------------------------------------------
        // R48: Elative-case reading removes adverb alternative.
        // REMOVE seikkasana IF (0 HAS SIJAMUOTO=sisaeronto)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "seikkasana".into(),
            has_case: "sisaeronto".into(),
        }),
        // -----------------------------------------------------------------
        // R49: Partitive-case reading removes adverb alternative.
        // REMOVE seikkasana IF (0 HAS SIJAMUOTO=osanto)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "seikkasana".into(),
            has_case: "osanto".into(),
        }),
        // -----------------------------------------------------------------
        // R50: Illative-case reading removes adverb alternative.
        // REMOVE seikkasana IF (0 HAS SIJAMUOTO=sisatulento)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "seikkasana".into(),
            has_case: "sisatulento".into(),
        }),
        // -----------------------------------------------------------------
        // R51: Adessive-case reading removes adverb alternative.
        // REMOVE seikkasana IF (0 HAS SIJAMUOTO=ulkoolento)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "seikkasana".into(),
            has_case: "ulkoolento".into(),
        }),
        // -----------------------------------------------------------------
        // R52: If the word has a participle attribute, prefer verb reading
        // (participial forms in UD Finnish-TDT are tagged VERB, not ADJ).
        // This rule removes adverb/noun alternatives when a participle
        // reading exists.
        // REMOVE seikkasana IF (0 HAS PARTICIPLE=past_passive)
        // -----------------------------------------------------------------
        Box::new(RemoveIfAttr {
            remove_class: "seikkasana".into(),
            attr_name: ATTR_PARTICIPLE.into(),
            attr_value: "past_passive".into(),
        }),
        // -----------------------------------------------------------------
        // R53: After "kuin" (SCONJ), prefer adjective reading for
        // comparative constructions. "suurempi kuin talo" -- the word
        // after "kuin" is typically a noun. But the word *before* "kuin"
        // is typically comparative ADJ.
        // DISABLED: Over-aggressive — after conjunction, verb IS common
        // ("ja tulee", "mutta sanoi"). Conflicts with R55 which selects
        // verb after SCONJ. Removing verb after conjunction hurts
        // coordinated verb patterns.
        // REMOVE teonsana IF (-1 sidesana)
        // -----------------------------------------------------------------
        // Box::new(RemoveIfPreceded {
        //     remove_class: "teonsana".into(),
        //     preceded_by_class: "sidesana".into(),
        // }),
        // =================================================================
        // PHASE 16: NOUN/VERB disambiguation refinements
        // Targets: VERB->NOUN (191) and NOUN->VERB (148) confusions.
        // =================================================================
        //
        // R54: After "joka/mikä" (relative pronouns), prefer verb reading.
        // "talo, joka seisoo" -- after relative pronoun, verb is expected.
        // SELECT teonsana IF (-1 BASEFORM IN {"joka", "mikä"})
        // -----------------------------------------------------------------
        Box::new(SelectByBaseformList {
            select_class: "teonsana".into(),
            preceded_by_baseforms: vec!["joka".into(), "mikä".into()],
        }),
        // -----------------------------------------------------------------
        // R55: After "kun/jos/vaikka/koska" (SCONJ), prefer verb reading.
        // "kun tulee" -- after subordinating conjunction, verb is expected.
        // SELECT teonsana IF (-1 BASEFORM IN {"kun", "jos", "vaikka",
        //   "koska", "kunnes", "jotta", "ellei"})
        // -----------------------------------------------------------------
        Box::new(SelectByBaseformList {
            select_class: "teonsana".into(),
            preceded_by_baseforms: vec![
                "kun".into(),
                "jos".into(),
                "vaikka".into(),
                "koska".into(),
                "kunnes".into(),
                "jotta".into(),
                "ellei".into(),
            ],
        }),
        // -----------------------------------------------------------------
        // R56: After "että" (complementizer), prefer verb reading.
        // "sanoi, että tulee" -- after "että", verb is expected.
        // SELECT teonsana IF (-1 BASEFORM "että")
        // -----------------------------------------------------------------
        Box::new(SelectByBaseform {
            select_class: "teonsana".into(),
            preceded_by_baseform: "että".into(),
        }),
        // -----------------------------------------------------------------
        // R57: After interrogative pronoun "kuka/ken", prefer verb reading.
        // "Kuka tuli?" -- "tuli" should be verb after interrogative.
        // SELECT teonsana IF (-1 BASEFORM IN {"kuka", "ken"})
        // -----------------------------------------------------------------
        Box::new(SelectByBaseformList {
            select_class: "teonsana".into(),
            preceded_by_baseforms: vec!["kuka".into(), "ken".into()],
        }),
        // -----------------------------------------------------------------
        // R58: Before "ei" (negation), prefer noun reading.
        // "koira ei juokse" -- word before negation is subject (noun).
        // DISABLED: Same problem as R13/R15 -- the word before "ei" can
        // be PRON ("hän ei"), ADV ("enää ei"), ADJ ("pitkä ei...").
        // SELECT nimisana is too aggressive.
        // SELECT nimisana IF (+1 BASEFORM IN {"ei"})
        // -----------------------------------------------------------------
        // Box::new(SelectIfFollowedByBaseformList {
        //     select_class: "nimisana".into(),
        //     followed_by_baseforms: vec!["ei".into()],
        // }),
        // -----------------------------------------------------------------
        // R59: REQUIRE_FOLLOWING_VERB attribute selects verb reading.
        // If the FST marks a word as requiring a following verb, it is
        // itself a verb (or auxiliary-like word).
        // SELECT teonsana IF (0 HAS REQUIRE_FOLLOWING_VERB=true)
        // -----------------------------------------------------------------
        Box::new(SelectIfAttr {
            select_class: "teonsana".into(),
            attr_name: ATTR_REQUIRE_FOLLOWING_VERB.into(),
            attr_value: "true".into(),
        }),
        // -----------------------------------------------------------------
        // R60: If the word has MOOD=indicative attribute, it is a finite
        // verb. Select verb reading.
        // SELECT teonsana IF (0 HAS MOOD=indicative)
        // -----------------------------------------------------------------
        Box::new(SelectIfAttr {
            select_class: "teonsana".into(),
            attr_name: ATTR_MOOD.into(),
            attr_value: "indicative".into(),
        }),
        // -----------------------------------------------------------------
        // R61: Conditional mood also indicates verb.
        // SELECT teonsana IF (0 HAS MOOD=conditional)
        // -----------------------------------------------------------------
        Box::new(SelectIfAttr {
            select_class: "teonsana".into(),
            attr_name: ATTR_MOOD.into(),
            attr_value: "conditional".into(),
        }),
        // -----------------------------------------------------------------
        // R62: Imperative mood indicates verb.
        // SELECT teonsana IF (0 HAS MOOD=imperative)
        // -----------------------------------------------------------------
        Box::new(SelectIfAttr {
            select_class: "teonsana".into(),
            attr_name: ATTR_MOOD.into(),
            attr_value: "imperative".into(),
        }),
        // -----------------------------------------------------------------
        // R63: NEGATIVE attribute indicates verb (connegative form).
        // SELECT teonsana IF (0 HAS NEGATIVE=true)
        // -----------------------------------------------------------------
        Box::new(SelectIfAttr {
            select_class: "teonsana".into(),
            attr_name: ATTR_NEGATIVE.into(),
            attr_value: "true".into(),
        }),
        // =================================================================
        // PHASE 17: Sentence-initial PROPN suppression
        // Targets: NOUN->PROPN confusion (187 errors).
        // At sentence start, common nouns with uppercase are over-tagged
        // as PROPN. Prefer nimisana when both readings exist.
        // =================================================================
        //
        // R64: At sentence start, prefer nimisana over etunimi.
        // "Vuonna 2020..." -- "Vuonna" is sentence-initial but is a
        // common noun (vuosi in essive), not a proper noun.
        // DISABLED: Over-aggressive SELECT. Some sentences DO start with
        // proper nouns ("Matti tuli kotiin", "Helsinki on kaunis").
        // SELECT nimisana removes etunimi/sukunimi/paikannimi readings.
        // However, sentence-initial capitalization in Finnish makes ALL
        // words look like proper nouns, so this was trying to fix that.
        // The net effect is unclear -- disabling to test.
        // SELECT nimisana IF (-1 BOS)
        // -----------------------------------------------------------------
        // Box::new(SelectAtSentenceStart {
        //     select_class: "nimisana".into(),
        // }),
        // -----------------------------------------------------------------
        // R65: At sentence start, remove sukunimi reading.
        // Sentence-initial surnames are unlikely without a preceding
        // first name.
        // DISABLED: Testing impact -- might be neutral or slightly helpful.
        // REMOVE sukunimi IF (-1 BOS)
        // -----------------------------------------------------------------
        // Box::new(RemoveAtSentenceStart {
        //     remove_class: "sukunimi".into(),
        // }),
        // =================================================================
        // PHASE 18: SCONJ patterns
        // Targets: improve SCONJ recall (currently 77.5% recall).
        // =================================================================
        //
        // R66: Known SCONJ baseforms should select alistuskonjunktio.
        // "koska hän tuli" -- "koska" should be SCONJ.
        // SELECT alistuskonjunktio IF (0 BASEFORM IN {"koska", "kun",
        //   "jos", "vaikka", "kunnes", "jotta", "ellei", "ettei",
        //   "mikäli", "joskin"})
        // -----------------------------------------------------------------
        Box::new(SelectByCurrentBaseformList {
            select_class: "alistuskonjunktio".into(),
            baseforms: vec![
                "koska".into(),
                "kun".into(),
                "jos".into(),
                "vaikka".into(),
                "kunnes".into(),
                "jotta".into(),
                "ellei".into(),
                "ettei".into(),
                "mikäli".into(),
                "joskin".into(),
            ],
        }),
        // -----------------------------------------------------------------
        // R67: "että" as complementizer should select alistuskonjunktio.
        // "sanoi, että tulee" -- "että" is SCONJ.
        // SELECT alistuskonjunktio IF (0 BASEFORM IN {"että"})
        // -----------------------------------------------------------------
        Box::new(SelectByCurrentBaseformList {
            select_class: "alistuskonjunktio".into(),
            baseforms: vec!["että".into()],
        }),
        // =================================================================
        // PHASE 19: Participle context rules
        // Targets: ADJ->VERB (134 errors).
        // Attributive participles before nouns should be ADJ, not VERB.
        // =================================================================
        //
        // R68: Remove nimisana when present active participle exists.
        // If a word has a participle reading, it is not a plain noun.
        // REMOVE nimisana IF (0 HAS PARTICIPLE=present_active)
        // -----------------------------------------------------------------
        Box::new(RemoveIfAttr {
            remove_class: "nimisana".into(),
            attr_name: ATTR_PARTICIPLE.into(),
            attr_value: "present_active".into(),
        }),
        // -----------------------------------------------------------------
        // R69: Remove nimisana when past passive participle exists.
        // REMOVE nimisana IF (0 HAS PARTICIPLE=past_passive)
        // -----------------------------------------------------------------
        Box::new(RemoveIfAttr {
            remove_class: "nimisana".into(),
            attr_name: ATTR_PARTICIPLE.into(),
            attr_value: "past_passive".into(),
        }),
        // -----------------------------------------------------------------
        // R70: Remove seikkasana when present active participle exists.
        // Adverb reading is unlikely if the word has a participle form.
        // REMOVE seikkasana IF (0 HAS PARTICIPLE=present_active)
        // -----------------------------------------------------------------
        Box::new(RemoveIfAttr {
            remove_class: "seikkasana".into(),
            attr_name: ATTR_PARTICIPLE.into(),
            attr_value: "present_active".into(),
        }),
        // =================================================================
        // PHASE 20: Additional case-based verb removal
        // Targets: NOUN->VERB (148 errors). If the word has a noun-typical
        // case, the verb reading should be removed more aggressively.
        // =================================================================
        //
        // R71: Comitative case (seuranto) indicates noun, not verb.
        // "koirineen" (-ineen) -- comitative is always nominal.
        // REMOVE teonsana IF (0 HAS SIJAMUOTO=seuranto)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "seuranto".into(),
        }),
        // -----------------------------------------------------------------
        // R72: Instructive case (kerrontosti) indicates adverb, not verb.
        // "jalkaisin" -- instructive is often adverbial.
        // REMOVE teonsana IF (0 HAS SIJAMUOTO=kerrontosti)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "kerrontosti".into(),
        }),
        // -----------------------------------------------------------------
        // R73: Comitative case also rules out adverb reading.
        // REMOVE seikkasana IF (0 HAS SIJAMUOTO=seuranto)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "seikkasana".into(),
            has_case: "seuranto".into(),
        }),
        // =================================================================
        // PHASE 21: ADP context refinements
        // Targets: ADP->NOUN (88), ADP->ADV (60) errors.
        // =================================================================
        //
        // R74: If word has both suhdesana and seikkasana readings and is
        // preceded by a noun, remove the adverb reading (prefer ADP).
        // "talon takana" -- "takana" after genitive noun is ADP.
        // REMOVE seikkasana IF (-1 nimisana) (0 HAS CLASS=suhdesana)
        // -----------------------------------------------------------------
        Box::new(RemoveIfPrecededAndAttr {
            remove_class: "seikkasana".into(),
            preceded_by_class: "nimisana".into(),
            attr_name: ATTR_CLASS.into(),
            attr_value: "suhdesana".into(),
        }),
        // -----------------------------------------------------------------
        // R75: Adposition reading unlikely when NOT followed by a noun.
        // DISABLED: Over-aggressive — postpositions follow their noun argument
        // (which precedes them), so the word AFTER a postposition need not
        // be a noun. Also, prepositions can be followed by pronouns or
        // proper nouns, not just nimisana. This rule incorrectly removes
        // ADP readings in many valid contexts.
        // REMOVE suhdesana IF (NOT +1 nimisana)
        // -----------------------------------------------------------------
        // Box::new(RemoveIfNotFollowed {
        //     remove_class: "suhdesana".into(),
        //     not_followed_by_class: "nimisana".into(),
        // }),
        // =================================================================
        // PHASE 22: Verb-verb sequence and coordination
        // Targets: VERB->NOUN (191 errors).
        // =================================================================
        //
        // R76: Between two verbs, the middle word is more likely verb.
        // "haluaa tulla takaisin" -- serial verb constructions.
        // REMOVE nimisana IF (-1 teonsana) (1 teonsana)
        // -----------------------------------------------------------------
        Box::new(RemoveIfSandwiched {
            remove_class: "nimisana".into(),
            preceded_by_class: "teonsana".into(),
            followed_by_class: "teonsana".into(),
        }),
        // =================================================================
        // PHASE 23: Adverb-adverb patterns and late-stage cleanup
        // =================================================================
        //
        // R77: When followed by an adverb, prefer adverb.
        // "hyvin nopeasti" -- adverb modifying another adverb.
        // SELECT seikkasana IF (+1 seikkasana)
        // -----------------------------------------------------------------
        Box::new(SelectIfFollowed {
            select_class: "seikkasana".into(),
            followed_by_class: "seikkasana".into(),
        }),
        // -----------------------------------------------------------------
        // R78: After adverb, if the word has genitive case, prefer noun
        // over verb.
        // "hyvin talon" -- genitive after adverb = noun, not verb.
        // REMOVE teonsana IF (-1 seikkasana) (0 HAS SIJAMUOTO=omanto)
        // -----------------------------------------------------------------
        Box::new(RemoveIfPrecededAndAttr {
            remove_class: "teonsana".into(),
            preceded_by_class: "seikkasana".into(),
            attr_name: ATTR_SIJAMUOTO.into(),
            attr_value: "omanto".into(),
        }),
        // -----------------------------------------------------------------
        // R79: After adverb, if the word has partitive case, prefer noun.
        // "paljon koiraa" -- partitive after adverb = noun, not verb.
        // REMOVE teonsana IF (-1 seikkasana) (0 HAS SIJAMUOTO=osanto)
        // -----------------------------------------------------------------
        Box::new(RemoveIfPrecededAndAttr {
            remove_class: "teonsana".into(),
            preceded_by_class: "seikkasana".into(),
            attr_name: ATTR_SIJAMUOTO.into(),
            attr_value: "osanto".into(),
        }),
        // -----------------------------------------------------------------
        // R80: Translative case removes adverb reading.
        // "opettajaksi" (-ksi) -- translative is nominal.
        // REMOVE seikkasana IF (0 HAS SIJAMUOTO=tulento)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "seikkasana".into(),
            has_case: "tulento".into(),
        }),
        // -----------------------------------------------------------------
        // R81: Allative case removes adverb reading.
        // "pöydälle" (-lle) -- allative is nominal.
        // REMOVE seikkasana IF (0 HAS SIJAMUOTO=ulkotulento)
        // -----------------------------------------------------------------
        Box::new(RemoveIfCase {
            remove_class: "seikkasana".into(),
            has_case: "ulkotulento".into(),
        }),
    ]
}

// ---------------------------------------------------------------------------
// Rule application: coKleisli composition over a sentence
// ---------------------------------------------------------------------------

/// Apply a sequence of CG rules over a sentence of reading sets.
///
/// Each rule is applied as a pass over the entire sentence using
/// [`Zipper::extend`], which treats the rule as a coKleisli arrow.
/// Rules are applied in order: the output of one pass becomes the
/// input to the next.
///
/// Returns the disambiguated sentence (one `ReadingSet` per position).
/// If `sentence` is empty, returns an empty vector.
pub fn apply_cg_rules(sentence: &[ReadingSet], rules: &[Box<dyn CgRule>]) -> Vec<ReadingSet> {
    if sentence.is_empty() {
        return Vec::new();
    }

    let mut current: Vec<ReadingSet> = sentence.to_vec();

    for rule in rules {
        // Build a zipper from the current sentence state.
        if let Some(z) = Zipper::new(current.clone()) {
            let result = z.extend(|focused| rule.apply(focused));
            current = result.to_vec();
        }
    }

    current
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use mce_core::analysis::ATTR_CLASS;

    // -- Helpers --

    fn make(class: &str) -> Analysis {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, class);
        a
    }

    fn make_with_baseform(class: &str, baseform: &str) -> Analysis {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, class);
        a.set("BASEFORM", baseform);
        a
    }

    fn classes(readings: &[Analysis]) -> Vec<&str> {
        let mut cs: Vec<&str> = readings.iter().filter_map(|a| a.get(ATTR_CLASS)).collect();
        cs.sort();
        cs
    }

    // -- RemoveIfPreceded -------------------------------------------------

    #[test]
    fn remove_if_preceded_removes_correct_reading() {
        // Sentence: [lukusana] [nimisana, teonsana]
        // Rule: REMOVE teonsana IF (-1 lukusana)
        let sentence = vec![
            vec![make("lukusana")],
            vec![make("nimisana"), make("teonsana")],
        ];

        let rule = RemoveIfPreceded {
            remove_class: "teonsana".into(),
            preceded_by_class: "lukusana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(result.len(), 2);
        // Position 0 unchanged
        assert_eq!(classes(&result[0]), vec!["lukusana"]);
        // Position 1: teonsana removed
        assert_eq!(classes(&result[1]), vec!["nimisana"]);
    }

    #[test]
    fn remove_if_preceded_no_match_at_minus1() {
        // Sentence: [nimisana] [nimisana, teonsana]
        // Rule: REMOVE teonsana IF (-1 lukusana)
        // Position -1 has nimisana, not lukusana, so nothing removed.
        let sentence = vec![
            vec![make("nimisana")],
            vec![make("nimisana"), make("teonsana")],
        ];

        let rule = RemoveIfPreceded {
            remove_class: "teonsana".into(),
            preceded_by_class: "lukusana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["nimisana", "teonsana"]);
    }

    #[test]
    fn remove_if_preceded_at_sentence_start() {
        // Position 0 has no left neighbor, so rule should not fire.
        let sentence = vec![vec![make("nimisana"), make("teonsana")]];

        let rule = RemoveIfPreceded {
            remove_class: "teonsana".into(),
            preceded_by_class: "lukusana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana", "teonsana"]);
    }

    // -- Safety: last reading never removed -------------------------------

    #[test]
    fn safety_last_reading_never_removed() {
        // Sentence: [lukusana] [teonsana]  (only one reading at pos 1)
        // Rule: REMOVE teonsana IF (-1 lukusana)
        // Without safety, pos 1 would become empty. Safety prevents this.
        let sentence = vec![vec![make("lukusana")], vec![make("teonsana")]];

        let rule = RemoveIfPreceded {
            remove_class: "teonsana".into(),
            preceded_by_class: "lukusana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(result[1].len(), 1);
        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn safety_select_no_matching_class_keeps_all() {
        // Sentence: [nimisana] [nimisana, teonsana]
        // Rule: SELECT laatusana IF (+1 something) — but no laatusana exists
        // Safety: since selecting laatusana would leave empty, keep all.
        let sentence = vec![
            vec![make("nimisana"), make("teonsana")],
            vec![make("nimisana")],
        ];

        let rule = SelectIfFollowed {
            select_class: "laatusana".into(),
            followed_by_class: "nimisana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        // All readings preserved due to safety
        assert_eq!(classes(&result[0]), vec!["nimisana", "teonsana"]);
    }

    // -- SelectIfFollowed ------------------------------------------------

    #[test]
    fn select_if_followed_selects_correct_reading() {
        // Sentence: [nimisana, teonsana] [nimisana]
        // Rule: SELECT teonsana IF (+1 nimisana)
        let sentence = vec![
            vec![make("nimisana"), make("teonsana")],
            vec![make("nimisana")],
        ];

        let rule = SelectIfFollowed {
            select_class: "teonsana".into(),
            followed_by_class: "nimisana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(result[0].len(), 1);
        assert_eq!(classes(&result[0]), vec!["teonsana"]);
        // Position 1 unchanged (no right neighbor)
        assert_eq!(classes(&result[1]), vec!["nimisana"]);
    }

    #[test]
    fn select_if_followed_no_match_at_plus1() {
        // Sentence: [nimisana, teonsana] [teonsana]
        // Rule: SELECT teonsana IF (+1 nimisana)
        // Position +1 has teonsana, not nimisana, so nothing selected.
        let sentence = vec![
            vec![make("nimisana"), make("teonsana")],
            vec![make("teonsana")],
        ];

        let rule = SelectIfFollowed {
            select_class: "teonsana".into(),
            followed_by_class: "nimisana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana", "teonsana"]);
    }

    #[test]
    fn select_if_followed_at_sentence_end() {
        // Last position has no right neighbor.
        let sentence = vec![vec![make("nimisana"), make("teonsana")]];

        let rule = SelectIfFollowed {
            select_class: "teonsana".into(),
            followed_by_class: "nimisana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana", "teonsana"]);
    }

    // -- RemoveIfNotPreceded -----------------------------------------------

    #[test]
    fn remove_if_not_preceded_fires_when_absent() {
        // Sentence: [nimisana] [laatusana, teonsana]
        // Rule: REMOVE laatusana IF (NOT -1 lukusana)
        // Position -1 has nimisana, not lukusana => NOT condition met => remove.
        let sentence = vec![
            vec![make("nimisana")],
            vec![make("laatusana"), make("teonsana")],
        ];

        let rule = RemoveIfNotPreceded {
            remove_class: "laatusana".into(),
            not_preceded_by_class: "lukusana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn remove_if_not_preceded_does_not_fire_when_present() {
        // Sentence: [lukusana] [laatusana, teonsana]
        // Rule: REMOVE laatusana IF (NOT -1 lukusana)
        // Position -1 HAS lukusana => NOT condition fails => no removal.
        let sentence = vec![
            vec![make("lukusana")],
            vec![make("laatusana"), make("teonsana")],
        ];

        let rule = RemoveIfNotPreceded {
            remove_class: "laatusana".into(),
            not_preceded_by_class: "lukusana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["laatusana", "teonsana"]);
    }

    #[test]
    fn remove_if_not_preceded_at_sentence_start() {
        // Position 0 has no left neighbor => class is absent => NOT fires.
        let sentence = vec![vec![make("laatusana"), make("teonsana")]];

        let rule = RemoveIfNotPreceded {
            remove_class: "laatusana".into(),
            not_preceded_by_class: "lukusana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["teonsana"]);
    }

    // -- Multiple rules composed ------------------------------------------

    #[test]
    fn multiple_rules_compose_sequentially() {
        // Sentence: [lukusana] [nimisana, teonsana, laatusana] [nimisana]
        //
        // Rule 1: REMOVE teonsana IF (-1 lukusana)
        //   => pos 1 becomes [nimisana, laatusana]
        //
        // Rule 2: SELECT nimisana IF (+1 nimisana)
        //   => pos 1 becomes [nimisana]
        let sentence = vec![
            vec![make("lukusana")],
            vec![make("nimisana"), make("teonsana"), make("laatusana")],
            vec![make("nimisana")],
        ];

        let rules: Vec<Box<dyn CgRule>> = vec![
            Box::new(RemoveIfPreceded {
                remove_class: "teonsana".into(),
                preceded_by_class: "lukusana".into(),
            }),
            Box::new(SelectIfFollowed {
                select_class: "nimisana".into(),
                followed_by_class: "nimisana".into(),
            }),
        ];

        let result = apply_cg_rules(&sentence, &rules);
        assert_eq!(result[1].len(), 1);
        assert_eq!(classes(&result[1]), vec!["nimisana"]);
    }

    #[test]
    fn three_rules_cascade() {
        // Sentence: [nimisana] [nimisana, teonsana, laatusana, seikkasana] [teonsana]
        //
        // Rule 1: REMOVE laatusana IF (NOT -1 lukusana)
        //   pos 0 has nimisana (not lukusana) => remove laatusana
        //   => pos 1: [nimisana, teonsana, seikkasana]
        //
        // Rule 2: REMOVE seikkasana IF (-1 nimisana)
        //   => pos 1: [nimisana, teonsana]
        //
        // Rule 3: SELECT teonsana IF (+1 teonsana)
        //   => pos 1: [teonsana]
        let sentence = vec![
            vec![make("nimisana")],
            vec![
                make("nimisana"),
                make("teonsana"),
                make("laatusana"),
                make("seikkasana"),
            ],
            vec![make("teonsana")],
        ];

        let rules: Vec<Box<dyn CgRule>> = vec![
            Box::new(RemoveIfNotPreceded {
                remove_class: "laatusana".into(),
                not_preceded_by_class: "lukusana".into(),
            }),
            Box::new(RemoveIfPreceded {
                remove_class: "seikkasana".into(),
                preceded_by_class: "nimisana".into(),
            }),
            Box::new(SelectIfFollowed {
                select_class: "teonsana".into(),
                followed_by_class: "teonsana".into(),
            }),
        ];

        let result = apply_cg_rules(&sentence, &rules);
        assert_eq!(result[1].len(), 1);
        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    // -- Edge cases -------------------------------------------------------

    #[test]
    fn empty_sentence() {
        let rules: Vec<Box<dyn CgRule>> = vec![Box::new(RemoveIfPreceded {
            remove_class: "teonsana".into(),
            preceded_by_class: "lukusana".into(),
        })];

        let result = apply_cg_rules(&[], &rules);
        assert!(result.is_empty());
    }

    #[test]
    fn single_position_sentence() {
        // Single position, rules referencing left/right neighbors should not fire.
        let sentence = vec![vec![make("nimisana"), make("teonsana")]];

        let rules: Vec<Box<dyn CgRule>> = vec![
            Box::new(RemoveIfPreceded {
                remove_class: "teonsana".into(),
                preceded_by_class: "lukusana".into(),
            }),
            Box::new(SelectIfFollowed {
                select_class: "nimisana".into(),
                followed_by_class: "nimisana".into(),
            }),
        ];

        let result = apply_cg_rules(&sentence, &rules);
        // Nothing should change
        assert_eq!(classes(&result[0]), vec!["nimisana", "teonsana"]);
    }

    #[test]
    fn no_rules_returns_original() {
        let sentence = vec![
            vec![make("nimisana"), make("teonsana")],
            vec![make("laatusana")],
        ];

        let result = apply_cg_rules(&sentence, &[]);
        assert_eq!(result.len(), 2);
        assert_eq!(classes(&result[0]), vec!["nimisana", "teonsana"]);
        assert_eq!(classes(&result[1]), vec!["laatusana"]);
    }

    #[test]
    fn position_with_empty_readings_unchanged() {
        // A position with no readings should stay empty (edge case).
        let sentence: Vec<ReadingSet> = vec![vec![make("lukusana")], vec![]];

        let rule = RemoveIfPreceded {
            remove_class: "teonsana".into(),
            preceded_by_class: "lukusana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(result[1].len(), 0);
    }

    // -- Finnish example: "kuusi koiraa" ---------------------------------

    #[test]
    fn finnish_kuusi_koiraa_disambiguation() {
        // "kuusi koiraa" — "six dogs"
        //
        // Position 0 ("kuusi"): nimisana (spruce) | lukusana (six)
        // Position 1 ("koiraa"): nimisana (dog, partitive)
        //
        // CG rule: SELECT lukusana IF (+1 nimisana)
        //   => When followed by a noun (partitive), prefer the numeral reading.
        //
        // This mirrors real Finnish CG behavior: a numeral before a partitive
        // noun is the common pattern ("kuusi koiraa" = "six dogs").
        let kuusi_noun = make_with_baseform("nimisana", "kuusi");
        let kuusi_num = make_with_baseform("lukusana", "kuusi");
        let koiraa = make_with_baseform("nimisana", "koira");

        let sentence = vec![vec![kuusi_noun, kuusi_num], vec![koiraa]];

        let rule = SelectIfFollowed {
            select_class: "lukusana".into(),
            followed_by_class: "nimisana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);

        assert_eq!(result.len(), 2);
        // Position 0: lukusana selected (six)
        assert_eq!(result[0].len(), 1);
        assert_eq!(result[0][0].get(ATTR_CLASS), Some("lukusana"));
        assert_eq!(result[0][0].get("BASEFORM"), Some("kuusi"));
        // Position 1: unchanged
        assert_eq!(result[1][0].get(ATTR_CLASS), Some("nimisana"));
        assert_eq!(result[1][0].get("BASEFORM"), Some("koira"));
    }

    #[test]
    fn finnish_kuusi_kasvaa_disambiguation() {
        // "kuusi kasvaa" — "the spruce grows"
        //
        // Position 0 ("kuusi"): nimisana (spruce) | lukusana (six)
        // Position 1 ("kasvaa"): teonsana (grows)
        //
        // CG rule: REMOVE lukusana IF (+1 has teonsana)
        //   => When followed by a verb, the numeral reading is unlikely.
        //   "kuusi kasvaa" = "the spruce grows" (not "six grows").
        let kuusi_noun = make_with_baseform("nimisana", "kuusi");
        let kuusi_num = make_with_baseform("lukusana", "kuusi");
        let kasvaa = make_with_baseform("teonsana", "kasvaa");

        let sentence = vec![vec![kuusi_noun, kuusi_num], vec![kasvaa]];

        // We express "REMOVE lukusana IF (+1 teonsana)" via SelectIfFollowed:
        // SELECT nimisana IF (+1 teonsana) — keep only noun when followed by verb.
        let rule = SelectIfFollowed {
            select_class: "nimisana".into(),
            followed_by_class: "teonsana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);

        assert_eq!(result[0].len(), 1);
        assert_eq!(result[0][0].get(ATTR_CLASS), Some("nimisana"));
        assert_eq!(result[0][0].get("BASEFORM"), Some("kuusi"));
    }

    // -- Verify coKleisli structure ----------------------------------------

    #[test]
    fn rule_is_cokleisli_arrow() {
        // Verify that a CG rule can be used directly with Zipper::extend,
        // confirming it is a genuine coKleisli morphism.
        let sentence = vec![
            vec![make("lukusana")],
            vec![make("nimisana"), make("teonsana")],
            vec![make("nimisana")],
        ];

        let z = Zipper::new(sentence).unwrap();

        let rule = RemoveIfPreceded {
            remove_class: "teonsana".into(),
            preceded_by_class: "lukusana".into(),
        };

        // Apply the rule as a coKleisli arrow via extend.
        let result = z.extend(|focused| rule.apply(focused));
        let output = result.to_vec();

        assert_eq!(output[0].len(), 1); // lukusana unchanged
        assert_eq!(classes(&output[1]), vec!["nimisana"]); // teonsana removed
        assert_eq!(output[2].len(), 1); // nimisana unchanged
    }

    // -- RemoveByClass ---------------------------------------------------

    #[test]
    fn remove_by_class_with_alternative() {
        // Sentence: [lukusana] [nimisana, lukusana]
        // Rule: REMOVE nimisana IF (-1 lukusana) IF (0 HAS lukusana)
        // Both conditions met => remove nimisana, keep lukusana.
        let sentence = vec![
            vec![make("lukusana")],
            vec![make("nimisana"), make("lukusana")],
        ];

        let rule = RemoveByClass {
            remove_class: "nimisana".into(),
            context_class: "lukusana".into(),
            require_alternative: "lukusana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["lukusana"]);
    }

    #[test]
    fn remove_by_class_no_alternative_keeps_all() {
        // Sentence: [lukusana] [nimisana, teonsana]
        // Rule: REMOVE nimisana IF (-1 lukusana) IF (0 HAS lukusana)
        // Context matches but no lukusana alternative => keep all.
        let sentence = vec![
            vec![make("lukusana")],
            vec![make("nimisana"), make("teonsana")],
        ];

        let rule = RemoveByClass {
            remove_class: "nimisana".into(),
            context_class: "lukusana".into(),
            require_alternative: "lukusana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["nimisana", "teonsana"]);
    }

    #[test]
    fn remove_by_class_no_context_keeps_all() {
        // Sentence: [nimisana] [nimisana, lukusana]
        // Rule: REMOVE nimisana IF (-1 lukusana) IF (0 HAS lukusana)
        // Position -1 is nimisana, not lukusana => context fails => keep all.
        let sentence = vec![
            vec![make("nimisana")],
            vec![make("nimisana"), make("lukusana")],
        ];

        let rule = RemoveByClass {
            remove_class: "nimisana".into(),
            context_class: "lukusana".into(),
            require_alternative: "lukusana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["lukusana", "nimisana"]);
    }

    // -- SelectByBaseform -------------------------------------------------

    #[test]
    fn select_by_baseform_fires() {
        // Sentence: [kieltosana baseform="ei"] [nimisana, teonsana]
        // Rule: SELECT teonsana IF (-1 baseform "ei")
        let mut ei = Analysis::new();
        ei.set(ATTR_CLASS, "kieltosana");
        ei.set("BASEFORM", "ei");

        let sentence = vec![vec![ei], vec![make("nimisana"), make("teonsana")]];

        let rule = SelectByBaseform {
            select_class: "teonsana".into(),
            preceded_by_baseform: "ei".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn select_by_baseform_no_match() {
        // Sentence: [nimisana baseform="koira"] [nimisana, teonsana]
        // Rule: SELECT teonsana IF (-1 baseform "ei")
        // Position -1 baseform is "koira", not "ei" => no change.
        let mut koira = Analysis::new();
        koira.set(ATTR_CLASS, "nimisana");
        koira.set("BASEFORM", "koira");

        let sentence = vec![vec![koira], vec![make("nimisana"), make("teonsana")]];

        let rule = SelectByBaseform {
            select_class: "teonsana".into(),
            preceded_by_baseform: "ei".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["nimisana", "teonsana"]);
    }

    #[test]
    fn select_by_baseform_safety() {
        // Only nimisana at pos 1 — selecting teonsana would leave empty.
        // Safety: keep all.
        let mut ei = Analysis::new();
        ei.set(ATTR_CLASS, "kieltosana");
        ei.set("BASEFORM", "ei");

        let sentence = vec![vec![ei], vec![make("nimisana")]];

        let rule = SelectByBaseform {
            select_class: "teonsana".into(),
            preceded_by_baseform: "ei".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(result[1].len(), 1);
        assert_eq!(classes(&result[1]), vec!["nimisana"]);
    }

    // -- SelectIfNotFollowed -----------------------------------------------

    #[test]
    fn select_if_not_followed_fires_when_absent() {
        // Sentence: [nimisana, laatusana] [nimisana]
        // Rule: SELECT nimisana IF (NOT +1 teonsana)
        // Position +1 has nimisana, not teonsana => NOT condition met => select.
        let sentence = vec![
            vec![make("nimisana"), make("laatusana")],
            vec![make("nimisana")],
        ];

        let rule = SelectIfNotFollowed {
            select_class: "nimisana".into(),
            not_followed_by_class: "teonsana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn select_if_not_followed_does_not_fire_when_present() {
        // Sentence: [nimisana, laatusana] [teonsana]
        // Rule: SELECT nimisana IF (NOT +1 teonsana)
        // Position +1 HAS teonsana => NOT condition fails => keep all.
        let sentence = vec![
            vec![make("nimisana"), make("laatusana")],
            vec![make("teonsana")],
        ];

        let rule = SelectIfNotFollowed {
            select_class: "nimisana".into(),
            not_followed_by_class: "teonsana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["laatusana", "nimisana"]);
    }

    #[test]
    fn select_if_not_followed_at_sentence_end() {
        // Last position has no right neighbor => class is absent => NOT fires.
        let sentence = vec![vec![make("nimisana"), make("laatusana")]];

        let rule = SelectIfNotFollowed {
            select_class: "nimisana".into(),
            not_followed_by_class: "teonsana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn select_if_not_followed_no_matching_select_class() {
        // Sentence: [laatusana, teonsana] [nimisana]
        // Rule: SELECT nimisana IF (NOT +1 teonsana)
        // NOT condition met, but no nimisana exists => keep all.
        let sentence = vec![
            vec![make("laatusana"), make("teonsana")],
            vec![make("nimisana")],
        ];

        let rule = SelectIfNotFollowed {
            select_class: "nimisana".into(),
            not_followed_by_class: "teonsana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["laatusana", "teonsana"]);
    }

    // -- RemoveIfFollowed --------------------------------------------------

    #[test]
    fn remove_if_followed_removes_correct_reading() {
        // Sentence: [nimisana, asemosana] [teonsana]
        // Rule: REMOVE asemosana IF (+1 teonsana)
        let sentence = vec![
            vec![make("nimisana"), make("asemosana")],
            vec![make("teonsana")],
        ];

        let rule = RemoveIfFollowed {
            remove_class: "asemosana".into(),
            followed_by_class: "teonsana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn remove_if_followed_no_match_at_plus1() {
        // Sentence: [nimisana, asemosana] [nimisana]
        // Rule: REMOVE asemosana IF (+1 teonsana)
        // Position +1 has nimisana, not teonsana => no removal.
        let sentence = vec![
            vec![make("nimisana"), make("asemosana")],
            vec![make("nimisana")],
        ];

        let rule = RemoveIfFollowed {
            remove_class: "asemosana".into(),
            followed_by_class: "teonsana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["asemosana", "nimisana"]);
    }

    #[test]
    fn remove_if_followed_safety() {
        // Only asemosana at pos 0 — removing it would leave empty.
        let sentence = vec![vec![make("asemosana")], vec![make("teonsana")]];

        let rule = RemoveIfFollowed {
            remove_class: "asemosana".into(),
            followed_by_class: "teonsana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(result[0].len(), 1);
        assert_eq!(classes(&result[0]), vec!["asemosana"]);
    }

    // -- Finnish rule set -------------------------------------------------

    #[test]
    fn finnish_rules_ei_voi_disambiguation() {
        // "ei voi" — "cannot"
        // "voi" is ambiguous: nimisana (butter) / teonsana (can)
        // After "ei" (kieltosana, baseform="ei"), verb reading should be selected.
        let mut ei = Analysis::new();
        ei.set(ATTR_CLASS, "kieltosana");
        ei.set("BASEFORM", "ei");
        let voi_noun = make_with_baseform("nimisana", "voi");
        let voi_verb = make_with_baseform("teonsana", "voida");

        let sentence = vec![vec![ei], vec![voi_noun, voi_verb]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(result[1].len(), 1);
        assert_eq!(result[1][0].get(ATTR_CLASS), Some("teonsana"));
    }

    #[test]
    fn finnish_rules_se_koira_disambiguation() {
        // "se koira" — "that dog"
        // R5 (REMOVE teonsana IF -1 asemosana) is DISABLED because pronouns
        // commonly precede verbs. Both readings should survive.
        let se = make("asemosana");
        let sentence = vec![vec![se], vec![make("nimisana"), make("teonsana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R5 disabled: both readings survive.
        assert_eq!(classes(&result[1]), vec!["nimisana", "teonsana"]);
    }

    #[test]
    fn finnish_rules_kolme_kissaa() {
        // "kolme kissaa" — "three cats"
        // Position 0 ("kolme"): lukusana | nimisana
        // Position 1 ("kissaa"): nimisana
        // Rules should select lukusana at pos 0 and remove verb at pos 1.
        let sentence = vec![
            vec![make("lukusana"), make("nimisana")],
            vec![make("nimisana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // Numeral selected before noun.
        assert_eq!(classes(&result[0]), vec!["lukusana"]);
    }

    #[test]
    fn finnish_rules_koira_juoksee() {
        // "koira juoksee" — "dog runs"
        // Position 0: nimisana | seikkasana | asemosana (ambiguous)
        // Position 1: teonsana
        // R13 (SELECT nimisana IF +1 teonsana) and R14 (REMOVE asemosana IF +1
        // teonsana) are DISABLED. All readings should survive at pos 0.
        let sentence = vec![
            vec![make("nimisana"), make("seikkasana"), make("asemosana")],
            vec![make("teonsana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R13/R14 disabled: all readings survive before verb.
        assert_eq!(
            classes(&result[0]),
            vec!["asemosana", "nimisana", "seikkasana"]
        );
    }

    #[test]
    fn finnish_rules_never_remove_last_reading() {
        // Even with the Finnish rule set, a position with a single reading
        // must never lose that reading.
        let sentence = vec![
            vec![make("asemosana")],
            vec![make("teonsana")], // single reading, R2 would try to remove
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // Safety: teonsana is the only reading, so it must survive.
        assert_eq!(result[1].len(), 1);
        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn finnish_rules_iso_koira() {
        // "iso koira" — "big dog"
        // Position 0 ("iso"): laatusana (adj)
        // Position 1 ("koira"): nimisana | teonsana
        // After adjective, verb reading should be removed.
        let sentence = vec![
            vec![make("laatusana")],
            vec![make("nimisana"), make("teonsana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[1]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_empty_sentence() {
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&[], &rules);
        assert!(result.is_empty());
    }

    // -- SelectIfPreceded --------------------------------------------------

    #[test]
    fn select_if_preceded_fires() {
        // Sentence: [sidesana] [nimisana, teonsana]
        // Rule: SELECT nimisana IF (-1 sidesana)
        let sentence = vec![
            vec![make("sidesana")],
            vec![make("nimisana"), make("teonsana")],
        ];

        let rule = SelectIfPreceded {
            select_class: "nimisana".into(),
            preceded_by_class: "sidesana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["nimisana"]);
    }

    #[test]
    fn select_if_preceded_no_match() {
        // Sentence: [nimisana] [nimisana, teonsana]
        // Rule: SELECT nimisana IF (-1 sidesana)
        // No sidesana at position -1.
        let sentence = vec![
            vec![make("nimisana")],
            vec![make("nimisana"), make("teonsana")],
        ];

        let rule = SelectIfPreceded {
            select_class: "nimisana".into(),
            preceded_by_class: "sidesana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["nimisana", "teonsana"]);
    }

    #[test]
    fn select_if_preceded_safety() {
        // Only teonsana at pos 1; selecting nimisana would leave empty.
        let sentence = vec![vec![make("sidesana")], vec![make("teonsana")]];

        let rule = SelectIfPreceded {
            select_class: "nimisana".into(),
            preceded_by_class: "sidesana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(result[1].len(), 1);
        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    // -- SelectByBaseformList -----------------------------------------------

    #[test]
    fn select_by_baseform_list_fires() {
        // "minä tulen" -- after personal pronoun, prefer verb.
        let mut mina = Analysis::new();
        mina.set(ATTR_CLASS, "asemosana");
        mina.set("BASEFORM", "minä");

        let sentence = vec![vec![mina], vec![make("nimisana"), make("teonsana")]];

        let rule = SelectByBaseformList {
            select_class: "teonsana".into(),
            preceded_by_baseforms: vec!["minä".into(), "sinä".into(), "hän".into()],
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn select_by_baseform_list_no_match() {
        let mut koira = Analysis::new();
        koira.set(ATTR_CLASS, "nimisana");
        koira.set("BASEFORM", "koira");

        let sentence = vec![vec![koira], vec![make("nimisana"), make("teonsana")]];

        let rule = SelectByBaseformList {
            select_class: "teonsana".into(),
            preceded_by_baseforms: vec!["minä".into(), "sinä".into()],
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["nimisana", "teonsana"]);
    }

    #[test]
    fn select_by_baseform_list_safety() {
        // Only nimisana at pos 1; selecting teonsana would leave empty.
        let mut mina = Analysis::new();
        mina.set(ATTR_CLASS, "asemosana");
        mina.set("BASEFORM", "minä");

        let sentence = vec![vec![mina], vec![make("nimisana")]];

        let rule = SelectByBaseformList {
            select_class: "teonsana".into(),
            preceded_by_baseforms: vec!["minä".into()],
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(result[1].len(), 1);
        assert_eq!(classes(&result[1]), vec!["nimisana"]);
    }

    // -- RemoveIfCase -------------------------------------------------------

    fn make_with_case(class: &str, case: &str) -> Analysis {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, class);
        a.set(ATTR_SIJAMUOTO, case);
        a
    }

    #[test]
    fn remove_if_case_fires() {
        // Word with inessive case reading: remove verb reading.
        // "talossa" -- nimisana(sisaolento) | teonsana
        let sentence = vec![vec![
            make_with_case("nimisana", "sisaolento"),
            make("teonsana"),
        ]];

        let rule = RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "sisaolento".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn remove_if_case_no_matching_case() {
        // No inessive case reading present.
        let sentence = vec![vec![
            make_with_case("nimisana", "nimento"),
            make("teonsana"),
        ]];

        let rule = RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "sisaolento".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana", "teonsana"]);
    }

    #[test]
    fn remove_if_case_safety() {
        // Only teonsana, removing would leave empty.
        let mut a = make("teonsana");
        a.set(ATTR_SIJAMUOTO, "sisaolento");
        let sentence = vec![vec![a]];

        let rule = RemoveIfCase {
            remove_class: "teonsana".into(),
            has_case: "sisaolento".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(result[0].len(), 1);
        assert_eq!(classes(&result[0]), vec!["teonsana"]);
    }

    // -- RemoveByBaseformList ------------------------------------------------

    #[test]
    fn remove_by_baseform_list_fires() {
        // After "olla", remove nimisana readings.
        let mut olla = Analysis::new();
        olla.set(ATTR_CLASS, "teonsana");
        olla.set("BASEFORM", "olla");

        let sentence = vec![vec![olla], vec![make("nimisana"), make("seikkasana")]];

        let rule = RemoveByBaseformList {
            remove_class: "nimisana".into(),
            preceded_by_baseforms: vec!["olla".into()],
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["seikkasana"]);
    }

    #[test]
    fn remove_by_baseform_list_no_match() {
        let mut koira = Analysis::new();
        koira.set(ATTR_CLASS, "nimisana");
        koira.set("BASEFORM", "koira");

        let sentence = vec![vec![koira], vec![make("nimisana"), make("seikkasana")]];

        let rule = RemoveByBaseformList {
            remove_class: "nimisana".into(),
            preceded_by_baseforms: vec!["olla".into()],
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["nimisana", "seikkasana"]);
    }

    // -- SelectIfFollowedByBaseformList -------------------------------------

    #[test]
    fn select_if_followed_by_baseform_list_fires() {
        let mut olla = Analysis::new();
        olla.set(ATTR_CLASS, "teonsana");
        olla.set("BASEFORM", "olla");

        let sentence = vec![vec![make("nimisana"), make("teonsana")], vec![olla]];

        let rule = SelectIfFollowedByBaseformList {
            select_class: "nimisana".into(),
            followed_by_baseforms: vec!["olla".into()],
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    // -- Finnish rules: new patterns ----------------------------------------

    #[test]
    fn finnish_rules_mina_tulen() {
        // "minä tulen" -- "I come"
        // After personal pronoun "minä", verb reading should be preferred.
        let mut mina = Analysis::new();
        mina.set(ATTR_CLASS, "asemosana");
        mina.set("BASEFORM", "minä");

        let sentence = vec![vec![mina], vec![make("nimisana"), make("teonsana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn finnish_rules_han_nayttaa() {
        // "hän näyttää" -- "s/he shows"
        // After personal pronoun "hän", verb reading preferred.
        let mut han = Analysis::new();
        han.set(ATTR_CLASS, "asemosana");
        han.set("BASEFORM", "hän");

        let sentence = vec![vec![han], vec![make("nimisana"), make("teonsana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn finnish_rules_on_tullut() {
        // "on tullut" -- "has come"
        // R3 (SELECT teonsana IF -1 BASEFORM "olla") is DISABLED because
        // "olla" can be followed by nouns/adjectives as copula.
        // Both readings survive.
        let mut on = Analysis::new();
        on.set(ATTR_CLASS, "teonsana");
        on.set("BASEFORM", "olla");

        let sentence = vec![vec![on], vec![make("nimisana"), make("teonsana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R3 disabled: both readings survive.
        assert_eq!(classes(&result[1]), vec!["nimisana", "teonsana"]);
    }

    #[test]
    fn finnish_rules_inessive_case_removes_verb() {
        // "talossa" -- inessive of "talo" (in the house)
        // If the word has an inessive (sisaolento) case reading alongside
        // a verb reading, the verb should be removed.
        let sentence = vec![vec![
            make_with_case("nimisana", "sisaolento"),
            make("teonsana"),
        ]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_genitive_removes_adverb() {
        // "talon" -- genitive of "talo" (of the house)
        // If the word has a genitive (omanto) case reading alongside
        // an adverb reading, the adverb should be removed.
        let sentence = vec![vec![
            make_with_case("nimisana", "omanto"),
            make("seikkasana"),
        ]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_partitive_removes_verb() {
        // "koiraa" -- partitive of "koira"
        // If the word has a partitive (osanto) case reading alongside
        // a verb reading, the verb should be removed.
        let sentence = vec![vec![make_with_case("nimisana", "osanto"), make("teonsana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_after_adposition_prefer_noun() {
        // "ilman syytä" -- "without reason"
        // R17 (SELECT nimisana IF -1 suhdesana) is DISABLED because
        // the word after an adposition can be various POS types.
        // Both readings survive.
        let sentence = vec![
            vec![make("suhdesana")],
            vec![make("nimisana"), make("seikkasana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R17 disabled: both readings survive.
        assert_eq!(classes(&result[1]), vec!["nimisana", "seikkasana"]);
    }

    #[test]
    fn finnish_rules_before_adposition_prefer_noun() {
        // "talon takana" -- "behind the house"
        // R16 (SELECT nimisana IF +1 suhdesana) is DISABLED because
        // pronouns can also precede adpositions.
        // Both readings survive.
        let sentence = vec![
            vec![make("nimisana"), make("seikkasana")],
            vec![make("suhdesana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R16 disabled: both readings survive.
        assert_eq!(classes(&result[0]), vec!["nimisana", "seikkasana"]);
    }

    #[test]
    fn finnish_rules_adj_before_noun() {
        // "suuri talo" -- "big house"
        // When followed by a noun, prefer adjective over adverb.
        let sentence = vec![
            vec![make("laatusana"), make("seikkasana")],
            vec![make("nimisana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["laatusana"]);
    }

    #[test]
    fn finnish_rules_adv_before_verb() {
        // "nopeasti juoksee" -- "quickly runs"
        // R33 (SELECT teonsana IF -1 seikkasana) is DISABLED because
        // after adverb, the next word can be a noun too.
        // Both readings survive.
        let sentence = vec![
            vec![make("seikkasana")],
            vec![make("nimisana"), make("teonsana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R33 disabled: both readings survive after adverb.
        assert_eq!(classes(&result[1]), vec!["nimisana", "teonsana"]);
    }

    #[test]
    fn finnish_rules_after_verb_remove_adverb_when_noun_exists() {
        // "näkee talon" -- "sees the house"
        // R36 (REMOVE seikkasana IF -1 teonsana) is DISABLED because
        // adverbs commonly follow verbs ("juoksee nopeasti").
        // Both readings survive.
        let sentence = vec![
            vec![make("teonsana")],
            vec![make("nimisana"), make("seikkasana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R36 disabled: both readings survive after verb.
        assert_eq!(classes(&result[1]), vec!["nimisana", "seikkasana"]);
    }

    #[test]
    fn finnish_rules_numeral_removes_adverb() {
        // "kolme kissaa" -- after numeral, remove adverb reading.
        // R9 (REMOVE seikkasana IF -1 lukusana) is DISABLED.
        // Both readings survive.
        let sentence = vec![
            vec![make("lukusana")],
            vec![make("nimisana"), make("seikkasana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R9 disabled: both readings survive.
        assert_eq!(classes(&result[1]), vec!["nimisana", "seikkasana"]);
    }

    #[test]
    fn finnish_rules_total() {
        // Verify the rule count.
        // 81 original - 24 disabled (R3,R5,R8,R9,R10,R13,R14,R15,R16,R17,
        // R28,R29,R31,R33,R36,R37,R43,R44,R45,R53,R58,R64,R65,R75) = 57 active
        let rules = finnish_disambiguation_rules();
        assert_eq!(rules.len(), 57);
    }

    // -- RemoveIfNotFollowed -----------------------------------------------

    #[test]
    fn remove_if_not_followed_fires_when_absent() {
        // Sentence: [suhdesana, nimisana] [teonsana]
        // Rule: REMOVE suhdesana IF (NOT +1 nimisana)
        // Position +1 has teonsana, not nimisana => NOT condition met => remove.
        let sentence = vec![
            vec![make("suhdesana"), make("nimisana")],
            vec![make("teonsana")],
        ];

        let rule = RemoveIfNotFollowed {
            remove_class: "suhdesana".into(),
            not_followed_by_class: "nimisana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn remove_if_not_followed_does_not_fire_when_present() {
        // Sentence: [suhdesana, nimisana] [nimisana]
        // Rule: REMOVE suhdesana IF (NOT +1 nimisana)
        // Position +1 HAS nimisana => NOT condition fails => no removal.
        let sentence = vec![
            vec![make("suhdesana"), make("nimisana")],
            vec![make("nimisana")],
        ];

        let rule = RemoveIfNotFollowed {
            remove_class: "suhdesana".into(),
            not_followed_by_class: "nimisana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana", "suhdesana"]);
    }

    #[test]
    fn remove_if_not_followed_at_sentence_end() {
        // Last position has no right neighbor => class absent => NOT fires.
        let sentence = vec![vec![make("suhdesana"), make("nimisana")]];

        let rule = RemoveIfNotFollowed {
            remove_class: "suhdesana".into(),
            not_followed_by_class: "nimisana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    // -- SelectIfAttr -------------------------------------------------------

    #[test]
    fn select_if_attr_fires() {
        // Word with comparative form: select adjective.
        let mut comp = make("laatusana");
        comp.set(ATTR_COMPARISON, "comparative");
        let sentence = vec![vec![comp, make("nimisana")]];

        let rule = SelectIfAttr {
            select_class: "laatusana".into(),
            attr_name: ATTR_COMPARISON.into(),
            attr_value: "comparative".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["laatusana"]);
    }

    #[test]
    fn select_if_attr_no_matching_attr() {
        // No comparative attribute present.
        let sentence = vec![vec![make("laatusana"), make("nimisana")]];

        let rule = SelectIfAttr {
            select_class: "laatusana".into(),
            attr_name: ATTR_COMPARISON.into(),
            attr_value: "comparative".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["laatusana", "nimisana"]);
    }

    // -- RemoveIfAttr -------------------------------------------------------

    #[test]
    fn remove_if_attr_fires() {
        // Word with geographical name flag: remove nimisana.
        let mut geo = make("nimisana");
        geo.set(ATTR_POSSIBLE_GEOGRAPHICAL_NAME, "true");
        let sentence = vec![vec![geo, make("etunimi")]];

        let rule = RemoveIfAttr {
            remove_class: "nimisana".into(),
            attr_name: ATTR_POSSIBLE_GEOGRAPHICAL_NAME.into(),
            attr_value: "true".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["etunimi"]);
    }

    #[test]
    fn remove_if_attr_no_matching_attr() {
        // No geographical name flag.
        let sentence = vec![vec![make("nimisana"), make("etunimi")]];

        let rule = RemoveIfAttr {
            remove_class: "nimisana".into(),
            attr_name: ATTR_POSSIBLE_GEOGRAPHICAL_NAME.into(),
            attr_value: "true".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["etunimi", "nimisana"]);
    }

    // -- SelectByCurrentBaseformList ----------------------------------------

    #[test]
    fn select_by_current_baseform_list_fires() {
        let olla = make_with_baseform("teonsana", "olla");
        let nimisana = make_with_baseform("nimisana", "olla");

        let sentence = vec![vec![olla, nimisana]];

        let rule = SelectByCurrentBaseformList {
            select_class: "teonsana".into(),
            baseforms: vec!["olla".into(), "voida".into()],
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["teonsana"]);
    }

    #[test]
    fn select_by_current_baseform_list_no_match() {
        let sentence = vec![vec![
            make_with_baseform("teonsana", "juosta"),
            make_with_baseform("nimisana", "juosta"),
        ]];

        let rule = SelectByCurrentBaseformList {
            select_class: "teonsana".into(),
            baseforms: vec!["olla".into()],
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana", "teonsana"]);
    }

    // -- RemoveIfSandwiched -------------------------------------------------

    #[test]
    fn remove_if_sandwiched_fires() {
        // noun - adverb - noun => remove adverb.
        let sentence = vec![
            vec![make("nimisana")],
            vec![make("seikkasana"), make("nimisana")],
            vec![make("nimisana")],
        ];

        let rule = RemoveIfSandwiched {
            remove_class: "seikkasana".into(),
            preceded_by_class: "nimisana".into(),
            followed_by_class: "nimisana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["nimisana"]);
    }

    #[test]
    fn remove_if_sandwiched_no_right_context() {
        // noun - adverb (no right neighbor) => no removal.
        let sentence = vec![
            vec![make("nimisana")],
            vec![make("seikkasana"), make("nimisana")],
        ];

        let rule = RemoveIfSandwiched {
            remove_class: "seikkasana".into(),
            preceded_by_class: "nimisana".into(),
            followed_by_class: "nimisana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["nimisana", "seikkasana"]);
    }

    #[test]
    fn remove_if_sandwiched_no_left_context() {
        // (start) - adverb - noun => no removal (no left context matches).
        let sentence = vec![
            vec![make("seikkasana"), make("nimisana")],
            vec![make("nimisana")],
        ];

        let rule = RemoveIfSandwiched {
            remove_class: "seikkasana".into(),
            preceded_by_class: "nimisana".into(),
            followed_by_class: "nimisana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        // Position 0 has no -1, so the rule does not fire.
        assert_eq!(classes(&result[0]), vec!["nimisana", "seikkasana"]);
    }

    // -- SelectIfSandwiched ------------------------------------------------

    #[test]
    fn select_if_sandwiched_fires() {
        // noun - (adj|adv) - verb => select adjective.
        let sentence = vec![
            vec![make("nimisana")],
            vec![make("laatusana"), make("seikkasana")],
            vec![make("teonsana")],
        ];

        let rule = SelectIfSandwiched {
            select_class: "laatusana".into(),
            preceded_by_class: "nimisana".into(),
            followed_by_class: "teonsana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["laatusana"]);
    }

    #[test]
    fn select_if_sandwiched_no_context() {
        // Only one neighbor => no sandwich.
        let sentence = vec![
            vec![make("nimisana")],
            vec![make("laatusana"), make("seikkasana")],
        ];

        let rule = SelectIfSandwiched {
            select_class: "laatusana".into(),
            preceded_by_class: "nimisana".into(),
            followed_by_class: "teonsana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["laatusana", "seikkasana"]);
    }

    // -- RemoveAtSentenceStart ----------------------------------------------

    #[test]
    fn remove_at_sentence_start_fires() {
        // Sentence start: remove suhdesana.
        let sentence = vec![
            vec![make("suhdesana"), make("nimisana")],
            vec![make("teonsana")],
        ];

        let rule = RemoveAtSentenceStart {
            remove_class: "suhdesana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana"]);
        // Position 1 unchanged (not at sentence start).
        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn remove_at_sentence_start_does_not_fire_midsentence() {
        // Not at sentence start.
        let sentence = vec![
            vec![make("nimisana")],
            vec![make("suhdesana"), make("nimisana")],
        ];

        let rule = RemoveAtSentenceStart {
            remove_class: "suhdesana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        // Position 1: suhdesana not removed (not at sentence start).
        assert_eq!(classes(&result[1]), vec!["nimisana", "suhdesana"]);
    }

    #[test]
    fn remove_at_sentence_start_safety() {
        // Only suhdesana at sentence start — safety keeps it.
        let sentence = vec![vec![make("suhdesana")], vec![make("nimisana")]];

        let rule = RemoveAtSentenceStart {
            remove_class: "suhdesana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(result[0].len(), 1);
        assert_eq!(classes(&result[0]), vec!["suhdesana"]);
    }

    // -- RemoveIfFollowedByBaseformList --------------------------------------

    #[test]
    fn remove_if_followed_by_baseform_list_fires() {
        let mut olla = Analysis::new();
        olla.set(ATTR_CLASS, "teonsana");
        olla.set("BASEFORM", "olla");

        let sentence = vec![vec![make("nimisana"), make("seikkasana")], vec![olla]];

        let rule = RemoveIfFollowedByBaseformList {
            remove_class: "seikkasana".into(),
            followed_by_baseforms: vec!["olla".into()],
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn remove_if_followed_by_baseform_list_no_match() {
        let mut juosta = Analysis::new();
        juosta.set(ATTR_CLASS, "teonsana");
        juosta.set("BASEFORM", "juosta");

        let sentence = vec![vec![make("nimisana"), make("seikkasana")], vec![juosta]];

        let rule = RemoveIfFollowedByBaseformList {
            remove_class: "seikkasana".into(),
            followed_by_baseforms: vec!["olla".into()],
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana", "seikkasana"]);
    }

    // -- Finnish rules: new pattern tests ----------------------------------

    #[test]
    fn finnish_rules_modal_aux_before_verb() {
        // "voi tehdä" -- after modal auxiliary, prefer verb.
        let mut voi = Analysis::new();
        voi.set(ATTR_CLASS, "teonsana");
        voi.set("BASEFORM", "voida");

        let sentence = vec![vec![voi], vec![make("nimisana"), make("teonsana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn finnish_rules_numeral_removes_pronoun() {
        // "viisi kissaa" -- after numeral, pronoun reading.
        // R10 (REMOVE asemosana IF -1 lukusana) is DISABLED because
        // pronouns can follow numerals ("kolme toista").
        // Both readings survive.
        let sentence = vec![
            vec![make("lukusana")],
            vec![make("nimisana"), make("asemosana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R10 disabled: both readings survive.
        assert_eq!(classes(&result[1]), vec!["asemosana", "nimisana"]);
    }

    #[test]
    fn finnish_rules_sentence_start_adposition_removed() {
        // Sentence-initial suhdesana is removed.
        let sentence = vec![
            vec![make("suhdesana"), make("nimisana")],
            vec![make("teonsana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_illative_case_removes_verb() {
        // "taloon" -- illative case removes verb reading.
        let sentence = vec![vec![
            make_with_case("nimisana", "sisatulento"),
            make("teonsana"),
        ]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_essive_case_removes_verb() {
        // "opettajana" -- essive case removes verb reading.
        let sentence = vec![vec![make_with_case("nimisana", "olento"), make("teonsana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_abessive_case_removes_verb() {
        // "syyttä" -- abessive case removes verb reading.
        let sentence = vec![vec![
            make_with_case("nimisana", "vajanto"),
            make("teonsana"),
        ]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_comparative_prefers_adjective() {
        // "suurempi" -- comparative prefers ADJ.
        let mut comp = make("laatusana");
        comp.set(ATTR_COMPARISON, "comparative");
        let sentence = vec![vec![comp, make("nimisana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["laatusana"]);
    }

    #[test]
    fn finnish_rules_superlative_prefers_adjective() {
        // "suurin" -- superlative prefers ADJ.
        let mut sup = make("laatusana");
        sup.set(ATTR_COMPARISON, "superlative");
        let sentence = vec![vec![sup, make("nimisana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["laatusana"]);
    }

    #[test]
    fn finnish_rules_geographical_name_removes_nimisana() {
        // Word with geographical name flag: remove plain noun.
        let mut geo = make("nimisana");
        geo.set(ATTR_POSSIBLE_GEOGRAPHICAL_NAME, "true");
        let sentence = vec![vec![geo, make("etunimi")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["etunimi"]);
    }

    #[test]
    fn finnish_rules_surname_after_firstname() {
        // "Matti Virtanen" -- select sukunimi after etunimi.
        let sentence = vec![
            vec![make("etunimi")],
            vec![make("sukunimi"), make("nimisana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[1]), vec!["sukunimi"]);
    }

    #[test]
    fn finnish_rules_noun_sandwich_removes_adverb() {
        // N - ADV/N - N => R43 (RemoveIfSandwiched) is DISABLED.
        // R28 (REMOVE seikkasana IF -1 nimisana) is also DISABLED.
        // Both readings survive in the middle position.
        let sentence = vec![
            vec![make("nimisana")],
            vec![make("seikkasana"), make("nimisana")],
            vec![make("nimisana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R43 and R28 disabled: both readings survive.
        assert_eq!(classes(&result[1]), vec!["nimisana", "seikkasana"]);
    }

    #[test]
    fn finnish_rules_conjunction_verb_sandwich_prefers_noun() {
        // "ja koira juoksee" => CONJ - N/ADV - V
        // R29 (SELECT nimisana IF -1 sidesana) and R45 (SelectIfSandwiched)
        // are both DISABLED. Both readings survive.
        let sentence = vec![
            vec![make("sidesana")],
            vec![make("nimisana"), make("seikkasana")],
            vec![make("teonsana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R29 and R45 disabled: both readings survive.
        assert_eq!(classes(&result[1]), vec!["nimisana", "seikkasana"]);
    }

    #[test]
    fn finnish_rules_after_verb_remove_pronoun() {
        // "näkee talon" -- after verb.
        // R37 (REMOVE asemosana IF -1 teonsana) is DISABLED because
        // pronouns commonly follow verbs as objects.
        // Both readings survive.
        let sentence = vec![
            vec![make("teonsana")],
            vec![make("nimisana"), make("asemosana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R37 disabled: both readings survive after verb.
        assert_eq!(classes(&result[1]), vec!["asemosana", "nimisana"]);
    }

    #[test]
    fn finnish_rules_before_negation_prefer_noun() {
        // "koira ei ..." -- noun before negation verb.
        // R15 (SELECT nimisana IF +1 kieltosana) is DISABLED because
        // pronouns/adverbs can also precede negation verbs.
        // Both readings survive.
        let sentence = vec![
            vec![make("nimisana"), make("seikkasana")],
            vec![make("kieltosana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R15 disabled: both readings survive before negation.
        assert_eq!(classes(&result[0]), vec!["nimisana", "seikkasana"]);
    }

    #[test]
    fn finnish_rules_inessive_removes_adverb() {
        // "talossa" -- inessive case removes adverb reading.
        let sentence = vec![vec![
            make_with_case("nimisana", "sisaolento"),
            make("seikkasana"),
        ]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_elative_removes_adverb() {
        // "talosta" -- elative case removes adverb reading.
        let sentence = vec![vec![
            make_with_case("nimisana", "sisaeronto"),
            make("seikkasana"),
        ]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_partitive_removes_adverb() {
        // "koiraa" -- partitive case removes adverb reading.
        let sentence = vec![vec![
            make_with_case("nimisana", "osanto"),
            make("seikkasana"),
        ]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_after_conjunction_remove_verb() {
        // "ja koira" -- after conjunction.
        // R29 (SELECT nimisana IF -1 sidesana) and R53 (REMOVE teonsana
        // IF -1 sidesana) are both DISABLED. Both readings survive.
        let sentence = vec![
            vec![make("sidesana")],
            vec![make("nimisana"), make("teonsana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R29 and R53 disabled: both readings survive after conjunction.
        assert_eq!(classes(&result[1]), vec!["nimisana", "teonsana"]);
    }

    #[test]
    fn finnish_rules_participle_removes_adverb() {
        // Word with past_passive participle: remove adverb.
        let mut participle = make("laatusana");
        participle.set(ATTR_PARTICIPLE, "past_passive");
        let sentence = vec![vec![participle, make("seikkasana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["laatusana"]);
    }

    #[test]
    fn finnish_rules_adj_before_propn() {
        // "suuri Suomi" -- adjective before proper noun.
        // R31 (SELECT laatusana IF +1 etunimi) is DISABLED because
        // other POS types can precede proper nouns.
        // Both readings survive.
        let sentence = vec![
            vec![make("laatusana"), make("seikkasana")],
            vec![make("etunimi")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R31 disabled: both readings survive.
        assert_eq!(classes(&result[0]), vec!["laatusana", "seikkasana"]);
    }

    #[test]
    fn finnish_rules_genitive_removes_adposition() {
        // "talon" -- genitive case removes adposition reading.
        let sentence = vec![vec![
            make_with_case("nimisana", "omanto"),
            make("suhdesana"),
        ]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    // -- SelectAtSentenceStart -----------------------------------------------

    #[test]
    fn select_at_sentence_start_fires() {
        // Sentence start: select nimisana over etunimi.
        let sentence = vec![
            vec![make("nimisana"), make("etunimi")],
            vec![make("teonsana")],
        ];

        let rule = SelectAtSentenceStart {
            select_class: "nimisana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana"]);
        // Position 1 unchanged (not at sentence start).
        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn select_at_sentence_start_does_not_fire_midsentence() {
        // Not at sentence start.
        let sentence = vec![
            vec![make("teonsana")],
            vec![make("nimisana"), make("etunimi")],
        ];

        let rule = SelectAtSentenceStart {
            select_class: "nimisana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["etunimi", "nimisana"]);
    }

    #[test]
    fn select_at_sentence_start_safety() {
        // Only etunimi at sentence start -- selecting nimisana would leave empty.
        let sentence = vec![vec![make("etunimi")], vec![make("nimisana")]];

        let rule = SelectAtSentenceStart {
            select_class: "nimisana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        // nimisana not present, so rule doesn't even fire.
        assert_eq!(result[0].len(), 1);
        assert_eq!(classes(&result[0]), vec!["etunimi"]);
    }

    // -- RemoveByCurrentBaseformList ------------------------------------------

    #[test]
    fn remove_by_current_baseform_list_fires() {
        let olla_verb = make_with_baseform("teonsana", "olla");
        let olla_noun = make_with_baseform("nimisana", "olla");

        let sentence = vec![vec![olla_verb, olla_noun]];

        let rule = RemoveByCurrentBaseformList {
            remove_class: "nimisana".into(),
            baseforms: vec!["olla".into(), "voida".into()],
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["teonsana"]);
    }

    #[test]
    fn remove_by_current_baseform_list_no_match() {
        let sentence = vec![vec![
            make_with_baseform("teonsana", "juosta"),
            make_with_baseform("nimisana", "juosta"),
        ]];

        let rule = RemoveByCurrentBaseformList {
            remove_class: "nimisana".into(),
            baseforms: vec!["olla".into()],
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[0]), vec!["nimisana", "teonsana"]);
    }

    // -- SelectIfPrecededByBaseformAndFollowed --------------------------------

    #[test]
    fn select_if_preceded_by_baseform_and_followed_fires() {
        // "ei voi tehdä" -- after "ei" and before verb, select verb.
        let mut ei = Analysis::new();
        ei.set(ATTR_CLASS, "kieltosana");
        ei.set("BASEFORM", "ei");

        let sentence = vec![
            vec![ei],
            vec![make("nimisana"), make("teonsana")],
            vec![make("teonsana")],
        ];

        let rule = SelectIfPrecededByBaseformAndFollowed {
            select_class: "teonsana".into(),
            preceded_by_baseforms: vec!["ei".into()],
            followed_by_class: "teonsana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn select_if_preceded_by_baseform_and_followed_no_right() {
        // No right context -- rule should not fire.
        let mut ei = Analysis::new();
        ei.set(ATTR_CLASS, "kieltosana");
        ei.set("BASEFORM", "ei");

        let sentence = vec![vec![ei], vec![make("nimisana"), make("teonsana")]];

        let rule = SelectIfPrecededByBaseformAndFollowed {
            select_class: "teonsana".into(),
            preceded_by_baseforms: vec!["ei".into()],
            followed_by_class: "teonsana".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["nimisana", "teonsana"]);
    }

    // -- RemoveIfPrecededAndAttr ----------------------------------------------

    #[test]
    fn remove_if_preceded_and_attr_fires() {
        // After adverb, if word has genitive case, remove verb.
        let sentence = vec![
            vec![make("seikkasana")],
            vec![make_with_case("nimisana", "omanto"), make("teonsana")],
        ];

        let rule = RemoveIfPrecededAndAttr {
            remove_class: "teonsana".into(),
            preceded_by_class: "seikkasana".into(),
            attr_name: ATTR_SIJAMUOTO.into(),
            attr_value: "omanto".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["nimisana"]);
    }

    #[test]
    fn remove_if_preceded_and_attr_no_preceding_class() {
        // No adverb at position -1 -- rule should not fire.
        let sentence = vec![
            vec![make("nimisana")],
            vec![make_with_case("nimisana", "omanto"), make("teonsana")],
        ];

        let rule = RemoveIfPrecededAndAttr {
            remove_class: "teonsana".into(),
            preceded_by_class: "seikkasana".into(),
            attr_name: ATTR_SIJAMUOTO.into(),
            attr_value: "omanto".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["nimisana", "teonsana"]);
    }

    #[test]
    fn remove_if_preceded_and_attr_no_matching_attr() {
        // Adverb at -1 but no genitive case -- rule should not fire.
        let sentence = vec![
            vec![make("seikkasana")],
            vec![make_with_case("nimisana", "nimento"), make("teonsana")],
        ];

        let rule = RemoveIfPrecededAndAttr {
            remove_class: "teonsana".into(),
            preceded_by_class: "seikkasana".into(),
            attr_name: ATTR_SIJAMUOTO.into(),
            attr_value: "omanto".into(),
        };

        let result = apply_cg_rules(&sentence, &[Box::new(rule)]);
        assert_eq!(classes(&result[1]), vec!["nimisana", "teonsana"]);
    }

    // -- New Finnish rules tests (Phase 16-23) --------------------------------

    #[test]
    fn finnish_rules_after_relative_pronoun() {
        // Test R54 in isolation: "joka seisoo" -- after relative pronoun "joka",
        // prefer verb. (With full rule set, R5 also fires since joka=asemosana.)
        let mut joka = Analysis::new();
        joka.set(ATTR_CLASS, "asemosana");
        joka.set("BASEFORM", "joka");

        let sentence = vec![vec![joka], vec![make("nimisana"), make("teonsana")]];

        // Test R54 alone.
        let rule: Box<dyn CgRule> = Box::new(SelectByBaseformList {
            select_class: "teonsana".into(),
            preceded_by_baseforms: vec!["joka".into(), "mikä".into()],
        });
        let result = apply_cg_rules(&sentence, &[rule]);
        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn finnish_rules_after_sconj_kun() {
        // "kun tulee" -- after "kun", prefer verb.
        let mut kun = Analysis::new();
        kun.set(ATTR_CLASS, "alistuskonjunktio");
        kun.set("BASEFORM", "kun");

        let sentence = vec![vec![kun], vec![make("nimisana"), make("teonsana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn finnish_rules_after_etta() {
        // "että tulee" -- after "että", prefer verb.
        let mut etta = Analysis::new();
        etta.set(ATTR_CLASS, "alistuskonjunktio");
        etta.set("BASEFORM", "että");

        let sentence = vec![vec![etta], vec![make("nimisana"), make("teonsana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn finnish_rules_mood_indicative_selects_verb() {
        // Word with indicative mood reading: select verb.
        let mut verb_mood = make("teonsana");
        verb_mood.set(ATTR_MOOD, "indicative");
        let sentence = vec![vec![verb_mood, make("nimisana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["teonsana"]);
    }

    #[test]
    fn finnish_rules_mood_conditional_selects_verb() {
        // Word with conditional mood: select verb.
        let mut verb_mood = make("teonsana");
        verb_mood.set(ATTR_MOOD, "conditional");
        let sentence = vec![vec![verb_mood, make("nimisana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["teonsana"]);
    }

    #[test]
    fn finnish_rules_sentence_initial_nimisana_over_etunimi() {
        // "Vuonna" -- sentence-initial common noun, not proper noun.
        // R64 (SelectAtSentenceStart nimisana) is DISABLED because
        // some sentences genuinely start with proper nouns.
        // Both readings survive.
        let sentence = vec![
            vec![make("nimisana"), make("etunimi")],
            vec![make("teonsana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R64 disabled: both readings survive at sentence start.
        assert_eq!(classes(&result[0]), vec!["etunimi", "nimisana"]);
    }

    #[test]
    fn finnish_rules_sentence_initial_sukunimi_removed() {
        // Sentence-initial sukunimi.
        // R65 (RemoveAtSentenceStart sukunimi) is DISABLED.
        // Both readings survive.
        let sentence = vec![
            vec![make("sukunimi"), make("nimisana")],
            vec![make("teonsana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // R65 disabled: both readings survive at sentence start.
        assert_eq!(classes(&result[0]), vec!["nimisana", "sukunimi"]);
    }

    #[test]
    fn finnish_rules_participle_removes_nimisana() {
        // Word with present active participle: remove nimisana.
        // We give the word a following noun so the laatusana isn't
        // removed by other rules (R30 selects laatusana before noun).
        let mut participle = make("laatusana");
        participle.set(ATTR_PARTICIPLE, "present_active");
        let sentence = vec![vec![participle, make("nimisana")], vec![make("nimisana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["laatusana"]);
    }

    #[test]
    fn finnish_rules_comitative_removes_verb() {
        // "koirineen" -- comitative case removes verb reading.
        let sentence = vec![vec![
            make_with_case("nimisana", "seuranto"),
            make("teonsana"),
        ]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_instructive_removes_verb() {
        // "jalkaisin" -- instructive case removes verb reading.
        let sentence = vec![vec![
            make_with_case("nimisana", "kerrontosti"),
            make("teonsana"),
        ]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_adv_before_adv() {
        // Test R77 in isolation: "hyvin nopeasti" -- adverb before adverb.
        // With the full rule set, R28 (REMOVE seikkasana IF -1 nimisana)
        // has already fired and other rules interact. Test individually.
        let sentence = vec![
            vec![make("seikkasana"), make("nimisana")],
            vec![make("seikkasana")],
        ];

        let rule: Box<dyn CgRule> = Box::new(SelectIfFollowed {
            select_class: "seikkasana".into(),
            followed_by_class: "seikkasana".into(),
        });
        let result = apply_cg_rules(&sentence, &[rule]);
        assert_eq!(classes(&result[0]), vec!["seikkasana"]);
    }

    #[test]
    fn finnish_rules_adv_genitive_removes_verb() {
        // Test R78 in isolation: After adverb, genitive case word prefers
        // noun over verb. With full rules, R33 (SELECT teonsana IF -1
        // seikkasana) fires first. Test the rule individually.
        let sentence = vec![
            vec![make("seikkasana")],
            vec![make_with_case("nimisana", "omanto"), make("teonsana")],
        ];

        let rule: Box<dyn CgRule> = Box::new(RemoveIfPrecededAndAttr {
            remove_class: "teonsana".into(),
            preceded_by_class: "seikkasana".into(),
            attr_name: ATTR_SIJAMUOTO.into(),
            attr_value: "omanto".into(),
        });
        let result = apply_cg_rules(&sentence, &[rule]);
        assert_eq!(classes(&result[1]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_translative_removes_adverb() {
        // "opettajaksi" -- translative case removes adverb reading.
        let sentence = vec![vec![
            make_with_case("nimisana", "tulento"),
            make("seikkasana"),
        ]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_allative_removes_adverb() {
        // "pöydälle" -- allative case removes adverb reading.
        let sentence = vec![vec![
            make_with_case("nimisana", "ulkotulento"),
            make("seikkasana"),
        ]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["nimisana"]);
    }

    #[test]
    fn finnish_rules_verb_verb_sandwich_removes_noun() {
        // Test R76 in isolation: verb-verb-verb sandwich removes noun.
        // With full rules, R36 and R37 fire first (REMOVE seikkasana/
        // asemosana IF -1 teonsana) which is fine, but R13 also fires
        // (SELECT nimisana IF +1 teonsana) overriding the sandwich.
        // Test the rule individually.
        let sentence = vec![
            vec![make("teonsana")],
            vec![make("nimisana"), make("teonsana")],
            vec![make("teonsana")],
        ];

        let rule: Box<dyn CgRule> = Box::new(RemoveIfSandwiched {
            remove_class: "nimisana".into(),
            preceded_by_class: "teonsana".into(),
            followed_by_class: "teonsana".into(),
        });
        let result = apply_cg_rules(&sentence, &[rule]);
        assert_eq!(classes(&result[1]), vec!["teonsana"]);
    }

    #[test]
    fn finnish_rules_negative_attr_selects_verb() {
        // Connegative form: select verb.
        let mut neg = make("teonsana");
        neg.set(ATTR_NEGATIVE, "true");
        let sentence = vec![vec![neg, make("nimisana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        assert_eq!(classes(&result[0]), vec!["teonsana"]);
    }

    #[test]
    fn finnish_rules_adp_not_followed_by_noun_removed() {
        // ADP not followed by noun is removed.
        let sentence = vec![
            vec![make("suhdesana"), make("seikkasana")],
            vec![make("teonsana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // suhdesana removed because not followed by nimisana.
        assert_eq!(classes(&result[0]), vec!["seikkasana"]);
    }
}
