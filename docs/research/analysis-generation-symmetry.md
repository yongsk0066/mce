---
title: Analysis-Generation Symmetry
created: 2026-03-05
commit: 37462bf
status: active
relates-to:
  - irregular-verb-generation.md
  - generation-consonant-gradation.md
  - long-term-roadmap.md
---

# Analysis-Generation Symmetry: Mathematical Framework and Architecture Design

## 1. Executive Summary (요약)

MCE v0.3.0의 분석(analysis)과 생성(generation) 파이프라인은 구조적 비대칭을 가진다. 분석은 VFST 트랜스듀서 + Comonad 모포포놀로지 + CG-lite + Suffix Tagger로 구성된 정밀한 파이프라인인 반면, 생성은 `generator.rs`의 접미사 연결(concatenation) + 정규 패턴 매칭에 의존한다. 이 비대칭의 근본 원인은:

1. **지식의 편재**: FST(mor.vfst)가 ~400K+ 전이(transition)에 인코딩한 불규칙 어간 변화, 예외 패턴, 복합어 구조 등이 생성기에는 전혀 전달되지 않는다.
2. **방향성의 고정**: VFST 순회 알고리즘(`UnweightedTransducer::next_inner`)이 입력 심볼(`sym_in`) 매칭에 의한 순방향 탐색만 지원하며, 출력 심볼(`sym_out`)에 의한 역방향 탐색 경로가 없다.
3. **Comonad의 단방향성**: Writer Comonad의 coKleisli 화살표들(`gradation_writer`, `harmony_writer`, `possessive_writer`)은 분석 방향(형태론적 표현 -> 표면형)으로 설계되었으며, 역방향(표면형 -> 형태론적 표현)은 정의되지 않았다.

본 문서는 이 비대칭을 범주론적 프레임워크로 정밀하게 진단하고, FST 역방향 탐색의 실현 가능성을 평가하며, WASM 365KB / <5ms 제약 하에서의 실용적 아키텍처 옵션을 비교 평가한다.

**권장 아키텍처**: 옵션 D (하이브리드) — 분석 결과 캐시 기반 생성 + Comonad 정방향 파이프라인 + 예외 테이블. FST 역방향 탐색(옵션 A)은 VFST 포맷의 구조적 한계로 비실용적이며, 별도 생성 FST(옵션 B)는 바이너리 크기 제약에 위배된다.

---

## 2. 현재 비대칭 지점 매핑

### 2.1 분석 파이프라인 (Analysis Path)

```
WASM API: MceEngine.analyze(word)
  └─> FinnishAnalyzer.analyze(word, word_len)          [mce-fi/src/morphology.rs:124]
      └─> UnweightedTransducer.prepare(config, input)  [mce-fst/src/unweighted.rs:217]
      └─> UnweightedTransducer.next(config, output)    [mce-fst/src/unweighted.rs:101]
          ├─ sym_in matching: config.input_symbol_stack[input_depth] == ct.sym_in
          ├─ flag diacritics: P/C/U/R/D operations
          └─ sym_out accumulation: output_symbol_stack -> symbol_strings join
      └─> tag_parser: parse FST output string -> Analysis struct
          ├─ parse_basic_attributes() -> CLASS, SIJAMUOTO, NUMBER, etc.
          ├─ parse_baseform() -> lemma
          └─ parse_structure() -> compound structure markers
```

분석 파이프라인의 각 단계가 보유한 지식:

| 단계 | 보유 지식 | 크기 |
|------|----------|------|
| M1 (Succinct Trie) | 사전 단어 집합 (존재 여부) | ~0.5-1MB |
| M3 (VFST/FST) | 표면형 -> 형태론적 분석 매핑 (모든 변곡, 불규칙, 복합어) | ~3.8MB |
| M2' (Comonad) | 자음교체 11패턴, 모음조화 3아키음소, 소유접미사 V복사 | ~2KB (코드) |
| M4' (CG+Tagger) | 문맥 기반 중의성 해소, POS 확률 분포 | ~5MB (모델) |

### 2.2 생성 파이프라인 (Generation Path)

```
WASM API: MceEngine.generate_form(baseform, case, number)
  └─> MorphGenerator.generate(baseform, features)      [mce-fi/src/generator.rs:363]
      └─> find_case(case_name) or find_plural_case()    [SINGULAR_CASES / PLURAL_CASES lookup]
      └─> apply_case(baseform, case_info)               [mce-fi/src/generator.rs:559]
          ├─ gradate(baseform, grade)                    [mce-comonad/src/finnish.rs via writer.rs]
          │   └─ WriterZipper.extend(gradation_writer)   [11 patterns: pp->p, tt->t, kk->k, etc.]
          ├─ format!("{}{}", graded_stem, suffix)        [단순 문자열 연결]
          ├─ harmonize(&intermediate)                    [Zipper.extend(apply_vowel_harmony)]
          └─ apply_possessive_to_word()                  [Zipper.extend(apply_possessive)]
```

생성 파이프라인의 보유 지식:

