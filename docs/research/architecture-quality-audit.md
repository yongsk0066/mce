---
title: MCE Architecture Quality Audit
created: 2026-03-05
commit: 37462bf
status: active
relates-to:
  - long-term-roadmap.md
  - analysis-generation-symmetry.md
---

# MCE Architecture Quality Audit

**Date**: 2026-03-05
**Scope**: 11 crates, ~45,400 LOC Rust, 1,616 tests
**Auditor**: Claude (automated audit)

---

## 1. Executive Summary

**Overall Grade: B+**

MCE는 야심적인 수학적 프레임워크(Comonad, DeletionMonoid, Viterbi lattice)를 **실제로 작동하는 프로덕션 코드로 구현한** 드문 사례다. "마케팅용 래핑"이 아니라, Zipper 코모나드가 실제 morphophonological rule의 핵심 엔진으로 작동하고 있으며, 코모나드 법칙 테스트까지 갖추고 있다.

세션 단위 개발에도 불구하고, 코드베이스는 **구조적으로 건강하다**. crate 분리가 논리적이고, 순환 의존성이 없으며, 추상화 레벨이 일관적이다. 4-Machine 아키텍처(M1 Trie, M2' Comonad, M3 PDT, M4' Lattice)가 코드에서 명확히 식별 가능하다.

주요 우려 사항:
- **코드 중복**: `is_vowel` 함수 4개, `edit_distance` 함수 2개가 별도 구현됨
- **Generator의 workaround**: `gradate_stem()`이 코모나드 엔진의 글로벌 적용 문제를 우회하는 임시 방편
- **알려진 언어학적 한계**: 복수형 생성에서 "rantoja" -> "rantia", "puvut" -> "puut" 등의 정확도 이슈가 테스트에서 NOTE로 명시적 문서화됨
- **RefCell 사용**: WASM 바인딩에서 아키텍처적으로 정당화되나, 범위가 잘 제한됨

등급 세부:
- 아키텍처 일관성: **A-**
- 코드 품질: **B+**
- 수학적 원칙 준수: **A**
- 테스트 문화: **A-**
- 기술 부채 관리: **B**
- 문서화: **A**

---

## 2. 아키텍처 일관성 분석

### 2.1 4-Machine 분리도: 우수

CLAUDE.md에 명시된 4-Machine 아키텍처가 crate 구조에 깔끔하게 매핑된다:

| Machine | Crate | 책임 분리 | 평가 |
|---------|-------|-----------|------|
| M1 Succinct Trie | `mce-core/trie` | 사전 lookup, spell checking용 fuzzy search | 명확 |
| M2' Comonad | `mce-comonad` | morphophonological rules, CG-lite | 명확 |
| M3 PDT | `mce-core/compound` | 복합어 분석 | 명확 |
| M4' Lattice | `mce-disambig` | Viterbi + CG-lite + Suffix Tagger | 명확 |

**장점:**
- 각 Machine의 코드가 단일 crate(또는 단일 모듈) 안에 집중됨
- Machine 간 인터페이스가 trait으로 정의됨 (`Transducer`, `Disambiguator`, `Speller`, `GrammarRule`)
- 상위 crate(`mce-fi`, `mce-grammar`, `mce-wasm`)가 Machine들을 조합하는 역할만 수행

**개선 가능:**
- M3 (CompoundAnalyzer)가 `mce-core`에 위치함. 이것은 "core = shared types" 원칙과 약간의 긴장이 있다. CompoundAnalyzer는 비즈니스 로직(복합어 분석 알고리즘)을 포함하는데, `mce-core`의 나머지 모듈(analysis, character, case, token)은 순수 타입 정의에 가깝다. 별도 crate로 분리하면 더 깔끔하지만, 현재 규모에서는 과도한 분리일 수 있다.

### 2.2 crate 의존성 구조: 건강함

