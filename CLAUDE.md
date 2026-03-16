# MCE -- Morphological Computation Engine

Browser-first Finnish NLP engine. Runs entirely offline in WebAssembly with no server dependency.

## Target Specs

- Deploy size: ~9.2MB (WASM ~395KB + dictionary 3.8MB + model 5.0MB)
- Latency: <5ms per sentence (actual: ~1.35ms)
- Accuracy: UPOS 94%+ (actual: 94.58%)
- Environment: WASM browser (fully offline)

## Current Metrics

| Metric | Value |
|--------|-------|
| UPOS (CG + Suffix Tagger) | **94.66%** (dev) / **94.58%** (test) |
| UPOS (rule-only) | 83.92% |
| Lemma | 93.09% (dev) / 88.44% (test) |
| Coverage | 99.35% |
| Speed | 84,973 tokens/sec (~1.35ms/sentence) |
| WASM binary | ~395KB |
| Deploy (total) | ~9.2MB (gzip: ~2-3MB) |
| CG rules | 62 active (85 total), 24 rule types, 23 phases |
| Grammar rules | 21 (258 tests) |
| Generation | Nouns: 22 forms (11 sg + 11 pl), Verbs: 4 conjugation types |
| Tests | 1,619 passed (+ 375 JS, 18 proptest, 2 fuzz targets) |
| LOC | ~45,600 Rust |

## Architecture: MCE v3 (4 Machines)

| Machine | Role | Mathematical Basis | Crate |
|---------|------|--------------------|-------|
| M1: Succinct Trie | Dictionary lookup / spell checking | LOUDS encoding | `mce-core` (trie module) |
| M2': Comonadic Engine | Morphological analysis + morphophonological rules | Writer Comonad (extend/extract, DeletionMonoid) | `mce-comonad` |
| M3: PDT | Compound word structure analysis | Pushdown Transducer | `mce-fst` |
| M4': Weighted Lattice | POS disambiguation | Viterbi + CG-lite + Suffix Tagger | `mce-disambig` |

### Writer Comonad (M2')

All Finnish morphophonological rules are expressed as pure coKleisli arrows composing over a `Writer Comonad` with a `DeletionMonoid`. This eliminates mutation and sentinel characters from the pipeline.

- 11 consonant gradation patterns as coKleisli arrows
- Vowel harmony via coKleisli composition
- Boundary effects handled by `WriterZipper` context
- Implementation: `mce-comonad/src/writer.rs` (980 LOC)
- CG-lite rules: `mce-comonad/src/cg.rs` (62 active rules, 24 types, 23 phases)

### Suffix Tagger (M4')

Statistical POS tagger using logistic regression on suffix features. Trained on UD Finnish-TDT treebank.

- Model: 5.0MB binary (MCET format v1)
- Implementation: `mce-disambig/src/suffix_tagger.rs` (1,480 LOC)
- Pipeline: CG-lite -> Suffix Tagger -> Viterbi
- Accuracy boost: 82.71% -> 94.58% (+11.87pp)

## Crate Structure

```plaintext
crates/
├── mce-core/       # Shared types, character classification, M1 Succinct Trie (LOUDS)
├── mce-fst/        # FST engine (format abstraction, VFST traversal, flag diacritics)
├── mce-tokenizer/  # Text tokenizer (words, sentences, URLs, emails)
├── mce-speller/    # Spell checking and suggestion engine
├── mce-disambig/   # M4' Disambiguation (Viterbi + CG-lite + Suffix Tagger)
├── mce-comonad/    # M2' Comonadic morphophonological engine + CG rules
├── mce-fi/         # Finnish language module (analysis, generation, compounds, hyphenation)
├── mce-grammar/    # Grammar checking (21 rules)
├── mce-eval/       # UPOS/Lemma evaluation against UD treebanks
├── mce-wasm/       # WASM bindings (22 API methods)
└── mce-cli/        # CLI tools (11 subcommands)
```

## Key Files

