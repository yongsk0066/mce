# Research Documents Index

> Last updated: 2026-03-05
> Maintainer: Yongseok Jang

## Research Document Registry

| # | Document | Status | Created | Commit | Summary |
|---|----------|--------|---------|--------|---------|
| 1 | [compound-improvement-plan.md](compound-improvement-plan.md) | active | 2026-03-04 | `37462bf` | Compound boundary accuracy 80.9% to 95%+ plan; FST hybrid approach identified as optimal path |
| 2 | [kotus-integration-plan.md](kotus-integration-plan.md) | active | 2026-03-04 | `37462bf` | Kotus 2024 CSV (100K+ lemmas) integration plan for speller enrichment |
| 3 | [long-term-roadmap.md](long-term-roadmap.md) | active | 2026-03-04 | `37462bf` | Phase 2-4 planning: micro transformer, edit-tree lemmatizer, IDE integration, papers |
| 4 | [irregular-verb-generation.md](irregular-verb-generation.md) | active | 2026-03-05 | `37462bf` | Irregular verb generation failures; recommends exception table (~30 verbs) |
| 5 | [generation-consonant-gradation.md](generation-consonant-gradation.md) | implemented | 2026-03-05 | `37462bf` | Stem-only gradation for generation; `gradate_stem()` implemented in generator.rs |
| 6 | [suggest-algorithm-gap.md](suggest-algorithm-gap.md) | active | 2026-03-05 | `37462bf` | Byte-level Levenshtein inflates edit distance for ä/ö; needs char-level DP |
| 7 | [analysis-generation-symmetry.md](analysis-generation-symmetry.md) | active | 2026-03-05 | `37462bf` | Analysis vs generation pipeline asymmetry; recommends hybrid approach (Option D) |
| 8 | [verb-generation-verification.md](verb-generation-verification.md) | active | 2026-03-05 | `37462bf` | Full-dictionary verb generation verification: 44.3% OK, 55.7% mismatch (mostly NO_ANALYSIS) |
| 9 | [architecture-quality-audit.md](architecture-quality-audit.md) | active | 2026-03-05 | `37462bf` | 11-crate architecture quality audit; grade B+; identifies code duplication and generator workarounds |

### Status Definitions

| Status | Meaning |
|--------|---------|
| **draft** | Work in progress, not yet actionable |
| **active** | Research complete, findings inform future work but not yet implemented |
| **implemented** | Key findings have been implemented in code |
| **superseded** | Replaced by a newer document (link to successor) |

## Document Dependency Map

```
long-term-roadmap.md
  ├── compound-improvement-plan.md  (Phase 1 detail: compound accuracy)
  ├── kotus-integration-plan.md     (Phase 1 detail: speller enrichment)
  └── analysis-generation-symmetry.md (Phase 2 context: FST reverse)

irregular-verb-generation.md
  ├── generation-consonant-gradation.md  (related: gradation in generation)
  ├── verb-generation-verification.md    (empirical validation data)
  └── analysis-generation-symmetry.md    (architectural context)

suggest-algorithm-gap.md
  └── (standalone, affects mce-speller and mce-core trie)
```

## Project Documentation Map

### Root Level

| File | Purpose | Update Frequency |
|------|---------|-----------------|
| `README.md` | Public-facing project overview, metrics, quick start | Every release |
| `CLAUDE.md` | AI context: metrics, architecture, key files | Every session with metric changes |
| `CHANGELOG.md` | Version history with categorized changes | Every release |
| `ARCHITECTURE.md` | 4-machine architecture deep dive | Major architecture changes |
| `CONTRIBUTING.md` | Development setup, PR process | Rarely |
| `SECURITY.md` | Security policy | Rarely |
| `CODE_OF_CONDUCT.md` | Community standards | Rarely |
| `THIRD_PARTY_NOTICES.md` | License attribution for dependencies | When data sources change |
| `LEARNING.md` | Learning notes | As needed |

### Crate READMEs