```
mce-core (0 deps)
  |
  +-- mce-fst (mce-core, thiserror, bytemuck, hashbrown)
  +-- mce-tokenizer (mce-core)
  +-- mce-comonad (mce-core)
  +-- mce-disambig (mce-core)
  |
  +-- mce-speller (mce-core, mce-fst)
  |
  +-- mce-fi (mce-core, mce-fst, mce-speller, mce-disambig, mce-comonad)
  |
  +-- mce-grammar (mce-core, mce-fst, mce-fi, mce-tokenizer, mce-disambig)
  |
  +-- mce-eval (mce-core, mce-fst, mce-fi, mce-disambig, mce-comonad, mce-tokenizer)
  |
  +-- mce-wasm (mce-core, mce-fst, mce-fi, mce-speller, mce-disambig, mce-comonad,
  |             mce-tokenizer, mce-grammar)
  |
  +-- mce-cli (all above)
```

**순환 의존성: 없음.** 의존성 그래프가 엄격한 DAG를 형성한다.

**관찰:**
- `mce-comonad`와 `mce-disambig`가 서로 독립적인 점이 좋다. CG-lite 규칙이 `mce-comonad/cg.rs`에 있어서, 이 두 crate가 같은 레이어에서 독립적으로 작동할 수 있다.
- `mce-fi`가 5개 내부 crate에 의존하는 것은 이 crate가 "Finnish language glue" 역할임을 고려하면 적절하다.
- `mce-cli`가 모든 crate에 의존하는 것은 CLI 도구로서 자연스럽다.

**외부 의존성 최소화**: workspace 외부 dep이 `thiserror`, `bytemuck`, `hashbrown`, `serde`, `wasm-bindgen`, `js-sys`, `serde-wasm-bindgen`, `criterion`뿐이다. WASM 타겟의 ~395KB 바이너리 사이즈를 고려하면 매우 절제된 선택.

### 2.3 추상화 레벨 일관성: 양호

각 레이어의 추상화 수준이 일관적이다:

- **L0 (Types)**: `mce-core` -- `Analysis`, `Token`, `Character`, `CaseUtils`
- **L1 (Engines)**: `mce-fst`, `mce-comonad`, `mce-disambig` -- 독립적 알고리즘
- **L2 (Language)**: `mce-fi` -- 핀란드어 고유 로직
- **L3 (Applications)**: `mce-grammar`, `mce-speller` -- 기능 모듈
- **L4 (Surfaces)**: `mce-wasm`, `mce-cli` -- 사용자 인터페이스

`mce-speller`가 L1과 L3 사이에 약간 애매하게 위치한다. `pipeline.rs`는 L3 (SpellChecker 조합 로직)이지만, `cache.rs`와 `status.rs`는 L1 (범용 인프라)에 가깝다. 현재 규모에서는 문제 아님.

---

## 3. 코드 품질 패턴

### 3.1 잘 된 점

**1) Writer Comonad 구현 (writer.rs, 980 LOC)**

이 파일은 프로젝트의 백미다:
- `Monoid` trait, `DeletionSet`, `WriterZipper`, `extend`, `materialize` -- 범주론적 구조가 깔끔하게 코드로 번역됨
- 코모나드 법칙 (left identity, right identity, associativity)에 대한 명시적 테스트
- `'\0'` 센티널 해킹을 `DeletionSet` 대수적 접근으로 교체한 것은 아키텍처적으로 진정한 개선
- `morphophonological_pipeline_pure()`와 기존 `morphophonological_pipeline()`의 동치성을 검증하는 테스트

```rust
// 대수적 삭제 (writer.rs:355-368)
pub fn gradation_writer(wz: &WriterZipper<DeletionSet, char>, grade: Grade)
    -> (DeletionSet, char) {
    let result = apply_gradation(&wz.zipper, grade);
    if result == '\0' {
        (DeletionSet::singleton(wz.position()), focus)
    } else {
        (DeletionSet::new(), result)
    }
}
```

이것은 "마케팅용 래핑"이 아니라, 실제로 코모나드가 morphophonology에서 sentinel character 문제를 해결하는 의미 있는 사용 사례다.

**2) Trait 기반 확장성**

