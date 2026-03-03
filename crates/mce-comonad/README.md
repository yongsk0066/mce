# mce-comonad

Comonadic morphophonological engine for context-dependent rule application.

## Purpose

This crate provides the M2' engine in the MCE v3 architecture. It uses the Writer comonad and list zipper to express morphophonological rules (vowel harmony, consonant gradation, allomorph selection) as coKleisli arrows. Each rule inspects local context around a focus position and produces a transformed output; rules compose cleanly via `extend` chaining with guaranteed identity and associativity from the comonad laws.

## Key Types

- `Zipper<A>` — list zipper comonad: a focused sequence with bidirectional context
- `writer` — Writer comonad for composable rule application with deletion support
- `cg` — Constraint Grammar (CG-lite) rules for deterministic disambiguation
- `finnish` — Finnish-specific coKleisli arrows (vowel harmony, gradation)
- `bench` — benchmarking utilities for CG rules and coKleisli arrows

## Dependencies

Uses: `mce-core`

Used by: `mce-fi`, `mce-eval`, `mce-wasm`, `mce-cli`
