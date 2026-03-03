# mce-fi

Finnish language module for MCE.

## Purpose

This crate contains all Finnish-specific linguistic knowledge: phonological constants (vowels, consonants, vowel harmony), FST-based morphological analysis, compound word handling, hyphenation, spell checking integration, and morphological generation. It bridges the language-agnostic MCE infrastructure with Finnish grammar rules.

## Key Types

- `morphology::FinnishAnalyzer` — FST-based morphological analyzer
- `morphology::Analyzer` trait — analysis interface
- `compound::FinnishCompoundAnalyzer` — Finnish compound word splitting
- `hyphenation::FinnishHyphenator` — rule-based Finnish hyphenation
- `spellcheck::FinnishSpellChecker` — Finnish spell checker using FST
- `generator::MorphGenerator` — morphological form generation (11 noun cases, 4 verb types)
- `VOWELS`, `CONSONANTS`, `BACK_VOWELS`, `FRONT_VOWELS` — phonological constants
- `is_vowel()`, `is_consonant()` — character classification helpers

## Dependencies

Uses: `mce-core`, `mce-fst`, `mce-speller`, `mce-disambig`, `mce-comonad`

Used by: `mce-grammar`, `mce-eval`, `mce-wasm`, `mce-cli`
