# mce-disambig

M4' disambiguation engine for MCE. Resolves morphological ambiguity by selecting the globally optimal POS reading sequence across a sentence.

## Pipeline

Disambiguation runs as a three-stage pipeline:

1. **CG-lite rules** (upstream, `mce-comonad`) -- high-precision deterministic elimination of impossible readings.
2. **Suffix Tagger** -- logistic regression emission scoring based on character suffix/prefix features.
3. **Viterbi decoder** -- bigram-based dynamic programming over a weighted lattice to find the best global sequence.

This pipeline lifts UPOS accuracy from 82.71% (rule-only) to **95.56%** (CG + Suffix Tagger + Viterbi).

## Suffix Tagger

A lightweight logistic regression POS tagger trained on UD Finnish-TDT. It does not replace the FST analysis; instead it re-ranks FST-generated candidates by providing per-UPOS emission log-probabilities to the Viterbi lattice.

| Property | Value |
|----------|-------|
| Standalone accuracy | 95.56% UPOS |
| Model format | MCET v1 (magic `b"MCET"`, INT8 quantized weights) |
| Model size | 5.0 MB |
| Features per word | ~20-30 sparse (suffixes 1-8, prefixes 1-5, word shape, context) |
| Inference | Sparse dot product + log-softmax |

`FeatureConfig` controls feature extraction parameters (suffix/prefix lengths, context window, word-form thresholds). `SuffixTagger::from_bytes()` loads a binary model file at runtime.

## Viterbi Decoder

Standard Viterbi algorithm over the disambiguation lattice. Complexity is O(n * |S|^2) where n = sentence length and |S| = max readings per position (typically 2-10 for morphological disambiguation).

Score at each position combines:
- **Emission scores** -- per-reading log-probabilities from the lattice (FST weights + suffix tagger scores).
- **Transition scores** -- POS bigram weights from `BigramModel`.

## Key Types

- **`Disambiguator`** -- trait with `disambiguate(&self, sentence: &[Vec<Analysis>]) -> Vec<Analysis>`
- **`ViterbiDisambiguator`** -- primary implementation combining `BigramModel`, optional `EmissionScorer`, optional `SuffixTagger`
- **`SuffixTagger`** -- loaded via `from_bytes()`, provides `emission_scores()` and `emission_scores_ext()`
- **`BigramModel`** -- POS transition weights; `finnish_defaults()` or `from_counts()` (corpus-derived)
- **`Lattice` / `LatticeNode` / `Reading`** -- weighted lattice data structures
- **`FeatureConfig`** -- suffix/prefix lengths, context features, word-form thresholds
- **`EmissionScorer`** -- surface-form-based emission adjustments (baseform match, structure penalty)
- **`SparseDisambiguator`** -- experimental Compressed Sensing scorer (FISTA)

## Example

```rust
use mce_core::analysis::Analysis;
use mce_disambig::{Disambiguator, ViterbiDisambiguator};
use mce_disambig::suffix_tagger::SuffixTagger;

// 1. Set up disambiguator with Finnish POS bigram defaults
let mut disambiguator = ViterbiDisambiguator::with_finnish_defaults();

// 2. Load suffix tagger model (optional but recommended for 95%+ accuracy)
let model_bytes = std::fs::read("data/suffix_tagger.bin").unwrap();
let tagger = SuffixTagger::from_bytes(&model_bytes).unwrap();
disambiguator.set_suffix_tagger(tagger);

// 3. Build a sentence with ambiguous words
let mut noun = Analysis::new();
noun.set("CLASS", "nimisana");   // NOUN
noun.set("BASEFORM", "kuusi");   // spruce

let mut num = Analysis::new();
num.set("CLASS", "lukusana");    // NUM
num.set("BASEFORM", "kuusi");    // six

let mut verb = Analysis::new();
verb.set("CLASS", "teonsana");   // VERB
verb.set("BASEFORM", "kasvaa");  // grows

// 4. Disambiguate with surface forms for suffix tagger scoring
let words = &["kuusi", "kasvaa"];
let sentence = vec![
    vec![noun, num],    // "kuusi" -- spruce or six?
    vec![verb],         // "kasvaa" -- grows
];
let result = disambiguator.disambiguate_with_words(words, &sentence);

assert_eq!(result[0].get("CLASS"), Some("nimisana")); // NOUN wins
assert_eq!(result[1].get("CLASS"), Some("teonsana")); // VERB
```

## Dependencies

**Uses:** `mce-core`

**Used by:** `mce-fi`, `mce-eval`, `mce-grammar`, `mce-wasm`, `mce-cli`