| 단계 | 보유 지식 | 누락 지식 |
|------|----------|----------|
| CaseInfo 테이블 | 11 격변화 접미사 + 등급(strong/weak) | 어간 변이형(allomorphs) |
| `gradate()` | 11 자음교체 패턴 (정규) | 불규칙 교체 (예: `mies->miehen`) |
| `harmonize()` | 모음조화 A/O/U 해소 | 복합어 경계에서의 조화 리셋 |
| `plural_stem()` | 규칙적 복수 어간 변화 6패턴 | 불규칙 복수 (예: `lapsi->lapset` vs `*lapse+t`) |
| `classify_verb()` | 4가지 동사 유형 분류 | 불규칙 동사 (`olla`, `tehdä`, `nähdä`) |

### 2.3 비대칭 매트릭스

| 지식 범주 | 분석에 있는가? | 생성에 있는가? | 비대칭 유형 |
|----------|:---:|:---:|------------|
| 규칙적 자음교체 (11패턴) | FST + Comonad | Comonad | **대칭** |
| 모음조화 (A/O/U) | FST + Comonad | Comonad | **대칭** |
| 소유접미사 V복사 | FST + Comonad | Comonad | **대칭** |
| 불규칙 어간 변이 (mies/miehen) | FST | 없음 | **비대칭** |
| 복합어 분석/생성 | FST + STRUCTURE | 없음 | **비대칭** |
| 어간 교체 (vesi/veden, lapsi/lapsen) | FST | 없음 | **비대칭** |
| 파생접미사 결합 규칙 | FST | 없음 | **비대칭** |
| 동사 불규칙 변화 (olla, tehdä) | FST | 없음 | **비대칭** |
| 문맥 기반 형태 선택 | CG + Tagger | 없음 | **비대칭** |

**핵심 발견**: Comonad 파이프라인(자음교체/모음조화/V복사)은 이미 양방향으로 작동 가능하며 실제로 생성에서 활용 중이다. 진짜 비대칭은 **FST에 인코딩된 어휘적 지식**(불규칙 어간, allomorph 선택 규칙)이 생성에 전달되지 않는 것이다.

---

## 3. 수학적 프레임워크: 양방향 형태론의 범주론적 모델

### 3.1 현재 구조의 범주론적 해석

MCE의 분석 파이프라인을 범주론적으로 표현하면:

**대상(Objects)**:
- `Surf` = 표면형 문자열의 범주 (예: "kaapin", "taloissa")
- `Morph` = 형태론적 분석의 범주 (예: `(kaappi, GEN, SG)`)
- `Char` = 문자 시퀀스의 범주
- `Tag` = 형태 태그의 범주

**사상(Morphisms)**:

FST 순회는 사상 `A: Surf -> P(Morph)` (P는 멱집합 함수자, 비결정성을 인코딩)로서, 하나의 표면형을 다수의 형태론적 분석에 매핑한다:

```
A("kaapin") = { (kaappi, GEN, SG), (kaappi, ACC, SG), (kaappi, INST, SG) }
```

Comonad 규칙은 coKleisli 화살표 `k: W(Char) -> Char`이며, 여기서 `W = Zipper`는 리스트 코모나드이다. `extend(k)`: `W(Char) -> W(Char)`는 국소 변환을 전역 변환으로 승격한다.

CG 규칙은 coKleisli 화살표 `c: Zipper(ReadingSet) -> ReadingSet`로, 형태론적 중의성을 해소한다.

### 3.2 수반 함자 쌍 (Adjoint Functor Pair)

분석과 생성의 이상적 관계를 수반 관계 `A ⊣ G`로 모델링할 수 있다:

```
A: Surf -> P(Morph)    (분석: 표면형 -> 분석 집합)
G: Morph -> P(Surf)    (생성: 형태 명세 -> 표면형 집합)
```

수반 조건 (Galois connection 해석):

```
m ∈ A(s)  ⟺  s ∈ G(m)
```

즉 "s의 분석 중에 m이 있다" iff "m을 생성하면 s가 나온다". 이것은 FST의 본질적 성질이다: 유한 상태 트랜스듀서 `T: Sigma* -> Delta*`에 대해, 역 트랜스듀서 `T^{-1}: Delta* -> Sigma*`는 입출력 심볼을 교환하면 자동으로 정의된다.

그러나 이 수반 관계는 **엄밀한 범주론적 수반(adjunction)**이 아니라 **Galois connection** (순서 범주에서의 수반)에 더 가깝다. 엄밀한 수반은 자연 변환 `eta: Id -> GA`와 `epsilon: AG -> Id`의 존재를 요구하며, 형태론에서 이는:

- `eta(s) ∈ GA(s)`: 단어 s를 분석하고 그 분석을 생성하면, s가 결과에 포함되어야 함 (**round-trip 조건**)
- `epsilon(m) ∈ AG(m)`: 형태 명세 m을 생성하고 그 결과를 분석하면, m이 결과에 포함되어야 함

이 round-trip 조건은 **정보 손실 없는 양방향 매핑**을 요구하는데, 실제 핀란드어 형태론에서는:

1. **분석 후 생성**: `G(A("kaapin"))` → `{"kaapin"}` (GEN)과 `{"kaapin"}` (ACC)은 동일 표면형이므로 성립
2. **생성 후 분석**: `A(G(kaappi, GEN, SG))` = `A("kaapin")` → `{(kaappi, GEN), (kaappi, ACC)}` ⊋ `{(kaappi, GEN)}` — **초과 분석** 발생

따라서 `epsilon`은 포함(inclusion)이지 동치가 아니며, 이는 **lax adjunction** (느슨한 수반)에 해당한다. 이 불완전성은 본질적이다: 핀란드어의 격변화 형태 동음이의(syncretic forms)가 존재하는 한, 엄밀한 수반은 불가능하다.

