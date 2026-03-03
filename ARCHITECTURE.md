# Architecture

MCE (Morphological Computation Engine) is a Rust workspace of 11 crates that implements Finnish morphological analysis, spell checking, grammar checking, hyphenation, and disambiguation. It compiles to a ~225KB WASM module that runs entirely in the browser with no server, targeting <5ms per sentence at 95.56% UPOS accuracy.

## Design Rationale

### Why 11 Crates?

The 11-crate structure follows from five principles that are each independently necessary. Collapsing to fewer crates violates at least one of them.

**1. Compilation target isolation.** The WASM build (`mce-wasm`) pulls only the crates that ship to the browser. The evaluation harness (`mce-eval`) and CLI (`mce-cli`) depend on `std::fs`, test data loading, and reporting logic that must never appear in a WASM binary. Separate crates enforce this at the type level -- a `use mce_eval::*` in `mce-wasm` is a compile error, not a runtime oversight.

**2. Zero-dependency foundation.** `mce-core` has no internal MCE dependencies and only one external dependency (`thiserror`). Every other crate depends on it. This means core types (`Analysis`, `Token`, `CaseType`, LOUDS trie) can be trusted to compile anywhere without pulling in FST parsers, WASM bindings, or statistical models.

**3. One NLP capability per crate.** Each crate maps to a single NLP concern: `mce-speller` does spell checking, `mce-grammar` does grammar checking, `mce-tokenizer` does tokenization, and so on. This makes it possible to understand, test, and review each capability in isolation. Grammar rule changes never risk breaking the speller; tokenizer refactors never touch disambiguation.

**4. Mathematical boundary.** The comonadic engine (`mce-comonad`) encapsulates all Writer Comonad machinery -- Zipper, WriterZipper, DeletionSet, coKleisli arrows, and CG rules. The statistical engine (`mce-disambig`) encapsulates Viterbi decoding, emission priors, and the suffix tagger. These are fundamentally different computational models (algebraic vs. probabilistic), and mixing them in one module would obscure the architecture's mathematical structure.

**5. Cherry-pick alignment.** Roughly 25% of MCE code was cherry-picked from corevoikko. The crate boundaries align with the source boundaries: `voikko-core` maps to `mce-core`, `voikko-fst` maps to `mce-fst`, `voikko-fi/tokenizer` maps to `mce-tokenizer`, and `voikko-fi/speller+suggestion` maps to `mce-speller`. Code that was written from scratch (`mce-comonad`, `mce-disambig`, `mce-grammar`, `mce-eval`) occupies separate crates with no corevoikko ancestry, keeping provenance clear.

### Why This Dependency Graph

Each dependency edge exists for a specific architectural reason:

| Edge | Why it exists |
|------|---------------|
| `mce-fst` -> `mce-core` | FST traversal produces `Analysis` structs and uses `Token` types defined in core. |
| `mce-tokenizer` -> `mce-core` | Tokenizer emits `Token` and `Sentence` types from core. |
| `mce-comonad` -> `mce-core` | coKleisli arrows operate on `Analysis` and character types from core. CG rules filter `Analysis` readings. |
| `mce-disambig` -> `mce-core` | Viterbi decoder and suffix tagger score `Analysis` candidates from core. |
| `mce-speller` -> `mce-core`, `mce-fst` | Spell checking needs dictionary lookup (FST traversal) and returns `SpellResult` types (core). |
| `mce-fi` -> `mce-core`, `mce-fst`, `mce-speller`, `mce-disambig`, `mce-comonad` | The Finnish module orchestrates all engines: FST for word lookup, comonad for morphophonological rules, speller for spelling, and disambig for POS tagging. This is the integration point for Finnish-specific logic. |
| `mce-grammar` -> `mce-core`, `mce-fst`, `mce-fi`, `mce-tokenizer`, `mce-disambig` | Grammar checking needs tokenized sentences (tokenizer), analyzed words (fi), disambiguated POS tags (disambig), and dictionary access (fst). |
| `mce-eval` -> `mce-core`, `mce-fst`, `mce-fi`, `mce-disambig`, `mce-comonad`, `mce-tokenizer` | Evaluation runs the full analysis pipeline against UD treebank gold data. Needs every analysis-stage crate. |
| `mce-wasm` -> (7 crates) | WASM bindings expose all user-facing features: analysis, spelling, grammar, hyphenation, disambiguation, generation. |
| `mce-cli` -> (all crates) | CLI provides interactive access to every feature plus evaluation. |

