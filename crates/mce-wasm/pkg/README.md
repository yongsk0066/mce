# @yongsk0066/mce

Finnish NLP engine compiled to WebAssembly. Runs entirely in the browser with no server required.

## Features

- **Morphological analysis** — decompose Finnish words into morphemes
- **Spell checking** — validate words against the dictionary
- **Suggestions** — context-aware spelling suggestions
- **POS tagging** — sentence-level disambiguation (UPOS 95.56%)
- **Hyphenation** — Finnish hyphenation rules
- **Grammar checking** — detect common grammar errors
- **Compound splitting** — decompose compound words
- **Lemmatization** — extract base forms
- **Morphological generation** — generate inflected forms (noun cases, verb conjugations)

225KB WASM, <5ms/sentence, fully offline.

## Quick Start

```js
import init, { MceEngine } from '@yongsk0066/mce';

await init();

// Load the dictionary (mor.vfst — not included, see below)
const dictBytes = await fetch('mor.vfst').then(r => r.arrayBuffer());
const engine = MceEngine.load(new Uint8Array(dictBytes));

// Spell check
engine.spell_check("koira");          // true
engine.spell_check("koirra");         // false

// Morphological analysis
engine.analyze("koirien");            // JSON array of analyses

// Suggestions
engine.suggest("koirra", 1);          // ["koira", ...]

// Sentence analysis with POS tagging
engine.analyze_sentence("Koira juoksee nopeasti.");

// Hyphenation
engine.hyphenate("suomalainen");      // "suo-ma-lai-nen"

// Grammar checking
engine.grammar_check("koira koira juoksee.");

// Compound splitting
engine.compound_split("rautatieasema");

// Lemmatization
engine.get_baseform("koirien");       // "koira"

// Generate inflected forms
engine.generate_paradigm("koira");    // all 11 noun cases
engine.generate_verb_paradigm("juosta"); // verb conjugations
```

## Dictionary

This package provides the WASM engine only. You need a `mor.vfst` dictionary file to use it. The dictionary is available from the [MCE repository](https://github.com/yongsk0066/mce).

## API

| Method | Description |
|--------|-------------|
| `MceEngine.load(bytes)` | Create engine from VFST dictionary |
| `engine.load_model(bytes)` | Load suffix tagger model for better POS accuracy |
| `engine.has_model()` | Check if suffix tagger model is loaded |
| `engine.analyze(word)` | Morphological analysis (JSON) |
| `engine.spell_check(word)` | Spell check (boolean) |
| `engine.suggest(word, maxEdits)` | Spelling suggestions (JSON) |
| `engine.suggest_with_context(word, prev, maxEdits)` | Context-aware suggestions (JSON) |
| `engine.analyze_sentence(text)` | Sentence analysis with disambiguation (JSON) |
| `engine.disambiguate_sentence(text)` | Full disambiguation with scores (JSON) |
| `engine.compound_split(word)` | Compound word splitting (JSON) |
| `engine.grammar_check(text)` | Grammar checking (JSON) |
| `engine.hyphenate(word)` | Hyphenate a word |
| `engine.hyphenate_text(text)` | Hyphenate full text |
| `engine.get_baseform(word)` | Get lemma / base form |
| `engine.is_valid_word(word)` | Check if word exists in dictionary |
| `engine.generate_form(base, case, number)` | Generate a specific inflected form |
| `engine.generate_paradigm(base)` | Generate all noun case forms |
| `engine.generate_verb_form(base, tense, person, number)` | Generate a specific verb form |
| `engine.generate_verb_paradigm(base)` | Generate all verb conjugations |
| `MceEngine.version()` | Engine version string |

## License

Apache-2.0

## Links

- [GitHub](https://github.com/yongsk0066/mce)
- [Full Documentation](https://github.com/yongsk0066/mce#readme)
