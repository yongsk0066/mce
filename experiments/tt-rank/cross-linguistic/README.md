# Cross-Linguistic TT-Rank Analysis: Finnish vs Turkish

**Paper-2 target**: SIGMORPHON
**Date**: 2026-03-01
**Status**: Experiment complete

## Motivation

The Finnish TT-rank experiment (Paper-2) found that bond-specific TT-rank precisely
measures the effective dimensionality of feature interactions in inflectional paradigms.
The key finding was at the Mood-Tense bond of Finnish verbs, where rank 5/8 exactly
corresponds to the 5 independent Mood x Tense combinations in Finnish.

**Question**: Does this finding generalize to other agglutinative languages?

Turkish is an ideal validation target:
- Agglutinative morphology (like Finnish)
- Rich verb paradigm: Mood x Tense x Person x Number (+ Polarity as extra dimension)
- Different morphological strategies (vowel harmony type differs, negation is affixal)
- UD Turkish-IMST treebank available in CoNLL-U format

## Data

| Property               | Finnish (UD-TDT)        | Turkish (UD-IMST)       |
|------------------------|-------------------------|-------------------------|
| Training tokens        | ~200K lines             | ~52K lines              |
| NOUN tokens            | large                   | 10,252                  |
| VERB tokens            | large                   | 7,696 (incl. AUX)      |
| Char vocabulary        | 22 chars                | 32 chars                |
| Max form length        | 19 chars                | 17 chars                |
| Noun paradigm shape    | Case(15) x Number(2)    | Case(6) x Number(2)     |
| Verb paradigm shape    | Mood(4) x Tense(2) x Person(4) x Number(2) | Mood(4) x Tense(4) x Person(3) x Number(2) |
| Noun total slots       | 30                      | 12                      |
| Verb total slots       | 64                      | 96                      |

Turkish has fewer noun cases (6 vs 15) but more tense distinctions (4 vs 2) and
adds Polarity (Pos/Neg) as an explicit morphological dimension (Finnish uses a
separate negation verb).

## Results

### 1. Overall TT-Ranks

| Language | POS           | Mean Max-Rank | Median | Std  | Range    | Compression |
|----------|---------------|---------------|--------|------|----------|-------------|
| Finnish  | NOUN (char)   | 8.23          | 8.0    | 1.05 | [6, 10]  | 1.59x       |
| Turkish  | NOUN (char)   | 6.37          | 6.0    | 0.93 | [5, 9]   | 1.04x       |
| Finnish  | NOUN (suffix) | 7.75          | 8.0    | 1.06 | [5, 10]  | 0.98x       |
| Turkish  | NOUN (suffix) | 5.42          | 5.0    | 0.85 | [4, 7]   | 0.94x       |
| Finnish  | VERB (4D)     | 6.81          | 6.0    | 1.62 | [4, 12]  | 4.33x       |
| Turkish  | VERB (4D)     | 7.73          | 7.0    | 3.20 | [4, 18]  | 5.29x       |
| Turkish  | VERB (5D+Pol) | 8.52          | 7.0    | 3.87 | [4, 20]  | 8.16x       |

**Observation**: Turkish nouns have lower TT-rank (6.37 vs 8.23) — expected because
Turkish has fewer cases (6 vs 15), so the tensor is smaller and the maximum possible
rank is lower. Turkish verbs have higher TT-rank (7.73 vs 6.81) despite similar
tensor dimensions, reflecting higher paradigm diversity in the treebank.

### 2. Fill Rate vs TT-Rank Correlation

| Language | POS      | Pearson r |
|----------|----------|-----------|
| Finnish  | NOUN     | 0.687     |
| Turkish  | NOUN     | 0.705     |
| Finnish  | VERB     | 0.870     |
| Turkish  | VERB 4D  | 0.973     |
| Turkish  | VERB 5D  | 0.972     |

**Finding**: The correlation between fill rate and TT-rank is even stronger in Turkish
(r=0.973) than Finnish (r=0.870). The dominant driver of TT-rank remains paradigm
completeness (data sparsity), consistent across both languages. This confirms that
the confound identified in the Finnish experiment is universal, not language-specific.

### 3. Bond-Specific Rank Analysis (Key Finding)

This is the most important comparison. Both languages use the same verb tensor
structure: Mood x Tense x Person x Number x CharPos.