Note the edges that do *not* exist:

- `mce-comonad` does not depend on `mce-fst`. Morphophonological rules are pure character-level transformations independent of FST format.
- `mce-disambig` does not depend on `mce-fst` or `mce-comonad`. Disambiguation is format-agnostic and works on any `Analysis` input.
- `mce-speller` does not depend on `mce-comonad` or `mce-disambig`. Spell checking is a dictionary-only operation.
- `mce-wasm` does not depend on `mce-eval`. Evaluation code never ships to the browser.

### What Each Crate Does and Why It Is Separate

**`mce-core`** (~3,000 LOC) -- Shared types (`Analysis`, `Token`, `CaseType`), Unicode character classification, and the LOUDS succinct trie (Machine M1). Cannot be merged into any other crate because every crate depends on it; it must remain dependency-free to avoid cycles.

**`mce-fst`** (~1,700 LOC) -- VFST binary format parser, DFS transducer traversal, and flag diacritic evaluation. Separated from `mce-core` because it introduces external dependencies (`bytemuck`, `hashbrown`) for zero-copy parsing, and not all consumers need FST machinery (e.g., `mce-disambig` and `mce-tokenizer` do not).

**`mce-tokenizer`** (~1,400 LOC) -- Splits raw text into word, punctuation, URL, email, and sentence boundary tokens. Separated because tokenization has no dependency on FST, morphology, or disambiguation -- it is a pure text-processing stage.

**`mce-comonad`** (~8,400 LOC) -- Writer Comonad engine: `Zipper`, `WriterZipper`, `DeletionSet`, coKleisli arrows for consonant gradation (11 patterns) and vowel harmony, plus 62 active CG rules. This is the mathematical core of the project. It cannot merge into `mce-fi` because it contains language-agnostic comonadic abstractions and CG rule types that could serve other agglutinative languages. It cannot merge into `mce-disambig` because it implements algebraic (deterministic) reasoning, not statistical inference.

**`mce-disambig`** (~5,800 LOC) -- Viterbi decoder with POS bigram transitions, emission priors, suffix-based statistical tagger (95.56% UPOS), and CG-lite integration. Cannot merge into `mce-comonad` because it is inherently probabilistic (trained model, feature weights), while the comonad is algebraic (laws, composition). Cannot merge into `mce-fi` because disambiguation logic is language-independent.

**`mce-speller`** (~1,900 LOC) -- Spell checking and suggestion generation with edit-distance ranking and priority-queue candidate collection. Separated from `mce-fi` because spelling is a self-contained feature with its own FST traversal patterns (fuzzy matching), distinct from morphological analysis.

**`mce-fi`** (~7,100 LOC) -- Finnish language module: morphological analyzer wrapping FST output, compound word analysis with 6 linking morphemes, hyphenation, and morphological generation (11 noun cases, 4 verb types). This is the only language-specific crate; all others are language-agnostic in principle. It cannot absorb `mce-comonad` or `mce-disambig` because those contain reusable abstractions.

**`mce-grammar`** (~6,400 LOC) -- 21 grammar rules for Finnish writing errors (repeated words, case agreement, punctuation, etc.) with context-sensitive paragraph analysis. Separated from `mce-fi` because grammar checking requires sentence-level context (tokenizer + disambiguator) while `mce-fi` operates at the word level.

**`mce-eval`** (~2,700 LOC) -- Evaluation harness for UPOS and lemma accuracy against UD treebanks (Finnish-TDT). Must be separate because it depends on filesystem I/O and test data loading that must never compile into WASM.

