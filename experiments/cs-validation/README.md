# CS (Compressed Sensing) Disambiguation Validation

**Date**: 2026-03-01
**Goal**: Empirically test whether the FISTA/CS layer in `mce-disambig/src/cs.rs` improves UPOS tagging accuracy.

## Hypothesis

The `SparseDisambiguator` encodes morphological analyses as sparse vectors in a 159-dimensional feature space, runs FISTA sparse recovery, and uses reconstruction error as a disambiguation signal. The idea: analyses that reconstruct well from compressed measurements are more "natural" and should be preferred.

## Setup

- **Dataset**: Finnish-TDT UD v2.5 dev set (`fi_tdt-ud-dev.conllu`, 1364 sentences, 15,651 non-punct tokens)
- **Training**: Bigram model + emission priors from `fi_tdt-ud-train.conllu` (46,293 word forms)
- **Pipeline**: FST analyzer + CG-lite pruning + Viterbi disambiguator
- **Gold tokenization**: Used gold tokens to isolate POS tagging from tokenization errors
- **Hardware**: Apple M1 (release build)

## Results Summary

### Main Comparison (full dev set, 1364 sentences)

| Config | UPOS Accuracy | Lemma Accuracy | Speed (tok/s) | Slowdown |
|--------|--------------|----------------|---------------|----------|
| **Baseline** (bigrams + emission) | **82.47%** (12907/15651) | **82.65%** | 8319 | 1x |
| **+CS** (m=50, lambda=0.1) | 82.35% (12889/15651) | 82.33% | 154 | **54x** |

### Hyperparameter Sweep (200 sentences subset)

| Config | m | lambda | UPOS | Speed (tok/s) | Delta vs Baseline |
|--------|---|--------|------|---------------|-------------------|
| Baseline | - | - | 81.47% | 9585 | - |
| +CS | 50 | 0.1 | 81.20% | 158 | **-0.27%** |
| +CS | 30 | 0.01 | 81.11% | 193 | **-0.36%** |
| +CS | 20 | 0.5 | 81.11% | 260 | **-0.36%** |
| +CS | 80 | 0.001 | 81.29% | 87 | **-0.18%** |
| CS-only (no emission) | 50 | 0.1 | 79.70% | 157 | **-1.77%** |

### Per-POS Comparison (full dev set)

| POS | Baseline F1 | +CS F1 | Delta |
|-----|------------|--------|-------|
| ADJ | 67.11% | 67.10% | -0.01% |
| ADP | 40.75% | 40.75% | 0.00% |
| ADV | 66.05% | 65.90% | -0.15% |
| AUX | 92.01% | 91.84% | -0.17% |
| CCONJ | 96.22% | 96.22% | 0.00% |
| INTJ | 40.00% | 40.00% | 0.00% |
| NOUN | 84.55% | 84.34% | -0.21% |
| NUM | 74.95% | 74.95% | 0.00% |
| PRON | 88.59% | 88.59% | 0.00% |
| PROPN | 82.09% | 82.25% | +0.16% |
| SCONJ | 84.34% | 84.34% | 0.00% |
| VERB | 87.27% | 87.29% | +0.02% |
| X | 1.75% | 1.69% | -0.06% |

### Key Confusion Changes (full dev set)

| Confusion | Baseline | +CS | Change |
|-----------|----------|-----|--------|
| VERB -> NOUN | 148 | 172 | **+24 (worse)** |
| ADV -> ADJ | 181 | 186 | +5 |
| ADJ -> NOUN | 279 | 285 | +6 |
| NOUN -> VERB | 158 | 144 | -14 (better) |
| ADJ -> VERB | 132 | 124 | -8 (better) |

## Analysis

### Why CS Does Not Help

1. **Wrong signal granularity**: CS measures reconstruction error of individual analyses in isolation. It knows nothing about the sentence context. The Viterbi bigram model already handles context, and emission priors already handle word-level preferences. CS adds a redundant, context-free signal.

