# data/

Runtime data files used by the MCE engine.

## Files

- **`suffix_tagger.bin`** (5.2 MB) — Trained suffix-based logistic regression model for POS tagging. Binary format containing feature vocabulary, weight matrix, class labels, and biases. Loaded by `mce-disambig::suffix_tagger::SuffixTagger` and via `MceEngine::load_model()` in WASM. Achieves ~94.87% standalone UPOS accuracy.

- **`lemma_dict.tsv`** (977 KB) — Lemma dictionary extracted from CoNLL-U training data. TSV format: `form<TAB>UPOS<TAB>lemma`. For each unique (lowercase form, UPOS) pair, contains the most frequent lemma. Used by `mce-eval::lemma_dict` for evaluation.

- **`suffix_tagger.bin.bak`** — Backup of a previous suffix tagger model version.

## Regeneration

- `suffix_tagger.bin`: Run `experiments/suffix-tagger/train_and_export.py`
- `lemma_dict.tsv`: Run `scripts/extract_lemma_dict.py <train.conllu> data/lemma_dict.tsv`