**`mce-wasm`** (~2,000 LOC) -- 20 JavaScript API methods via `wasm-bindgen`. Thin binding layer that translates between JS types and Rust types. Must be separate because it is the only `cdylib` crate and depends on `wasm-bindgen`, `js-sys`, and `serde-wasm-bindgen` -- dependencies irrelevant to native builds.

**`mce-cli`** (~1,500 LOC) -- 11 CLI subcommands for interactive analysis, evaluation, and debugging. Separated because it depends on all other crates (including `mce-eval`) and is a native-only binary target.

### Data Flow: Tracing "Koirat juoksevat nopeasti."

A complete sentence analysis passes through all four machines, handled by specific crates at each step:

```plaintext
Input: "Koirat juoksevat nopeasti."

Step 1: TOKENIZATION (mce-tokenizer)
  Split into tokens: ["Koirat", "juoksevat", "nopeasti", "."]
  Identify sentence boundary at "."

Step 2: FST TRAVERSAL per token (mce-fst, mce-fi)
  "Koirat"     -> mce-fst runs VFST transducer
                -> mce-fi wraps results into Analysis structs:
                   [{CLASS=nimisana, BASEFORM=koira, NUMBER=plural, SIJAMUOTO=nimento},
                    {CLASS=nimisana, BASEFORM=koira, NUMBER=singular, SIJAMUOTO=kohdanto},
                    ...]
  "juoksevat"  -> [{CLASS=teonsana, BASEFORM=juosta, PERSON=3, NUMBER=plural, AIKAMUOTO=present},
                    ...]
  "nopeasti"   -> [{CLASS=seikkasana, BASEFORM=nopeasti}, ...]
  "."          -> [{CLASS=merkki}]

Step 3: COMONADIC RULES (mce-comonad)
  For generation and morphophonological validation:
  - WriterZipper wraps each morpheme sequence
  - coKleisli arrows apply consonant gradation, vowel harmony
  - DeletionSet accumulates position-stable deletion marks
  - extend(gradation) . extend(harmony) composes without intermediate materialization
  For CG disambiguation:
  - 62 active CG rules prune impossible readings based on context
  - e.g., if token[i-1] is a preposition, remove ADV reading from token[i]
  - "Koirat": CG selects NOM.PL over ACC.SG (sentence-initial, no governing verb)

Step 4: DISAMBIGUATION (mce-disambig)
  Input: remaining candidate readings after CG pruning
  Suffix tagger computes P(UPOS | word_suffix) emission scores
  Viterbi decoder finds optimal POS sequence using bigram transitions:
    "Koirat"=NOUN  "juoksevat"=VERB  "nopeasti"=ADV  "."=PUNCT
  Result: 1-best analysis per token (95.56% UPOS accuracy with suffix tagger)

Step 5: GRAMMAR CHECK (mce-grammar, optional)
  Scans disambiguated sentence for errors using 21 rules
  No errors found in this sentence

Output: Structured JSON with disambiguated POS, lemma, morphological features
```

## Crate Map

```mermaid
graph TD
    subgraph Foundation
        core[mce-core]
    end

    subgraph Engines
        fst[mce-fst]
        tokenizer[mce-tokenizer]
        comonad[mce-comonad]
        disambig[mce-disambig]
        speller[mce-speller]
    end

    subgraph Language
        fi[mce-fi]
        grammar[mce-grammar]
    end

    subgraph Evaluation
        eval[mce-eval]
    end

    subgraph Interfaces
        wasm[mce-wasm]
        cli[mce-cli]
    end

    core --> fst
    core --> tokenizer
    core --> comonad
    core --> disambig
    core --> speller
    fst --> speller
    core --> fi
    fst --> fi
    speller --> fi
    disambig --> fi
    comonad --> fi
    core --> grammar
    fst --> grammar
    fi --> grammar
    tokenizer --> grammar
    disambig --> grammar
    core --> eval
    fst --> eval
    fi --> eval
    disambig --> eval
    comonad --> eval
    tokenizer --> eval
    core --> wasm
    fst --> wasm
    fi --> wasm
    speller --> wasm
    disambig --> wasm
    comonad --> wasm
    tokenizer --> wasm
    grammar --> wasm
    core --> cli
    fst --> cli
    tokenizer --> cli
    speller --> cli
    disambig --> cli
    comonad --> cli
    fi --> cli
    grammar --> cli
    eval --> cli
```

