# Third-Party Notices

MCE uses data from the following third-party sources.

## Important License Notice

The dictionary file `data/mor.vfst` is derived from Voikko/HFST and is licensed
under **GPL-3.0**. This file is NOT covered by MCE's Apache-2.0 license.

- The MCE engine code (all Rust crates) is Apache-2.0
- The `mor.vfst` dictionary is GPL-3.0 (separate work, loaded at runtime)
- Users must comply with GPL-3.0 when distributing `mor.vfst`

See: https://voikko.puimula.org/ for the original Voikko project.

## Universal Dependencies Finnish-TDT

- **Source**: https://github.com/UniversalDependencies/UD_Finnish-TDT
- **License**: CC BY-SA 4.0 (Creative Commons Attribution-ShareAlike 4.0 International)
- **Usage**: Training data for suffix tagger model (`data/suffix_tagger.bin`) and lemma dictionary (`data/lemma_dict.tsv`)
- **Citation**: Haverinen et al. (2014). "The TDT treebank." In *Proceedings of LREC*.

## Universal Dependencies Finnish-OOD

- **Source**: https://github.com/UniversalDependencies/UD_Finnish-OOD
- **License**: CC BY-SA 4.0 (Creative Commons Attribution-ShareAlike 4.0 International)
- **Usage**: Additional lemma entries in `data/lemma_dict.tsv` (out-of-domain Finnish text)
- **Citation**: See the repository for full citation details.

## Universal Dependencies Finnish-PUD

- **Source**: https://github.com/UniversalDependencies/UD_Finnish-PUD
- **License**: CC BY-SA 4.0 (Creative Commons Attribution-ShareAlike 4.0 International)
- **Usage**: Additional lemma entries in `data/lemma_dict.tsv` (parallel universal dependencies)
- **Citation**: See the repository for full citation details.

## Voikko Finnish Dictionary

- **Source**: https://voikko.puimula.org/ via [corevoikko](https://github.com/yongsk0066/corevoikko)
- **License**: GPL-3.0
- **Usage**: Morphological FST dictionary (`data/mor.vfst`)

---

The full text of the CC BY-SA 4.0 license is available at:
https://creativecommons.org/licenses/by-sa/4.0/legalcode
