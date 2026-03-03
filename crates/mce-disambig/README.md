# mce-disambig

Disambiguation engine (Weighted Lattice + Viterbi + Suffix Tagger) for MCE.

## Purpose

This crate resolves morphological ambiguity by selecting the globally optimal reading sequence across a sentence. It implements the M4' machine in the MCE v3 architecture using a three-stage pipeline: CG-lite rule elimination (upstream), suffix-based logistic regression emission scoring, and bigram Viterbi decoding over a weighted lattice.

## Key Types

- `Disambiguator` trait — interface for disambiguation strategies
- `ViterbiDisambiguator` — primary implementation using POS bigram transitions
- `bigram::BigramModel` — POS transition weight model (hand-tuned or corpus-derived)
- `bigram::EmissionScorer` — surface-form-based emission adjustments
- `suffix_tagger::SuffixTagger` — lightweight logistic regression POS tagger (~94.87% standalone)
- `lattice::Lattice` / `LatticeNode` / `Reading` — weighted lattice data structures
- `viterbi::viterbi()` — Viterbi algorithm for optimal path finding
- `corpus` — CoNLL-U parser for bigram extraction
- `cs::SparseDisambiguator` — experimental Compressed Sensing scorer

## Dependencies

Uses: `mce-core`

Used by: `mce-fi`, `mce-eval`, `mce-grammar`, `mce-wasm`, `mce-cli`

## Example

```rust
use mce_core::analysis::Analysis;
use mce_disambig::{Disambiguator, ViterbiDisambiguator};

let mut noun = Analysis::new();
noun.set("CLASS", "nimisana");
noun.set("BASEFORM", "kuusi");

let mut num = Analysis::new();
num.set("CLASS", "lukusana");
num.set("BASEFORM", "kuusi");

let sentence = vec![vec![noun, num]];
let d = ViterbiDisambiguator::with_finnish_defaults();
let result = d.disambiguate(&sentence);
// Selects the best reading based on POS bigram transitions
```