| Crate | Role | LOC | Dependencies |
|-------|------|----:|-------------|
| `mce-core` | Shared types: `Analysis`, `Token`, character classification, LOUDS succinct trie | ~3,000 | (none) |
| `mce-fst` | FST engine: VFST format parser, flag diacritics, transducer traversal | ~1,700 | `mce-core` |
| `mce-tokenizer` | Text tokenizer: word, URL, email, sentence boundary detection | ~1,400 | `mce-core` |
| `mce-comonad` | Writer Comonad engine: `Zipper`, `WriterZipper`, `DeletionSet`, coKleisli arrows, CG rules, Finnish morphophonology (vowel harmony, consonant gradation, allomorph selection) | ~8,400 | `mce-core` |
| `mce-disambig` | Disambiguation: Viterbi decoder, emission priors, suffix tagger (95.56% UPOS), CG-lite (62 active rules) | ~5,800 | `mce-core` |
| `mce-speller` | Spell checking and suggestion generation with edit-distance ranking | ~1,900 | `mce-core`, `mce-fst` |
| `mce-fi` | Finnish language module: morphological analyzer, hyphenator, compound analysis | ~7,100 | `mce-core`, `mce-fst`, `mce-speller`, `mce-disambig`, `mce-comonad` |
| `mce-grammar` | Grammar checker: 21 Finnish grammar rules with context-sensitive paragraph analysis | ~6,400 | `mce-core`, `mce-fst`, `mce-fi`, `mce-tokenizer`, `mce-disambig` |
| `mce-eval` | Evaluation harness: UPOS accuracy against UD treebanks (Finnish-TDT) | ~2,700 | `mce-core`, `mce-fst`, `mce-fi`, `mce-disambig`, `mce-comonad`, `mce-tokenizer` |
| `mce-wasm` | WASM bindings: 20 JavaScript API methods via `wasm-bindgen` | ~2,000 | `mce-core`, `mce-fst`, `mce-fi`, `mce-speller`, `mce-disambig`, `mce-comonad`, `mce-tokenizer`, `mce-grammar` |
| `mce-cli` | CLI tools for interactive analysis, evaluation, and debugging | ~1,500 | all crates |

Total: ~41,800 lines of Rust, 1,365 tests passed.

## Pipeline Architecture (MCE v3)

MCE v3 organizes computation into four machines, each with a distinct mathematical basis:

```mermaid
flowchart LR
    subgraph M1["M1: Succinct Trie"]
        direction TB
        m1a[LOUDS encoding]
        m1b[Dictionary lookup]
        m1c[Spelling check]
    end

    subgraph M2["M2': Comonadic Engine"]
        direction TB
        m2a["Zipper (focus + context)"]
        m2b[coKleisli arrows]
        m2c["extend = global transform"]
        m2d["Writer Comonad (DeletionSet)"]
    end

    subgraph M3["M3: PDT Compound"]
        direction TB
        m3a[Pushdown Transducer]
        m3b[Compound word parsing]
        m3c[Stack-based structure]
    end

    subgraph M4["M4': Weighted Lattice"]
        direction TB
        m4a[Viterbi decoder]
        m4b[Emission priors]
        m4c[Suffix tagger]
        m4d["CG-lite (57 rules)"]
    end

    M1 --> M2 --> M3 --> M4
```