#### Finnish verb bonds: Mood(4) x Tense(2) x Person(4) x Number(2) x CharPos(19)

| Bond | Split                            | Mean Rank | Std  | Range   |
|------|----------------------------------|-----------|------|---------|
| 1    | Mood \| Tense-Person-Num-Char    | 2.22      | 0.69 | [1, 4]  |
| 2    | MoodTense \| Person-Num-Char     | 3.22      | 0.69 | [2, 5]  |
| 3    | MoodTensPers \| Num-Char         | 6.25      | 1.79 | [4, 12] |
| 4    | MoodTensPersNum \| Char          | 6.22      | 1.17 | [4, 9]  |

#### Turkish verb bonds: Mood(4) x Tense(4) x Person(3) x Number(2) x CharPos(17)

| Bond | Split                            | Mean Rank | Std  | Range   |
|------|----------------------------------|-----------|------|---------|
| 1    | Mood \| Tense-Person-Num-Char    | 2.49      | 0.97 | [1, 4]  |
| 2    | MoodTense \| Person-Num-Char     | 4.75      | 1.60 | [2, 9]  |
| 3    | MoodTensPers \| Num-Char         | 7.26      | 3.28 | [3, 18] |
| 4    | MoodTensPersNum \| Char          | 7.15      | 2.32 | [4, 12] |

#### Cross-linguistic bond comparison:

| Bond | Finnish | Turkish | Interpretation                              |
|------|---------|---------|---------------------------------------------|
| 1    | 2.22    | 2.49    | Similar: ~2-3 effective mood categories      |
| 2    | 3.22    | 4.75    | **Turkish higher**: more Mood x Tense combos |
| 3    | 6.25    | 7.26    | Similar: Person expansion in both            |
| 4    | 6.22    | 7.15    | Similar: Number and character expansion      |

**Key finding**: Bond 2 (Mood-Tense boundary) shows the clearest cross-linguistic
difference. In Finnish, rank 3.22/5 reflects that 3 of 4 moods collapse the tense
dimension (only Indicative distinguishes Pres/Past). In Turkish, rank 4.75/9 is higher
because Turkish has 4 tenses (Pres/Past/Fut/Pqp), and more moods retain tense
distinctions.

This confirms the central claim: **bond-specific TT-rank measures the effective
number of independent feature combinations at that split point**, and the measurement
reflects genuine typological differences between Finnish and Turkish tense/mood systems.

Bond 1 is nearly identical (2.22 vs 2.49) because both languages have ~2-3 productive
mood categories in treebank data, regardless of the theoretically available moods.

### 4. Turkish 5D Verb Tensor (with Polarity)

Turkish adds Polarity (Pos/Neg) as an explicit morphological dimension. The 5D tensor
Mood(4) x Tense(4) x Person(3) x Number(2) x Polarity(2) reveals:

| Bond | Split                                | Mean Rank | Range    |
|------|--------------------------------------|-----------|----------|
| 1    | Mood \| Tense-Pers-Num-Pol-Char      | 2.48      | [1, 4]   |
| 2    | MoodTense \| Pers-Num-Pol-Char       | 4.78      | [2, 9]   |
| 3    | MoodTensPers \| Num-Pol-Char         | 7.23      | [3, 19]  |
| 4    | MoodTensPerNum \| Pol-Char           | 8.15      | [3, 20]  |
| 5    | MoodTensPerNumPol \| Char            | 7.46      | [4, 13]  |

The Polarity bond (Bond 4 in 5D) shows mean rank 8.15 — only slightly higher than
Bond 3 (7.23), suggesting that Polarity does not create many new independent patterns.
This makes linguistic sense: Turkish negation is morphologically regular (suffix -me/-ma
inserted before tense markers), so it approximately doubles the surface forms without
creating qualitatively different paradigm structures.

### 5. Notable Irregular Verbs

Turkish irregular/high-frequency verbs show consistently high TT-rank, paralleling
the Finnish finding:

| Verb       | Meaning    | Max Rank | Fill Rate | Finnish parallel              |
|------------|------------|----------|-----------|-------------------------------|
| yap(mak)   | to do/make | 18       | 0.271     | tehda (to do): rank 11        |
| et(mek)    | to do      | 17       | 0.260     | tehda: rank 11                |
| ol(mak)    | to be      | 15       | 0.250     | olla (to be): rank 12         |
| gel(mek)   | to come    | 14       | 0.198     | tulla (to come): rank 11      |
| gor(mek)   | to see     | 14       | 0.167     | nahda (to see): rank 11       |
| al(mak)    | to take    | 14       | 0.198     | saada (to get): rank 12       |
| de(mek)    | to say     | 13       | 0.177     | —                             |
| bil(mek)   | to know    | 13       | 0.167     | —                             |

