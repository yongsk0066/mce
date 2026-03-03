# mce-cli

Command-line interface for the MCE Finnish NLP engine. Provides 11 subcommands for morphological analysis, spell checking, grammar checking, hyphenation, UD evaluation, and benchmarking.

## Setup

```bash
export MCE_DICT_PATH=~/oss/corevoikko/voikko-fi/vvfst
```

The dictionary directory must contain `mor.vfst`.

## Subcommands

### analyze -- morphological analysis

```bash
mce-cli analyze koira
# koira: 1 analysis(es)
#   [1] nimisana, BASEFORM=koira, STRUCTURE==ppppp
```

### spell -- spell checking with suggestions

```bash
mce-cli spell koirra
# koirra: MISSPELLED
#   Suggestions (d<=1): koira
```

### compound -- compound word splitting

```bash
mce-cli compound rautatieasema
# rautatieasema: compound word (1 split(s))
#   Split 1 (penalty 30): rauta + tie + asema
```

### sentence -- tokenize, analyze, disambiguate

```bash
mce-cli sentence "Koira juoksee nopeasti"
# [1] Koira -> nimisana (BASEFORM=koira) [1/2 readings]
# [2] juoksee -> teonsana (BASEFORM=juosta) [1/3 readings]
# [3] nopeasti -> seikkasana (BASEFORM=nopeasti)
```

### grammar -- grammar checking

```bash
mce-cli grammar "Koira koira juoksee pihalla."
# Error at 6..11: REPEATED_WORD
#   "koira" -- Repeated word: koira
#   Suggestion: koira
```

### hyphenate -- hyphenate individual words

```bash
mce-cli hyphenate suomalainen rautatieasema
# suomalainen -> suo-ma-lai-nen
# rautatieasema -> rau-ta-tie-a-se-ma
```

### hyphenate-text -- hyphenate running text

```bash
mce-cli hyphenate-text "Koira juoksee nopeasti."
# Koi-ra juok-see no-pe-as-ti.
```

### generate -- morphological generation (nouns and verbs)

```bash
# Noun paradigm (11 cases)
mce-cli generate koira --all

# Single noun case
mce-cli generate koira --case partitive

# Verb paradigm
mce-cli generate --verb juosta --all

# Single verb form
mce-cli generate --verb juosta --tense present --person 3sg
```

### info -- dictionary metadata

```bash
mce-cli info
# MCE Dictionary Info
# File size:        3801234 bytes (3.6 MB)
# Total symbols:    542
```

### eval -- evaluate against UD treebank

```bash
# Basic evaluation
mce-cli eval --conllu fi_tdt-ud-dev.conllu

# With corpus-trained bigrams
mce-cli eval --conllu fi_tdt-ud-dev.conllu --train fi_tdt-ud-train.conllu

# With suffix tagger model
mce-cli eval --conllu fi_tdt-ud-dev.conllu --model data/suffix_tagger.bin

# JSON output
mce-cli eval --conllu fi_tdt-ud-dev.conllu --format json
```

### benchmark -- performance benchmarking

```bash
mce-cli benchmark --iterations 500
```

## Dependencies

Uses: `mce-core`, `mce-fst`, `mce-tokenizer`, `mce-speller`, `mce-disambig`, `mce-comonad`, `mce-fi`, `mce-grammar`, `mce-eval`