| Machine | Crate | Math basis | Function |
|---------|-------|-----------|----------|
| **M1: Succinct Trie** | `mce-core` (trie module) | LOUDS encoding | Dictionary lookup, spell checking. O(n) lookup, O(k) fuzzy match. |
| **M2': Comonadic Engine** | `mce-comonad` | Writer Comonad (`extend`/`extract`) | Morphophonological rules as composable coKleisli arrows. Consonant gradation (11 patterns), vowel harmony, allomorph selection, CG-lite rules. |
| **M3: PDT** | `mce-fst` | Pushdown Transducer | Compound word structure analysis. Context-free decomposition of Finnish compounds (e.g., `rautatieasema` -> `rauta+tie+asema`). |
| **M4': Weighted Lattice** | `mce-disambig` | Viterbi + Emission Priors | 1-best disambiguation. POS bigram model + suffix tagger emissions. CG-lite (62 active rules) pre-filters candidates. Rule-only: 82.71% UPOS; with suffix tagger: 95.56% UPOS. |

## Data Flow

```mermaid
flowchart TD
    input["Raw text input"]
    tok["Tokenizer<br/>(mce-tokenizer)"]
    fst["FST Traversal<br/>(mce-fst)"]
    morph["Morphological Analysis<br/>(mce-fi)"]
    comonad["Comonadic Rules<br/>(mce-comonad)"]
    disambig["Disambiguation<br/>(mce-disambig)"]
    grammar["Grammar Check<br/>(mce-grammar)"]
    output["Structured output<br/>(JSON / CoNLL-U)"]

    input --> tok
    tok -->|"word tokens"| fst
    fst -->|"FST analyses<br/>(multiple candidates)"| morph
    morph -->|"Analysis[]<br/>per token"| comonad
    comonad -->|"coKleisli composed<br/>rules applied"| disambig
    disambig -->|"1-best POS<br/>per token"| grammar
    grammar -->|"error annotations"| output
    disambig -->|"disambiguated<br/>analyses"| output

    subgraph "Per-word path"
        fst
        morph
    end

    subgraph "Sentence-level path"
        comonad
        disambig
    end
```

**Step-by-step**:

1. **Tokenize** -- `mce-tokenizer` splits raw text into word, punctuation, URL, and sentence boundary tokens.
2. **FST Traverse** -- `mce-fst` runs the VFST transducer over each word token, producing all valid morphological decompositions.
3. **Analyze** -- `mce-fi` wraps FST output into structured `Analysis` objects with attributes (CLASS, BASEFORM, STRUCTURE, etc.).
4. **Comonadic Rules** -- `mce-comonad` applies morphophonological transformations as coKleisli arrows: consonant gradation, vowel harmony, allomorph selection. The Writer Comonad accumulates deletion marks algebraically.
5. **Disambiguate** -- `mce-disambig` selects the best analysis per token using Viterbi decoding with POS bigram transitions, emission priors, CG-lite constraint rules (62 active), and optionally the suffix tagger.
6. **Grammar Check** -- `mce-grammar` scans the disambiguated sentence for grammar errors (21 rules).

## WASM Deployment

```mermaid
flowchart LR
    subgraph Browser
        app[Web Application]
        idb[(IndexedDB<br/>cache)]
        wasm[WASM Module<br/>~225KB]
        dict[Dictionary<br/>mor.vfst ~8MB]
        model["Suffix Tagger<br/>model.mcet ~1MB"]
    end

    subgraph CDN / Server
        cdn_wasm[mce_wasm.js + .wasm]
        cdn_dict[mor.vfst]
        cdn_model[model.mcet]
    end

    cdn_wasm -->|"fetch once"| wasm
    cdn_dict -->|"fetch once"| idb
    cdn_model -->|"fetch once (optional)"| idb
    idb --> dict
    idb --> model
    dict -->|"load bytes"| wasm
    model -->|"load_model()"| wasm
    app -->|"analyze() / spell_check() / ..."| wasm
    wasm -->|"JSON results"| app
```

**Deployment sizes** (gzip):

| Asset | Raw | Gzip |
|-------|----:|-----:|
| WASM module | ~225KB | ~100KB |
| Dictionary (mor.vfst) | ~8MB | ~1MB |
| Suffix tagger (model.mcet) | ~5MB | ~1MB |
| **Total** | **~13MB** | **~2.1MB** |