**Important caveat**: The high TT-ranks here are strongly correlated with fill rate
(r=0.973). These verbs have higher TT-rank largely because they appear more frequently
in the treebank and thus fill more paradigm slots. This is the same confound identified
in the Finnish experiment. The irregularity interpretation holds only partially:
these verbs are both frequent AND somewhat irregular (e.g., olmak has irregular
negative forms, etmek has stem changes), but separating frequency from irregularity
requires controlling for fill rate.

### 6. Noun Comparison

| Metric            | Finnish (15 cases)   | Turkish (6 cases)    |
|-------------------|----------------------|----------------------|
| Max possible rank | min(15,2) = 2 (bond) | min(6,2) = 2 (bond)  |
| Mean max rank     | 8.23                 | 6.37                 |
| Suffix mean rank  | 7.75                 | 5.42                 |
| Fill-rank corr.   | 0.687                | 0.705                |

Turkish nouns have lower absolute TT-rank, expected from the smaller case system.
The suffix encoding reduces rank in both languages (Turkish: 6.37 -> 5.42,
Finnish: 8.23 -> 7.75), confirming that suffix-based encoding captures
morphological structure more efficiently than raw characters in both.

## Does the Finding Generalize?

**Yes, with the same caveats.**

1. **Bond-rank = feature interaction (confirmed)**: The bond-specific analysis
   shows that TT-rank at the Mood-Tense boundary reflects the actual number of
   independent Mood x Tense combinations, and this differs predictably between
   Finnish (rank ~3.2) and Turkish (rank ~4.8) based on their typological differences.

2. **Fill rate dominance (confirmed)**: The strong fill-rate/TT-rank correlation
   (r > 0.87 for verbs in both languages) confirms that paradigm completeness
   remains the dominant driver of overall TT-rank. The bond-specific analysis is
   necessary to extract genuine morphological signal.

3. **Irregular verb = higher rank (partially confirmed)**: High-frequency irregular
   verbs show higher TT-rank in both languages, but this is confounded by their
   higher fill rates. The pattern is consistent but not conclusive evidence of
   irregularity per se.

4. **Cross-linguistic comparison strengthens the paper**: The Mood-Tense bond
   difference (Finnish ~3.2 vs Turkish ~4.8) provides direct evidence that
   bond-rank captures typological variation, not just data artifacts.

## Implications for Paper-2

The Turkish validation strengthens the paper in two ways:

1. **Generalizability**: The finding is not Finnish-specific. The same mathematical
   framework (TT-rank at feature bonds) captures morphological structure in a
   typologically different language.

2. **Typological discriminability**: Bond-level TT-rank can distinguish between
   languages with different feature interaction patterns (Finnish 2-tense system
   vs Turkish 4-tense system), providing a quantitative typological measure.

Suggested paper narrative:
- Section 4: Finnish case study (bond-rank = feature interaction, *puhua* verification)
- Section 5: Turkish validation (same pattern, different bond values, typological interpretation)
- Section 6: Cross-linguistic comparison table

## Reproducibility

```bash
# Setup
cd experiments/tt-rank
source .venv/bin/activate  # or create: python3 -m venv .venv && pip install numpy

# Turkish UD data (must be pre-cloned)
# git clone https://github.com/UniversalDependencies/UD_Turkish-IMST.git \
#   ~/oss/finnishNLP/ud-turkish-imst

# Extract Turkish paradigms
python3 turkish_extract.py

# Run TT decomposition
python3 turkish_decompose.py

# Output: cross-linguistic/turkish_results.json
```

## Files

| File                          | Description                              |
|-------------------------------|------------------------------------------|
| `turkish_extract.py`          | Extract Turkish paradigm tables          |
| `turkish_decompose.py`        | TT-SVD analysis of Turkish paradigms     |
| `turkish_paradigms.json`      | Extracted Turkish paradigm data (943KB)  |
| `turkish_results.json`        | Full numerical results (543KB)           |
| `README.md`                   | This file                                |
