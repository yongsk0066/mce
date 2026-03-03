# scripts/

Utility scripts for data preparation.

## Scripts

- **`extract_lemma_dict.py`** — Extracts a (form, UPOS) -> lemma dictionary from CoNLL-U training data. For each unique (lowercase form, UPOS) pair, keeps the most frequent lemma. Outputs a sorted TSV file used by the evaluation pipeline.

```bash
python3 scripts/extract_lemma_dict.py \
    ../ud-finnish-tdt/fi_tdt-ud-train.conllu \
    data/lemma_dict.tsv
```
