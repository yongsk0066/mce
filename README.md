# MCE — Morphological Computation Engine

[![CI](https://github.com/yongsk0066/mce/actions/workflows/ci.yml/badge.svg)](https://github.com/yongsk0066/mce/actions/workflows/ci.yml)
[![Performance](https://github.com/yongsk0066/mce/actions/workflows/perf.yml/badge.svg)](https://github.com/yongsk0066/mce/actions/workflows/perf.yml)
[![npm](https://img.shields.io/npm/v/@yongsk0066/mce)](https://www.npmjs.com/package/@yongsk0066/mce)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Browser-first Finnish NLP engine -- morphological analysis, POS tagging, spell checking, grammar checking, hyphenation, compound analysis, and morphological generation, all running offline in WebAssembly.

MCE uses a mathematically grounded architecture: a Writer Comonad for morphophonological rules, Constraint Grammar for disambiguation, and a suffix-based statistical tagger -- achieving 94.58% UPOS accuracy in ~395KB of WASM with no server required.

## Why MCE?

MCE occupies an unusual position in the Finnish NLP landscape. To our knowledge, no other system combines all five of these properties:

**Browser-first, no server.** MCE compiles to a ~395KB WASM module. The dictionary and model are fetched once from a CDN (~9.2MB, gzip ~2-3MB) and cached in IndexedDB. After first load, everything runs offline with zero network dependency. To our knowledge, no other Finnish NLP tool runs in the browser.

**Mathematical foundation.** Morphophonological rules (consonant gradation, vowel harmony, boundary effects) are expressed as coKleisli arrows over a Writer Comonad with a DeletionSet monoid. To our knowledge, this is the first production NLP system to use comonadic composition for morphophonology, ensuring that rules compose purely without mutation or sentinel characters.

**94.58% UPOS in 9.2MB.** MCE achieves near-neural accuracy through a hybrid pipeline: 62 Constraint Grammar rules prune impossible readings, a suffix-based statistical tagger provides emission scores, and Viterbi decoding selects the optimal POS sequence. Neural systems (TurkuNLP, Trankit) score 3-4pp higher but require 100-1000x more storage and a GPU server.

**Complete writer-tools stack.** MCE is not just a POS tagger. A single 9.2MB deployment provides spell checking, spelling suggestions, grammar checking (21 rules), hyphenation, compound word analysis, and morphological generation (11 noun cases, 4 verb conjugation types). To our knowledge, no other tool offers all of these in one package that runs client-side.

**Offline-first, privacy-preserving.** No text ever leaves the user's device. All computation happens locally in the browser or native runtime. This makes MCE suitable for sensitive documents, air-gapped environments, and privacy-conscious applications.

## Features

- **Morphological analysis** with full inflection details and POS disambiguation
- **POS tagging** at 94.58% UPOS accuracy (CG + Suffix Tagger)
- **Spell checking** with compound word and derivation support
- **Spelling suggestions** with context-aware ranking
- **Grammar checking** with 21 rule-based checks
- **Hyphenation** with compound-aware syllable splitting
- **Compound word analysis** with 6 linking morpheme types
- **Morphological generation** for nouns (22 forms: 11 singular + 11 plural) and verbs (4 conjugation types, beta -- irregular verbs may produce incorrect forms)
- **Sentence-level disambiguation** via Viterbi + Constraint Grammar + Suffix Tagger
- **Writer Comonad** pipeline for morphophonological rules (consonant gradation, vowel harmony)

## Use Cases

### Browser-Based Writing Tools
- In-browser Finnish spell checker with no server -- data never leaves the device
- Grammar checker extension for web editors (Google Docs, Notion, CMS platforms)
- Real-time hyphenation engine for responsive Finnish typography
- Offline-capable PWA for Finnish writers working without internet

### Education & Language Learning
- Interactive morphology explorer showing all 22 noun forms (11 cases x singular/plural) for any Finnish word
- Verb conjugation trainer that generates paradigms and quizzes learners
- Reading assistant: hover over any word to see its lemma, POS tag, and case
- L2 Finnish error correction with explanations of grammar mistakes

### Accessibility & Privacy
- Fully offline NLP for air-gapped environments (military, healthcare, legal)
- GDPR-compliant by design -- no cloud dependency, text never leaves the browser
- TTS preprocessing: correct morphological analysis improves text-to-speech output
- 9.2MB total footprint fits on IoT, edge, and mobile devices

### Research & NLP Pipelines
- Morphological annotation of Finnish corpora at 94.58% UPOS accuracy
- Lemmatization for information retrieval and search indexing
- Compound word decomposition for machine translation preprocessing
- Writer Comonad as a research tool for studying morphophonological theory

### Developer Integration
- npm package (`@yongsk0066/mce`) -- drop-in for any JS/TS project
- Rust crate -- native speed for server-side batch processing
- CLI with 11 subcommands for scripting and automation
- WASM API with 22 methods covering the full NLP pipeline

### What Only MCE Can Do
- Run a complete Finnish NLP stack in a browser tab with no internet connection
- Apply category theory (comonads) to morphophonological rule composition
- Generate full noun paradigms (22 forms: 11 singular + 11 plural) and verb conjugations client-side
- Analyze Finnish compounds of arbitrary depth (*lentokonesuihkuturbiinimoottori*)
- Deliver spell check + grammar check + hyphenation + POS tagging in under 9.2MB

## Quick Start

### npm (Browser / Node.js)

```bash
npm install @yongsk0066/mce
```

```typescript
import init, { MceEngine } from '@yongsk0066/mce';

await init();
const dictBytes = await fetch('mor.vfst').then(r => r.arrayBuffer());
const engine = MceEngine.load(new Uint8Array(dictBytes));

// Morphological analysis
engine.analyze('koirien');         // JSON: [{ BASEFORM: 'koira', CLASS: 'nimisana', ... }]

// Spell checking
engine.spell_check('koira');       // true
engine.suggest('koirra', 1);       // ['koira', ...]

// Sentence-level analysis with POS disambiguation
engine.analyze_sentence('Koira juoksee nopeasti.');

// Grammar checking
engine.grammar_check('Koira koira juoksee pihalla.');

// Hyphenation
engine.hyphenate('suomalainen');   // 'suo-ma-lai-nen'

// Compound word splitting
engine.compound_split('rautatieasema');

// Morphological generation
engine.generate_form('koira', 'genitive', 'singular');
engine.generate_verb_form('juosta', 'present', '3sg', 'affirmative');

// Load suffix tagger model for higher accuracy (94.58% UPOS)
const modelBytes = await fetch('suffix_tagger.bin').then(r => r.arrayBuffer());
engine.load_model(new Uint8Array(modelBytes));

// Optional: load wordlist for spelling suggestions
const wordlistBytes = await fetch('wordlist.txt').then(r => r.arrayBuffer());
engine.load_wordlist(new Uint8Array(wordlistBytes));

engine.free();
```

### Rust

```bash
cd crates
cargo test --all-features     # 1,579 tests
cargo clippy --all-features -- -D warnings
```

### CLI

Eleven subcommands for interactive use:

```bash
export MCE_DICT_PATH=/path/to/dictionary
cargo run -p mce-cli -- analyze koira
cargo run -p mce-cli -- spell koirra
cargo run -p mce-cli -- compound rautatieasema
cargo run -p mce-cli -- sentence "Koira juoksee nopeasti."
cargo run -p mce-cli -- grammar "Koira koira juoksee pihalla."
cargo run -p mce-cli -- hyphenate suomalainen
cargo run -p mce-cli -- hyphenate-text "Koira juoksee pihalla nopeasti."
cargo run -p mce-cli -- info
cargo run -p mce-cli -- eval --conllu fi_tdt-ud-dev.conllu
cargo run -p mce-cli -- benchmark --iterations 500
cargo run -p mce-cli -- benchmark --rules
```

## How It Fits Together

```mermaid
flowchart LR
    dict[VFST Dictionary] --> fst[mce-fst]
    fst --> core[mce-core]
    core --> comonad[mce-comonad]
    core --> tokenizer[mce-tokenizer]
    core --> speller[mce-speller]
    fst --> speller
    comonad --> fi[mce-fi]
    fst --> fi
    speller --> fi
    core --> disambig[mce-disambig]
    fi --> grammar[mce-grammar]
    tokenizer --> grammar
    disambig --> grammar
    fi --> wasm[mce-wasm]
    fi --> cli[mce-cli]
    fi --> eval[mce-eval]
    disambig --> eval
    grammar --> wasm
    grammar --> cli
    wasm --> js[JS/TS npm]
```

The Rust workspace contains 11 crates:

| Crate | Role |
|-------|------|
| `mce-core` | Shared types, character classification, LOUDS succinct trie (M1) |
| `mce-fst` | FST engine with format abstraction and VFST traversal |
| `mce-tokenizer` | Text tokenizer (words, sentences, URLs, emails) |
| `mce-speller` | Spell checking and suggestion engine |
| `mce-comonad` | Writer Comonad morphophonological engine (M2') + CG rules |
| `mce-disambig` | Disambiguation: Viterbi + CG-lite + Suffix Tagger (M4') |
| `mce-fi` | Finnish language module (analysis, generation, compounds, hyphenation) |
| `mce-grammar` | Grammar checking (21 rules) |
| `mce-eval` | UPOS/Lemma evaluation against UD treebanks |
| `mce-wasm` | WebAssembly bindings (22 API methods) |
| `mce-cli` | Command-line tools (11 subcommands) |

## Performance

| Metric | Value |
|--------|-------|
| UPOS accuracy (CG + Suffix Tagger) | **94.58%** |
| UPOS accuracy (rule-only) | 82.71% |
| Lemma accuracy | 88.44% |
| Coverage | 99.64% |
| Speed | 88,285 tokens/sec (~0.8ms per sentence) |
| WASM binary | ~395KB |
| Total deploy size | ~9.2MB (WASM + dictionary + model) |
| Deploy size (gzip) | ~2-3MB |
| CG rules | 62 active (85 total) |
| Grammar rules | 21 |
| Tests | 1,579 passed |
| Lines of code | ~45,600 Rust |

### Comparison with Other Finnish NLP Tools

| | MCE | Omorfi | TurkuNLP (TNPP) | Trankit |
|--|-----|--------|-----------------|---------|
| **UPOS** | 94.58% | 83.88% | 97.80% | 98.48% |
| **Environment** | Browser (WASM) | CLI / HFST | GPU server | GPU server |
| **Offline** | Yes (fully) | Yes | No | No |
| **Deploy size** | 9.2MB | ~50MB+ | ~1GB+ | ~1GB+ |
| **Latency** | 1.35ms/sent | ~10ms | ~100ms+ | ~100ms+ |
| **Writer tools** | Yes | No | No | No |
| **Maintained** | Yes | Yes | Deprecated | Yes |

MCE trades ~3-4pp of UPOS accuracy for a deployment that is orders of magnitude smaller and runs entirely in the browser with no network dependency.

*UPOS and lemma accuracy measured with gold tokenization from UD Finnish-TDT test set. Lemma accuracy measured with 42K-entry evaluation dictionary (evaluation splits excluded); production dictionary includes 48K entries for broader coverage. End-to-end accuracy including MCE's own tokenizer may differ slightly.*

## Architecture

MCE v3 combines four computational machines:

| Machine | Role | Mathematical Basis |
|---------|------|--------------------|
| M1: Succinct Trie | Dictionary lookup / spell checking | LOUDS encoding |
| M2': Comonadic Engine | Morphological analysis + morphophonological rules | Writer Comonad (coKleisli composition) |
| M3: PDT | Compound word structure analysis | Pushdown Transducer |
| M4': Weighted Lattice | POS disambiguation | Viterbi + CG-lite + Suffix Tagger |

The Writer Comonad (M2') expresses all Finnish morphophonological rules -- consonant gradation (11 patterns), vowel harmony, and boundary effects -- as pure coKleisli arrows that compose without mutation or sentinel characters.

## License

Apache-2.0

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## Credits

MCE is built by Yongseok Jang as the analytical core for [corevoikko](https://github.com/yongsk0066/corevoikko), a Rust+WASM rewrite of [Voikko](https://voikko.puimula.org/). The Finnish dictionary data originates from the Voikko project contributors.

## Documentation

- [CLAUDE.md](CLAUDE.md) -- project context and architecture details
- [ARCHITECTURE.md](ARCHITECTURE.md) -- 4-machine architecture and crate dependency graph

## Links

- [corevoikko](https://github.com/yongsk0066/corevoikko) -- parent project (Voikko in Rust+WASM)
- [Live Demo](https://yongsk0066.github.io/mce/) -- try Finnish NLP in the browser
- [Original Voikko](https://voikko.puimula.org/)