| Crate | README | Key Content |
|-------|--------|-------------|
| `mce-core` | `crates/mce-core/README.md` | Shared types, M1 Succinct Trie, CompoundAnalyzer |
| `mce-fst` | `crates/mce-fst/README.md` | VFST format, flag diacritics, traversal |
| `mce-tokenizer` | `crates/mce-tokenizer/README.md` | Word/sentence tokenization |
| `mce-speller` | `crates/mce-speller/README.md` | SpellChecker pipeline, trie + morph + cache |
| `mce-comonad` | `crates/mce-comonad/README.md` | Writer Comonad, CG-lite, gradation patterns |
| `mce-disambig` | `crates/mce-disambig/README.md` | Suffix Tagger, Viterbi, disambiguation pipeline |
| `mce-fi` | `crates/mce-fi/README.md` | Finnish module: analysis, generation, compounds, hyphenation |
| `mce-grammar` | `crates/mce-grammar/README.md` | 21 grammar rules |
| `mce-eval` | `crates/mce-eval/README.md` | UD evaluation pipeline, lemma dict |
| `mce-wasm` | `crates/mce-wasm/README.md` | 22 WASM API methods |
| `mce-cli` | `crates/mce-cli/README.md` | 11 CLI subcommands |

### Other Documentation

| File | Purpose |
|------|---------|
| `data/README.md` | Runtime data files description |
| `demo/README.md` | Local demo setup instructions |
| `docs/index.html` | Live demo page (GitHub Pages) |
| `scripts/README.md` | Build/utility scripts |
| `experiments/README.md` | Experimental code |

## Update Checklist

When adding a new feature, update the following documents:

### Metric Changes (accuracy, test count, WASM size, etc.)

- [ ] `README.md` -- Performance table, test count in Quick Start
- [ ] `CLAUDE.md` -- Current Metrics table
- [ ] `CHANGELOG.md` -- Under `[Unreleased]`
- [ ] `docs/index.html` -- Hero stats, feature descriptions (if applicable)
- [ ] `docs/research/long-term-roadmap.md` -- Current State Summary table

### New API Method

- [ ] `README.md` -- WASM API count, Features list, Quick Start example
- [ ] `CLAUDE.md` -- WASM API list
- [ ] `CHANGELOG.md` -- Under `[Unreleased] > Added`
- [ ] `crates/mce-wasm/README.md` -- API table, Usage example
- [ ] `docs/index.html` -- API Reference section, possibly new demo tile

### Generation Changes (forms, labels, verb support)

- [ ] `README.md` -- Features list, "What Only MCE Can Do"
- [ ] `CLAUDE.md` -- Generation row in metrics
- [ ] `CHANGELOG.md` -- Under `[Unreleased]`
- [ ] `crates/mce-fi/README.md` -- Generation section, Noun Cases header
- [ ] `crates/mce-wasm/README.md` -- generate_paradigm description
- [ ] `crates/mce-cli/README.md` -- generate subcommand description
- [ ] `docs/index.html` -- Paradigm Generation tile description, API Reference

### New CG or Grammar Rule

- [ ] `CLAUDE.md` -- CG rules / Grammar rules count
- [ ] `README.md` -- Performance table
- [ ] `CHANGELOG.md`
- [ ] `crates/mce-comonad/README.md` -- CG count
- [ ] `crates/mce-grammar/README.md` -- Rules table

### New Research Document

- [ ] `docs/research/INDEX.md` (this file) -- Add to registry
- [ ] Add metadata header to new document (see template below)

## Research Document Metadata Template

Every research document should begin with a YAML-style metadata header:

```markdown
---
title: [Descriptive Title]
created: [YYYY-MM-DD]
commit: [git short hash at time of writing, e.g., 37462bf]
status: [draft | active | implemented | superseded]
superseded-by: [filename, if status is superseded]
relates-to:
  - [related-doc-1.md]
  - [related-doc-2.md]
---
```

The `commit` field records the project state when the research was conducted,
enabling future readers to understand which codebase version the analysis refers to.
