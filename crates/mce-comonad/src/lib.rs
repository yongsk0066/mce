//! MCE Comonad — M2' Comonadic morphophonological engine.
//!
//! This crate provides the core comonadic infrastructure for MCE's
//! context-dependent morphophonological rule system. Where a monad
//! abstracts over *effects* (I/O, state, failure), a comonad abstracts
//! over *context* — exactly what morphophonological rules need.
//!
//! # Why comonads for morphophonology?
//!
//! Finnish morphophonological rules (vowel harmony, consonant gradation,
//! allomorph selection) share a common computational pattern: each rule
//! inspects the **local context** (neighboring morphemes or characters)
//! around a focus position and produces a transformed output for that
//! position. The comonadic `extend` operation captures this pattern
//! precisely — it lifts a local, context-dependent function into a
//! global transformation over the entire sequence.
//!
//! In category-theoretic terms, each morphophonological rule is a
//! **coKleisli arrow** `W A -> B`, and rule composition is coKleisli
//! composition `(=>=)`. This gives us:
//!
//! - **Modularity**: each rule is an independent function, testable in isolation.
//! - **Composability**: rules compose cleanly via `extend` chaining.
//! - **Correctness**: the comonad laws guarantee identity and associativity.
//!
//! # Core data structure
//!
//! The [`Zipper`] (list zipper) is the primary comonad. It represents a
//! 1-D sequence with a focused element and bidirectional context access.
//! This maps directly to the bimachine model from Schutzenberger's theory:
//! a bimachine computes each output symbol from left and right context,
//! and `Zipper::extend` does the same — but with the added flexibility
//! of composable, higher-order rule functions rather than fixed state machines.
//!
//! # Relationship to MCE architecture
//!
//! In the MCE v3 architecture, this crate unifies what were previously
//! two separate machines:
//!
//! - **M2 (Bimachine Cascade)**: morphophonological transformations
//! - **M5 (Rule Engine)**: CG-lite disambiguation rules
//!
//! Both are naturally expressed as coKleisli arrows over a zipper and
//! composed via `extend`.
//!
//! # References
//!
//! - Uustalu, T. & Vene, V. (2005). "The Essence of Dataflow Programming."
//! - Capobianco, S. & Uustalu, T. (2010). "A Categorical Outlook on Cellular Automata."
//! - Orchard, D. (2012). "Should I use a Monad or a Comonad?"

pub mod finnish;
pub mod zipper;

pub use zipper::Zipper;
