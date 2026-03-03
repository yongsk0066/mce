# experiments/

Experiment scripts for model training, validation, and research.

## Directories

- **`suffix-tagger/`** — Training pipeline for the suffix-based logistic regression POS tagger. `train_and_export.py` trains on CoNLL-U data and exports the binary model to `data/suffix_tagger.bin`.

- **`tt-rank/`** — Tensor-Train rank decomposition experiments for morphological paradigm analysis. Includes paradigm extraction, TT decomposition, statistical tests (Kruskal-Wallis), and cross-linguistic comparison across 12 languages. Supports the "Morphological Fingerprint" (Paper-2) research.

- **`cs-validation/`** — Compressed Sensing validation experiments for the disambiguation engine. Documents why the CS approach was found to provide negative results for this task.
