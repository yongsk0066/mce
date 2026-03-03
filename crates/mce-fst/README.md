# mce-fst

FST engine with VFST format loading, traversal, and flag diacritic support.

## Purpose

This crate handles loading and traversing finite-state transducer (FST) binaries in the VFST format. It provides both unweighted and weighted traversal algorithms, flag diacritic operations (P/C/U/R/D), symbol table management, and a format abstraction layer. Cherry-picked and adapted from corevoikko's `voikko-fst`.

## Key Types

- `Transducer` trait — abstract FST traversal interface (`prepare` + `next`)
- `format::VfstHeader` — VFST binary header parser
- `symbols::SymbolTable` — character-to-index and index-to-character mapping
- `transition::Transition` — zero-copy transition struct
- `flags::FlagDiacriticOp` — flag diacritic operations
- `config::TraversalConfig` — traversal state stack
- `unweighted` / `weighted` — traversal algorithm implementations
- `VfstError` — error type for VFST parsing failures

## Dependencies

Uses: `mce-core`, `thiserror`, `bytemuck`, `hashbrown`

Used by: `mce-fi`, `mce-speller`, `mce-grammar`, `mce-eval`, `mce-wasm`, `mce-cli`