**Loading strategy**:

1. Browser fetches `mce_wasm.js` + `.wasm` from CDN.
2. Dictionary and model are fetched once, cached in IndexedDB.
3. On subsequent visits, dictionary and model load from cache (zero network).
4. `MceEngine.load(dictBytes)` initializes the engine.
5. `engine.load_model(modelBytes)` optionally enables the suffix tagger (82.71% -> 95.56% UPOS).
6. All 20 API methods (`analyze`, `spell_check`, `suggest`, `hyphenate`, `grammar_check`, `analyze_sentence`, etc.) run synchronously in the main thread or a Web Worker.

## Mathematical Foundation: Writer Comonad

The central mathematical contribution is using the **Writer Comonad** to achieve pure coKleisli composition for morphophonological rules that involve character deletion.

```mermaid
flowchart TD
    subgraph "Writer Comonad = DeletionSet x Zipper"
        zipper["Zipper&lt;char&gt;<br/>[...left] focus [...right]"]
        log["DeletionSet<br/>(BTreeSet&lt;usize&gt;)"]
    end

    subgraph "coKleisli Arrow"
        arrow["f: &WriterZipper → (DeletionSet, char)<br/>e.g. consonant_gradation"]
    end

    subgraph "extend"
        ext["extend(f):<br/>apply f at every position,<br/>combine all DeletionSets"]
    end

    subgraph "Composition"
        comp["extend(f) . extend(g) . extend(h)<br/>= pure coKleisli composition<br/>(no intermediate filtering)"]
    end

    subgraph "Materialize"
        mat["Apply accumulated DeletionSet<br/>once at the end"]
    end

    zipper --> arrow
    log --> arrow
    arrow --> ext
    ext --> comp
    comp --> mat
```

**Why it matters**:

Without the Writer Comonad, consonant gradation (e.g., `pp` -> `p`, deleting one character) requires inserting `'\0'` sentinel characters, then filtering between pipeline steps. This breaks coKleisli composition because intermediate null characters shift positions for subsequent rules.

With `WriterZipper<DeletionSet, char>`:
- Each coKleisli arrow returns `(DeletionSet::singleton(pos), original_char)` for deletions.
- `extend` combines all deletion sets via set union (the monoidal operation).
- Deletions are applied once at the end -- positions stay stable throughout the pipeline.
- The comonad laws (identity and associativity) hold without qualification.

```plaintext
gradation : &WriterZipper<DeletionSet, char> -> (DeletionSet, char)
harmony   : &WriterZipper<DeletionSet, char> -> (DeletionSet, char)
possessive: &WriterZipper<DeletionSet, char> -> (DeletionSet, char)

pipeline = extend(gradation)
         . extend(harmony)
         . extend(possessive)
         -- pure composition, no intermediate materialization
```

## Performance Characteristics

| Metric | Value | Notes |
|--------|------:|-------|
| UPOS accuracy (rule-only) | 82.71% | CG-lite rules + Viterbi |
| UPOS accuracy (+ suffix tagger) | 95.56% | CG + Viterbi + emission model |
| Lemmatization accuracy | 86.24% | FST-based |
| Dictionary coverage | 99.64% | Finnish-TDT test set |
| Throughput | 42,000 tok/s | Native, single-threaded |
| Latency target | <5ms/sentence | ~20 tokens average sentence |
| WASM binary | ~225KB | `opt-level = "z"`, LTO, `panic = "abort"` |
| CG rules | 62 active / 85 total | coKleisli arrows in `mce-comonad` |
| Grammar rules | 21 | Context-sensitive paragraph rules |
| Morphological generation | 11 noun cases + 4 verb types | coKleisli composition |
| Test count | 1,365 | `cargo test --all-features` |

Build profile (`Cargo.toml`):
```toml
[profile.release]
opt-level = "z"      # optimize for size
lto = true           # link-time optimization
codegen-units = 1    # single codegen unit for better optimization
strip = true         # strip debug symbols
panic = "abort"      # no unwinding overhead
```

