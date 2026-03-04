---
title: Verb Generation FST Reverse Verification
created: 2026-03-05
commit: 37462bf
status: active
relates-to:
  - irregular-verb-generation.md
  - generation-consonant-gradation.md
---

# Verb Generation FST Reverse Verification

**Date**: 2026-03-05
**Method**: Automated full-dictionary verification via `verb_generation_verification.rs`
**Test file**: `crates/mce-fi/tests/verb_generation_verification.rs`

## Summary

| Metric | Value |
|--------|-------|
| Total verb lemmas in dict | 2,446 |
| Generator-rejected (unknown type) | 88 (3.6%) |
| Total forms generated | 56,592 |
| Forms OK (analyzer confirms) | 25,063 (44.3%) |
| Total mismatches | 31,617 (55.7%) |
| - NO_ANALYSIS | 30,396 |
| - WRONG_BASEFORM | 1,133 |
| - GENERATOR_REJECT | 88 |
| Verbs with at least 1 issue | 2,147 / 2,446 (87.8%) |
| Verbs fully correct (all 24 forms) | ~299 (12.2%) |

## Methodology

For each verb lemma extracted from `data/lemma_dict.tsv`:
1. Generate all 24 conjugated forms (4 tenses x 6 persons) using `MorphGenerator`
2. Analyze each generated form with `FinnishAnalyzer` (FST-based)
3. Check if any analysis has `BASEFORM` matching the original lemma

Three mismatch categories:
- **GENERATOR_REJECT**: `classify_verb()` returns `None` (verb ending not recognized)
- **NO_ANALYSIS**: generated form gets zero analyses from FST (non-existent Finnish word)
- **WRONG_BASEFORM**: FST analyzes the form but maps it to a different lemma

## Mismatch Distribution by Tense

| Tense | Mismatches | % of total |
|-------|-----------|------------|
| past | 10,057 | 31.9% |
| conditional | 9,958 | 31.6% |
| present | 6,078 | 19.3% |
| neg_present | 5,436 | 17.2% |

Past tense and conditional have the highest error rates because they require stem vowel
changes that the generator does not implement correctly.

## Mismatch Distribution by Person

| Person | Mismatches |
|--------|-----------|
| 3pl | 5,916 |
| 3sg | 5,142 |
| 1sg | 5,117 |
| 2sg | 5,118 |
| 1pl | 5,118 |
| 2pl | 5,118 |

3sg and 3pl are slightly higher because 3sg uses strong grade (vowel lengthening) and
3pl adds `-vAt` which exposes additional gradation issues.

## Root Cause Analysis

### Bug 1: Past Tense Stem Vowel Changes (Type 1 verbs ending in -aa/-ea)

**Severity**: CRITICAL -- affects all Type 1 verbs with stem vowel `a` or `e`
**Affected verbs**: ~1,500+ (largest category)

The generator's `extract_past_stem()` for Type 1 verbs simply drops the infinitive
final `-a`/`-a`, keeping the stem vowel intact. But Finnish past tense requires
stem vowel changes:

| Stem vowel | Past change | Example | Generator | Correct |
|-----------|-------------|---------|-----------|---------|
| -aa | a -> o + i | ajaa -> ajoi | ajai | ajoi |
| -aa | a -> i (drop) | aavistaa -> aavisti | aavistai | aavisti |
| -ea | e -> i (drop) | lukea -> luki | luki (OK) | luki |
| -ua | u stays + i | puhua -> puhui | puhui (OK) | puhui |

The `extract_past_stem()` function only handles the `-e-` -> drop case (line 993-998)
but misses:
- `-a-` (in -aa verbs): should drop `a` before past marker `i`, giving "aavista" + i = "aavisti"
- `-aa` (long vowel): stem-final `a` should become `o` before `i`, e.g., "ajaa" -> stem "ajo" + i = "ajoi"
- Two-syllable `-aa` verbs: stem vowel `a` drops before `i`, e.g., "alkaa" -> "alko" + i = "alkoi"

**Fix**: In `extract_past_stem()` for Type 1:
```
// Current (wrong):
let stem: String = chars[..chars.len() - 1].iter().collect();
// Should be: check the stem-final vowel and apply the correct vowel change
```

### Bug 2: Incorrect Consonant Gradation Application

**Severity**: HIGH -- produces nonsense words
**Affected verbs**: Words with consonant clusters not subject to gradation

The `gradate()` function applies gradation patterns too aggressively. Examples:

| Verb | Generated | Correct | Gradation applied |
|------|-----------|---------|-------------------|
| eksya | esyn | eksyn | ks -> s (wrong, ks is NOT a gradation pair here) |
| aktivoida | ativoin | aktivoin | kt -> t (wrong, not at a syllable boundary) |
| ajatella | ajadelen | ajattelen | tt -> d (wrong scope) |
| aivastaa | aipastaa (3sg) | aivastaa | st -> s? (wrong) |
| allekirjoittaa | alleirjoitai | allekirjoitti | kk -> k misapplied |
| altistua | allistui | altistui | lt -> ll (wrong) |
| aikaistaa | aiaistaa | aikaistaa (no change) | k -> nothing (wrong) |

The core issue: `gradate()` applies consonant gradation rules without checking that the
consonant cluster is at the correct morphological boundary (between the penultimate and
final syllable). It pattern-matches anywhere in the word.

**Fix**: Gradation should only apply at the stem boundary (typically the last occurrence
of a gradation-eligible consonant cluster before the final syllable). The current
`gradate()` function in `mce-comonad` applies to ALL matching patterns in the word.

**Note**: This same bug also manifests in plural noun generation. The pre-existing
kaupunki plural tests (7 failures) show `kaupunki` -> `kauvungit` instead of
`kaupungit`: the engine correctly applies `nk -> ng` but ALSO applies `p -> v` to the
first syllable's `p`, which is wrong. These failures predate this verification and
are tracked separately.

### Bug 3: Type 2 Verb Past/Conditional Stem (verbs in -oda/-oda/-ida)

**Severity**: MEDIUM
**Affected verbs**: -oida verbs (ahkeroida, aktivoida, analysoida, etc.)

For `-oida` verbs, the past stem keeps the stem vowels and adds `-i-`:
- ahkeroida -> ahkeroi + i = ahkeroii? No, correct is "ahkeroi" (past 3sg)
- The generator produces doubled vowels: "ahkeroii", "ahkeroiin", etc.

Actually for `-oida` verbs:
- Present: ahkeroin (1sg), ahkeroi (3sg with no lengthening needed)
- Past: ahkeroin (same form! context-dependent)
- The past tense doesn't add an extra `-i-` since the stem already ends in `-oi`

**Fix**: Detect when the stem already ends in `-i` (or `-oi`) and skip the past tense
`-i-` marker.

### Bug 4: Type 3 Stem Extraction Loses Consonant

**Severity**: HIGH
**Affected verbs**: All Type 3 verbs with geminate consonants

Current `extract_stem()` for Type 3: drops the last 2 characters and adds `e`.
- tulla -> tul + e = tule (CORRECT -- ll -> l + e)
- ajatella -> ajatel + e = ajatele (WRONG -- should be ajattele)

The problem: "ajatella" has stem "ajatell-", so dropping 2 gives "ajatel" (only one l).
The Type 3 pattern is `stem + doubled_consonant + a/a`, so:
- tulla = tul + l + a (stem = tul, double = l)
- ajatella = ajatel + l + a (stem = ajatel, double = l)
- But the present stem should be "ajattele" (the stem consonant `t` uses strong grade)

This reveals the stem extraction doesn't account for the strong-grade stem -- the
infinitive shows the geminate form of the final consonant, but the actual stem has a
different consonant grade.

**Fix**: For Type 3, the stem extraction should:
1. Remove the suffix `-la`/`-la`/`-na`/`-na`/`-ra`/`-ra`/`-ta`/`-sta` etc.
2. The remaining stem IS the stem (e.g., "ajatel" from "ajatella")
3. Add `-e-` to form the present stem
4. Apply gradation separately

But the deeper issue is that "ajatella" -> stem "ajattel-" where the double `ll` is
part of the infinitive morphology, not the stem. The present stem is "ajattele-" with
strong grade `tt`.

### Bug 5: Type 4 Verbs (haluta, tavata, etc.)

**Severity**: MEDIUM
**Affected verbs**: Type 4 verbs

The generator's `extract_stem()` for Type 4: drops `-ta`/`-ta` and adds `a`/`a`.
- haluta -> halu + a = halua (present stem)
- Present 1sg: "haluan" (correct)
- But for consonant-stem verbs like "tavata":
  - tavata -> tava + a = tavaa
  - Present 1sg: "tavaan" (WRONG, correct is "tapaan" with gradation)

The issue: Type 4 classification catches some verbs that need different treatment.
"tavata" has the stem "tapaa-" (with gradation t->v->p), not "tavaa-".

### Bug 6: Generator-Rejected Verbs (88 verbs) -- UTF-8 Byte Length Bug

The `classify_verb()` function cannot classify 88 verbs. 79 of these end in `-ta`.

**Root cause**: UTF-8 byte length vs char length confusion in `classify_verb()`.