### 3.3 Writer Comonad의 양방향성

현재 MCE의 Writer Comonad 구조:

```
W(A) = (DeletionSet, Zipper(A))

extract: W(A) -> A
extend:  (W(A) -> (DeletionSet, B)) -> W(A) -> W(B)
```

자음교체 coKleisli 화살표:

```
gradation_writer: W(Char) -> (DeletionSet, Char)
  // 입력: 형태론적 표현 (어간 + 아키음소 접미사)
  // 출력: 표면형에 가까운 문자열
```

이 화살표의 **역**은 존재하는가?

자음교체의 수학적 성질:
- **약화(weakening)**: pp -> p, kk -> k, tt -> t (양적 교체)
- **변환(alternation)**: p -> v, t -> d, k -> ∅ (질적 교체)
- **클러스터 변환**: mp -> mm, nt -> nn, nk -> ng, lt -> ll, rt -> rr

약화의 역(강화)은 **결정적(deterministic)**이다: p -> pp, k -> kk, t -> tt는 유일하다.

그러나 변환의 역은 **비결정적(non-deterministic)**이다:
- v -> ? : v는 `p->v`의 약형이거나, 원래의 v일 수 있다
- d -> ? : d는 `t->d`의 약형이거나, 원래의 d일 수 있다 (특히 차용어)
- ∅ -> ? : 삭제된 위치는 표면형에서 보이지 않으므로 역으로 복원 불가능

이 비결정성은 **역 coKleisli 화살표가 단일 함수가 아닌 관계(relation)**임을 의미한다. 범주론적으로는 `Char -> P(W(Char))`로 표현되며, 이는 **코모나드가 아닌 모나드적 구조**(Kleisli 화살표)를 요구한다:

```
generation_gradation: Char -> P(W(Char))   // Kleisli arrow for P monad
```

### 3.4 Bimachine 관점

MCE의 doc 주석에서 언급된 Schutzenberger bimachine과의 관계:

**정의**: Bimachine `B = (Q_L, Q_R, q_0^L, q_0^R, delta_L, delta_R, omega)`는:
- `Q_L, delta_L`: 좌방향 오토마톤 (왼쪽 문맥 계산)
- `Q_R, delta_R`: 우방향 오토마톤 (오른쪽 문맥 계산)
- `omega(q_l, a, q_r)`: 출력 함수 (양쪽 문맥과 현재 심볼로 출력 결정)

`Zipper.extend`는 정확히 이 bimachine 계산을 구현한다:
- `peek_left(n)`: 좌방향 문맥 상태
- `peek_right(n)`: 우방향 문맥 상태
- coKleisli arrow: 출력 함수 `omega`

**Bimachine의 역(inverse)**:

FST(유한 상태 트랜스듀서)의 역은 항상 정의 가능하지만, bimachine의 역은 그렇지 않다. Bimachine은 **결정적 좌순차 함수**를 인코딩하며, 그 역은 일반적으로 비결정적이므로 bimachine으로 표현 불가능하다.

그러나 MCE의 음운 규칙들은 **거의 결정적 역**을 가진다:
- 모음조화: `a/ä -> A`, `o/ö -> O`, `u/y -> U` — 결정적 역 (항상 아키음소로 복원 가능)
- V복사: `V -> 선행 모음` — 선행 모음이 동일하면 결정적 역
- 자음교체: 앞서 분석한 대로, 비결정적 역

### 3.5 Inverse Semigroup 접근

양방향 재작성(rewriting)을 위한 대안적 프레임워크로서 **역반군(inverse semigroup)**을 고려할 수 있다.

**정의**: 역반군 S에서 모든 원소 a에 대해 유일한 역원소 a*가 존재하여 `a a* a = a`와 `a* a a* = a*`를 만족한다. (군과 달리 `a a* = 1`을 요구하지 않음.)

형태론적 규칙 `r: pp -> p` (약화)에 대해:
- `r*: p -> pp` (강화)
- `r r* r = r`: pp -> p -> pp -> p (재적용 시 동일 결과)
- `r* r r* = r*`: p -> pp -> p -> pp (역 재적용 시 동일 결과)

이 구조는 핀란드어 자음교체의 양적 교체(geminate weakening)에 대해 **정확히 성립**한다. 그러나 질적 교체(p->v, t->d)는 역반군 조건을 만족하지 않는다: `p -> v -> ? -> v`에서 두 번째 역변환이 비결정적이므로 `r* r r* != r*`일 수 있다.

**결론**: 역반군 프레임워크는 양적 교체에만 적용 가능하며, 전체 핀란드어 형태음운론에는 불충분하다.

### 3.6 종합: 수학적 프레임워크 선택

| 프레임워크 | 강점 | 한계 | 적용 범위 |
|-----------|------|------|----------|
| Adjoint functor pair (A ⊣ G) | 분석-생성 관계의 정확한 특성화 | Lax adjunction만 가능 (syncretic forms) | 전체 시스템 수준 |
| Dual Comonad / Monad | Comonad 역 = Monad Kleisli | 비결정적 역은 P monad 필요 | Comonad 규칙의 역방향 |
| Inverse semigroup | 양적 교체에 수학적으로 정확 | 질적 교체에 부적합 | 자음교체 일부 |
| Bimachine pair | Zipper 와 직접 대응 | 비결정적 역은 bimachine 아님 | 음운 규칙 |