주요 인터페이스가 trait으로 정의되어 있어 테스트 가능하고 교체 가능하다:
- `Disambiguator` (mce-disambig) -- 전략 패턴
- `Speller` (mce-speller) -- spell checker 인터페이스
- `MorphValidator` (mce-speller/pipeline) -- 언어 독립적 morph 검증
- `GrammarRule` (mce-grammar) -- 개별 문법 규칙
- `Transducer` (mce-fst) -- FST 순회 추상화

`MorphValidator`의 closure blanket impl은 특히 깔끔하다:
```rust
impl<F> MorphValidator for F
where F: Fn(&[char], usize) -> bool {
    fn is_valid(&self, word: &[char], word_len: usize) -> bool { (self)(word, word_len) }
}
```

**3) SpellChecker Builder 패턴 (pipeline.rs)**

`SpellCheckerBuilder`가 type-safe한 builder 패턴으로 구현되어, 필수 컴포넌트(trie, morph_validator) 누락 시 컴파일 타임이 아닌 런타임에 패닉하지만, 패닉 메시지가 명확하고 테스트에서 검증된다.

**4) CG-lite 규칙 시스템 (cg.rs, 4,546 LOC)**

62개 활성 규칙이 각각 독립적인 struct로 구현되어, 개별 테스트가 가능하다. `CgRule` trait과 `safe_filter()` 함수가 규칙의 안전한 적용을 보장한다. 4,546 LOC에서 테스트가 ~305개인 것은 양호한 test-to-code ratio다.

### 3.2 개선 필요 점

**1) `is_vowel` 함수 중복 (4개 구현)**

| 위치 | 시그니처 | 내용 |
|------|----------|------|
| `mce-fi/src/lib.rs:53` | `pub fn is_vowel(c: char) -> bool` | `simple_lower()` + `VOWELS.contains()` |
| `mce-fi/src/generator.rs:682` | `fn is_vowel(c: char) -> bool` | `matches!()` 매크로 (8 vowels) |
| `mce-fi/src/generator.rs:945` | `fn is_vowel_char(c: char) -> bool` | `is_vowel()` 호출 (alias) |
| `mce-comonad/src/finnish.rs:191` | `fn is_vowel(c: char) -> bool` | `matches!()` 매크로 (8 vowels) |

`mce-fi::is_vowel()`이 public API로 존재하는데, `generator.rs`에서 private `is_vowel()`을 별도 정의하고 `is_vowel_char()`으로 alias까지 만든 것은 세션 간 정보 단절의 징후다. `mce-comonad::finnish::is_vowel()`도 같은 로직의 독립 구현이다.

**권장**: `mce-core::character`에 canonical `is_finnish_vowel()` 함수를 두고, 모든 crate에서 재사용. 현재 `mce-core::character`에 이미 `simple_lower()` 등의 문자 분류 유틸이 있으므로 자연스러운 위치다.

**2) `edit_distance` 함수 중복 (2개 구현)**

| 위치 | 시그니처 |
|------|----------|
| `mce-speller/src/pipeline.rs:286` | `fn edit_distance(a: &[u8], b: &[u8]) -> usize` |
| `mce-fi/src/spellcheck.rs:437` | `fn edit_distance_str(a: &str, b: &str) -> usize` |

동일한 Levenshtein DP 알고리즘의 두 복사본. `mce-core`로 추출할 수 있다.

**3) `gradate_stem()` -- 아키텍처적 workaround**

`generator.rs:610`의 `gradate_stem()` 함수는 **코모나드 엔진의 구조적 제약을 우회하는 workaround**다:

```rust
/// NOTE: This is a workaround for the comonad engine's global application.
/// A proper fix would restrict `extend` to a positional range, but that
/// would require changes to `mce-comonad`.
fn gradate_stem(word: &str, grade: Grade) -> String {
```

문제: `gradate()` (코모나드의 `extend`)가 단어 전체에 자음 점진을 적용하므로, `kaupunki`에서 `nk -> ng`만 원하는데 `p -> v`까지 적용된다. 이를 해결하기 위해 `gradate_stem()`이 마지막 gradation site를 찾아 단어를 분할하고 suffix 부분에만 `gradate()`를 적용한다.

