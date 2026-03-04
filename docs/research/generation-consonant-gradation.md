---
title: Generation-Direction Consonant Gradation
created: 2026-03-05
commit: 37462bf
status: implemented
relates-to:
  - irregular-verb-generation.md
  - analysis-generation-symmetry.md
---

# Generation-Direction Consonant Gradation: Deep Analysis

## 1. Executive Summary (한국어)

MCE의 Comonad 엔진(M2')에는 11개 자음교체(consonant gradation) 패턴이 coKleisli arrow로 구현되어 있으며, 분석(surface → base) 방향에서 정확히 작동한다. 그러나 현재 생성기(`mce-fi/src/generator.rs`)에서 consonant gradation이 **이미 사용되고 있음**을 분석 결과 확인했다.

핵심 발견:
- **생성기는 이미 `gradate()` 함수를 호출**하여 단수/복수 모든 격변화에서 자음교체를 적용한다. `apply_case()`에서 `gradate(baseform, case_info.grade)`를 명시적으로 호출.
- **문제의 실체는 복수 어간(plural stem) 형성 시점**에 있다. `plural_stem("kaupunki")` → `"kaupunke"` (i→e 교체)를 먼저 수행한 후, `gradate("kaupunke", Grade::Weak)` → `"kaupuge"` 가 아니라 `"kaupunge"` (nk→ng)가 올바르게 적용된다.
- **실제 오류 지점은 nominative plural 경로**에 있다: `apply_plural_case()`에서 nominative plural은 `gradate(baseform, Grade::Strong)`으로 원형을 사용하므로 `"kaupunkit"`이 되어야 하지만, 문제의 "kaupungit"는 **weak grade가 적용된 형태**이다. 즉, 복수 주격에서도 자음교체가 필요한 일부 단어 유형의 문제이다.

사실 "kaupunki" → "kaupungit" (nominative plural)의 오류는 더 근본적인 문제를 드러낸다:
1. 현재 생성기의 nominative plural은 `gradate(baseform, Grade::Strong) + "t"`로 처리됨
2. "kaupunki"는 nominative singular에서 strong grade (nk)이므로, nominative plural도 strong grade → "kaupunkit"
3. 하지만 실제 핀란드어에서 "kaupunki"의 nominative plural은 "kaupungit" (weak grade nk→ng)
4. **이것은 gradation 미적용이 아니라, `-i` 어미 단어의 복수형에서 특수한 어간 교체가 필요한 문제**

수학적 분석에서는 다음을 확인:
- 역방향(weak → strong) gradation은 **대부분 결정론적**이지만 3가지 ambiguous case 존재
- Writer Comonad의 `extend`는 **bidirectional** — `Grade::Strong` 파라미터로 이미 역방향 적용 가능
- 별도의 "Generation Comonad"나 inverse coKleisli 구성은 불필요
- 진짜 필요한 것은 **어간 유형(stem class) 정보를 생성 파이프라인에 주입**하는 것

---

## 2. 현재 Comonad 자음교체 구현 분석

### 2.1 GradationPattern 구조

파일: `crates/mce-comonad/src/finnish.rs`, lines 96-183

11개 패턴이 `PATTERNS` 상수로 정의:

```rust
pub struct GradationPattern {
    pub strong: [char; 2],  // [context_char, graded_char]
    pub weak: [char; 2],    // [context_char, replacement_char]
}
```

| # | Strong pair | Weak pair | Type | Description |
|---|-----------|---------|------|-------------|
| 1 | `['p', 'p']` | `['p', '\0']` | Quantitative | pp → p (deletion) |
| 2 | `['t', 't']` | `['t', '\0']` | Quantitative | tt → t (deletion) |
| 3 | `['k', 'k']` | `['k', '\0']` | Quantitative | kk → k (deletion) |
| 4 | `['m', 'p']` | `['m', 'm']` | Cluster | mp → mm |
| 5 | `['n', 't']` | `['n', 'n']` | Cluster | nt → nn |
| 6 | `['n', 'k']` | `['n', 'g']` | Cluster | nk → ng |
| 7 | `['l', 't']` | `['l', 'l']` | Cluster | lt → ll |
| 8 | `['r', 't']` | `['r', 'r']` | Cluster | rt → rr |
| 9 | `['\0', 'p']` | `['\0', 'v']` | Single | p → v |
| 10 | `['\0', 't']` | `['\0', 'd']` | Single | t → d |
| 11 | `['\0', 'k']` | `['\0', '\0']` | Single | k → ∅ (deletion) |

`'\0'` at position 0 means "preceded by a vowel" (wildcard for any vowel). `'\0'` at position 1 means "delete this character."

Ordering is critical: more specific patterns (geminates, clusters) come before single-consonant patterns. This prevents false matches (e.g., the `p` in `mp` should match cluster pattern #4, not single-consonant pattern #9).

### 2.2 coKleisli Arrow: `apply_gradation`

File: `crates/mce-comonad/src/finnish.rs`, lines 296-312

```rust
pub fn apply_gradation(z: &Zipper<char>, grade: Grade) -> char {
    let focus = *z.extract();
    let left = z.peek_left(1).copied();
    let right = z.peek_right(1).copied();

    if let Some(pat) = find_pattern_at_pos1(left, focus, right, grade) {
        let target = match grade {
            Grade::Weak => &pat.weak,
            Grade::Strong => &pat.strong,
        };
        return target[1];
    }
    focus
}
```

Key architectural insight: **this arrow is already bidirectional**. The `grade` parameter determines the direction:
- `Grade::Weak`: match against `strong` side, produce `weak` side (strong→weak = weakening)
- `Grade::Strong`: match against `weak` side, produce `strong` side (weak→strong = strengthening)

The `find_pattern_at_pos1()` function (lines 208-255) implements the pattern matching. For `Grade::Strong`, it uses the `weak` pair as the source pattern to match against and the `strong` pair as the target to produce.

### 2.3 Writer Comonad Integration

File: `crates/mce-comonad/src/writer.rs`, lines 355-368

The Writer Comonad wraps the raw arrow to handle deletions algebraically:

```rust
pub fn gradation_writer(
    wz: &WriterZipper<DeletionSet, char>, grade: Grade
) -> (DeletionSet, char) {
    let focus = *wz.extract();
    let result = apply_gradation(&wz.zipper, grade);

    if result == '\0' {
        (DeletionSet::singleton(wz.position()), focus)
    } else {
        (DeletionSet::new(), result)
    }
}
```

When `apply_gradation` returns `'\0'` (deletion), instead of inserting a null sentinel, the Writer arrow returns `(DeletionSet::singleton(pos), original_char)`. This marks the position for later removal during `materialize()`, preserving pure coKleisli composition.

### 2.4 Pipeline Composition

File: `crates/mce-comonad/src/writer.rs`, lines 407-431

```rust
pub fn morphophonological_pipeline_pure(word: &str, grade: Grade) -> String {
    let writer = WriterZipper::<DeletionSet, char>::new(zipper);
    let after_gradation = writer.extend(|wz| gradation_writer(wz, grade));
    let after_harmony = after_gradation.extend(harmony_writer);
    let after_possessive = after_harmony.extend(possessive_writer);
    after_possessive.materialize_string()
}
```

Three coKleisli arrows composed via `extend`:
1. Consonant gradation (may produce deletions)
2. Vowel harmony (resolves archiphonemes A, O, U)
3. Possessive vowel copying (resolves archiphoneme V)

Deletions accumulate in the `DeletionSet` (a BTreeSet monoid under set union) and are applied once at `materialize()`.

### 2.5 Convenience Wrappers Used by Generator

The generator calls these convenience functions (all ultimately use the Writer pipeline):

```rust
// finnish.rs line 334
pub fn gradate(word: &str, grade: Grade) -> String {
    crate::writer::gradate_pure(word, grade)  // Writer pipeline
}

// finnish.rs line 538
pub fn harmonize(word: &str) -> String {
    // Zipper extend over apply_vowel_harmony
}
```

---

## 3. 현재 생성기의 구현과 한계

### 3.1 Singular Generation: 이미 Gradation 적용됨

File: `crates/mce-fi/src/generator.rs`, lines 559-576

```rust
fn apply_case(baseform: &str, case_info: &CaseInfo) -> String {
    if case_info.suffix.is_empty() {
        return baseform.to_string();  // nominative: unchanged
    }
    // Step 1: Apply consonant gradation to the stem only.
    let graded_stem = gradate(baseform, case_info.grade);
    // Step 2: Concatenate graded stem + archiphonemic suffix.
    let intermediate = format!("{}{}", graded_stem, case_info.suffix);
    // Step 3: Apply vowel harmony
    let after_harmony = harmonize(&intermediate);
    // Step 4: Apply possessive vowel copying.
    apply_possessive_to_word(&after_harmony)
}
```

This correctly produces, e.g.:
- `kaappi` + genitive (weak): `gradate("kaappi", Weak)` → `"kaapi"` → `"kaapin"` ✓
- `ranta` + genitive (weak): `gradate("ranta", Weak)` → `"ranna"` → `"rannan"` ✓
- `puku` + genitive (weak): `gradate("puku", Weak)` → `"puu"` → `"puun"` ✓

### 3.2 Plural Generation: Where the Bug Lies

File: `crates/mce-fi/src/generator.rs`, lines 768-822

```rust
fn apply_plural_case(baseform: &str, case_info: &CaseInfo) -> String {
    // Nominative plural: baseform (strong grade) + "t"
    if case_info.name == "nominative" {
        let graded = gradate(baseform, Grade::Strong);
        return format!("{}t", graded);
    }
    // ... special cases for genitive, partitive, illative ...

    // Standard plural cases: plural stem + gradation + suffix + harmony
    let ps = plural_stem(baseform);
    let graded = gradate(&ps, case_info.grade);
    // ... harmony handling ...
}
```

The `plural_stem()` function (lines 607-636):

```rust
fn plural_stem(baseform: &str) -> String {
    let last = chars[chars.len() - 1];
    match last {
        'a' | 'ä' => { /* drop final vowel, add -i- */ }
        'i' => { /* drop -i, add -e- */ }  // kaupunki → kaupunke
        'o' | 'ö' | 'u' | 'y' | 'e' => { /* keep vowel, add -i- */ }
        _ => { /* add -i- */ }
    }
}
```

### 3.3 The "kaupunki" Problem Decomposed

Let's trace exactly what happens for `generate_paradigm("kaupunki")`:

**Nominative plural:**
```
apply_plural_case("kaupunki", CaseInfo { name: "nominative", grade: Strong })
→ gradate("kaupunki", Grade::Strong)  // Strong grade: nk stays nk
→ "kaupunki"  // no change (nk is already strong)
→ "kaupunkit"  // + "t"
```

Expected: `"kaupungit"`. The issue is that for `-i` stem words, the nominative plural in Finnish uses the **inflectional stem** (vartalo) rather than the nominative form. For "kaupunki":
- Nominative singular: kaupunki (strong grade)
- Inflectional stem: kaupungi- (weak grade, with stem vowel change i→∅)
- Nominative plural: kaupungi + t = kaupungit

**Inessive plural:**
```
plural_stem("kaupunki") = "kaupunke"  // i → e
gradate("kaupunke", Grade::Weak) = ?
```

Here, `gradate("kaupunke", Grade::Weak)` would apply nk→ng, giving `"kaupunge"`. Then with suffix `"ssA"` → `"kaupungessa"`.

Expected: `"kaupungeissa"` (plural marker -i- between stem and suffix).

This reveals a deeper issue: **the plural stem formation for `-i` words is incomplete**. The plural marker `-i-` should be inserted between the inflectional stem and the case suffix, but `plural_stem()` replaces `-i` with `-e` without considering that the plural `-i-` still needs to appear in oblique cases.

### 3.4 Root Cause Summary

The gradation coKleisli arrow itself works perfectly in both directions. The problem is in the **morphological logic** of the generator:

1. **Nominative plural of `-i` stems**: Should use the inflectional stem (with weak-grade consonants in some paradigms) rather than `baseform + "t"`.

2. **Plural stem formation**: The `plural_stem()` function is overly simplified. For `-i` ending words like "kaupunki" (declension type = KPT-gradation + `-i` stem), the correct plural formation requires knowing the **stem class**.

3. **Missing stem class information**: The generator treats all words uniformly based on their final vowel. But Finnish has multiple declension classes with different plural formations for words ending in `-i`.

---

## 4. 역방향 결정론성 분석 (Strong ↔ Weak Mapping)

### 4.1 Forward Direction (Strong → Weak): Deterministic

Every strong form maps to exactly one weak form:

| # | Strong | → | Weak | Deterministic? |
|---|--------|---|------|----------------|
| 1 | pp | → | p | Yes |
| 2 | tt | → | t | Yes |
| 3 | kk | → | k | Yes |
| 4 | mp | → | mm | Yes |
| 5 | nt | → | nn | Yes |
| 6 | nk | → | ng | Yes |
| 7 | lt | → | ll | Yes |
| 8 | rt | → | rr | Yes |
| 9 | Vp | → | Vv | Yes |
| 10 | Vt | → | Vd | Yes |
| 11 | Vk | → | V∅ | Yes |

All 11 patterns are deterministic in the forward direction. Given the left context and the focus character, at most one pattern matches.

### 4.2 Reverse Direction (Weak → Strong): Mostly Deterministic, 3 Ambiguities

| # | Weak | → | Strong | Deterministic? | Notes |
|---|------|---|--------|----------------|-------|
| 1 | (single) p | → | pp | **AMBIGUOUS** | Single `p` after vowel could be either weakened `pp` or weakened `mp`. But `mp`→`mm`, not `p`, so actually safe. However, if the word has a single `p` that was never gradated, this would be a false match. |
| 2 | (single) t | → | tt | **AMBIGUOUS** | Same issue: `t` after vowel could be original (non-gradating) `t`, or weakened `tt`. Cluster patterns (nt→nn, lt→ll, rt→rr) produce different weak forms, so no collision with those. |
| 3 | (single) k | → | kk | **AMBIGUOUS** | Single `k` after vowel: original or weakened `kk`? |
| 4 | mm | → | mp | Yes | No other pattern produces `mm` |
| 5 | nn | → | nt | Yes | No other pattern produces `nn` |
| 6 | ng | → | nk | Yes | No other pattern produces `ng` |
| 7 | ll | → | lt | Yes | No other pattern produces `ll` |
| 8 | rr | → | rt | Yes | No other pattern produces `rr` |
| 9 | Vv | → | Vp | Yes | `v` after vowel always comes from `p` |
| 10 | Vd | → | Vt | Yes | `d` after vowel always comes from `t` |
| 11 | V∅ | → | Vk | **N/A** | Deletion cannot be reversed from surface form alone |

**Detailed ambiguity analysis:**

**Ambiguity 1-3 (geminate restoration):** When the gradation arrow operates on a word in weak form with `Grade::Strong`, a single `p`/`t`/`k` after a vowel will match the `weak` side of the single-consonant pattern (`['\0', 'v']` → `['\0', 'p']`, etc.) NOT the geminate pattern. This is because:
- Geminate weak side is `['p', '\0']` — the focus is `'\0'` (deletion marker), not a `p`.
- After materialization (deletion applied), the geminate weak form has only ONE `p`, but the gradation arrow sees the un-materialized zipper (before deletion).

This means that `gradate("kaapi", Grade::Strong)` would try to restore the strong form:
- Position 3 is `p`. Left is `a` (vowel). Right is `i`.
- It matches single-consonant pattern `['\0', 'p'] → ['\0', 'v']` in the WEAK direction, but we want STRONG direction: so it looks for `weak[1] == 'p'` with `weak[0] == '\0'` and `left == vowel`.
- Wait — in `Grade::Strong`, the source is `weak` and target is `strong`. So it matches `weak: ['\0', 'v']` against the focus. Focus is `p`, not `v`. No match.
- Then it checks geminate pattern: `weak: ['p', '\0']`. Focus is `p`, `source[1] = '\0'` — focus `p` != `'\0'`. No match either.
- So `gradate("kaapi", Grade::Strong)` returns `"kaapi"` unchanged!

**This is actually a fundamental limitation**: the geminate patterns' weak form has `'\0'` at position 1 (the second consonant is deleted). After materialization, that position no longer exists. So strengthening cannot recover the deleted consonant from the surface form.

The existing tests confirm this:
```rust
// finnish.rs line 818-820
// Note: reversing geminate weakening (single p -> pp) from filtered
// text is ambiguous and not tested here. The raw zipper output before
// '\0' filtering would be needed for that.
```

**Ambiguity 11 (k-deletion restoration):** Even worse for `k → ∅`: the deleted character leaves no trace in the surface form. `gradate("puu", Grade::Strong)` has no `k` to match, so it cannot restore `"puku"`.

### 4.3 Complete Reverse Mapping Table

| Weak surface form | Strong form | Reversible from surface? | Method |
|-------------------|-------------|--------------------------|--------|
| p (from pp) | pp | **NO** | Surface has only `p`; cannot distinguish from non-gradating `p` |
| t (from tt) | tt | **NO** | Same issue |
| k (from kk) | kk | **NO** | Same issue |
| mm (from mp) | mp | **YES** | `mm` after vowel uniquely maps to `mp` |
| nn (from nt) | nt | **YES** | `nn` after vowel uniquely maps to `nt` |
| ng (from nk) | nk | **YES** | `ng` uniquely maps to `nk` |
| ll (from lt) | lt | **YES** | `ll` after vowel uniquely maps to `lt` |
| rr (from rt) | rt | **YES** | `rr` after vowel uniquely maps to `rt` |
| v (from p) | p | **YES** | `v` after vowel uniquely maps to `p` |
| d (from t) | t | **YES** | `d` after vowel uniquely maps to `t` |
| ∅ (from k) | k | **NO** | No surface trace of deletion |

**Result**: 8 of 11 patterns are deterministically reversible from surface form. The 3 non-reversible cases (geminate weakening and k-deletion) require external knowledge (the base form or stem class).

### 4.4 Impact on Generation

For generation, we start from the **base form** (strong grade = citation form), so reversal from surface is **NOT needed**. We only need the forward direction (strong → weak), which is always deterministic. The generator calls `gradate(baseform, grade)` where `baseform` is the citation form (strong grade), and `grade` is determined by the target case.

This confirms: **the Comonad gradation machinery is sufficient for generation. The bug is in the morphological stem formation logic, not in the gradation engine.**

---

## 5. Writer Comonad 프레임워크의 수학적 분석

### 5.1 Current Structure

The Writer Comonad in MCE has the structure:

```
W = WriterZipper<DeletionSet, char>
```

where:
- **Underlying comonad D**: `Zipper<char>` — the list zipper
- **Monoid M**: `DeletionSet` — `(P(N), ∪, ∅)` where P(N) is the power set of natural numbers

The product comonad `W = M × D` has operations:
- `extract(m, d) = extract_D(d)` — project the focused character
- `extend(f)(m, d) = (m ⊕ ⊕ᵢ mᵢ, extend_D(π₂ ∘ f)(d))` where `f` returns `(mᵢ, bᵢ)` at each position

The `materialize()` method applies accumulated deletions to produce the final string.

### 5.2 Bidirectionality of the coKleisli Arrow

The gradation coKleisli arrow is parametric in `Grade`:

```
g: Grade → (W char → (DeletionSet, char))
```

Setting `Grade::Weak` gives the weakening arrow `g_w: W char → (DeletionSet, char)`.
Setting `Grade::Strong` gives the strengthening arrow `g_s: W char → (DeletionSet, char)`.

Both are valid coKleisli arrows. The composition:

```
extend(g_w) ; extend(harmony) ; extend(possessive)
```

produces the weakened form, and

```
extend(g_s) ; extend(harmony) ; extend(possessive)
```

produces the strengthened form.

**There is no need for a separate "Generation Comonad"** or an "inverse coKleisli" construction. The parametric arrow already provides bidirectional gradation.

### 5.3 Why Adjoint Functors Are Not Needed

The user asked about using adjoint functors to relate analysis and generation. Let us examine this:

An adjunction `F ⊣ G` between analysis and generation would require:
- `F: Analysis → Generation` (lifting analysis to generation)
- `G: Generation → Analysis` (embedding generation into analysis)
- Natural bijection: `Hom(F(A), B) ≅ Hom(A, G(B))`

In our case, the analysis arrow and the generation arrow share the same computational substrate (Zipper) and the same rule table (PATTERNS). The only difference is the direction parameter. This is not an adjunction — it is a **parametric family** of arrows indexed by `Grade`.

The mathematical relationship is simpler than an adjunction:

```
g_s ∘ g_w ≈ id   (modulo deletion irreversibility)
```

For the 8 reversible patterns, `gradate(gradate(word, Weak), Strong) = word`. For the 3 irreversible patterns (geminate deletion, k-deletion), the composition is not the identity because information is lost during deletion. This is a fundamental asymmetry, not one that can be resolved by categorical machinery.

### 5.4 DeletionSet in Generation Context

For generation (strong → weak), the DeletionSet is used exactly as in analysis:
- Geminate weakening (pp → p): marks position of second consonant for deletion
- k-deletion (Vk → V∅): marks position of k for deletion
- All other patterns: no deletion, just character replacement

The `materialize()` step at the end removes the marked positions, producing the correct surface form. No modification to the Writer Comonad framework is needed.

For generation (weak → strong, if ever needed), the DeletionSet is **never populated** because strengthening never deletes characters — it only replaces them. The existing framework handles this correctly: the returned `DeletionSet` is always empty for strengthening patterns.

---

## 6. 구현 전략 비교

### 6.1 Strategy A: Fix Morphological Logic (Recommended)

**Approach**: Keep the Comonad gradation engine unchanged. Fix the generator's morphological logic to correctly handle stem classes.

**What needs to change in `generator.rs`:**

1. **Nominative plural for `-i` stems**: Instead of `gradate(baseform, Strong) + "t"`, use the inflectional stem:
   ```
   kaupunki → inflectional stem "kaupungi" → + "t" → "kaupungit"
   ```

2. **Plural stem formation for `-i` stems**: Different `-i` words have different plural formations:
   - "kaupunki" (type 5, -nki): stem = kaupungi-, plural = kaupungei-
   - "suomi" (type 7): stem = suome-, plural = suomi-
   - "kivi" (type 7): stem = kive-, plural = kivi-

3. **Stem class detection**: Add a function that determines the declension type from the base form. At minimum, detect whether a word undergoes gradation (based on the consonant pattern in the stem).

**Pros**: Minimal code change, no Comonad modification needed, all existing tests pass.

**Cons**: Requires hardcoding or heuristic stem class detection without dictionary lookup.

### 6.2 Strategy B: Lookup Table for Stem Alternations

**Approach**: Maintain a mapping from base form patterns to their inflectional stems, separate from the gradation engine.

```rust
fn inflectional_stem(baseform: &str) -> String {
    // For words ending in -nki, -nki → -ngi (weak grade of nk)
    // For words ending in -ppi, -ppi → -pi (weak grade of pp)
    // etc.
    gradate(baseform, Grade::Weak)  // This already works!
}
```

Wait — this is actually simpler than expected. For most `-i` stem words, the inflectional stem is simply the **weak-grade form** of the base. The existing `gradate()` function already produces this:
- `gradate("kaupunki", Grade::Weak)` → `"kaupungi"` ✓
- `gradate("kaappi", Grade::Weak)` → `"kaapi"` ✓

For nominative plural, we need:
```rust
fn nominative_plural(baseform: &str) -> String {
    let stem = gradate(baseform, Grade::Weak);
    // Drop the final vowel and add "t"
    // kaupungi → kaupungi + t → kaupungit? No, we need to drop 'i' first
    // Actually: kaupunki → inflectional stem (weak) → kaupungi → + t → kaupungit
    format!("{}t", stem)
}
```

Hmm, this would give `"kaupungit"` from `gradate("kaupunki", Weak) = "kaupungi"` + `"t"`. That's correct!

But it would also give `"talotn"` problems — `gradate("talo", Weak) = "talo"` (no gradation) + `"t"` = `"talot"`. Still correct.

And `gradate("kukka", Weak) = "kuka"` + `"t"` = `"kukat"`. Expected: `"kukat"`. Correct!

**Wait. This means the fix might be as simple as: for nominative plural, use weak grade instead of strong grade for the gradation call, for words that undergo gradation in their plural form.**

But that's not universally correct either. Some words have strong grade in nominative plural. The issue is that **nominative plural grade assignment depends on the stem type**, not just the case.

**Pros**: Uses existing infrastructure, no new data structures.

**Cons**: Still requires stem type classification.

### 6.3 Strategy C: Comonad Extension (Not Recommended)

**Approach**: Modify the Writer Comonad to support "generation mode" with an InsertionMonoid alongside DeletionSet.

For geminate strengthening (p → pp), we need to INSERT a character. The current framework only supports deletion (removing characters from fixed positions). Insertion would require:

```rust
pub struct InsertionSet {
    insertions: BTreeMap<usize, char>,  // position → char to insert
}
```

And an extended `materialize()` that both removes and inserts characters.

**Pros**: Theoretically clean — symmetric with the analysis direction.

**Cons**:
- Massive overengineering for the actual problem (which is morphological, not phonological)
- Breaks the simplicity of the DeletionSet monoid
- InsertionSet + DeletionSet interaction is complex (how to handle overlapping positions)
- Not needed: generation starts from base form (strong), applies weakening — no insertion ever needed

### 6.4 Strategy Comparison Matrix

| Criterion | A: Fix Morphology | B: Lookup Table | C: Comonad Extension |
|-----------|-------------------|-----------------|---------------------|
| Code changes | ~50 lines in generator.rs | ~30 lines + table | ~200+ lines in writer.rs |
| Comonad changes | None | None | Major |
| Test impact | Add new tests | Add new tests | Modify existing + add |
| Correctness | High (pattern-based) | High (data-driven) | Theoretically highest |
| Complexity | Low | Low | High |
| Performance | O(1) per word | O(1) per word | Negligible difference |
| Risk | Low | Low | High (may break analysis) |

---

## 7. 권장 접근법 + 구현 로드맵

### 7.1 Recommended: Strategy A + B Hybrid

The fix is simpler than initially expected because the Comonad gradation engine **already works correctly in the generation direction**. The real fix is in the morphological stem formation logic.

**Phase 1: Fix nominative plural (immediate, ~30 LOC)**

In `apply_plural_case()`, change the nominative plural path:

```rust
if case_info.name == "nominative" {
    // For words with gradation, nominative plural may need weak grade
    // to form the correct inflectional stem.
    // E.g., kaupunki (nk→ng) → kaupungit, not kaupunkit
    //       kukka (kk→k) → kukat (but this needs stem vowel handling too)

    // For -i ending words with consonant gradation in the stem,
    // the nominative plural uses the weak-grade stem + t:
    let last_char = baseform.chars().last().unwrap_or(' ');
    if last_char == 'i' && has_gradation(baseform) {
        let weak_stem = gradate(baseform, Grade::Weak);
        return format!("{}t", weak_stem);
    }

    let graded = gradate(baseform, Grade::Strong);
    return format!("{}t", graded);
}
```

Where `has_gradation()` checks if the baseform contains a gradation-susceptible consonant pattern (nk, mp, nt, lt, rt, pp, tt, kk, Vp, Vt, Vk).

**Phase 2: Fix oblique plural cases (~50 LOC)**

The plural stem for `-i` words with gradation needs the weak-grade stem:

```rust
fn plural_stem_graded(baseform: &str, grade: Grade) -> String {
    let chars: Vec<char> = baseform.chars().collect();
    let last = chars[chars.len() - 1];

    match last {
        'i' => {
            // First apply gradation, then form plural stem
            let graded = gradate(baseform, grade);
            let graded_chars: Vec<char> = graded.chars().collect();
            // Drop final vowel, add plural marker
            let stem: String = graded_chars[..graded_chars.len() - 1].iter().collect();
            format!("{}e", stem)
            // kaupunki → kaupungi (weak) → kaupunge (plural stem)
        }
        'a' | 'ä' => {
            // Gradation applied to plural stem
            let stem: String = chars[..chars.len() - 1].iter().collect();
            let ps = format!("{}i", stem);
            gradate(&ps, grade)
        }
        // ... other cases
    }
}
```

**Phase 3: Add `has_gradation()` helper (~20 LOC)**

```rust
fn has_gradation(word: &str) -> bool {
    let chars: Vec<char> = word.to_lowercase().chars().collect();
    for i in 1..chars.len() {
        let prev = chars[i - 1];
        let curr = chars[i];
        // Check geminate patterns
        if (prev == curr) && matches!(curr, 'p' | 't' | 'k') {
            return true;
        }
        // Check cluster patterns
        if matches!((prev, curr),
            ('m', 'p') | ('n', 't') | ('n', 'k') | ('l', 't') | ('r', 't')
        ) {
            return true;
        }
        // Check single consonant patterns (preceded by vowel)
        if is_vowel(prev) && matches!(curr, 'p' | 't' | 'k') {
            // Need to check it's not part of a cluster
            if i + 1 < chars.len() {
                let next = chars[i + 1];
                if !matches!((curr, next), ('p', 'p') | ('t', 't') | ('k', 'k')) {
                    return true;
                }
            } else {
                return true;
            }
        }
    }
    false
}
```

### 7.2 Implementation Roadmap

| Step | Task | Files | LOC | Tests |
|------|------|-------|-----|-------|
| 1 | Add `has_gradation()` helper | generator.rs | ~25 | 8 |
| 2 | Fix nominative plural for `-i` stems | generator.rs | ~15 | 5 |
| 3 | Fix oblique plural stem for `-i` stems | generator.rs | ~30 | 11 |
| 4 | Fix genitive/partitive plural for `-i` stems | generator.rs | ~20 | 6 |
| 5 | Add integration tests with kaupunki paradigm | generator.rs | ~40 | 1 (22 assertions) |
| **Total** | | | **~130** | **31** |

### 7.3 Test Cases for Validation

```
kaupunki:
  nom sg: kaupunki (strong, no change)
  gen sg: kaupungin (weak, nk→ng + n)
  nom pl: kaupungit (weak inflectional stem + t)
  gen pl: kaupunkien (strong, -ien)
  par pl: kaupunkeja (strong, -ja)
  ine pl: kaupungeissa (plural stem weak + ssA)

kenkä:
  nom sg: kenkä (strong, nk stays)
  gen sg: kengän (weak, nk→ng)
  nom pl: kengät (weak + t)

lintu:
  nom sg: lintu (strong, nt stays as part of base)
  gen sg: linnun (weak, nt→nn)
  nom pl: linnut (weak + t)
```

---

## 8. 기존 테스트 영향 분석

### 8.1 Currently Passing Tests

The generator module has 253 tests. The proposed changes affect:

**Tests that should remain unchanged (no impact):**
- All singular case tests (kaappi, talo, poyta, kukka, ranta, kampa, puku series): These use `apply_case()` which already correctly applies gradation. No changes to this path.
- All verb conjugation tests: No changes to verb generation.
- Case lookup tests, harmony tests, feature parsing tests: Unaffected.

**Tests that may need updating:**

One existing test explicitly acknowledges the problem: `kaappi_plural_nominative` (generator.rs, line 2686). This test currently has a **no-op assertion** (`let _ = form;`) with extensive comments explaining that the simplified plural stem logic for `-i` words produces incorrect output. The comments note:

```rust
// The nominative plural of -i stems actually just adds -t to the baseform.
// We'll revisit this if needed.
let _ = form; // Accept whatever the generator produces for now
```

This is the exact entry point for the fix. After implementation, this test should assert `Some("kaapit".to_string())`.

The other existing plural tests cover:
- koira (back harmony, -a ending): no gradation in stem
- talo (back harmony, -o ending): no gradation
- kissa (back harmony, -a ending, ss is non-gradating): no gradation
- poyta (front harmony, -a ending, t->d gradation): yes, but -a ending, not -i

So the proposed changes should not break any existing tests with real assertions, because no existing test covers the `-i` + gradation + plural combination with a concrete expected value.

### 8.2 New Tests to Add

```rust
// kaupunki paradigm (nk→ng gradation, -i stem)
#[test]
fn kaupunki_genitive_sg() {
    let g = make_gen();
    let form = g.generate("kaupunki", &[("SIJAMUOTO", "genitive")]);
    assert_eq!(form, Some("kaupungin".to_string()));
}

#[test]
fn kaupunki_nominative_pl() {
    let g = make_gen();
    let form = g.generate("kaupunki", &[("SIJAMUOTO", "nominative"), ("LUKU", "plural")]);
    assert_eq!(form, Some("kaupungit".to_string()));
}

#[test]
fn kaupunki_inessive_sg() {
    let g = make_gen();
    let form = g.generate("kaupunki", &[("SIJAMUOTO", "inessive")]);
    assert_eq!(form, Some("kaupungissa".to_string()));
}

// kenkä paradigm (nk→ng gradation, -ä stem, front harmony)
#[test]
fn kenka_genitive() {
    let g = make_gen();
    let form = g.generate("kenkä", &[("SIJAMUOTO", "genitive")]);
    assert_eq!(form, Some("kengän".to_string()));
}
```

### 8.3 Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Breaking singular gradation | Very Low | High | `apply_case()` is unchanged |
| Breaking non-gradating plurals | Low | Medium | Conditional on `has_gradation()` |
| False positive in `has_gradation()` | Low | Medium | Conservative pattern matching |
| Incorrect plural stem for edge cases | Medium | Low | Simplified generator scope is documented |

### 8.4 Existing Analysis Pipeline Integrity

The proposed changes are entirely within `mce-fi/src/generator.rs`. No changes to:
- `mce-comonad/src/finnish.rs` — gradation arrows unchanged
- `mce-comonad/src/writer.rs` — Writer Comonad unchanged
- `mce-comonad/src/cg.rs` — CG rules unchanged
- Any analysis path — morphological analysis is completely separate

The Comonad engine's mathematical properties (comonad laws, monoid laws for DeletionSet, idempotency on non-gradating words) are preserved because the engine itself is not modified.

---

## Appendix A: Mathematical Formalization

### A.1 The Gradation Arrow as a Natural Transformation

Let `Char` be the category with a single object (the type `char`) and morphisms as character transformations. The gradation function can be viewed as a natural transformation:

```
η_g : Zipper ⇒ Id
η_g(z) = apply_gradation(z, g)
```

indexed by `g ∈ {Strong, Weak}`. The coKleisli extension lifts this to:

```
extend(η_g) : Zipper(Char) → Zipper(Char)
```

The key property is that `η_Strong` and `η_Weak` are **pseudo-inverses**:

```
extend(η_Strong) ∘ extend(η_Weak) ≈ id
```

modulo the deletion positions (where information is irreversibly lost).

### A.2 DeletionSet as a Graded Monoid

The DeletionSet can be viewed as a **graded monoid** where the grade tracks the number of deletions:

```
|D| : DeletionSet → N
|D₁ ⊕ D₂| ≤ |D₁| + |D₂|  (idempotent union)
```

For the generation direction, `|D| ∈ {0, 1}` for each pattern application (at most one position is deleted per geminate/k-deletion). The total deletion count across a word is bounded by the number of gradation sites, which is typically 0-2 for Finnish words.

### A.3 Why Generation Does Not Require InsertionMonoid

For generation from base form (strong grade → surface):
- **Weakening** (strong → weak): may delete (geminate shortening, k-deletion). DeletionSet handles this.
- **Strengthening** (weak → strong): may need to insert (p → pp). But generation NEVER goes in the weak → strong direction starting from base form, because the base form IS the strong form.

The only scenario requiring insertion would be:
1. Start with weak-grade surface form
2. Need to produce strong-grade form
3. Geminate restoration: p → pp (need to insert a `p`)

But this scenario does not occur in generation. Generation always starts from the citation form (strong grade) and either:
- Keeps it as-is (strong grade cases: nominative, partitive, essive, illative)
- Weakens it (weak grade cases: genitive, inessive, elative, adessive, ablative, allative, translative)

Therefore, the DeletionSet is sufficient. InsertionMonoid is unnecessary.