**권장**: 시스템 수준에서는 **Galois connection** (Surf, ⊆) ⇄ (Morph, ⊆)으로 모델링하되, 규칙 수준에서는 결정적 역이 존재하는 규칙(모음조화, 양적 교체)과 비결정적 역이 필요한 규칙(질적 교체, 어간 교체)을 분리하여 처리한다.

---

## 4. FST 역방향 탐색 가능성 (VFST 포맷 분석)

### 4.1 VFST 포맷 구조

현재 VFST 바이너리 포맷 (`mce-fst/src/format.rs`):

```
Header (16 bytes):
  cookie1: u32 (0x00013A6E)
  cookie2: u32 (0x000351FA)
  weighted: u8
  padding: 7 bytes

Symbol Table:
  symbol_count: u16
  symbols: null-terminated strings (flag diacritics + normal chars + multi-char tags)

Transition Table:
  transitions: [Transition; N]  (each 8 bytes)
```

`Transition` 구조체 (`mce-fst/src/transition.rs:8`):

```rust
#[repr(C)]
pub struct Transition {
    pub sym_in: u16,     // 입력 심볼 인덱스
    pub sym_out: u16,    // 출력 심볼 인덱스
    pub trans_info: u32, // target_state (24bit) | more_transitions (8bit)
}
```

### 4.2 순방향 탐색 알고리즘

`UnweightedTransducer::next_inner()` (`mce-fst/src/unweighted.rs:101-184`):

```
for each transition ct at current state:
    if ct.sym_in == input[input_depth]:   // 입력 심볼 매칭
        push state, advance input_depth
        record ct.sym_out                 // 출력 심볼 기록
```

핵심: 탐색은 `sym_in`에 의해 구동된다. 상태 `s`에서 입력 심볼 `a`가 들어오면, 전이 테이블을 순차 스캔하여 `sym_in == a`인 전이를 찾는다.

### 4.3 역방향 탐색의 요구사항

생성(역방향)은 `sym_out`에 의해 구동되어야 한다: 출력 심볼(형태 태그)을 입력으로 받아 입력 심볼(표면 문자)을 출력해야 한다.

```
for each transition ct at current state:
    if ct.sym_out == tag[tag_depth]:      // 출력 심볼 매칭
        push state, advance tag_depth
        record ct.sym_in                  // 입력 심볼 기록 (= 표면 문자)
```

### 4.4 실현 가능성 분석

**문제 1: 전이 테이블 정렬**

현재 전이 테이블은 `sym_in`으로 정렬되어 있지 않으며 (순차 스캔 방식), `sym_out`으로도 정렬되어 있지 않다. 역방향 탐색은 모든 전이를 순차 스캔해야 하므로, 상태당 전이 수가 많을수록 비효율적이다.

다만 현재 순방향 탐색도 순차 스캔이므로 (이진 탐색 아님), **동일한 알고리즘 복잡도**에서 `sym_in` 대신 `sym_out`을 매칭하는 것은 가능하다.

**문제 2: 비결정성 폭발**

분석에서: 하나의 표면형이 여러 분석을 가질 수 있지만, 이는 트랜스듀서의 서로 다른 경로로 표현된다. 대부분의 상태에서 현재 입력 심볼에 매칭되는 전이는 1-3개이다.

생성에서: 형태 태그(출력 심볼)는 주로 multi-char 심볼 `[Ln]`, `[Xp]`, `[Sn]` 등인데, 이들은 상태 그래프에서 **소수의 특정 상태에서만 출현**한다. 그러나 해당 태그에서 다수의 입력 심볼로 분기하면, 경로 수가 지수적으로 증가할 수 있다.

예: `[Sn]` (nominative case) 태그가 나올 수 있는 상태에서, 가능한 `sym_in` 전이가 수십 개라면, 각각에 대해 다시 분기하여 역방향 경로 수가 폭발한다.

**문제 3: Flag Diacritics**

현재 VFST의 flag diacritics (P/C/U/R/D 연산)는 **순방향 의미론**을 가진다. 역방향 탐색 시 flag 상태의 올바른 전파가 보장되지 않는다. 특히:

- `@P.feature.value@` (positive set): 역방향에서는 feature가 이미 value여야 함을 사후 검증해야 함
- `@D.feature@` (disallow): 역방향에서 이 제약의 의미가 달라짐
- `@U.feature.value@` (unification): 역방향에서 unification의 방향이 반전

Flag diacritics를 역방향으로 올바르게 처리하려면, 전이 그래프에 대한 비자명한(non-trivial) 역 의미론 구현이 필요하다.

**문제 4: VFST는 Voikko의 분석 전용 포맷**

VFST (Voikko FST)는 `voikko-fst`에서 유래하며, 설계 목적이 **분석 전용**이다. Voikko 프로젝트 자체가 맞춤법 검사 + 제안(suggest) + 하이픈네이션에 초점을 두고 있으며, 형태 생성은 Voikko의 범위 밖이다. 따라서 VFST 바이너리 포맷이 역방향 탐색에 최적화되어 있을 이유가 없다.

### 4.5 결론

