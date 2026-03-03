# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- CI/CD pipeline (GitHub Actions)
- CHANGELOG.md
- M1 Trie integration: `load_wordlist()` and `has_wordlist()` WASM API methods (20 -> 22 methods)
- Trie-based fuzzy spelling suggestions via `suggest()` when wordlist is loaded
- Demo site now loads wordlist.txt for better spelling suggestions

### Changed
- Differentiate `spell_check()` (compound-aware) from `is_valid_word()` (pure morphological analysis)
- Remove `mce-speller` dependency from `mce-wasm` (spell checking now handled by `mce-fi`; `mce-speller` still used by `mce-fi` and `mce-cli`)

### Fixed
- `generate_verb_form()` API docs: corrected signature from `(baseform, tense, person, number)` to `(baseform, tense, person, polarity)`
- `is_valid_word()` API docs: corrected description to reflect actual behavior (VFST dictionary check)
- `suggest()` API docs: corrected description to mention trie-based fuzzy search and wordlist dependency
- Added gold tokenization footnote to accuracy metrics in README and docs
- Updated WASM method count from 20 to 22 across all documentation
- `analyze_sentence()` now preserves non-word tokens (punctuation, numbers) in output

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

[Unreleased]: https://github.com/yongsk0066/mce/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yongsk0066/mce/releases/tag/v0.1.0