2. **Reconstruction error does not correlate with correctness**: A morphological analysis that is "sparse" (few features set) is not necessarily the correct analysis. For example, a simple noun reading `(nimisana, nimento, singular)` and a verb reading `(teonsana, present_simple, indicative, 3, singular)` are both equally "sparse" (k=5-7) -- the reconstruction error is similar for both, providing no discrimination.

3. **Scale mismatch**: The CS scores (negated reconstruction error) are small negative values that get added to emission scores. The emission prior scores (based on P(UPOS|word) from training data) are much stronger signals. CS scores end up being noise on top of already-calibrated emission priors.

4. **Noise injection effect**: In the VERB->NOUN confusion, CS made 24 more errors than baseline. This suggests the CS scores are actively pushing some verb readings toward nouns (likely because noun feature vectors have slightly lower reconstruction error due to fewer required features).

5. **Massive computational cost**: FISTA involves creating a 50x159 random Gaussian matrix, computing A^T*A (159x159), running power iteration, then 200 FISTA iterations -- all per candidate analysis per word. This is O(m*n*max_iter) per analysis, which for 159-dim features and 50 measurements is about 1.6M floating-point operations per analysis. With ~3 analyses/word and ~11.5 words/sentence, that is ~55M FLOPs per sentence, explaining the 54x slowdown.

### Why CS Was Theoretically Appealing But Practically Fails

The compressed sensing framework works well when:
- The signal is truly sparse in a known basis (CS recovers it perfectly)
- The "correct" signal has systematically lower reconstruction error than "incorrect" signals
- There is no better signal available

In our case:
- All valid morphological analyses are equally sparse (k=3-7 features)
- The measurement matrix is random and unlearned (no supervision)
- The emission priors from training data are a much stronger, learned signal

## Conclusion

**CS does NOT improve UPOS accuracy and should not be included in the disambiguation pipeline.**

- Accuracy delta: **-0.12%** on the full dev set (consistently negative across all hyperparameters)
- Speed penalty: **54x slower** (8319 -> 154 tokens/sec)
- No POS category benefits significantly

### Recommendation for paper-5

The CS layer should be kept as an **academic contribution** (demonstrating the theoretical framework) but clearly marked as **not recommended for production use**. The paper should honestly report these negative results. This is valuable: it demonstrates that sparse recovery of morphological features, while mathematically sound, does not provide a useful disambiguation signal when context-free reconstruction error is the scoring criterion.

A more promising direction would be to learn the measurement matrix from supervised data (rather than random Gaussian), or to use CS for a different task where sparsity correlates with correctness (e.g., detecting invalid/corrupted analyses rather than disambiguating valid ones).

## Reproduction Commands

```bash
# Baseline
MCE_DICT_PATH=data cargo run -p mce-eval --release -- \
  --conllu vendor/ud-finnish-tdt/fi_tdt-ud-dev.conllu \
  --train vendor/ud-finnish-tdt/fi_tdt-ud-train.conllu

# +CS
MCE_DICT_PATH=data cargo run -p mce-eval --release -- \
  --conllu vendor/ud-finnish-tdt/fi_tdt-ud-dev.conllu \
  --train vendor/ud-finnish-tdt/fi_tdt-ud-train.conllu \
  --enable-cs

# CS-only (no emission priors)
MCE_DICT_PATH=data cargo run -p mce-eval --release -- \
  --conllu vendor/ud-finnish-tdt/fi_tdt-ud-dev.conllu \
  --train vendor/ud-finnish-tdt/fi_tdt-ud-train.conllu \
  --cs-only

# Custom CS parameters
MCE_DICT_PATH=data cargo run -p mce-eval --release -- \
  --conllu vendor/ud-finnish-tdt/fi_tdt-ud-dev.conllu \
  --train vendor/ud-finnish-tdt/fi_tdt-ud-train.conllu \
  --enable-cs --cs-measurements 30 --cs-lambda 0.01
```
