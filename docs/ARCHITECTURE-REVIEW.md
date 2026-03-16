# MCE Architecture Review Plan

**Created:** 2026-03-16
**Scope:** All 11 crates, ~45,600 LOC Rust + 375 JS tests

---

## 1. Dependency Graph

### Actual DAG

```
mce-core (leaf)
  ├── mce-fst          [+ thiserror, bytemuck, hashbrown]
  ├── mce-tokenizer
  ├── mce-comonad
  ├── mce-disambig
  ├── mce-speller      [+ mce-fst]
  ├── mce-fi           [+ mce-fst, mce-speller, mce-disambig, mce-comonad]
  ├── mce-grammar      [+ mce-fst, mce-fi, mce-tokenizer, mce-disambig]
  ├── mce-eval         [+ mce-fst, mce-fi, mce-disambig, mce-comonad, mce-tokenizer]
  ├── mce-wasm         [+ mce-fst, mce-fi, mce-speller, mce-disambig, mce-comonad,
  │                       mce-tokenizer, mce-grammar, wasm-bindgen]
  └── mce-cli          [+ mce-fst, mce-fi, mce-speller, mce-disambig, mce-comonad,
                          mce-tokenizer, mce-grammar, mce-eval]
```

No circular dependencies. DAG is clean.

### Findings

| ID | Issue | Sev | Location |
|----|-------|-----|----------|
| D-1 | mce-core declares `thiserror` but never uses it | Low | `mce-core/Cargo.toml` |
| D-2 | mce-wasm depends on mce-comonad but only transitively (via mce-fi) | Low | `mce-wasm/Cargo.toml` |
| D-3 | mce-grammar depends on mce-fst solely for `VfstError` re-export | Low | `mce-grammar/src/finnish.rs:61,88` |
| D-4 | mce-fi → mce-disambig coupling for `BigramModel` in spellcheck | Med | `mce-fi/src/spellcheck.rs:28` |

---

## 2. Public API Surface (mce-wasm, 22 methods)

| Group | Methods | Count |
|-------|---------|-------|
| Lifecycle | `load`, `load_model`, `has_model`, `load_wordlist`, `has_wordlist`, `version` | 6 |
| Single-word | `analyze`, `spell_check`, `is_valid_word`, `get_baseform` | 4 |
| Sentence-level | `analyze_sentence`, `disambiguate_sentence` | 2 |
| Suggestions | `suggest`, `suggest_with_context` | 2 |
| Compound | `compound_split` | 1 |
| Grammar | `grammar_check` | 1 |
| Hyphenation | `hyphenate`, `hyphenate_text` | 2 |
| Generation | `generate_form`, `generate_paradigm`, `generate_verb_form`, `generate_verb_paradigm` | 4 |

### Findings

| ID | Issue | Sev | Details |
|----|-------|-----|---------|
| **A-5** | **CG rules NOT applied in WASM pipeline** — browser users get ~83% UPOS, not 94% | **HIGH** | `mce-wasm/src/lib.rs:288-341` vs `mce-eval/src/pipeline.rs` |
| A-4 | Silent failures: 19/22 methods return bare types, no error signaling | Med | All JSON-returning methods |
| A-1 | JSON serialization is hand-rolled everywhere (fragile) | Med | ~15 methods in mce-wasm |
| A-2 | `RefCell` for interior mutability (safe in WASM, not in native) | Low | `mce-wasm/src/lib.rs:78-80` |
| A-3 | `analyze_sentence` and `disambiguate_sentence` overlap | Low | Lines 288, 525 |

---

## 3. Crate Boundary Review

| Crate | LOC | Concerns | Assessment |
|-------|-----|----------|------------|
| mce-core | 3,194 | 8 modules | Clean |
| mce-fst | 1,745 | FST format + traversal | Clean |
| mce-tokenizer | 1,357 | Single tokenizer | Clean |
| mce-comonad | 8,373 | Morphophonology + CG rules | **2 concerns unified by comonad** |
| mce-disambig | 6,513 | Lattice, Viterbi, Bigram, Suffix tagger | Moderate coupling |
| mce-fi | 7,020 | Morphology, Generation, Hyphenation, Compound, Spellcheck | **Largest, 5 concerns** |
| mce-grammar | 6,340 | 21 rules + orchestrator | Well-structured |
| mce-speller | 1,852 | Cache, pipeline, status | Clean |
| mce-eval | 3,113 | CoNLL-U, metrics, pipeline | Tooling only |
| mce-wasm | 2,038 | 22 API methods, monolithic | Could decompose |
| mce-cli | 1,522 | 11 subcommands | Acceptable |

### Findings

| ID | Issue | Sev | Details |
|----|-------|-----|---------|
| B-1 | CG rules (cg.rs, 4,477 LOC) are disambiguation but live in mce-comonad | Med | Justified by comonadic model, but pipeline is split |
| B-2 | mce-fi is a grab bag (11.5K LOC incl. tests) | Low | Spellcheck has its own dep chain |
| B-3 | mce-wasm/src/lib.rs is a 2,038-line monolith | Low | No submodules |

---

## 4. Error Handling

| Crate | Error Type | Implementation |
|-------|-----------|----------------|
| mce-fst | `VfstError` | `thiserror` — 6 variants |
| mce-disambig | `SuffixTaggerError` | Manual `Display`+`Error` — 4 variants |
| mce-core | *(none)* | Has thiserror dep but never uses it |
| mce-wasm | Ad-hoc `JsValue` | `.map_err(\|e\| JsValue::from_str(...))` |
| Others | *(none)* | `Vec<T>` (empty=no results), `Option<T>`, `bool` |