| 요소 | 역방향 탐색 가능? | 실용적? |
|------|:---:|:---:|
| 전이 테이블 매칭 (`sym_out` 기반) | 가능 | O (순차 스캔으로 동일 복잡도) |
| 비결정성 제어 | 가능 (pruning 필요) | 삼각 (경로 폭발 위험) |
| Flag diacritics 역전 | 이론적 가능 | X (역 의미론 구현 비용 과다) |
| WASM 365KB 제약 내 구현 | 가능 | X (추가 코드량 ~2-3KB) |
| 생성 품질 보장 | 불확실 | X (정확한 표면형 선택 보장 불가) |

**판정: 비실용적**. VFST 역방향 탐색은 기술적으로 가능하나, flag diacritics 역 의미론과 비결정성 폭발 제어의 엔지니어링 비용이 매우 높으며, 생성 품질 보장이 어렵다.

---

## 5. 경쟁 시스템 참조

### 5.1 Omorfi: 별도 생성 FST

Omorfi는 분석과 생성을 **별도의 FST 바이너리**로 컴파일한다:
- `omorfi.analyse.hfst`: 분석기 (표면형 -> 분석)
- `omorfi.generate.hfst`: 생성기 (분석 -> 표면형)

빌드 파이프라인에서 `hfst-invert` 연산을 통해 분석기의 `sym_in`과 `sym_out`을 교환하여 생성기를 만든다. 이 접근법의 핵심:

- **동일 어휘 DB**에서 양방향 FST가 생성되므로 지식 동기화가 보장됨
- 생성기 크기 ≈ 분석기 크기 (~3.8MB 추가 예상)
- HFST 라이브러리의 `hfst-invert`, `hfst-compose` 등의 연산에 의존

### 5.2 HFST: invert() 연산

HFST (Helsinki Finite-State Technology)의 FST 반전 연산:

```
hfst-invert < analyser.hfst > generator.hfst
```

내부적으로 모든 전이의 `(sym_in, sym_out)` 쌍을 `(sym_out, sym_in)`으로 교환한다. 결과 FST는 역방향 탐색이 아니라, **순방향 알고리즘으로 역방향 매핑을 수행**하는 새 FST이다.

이 접근의 장점:
- Flag diacritics 문제 없음 (HFST가 invert 시 flag 재배치 처리)
- 비결정성은 원래 FST와 동일 수준 (epsilon 전이 처리 포함)
- 생성 품질이 분석 품질과 동일

단점:
- 별도 바이너리 크기 (MCE의 ~3.8MB 추가 불가능)

### 5.3 Voikko (Java): 생성 미지원

Voikko의 Java 라이브러리(`libvoikko`)는 형태 생성을 공식 지원하지 않는다. Voikko의 핵심 기능은 맞춤법 검사, 제안, 하이픈네이션이며, 이는 모두 분석 방향(표면형 -> 분석)만 요구한다.

### 5.4 OpenFst

OpenFst의 관련 연산:
- `Invert()`: 전이의 입출력 교환
- `Compose(A, B)`: 두 FST의 합성 (A의 출력 = B의 입력)
- `ShortestPath()`: 최단 경로 추출 (가중 FST에서)

MCE에서 OpenFst를 직접 사용하는 것은 불가능하다 (WASM 제약, C++ 의존). 그러나 OpenFst의 `Invert` 개념을 VFST 포맷에 적용하는 것은 가능하다 — **빌드 타임**에 반전된 VFST를 생성하는 방식으로.

---

## 6. 아키텍처 옵션 비교 매트릭스

### 옵션 A: FST 역방향 조회 + Comonad 생성 파이프라인

**개요**: 기존 mor.vfst를 런타임에 역방향 탐색하여, 형태 태그 입력 -> 표면형 출력을 수행한다.

| 기준 | 평가 |
|------|------|
| 구현 난이도 | 극히 높음 (flag diacritics 역전, 비결정성 제어) |
| 추가 바이너리 크기 | +0 (기존 mor.vfst 재사용) |
| 추가 WASM 코드 | +2-3KB (역방향 탐색 로직) |
| 생성 정확도 | 불확실 (비결정성 해소 전략 필요) |
| 지연시간 | 높을 수 있음 (경로 폭발 시 >5ms) |
| 유지보수성 | 낮음 (VFST 포맷 변경 시 역전 로직도 갱신 필요) |

### 옵션 B: 별도 생성 VFST (Omorfi 방식)

**개요**: 빌드 시점에 mor.vfst를 반전하여 gen.vfst를 생성하고, WASM에서 추가 로딩한다.

| 기준 | 평가 |
|------|------|
| 구현 난이도 | 중간 (빌드 도구 필요, 런타임은 기존 코드 재사용) |
| 추가 바이너리 크기 | **+3.8MB** (gen.vfst ≈ mor.vfst 크기) |
| 추가 WASM 코드 | +0 (동일 UnweightedTransducer 재사용) |
| 생성 정확도 | 높음 (분석기와 동일 품질) |
| 지연시간 | 낮음 (순방향 탐색 = 분석과 동일) |
| 유지보수성 | 높음 (빌드 시 자동 생성) |

**치명적 문제**: 총 배포 크기 ~13MB (9.2 + 3.8)는 브라우저 배포에 부담. gzip 후 ~4-5MB 추가.

### 옵션 C: 분석 결과 캐시 기반 생성 (Analyze-then-Invert)

**개요**: 주어진 baseform의 가능한 변곡형들을 열거하여 분석하고, 원하는 형태 자질과 일치하는 표면형을 반환한다.

