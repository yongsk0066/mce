# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Research docs: irregular verb generation analysis, consonant gradation in generation, suggest algorithm gap, analysis-generation symmetry, verb generation verification
- `docs/research/INDEX.md` -- research document registry with dependency map and update checklist
- Metadata headers on all research documents (status, created, relates-to)
- `gradate_stem()` -- stem-only consonant gradation for generation (fixes kaupunki -> kauvunki false positive)

### Changed
- Tests: 1,553 -> 1,579
- Verb generation accuracy: 47.7% -> 69.9% (improved stem classification and gradation)
- Code deduplication: `is_finnish_vowel` and `levenshtein_distance` consolidated to single canonical locations
- Noun plural generation: improved consonant gradation accuracy via `gradate_stem()`

### Fixed
- Stale documentation sweep across README.md, CLAUDE.md, crate READMEs, docs/index.html (test counts, generation form counts, label formats, removed file references)

## [0.3.0] - 2026-03-04

### Added
- UD Finnish-OOD and UD Finnish-PUD as git submodules for broader lemma coverage
- Multi-source lemma dictionary extraction (`scripts/extract_lemma_dict.py -o` flag)
- THIRD_PARTY_NOTICES.md with CC-BY-SA 4.0 attribution for UD treebanks
- Plural noun generation: `generate_form(base, case, "plural")` and `generate_paradigm()` now returns 22 forms (11 singular + 11 plural)
- `suggest()` routed through SpellChecker pipeline (trie + cache + morph validation) when wordlist loaded
- `spell_check()` now uses SpellChecker pipeline for cache-aware, trie+morph validation
- v0.4.0 research docs: compound improvement plan, Kotus integration plan, long-term roadmap
- Paper-3 SCiL submission prep: OPENREVIEW-SUBMISSION.md with all form fields

### Changed
- Lemma dictionary expanded from 36K to 48K entries (TDT train + OOD + PUD)
- Lemma accuracy improved: 86.24% -> 88.44% on test set (+2.20pp)
- `mce-speller` promoted from transitive to explicit dependency in mce-wasm
- Tests: 1,496 -> 1,532 (34 new: 31 plural generation + 3 SpellChecker integration)

### Fixed
- UPOS accuracy corrected: 95.56% -> 94.58% (previous number included PUNCT/SYM; now uses CoNLL standard excluding PUNCT/SYM)

### Removed
- `data/suffix_tagger.bin.bak` (6.4MB backup file, already in .gitignore)

### Investigated
- Tier 2 feasibility: compound boundary accuracy 80.9% (below 95% threshold) — Wiktionary integration deferred to v0.4.0
- Kotus word list: GO for speller enrichment and verb validation in future release
- Compound improvement: FST hybrid approach identified as most efficient path to 95%+
- Long-term roadmap: Phase 2 (micro transformer, edit-tree lemmatizer), Phase 3 (VS Code, Chrome, Docs)

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
- Suffix Tagger (logistic regression on suffix features): UPOS 95.56% (later corrected to 94.58% excl. PUNCT/SYM)
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

[Unreleased]: https://github.com/yongsk0066/mce/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/yongsk0066/mce/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/yongsk0066/mce/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/yongsk0066/mce/releases/tag/v0.1.0