### Findings

| ID | Issue | Sev | Details |
|----|-------|-----|---------|
| E-2 | mce-fi and mce-grammar leak `VfstError` through public APIs | Med | Tight coupling to FST crate |
| E-4 | 176 `.unwrap()` calls — most in tests, some in library code | Med | Audit: zipper.rs(34), writer.rs(30), suffix_tagger.rs(12), generator.rs(11) |
| E-1 | SuffixTaggerError uses manual impls instead of thiserror | Low | `mce-disambig/src/suffix_tagger.rs:458-493` |

---

## 5. Performance

### Critical Path: `analyze_sentence`

```
Text → tokenize → FST analyze/word → [CG prune] → SuffixTagger emit → Viterbi decode → JSON
```

### Findings

| ID | Issue | Sev | Details |
|----|-------|-----|---------|
| P-1 | `RefCell<Config>` in FinnishAnalyzer prevents parallel analysis | Med | `mce-fi/src/morphology.rs:37-38` |
| P-4 | Only 1 criterion benchmark (trie). No pipeline/FST/disambig benchmarks | Med | `mce-core/benches/trie_bench.rs` |
| P-2 | SuffixTagger dot product is well-optimized (INT8, pre-computed) | OK | Already addressed |

---

## 6. Code Health

### Positive
- Zero `unsafe` blocks
- Zero `panic!`/`todo!`/`unimplemented!` in library code
- Only 3 `#[allow(dead_code)]` (documented intent)
- Clippy `-D warnings` enforced
- 2 fuzz targets, 18 proptest

### Findings

| ID | Issue | Sev | Details |
|----|-------|-----|---------|
| H-1 | Unused thiserror in mce-core | Low | `mce-core/Cargo.toml` |
| H-2 | RefCell in library code (2 locations) | Low | WASM-safe, native-risky |

---

## 7. Test Coverage

| Crate | Tests | LOC | Tests/KLOC | Assessment |
|-------|-------|-----|------------|------------|
| mce-core | 109 | 3,194 | 34.1 | Good |
| mce-fst | 43 | 1,745 | 24.6 | **Low — safety-critical** |
| mce-tokenizer | 96 | 1,357 | 70.7 | Excellent |
| mce-comonad | 317 | 8,373 | 37.9 | Good |
| mce-disambig | 190 | 6,513 | 29.2 | Good |
| mce-fi | 405 | 7,020 | 57.7 | Excellent |
| mce-grammar | 261 | 6,340 | 41.2 | Good |
| mce-speller | 79 | 1,852 | 42.7 | Good |
| mce-eval | 127 | 3,113 | 40.8 | Good |
| mce-wasm | 93 | 2,038 | 45.6 | Good (+375 JS) |
| mce-cli | 0 | 1,522 | 0 | None |

### Findings

| ID | Issue | Sev | Details |
|----|-------|-----|---------|
| T-1 | mce-fst lowest density (24.6/KLOC) for safety-critical binary parsing | Med | Only 5 integration tests + 1 fuzz target |
| T-2 | mce-cli has zero tests | Low | 11 subcommands unverified |
| T-4 | No end-to-end CG+Viterbi integration test outside mce-eval | Med | Relates to A-5 |

---

## 8. Documentation

| Crate | `//!` docs | Quality |
|-------|-----------|---------|
| mce-core | 12 lines | Good |
| mce-fst | 15 lines | Excellent |
| mce-tokenizer | 9 lines | Good |
| mce-comonad | 51 lines | Excellent |
| mce-disambig | 67 lines | Excellent |
| mce-fi | 6 lines | **Minimal** |
| mce-grammar | 58 lines | Excellent |
| mce-speller | 3 lines | **Minimal** |
| mce-eval | 36 lines | Good |
| mce-wasm | 37 lines | Good |

---

## 9. Priority Summary

### High — RESOLVED
1. ~~**A-5**: CG rules missing from WASM~~ → **FIXED** (2026-03-16): CG 62 rules integrated into `analyze_sentence()` and `disambiguate_sentence()`

### Medium — RESOLVED
2. ~~**E-2/E-3**: Error type leaking~~ → **FIXED**: `FiError`, `InitError` created with `From<VfstError>`
3. **A-4**: Silent failures → **MITIGATED** by A-5 fix; remaining methods already guard optional components
4. ~~**P-4**: Benchmark gaps~~ → **FIXED**: 3 criterion benchmarks added (CG, Viterbi, full pipeline)
5. ~~**T-1**: mce-fst low test density~~ → **FIXED**: +90 tests (31→121), malformed input fully covered
6. **B-1**: CG in mce-comonad → **KEEP** (Paper-3 narrative); restructure into submodules post-acceptance
7. **D-4**: mce-fi → mce-disambig → **TODO**: refactor suggest_with_context to accept ranking closure

### Low (unchanged)
8. D-1: Remove unused thiserror from mce-core
9. D-2: Remove unnecessary mce-comonad from mce-wasm Cargo.toml
10. E-1: Migrate SuffixTaggerError to thiserror
11. A-1: Structured JSON serialization
12. DOC: Expand mce-fi and mce-speller module docs
13. Q3: Split mce-wasm into modules (json.rs, edit.rs)
14. Q7: Add TypeScript .d.ts type definitions

---

## 10. Review Execution Checklist

- [ ] Verify each finding by reading cited file(s)
- [ ] Assess WASM size impact (420KB budget) for proposed changes
- [ ] Check whether findings affect SCiL 2026 paper claims
- [ ] Determine if fixes require WASM API breaking changes
- [ ] Estimate effort (S/M/L) per item
- [ ] Create tracking issues or add to roadmap
