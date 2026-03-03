# mce-eval

Evaluation infrastructure for benchmarking MCE against Universal Dependencies treebanks. Measures UPOS accuracy, lemma accuracy, coverage, and per-POS precision/recall/F1.

## Architecture

```text
CoNLL-U file (gold) ──┐
                       ├──> EvalPipeline ──> EvalResults
MCE Pipeline ─────────┘
  (tokenize -> analyze -> CG prune -> disambiguate -> map POS)
```

The pipeline uses gold tokenization (tokens from CoNLL-U) to isolate POS tagging errors from tokenization errors.

## Usage (CLI)

```bash
export MCE_DICT_PATH=~/oss/corevoikko/voikko-fi/vvfst

# Basic evaluation
mce-cli eval --conllu fi_tdt-ud-dev.conllu

# With corpus-trained bigram model
mce-cli eval --conllu fi_tdt-ud-dev.conllu --train fi_tdt-ud-train.conllu

# With suffix tagger model (UPOS 83% -> 95%)
mce-cli eval --conllu fi_tdt-ud-dev.conllu --model data/suffix_tagger.bin

# JSON output for automated processing
mce-cli eval --conllu fi_tdt-ud-dev.conllu --format json
```

## Usage (Rust)

```rust
use std::path::Path;
use mce_eval::conllu::parse_conllu_file;
use mce_eval::pipeline::EvalPipeline;

// Basic pipeline
let dict_data = std::fs::read("path/to/mor.vfst").unwrap();
let pipeline = EvalPipeline::from_bytes(&dict_data).unwrap();

// Parse gold-standard CoNLL-U file
let sentences = parse_conllu_file(Path::new("fi_tdt-ud-dev.conllu")).unwrap();
let results = pipeline.evaluate(&sentences);

println!("UPOS:     {:.2}%", results.upos_accuracy() * 100.0);
println!("Lemma:    {:.2}%", results.lemma_accuracy() * 100.0);
println!("Coverage: {:.2}%", results.coverage() * 100.0);
```

### Corpus-Trained Pipeline

```rust
let train_data = std::fs::read_to_string("fi_tdt-ud-train.conllu").unwrap();
let pipeline = EvalPipeline::from_bytes_with_corpus(&dict_data, &train_data).unwrap();
```

### Lemma Dictionary

The `lemma_dict` module provides dictionary-enhanced lemmatization from UD training data. A TSV file of `(form, UPOS) -> lemma` mappings is consulted before falling back to FST baseforms:

```rust
use mce_eval::lemma_dict::LemmaDict;

let dict = LemmaDict::from_file("data/lemma_dict.tsv").unwrap();
if let Some(lemma) = dict.lookup("juoksee", "VERB") {
    assert_eq!(lemma, "juosta");
}

// Set on pipeline
pipeline.set_lemma_dict(dict);
```

For out-of-vocabulary words, `strip_suffix()` applies heuristic suffix stripping as a last resort:

```rust
use mce_eval::lemma_dict::strip_suffix;

let lemma = strip_suffix("koirassa", "NOUN");  // strips "-ssa" -> "koira"
```

## Modules

| Module | Description |
|--------|-------------|
| `conllu` | CoNLL-U format parser (`parse_conllu_file`) |
| `pos_map` | MCE Finnish class -> UD UPOS tag mapping (`mce_to_upos`) |
| `metrics` | Accuracy, precision/recall/F1, confusion matrix, top confusions |
| `pipeline` | `EvalPipeline` connecting MCE analysis to gold annotations |
| `lemma_dict` | TSV-based `(form, UPOS) -> lemma` dictionary + suffix stripping |

## Eval Pipeline Stages

1. **Analyze** each gold token with `FinnishAnalyzer` (FST traversal)
2. **CG prune** unlikely readings with 62 active CG-lite rules
3. **Suffix tagger** emission scoring (if model loaded, +12pp UPOS)
4. **Disambiguate** with `ViterbiDisambiguator` (POS bigram model)
5. **Map POS** from MCE Finnish class (e.g., `nimisana`) to UPOS (e.g., `NOUN`)
6. **Compare** predicted UPOS/lemma against gold annotations

## Dependencies

Uses: `mce-core`, `mce-fst`, `mce-fi`, `mce-disambig`, `mce-comonad`, `mce-tokenizer`

Used by: `mce-cli`
