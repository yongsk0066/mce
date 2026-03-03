# mce-cli

Command-line interface for interactive testing and evaluation of the MCE engine.

## Purpose

This binary crate provides subcommands for all MCE capabilities: morphological analysis, spell checking, compound word analysis, sentence-level disambiguation, grammar checking, hyphenation, UD treebank evaluation, and performance benchmarking. It is the primary development and debugging tool for the MCE pipeline.

## Subcommands

- `analyze <word>` — morphological analysis of a single word
- `spell <word>` — spell check a word
- `compound <word>` — compound word splitting
- `sentence "<text>"` — sentence-level analysis with disambiguation
- `grammar "<text>"` — grammar checking
- `hyphenate <words...>` — hyphenate words
- `hyphenate-text "<text>"` — hyphenate running text
- `info` — show dictionary information
- `eval --conllu <file>` — evaluate against UD treebank (UPOS, lemma accuracy)
- `benchmark --iterations N` — performance benchmarking

## Dependencies

Uses: `mce-core`, `mce-fst`, `mce-tokenizer`, `mce-speller`, `mce-disambig`, `mce-comonad`, `mce-fi`, `mce-grammar`, `mce-eval`

## Usage

```bash
export MCE_DICT_PATH=~/oss/corevoikko/voikko-fi/vvfst
cargo run -p mce-cli -- analyze koira
cargo run -p mce-cli -- sentence "Koira juoksee nopeasti"
cargo run -p mce-cli -- eval --conllu fi_tdt-ud-dev.conllu
```
