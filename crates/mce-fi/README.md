# mce-fi

Finnish language module for MCE (Morphological Computation Engine).

Provides all Finnish-specific linguistic knowledge: FST-based morphological
analysis, morphological generation via coKleisli pipelines, compound word
splitting, rule-based hyphenation, spell checking, and phonological constants
(vowels, consonants, vowel harmony).

## Features

- **Morphological analysis** -- FST (VFST) transducer maps surface forms to
  baseforms, POS tags, case, number, person, mood, tense, and more.
- **Morphological generation** -- coKleisli pipeline (consonant gradation,
  vowel harmony, possessive suffix) generates inflected forms from baseforms.
  Covers 22 noun forms (11 singular + 11 plural cases) and 4 verb conjugation types.
  Verb generation is beta-quality: regular verbs work correctly but irregular
  verbs (e.g., olla, syoda, juosta) produce incorrect forms.
- **Compound analysis** -- pushdown-transducer-based splitter with 6 Finnish
  linking morphemes (`-en-`, `-n-`, `-s-`, `-i-`, `-o-`, `-u-`, and zero).
  Includes nen-stem reconstruction (e.g., `hevosenkenkä` -> `hevonen + kenkä`).
- **Hyphenation** -- purely algorithmic syllable-based hyphenation. Handles
  16 diphthongs, long vowels, and consonant clusters without a dictionary.
- **Spell checking** -- delegates to `mce-speller` with Finnish-specific
  FST dictionary lookup.

## Key Types

| Type | Description |
|------|-------------|
| `morphology::FinnishAnalyzer` | FST-based morphological analyzer (VFST backend) |
| `morphology::Analyzer` | Trait for pluggable analysis backends |
| `generator::MorphGenerator` | Noun/verb form generation via coKleisli pipeline |
| `compound::FinnishCompoundAnalyzer` | Compound word splitting with stem reconstruction |
| `hyphenation::FinnishHyphenator` | Rule-based Finnish syllable hyphenation |
| `spellcheck::FinnishSpellChecker` | Spell checker using FST dictionary |
| `generator::CaseInfo` | Case metadata (name, suffix, gradation grade) |
| `generator::VerbTense` | Present / Past / Conditional |
| `generator::VerbPerson` | First / Second / Third |
| `generator::VerbNumber` | Singular / Plural |

## Noun Cases (22 forms: 11 singular + 11 plural)
Nominative, genitive, partitive, inessive, elative, illative, adessive,
ablative, allative, essive, and translative -- each in both singular and
plural. Labels use the format "nominative sg", "genitive pl", etc.
Each case carries a suffix pattern with archiphonemic characters
(`A` -> `a`/`ä`, `V` -> vowel copy) and a consonant gradation grade
(strong or weak).

## Verb Conjugation Types (4)

| Type | Pattern | Examples |
|------|---------|----------|
| Type 1 | Two vowels: `-Va` / `-Vä` | puhua, lukea, antaa |
| Type 2 | Consonant + `-da` / `-dä` | syödä, juoda, viedä |
| Type 3 | Consonant + `-la/-na/-ra/-sta` | tulla, mennä, purra, nousta |
| Type 4 | Vowel + `-ta` / `-tä` | haluta, pelätä |

## Usage

```rust
use mce_fi::hyphenation::FinnishHyphenator;
use mce_fi::generator::MorphGenerator;

// Hyphenation
let h = FinnishHyphenator::new();
assert_eq!(h.hyphenate_word("suomalainen"), "suo-ma-lai-nen");
assert_eq!(h.hyphenate_word("Helsinki"), "Hel-sin-ki");

// Morphological generation
let gen = MorphGenerator::new();
let form = gen.generate("kaappi", &[("SIJAMUOTO", "omanto")]);
assert_eq!(form, Some("kaapin".to_string()));

// Full noun paradigm (22 forms: 11 sg + 11 pl)
let paradigm = gen.generate_paradigm("talo");
assert_eq!(paradigm.len(), 22);
assert!(paradigm.iter().any(|(case, form)| case == "genitive sg" && form == "talon"));
assert!(paradigm.iter().any(|(case, form)| case == "nominative pl" && form == "talot"));
```

## Dependencies

| Uses | Used by |
|------|---------|
| `mce-core`, `mce-fst`, `mce-speller`, `mce-disambig`, `mce-comonad` | `mce-grammar`, `mce-eval`, `mce-wasm`, `mce-cli` |
