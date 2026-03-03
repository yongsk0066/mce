# mce-core

Shared types and foundational data structures for the MCE engine.

## Purpose

This crate defines the core types that every other MCE crate depends on. It provides morphological analysis result containers, Unicode character classification, case pattern detection, compound word analysis via a pushdown transducer (M3), word frequency lists, token/sentence boundary types, and the M1 Succinct Trie (LOUDS-encoded dictionary).

## Key Types

- `analysis::Analysis` — key-value attribute set representing a morphological reading
- `character::CharType` / `get_char_type` — Unicode character classification (letter, digit, whitespace, punctuation)
- `case::CasePattern` — detects and converts case patterns (UPPER, Title, lower)
- `compound::CompoundAnalyzer` — M3 pushdown transducer for compound word splitting
- `frequency::FrequencyList` — word frequency lookup (CoNLL-U-derived)
- `token::TokenType` / `SentenceType` — token and sentence boundary classification
- `trie::SuccinctTrie` — M1 LOUDS-encoded dictionary for fast prefix/exact lookup

## Dependencies

Uses: `thiserror`

Used by: all other MCE crates

## Benchmarks

Run `cargo bench -p mce-core` for Succinct Trie performance benchmarks.