알고리즘:
```
generate(baseform, target_features):
  candidates = generate_candidates(baseform)  // 규칙 기반 후보 생성
  for candidate in candidates:
    analyses = analyzer.analyze(candidate)     // FST 분석
    for analysis in analyses:
      if analysis.baseform == baseform && matches(analysis, target_features):
        return candidate                        // 검증된 표면형
  return rule_based_fallback(baseform, target_features)  // 규칙 기반 폴백
```

| 기준 | 평가 |
|------|------|
| 구현 난이도 | 중간 (후보 열거 + 분석 검증) |
| 추가 바이너리 크기 | +0 (기존 분석기 재사용) |
| 추가 WASM 코드 | +1-2KB |
| 생성 정확도 | 높음 (FST 분석으로 검증) |
| 지연시간 | 중간 (후보 수 * 분석 시간, 캐싱으로 완화) |
| 유지보수성 | 높음 (분석기 갱신 시 자동 반영) |

**핵심 아이디어**: "생성은 검증된 분석의 역"이다. 규칙 기반으로 후보를 만들되, FST 분석기로 검증함으로써, FST의 지식을 간접적으로 생성에 활용한다.

### 옵션 D: 하이브리드 (규칙 기반 + 예외 테이블 + FST 검증)

**개요**: 현재 Comonad 기반 생성기를 유지하되, 세 가지 강화:
1. 불규칙 어간 예외 테이블 (mies->mieh, vesi->ved, lapsi->laps 등)
2. FST 분석 검증 (생성 결과를 분석하여 올바른 형태인지 확인)
3. 규칙 기반 폴백 실패 시 캐시 기반 생성(옵션 C) 적용

```
generate(baseform, target_features):
  // Phase 1: 규칙 기반 생성 (현재 generator.rs 확장)
  candidate = comonad_generate(baseform, target_features)

  // Phase 2: 예외 테이블 조회
  if exceptions.contains(baseform):
    candidate = exception_generate(baseform, target_features)

  // Phase 3: FST 검증
  if analyzer.analyze(candidate).any(|a| a.baseform == baseform && matches(a, target_features)):
    return candidate  // 검증 성공

  // Phase 4: 캐시 기반 폴백 (옵션 C)
  return analyze_then_invert(baseform, target_features)
```

| 기준 | 평가 |
|------|------|
| 구현 난이도 | 낮음-중간 (점진적 개선 가능) |
| 추가 바이너리 크기 | +5-20KB (예외 테이블) |
| 추가 WASM 코드 | +1-2KB |
| 생성 정확도 | 높음 (FST 검증 + 예외 테이블) |
| 지연시간 | 낮음 (규칙 기반 + 검증 = ~1-2ms) |
| 유지보수성 | 높음 (예외 테이블만 갱신) |

### 비교 매트릭스

| 기준 | A (역방향 FST) | B (별도 FST) | C (분석 기반) | **D (하이브리드)** |
|------|:---:|:---:|:---:|:---:|
| 추가 크기 | +0 | **+3.8MB** | +0 | +5-20KB |
| 구현 난이도 | 극히 높음 | 중간 | 중간 | **낮음** |
| 정확도 | 불확실 | 높음 | 높음 | **높음** |
| 지연시간 | 높을 수 있음 | 낮음 | 중간 | **낮음** |
| WASM 제약 충족 | O | **X** | O | **O** |
| 점진적 개선 | X | X | O | **O** |
| 불규칙 동사 처리 | O | O | O | **O** |

---

## 7. 권장 아키텍처 + 단계별 구현 로드맵

### 7.1 권장: 옵션 D (하이브리드)

옵션 D를 세 단계로 구현한다:

### Phase D1: 예외 테이블 도입 (v0.4.x)

**목표**: 가장 빈번한 불규칙 어간 변화를 예외 테이블로 커버.

**구현**:
```rust
// mce-fi/src/generator.rs 에 추가
struct StemException {
    baseform: &'static str,
    stem_map: &'static [(&'static str, &'static str)], // (case_pattern, stem)
}

const NOUN_EXCEPTIONS: &[StemException] = &[
    StemException { baseform: "mies", stem_map: &[("gen", "mieh"), ("part", "mies")] },
    StemException { baseform: "vesi", stem_map: &[("gen", "ved"), ("part", "vet")] },
    StemException { baseform: "lapsi", stem_map: &[("gen", "laps"), ("part", "las")] },
    // ... 50-100 entries covering the most frequent irregular nouns
];
```

**예상 크기**: ~5-10KB (100개 예외 항목)
**정확도 향상**: 불규칙 명사/동사의 정확한 생성

### Phase D2: FST 검증 레이어 (v0.5.x)

**목표**: 생성 결과를 FST 분석기로 검증하여 잘못된 형태를 제거.

