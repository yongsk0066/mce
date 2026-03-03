# data/

Runtime data files used by the MCE engine.

## Files

- **`mor.vfst`** (3.8 MB) — Voikko Finnish morphological FST dictionary in VFST binary format. Source: [corevoikko](https://github.com/yongsk0066/corevoikko) (`voikko-fi/vvfst/mor.vfst`). License: GPL-3.0 (Voikko project). Required for all morphological analysis, spell checking, and evaluation.

- **`suffix_tagger.bin`** (5.0 MB) — Trained suffix-based logistic regression model for POS tagging. Binary format containing feature vocabulary, weight matrix, class labels, and biases. Loaded by `mce-disambig::suffix_tagger::SuffixTagger` and via `MceEngine::load_model()` in WASM. Achieves 95.56% UPOS accuracy (pipeline: CG + Suffix Tagger + Viterbi).

- **`lemma_dict.tsv`** (977 KB) — Lemma dictionary extracted from CoNLL-U training data. TSV format: `form<TAB>UPOS<TAB>lemma`. For each unique (lowercase form, UPOS) pair, contains the most frequent lemma. Used by `mce-eval::lemma_dict` for evaluation.

- **`suffix_tagger.bin.bak`** — Backup of a previous suffix tagger model version.

## Licenses

| File | License | Source |
|------|---------|--------|
| `mor.vfst` | GPL-3.0 | [Voikko](https://voikko.puimula.org/) via corevoikko |
| `suffix_tagger.bin` | Apache-2.0 | Trained on UD Finnish-TDT (CC BY-SA 4.0) |
| `lemma_dict.tsv` | Apache-2.0 | Extracted from UD Finnish-TDT (CC BY-SA 4.0) |

## Regeneration

- `mor.vfst`: Built from corevoikko (`voikko-fi/vvfst/mor.vfst`)
- `suffix_tagger.bin`: Run `experiments/suffix-tagger/train_and_export.py`
- `lemma_dict.tsv`: Run `scripts/extract_lemma_dict.py <train.conllu> data/lemma_dict.tsv`
