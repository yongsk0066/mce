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
//! - Concrete rules: [`RemoveIfPreceded`], [`SelectIfFollowed`], [`RemoveIfNotPreceded`],
//!   [`RemoveByClass`], [`SelectByBaseform`], [`SelectIfNotFollowed`].
//! - [`finnish_disambiguation_rules`]: Pre-built Finnish CG rule set targeting
//!   the top UPOS confusions (ADJ/NOUN, ADV/NOUN, NOUN/PROPN, NOUN/VERB, PRON/NOUN).
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

use mce_core::analysis::{Analysis, ATTR_BASEFORM, ATTR_CLASS};

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
/// Rule ordering matters: rules are applied in sequence, each pass over
/// the full sentence. Earlier rules prune readings that later rules
/// can exploit.
pub fn finnish_disambiguation_rules() -> Vec<Box<dyn CgRule>> {
    vec![
        // -----------------------------------------------------------------
        // R1: After negation verb "ei", select verb readings.
        // Targets: NOUN->VERB confusion (207).
        // "ei voi" (cannot) — "voi" is ambiguous nimisana(butter)/teonsana(can).
        // After "ei", the verb reading is strongly preferred.
        // -----------------------------------------------------------------
        Box::new(SelectByBaseform {
            select_class: "teonsana".into(),
            preceded_by_baseform: "ei".into(),
        }),
        // -----------------------------------------------------------------
        // R2: After determiner-like pronoun (se, tämä, tuo), remove verb.
        // Targets: NOUN->VERB confusion (207).
        // "se koira" (that dog) — after a demonstrative, verb reading is unlikely.
        // "se" is asemosana. REMOVE teonsana IF (-1 asemosana)
        // -----------------------------------------------------------------
        Box::new(RemoveIfPreceded {
            remove_class: "teonsana".into(),
            preceded_by_class: "asemosana".into(),
        }),
        // -----------------------------------------------------------------
        // R3: After numeral, remove verb readings.
        // Targets: NOUN->VERB confusion (207).
        // "kolme koiraa" — after a numeral, the next word is typically a noun.
        // REMOVE teonsana IF (-1 lukusana)
        // -----------------------------------------------------------------
        Box::new(RemoveIfPreceded {
            remove_class: "teonsana".into(),
            preceded_by_class: "lukusana".into(),
        }),
        // -----------------------------------------------------------------
        // R4: After adjective, remove verb readings.
        // Targets: NOUN->VERB confusion (207).
        // "iso koira juoksee" — "koira" after "iso" (adj) is a noun, not verb.
        // "uusi auto" — "auto" after "uusi" (adj) should stay noun.
        // REMOVE teonsana IF (-1 laatusana)
        // -----------------------------------------------------------------
        Box::new(RemoveIfPreceded {
            remove_class: "teonsana".into(),
            preceded_by_class: "laatusana".into(),
        }),
        // -----------------------------------------------------------------
        // R5: Numeral + noun pattern: SELECT lukusana IF (+1 nimisana).
        // Targets: ADJ->NOUN confusion (helps disambiguate the numeral position).
        // "kolme kissaa" — prefer numeral before partitive noun.
        // -----------------------------------------------------------------
        Box::new(SelectIfFollowed {
            select_class: "lukusana".into(),
            followed_by_class: "nimisana".into(),
        }),
        // -----------------------------------------------------------------
        // R6: Noun before verb pattern: SELECT nimisana IF (+1 teonsana).
        // Targets: ADJ->NOUN (290), PRON->NOUN (180), ADV->NOUN (278).
        // "koira juoksee" — word before verb is typically the subject (noun).
        // -----------------------------------------------------------------
        Box::new(SelectIfFollowed {
            select_class: "nimisana".into(),
            followed_by_class: "teonsana".into(),
        }),
        // -----------------------------------------------------------------
        // R7: When followed by a verb, remove pronoun readings
        // where noun readings also exist.
        // Targets: PRON->NOUN confusion (180).
        // "talo seisoo" — "talo" before "seisoo" should be noun, not pronoun.
        // REMOVE asemosana IF (+1 teonsana)
        // -----------------------------------------------------------------
        Box::new(RemoveIfFollowed {
            remove_class: "asemosana".into(),
            followed_by_class: "teonsana".into(),
        }),
        // -----------------------------------------------------------------
        // R8: Remove adverb reading when preceded by adjective.
        // Targets: ADV->NOUN confusion (278).
        // After an adjective, the next word is more likely a noun than adverb.
        // REMOVE seikkasana IF (-1 laatusana)
        // -----------------------------------------------------------------
        Box::new(RemoveIfPreceded {
            remove_class: "seikkasana".into(),
            preceded_by_class: "laatusana".into(),
        }),
        // -----------------------------------------------------------------
        // R9: Remove adverb reading when followed by a noun and preceded by
        // a noun (sandwich context: NOUN _ NOUN is typically NOUN or ADJ).
        // Targets: ADV->NOUN confusion (278).
        // REMOVE seikkasana IF (-1 nimisana)
        // This is aggressive — but seikkasana/nimisana ambiguity after a noun
        // is often resolved in favor of noun.
        // -----------------------------------------------------------------
        Box::new(RemoveIfPreceded {
            remove_class: "seikkasana".into(),
            preceded_by_class: "nimisana".into(),
        }),
        // -----------------------------------------------------------------
        // R10: After postposition/preposition, prefer noun.
        // Targets: ADV->NOUN, PRON->NOUN.
        // "talon takana" — after ADP, the dependent is typically a noun.
        // Here we handle the reverse: "X suhdesana" — X is likely a noun.
        // SELECT nimisana IF (+1 suhdesana)
        // -----------------------------------------------------------------
        Box::new(SelectIfFollowed {
            select_class: "nimisana".into(),
            followed_by_class: "suhdesana".into(),
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
        // "koira" might be ambiguous with teonsana in a contrived scenario.
        // After "se" (asemosana), verb reading should be removed.
        let se = make("asemosana");
        let sentence = vec![vec![se], vec![make("nimisana"), make("teonsana")]];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // Verb reading removed after pronoun.
        assert_eq!(classes(&result[1]), vec!["nimisana"]);
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
        // Before verb, noun should be preferred over adverb/pronoun.
        let sentence = vec![
            vec![make("nimisana"), make("seikkasana"), make("asemosana")],
            vec![make("teonsana")],
        ];
        let rules = finnish_disambiguation_rules();
        let result = apply_cg_rules(&sentence, &rules);

        // Noun selected; adverb and pronoun removed before verb.
        assert_eq!(classes(&result[0]), vec!["nimisana"]);
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
}