**구현**:
```rust
// mce-fi/src/generator.rs
impl MorphGenerator {
    pub fn generate_verified(
        &self,
        baseform: &str,
        features: &[(&str, &str)],
        analyzer: &FinnishAnalyzer,
    ) -> Option<String> {
        // Phase 1: 규칙 기반 생성
        let candidate = self.generate(baseform, features)?;

        // Phase 2: FST 검증
        let chars: Vec<char> = candidate.chars().collect();
        let analyses = analyzer.analyze(&chars, chars.len());
        let target_case = features.iter()
            .find(|(k, _)| *k == "SIJAMUOTO")
            .map(|(_, v)| *v);

        for analysis in &analyses {
            if analysis.get(ATTR_BASEFORM) == Some(baseform) {
                if let Some(tc) = target_case {
                    if analysis.get(ATTR_SIJAMUOTO) == Some(tc) {
                        return Some(candidate);
                    }
                }
            }
        }

        // Phase 3: 규칙 기반 결과가 검증 실패 시 None 반환
        // (또는 Phase D3의 캐시 기반 폴백으로 위임)
        None
    }
}
```

**변경 사항**: WASM API의 `generate_form()`에 analyzer 참조 전달. `MceEngine`이 이미 `FinnishAnalyzer`를 보유하고 있으므로 추가 의존성 없음.

### Phase D3: Analyze-then-Invert 폴백 (v0.6.x)

**목표**: 규칙 기반 생성이 실패하는 경우(불규칙 형태, 예외 테이블에 없는 경우)에 대한 완전한 폴백.

**알고리즘**:
```rust
fn analyze_then_invert(
    baseform: &str,
    target_features: &[(&str, &str)],
    analyzer: &FinnishAnalyzer,
) -> Option<String> {
    // 후보 생성: baseform의 가능한 변형들을 열거
    let candidates = generate_candidates(baseform);
    // candidates: baseform + 모든 가능한 접미사 조합
    //             + 자음교체 변이형 + 모음 변이형

    for candidate in candidates {
        let chars: Vec<char> = candidate.chars().collect();
        let analyses = analyzer.analyze(&chars, chars.len());
        for analysis in &analyses {
            if analysis.get(ATTR_BASEFORM) == Some(baseform)
                && matches_features(analysis, target_features)
            {
                return Some(candidate);
            }
        }
    }
    None
}
```

**후보 생성 전략**:
- baseform의 마지막 1-3 문자를 가능한 접미사로 교체
- 자음교체 패턴의 역방향 적용 (강형/약형 양쪽 시도)
- 모음조화의 양쪽 변이(back/front) 시도
- 복수 어간 변이(-i- 삽입, 최종 모음 교체) 시도

이 전략의 핵심은 **후보 수를 제한**하는 것이다. 핀란드어 격변화는 15격 * 2수(sg/pl) = 30형태이므로, 각 baseform에 대해 최대 ~100-200개의 후보를 시도하면 대부분 커버된다. 각 분석이 ~30us이면, ~200 * 30us = ~6ms로 5ms 제약에 근접하지만, 캐싱으로 완화 가능하다.

### 7.2 WASM API 확장

```rust
// 새 API 메서드
MceEngine.generate_form_verified(baseform, case, number) -> String
  // FST 검증 포함 생성. 검증 실패 시 빈 문자열 반환.

MceEngine.generate_paradigm_verified(baseform) -> String
  // FST 검증 포함 전체 패러다임 생성.
```

기존 `generate_form()`, `generate_paradigm()`은 하위 호환을 위해 유지하되, 내부적으로 검증 레이어를 추가한다.

---

## 8. WASM 제약 내 실현 가능성 분석

### 8.1 크기 제약 (365KB WASM + 9.2MB 배포)

| 구성 요소 | 현재 크기 | 옵션 D 추가 | 합계 |
|----------|----------|-----------|------|
| WASM binary | 365KB | +1-2KB | ~367KB |
| mor.vfst | ~3.8MB | +0 | ~3.8MB |
| suffix_tagger.bin | ~5.0MB | +0 | ~5.0MB |
| wordlist.txt | ~0.4MB | +0 | ~0.4MB |
| 예외 테이블 | 0 | +5-20KB | +5-20KB |
| **총합** | **~9.2MB** | **+6-22KB** | **~9.2MB** |

**판정**: 옵션 D는 크기 제약을 충족한다. 예외 테이블 20KB는 전체 배포 크기의 0.2%에 불과하다.

### 8.2 지연시간 제약 (<5ms per sentence)

| 작업 | 현재 | 옵션 D Phase 1 | Phase 2 | Phase 3 |
|------|------|:---:|:---:|:---:|
| 단일 단어 생성 | <0.1ms | <0.1ms | <0.2ms | <1ms |
| 패러다임 생성 (22형) | <1ms | <1ms | <3ms | <5ms (캐싱 시 <1ms) |
| 문장 분석 | ~1.35ms | 동일 | 동일 | 동일 |

**판정**: Phase 2(FST 검증)까지는 5ms 제약을 안전하게 충족. Phase 3(analyze-then-invert)은 첫 호출 시 지연이 있으나, LRU 캐시로 후속 호출은 <0.1ms.

### 8.3 코드 복잡도

| Phase | 추가 LOC (예상) | 복잡도 |
|-------|:---:|------|
| D1: 예외 테이블 | ~200-400 | 낮음 (정적 데이터 + lookup) |
| D2: FST 검증 | ~100-200 | 중간 (분석기 호출 + 결과 매칭) |
| D3: 캐시 기반 폴백 | ~300-500 | 중간 (후보 열거 + 반복 분석) |
| **총합** | **~600-1100** | |

현재 `generator.rs`가 ~1300 LOC이므로, 총 ~2000-2400 LOC으로 증가. 관리 가능한 범위이다.

---

## 9. 논문 기여 가능성