At line 888 of `generator.rs`:
```rust
let before_ta: Vec<char> = lower[..lower.len() - "ta".len()].chars().collect();
```

`"ta".len()` returns 2 (bytes), but when the verb ends in `-ta` (3 bytes in UTF-8
because `a` is 2 bytes), subtracting 2 leaves the `t` in the remaining string.
The last char check then finds `t` (a consonant) and rejects the verb.

Example: `edeta` (6 UTF-8 bytes: e=1 + d=1 + e=1 + t=1 + a=2)
- `lower.len() - "ta".len()` = 6 - 2 = 4
- `lower[..4]` = "edet" (first 4 bytes)
- Last char = `t`, `is_vowel_char('t')` = false -> REJECTED
- Should be: `lower[..3]` = "ede", last char = `e` (vowel) -> Type 4

**Fix**: Replace byte-level slicing with char-level iteration:
```rust
// Before (buggy):
let before_ta: Vec<char> = lower[..lower.len() - "ta".len()].chars().collect();
// After (correct):
let all_chars: Vec<char> = lower.chars().collect();
let before_ta: &[char] = &all_chars[..all_chars.len() - 2]; // drop last 2 chars
```

The same byte-level bug likely exists in the Type 2 check at line 876-883 (for `-da`
verbs), though it affects fewer verbs since `a` (1 byte) and `a` (2 bytes) have
different byte lengths.

## Correctly Generated Verbs

The ~299 verbs with 0 mismatches follow these patterns:
- Type 1 verbs with stem vowels `u`, `y`, `o`, `o` (no vowel change in past tense)
- Simple verbs without consonant gradation in the stem
- Examples: puhua, asua, kieltaa (partially), etc.

## Priority Fix Recommendations

### Priority 1: Past tense stem vowel changes (Bug 1)
- Impact: ~1,500 verbs, all past tense forms
- Effort: Medium (modify `extract_past_stem` for Type 1)
- The rules:
  - Stem-final `a` in polysyllabic verbs: drop before `i` (aavistaa -> aavisti)
  - Stem-final `a` in short verbs: `a` -> `o` (ajaa -> ajoi, alkaa -> alkoi)
  - Stem-final `e`: drop before `i` (lukea -> luki) -- ALREADY WORKS
  - Stem-final `u`, `y`, `o`, `o`: keep + `i` -- ALREADY WORKS

### Priority 2: Gradation scope limiting (Bug 2)
- Impact: ~500+ verbs, all tenses
- Effort: HIGH (requires syllable boundary awareness in gradate())
- This is a fundamental architectural issue in the comonadic gradation pipeline

### Priority 3: Type 3 stem extraction (Bug 4)
- Impact: All Type 3 verbs
- Effort: Medium

### Priority 4: Type 2 -oida past tense doubling (Bug 3)
- Impact: ~200 verbs
- Effort: Low (detect stem-final -i and skip past marker)

### Priority 5: UTF-8 byte/char length fix in classify_verb (Bug 6)
- Impact: 88 verbs (all forms)
- Effort: TRIVIAL (switch from byte-level to char-level slicing)
- This is a one-line fix that unblocks 88 verbs entirely

## How to Run the Verification

```bash
MCE_DICT_PATH=data cargo test -p mce-fi --test verb_generation_verification -- --ignored --nocapture
```

The test takes ~6 seconds and produces a full report to stdout.

## Appendix: Sample Mismatches by Category

### Past tense stem vowel (Bug 1)
```
aavistaa past 3sg -> "aavistai"    expected: "aavisti"
ajaa     past 3sg -> "ajai"        expected: "ajoi"
alkaa    past 3sg -> "alkai"       expected: "alkoi"
antaa    past 1sg -> "annain"      expected: "annoin"
```

### Consonant gradation scope (Bug 2)
```
eksya    present 1sg -> "esyn"       expected: "eksyn"     (ks not gradated)
aktivoida present 1sg -> "ativoin"   expected: "aktivoin"  (kt not gradated)
aivastaa present 3sg -> "aipastaa"   expected: "aivastaa"  (st not gradated)
aikaistaa present 1sg -> "aiaistan"  expected: "aikaistan" (k not gradated)
```

### Type 3 stem loss (Bug 4)
```
ajatella present 1sg -> "ajadelen"   expected: "ajattelen"
```

### Type 2 -oida doubling (Bug 3)
```
ahkeroida past 3sg -> "ahkeroii"    expected: "ahkeroi"
analysoida past 3sg -> "analysoii"   expected: "analysoi"
```

### Generator rejection (Bug 6)
```
edetä, evätä, heiketä, hypätä, hyökätä, hävetä, hävitä -- all rejected
(consonant before -tä/-ta is not recognized by classify_verb)
```
