# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-03-04

### Added
- CI/CD pipeline: CI (fmt, clippy, test, WASM build), Release (tag-based npm + GitHub Release + Pages), Docs (auto-deploy on main push)
- Performance CI: accuracy regression guard with PR comments (UPOS/Lemma thresholds)
- M1 Trie integration: `load_wordlist()` and `has_wordlist()` WASM API methods (20 → 22 methods)
- Trie-based fuzzy spelling suggestions via `suggest()` (44K words, Levenshtein automaton)
- Dependabot, issue templates, LICENSE (Apache-2.0), CHANGELOG
- UD Finnish-TDT v2.17 as git submodule for reproducible evaluation

### Changed
- `spell_check()` now compound-aware (morph analysis + compound splitting), differentiated from `is_valid_word()` (pure morph only)
- `analyze_sentence()` preserves non-word tokens (punctuation) with `"type"` field — CoNLL-U compliant
- Docs deploy builds fresh WASM automatically (no more stale binaries)
- Upgrade to Rust edition 2024, criterion 0.8, actions/checkout v6

### Fixed
- `suggest()` no longer returns empty array — uses trie fuzzy search with edit-distance fallback
- `generate_verb_form()` docs: 4th parameter is `polarity`, not `number`
- `is_valid_word()` docs: accurately describes pure morphological check
- Gold tokenization footnote added to accuracy metrics
- Remove dead `mce-speller` dependency from WASM crate

## [0.1.0] - 2026-03-03

### Added
- 4-machine architecture: Succinct Trie (M1), Comonadic Engine (M2'), PDT (M3), Weighted Lattice (M4')
- Writer Comonad morphophonological pipeline with DeletionMonoid
- 11 consonant gradation patterns as coKleisli arrows
- CG-lite disambiguation: 62 active rules, 24 rule types, 23 phases
- Suffix Tagger (logistic regression on suffix features): UPOS 95.56%
- Dictionary-enhanced lemmatization: 86.24% accuracy (36K entries)
- Spell checking and suggestion engine
- Grammar checking: 21 rules (258 tests)
- Morphological generation: nouns (11 cases) + verbs (4 conjugation types)
- Compound word analysis via Pushdown Transducer
- Hyphenation support (single-word and full-text)
- WASM bindings: 225KB binary, 22 API methods
- CLI tools: 11 subcommands
- Carbon docs site with 8 interactive demos
- npm package: @yongsk0066/mce@0.1.0
- 1,365 tests, ~41,800 LOC Rust

[Unreleased]: https://github.com/yongsk0066/mce/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/yongsk0066/mce/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/yongsk0066/mce/releases/tag/v0.1.0
