# Kotus Word List Integration Plan

> **Date**: 2026-03-04
> **Status**: Research document (does not modify implementation code)
> **Audience**: Project maintainer, contributors
> **Scope**: Integration of the Kotus Nykysuomen sanalista into MCE speller and lemma pipeline
> **Decision**: GO (approved in Session 11 Tier 2 assessment)

---

## Table of Contents

1. [Kotus Data Overview](#1-kotus-data-overview)
2. [Data Versions and Format](#2-data-versions-and-format)
3. [POS Mapping Table](#3-pos-mapping-table)
4. [Inflection Class to POS Mapping](#4-inflection-class-to-pos-mapping)
5. [Integration Architecture Recommendation](#5-integration-architecture-recommendation)
6. [Quality Validation Methodology](#6-quality-validation-methodology)
7. [Implementation Steps](#7-implementation-steps)
8. [Expected Impact Metrics](#8-expected-impact-metrics)
9. [Licensing and Attribution](#9-licensing-and-attribution)
10. [Risks and Mitigations](#10-risks-and-mitigations)
11. [References](#11-references)

---

## 1. Kotus Data Overview

### Source

The **Nykysuomen sanalista** (Contemporary Finnish Word List) is published by
**Kotimaisten kielten keskus** (Kotus), the Institute for the Languages of
Finland. It is derived from the headwords of the *Kielitoimiston sanakirja*
(Dictionary of Contemporary Finnish) and is updated regularly alongside
dictionary revisions.

### Key Facts

| Property | Value |
|----------|-------|
| Publisher | Kotimaisten kielten keskus (Kotus) |
| Name | Nykysuomen sanalista |
| Entries (v1 XML, 2006) | 94,110 lemmas |
| Entries (2024 CSV) | 100,000+ lemmas |
| License | CC BY 4.0 (2024 version) |
| License (v1 XML) | GNU LGPL / EUPL v1.1 / CC BY 3.0 |
| Encoding | UTF-8 |
| Update frequency | Every 1-2 years |
| Download (2024 CSV) | `https://kaino.kotus.fi/lataa/nykysuomensanalista2024.csv` |
| Download (2024 TXT) | `https://kaino.kotus.fi/lataa/nykysuomensanalista2024.txt` |
| Download (v1 XML) | `https://kaino.kotus.fi/sanat/nykysuomi/` |
| Kielipankki mirror | `urn:nbn:fi:lb-2021092006` |
| Last updated | April 11, 2025 (based on March 19, 2024 dictionary update) |

### Content Scope

The list covers general Finnish vocabulary including widely used specialized
terminology, stylistically marked words, and dialectal/slang terms. It is
explicitly *not* normative and does not claim to be exhaustive. Compound words
are typically listed only as the lemma (headword), not with decomposition.

---

## 2. Data Versions and Format

### Version 1: XML (2006, legacy)

The original release used a custom XML format with DTD schema.

**Root element**: `<kotus-sanalista>`

**Entry structure**:
```xml
<st>
  <s>aallokas</s>            <!-- lemma (headword) -->
  <hn>1</hn>                 <!-- homonym number (optional) -->
  <t>                        <!-- inflection block (0 or more) -->
    <tn>41</tn>              <!-- inflection class number (1-78, 99) -->
    <av>A</av>               <!-- consonant gradation type (A-M, optional) -->
  </t>
</st>
```

**DTD elements**:

| Element | Content | Description |
|---------|---------|-------------|
| `<st>` | `(s, hn?, t*)` | Word entry (sanatietue) |
| `<s>` | `#PCDATA` | Headword (sana) |
| `<hn>` | `#PCDATA` | Homonym number (homonyyminumero) |
| `<t>` | `(tn, av?)` | Inflection data (taivutustiedot) |
| `<tn>` | `#PCDATA` | Inflection class number (taivutusnumero) |
| `<av>` | `#PCDATA` | Consonant gradation type (astevaihtelutiedot) |

**Attributes on `<t>`**:
- `taivutus`: `"harvinainen"` (rare), `"mahdollinen"` (possible), `"yksikössä"` (singular only), `"monikossa"` (plural only)

**Attributes on `<av>`**:
- `astevaihtelu`: `"valinnainen"` (optional gradation)

**Notable**: The v1 XML does NOT include an explicit word class (sanaluokka)
field. Word class must be inferred from the inflection class number (see
Section 4).

### Version 2024: CSV (current)

The 2024 version uses CSV format with explicit word class information.

**Columns** (4 fields):

| Column | Finnish | Description | Example |
|--------|---------|-------------|---------|
| 1 | Hakusana | Headword (lemma) | `aallokas` |
| 2 | Homonymia | Homonym number | `1` (or empty) |
| 3 | Sanaluokka | Word class | `adjektiivi` |
| 4 | Taivutustiedot | Inflection data | `41*A` |

**Example entries**:
```
3D-tulostin,,substantiivi,41
aallokas,,adjektiivi,41*A
aallokko,,substantiivi,4*A
aallota,,verbi,75*I
aalto,,substantiivi,1*I
```

The inflection data in the 2024 CSV combines the inflection class number and
gradation type with an asterisk separator (e.g., `41*A` means class 41 with
gradation type A).

### Recommendation: Use the 2024 CSV

The 2024 CSV is strongly preferred because:

1. **Explicit word class**: No need to infer POS from inflection numbers
2. **Larger**: 100K+ entries vs 94K in v1
3. **More current**: Based on March 2024 dictionary update
4. **Better license**: CC BY 4.0 (simpler than v1's triple license)
5. **Simpler parsing**: CSV vs XML

---

## 3. POS Mapping Table

### Kotus Word Classes to UD UPOS Tags

| Kotus sanaluokka | Finnish term | UD UPOS | Notes |
|-------------------|-------------|---------|-------|
| `substantiivi` | nimisana | `NOUN` | Largest class (~60-70% of entries) |
| `adjektiivi` | laatusana | `ADJ` | Second largest (~10-15%) |
| `verbi` | teonsana | `VERB` | Third largest (~10-15%) |
| `pronomini` | asemosana | `PRON` | Small set; no inflection class in v1 |
| `numeraali` | lukusana | `NUM` | Cardinal and ordinal numerals |
| `adverbi` | seikkasana | `ADV` | Particles (uninflected) |
| `prepositio` | etuliite | `ADP` | Prepositions (rare in Finnish) |
| `postpositio` | jälkiliite | `ADP` | Postpositions (more common in Finnish) |
| `konjunktio` | konjunktio | `CCONJ` / `SCONJ` | Requires disambiguation |
| `interjektio` | huudahdussana | `INTJ` | Exclamations |
| *(no class)* | yhdyssanan alkuosa | `NOUN` (tentative) | Compound-initial elements without class |

### Edge Cases and Exceptions

1. **`konjunktio` split**: Finnish does not distinguish coordinating vs.
   subordinating in the Kotus system. For MCE integration, mapping to `CCONJ`
   is the safe default; individual disambiguation can be applied for known
   subordinating conjunctions (e.g., `että`, `koska`, `kun`, `jos`, `vaikka`).

2. **`prepositio` vs `postpositio`**: Both map to UD `ADP`. The distinction
   matters for syntax but not for speller integration.

3. **Pronouns without inflection class**: In the v1 XML, pronouns lack
   inflection numbers because they have irregular paradigms. The 2024 CSV may
   handle this differently.

4. **Entries without word class**: Some entries in the list are compound-initial
   elements (e.g., prefixoid morphemes) that lack word class. These should be
   included in the speller wordlist but excluded from lemma_dict POS mapping.

5. **Multiple inflection blocks**: Some entries have two or more `<t>` blocks
   (alternative inflections). The 2024 CSV encodes this as multiple rows or
   combined notation. For speller purposes, only the lemma matters.

---

## 4. Inflection Class to POS Mapping

For the legacy v1 XML (which lacks explicit word class), POS can be inferred
from the inflection class number:

| Class Range | POS | Description | Example Model Word |
|-------------|-----|-------------|--------------------|
| 1-51 | Nominal (NOUN/ADJ/PRON/NUM) | Declension types | 1=valo, 38=nainen, 41=vieras |
| 52-78 | VERB | Conjugation types | 52=sanoa, 62=voida, 71=nähdä |
| 99 | Indeclinable | No inflection | Particles, adverbs, etc. |
| *(none)* | N/A | Compound-initial | No `<t>` block at all |

### Nominal Subclass Discrimination (Classes 1-51)

Distinguishing NOUN from ADJ within classes 1-51 is NOT possible from the
inflection class alone -- both nouns and adjectives share the same declension
classes. For example, class 41 includes both `aallokas` (ADJ) and `vieras`
(NOUN). This is a key reason why the 2024 CSV (with explicit word class) is
preferred.

### Consonant Gradation Types

| Code | Strong : Weak | Example |
|------|---------------|---------|
| A | kk : k | takki : takin |
| B | pp : p | kaappi : kaapin |
| C | tt : t | tytti : tytön |
| D | k : - | puku : puvun |
| E | p : v | apu : avun |
| F | t : d | katu : kadun |
| G | nk : ng | renki : rengin |
| H | mp : mm | kampa : kamman |
| I | lt : ll | kulta : kullan |
| J | nt : nn | ranta : rannan |
| K | rt : rr | parta : parran |
| L | k : j | aika : ajan |
| M | k : v | poika : pojan |

These gradation codes are useful for morphological generation but are not
needed for speller wordlist integration (which only needs lemmas).

---

## 5. Integration Architecture Recommendation

### Current MCE Data Pipeline

```
data/lemma_dict.tsv  (48K entries, 1.3MB)
  -> LemmaDict (mce-eval)       : (form, UPOS) -> lemma mapping for eval
  -> load_wordlist (mce-wasm)    : forms + lemmas -> SuccinctTrie for speller

data/wordlist.txt    (44K entries, 500KB)
  -> load_wordlist (mce-wasm)    : words -> SuccinctTrie for speller
```

### Proposed Architecture: Option C (Both)

**Recommendation: Option C** -- use Kotus data for *both* lemma_dict enrichment
and speller wordlist expansion, with separate processing pipelines.

#### Option A: Extend lemma_dict.tsv only

- Merge Kotus lemmas + POS into the existing TSV format
- Pros: Single file, reuses existing infrastructure
- Cons: lemma_dict stores `(inflected_form, UPOS) -> lemma` mappings, not
  bare lemmas; Kotus data only provides lemmas (base forms), not inflected
  forms. Direct merge would require a different entry format.
- **Verdict**: Partial fit. Kotus entries would be identity mappings
  (`koira → NOUN → koira`) which the current LemmaDict intentionally omits.

#### Option B: Separate speller dictionary only

- Add Kotus lemmas to wordlist.txt (one word per line)
- Pros: Simple, directly improves speller coverage
- Cons: No POS information preserved, no lemma improvement
- **Verdict**: Good for speller, misses lemma opportunity.

#### Option C: Both (RECOMMENDED)

**Speller path**: Merge Kotus lemmas into `data/wordlist.txt`, expanding from
44K to an estimated 80-90K unique entries (after dedup with existing words).
This directly feeds the SuccinctTrie via `load_wordlist()`.

**Lemma path**: Create a new `data/kotus_lemmas.tsv` file with
`lemma<TAB>UPOS` pairs. This can be loaded as a supplementary dictionary:
- At eval time: use as a fallback when FST returns no baseform and the UD
  lemma_dict has no match
- At runtime: validate that OOV words which match a Kotus lemma are real words
  (improves `is_valid_word()`)

**Why Option C**: The speller and the lemmatizer have different needs. The
speller needs the broadest possible word coverage (all lemmas as valid
dictionary words). The lemmatizer needs `(form, POS) -> lemma` mappings, which
Kotus provides only for identity cases. By splitting, we maximize the value of
both data channels without compromising either.

### Integration Flow

```
                    kotus-2024.csv
                        |
              +---------+---------+
              |                   |
         [extract.py]        [extract.py]
              |                   |
    data/wordlist.txt     data/kotus_lemmas.tsv
    (lemmas only,         (lemma + UPOS pairs)
     one per line)
              |                   |
    load_wordlist()        LemmaDict supplement
    -> SuccinctTrie        -> OOV validation
    -> SpellChecker        -> lemma fallback
```

### File Format: kotus_lemmas.tsv

```tsv
# Kotus Nykysuomen sanalista 2024, CC BY 4.0
# Source: https://kaino.kotus.fi/lataa/nykysuomensanalista2024.csv
# Format: lemma<TAB>UPOS
aallokas	ADJ
aallokko	NOUN
aallota	VERB
aalto	NOUN
...
```

---

## 6. Quality Validation Methodology

### 6.1 Cross-Reference with VFST

Run every Kotus lemma through the MCE VFST analyzer and categorize results:

| Category | Description | Action |
|----------|-------------|--------|
| **VFST-match** | VFST recognizes the lemma | Confirm: word is already covered |
| **VFST-miss** | VFST does NOT recognize | Speller improvement: add to wordlist |
| **VFST-different-POS** | VFST recognizes but with different POS | Log for manual review |

Expected outcome: VFST covers ~70-80% of Kotus lemmas (common vocabulary
is well-represented in the Voikko dictionary). The remaining 20-30% represent
genuine speller coverage improvement.

### 6.2 Conflict Detection with Existing lemma_dict

Check for entries where Kotus and UD treebank lemma_dict disagree:

```
For each Kotus (lemma, UPOS) pair:
  If lemma_dict contains (lemma, UPOS, different_lemma):
    -> Flag as conflict (shouldn't happen since Kotus provides base forms)
  If lemma_dict contains a form that lemmatizes TO this lemma:
    -> Confirm: Kotus validates the UD lemma
  If lemma_dict does NOT contain any reference to this lemma:
    -> New coverage: add to speller
```

### 6.3 Duplicate Detection

Compare Kotus lemmas against the existing wordlist.txt:

```bash
# Pseudocode for overlap analysis
kotus_lemmas = extract_lemmas(kotus_2024.csv)
wordlist = load(data/wordlist.txt)
lemma_dict_forms = extract_col1(data/lemma_dict.tsv)
lemma_dict_lemmas = extract_col3(data/lemma_dict.tsv)

overlap_wordlist = kotus_lemmas & wordlist
overlap_lemma_dict = kotus_lemmas & (lemma_dict_forms | lemma_dict_lemmas)
new_entries = kotus_lemmas - wordlist - lemma_dict_forms - lemma_dict_lemmas
```

### 6.4 Verb Validation

Special focus on verbs, where MCE currently has 10,942 verb entries in
lemma_dict but Kotus likely provides additional verb lemmas not seen in the UD
treebank training data:

1. Extract all Kotus `verbi` entries
2. Cross-reference with MCE's existing verb lemma set
3. For new verbs: test VFST recognition of their inflected forms
4. Identify verbs that the VFST-based generator (`generate_verb_form`,
   `generate_verb_paradigm`) cannot conjugate

### 6.5 Regression Testing

After integration, verify no degradation:

- Run `mce-eval` pipeline: UPOS must remain >= 94.0% (CI threshold)
- Run speller tests: no false negatives on previously-correct words
- WASM binary size: must remain <= 390KB budget
- Deploy size: wordlist size increase is acceptable (loaded via `load_wordlist`)

---

## 7. Implementation Steps

### Phase 1: Data Acquisition and Parsing (Effort: 1-2 hours)

1. **Download** the 2024 CSV from Kotus
   - Primary: `https://kaino.kotus.fi/lataa/nykysuomensanalista2024.csv`
   - Fallback: Use v1 XML from GitHub mirror + parse with inflection class POS inference
   - Note: Cloudflare protection on kaino.kotus.fi may require browser download

2. **Create extraction script**: `scripts/extract_kotus.py`
   - Parse CSV (handle encoding, quoting)
   - Map Kotus word classes to UD UPOS tags (table from Section 3)
   - Handle edge cases: entries without word class, multiple homonyms
   - Output 1: `kotus_lemmas_raw.tsv` (all entries with POS)
   - Output 2: statistics report (counts per POS, duplicates, issues)

### Phase 2: Quality Analysis (Effort: 2-3 hours)

3. **Run overlap analysis** against existing MCE data
   - Cross-reference with wordlist.txt (44K entries)
   - Cross-reference with lemma_dict.tsv (48K entries, 21K unique lemmas)
   - Run VFST coverage check on all Kotus lemmas

4. **Generate quality report**
   - Number of new lemmas not in any existing MCE data source
   - Number of VFST misses (= direct speller improvement)
   - Conflict cases (if any)
   - Breakdown by POS

### Phase 3: Speller Integration (Effort: 1-2 hours)

5. **Merge into wordlist.txt**
   - Add Kotus lemmas to data/wordlist.txt
   - Deduplicate and sort
   - Expected growth: 44K -> ~80-90K entries
   - File size increase: ~500KB -> ~900KB-1MB

6. **Create kotus_lemmas.tsv** (if Option C pursued)
   - Format: `lemma<TAB>UPOS`
   - Only include entries NOT already covered by lemma_dict.tsv
   - Store as supplementary reference (not loaded at runtime initially)

### Phase 4: Testing and Validation (Effort: 2-3 hours)

7. **Run full test suite**
   ```bash
   cargo fmt --all --check
   cargo test --all-features
   cargo clippy --all-features -- -D warnings
   ```

8. **Run evaluation pipeline**
   ```bash
   cargo run --release -p mce-eval -- \
     --dict data/lemma_dict.tsv \
     --model data/suffix_tagger.bin \
     eval --corpus vendor/ud-finnish-tdt/fi_tdt-ud-test.conllu
   ```

9. **Verify WASM and deploy size**
   - WASM binary: should not change (wordlist is loaded separately)
   - Deploy: wordlist.txt size increase (~500KB) is within budget

### Phase 5: Documentation and Attribution (Effort: 30 min)

10. **Update THIRD_PARTY_NOTICES.md** (or create if not present)
    - Add CC BY 4.0 attribution for Kotus Nykysuomen sanalista 2024
    - Include the required credit line

11. **Update CHANGELOG.md**
    - Document the wordlist expansion and Kotus integration

12. **Update data/README.md** or similar documentation
    - Document the provenance of wordlist.txt entries

### Total Estimated Effort: 7-10 hours

---

## 8. Expected Impact Metrics

### Speller Coverage Improvement

| Metric | Before | After (estimated) | Change |
|--------|--------|--------------------|--------|
| wordlist.txt entries | 44,485 | ~80,000-90,000 | +80-100% |
| wordlist.txt file size | 500KB | ~900KB-1MB | +400-500KB |
| Speller dictionary coverage | Baseline | +35-45K new lemmas | Significant |
| `is_valid_word()` coverage | VFST-only + 44K | VFST + 80-90K | Better |

### WASM and Performance Impact

| Metric | Before | After | Impact |
|--------|--------|-------|--------|
| WASM binary | 365KB | 365KB | No change (wordlist loaded separately) |
| Deploy size (total) | ~9.2MB | ~9.6-9.7MB | +400-500KB |
| Deploy size (gzipped) | ~2-3MB | ~2.1-3.2MB | +100-200KB (text compresses well) |
| `load_wordlist()` time | Baseline | ~1.5-2x | Larger trie build |
| Trie memory usage | Baseline | ~1.5-2x | More nodes in succinct trie |
| `suggest()` latency | Baseline | Negligible change | Trie lookup is O(key length) |
| UPOS accuracy | 94.58% | 94.58% | No change (trie is not used for POS) |
| Lemma accuracy | 88.44% | 88.44% | No change (lemma_dict unchanged initially) |

### Qualitative Improvements

1. **Broader spell checking**: Common Finnish words not in UD treebanks will
   be recognized as valid, reducing false-positive spelling errors
2. **Better suggestions**: More candidate words in the trie means more
   relevant suggestions for misspelled words
3. **Verb validation**: Additional verb lemmas enable better validation of
   conjugated forms through VFST morphological analysis
4. **Professional vocabulary**: Kotus includes specialized terms from the
   Kielitoimiston sanakirja that UD treebanks may underrepresent

---

## 9. Licensing and Attribution

### License: CC BY 4.0 (2024 version)

The Creative Commons Attribution 4.0 International license requires:

1. **Attribution**: Credit must be given to the creator
2. **No additional restrictions**: Cannot apply legal/technological measures
   that restrict others from doing anything the license permits
3. **Commercial use**: Allowed
4. **Modifications**: Allowed (with attribution)
5. **Distribution**: Allowed (with attribution)

### Required Attribution

Add to `THIRD_PARTY_NOTICES.md` (or create the file):

```
## Nykysuomen sanalista 2024

Source: Kotimaisten kielten keskus (Kotus)
        Institute for the Languages of Finland
URL: https://kaino.kotus.fi/lataa/nykysuomensanalista2024.csv
License: Creative Commons Attribution 4.0 International (CC BY 4.0)
         https://creativecommons.org/licenses/by/4.0/

The Contemporary Finnish Word List (Nykysuomen sanalista) is based on
the headwords of the Kielitoimiston sanakirja (Dictionary of Contemporary
Finnish), published by Kotimaisten kielten keskus.
```

### Compatibility with Existing Licenses

- MCE project: MIT (compatible with CC BY 4.0)
- UD treebank data already in project: CC-BY-SA 4.0 (more restrictive than CC BY 4.0)
- Kotus CC BY 4.0 is strictly less restrictive than CC-BY-SA 4.0, so no new
  license obligations are introduced beyond what already exists

---

## 10. Risks and Mitigations

### R1: Cloudflare Protection on Download

**Risk**: The kaino.kotus.fi server uses Cloudflare challenge pages, blocking
automated downloads (curl/wget).

**Mitigation**: Download manually via browser, or use the v1 XML from GitHub
mirrors (pulmark/finnish-dictionary, hugovk/everyfinnishword) as a fallback.
The v1 XML has 94K entries and can be parsed with inflection-class-based POS
inference.

### R2: POS Ambiguity in Compound-Initial Elements

**Risk**: Some Kotus entries lack word class (compound-initial parts). Adding
these to the speller without POS could cause false positives.

**Mitigation**: Include in wordlist.txt for speller coverage (no POS needed for
spell checking), but exclude from kotus_lemmas.tsv (which requires POS).

### R3: Inflection Data Precision

**Risk**: The 2024 CSV format for inflection data (`41*A`) may differ from
expectations if the CSV uses different quoting or delimiter conventions.

**Mitigation**: Parse the inflection field conservatively. For speller
integration, only the lemma column is needed; inflection data is a bonus for
future morphological generation improvements.

### R4: Trie Memory Growth

**Risk**: Doubling the wordlist from 44K to ~90K entries could increase the
SuccinctTrie memory footprint and slow down `load_wordlist()`.

**Mitigation**: LOUDS-encoded succinct tries are extremely space-efficient
(~2-3 bits per node). A 90K wordlist should require ~200-400KB of trie memory,
well within browser constraints. Monitor with benchmarks.

### R5: Homonym Disambiguation

**Risk**: Kotus lists homonyms as separate entries with different inflection
classes (e.g., `kilo` meaning both "frizz" and "kilogram"). For speller
purposes, this is fine (both spellings are valid). For lemma purposes, the
distinction is irrelevant (same surface form).

**Mitigation**: Deduplicate by lemma for wordlist; keep POS-distinguished
entries in kotus_lemmas.tsv.

### R6: konjunktio CCONJ/SCONJ Ambiguity

**Risk**: Kotus uses a single `konjunktio` class for both coordinating and
subordinating conjunctions, but UD distinguishes CCONJ from SCONJ.

**Mitigation**: Maintain a small hardcoded list of known subordinating
conjunctions (`että, koska, kun, jos, vaikka, jotta, kunnes, ennen kuin`)
and map them to SCONJ. All others default to CCONJ. The list is small (<20
entries) and well-established in Finnish grammar.

---

## 11. References

- [Nykysuomen sanalista - Kotus](https://kotus.fi/sanakirjat/kielitoimiston-sanakirja/nykysuomen-sana-aineistot/nykysuomen-sanalista/) -- Official page with download and documentation
- [Nykysuomen sanalista updated](https://kotus.fi/nykysuomen-sanalista-paivitetty/) -- 2024 update announcement
- [hugovk/everyfinnishword](https://github.com/hugovk/everyfinnishword) -- GitHub mirror of v1 XML with plaintext extraction
- [Legisign/kotusparser](https://github.com/Legisign/kotusparser) -- Python XML parser for the v1 format
- [pulmark/finnish-dictionary](https://github.com/pulmark/finnish-dictionary/blob/master/kotus-sanalista_v1.xml) -- GitHub mirror of v1 XML
- [Kielipankki nykysuomi-sanalista](https://www.kielipankki.fi/lexical-conceptual-resources/nykysuomi-sanalista/) -- Academic mirror with metadata
- [Qalle's Kotus list variants](https://qalle.neocities.org/kotuslistat) -- Derived plaintext variants (104K+ entries)
- [Kaino inflection types](https://kaino.kotus.fi/sanat/nykysuomi/taivutustyypit.php) -- Official inflection type reference (78 types + 17 gradation types)
- [jkorpela: Inflection types of nouns](https://jkorpela.fi/finnish/inflection_types_of_nouns.html) -- Detailed nominal declension reference