| Purpose | File |
|---------|------|
| Writer Comonad | `mce-comonad/src/writer.rs` (980 LOC) |
| CG-lite rules | `mce-comonad/src/cg.rs` (62 active / 85 total) |
| Suffix Tagger | `mce-disambig/src/suffix_tagger.rs` (1,480 LOC) |
| Finnish morphophonology | `mce-comonad/src/finnish.rs` (Writer pipeline default) |
| Morphological generation | `mce-fi/src/generator.rs` (nouns 22 forms + verbs 4 types) |
| Grammar rules | `mce-grammar/src/rules/` (21 rules) |
| POS mapping | `mce-eval/src/pos_map.rs` |
| Eval pipeline | `mce-eval/src/pipeline.rs` (CG + SuffixTagger + Viterbi) |
| WASM API | `mce-wasm/src/lib.rs` (22 methods) |
| Trained model | `data/suffix_tagger.bin` (5.0MB, MCET format v1) |
| Per-rule benchmarks | `mce-comonad/src/bench.rs` (25 coKleisli + 21 CG) |

## Related Projects

- **Research documents**: `~/oss/finnishNLP/mce-research/` (architecture, math exploration, paper strategy)
- **Parent project**: `~/oss/corevoikko/` (Voikko Rust+WASM rewrite)
- **Reference NLP**: `~/oss/finnishNLP/` (Omorfi, Trankit, UralicNLP, TNPP)

## Build and Verification

Use the justfile task runner (preferred):

```bash
just              # Full pre-commit: fmt + clippy + test + audit
just test-all     # Include #[ignore] integration tests (needs data/)
just wasm-size    # Build WASM + check 420KB budget
just js-test      # WASM + JS integration tests (375 tests)
just eval         # Accuracy evaluation on dev set
```

Or run manually:

```bash
cargo fmt --all --check
cargo test --all-features
cargo clippy --all-features -- -D warnings
cargo audit
```

## Development Process

- **Task runner**: `justfile` (27 recipes, run `just --list`)
- **Pre-commit hooks**: `lefthook` v2.1.2 (`lefthook install`)
- **CI**: 7 workflows, `done` job as single required check
- **Release**: `release/vX.Y.Z` branch → PR → auto-tag → npm publish
- **Version**: workspace-unified (`version.workspace = true`, all crates)
- **Bump**: `scripts/bump-version.sh X.Y.Z`
- See `CONTRIBUTING.md` for full guide

## WASM API (22 methods)

```plaintext
MceEngine.load(dict)              # Load dictionary, create engine
MceEngine.load_model(data)        # Load suffix tagger model
MceEngine.has_model()             # Check if model is loaded
MceEngine.load_wordlist(data)     # Load wordlist for spelling suggestions
MceEngine.has_wordlist()          # Check if wordlist is loaded
MceEngine.analyze(word)           # Single-word morphological analysis
MceEngine.spell_check(word)       # Spell check
MceEngine.suggest(word, max)      # Spelling suggestions
MceEngine.suggest_with_context()  # Context-aware suggestions
MceEngine.analyze_sentence(text)  # Sentence analysis with disambiguation
MceEngine.disambiguate_sentence() # POS disambiguation only
MceEngine.compound_split(word)    # Compound word splitting
MceEngine.grammar_check(text)     # Grammar checking
MceEngine.hyphenate(word)         # Single-word hyphenation
MceEngine.hyphenate_text(text)    # Full-text hyphenation
MceEngine.get_baseform(word)      # Get base form (lemma)
MceEngine.is_valid_word(word)     # Dictionary lookup
MceEngine.generate_form()         # Generate noun case form
MceEngine.generate_paradigm()     # Full noun paradigm
MceEngine.generate_verb_form()    # Generate verb conjugation
MceEngine.generate_verb_paradigm()# Full verb paradigm
MceEngine.version()               # Engine version string
```

## Cherry-pick Origins

~25% of code cherry-picked and adapted from corevoikko:

| MCE crate | corevoikko source | Content |
|-----------|-------------------|---------|
| `mce-core` | `voikko-core` | Analysis, Token, Character, Case types |
| `mce-fst` | `voikko-fst` | FST traversal algorithms, flag diacritics |
| `mce-tokenizer` | `voikko-fi/tokenizer` | URL/email/word/sentence tokenizer |
| `mce-speller` | `voikko-fi/speller+suggestion` | Cache, status, SpellResult, Speller trait |

## Paper Status

Three papers based on MCE research:

| Paper | Target | Status |
|-------|--------|--------|
| Paper-3: Comonadic Morphophonology | SCiL 2026 | Submitted ([#14](https://openreview.net/forum?id=FYeH1Fiwx6)), review pending |
| Paper-2: Morphological Fingerprint | EMNLP ARR May 2026 | ~85% complete |
| Paper-5: Comonadic Classification | TACL / ACL 2027 | Research complete, impl pending |

Research documents and paper drafts are in `~/oss/finnishNLP/mce-research/`.
