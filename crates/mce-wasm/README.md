# mce-wasm

WebAssembly bindings for the MCE Finnish NLP engine.

## Purpose

This crate provides a browser-friendly JavaScript API via `wasm-bindgen`, exposing MCE's full feature set: morphological analysis, spell checking, grammar checking, hyphenation, sentence-level disambiguation, suggestion generation, and compound word splitting. It targets ~225KB WASM binary with <5ms/sentence latency for complete offline operation in the browser.

## Key Types

- `MceEngine` — main entry point, created from a VFST dictionary (`Uint8Array`)
- `MceEngine::load()` — instantiate from dictionary bytes
- `MceEngine::load_model()` — load suffix tagger model for improved accuracy
- `MceEngine::analyze()` — single-word morphological analysis (JSON)
- `MceEngine::analyze_sentence()` — sentence-level analysis with disambiguation
- `MceEngine::spell_check()` — spell checking (boolean)
- `MceEngine::suggest()` — spelling suggestions
- `MceEngine::grammar_check()` — grammar error detection (JSON)
- `MceEngine::hyphenate()` / `hyphenate_text()` — Finnish hyphenation
- `MceEngine::compound_split()` — compound word analysis

## Dependencies

Uses: `mce-core`, `mce-fst`, `mce-fi`, `mce-speller`, `mce-disambig`, `mce-comonad`, `mce-tokenizer`, `mce-grammar`, `wasm-bindgen`, `js-sys`, `serde`, `serde-wasm-bindgen`

Used by: JavaScript/browser consumers

## Usage (JavaScript)

```js
import init, { MceEngine } from './mce_wasm.js';

await init();
const dict = await fetch('mor.vfst').then(r => r.arrayBuffer());
const engine = MceEngine.load(new Uint8Array(dict));

engine.analyze("koira");                           // JSON array of analyses
engine.spell_check("koira");                       // true
engine.analyze_sentence("Koira juoksee nopeasti"); // disambiguated JSON
engine.grammar_check("koira koira juoksee.");      // grammar errors JSON
engine.hyphenate("suomalainen");                   // "suo-ma-lai-nen"
```
