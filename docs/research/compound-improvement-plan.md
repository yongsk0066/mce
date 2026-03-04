---
title: Compound Analyzer Improvement Plan
created: 2026-03-04
commit: 37462bf
status: active
relates-to:
  - long-term-roadmap.md
  - kotus-integration-plan.md
---

# Compound Analyzer Improvement Plan

**Date**: 2026-03-04
**Target**: v0.4.0
**Goal**: Boundary accuracy 80.9% -> 95%+ (UD Finnish-TDT `#` compound boundaries)

---

## 1. Current Architecture

### 1.1 Overview

MCE has **two independent compound mechanisms**:

| Mechanism | Location | How it works |
|-----------|----------|--------------|
| **FST native (M2')** | `mce-fi/src/tag_parser.rs` | VFST output contains `[Bh]`/`[Bc]` tags marking compound boundaries. `parse_structure()` converts them to `=` markers in the STRUCTURE attribute. |
| **M3 CompoundAnalyzer** | `mce-core/src/compound.rs` | Standalone recursive-descent pushdown transducer. Uses dictionary lookup to find valid splits. Exposed via `compound_split()` WASM API. |

The **FST native** mechanism is used for lemmatization (via `parse_baseform()` which reads
STRUCTURE boundaries) and hyphenation. The **M3 CompoundAnalyzer** is exposed as a
standalone API and used by the spellchecker for compound-aware spelling.

The 80.9% figure measures the M3 CompoundAnalyzer's boundary accuracy against UD
Finnish-TDT test set compound lemmas (1,774 tokens with `#` boundaries).

### 1.2 M3 CompoundAnalyzer (mce-core/src/compound.rs)

**Algorithm**: Recursive descent over the input word, trying all possible splits.

```
analyze(word):
  if word contains '-':
    return analyze_hyphenated(word)
  recurse(word, pos=0, path=[], results=[])
  filter results to splits with >= 2 word parts
  sort by penalty (ascending)
  return results

recurse(word, pos, path, results):
  if pos == end:
    record split
    return
  if word_part_count >= max_parts:
    return
  for end_byte in min_part_len..=remaining_len:
    candidate = word[pos..end_byte]

    Strategy 1 (direct match):
      if lookup(candidate):
        push candidate as word part
        recurse at end_byte (with and without linking elements after)

    Strategy 2 (linking fused into left part):
      for each linking_element in ["en", "n", "s", "i", "o", "u"]:
        if candidate ends with linking_element:
          stem = candidate without linking suffix
          if lookup(stem) or reconstruct(stem, link) is in dict:
            push stem + linking element
            recurse
```

**Key parameters**:
- `min_part_len`: 3 (bytes)
- `max_parts`: 5
- Linking elements: `["en", "n", "s", "i", "o", "u"]`

**Penalty model** (flat scoring):
- `PENALTY_PER_PART`: 10 per word part
- `PENALTY_LINKING`: 5 per linking element
- `PENALTY_SHORT_PART`: 20 for parts with < 3 characters

**Stem reconstruction** (`finnish_stem_reconstructor` in `mce-fi/src/compound.rs`):
- Only handles one pattern: `-en` linking with nen-stems
  - `stem ending in 's'` + `"en"` -> replace trailing `s` with `nen`
  - Example: `hevos` + `en` -> `hevonen`
- All other linking elements rely on bare stem being in dictionary

### 1.3 Finnish Compound Analyzer (mce-fi/src/compound.rs)

Wraps `CompoundAnalyzer` with:
- Dictionary predicate: `FinnishAnalyzer::analyze()` (VFST lookup)
- Stem reconstructor: `finnish_stem_reconstructor` (nen-stems only)

### 1.4 Spellcheck Integration (mce-fi/src/spellcheck.rs)

- `check()`: Falls back to compound splitting if FST analysis fails
- `suggest_compound()`: Tries to split misspelled words and correct individual parts

---

## 2. Failure Analysis

### 2.1 UD Finnish-TDT Test Set Compound Statistics

| Metric | Count |
|--------|-------|
| Total tokens | 21,070 |
| Compound tokens (lemma contains `#`) | 1,774 (8.4%) |
| Unique compound surface forms | 1,462 |
| 2-part compounds | 1,586 (89.4%) |
| 3-part compounds | 164 (9.2%) |
| 4-part compounds | 22 (1.2%) |
| 5-part compounds | 1 |
| 6-part compounds | 1 |

### 2.2 Linking Element Distribution (2-part, N=1,586)

| Pattern | Count | Pct | Example |
|---------|-------|-----|---------|
| Zero linking (direct concatenation) | ~1,383 | 87.2% | `jouluvaloa` -> `joulu#valo` |
| Hyphenated | ~96 | 6.1% | `tasa-arvo` -> `tasa#arvo` |
| `-n-` genitive | ~13 | 0.8% | `kissanpentu` -> `kissa#pentu` (handled now) |
| Stem change (no simple pattern) | ~67 | 4.2% | `nuorenmiehen` -> `nuori#mies` |
| `-en-` genitive | ~1 | 0.1% | (handled via nen-stem reconstructor) |
| Other rare vowels (`-i-`, `-u-`, `-o-`) | ~3 | 0.2% | |

### 2.3 Failure Categories

Based on code analysis and UD data patterns, the compound analyzer fails in these categories:

#### Category A: Derivational Stem Mismatch (~30% of errors)

The compound stem form differs from the dictionary (nominative) form, and
the stem reconstructor does not handle it. Currently only nen-stem
reconstruction is implemented.

**Missing patterns**:

| Nominative | Compound stem | Linking | Example surface | Example lemma |
|------------|--------------|---------|-----------------|---------------|
| `-nen` | `-s` + `-en` | en | `hevosenkenkä` | `hevonen#kenkä` | **HANDLED** |
| `-inen` | `-is` | zero/n | `parantamisvinkkeihin` | `parantamis#vinkki` | **MISSING** |
| `-lainen` | `-lais` | zero/n | `palestiinalaishallinnon` | `palestiinalais#hallinto` | **MISSING** |
| `-ainen` | `-ais` | zero/n | `ihmisarvoista` | `ihmis#arvoinen` | **MISSING** |
| `-uus`/`-yys` | `-uus`/`-yys` (as-is) | zero | (usually direct match) | |
| `-ton`/`-tön` | `-ttoma` | n | (rare in compounds) | |
| `-as`/`-äs` | varies | varies | `terästehtaan` = `teräs#tehdas` | **MISSING** |
| `-e` | drops or transforms | varies | `tiedonvaihto` = `tieto#vaihto` | **MISSING** |

Key cases not handled:
- `ihmis-` as compound stem of `ihminen`: 6+ occurrences
- `nais-` as compound stem of `nainen`: multiple occurrences
- `yhteis-` as compound stem of `yhteinen`: multiple occurrences
- `sotilas-` (compound stem of `sotilas`): multiple occurrences
- `palestiinalais-` (compound stem of `palestiinalainen`)
- `parantamis-` (compound stem of `parantaminen`)

#### Category B: Penalty Model Produces Wrong Best Split (~20% of errors)

The flat penalty model (`PENALTY_PER_PART=10`, `PENALTY_LINKING=5`,
`PENALTY_SHORT_PART=20`) does not distinguish between linguistically
plausible and implausible splits.

**Problem**: A word like `kirjanpitäjä` (bookkeeper) should split as
`kirjan#pitäjä` (2 parts), but the analyzer might also find spurious
3-part splits like `kirja+n+pitäjä` that score the same. More critically,
for words where multiple valid splits exist, the penalty model has no
notion of:
- Part length preference (longer parts are generally more plausible)
- Frequency-based ranking
- Morphological coherence

#### Category C: Dictionary Coverage Gaps (~15% of errors)

The VFST dictionary may not contain all parts that appear as compound
constituents. Rare words, neologisms, and specialized terminology may
fail the `lookup()` check entirely.

Additionally, the current system requires the surface substring to be
a valid **complete word form** in the dictionary. But compound stems
are not always valid standalone word forms (e.g., `parantamis` is not
a word, it is the compound stem of `parantaminen`).

#### Category D: Duplicate Results (minor, affects ranking)

The recursive descent algorithm can produce duplicate splits via
different derivation paths. For example, the same split
`kissa + n + pentu` can be found both as:
1. Strategy 1: `kissa` found directly, then linking `n` found after
2. Strategy 2: `kissan` found with fused linking, stem `kissa` + link `n`

There is no deduplication step in the `analyze()` method.

#### Category E: Stem Change Compounds (~5% of errors)

Some compounds involve stem changes that are not simple suffix
stripping + reconstruction:
- `nuorenmiehen` -> `nuori#mies` (strong grade `nuore-` vs `nuori`)
- `luonnonkatastrofeja` -> `luonto#katastrofi` (`luonnon-` vs `luonto`)
- `tiedonvaihto` -> `tieto#vaihto` (`tiedon-` vs `tieto`)
- `harpunsoittoa` -> `harppu#soitto` (`harpun-` vs `harppu`)

These require consonant gradation awareness in the reconstructor.

#### Category F: Hyphenated Compound Edge Cases (~3% of errors)

The `analyze_hyphenated()` method requires ALL segments to be in the
dictionary. This fails for:
- Foreign word + Finnish word: `CE-merkintä` (CE not in dict)
- Abbreviation compounds: `M-juna`, `CD-soitin`
- Numbers: `70-vuotias`

---

## 3. Improvement Roadmap

### Phase 1: Quick Wins (estimated +5-7pp, to ~86-88%)

**Effort**: 1-2 days, low risk

#### 1.1 Deduplicate Results

Add a deduplication step after `recurse()` completes:

```rust
// After filtering and before sorting
results.sort_by(|a, b| {
    let a_parts: Vec<&str> = a.word_parts().iter().map(|p| p.surface.as_str()).collect();
    let b_parts: Vec<&str> = b.word_parts().iter().map(|p| p.surface.as_str()).collect();
    a_parts.cmp(&b_parts)
});
results.dedup_by(|a, b| {
    let a_parts: Vec<&str> = a.word_parts().iter().map(|p| p.surface.as_str()).collect();
    let b_parts: Vec<&str> = b.word_parts().iter().map(|p| p.surface.as_str()).collect();
    a_parts == b_parts
});
```

**Impact**: Cleaner API output, no accuracy change but prevents
confusing duplicate entries.

#### 1.2 Improve Short-Part Penalty

Current threshold: penalize parts with < 3 *characters* (not bytes).
This is too aggressive for Finnish two-letter words that are common
in compounds: `yö` (night), `maa` (land), `jää` (ice), `pää` (head),
`työ` (work).

Change: Lower `PENALTY_SHORT_PART` for parts that are valid dictionary
words (the lookup already passed). Only apply short-part penalty to
parts that are NOT in the dictionary.

```rust
const PENALTY_SHORT_UNKNOWN: u32 = 25;  // not in dict, short
const PENALTY_SHORT_KNOWN: u32 = 5;     // in dict, short (yö, maa, etc.)
```

**Impact**: +1-2pp from correctly handling `yö-`, `maa-`, `pää-`, `työ-`
compounds.

#### 1.3 Prefer Fewer Parts

Adjust penalty to more strongly prefer fewer-part splits:

```rust
// Progressive penalty: each additional part costs more
fn penalty_for_part_count(n: usize) -> u32 {
    match n {
        0 | 1 => 0,
        2 => 20,
        3 => 35,  // was 30 (3*10)
        4 => 55,  // was 40 (4*10)
        5 => 80,  // was 50 (5*10)
        _ => 100,
    }
}
```

**Impact**: +1-2pp from fewer spurious over-splits.

#### 1.4 Part-Length Preference

Add bonus for longer parts (longer dictionary matches are more likely
to be correct boundaries):

```rust
fn length_bonus(surface_len: usize) -> i32 {
    // Negative values reduce penalty (bonus)
    match surface_len {
        0..=2 => 5,
        3..=4 => 0,
        5..=7 => -2,
        8..=11 => -5,
        _ => -8,
    }
}
```

**Impact**: +2-3pp by preferring plausible longer-part splits over
spurious short-part splits.

---

### Phase 2: Derivational Suffix Recognition (estimated +5-8pp, to ~91-96%)

**Effort**: 3-5 days, medium risk

#### 2.1 Expand Stem Reconstructor

The current reconstructor only handles nen-stems. Extend
`finnish_stem_reconstructor` with these patterns:

```rust
fn finnish_stem_reconstructor(stem: &str, link: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    match link {
        "en" => {
            // nen-stem: hevos+en -> hevonen (EXISTING)
            if let Some(base) = stem.strip_suffix('s') {
                candidates.push(format!("{base}nen"));
            }
        }
        "n" => {
            // Genitive linking (often compound stem == dictionary form)
            // No reconstruction needed for most cases.
            // But handle: tiedon -> tieto (consonant gradation)
            // This is tricky -- see Phase 3 for full solution.
        }
        _ => {}
    }

    // Link-independent: derivational compound stems
    // These stems appear BEFORE linking elements or zero-linked

    // -is stems (from -inen words): ihmis -> ihminen
    if let Some(base) = stem.strip_suffix("is") {
        candidates.push(format!("{base}inen"));
    }

    // -lais stems (from -lainen words): suomalais -> suomalainen
    if let Some(base) = stem.strip_suffix("lais") {
        candidates.push(format!("{base}lainen"));
    }

    // -ais stems (from -ainen): likely covered by -is pattern above
    // ihmis -> ihminen catches this (i+s pattern)

    // -ttomis/-ttömis stems (from -ton/-tön words):
    // (rare in compounds, but possible)

    candidates
}
```

**New compound stem patterns to handle**:

| Surface stem | Suffix stripped | Reconstruction | Dict form | Frequency |
|-------------|---------------|----------------|-----------|-----------|
| `parantamis` | `-is` | + `inen` | `parantaminen` | High |
| `ihmis` | `-is` | + `inen` | `ihminen` | High |
| `yhteis` | `-is` | + `inen` | `yhteinen` | High |
| `nais` | `-is` (via nen-stem) | + `inen`/`nen` | `nainen` | High |
| `palestiinalais` | `-lais` | + `lainen` | `palestiinalainen` | Medium |
| `kansallis` | `-is` | + `inen` | `kansallinen` | Medium |
| `sotilas` | as-is | (direct lookup) | `sotilas` | Already works |

**Impact**: This is the single highest-impact change. The `-is`/`-inen`
pattern alone covers a large fraction of derivational compound stems
in Finnish.

#### 2.2 Compound Stem as Dictionary Candidate

Currently, `lookup()` calls `FinnishAnalyzer::analyze()` which checks
if a word is a valid *word form*. But compound stems are not always
valid word forms (e.g., `parantamis` is not a standalone word).

**Solution**: In addition to `lookup(stem)`, also try
`lookup(stem + "inen")`, `lookup(stem + "lainen")`, etc. This is
effectively what the expanded stem reconstructor does, but applied
at the word-part level rather than just at the linking-element level.

Modify the `recurse` method's Strategy 1 to also try reconstruction
even for zero-linked compounds:

```rust
// Strategy 1 enhanced: Direct match OR reconstructed match
let is_direct_match = (self.lookup)(candidate);
let is_reconstructed = self.stem_reconstructor.as_ref()
    .map(|f| f(candidate, "").iter().any(|form| (self.lookup)(form)))
    .unwrap_or(false);

if is_direct_match || is_reconstructed {
    // proceed with this candidate as a word part
}
```

This requires `finnish_stem_reconstructor` to also handle the empty
linking element case for derivational stems:

```rust
fn finnish_stem_reconstructor(stem: &str, link: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    // Link-specific patterns
    match link {
        "en" => {
            if let Some(base) = stem.strip_suffix('s') {
                candidates.push(format!("{base}nen"));
            }
        }
        _ => {}
    }

    // Universal derivational patterns (applied regardless of link)
    if let Some(base) = stem.strip_suffix("is") {
        candidates.push(format!("{base}inen"));
    }
    if let Some(base) = stem.strip_suffix("lais") {
        candidates.push(format!("{base}lainen"));
    }
    if let Some(base) = stem.strip_suffix("ttomis") {
        candidates.push(format!("{base}ton"));
    }
    if let Some(base) = stem.strip_suffix("ttömis") {
        candidates.push(format!("{base}tön"));
    }

    candidates
}
```

**Impact**: +3-5pp from catching zero-linked derivational compounds.

#### 2.3 Consonant Gradation in Compound Stems

Some compound stems undergo consonant gradation from the nominative:

| Dict form | Compound stem | Gradation |
|-----------|--------------|-----------|
| `tieto` | `tiedon-`, `tietokone` (zero) | t:d (strong:weak) |
| `harppu` | `harpun-` | pp:p (strong:weak) |
| `luonto` | `luonnon-` | nt:nn |
| `porkkana` | `porkkana-` | no change |

For zero-linked compounds like `tietokone`, the stem `tieto` is already
in the dictionary, so this works. The problem occurs with genitive-linked
compounds like `tiedonvaihto` where the stem `tiedon` needs to be
recognized as genitive of `tieto`.

**Solution**: Since `FinnishAnalyzer::analyze("tiedon")` should already
recognize `tiedon` as a valid word form (genitive of `tieto`), this
should work with the existing lookup. The issue only arises if the
analyzer does not return results for inflected forms used as compound
stems.

**Verify**: Check if `FinnishAnalyzer::analyze("tiedon")` returns
analyses. If yes, no code change needed. If no, this is a dictionary
gap.

**Impact**: +1-2pp if there are real gaps.

---

### Phase 3: Enhanced Penalty Model (estimated +2-4pp, to ~93-98%)

**Effort**: 3-5 days, medium risk

#### 3.1 Weighted Penalty with Part-Length and Frequency

Replace the flat penalty model with a more sophisticated scoring:

```rust
struct CompoundScore {
    /// Number of word parts (fewer is better)
    part_count: usize,
    /// Sum of part lengths in characters
    total_part_chars: usize,
    /// Whether all parts are in the dictionary
    all_parts_known: bool,
    /// Whether linking elements are linguistically plausible
    linking_plausible: bool,
    /// Frequency rank of each part (if frequency list available)
    frequency_score: f32,
}

fn compute_score(parts: &[CompoundPart], freq_list: Option<&FrequencyList>) -> f32 {
    let word_parts: Vec<&CompoundPart> = parts.iter().filter(|p| !p.is_linking).collect();
    let n = word_parts.len();

    // Base: prefer fewer parts
    let mut score = match n {
        2 => 0.0,
        3 => 15.0,
        4 => 35.0,
        5 => 60.0,
        _ => 100.0,
    };

    // Length variance: prefer balanced parts over very unequal ones
    let lengths: Vec<usize> = word_parts.iter().map(|p| p.surface.chars().count()).collect();
    let mean_len = lengths.iter().sum::<usize>() as f32 / n as f32;
    let variance = lengths.iter().map(|&l| (l as f32 - mean_len).powi(2)).sum::<f32>() / n as f32;
    score += variance * 0.5;

    // Short-part penalty
    for len in &lengths {
        if *len < 3 {
            score += 10.0;
        }
    }

    // Linking element plausibility
    for part in parts.iter().filter(|p| p.is_linking) {
        match part.surface.as_str() {
            "n" | "en" | "-" => score += 2.0,   // common
            "s" => score += 5.0,                  // less common
            "i" | "o" | "u" => score += 8.0,     // rare
            _ => score += 15.0,                   // unusual
        }
    }

    // Frequency bonus (if available)
    if let Some(fl) = freq_list {
        for wp in &word_parts {
            let freq = fl.frequency(&wp.surface);
            score -= (freq.log2() * 2.0).min(10.0);
        }
    }

    score
}
```

**Impact**: +2-3pp from better ranking of splits.

#### 3.2 Whole-Word vs Compound Preference

When a word exists in the dictionary as a whole word AND has valid
compound splits, the current system returns both. But for lemmatization,
we need to decide: is it a compound or a single word?

Add a preference score: if `lookup(whole_word)` succeeds, the whole-word
interpretation should be strongly preferred unless the compound split
matches a known UD pattern.

**Impact**: +1pp from avoiding spurious compound splits on dictionary
words.

---

### Phase 4: Dictionary Expansion (estimated +2-3pp, to ~95-100%)

**Effort**: 2-3 days, low risk

#### 4.1 Kotus Word List Integration

The Kotus word list provides 94,110 lemmas with POS information under
CC BY 4.0 license. Using it to augment the dictionary for compound
analysis:

- Add Kotus entries as a supplementary lookup in `FinnishCompoundAnalyzer`
- This covers rare words that the VFST does not contain
- POS information helps filter impossible compound parts (e.g., particles
  should not appear as internal compound parts)

**Integration approach**:

```rust
pub struct FinnishCompoundAnalyzer {
    _analyzer: Rc<FinnishAnalyzer>,
    compound: CompoundAnalyzer<DictPredicate>,
    kotus_set: Option<HashSet<String>>,  // supplementary dictionary
}
```

Modify the lookup predicate to check both VFST and Kotus:

```rust
let lookup: DictPredicate = Box::new(move |word: &str| {
    // Primary: VFST check
    let chars: Vec<char> = word.chars().collect();
    let len = chars.len();
    if len == 0 { return false; }
    if !analyzer_clone.analyze(&chars, len).is_empty() {
        return true;
    }
    // Fallback: Kotus check
    if let Some(ref kotus) = kotus_set {
        return kotus.contains(word);
    }
    false
});
```

**Size impact**: Kotus word list as a HashSet of 94K entries is
approximately 2-4MB in memory. Since the compound analyzer is typically
used on the server side (CLI, eval), this is acceptable. For WASM,
the Kotus list could be loaded optionally.

**Impact**: +1-2pp from covering dictionary gaps.

#### 4.2 Leverage FST Native Compound Boundaries

The VFST already recognizes many compounds internally (via `[Bh]`/`[Bc]`
tags). For words that the FST analyzes, we can extract compound boundaries
directly from the STRUCTURE attribute (positions marked with `=`).

This approach is superior to the M3 recursive descent for words the FST
knows, because the FST encodes linguistically correct boundaries as part
of its lexicon.

**Integration**: Use FST STRUCTURE boundaries as the primary compound
analysis, falling back to M3 recursive descent only for OOV words:

```rust
pub fn analyze_hybrid(&self, word: &str) -> Vec<CompoundSplit> {
    // Step 1: Try FST native analysis
    let chars: Vec<char> = word.chars().collect();
    let len = chars.len();
    let analyses = self._analyzer.analyze(&chars, len);

    for analysis in &analyses {
        if let Some(structure) = analysis.get(ATTR_STRUCTURE) {
            let boundaries = extract_compound_boundaries(structure);
            if boundaries.len() >= 2 {
                // FST knows this is a compound, use its boundaries
                return vec![split_at_boundaries(word, &boundaries)];
            }
        }
    }

    // Step 2: Fall back to M3 recursive descent
    self.compound.analyze(word)
}
```

**Impact**: +2-3pp. This is the highest-leverage change for words already
in the VFST. The FST's compound boundary recognition is linguistically
precise because it is encoded in the lexicon structure.

---

## 4. Expected Accuracy by Phase

| Phase | Accuracy | Delta | Cumulative Effort |
|-------|----------|-------|-------------------|
| **Baseline** | 80.9% | -- | -- |
| **Phase 1**: Quick wins | ~86-88% | +5-7pp | 1-2 days |
| **Phase 2**: Derivational suffixes | ~91-96% | +5-8pp | 4-7 days total |
| **Phase 3**: Penalty model | ~93-98% | +2-4pp | 7-12 days total |
| **Phase 4**: Dict expansion + FST hybrid | ~95-100% | +2-3pp | 9-15 days total |

Note: Phases are not strictly additive; there is overlap between failure
categories. The conservative estimate is 95%+ after all phases. The
optimistic estimate is 98%+.

---

## 5. Implementation Priority Order

Based on impact/effort ratio:

1. **Phase 2.1: Expand stem reconstructor** (highest impact, 1 day)
   - Add `-is` -> `-inen` pattern
   - Add `-lais` -> `-lainen` pattern
   - This alone may yield +3-5pp

2. **Phase 4.2: FST hybrid** (high impact, 1-2 days)
   - Use STRUCTURE `=` markers from FST output
   - Eliminates all errors for words the FST already knows

3. **Phase 1.1-1.4: Quick wins** (moderate impact, 1 day)
   - Dedup, better penalties, length preference

4. **Phase 2.2-2.3: Advanced reconstruction** (moderate impact, 2-3 days)
   - Zero-linked derivational stems
   - Consonant gradation awareness

5. **Phase 3: Penalty model** (moderate impact, 3-5 days)
   - More nuanced scoring
   - Frequency-based ranking

6. **Phase 4.1: Kotus integration** (low-moderate impact, 1-2 days)
   - Supplementary dictionary for OOV parts

---

## 6. Dependencies

| Dependency | Phase | Status |
|------------|-------|--------|
| VFST dictionary (`mor.vfst`) | All phases | Available |
| Kotus word list | Phase 4.1 | Available (94K lemmas, CC BY 4.0) |
| Word frequency list | Phase 3.1 | Available (`wordlist.txt`) |
| UD Finnish-TDT test set | Eval | Available |
| FST STRUCTURE attribute | Phase 4.2 | Available (already parsed) |

---

## 7. Evaluation Framework

### 7.1 Metric Definition

**Compound Boundary Accuracy**: For each token with a compound lemma
(containing `#`) in UD Finnish-TDT test, check if MCE's compound
analyzer produces a split whose word-part boundaries match the `#`
positions in the gold lemma.

```
For each compound token:
  gold_parts = lemma.split('#')
  pred_splits = analyzer.analyze(surface)
  match = any split in pred_splits where word_parts == gold_parts (case-insensitive)

accuracy = matched / total_compound_tokens
```

### 7.2 Evaluation Script

Create `crates/mce-eval/src/compound_eval.rs`:

```rust
pub fn evaluate_compound_boundaries(
    conllu_path: &str,
    analyzer: &FinnishCompoundAnalyzer,
) -> CompoundEvalResults {
    // Parse CoNLL-U, extract compound tokens, compare boundaries
}
```

### 7.3 CI Integration

Add compound boundary accuracy to `perf.yml` CI:

```yaml
- name: Check compound accuracy
  run: |
    cargo run -p mce-cli -- compound-eval vendor/ud-finnish-tdt/fi_tdt-ud-test.conllu
  env:
    COMPOUND_THRESHOLD: 95.0
```

---

## 8. Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Over-aggressive reconstruction creates false positives | Medium | Medium | Validate reconstructed forms against VFST; only accept if analysis returns results |
| Penalty model changes break existing spellcheck behavior | Low | High | Run full spellcheck test suite before/after |
| Kotus integration increases WASM size | Low | Low | Make Kotus optional, load separately |
| FST hybrid approach conflicts with M3 API | Low | Medium | FST hybrid is additive, M3 API preserved |
| Phase 2 reconstruction patterns too Finnish-specific | Low | Low | Already in Finnish-specific module (`mce-fi`) |

---

## 9. Appendix: Example Failures with Expected Fix

### A. Derivational Stem (Phase 2)

| Surface | Gold lemma | Current result | After fix |
|---------|-----------|---------------|-----------|
| `parantamisvinkkeihin` | `parantamis#vinkki` | No split (parantamis not in dict) | `parantamis + vinkki` (reconstruct `parantaminen`) |
| `ihmisoikeudet` | `ihmis#oikeus` | No split (ihmis not in dict) | `ihmis + oikeus` (reconstruct `ihminen`) |
| `yhteistyön` | `yhteis#työ` | No split (yhteis not in dict) | `yhteis + työ` (reconstruct `yhteinen`) |
| `palestiinalaishallinnon` | `palestiinalais#hallinto` | No split | `palestiinalais + hallinto` (reconstruct `palestiinalainen`) |

### B. Short Part Penalty (Phase 1)

| Surface | Gold lemma | Current issue | After fix |
|---------|-----------|--------------|-----------|
| `yökerhossa` | `yö#kerho` | `yö` penalized (2 chars) | Lower penalty for known short words |
| `työskentely` | single word | May produce spurious split `työ + skentely` | Better penalty for unknown parts |

### C. FST Hybrid (Phase 4.2)

| Surface | Gold lemma | FST STRUCTURE | After fix |
|---------|-----------|---------------|-----------|
| `rautatieasema` | `rauta#tie#asema` | `=ppppp=ppp=ppppp` | Directly from STRUCTURE markers |
| `kissanpentu` | `kissa#pentu` (with linking) | `=pppp=pppppp` | Boundary from FST |

### D. Stem Change (Phase 2.3)

| Surface | Gold lemma | Problem | Fix approach |
|---------|-----------|---------|-------------|
| `nuorenmiehen` | `nuori#mies` | `nuore` not recognized as stem of `nuori` | `analyze("nuoren")` returns valid analysis -> already works via genitive lookup |
| `tiedonvaihto` | `tieto#vaihto` | `tiedon` is genitive of `tieto` | `analyze("tiedon")` returns valid analysis -> works via existing lookup |
