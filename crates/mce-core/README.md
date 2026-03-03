# mce-core

Shared types and foundational data structures for the MCE (Morphological Computation Engine).

## Purpose

Every other MCE crate depends on `mce-core`. It defines the vocabulary of types that flow through the entire pipeline: morphological analysis results, token and sentence boundaries, character classification, case pattern detection, word frequency ranking, and compound word splitting.

The crate also hosts the M1 Succinct Trie, a LOUDS-encoded dictionary that serves as the engine's primary lookup structure. Because it is a leaf crate with only one external dependency (`thiserror`), it compiles quickly and keeps the dependency graph shallow.

## Key Types

- **`Analysis`** -- Key-value attribute set representing one morphological reading of a word. Attributes include `BASEFORM`, `CLASS`, `SIJAMUOTO` (case), `MOOD`, `TENSE`, and 15+ others adapted from the Voikko analysis model.
- **`CharType` / `get_char_type`** -- Classifies Unicode code points into `Letter`, `Digit`, `Whitespace`, `Punctuation`, or `Unknown`. Covers Latin, Cyrillic, and Canadian Aboriginal ranges.
- **`CaseType` / `detect_case` / `set_case`** -- Detects and transforms word-level casing patterns (`AllLower`, `FirstUpper`, `AllUpper`, `Complex`).
- **`CompoundAnalyzer<F>`** -- M3 pushdown transducer for Finnish compound word splitting. Handles zero-linking (`jaakaappi`), genitive `-n-` linking (`kissanpentu`), `-en-` linking with nen-stem reconstruction (`hevosenkenkä`), and hyphenated compounds (`maa-alue`).
- **`FrequencyList`** -- Word frequency lookup parsed from CoNLL-U corpora. Supports absolute/relative frequency, rank queries, and compact binary serialization.
- **`Token` / `Sentence`** -- Token and sentence boundary types used by `mce-tokenizer`.
- **`SuccinctTrie` / `TrieBuilder`** -- M1 LOUDS-encoded dictionary. ~2 bits per node, O(|key|) exact lookup, and edit-distance fuzzy search with DP pruning.

## Example

```rust
use mce_core::compound::{CompoundAnalyzer, CompoundSplit};

let dict = |w: &str| matches!(w, "rauta" | "tie" | "asema" | "rautatie");
let analyzer = CompoundAnalyzer::new(dict);
let splits = analyzer.analyze("rautatieasema");

// Best split (lowest penalty): rautatie + asema
assert_eq!(splits[0].word_parts().len(), 2);

// Also finds: rauta + tie + asema
let three = splits.iter().find(|s| s.word_parts().len() == 3);
assert!(three.is_some());
```

```rust
use mce_core::trie::TrieBuilder;

let mut builder = TrieBuilder::new();
builder.insert(b"cat");
builder.insert(b"car");
builder.insert(b"card");
let trie = builder.build();

assert!(trie.contains(b"car"));
assert!(!trie.contains(b"ca"));

// Fuzzy search within edit distance 1
let suggestions = trie.fuzzy_search(b"cot", 1);
assert!(suggestions.contains(&b"cat".to_vec()));
```

## Architecture Notes

**M1 Succinct Trie.** The dictionary is stored in LOUDS (Level-Order Unary Degree Sequence) encoding. Each node costs ~2 bits in the tree bitvector plus one byte for the edge label. Rank/Select operations provide O(1) parent-child navigation. The `TrieBuilder` constructs the trie via BFS over an intermediate plain trie.

**M3 Compound Analyzer.** Finnish compounds are context-free structures (binary branching trees). The analyzer uses recursive descent where the call stack serves as the implicit pushdown stack. It tries three strategies at each position: direct dictionary match, post-word linking morphemes, and fused stem+linking with optional reconstruction via a caller-supplied callback.

**Character Classification.** Hand-tuned Unicode ranges (not `char::is_alphabetic()`) ensure consistent behavior across platforms and match the original Voikko classification.

## Dependencies

- `thiserror` (error derive macro)
- `criterion` (dev only, benchmarks)

## Used By

All other MCE crates: `mce-fst`, `mce-tokenizer`, `mce-speller`, `mce-comonad`, `mce-disambig`, `mce-fi`, `mce-grammar`, `mce-eval`, `mce-wasm`, `mce-cli`.

## Benchmarks

```bash
cargo bench -p mce-core
```

Runs Succinct Trie performance benchmarks (exact lookup, fuzzy search).