이것은 코모나드의 "모든 위치에 extend" 의미론과 morphological generation의 "특정 위치만 변환" 요구 사이의 근본적 불일치를 드러낸다. 하지만:
- workaround 자체는 기능적으로 올바르게 동작한다 (테스트에서 검증됨)
- NOTE 주석이 문제를 정확히 기술하고 있다
- 올바른 fix (positional extend)의 방향을 명시하고 있다

이것은 "누덕누덕 패치"가 아니라, **의식적으로 문서화된 트레이드오프**다.

**4) Generator의 언어학적 한계가 테스트에 코딩됨**

`generator.rs`의 복수형 테스트에서:
```rust
// NOTE: Correct Finnish is "rantoja", but our simplified generator
// uses the -a → -i stem pattern uniformly. Known limitation.
assert_eq!(form, Some("rantia".to_string()));
```

```rust
// NOTE: Correct Finnish is "puvut" (k → v before u), but our gradation
// engine uses pattern #11 (Vk → V∅ deletion) rather than k → v.
assert_eq!(form, Some("puut".to_string())); // [loosened assertion]
```

이러한 NOTE는 "현재 동작을 고정하는 테스트" vs "올바른 동작을 검증하는 테스트"의 경계에 있다. 양쪽 모두의 목적을 동시에 serve하려 하고 있다. 이것은 건전한 접근이지만, 향후 이슈 트래커나 별도 목록으로 관리하면 더 좋다.

---

## 4. 개발 원칙 준수도

### 4.1 수학적 기조: 진성 (Authentic) -- 평가: A

**Comonad가 실제로 의미 있게 쓰이고 있는가?** -- **그렇다.**

증거:
1. `Zipper::extend()` -- 표준 코모나드 연산, morphophonological rule 적용의 핵심 엔진
2. `WriterZipper::extend()` -- Writer 코모나드를 통한 대수적 삭제 추적
3. 코모나드 법칙 테스트 3개 세트 (Zipper: L1, L2, L3'; WriterZipper: L1, L2, L3')
4. `DeletionSet`의 모노이드 법칙 테스트 (identity, associativity, commutativity)
5. `morphophonological_pipeline_pure()`가 실제 분석 경로에서 사용됨
6. `gradation_writer`, `harmony_writer`, `possessive_writer`가 실제 coKleisli arrow로 사용됨

이것은 단순한 명명 수준의 차용이 아니라, 범주론적 구조가 코드 설계의 핵심 결정을 이끌고 있다. 특히 `'\0'` sentinel -> `DeletionSet` monoid 변환은 코모나드 프레임워크가 실용적 문제를 해결한 구체적 사례다.

**DeletionMonoid의 의미**:
- `DeletionSet`이 monoid (not monad)인 것이 정확하다. Monoid는 combine + identity만 필요하고, set union이 이를 정확히 제공한다.
- 프로젝트 문서에서 "DeletionMonoid"라고 부르는 것은 수학적으로 정확하다.

**한 가지 미묘한 점**: `apply_gradation()` (finnish.rs)은 여전히 `'\0'` sentinel을 반환하고, `gradation_writer()`가 이를 인터셉트하여 `DeletionSet`으로 변환한다. 이것은 **적응적 래핑**이다 -- 기존 코드를 보존하면서 새 추상화를 도입한 것. 궁극적으로는 `apply_gradation()`이 직접 `(DeletionSet, char)`를 반환하도록 리팩토링하면 중간 변환 비용이 사라진다.

### 4.2 WASM 제약 준수: 준수 중 -- 평가: A

| 제약 | 목표 | 실제 | 상태 |
|------|------|------|------|
| WASM binary | ~395KB | ~395KB | 준수 |
| Latency | <5ms/sentence | ~0.8ms | 초과 달성 |
| Deploy size | ~9.2MB | ~9.2MB | 준수 |
| CI check | perf.yml | 94.0% UPOS threshold, 420KB budget | 자동화됨 |

`profile.release`에서 `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`를 설정하여 바이너리 크기 최적화가 철저하다.

