# mce-eval

UPOS evaluation infrastructure for benchmarking MCE against UD treebanks.

## Purpose

This crate evaluates MCE's morphological analysis pipeline against Universal Dependencies gold-standard annotations. It parses CoNLL-U files, runs MCE's full pipeline (tokenize, analyze, disambiguate, map POS), and computes UPOS accuracy, lemma accuracy, and per-POS precision/recall/F1 metrics.

## Key Types

- `pipeline::EvalPipeline` — end-to-end evaluation pipeline (VFST -> metrics)
- `conllu::parse_conllu_file()` — CoNLL-U format parser
- `pos_map` — MCE Finnish class to UD UPOS tag mapping
- `metrics` — accuracy, precision/recall/F1, confusion matrix computation
- `lemma_dict` — lemma dictionary lookup for evaluation

## Dependencies

Uses: `mce-core`, `mce-fst`, `mce-fi`, `mce-disambig`, `mce-comonad`, `mce-tokenizer`

Used by: `mce-cli`

## Example

```bash
# Run evaluation from the CLI
export MCE_DICT_PATH=~/oss/corevoikko/voikko-fi/vvfst
cargo run -p mce-cli -- eval --conllu fi_tdt-ud-dev.conllu
```