## ASCII Diagrams (Terminal-Friendly)

For terminal environments where mermaid rendering is unavailable, here are ASCII equivalents of key diagrams.

### Crate Dependency Graph

```plaintext
                           ┌─────────┐
                           │mce-core │
                           └────┬────┘
            ┌──────┬───────┬────┼────┬────────┬──────────┐
            │      │       │    │    │        │          │
            ▼      ▼       ▼    ▼    ▼        ▼          ▼
        ┌──────┐┌─────┐┌─────┐┌──────┐┌────────┐┌─────────┐
        │ fst  ││ tok ││como ││disam ││speller ││  (fi)   │
        └──┬───┘└──┬──┘└──┬──┘└──┬───┘└───┬────┘└────┬────┘
           │       │      │      │        │          │
           ├───────┼──────┼──────┼────────┘          │
           │       │      │      │                   │
           ▼       │      ▼      ▼                   │
        ┌──────────┼──────────────────┐              │
        │  mce-fi  │                  │◄─────────────┘
        └────┬─────┘                  │
             │                        │
     ┌───────┼────────────────────────┤
     │       │                        │
     ▼       ▼                        ▼
 ┌────────┐┌──────┐              ┌────────┐
 │grammar ││ eval │              │  wasm  │
 └────────┘└──────┘              └────────┘
                                      │
                                 (all above)
                                      │
                                 ┌────────┐
                                 │  cli   │
                                 └────────┘
```

### MCE v3 Pipeline

```plaintext
 ┌─────────────────┐   ┌──────────────────────┐   ┌──────────┐   ┌─────────────────┐
 │  M1: Succinct    │   │  M2': Comonadic       │   │  M3: PDT  │   │ M4': Weighted    │
 │      Trie        │──▶│       Engine           │──▶│ Compound  │──▶│     Lattice      │
 │                  │   │                        │   │           │   │                  │
 │  LOUDS encoding  │   │  Zipper + extend       │   │  Pushdown │   │  Viterbi +       │
 │  Dictionary O(n) │   │  coKleisli arrows      │   │  Stack    │   │  Suffix Tagger   │
 │  Fuzzy O(k)      │   │  Writer(DeletionSet)   │   │  O(n*k)   │   │  CG-lite (62)    │
 └─────────────────┘   └──────────────────────┘   └──────────┘   └─────────────────┘
```

### Analysis Data Flow

```plaintext
 "Koira juoksee nopeasti."
         │
         ▼
 ┌───────────────┐
 │  Tokenizer    │  word / punct / URL / sentence boundaries
 └───────┬───────┘
         │  ["Koira", "juoksee", "nopeasti", "."]
         ▼
 ┌───────────────┐
 │  FST Traverse │  VFST transducer → all valid decompositions
 └───────┬───────┘
         │  Koira → [{nimisana, koira, NOM.SG}, {nimisana, koira, NOM.PL}, ...]
         ▼
 ┌───────────────┐
 │  Comonadic    │  extend(gradation) . extend(harmony) . extend(...)
 │  Rules        │  Writer Comonad accumulates deletions
 └───────┬───────┘
         │
         ▼
 ┌───────────────┐
 │  Disambig     │  Viterbi 1-best: NOUN VERB ADV PUNCT
 │  (M4')        │  82.71% rule-only → 95.56% with suffix tagger
 └───────┬───────┘
         │
         ▼
 ┌───────────────┐
 │  Grammar      │  21 rules: repeated words, case agreement, ...
 └───────┬───────┘
         │
         ▼
 Structured output (JSON)
```

## Note on MonoSketch

[MonoSketch](https://monosketch.io/) is an open-source, browser-based ASCII diagram editor (Kotlin/JS) by tuanchauict. It provides an interactive canvas for drawing boxes, lines, and text using Unicode box-drawing characters -- exactly the kind of diagrams shown above. It could be a useful tool for creating and editing the ASCII diagrams in this document interactively, though for version-controlled documentation the hand-crafted ASCII art above is sufficient.