### 9.1 Paper-2 (Morphological Fingerprint, SIGMORPHON 2026)

본 연구의 범주론적 프레임워크(Section 3)는 Paper-2의 핵심 기여로 연결 가능:

- **Galois connection으로서의 분석-생성 관계**: FST 기반 형태론 시스템에서의 보편적 성질
- **Lax adjunction의 필요성**: Syncretic forms에 의한 엄밀한 수반의 불가능성 증명
- **Writer Comonad의 (부분적) 자기쌍대성**: 결정적 역이 존재하는 규칙과 그렇지 않은 규칙의 분류

### 9.2 Paper-5 (Comonadic Classification, ACL/EMNLP 2027)

옵션 D의 아키텍처는 Paper-5의 시스템 기여로 활용 가능:

- **Comonad + FST hybrid**: 규칙 기반 생성과 데이터 기반 검증의 결합
- **Analyze-then-Invert 패턴**: 분석기를 오라클로 활용하는 생성 전략의 일반화
- **WASM 배포 제약 하 양방향 형태론**: 브라우저 환경에서의 생성 문제 해결

### 9.3 독립 논문 가능성

본 연구 자체가 독립 논문으로 발전할 수 있는 기여:

- **"Bidirectional Morphology via Galois Connections: A Comonadic Approach"**
  - 분석-생성 대칭을 범주론적으로 정밀하게 특성화한 최초의 연구 (핀란드어 사례)
  - Writer Comonad의 역 coKleisli 화살표 존재 조건의 분류
  - Analyze-then-Invert 아키텍처의 정당화와 최적성 분석

---

## 부록 A: 코드 레벨 비대칭 증거

### A.1 분석에서의 불규칙 어간 처리

FST(mor.vfst)는 `mies` -> `mieh` 어간 교체를 내부 전이로 인코딩한다:

```
입력: m i e s       (표면형 "mies")
FST:  m i e [h/s]   (내부적으로 어간 변이형 선택)
출력: [Xp][Ln]mies[Sn]  (분석: 명사 "mies", 격 = nominative)

입력: m i e h e n   (표면형 "miehen")
FST:  m i e h e n
출력: [Xp][Ln]mies[Sg]  (분석: 명사 "mies", 격 = genitive)
```

### A.2 생성에서의 불규칙 어간 처리 (부재)

현재 `generator.rs`의 `apply_case()` (`line 559`):

```rust
fn apply_case(baseform: &str, case_info: &CaseInfo) -> String {
    let graded_stem = gradate(baseform, case_info.grade);  // "mies" 에 gradation 적용
    let intermediate = format!("{}{}", graded_stem, case_info.suffix);
    // ...
}
```

`gradate("mies", Grade::Weak)`는 자음교체 패턴에 `mies`가 매칭되지 않으므로 `"mies"`를 그대로 반환한다. 결과: `"mies" + "n" = "miesn"` (오류, 올바른 형태는 `"miehen"`).

### A.3 Comonad 파이프라인의 양방향 활용 현황

자음교체 coKleisli 화살표가 생성에서 활용되는 예 (`generator.rs:566`):

```rust
let graded_stem = gradate(baseform, case_info.grade);
// gradate() -> writer::gradate_pure() -> gradation_pipeline_pure()
// -> WriterZipper.extend(gradation_writer)
```

이 호출은 **정방향**(형태론적 표현 -> 표면형)으로 작동하며, 이는 생성 방향과 일치한다. 즉 Comonad 파이프라인은 이미 생성에 올바르게 활용되고 있으며, 비대칭의 원인은 Comonad 파이프라인이 아니라 **어간 선택(stem selection) 단계**에 있다.

---

## 부록 B: 하위 연구 문서와의 관계

본 문서는 다음 3개 연구의 상위 아키텍처 문서로서:

1. **불규칙 동사 연구**: Phase D1(예외 테이블)의 동사 부분이 직접 해당. 불규칙 동사 어간 테이블의 설계와 범위 결정.

2. **자음교체 연구**: Section 3.3-3.5의 수학적 분석(역 coKleisli 화살표, 역반군 구조)이 직접 해당. 자음교체의 양방향성 조건 분류.

3. **suggest 알고리즘 연구**: Phase D3(analyze-then-invert)의 후보 열거 전략이 suggest 알고리즘의 후보 생성과 구조적으로 동일. 두 연구의 후보 열거 로직을 공유할 수 있음.

---

## 부록 C: 구현 우선순위 매트릭스

| 구현 항목 | 영향도 | 난이도 | 우선순위 | 목표 버전 |
|----------|:---:|:---:|:---:|:---:|
| 불규칙 명사 예외 테이블 (50-100항) | 높음 | 낮음 | **1** | v0.4.x |
| `generate_form()`에 FST 검증 추가 | 높음 | 낮음 | **2** | v0.4.x |
| 불규칙 동사 예외 테이블 | 높음 | 낮음 | **3** | v0.4.x |
| `generate_paradigm()`에 FST 검증 추가 | 중간 | 낮음 | **4** | v0.5.x |
| Analyze-then-Invert 폴백 | 중간 | 중간 | **5** | v0.5.x |
| 생성 결과 LRU 캐시 | 낮음 | 낮음 | **6** | v0.5.x |
| WASM API `generate_form_verified()` | 낮음 | 낮음 | **7** | v0.6.x |
