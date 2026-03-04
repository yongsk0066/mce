# mce-wasm

WebAssembly bindings for the MCE Finnish NLP engine. Exposes 22 API methods via `wasm-bindgen` for complete offline Finnish language processing in the browser.

## Features

- **~365KB** WASM binary, **<5ms/sentence** latency
- Morphological analysis, spell checking, grammar checking, hyphenation
- Sentence-level POS disambiguation (Viterbi + CG + optional suffix tagger)
- Compound word splitting, spelling suggestions, morphological generation
- Fully offline -- no server dependency

## API (22 methods)

| Method | Description |
|--------|-------------|
| `load(dict)` | Create engine from VFST dictionary bytes |
| `load_model(data)` | Load suffix tagger model (boosts UPOS 82.71% -> 94.58%) |
| `has_model()` | Check if suffix tagger model is loaded |
| `load_wordlist(data)` | Load wordlist for trie-based spelling suggestions |
| `has_wordlist()` | Check if wordlist (suggestion trie) is loaded |
| `analyze(word)` | Single-word morphological analysis (JSON) |
| `spell_check(word)` | Check if word is correctly spelled (morph analysis + compound-aware) |
| `suggest(word, max)` | Spelling suggestions for misspelled words |
| `suggest_with_context(word, prev, max)` | Context-aware suggestions ranked by POS bigram |
| `analyze_sentence(text)` | Sentence analysis with disambiguation (JSON) |
| `disambiguate_sentence(text)` | POS disambiguation with full attributes (JSON) |
| `compound_split(word)` | Compound word splitting with penalties |
| `grammar_check(text)` | Grammar error detection with byte offsets |
| `hyphenate(word)` | Single-word Finnish hyphenation |
| `hyphenate_text(text)` | Full-text hyphenation preserving non-word tokens |
| `get_baseform(word)` | Lemma lookup (disambiguated) |
| `is_valid_word(word)` | Pure morphological analysis check (boolean) |
| `generate_form(base, case, number)` | Generate noun case form (singular or plural) |
| `generate_paradigm(base)` | Full noun paradigm (22 forms: 11 singular + 11 plural) |
| `generate_verb_form(...)` | Generate verb conjugation |
| `generate_verb_paradigm(inf)` | Full verb paradigm |
| `version()` | Engine version string |

## Usage

```typescript
import init, { MceEngine } from './mce_wasm.js';

await init();

// Load dictionary (required)
const dict = await fetch('mor.vfst').then(r => r.arrayBuffer());
const engine = MceEngine.load(new Uint8Array(dict));

// Optional: load suffix tagger for higher accuracy
const model = await fetch('suffix_tagger.bin').then(r => r.arrayBuffer());
engine.load_model(new Uint8Array(model));
console.log(engine.has_model()); // true

// Morphological analysis
engine.analyze("koira");
// [{"CLASS":"nimisana","BASEFORM":"koira","STRUCTURE":"=ppppp",...}]

// Spell checking
engine.spell_check("koira");    // true
engine.spell_check("koirra");   // false

// Sentence-level analysis with disambiguation
engine.analyze_sentence("Koira juoksee nopeasti");
// [{"word":"Koira","analysis":{"CLASS":"nimisana","BASEFORM":"koira"}},
//  {"word":"juoksee","analysis":{"CLASS":"teonsana","BASEFORM":"juosta"}}]

// Grammar checking (byte offsets for editor integration)
engine.grammar_check("koira koira juoksee.");
// [{"start":6,"end":11,"code":"REPEATED_WORD","message":"Repeated word: koira","suggestions":["koira"]}]

// Compound word splitting
engine.compound_split("rautatieasema");
// [{"parts":[{"surface":"rauta",...},{"surface":"tie",...},{"surface":"asema",...}],"penalty":30}]

// Hyphenation
engine.hyphenate("suomalainen");              // "suo-ma-lai-nen"
engine.hyphenate_text("Koira juoksee.");      // "Koi-ra juok-see."

// Lemma lookup
engine.get_baseform("koirien");               // "koira"

// Morphological generation
engine.generate_paradigm("koira");            // Full 11-case paradigm JSON
engine.generate_verb_paradigm("juosta");      // Full verb conjugation JSON
```

## Build

```bash
# Install wasm-pack
cargo install wasm-pack

# Build WASM package
wasm-pack build crates/mce-wasm --target web --release

# Output: pkg/mce_wasm.js, pkg/mce_wasm_bg.wasm (~365KB)
```

## Dependencies

Uses: `mce-core`, `mce-fst`, `mce-fi`, `mce-disambig`, `mce-comonad`, `mce-tokenizer`, `mce-grammar`, `wasm-bindgen`, `js-sys`, `serde`, `serde-wasm-bindgen`

Used by: JavaScript/browser consumers