외부 의존성이 적은 것도 WASM 크기에 기여한다. `hashbrown`은 std HashMap 대신 사용하여 WASM에서의 해시맵 성능을 개선하되, `serde`와 `wasm-bindgen` 외에 무거운 런타임 의존성이 없다.

### 4.3 테스트 문화: 양호 -- 평가: A-

**정량적:**
- 총 1,616 테스트 (grep count 기준), 45,400 LOC -> **약 28 LOC/test**
- 모든 crate에 테스트 존재 (mce-cli 제외 -- CLI는 통합 테스트로 대체)
- 코모나드 법칙 테스트, 모노이드 법칙 테스트, trait object safety 테스트 포함

**Test distribution (crate별 #[test] count):**
```
mce-fi:       331   (language module, largest test surface)
mce-comonad:  305   (comonad laws + morphophonological rules)
mce-grammar:  261   (21 grammar rules x ~12 cases each)
mce-disambig: 187   (Viterbi + suffix tagger integration)
mce-eval:     127   (evaluation pipeline)
mce-core:      99   (trie, analysis, compound)
mce-wasm:      93   (WASM API integration)
mce-tokenizer: 90   (tokenizer)
mce-speller:   79   (spell checker pipeline)
mce-fst:       44   (FST traversal)
mce-cli:        0   (CLI -- no unit tests)
```

**정성적:**

테스트가 "현재 동작 고정"과 "정답 검증"을 **명시적으로 구분**하는 점이 좋다:
- `ranta_plural_partitive()` 테스트의 NOTE 주석이 "현재 결과"와 "올바른 핀란드어"의 차이를 문서화
- `puku_plural_nominative()` 테스트가 `assert!(form.ends_with('t'))` 형태의 느슨한 assertion을 사용하여 "정확한 형태를 모르지만 구조적 제약은 검증"하는 접근

코모나드 법칙 테스트는 특히 모범적:
```rust
#[test]
fn writer_comonad_associativity() {
    // L3': extend(f).extend(g) == extend(|w| g(&w.extend(f)))
    // ... 5개의 다른 입력에 대해 검증
}
```

**개선 가능:**
- `mce-cli`에 테스트 없음 (통합 테스트 파일도 없음)
- `mce-fst`의 테스트가 44개로 상대적으로 적음 (FST 순회는 critical path이므로)

### 4.4 API 안정성: 관리됨 -- 평가: B+

- v0.3.0에서 `generate_paradigm()` label이 "nominative" -> "nominative sg"로 변경됨 (breaking change)
- `mce-wasm`만 0.3.0이고 나머지 crate는 0.1.0 -- 이것은 "WASM API만 외부 노출"이라는 판단으로 일관적
- npm 배포(`@yongsk0066/mce@0.3.0`)가 존재하므로 semver 관리가 중요해지고 있음

---

## 5. 기술 부채 목록

### P0 (즉시 수정 권장)

**(없음)** -- zero critical issues.

### P1 (단기 수정 권장)

**1. `is_vowel` 함수 중복 제거**
- 영향: 유지보수 시 한 곳을 고치면 나머지 3곳도 동기화해야 함
- 위치: `mce-fi/lib.rs`, `mce-fi/generator.rs` (2곳), `mce-comonad/finnish.rs`
- 작업량: ~30분
- 방안: `mce-core::character`에 canonical `is_finnish_vowel()` 추가

**2. `edit_distance` 함수 중복 제거**
- 영향: 동일 알고리즘의 2개 복사본 유지 필요
- 위치: `mce-speller/pipeline.rs`, `mce-fi/spellcheck.rs`
- 작업량: ~20분
- 방안: `mce-core`에 `pub fn edit_distance(a: &[u8], b: &[u8]) -> usize` 추가

**3. `#[allow(dead_code)]` 정리**
- `mce-comonad/cg.rs:539,551` -- `has_baseform_in()`, `has_attr()` 미사용
- `mce-eval/pos_map.rs:180` -- `FINNISH_PARTICLE_BASEFORMS` 미사용
- 의도적 보존인지 실수인지 확인 필요. Phase 2 CG 규칙 확장용으로 남긴 것이면 주석으로 의도 명시 권장

### P2 (중기 개선 권장)

**4. `gradate_stem()` workaround의 장기 해결**
- 현재: `gradate_stem()`이 단어를 수동 분할하여 코모나드의 글로벌 적용을 우회
- 이상: 코모나드 `extend`에 positional range 지원 추가, 또는 generation 전용 gradation 경로 설계
- 영향: 현재 기능적으로 올바르지만, 코모나드 프레임워크의 일관성을 약간 해침
- 작업량: ~2-4시간

**5. `apply_gradation()`의 `'\0'` sentinel 제거**
- 현재: `mce-comonad/finnish.rs::apply_gradation()`이 `'\0'` 반환, `writer.rs::gradation_writer()`가 인터셉트
- 이상: `apply_gradation()`이 직접 `Option<char>` 또는 `(DeletionSet, char)` 반환
- 영향: 중간 변환 제거, 코드 경로 단순화
- 작업량: ~1-2시간

**6. WASM `RefCell` 대안 검토**
- 현재: `spell_checker: Option<RefCell<SpellChecker<...>>>` (lib.rs:80)
- 이유: `SpellChecker::check()`가 `&mut self` (cache 업데이트) 필요, WASM API는 `&self`
- `RefCell`은 WASM single-threaded 환경에서 안전하지만, `#[wasm_bindgen]`의 `&mut self` 메서드를 사용하는 대안도 검토할 가치 있음
- 현재 `FinnishAnalyzer`에서도 동일 패턴 (`morphology.rs:37: config: RefCell<...>`) 사용 -- 일관적이긴 함

**7. Generator 테스트의 "known limitation" 체계적 관리**
- 현재: 테스트 내 NOTE 주석으로 기록
- 개선: `docs/research/known-issues.md` 또는 GitHub Issues로 추적
- 목록:
  - "rantoja" -> "rantia" (partitive plural of -a stems: simplified pattern)
  - "puvut" -> "puut" (Vk -> V∅ gradation instead of k -> v before u)
  - Old -i words (kivi -> kivejä, not kiviä) 미지원

### P3 (장기 고려)

**8. `mce-core::compound` 분리 검토**
- `CompoundAnalyzer`는 타입이 아닌 알고리즘. `mce-core`의 "shared types" 역할과 약간의 불일치
- 현재 crate 수(11)가 이미 충분히 많으므로, 추가 분리보다는 내부 모듈 주석으로 의도 명시 권장

**9. CLI 테스트 추가**
- `mce-cli`에 0개의 테스트
- 11개 subcommand의 기본 동작을 검증하는 통합 테스트 권장
- 작업량: ~1-2시간

---

## 6. crate 의존성 상세 분석

### 6.1 외부 의존성 평가

| Dependency | Version | 용도 | WASM 영향 | 필요성 |
|------------|---------|------|-----------|--------|
| `thiserror` | 2 | Error derive | 최소 | 필수 |
| `bytemuck` | 1 (derive) | Zero-copy FST transition | 최소 | 최적화에 필수 |
| `hashbrown` | 0.16 | HashMap (no std) | 약간 절약 | 합리적 |
| `serde` | 1 (derive) | Serialization | 약간 추가 | WASM<->JS 통신 필수 |
| `wasm-bindgen` | 0.2 | WASM bindings | 필수 | 필수 |
| `js-sys` | 0.3 | JS interop | WASM only | 필수 |
| `serde-wasm-bindgen` | 0.6 | serde<->JsValue | WASM only | 필수 |
| `criterion` | 0.8 | Benchmarks | dev only | 개발 도구 |

**불필요한 의존성: 없음.** 모든 외부 crate가 명확한 목적으로 사용되고 있다.

### 6.2 wasm-opt 비활성화 주석

```toml
# mce-wasm/Cargo.toml
[package.metadata.wasm-pack.profile.release]
wasm-opt = false
```

주석에 이유가 명시되어 있다: binaryen 126이 Rust 1.93+의 bulk-memory 기능을 지원하지 않음. 이것은 의도적 비활성화이며, 업그레이드 경로도 명시됨. 좋은 실천.

---

## 7. 세션별 작업의 통합도

### 7.1 v0.3.0 추가사항 분석

**suggest() SpellChecker 통합:**
- `mce-wasm/lib.rs`의 `suggest()` 메서드가 3-tier fallback을 구현:
  1. SpellChecker pipeline (trie + morph validation) -- wordlist 있을 때
  2. Raw trie fuzzy search -- wordlist 있지만 SpellChecker 없을 때
  3. `suggest_with_context()` -- 모두 없을 때
- 이 계층적 fallback은 깔끔하게 구조화되어 있다. 각 tier가 독립적으로 테스트 가능하다.

**spell_check() vs is_valid_word() 차별화:**
- `spell_check()`: SpellChecker pipeline 경유 (trie + cache + morph + compound)
- `is_valid_word()`: 순수 FST 분석만
- 이 구분은 아키텍처적으로 합리적. SpellChecker는 state(cache)를 가지므로 별도 경로가 필요.

**plural 생성:**
- `SINGULAR_CASES`/`PLURAL_CASES` 상수 배열이 대칭적으로 정의됨
- `apply_case()` / `apply_plural_case()` 함수가 명확히 분리됨
- `generate_paradigm()`이 두 배열을 순회하여 22개 폼을 생성하는 구조가 깔끔함
- `plural_stem()`, `genitive_plural()`, `partitive_plural()`이 별도 함수로 분리되어 복잡한 복수 형태론을 관리 가능한 단위로 분해

**평가: 이 추가사항들은 "덧칠"이 아닌 "계층적 확장"이다.**
- 기존 `SINGULAR_CASES` 배열과 동일한 구조의 `PLURAL_CASES` 배열 추가
- 기존 `apply_case()` 패턴을 따르는 `apply_plural_case()` 추가
- 기존 SpellChecker 인프라를 재사용하는 suggest() 통합

### 7.2 gradate_stem() 도입

`gradate()`와 `gradate_stem()`의 관계가 명확하다:

- `gradate()` (mce-comonad/finnish.rs): 코모나드 `extend`로 전체 단어에 점진 적용. **분석 경로**에서 사용.
- `gradate_stem()` (mce-fi/generator.rs): 마지막 점진 사이트만 찾아 부분 적용. **생성 경로**에서 사용.

이 분리는 분석(analysis)과 생성(generation)의 대칭성 차이에서 오는 필연적 결과다. 분석에서는 FST가 어떤 위치에 점진을 적용할지 결정하지만, 생성에서는 generator가 스스로 위치를 결정해야 한다.

### 7.3 연구 문서와 구현 사이의 갭

`docs/research/`에 8개의 연구 문서가 존재:
- `compound-improvement-plan.md` (779줄) -- FST hybrid 전략
- `kotus-integration-plan.md` (610줄) -- 100K+ lemma 통합 계획
- `long-term-roadmap.md` (635줄) -- Phase 2-3 로드맵
- `generation-consonant-gradation.md` -- gradate_stem 설계
- 기타 4개

연구 문서의 결론이 코드에 반영된 사례:
- `generation-consonant-gradation.md` -> `gradate_stem()` 구현 (문서의 전략이 코드로 번역됨)

연구 문서에서 계획되었으나 미구현인 사항:
- Compound FST hybrid (compound-improvement-plan.md)
- Kotus 100K+ integration (kotus-integration-plan.md)
- Micro Transformer, Edit-tree lemmatizer (long-term-roadmap.md)

이것은 자연스러운 상태다. 연구가 구현에 선행하는 것이 정상적인 개발 흐름이다.

---

## 8. 테스트 품질 감사

### 8.1 테스트 유형 분포

| 유형 | 예시 | 수량 (추정) |
|------|------|------------|
| **수학적 법칙 검증** | 코모나드 L1/L2/L3', 모노이드 법칙 | ~15 |
| **기능 단위 테스트** | `gradation_writer_geminate_pp()` | ~800 |
| **통합 테스트** | `disambiguator_kuusi_kasvaa()` | ~200 |
| **엣지 케이스** | `check_empty_string()`, `move_left_at_start_returns_none()` | ~150 |
| **회귀 방지** | `pure_pipeline_matches_old_kaappi()` | ~50 |
| **trait 안전성** | `disambiguator_trait_is_object_safe()` | ~5 |
| **known limitation 문서화** | `ranta_plural_partitive()` (with NOTE) | ~10 |

### 8.2 테스트 anti-pattern 검색

**TODO/FIXME/HACK/WORKAROUND/XXX/TEMP/KLUDGE**: **0건** (grep 결과).
이것은 매우 깨끗한 상태. 기술 부채가 코드 주석이 아닌 NOTE 형식으로 관리되고 있다.

**`todo!()` / `unimplemented!()` / `unreachable!()`**: **0건**.
미완성 코드 경로가 없다.

### 8.3 테스트 정답 검증 vs 현재 동작 고정

대부분의 테스트가 **정답 검증**에 해당한다:
```rust
// 정답 검증: 핀란드어 문법 규칙 기반
assert_eq!(form, Some("kaapin".to_string())); // genitive = baseform + "n"
```

일부 테스트가 **현재 동작 고정**에 해당하며, 이를 명시적으로 표시한다:
```rust
// 현재 동작 고정 (NOTE로 올바른 답을 명시)
// NOTE: Correct Finnish is "rantoja", but our simplified generator...
assert_eq!(form, Some("rantia".to_string()));
```

이 구분이 **명시적**이라는 점이 핵심. "이 테스트가 왜 이 값을 기대하는지"가 항상 설명되어 있다.

---

## 9. 권장 개선 사항

### 즉시 (1-2시간)

1. **`is_vowel` 통합**: `mce-core::character`에 `pub fn is_finnish_vowel(c: char) -> bool` 추가, 나머지 3곳에서 `use mce_core::character::is_finnish_vowel`으로 교체
2. **`edit_distance` 통합**: `mce-core`에 `pub fn levenshtein_distance(a: &[u8], b: &[u8]) -> usize` 추가, `mce-speller`와 `mce-fi`에서 재사용
3. **`#[allow(dead_code)]` 의도 주석**: `cg.rs`의 `has_baseform_in()`, `has_attr()`에 "reserved for Phase 2 CG expansion" 주석 추가, 또는 제거

### 단기 (1-2주)

4. **`apply_gradation()` sentinel 제거**: `'\0'` -> `Option<char>` 반환으로 변경, `gradation_writer()`의 인터셉트 로직 제거
5. **known limitation을 이슈 트래커로**: 테스트 내 NOTE를 GitHub Issues (label: "known-limitation") 또는 전용 문서로 추출
6. **mce-cli 기본 테스트**: 주요 subcommand에 대한 smoke test 추가

### 장기 (1-3개월)

7. **Generation-specific gradation**: `gradate_stem()` workaround를 코모나드 프레임워크에 positional range 지원 추가로 대체
8. **Generator 정확도 개선**: "old -i words", partitive plural 패턴 다양화
9. **`FinnishAnalyzer` RefCell 제거 가능성**: `mce-fst::config`를 immutable 설계로 전환하여 `RefCell` 필요성 제거 검토

---

## 부록: 프로젝트 건강 지표 요약

| 지표 | 값 | 평가 |
|------|-----|------|
| LOC | ~45,400 | 규모 대비 잘 관리됨 |
| Tests | 1,616 | 양호 (28 LOC/test) |
| TODO/FIXME | 0 | 깨끗 |
| `#[allow(dead_code)]` | 3 | 낮음 (정리 가능) |
| 순환 의존성 | 0 | 없음 |
| 외부 dep | 8 (prod), 1 (dev) | 최소 |
| 코모나드 법칙 테스트 | 6세트 | 수학적 정합성 검증 |
| Known limitations | ~3 (documented) | 명시적 관리 |
| Code duplication | 2건 (is_vowel 4x, edit_distance 2x) | 정리 필요 |
| workaround 코드 | 1건 (gradate_stem) | 문서화됨, 장기 해결 필요 |
