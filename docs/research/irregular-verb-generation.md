---
title: Irregular Verb Generation Deep Analysis
created: 2026-03-05
commit: 37462bf
status: active
relates-to:
  - generation-consonant-gradation.md
  - verb-generation-verification.md
  - analysis-generation-symmetry.md
---

# 불규칙 동사 생성 문제 심층 분석

## 1. Executive Summary (요약)

MCE의 동사 생성기(`mce-fi/src/generator.rs`)는 **순수 접미사 규칙 기반**으로 구현되어 있다. 동사를 4가지 유형(Type 1-4)으로 분류한 후, 어간 추출 → 자음 교체(gradation) → 시제/인칭 접미사 → 모음 조화의 파이프라인을 적용한다. 이 접근법은 **규칙적 동사**(puhua, lukea, antaa 등)에는 정확하게 작동하지만, **불규칙 동사**에서 체계적으로 실패한다.

### 핵심 문제 사례

| 동사 | 현재 출력 (3sg) | 정답 | 오류 유형 |
|------|-----------------|------|-----------|
| olla | olee | on | 보충형(suppletive) |
| syoda | syoo | syo | 축약형(contracted) |
| juosta | juosee | juoksee | 자음 삽입(consonant insertion) |
| syoda (past 1sg) | syoin | soin | 모음 변이(vowel mutation: y→o) |
| tehda | tekee | tekee | 자음 변이(d→k) -- 실제로 이건 맞을 수도 있음 |
| nahdä (3sg) | nahee/nakee? | nakee | 자음 변이(hd→k) |

### 결론 요약

1. **FST 역방향 조회는 기술적으로 불가능**하다. Voikko의 VFST 포맷은 surface→analysis 방향 전용이며, 역방향(baseform→surface) 탐색 API가 존재하지 않는다.
2. **예외 테이블(Exception Table)이 최적의 해결책**이다. ~30개의 불규칙 동사를 하드코딩하면 커버리지 99%+ 달성 가능하며, WASM 크기 증가는 < 2KB이다.
3. **Writer Comonad 프레임워크는 불규칙 동사 자체를 모델링하기에 부적합**하다. Comonad는 문자 단위 위치 변환(gradation, harmony)에 최적화되어 있고, 어간 수준의 비선형 변환(olla→on)은 이 추상화 레벨이 아니다.
4. **하이브리드 접근(예외 테이블 + 규칙 기반 fallback)**이 가장 현실적이다. 구현 난이도 낮음, WASM 크기 영향 극소, 정확도 극대화.

---

## 2. 현재 구현 분석 (코드 레벨)

### 2.1 파일 위치 및 구조

**`/Users/yongseok/oss/finnishNLP/mce/crates/mce-fi/src/generator.rs`** (약 1,700줄)

동사 생성의 핵심 구조:

```
MorphGenerator::generate_verb(infinitive, tense, person, number, polarity)
  └── classify_verb(infinitive) → VerbType (Type1/2/3/4)
  └── conjugate(infinitive, verb_type, tense, person, number, polarity)
        ├── conjugate_affirmative → conjugate_present/past/conditional_affirmative
        └── conjugate_negative
```

### 2.2 동사 유형 분류 (`classify_verb`)

```rust
// generator.rs:837-889
fn classify_verb(infinitive: &str) -> Option<VerbType> {
    // Type 3: -lla/-llä, -nna/-nnä, -rra/-rrä, -sta/-stä
    // Type 2: vowel + da/dä (syödä, juoda)
    // Type 4: vowel + ta/tä (haluta, pelätä)
    // Type 1: vowel + a/ä (puhua, lukea) -- default
}
```

분류는 **순전히 infinitive 형태의 접미사 패턴**에 의존한다. 이 분류 자체는 대체로 정확하지만, 같은 유형 안에서도 불규칙 어간 변화가 발생하는 경우를 처리하지 못한다.

### 2.3 어간 추출 (`extract_stem`)

```rust
// generator.rs:895-937
fn extract_stem(infinitive: &str, verb_type: VerbType) -> String {
    match verb_type {
        VerbType::Type1 => chars[..len-1],      // puhua → puhu (drop 'a')
        VerbType::Type2 => chars[..len-2],      // syödä → syö (drop 'dä')
        VerbType::Type3 => chars[..len-2] + "e", // tulla → tule
        VerbType::Type4 => chars[..len-2] + inf_vowel, // haluta → halua
    }
}
```

**문제의 근원**: 이 함수는 infinitive의 문자열을 기계적으로 자른다. 불규칙 동사의 어간 변이(olla→ol→on, juosta→juos→juoks)를 전혀 고려하지 않는다.

### 2.4 3인칭 단수 현재 생성 (핵심 버그 지점)

```rust
// generator.rs:1117-1124
(VerbPerson::Third, VerbNumber::Singular) => {
    // 3sg: lengthen the stem-final vowel.
    if let Some(v) = last_vowel(&graded) {
        format!("{}{}", graded, v)  // stem + 마지막 모음 반복
    } else {
        graded.to_string()
    }
}
```

이 로직이 `olla` → `ole` (Type3 stem) → `olee`를 생성하는 원인이다. 정답 `on`은 완전히 다른 형태(suppletive form)이므로, 어떤 규칙적 접미사 처리로도 도달할 수 없다.

### 2.5 과거 시제 어간 추출 (`extract_past_stem`)

```rust
// generator.rs:983-1028
fn extract_past_stem(infinitive: &str, verb_type: VerbType) -> String {
    match verb_type {
        VerbType::Type2 => chars[..len-2],  // syödä → syö
        // Past: syö + i → syöi (정답: söi -- 모음 y→ö 변이 누락)
    }
}
```

`syödä`의 과거형에서 `y→ö` 변이가 발생하지만, 현재 코드는 이를 처리하지 않는다. `syöin`을 생성하지만 정답은 `söin`이다.

### 2.6 WASM 호출 경로

```rust
// mce-wasm/src/lib.rs:822-848
pub fn generate_verb_form(&self, baseform, tense, person, polarity) -> String {
    let generator = MorphGenerator::new();
    generator.generate_verb(baseform, tense, person, number, polarity)
        .unwrap_or_else(|| baseform.to_string())
}
```

WASM 레이어는 단순한 pass-through이다. FST 엔진(`self.analyzer`)을 활용하지 않고, 순수하게 규칙 기반 `MorphGenerator`만 사용한다.

---

## 3. 핀란드어 불규칙 동사 유형학

핀란드어 동사의 불규칙성은 크게 4가지 범주로 분류된다.

### 3.1 보충형 (Suppletive Forms)

완전히 다른 어간을 사용하는 경우. 규칙으로 예측 불가능.

| 동사 (inf) | 형태 | 규칙 적용 결과 | 정답 | 메모 |
|-----------|------|---------------|------|------|
| olla | 3sg present | olee | **on** | 완전 보충형, 유일한 사례 |
| olla | 3pl present | olevat | **ovat** | 보충형 |
| olla | 1sg present | olen | **olen** | 이건 규칙적! |
| olla | neg present | ole | **ole** | 이것도 규칙적 |

`olla`는 핀란드어에서 유일한 진정한 보충형 동사이다. 3sg `on`과 3pl `ovat`만이 완전 불규칙이고, 나머지 형태(olen, olet, olemme, olette)는 Type 3 규칙을 따른다. 단, 부정형 3sg `ei ole`도 규칙적이다.

### 3.2 축약형 (Contracted Stems)

Type 2 동사 중 일부는 3sg에서 모음이 길어지는 대신 **축약**된다.

| 동사 | 어간 | 3sg 규칙 결과 | 3sg 정답 | 패턴 |
|------|------|-------------|---------|------|
| syoda | syo | syoo | **syo** | 장모음 축약: VV → V |
| juoda | juo | juoo | **juo** | 장모음 축약 |
| vieda | vie | viee | **vie** | 장모음 축약 |
| tuoda | tuo | tuoo | **tuo** | 장모음 축약 |
| saada | saa | saaa | **saa** | 3모음 → 2모음 |
| myoda | myo | myoo | **myo** | 장모음 축약 |

**패턴**: Type 2 동사의 3sg는 어간 그대로이다(모음 길어짐 없음). 이것은 규칙적인 예외 -- Type 2 전체에 적용되는 규칙이다. 현재 코드가 Type 2를 특별 처리하지 않는 버그.

### 3.3 자음 삽입/변이 (Consonant Alternation in Stems)

Type 3의 일부 동사에서 어간에 예상치 못한 자음이 나타난다.

| 동사 | 어간 (현재) | 규칙 결과 | 정답 | 변이 설명 |
|------|-----------|---------|------|----------|
| juosta | jouse | jousee | **juoksee** | s→ks (원래 stem juoks-, inf에서 s로 축약) |
| nousta | nouse | nousee | **nousee** | 이건 맞음! |
| tehda | teke(?) | -- | **tekee** | hd→k (consonant alternation) |
| nahda | nahe(?) | nahee(?) | **nakee** | hd→k |
| piesta | piese | piesee | **pieksee** | s→ks |

이 범주의 핵심은 infinitive에서 어간을 기계적으로 잘라서는 올바른 현재 어간을 얻을 수 없다는 것이다. `juosta`의 실제 어간은 `juoks-`이지만, infinitive에서는 `s`로 축약되어 나타난다.

`tehda`, `nahda`는 현재 코드에서 Type 2로 분류된다 (`hd` + `ä`). `teh`의 어간에서 `teke-`를 도출하려면 `h→k` 자음 변이를 알아야 한다.

### 3.4 과거 시제 모음 변이 (Past Tense Vowel Mutations)

과거 시제에서 어간 모음이 변하는 경우.

| 동사 | 현재 어간 | 과거 어간 | 규칙 결과 | 정답 | 변이 |
|------|---------|---------|---------|------|------|
| syoda | syo | syo+i=syoi | syoin | **soin** | yo → o (front→back) |
| juoda | juo | juo+i=juoi | juoin | **join** | uo → o |
| vieda | vie | vie+i=viei | viein | **vein** | ie → e |
| tuoda | tuo | tuo+i=tuoi | tuoin | **toin** | uo → o |
| saada | saa | saa+i=saai | saain | **sain** | aa → a |
| myoda | myo | myo+i=myoi | myoin | **moin** | yo → o |
| kayda | kay | kay+i=kayi | kayin | **kavin** | y→v (특수) |

이들은 Type 2 동사의 과거 시제에서 **이중모음이 단순화**되는 규칙적 패턴이다. `VV + i → Vi`가 아니라 `VV + i → V₂i`로 축약된다. 이것은 규칙으로 잡을 수 있는 준규칙적 현상이다.

### 3.5 불규칙 동사 완전 목록 (실용적 범위)

핀란드어에서 실질적으로 불규칙한 동사는 약 **25-30개**이다:

**보충형**: olla (유일)

**Type 2 축약형 (3sg)**: syoda, juoda, vieda, tuoda, saada, myoda, lyoda, kayda, voida, puida

**자음 삽입형**: juosta (juoks-), piesta (pieks-), tehda (tek-), nahda (nak-)

**과거 모음 변이**: 위의 Type 2 동사들 + kayda(특수)

대부분은 Type 2에 집중되어 있다. Type 2 동사의 수가 ~15개로 한정적이며, 이들 모두를 예외 테이블로 관리하는 것이 현실적이다.

---

## 4. FST 역방향 조회 가능성 분석

### 4.1 현재 FST 구조

MCE의 FST 엔진(`mce-fst`)은 Voikko의 VFST 바이너리 포맷을 사용한다.

**`/Users/yongseok/oss/finnishNLP/mce/crates/mce-fst/src/lib.rs`**:
```rust
pub trait Transducer {
    type Config;
    fn prepare(&self, config: &mut Self::Config, input: &[char]) -> bool;
    fn next(&self, config: &mut Self::Config, output: &mut String) -> bool;
}
```

이 인터페이스는 **단방향**이다:
- `prepare(input)`: 입력 문자열의 각 char를 심볼 인덱스로 변환
- `next(output)`: FST를 순회하여 다음 출력 문자열을 생성

입력은 `sym_in`으로, 출력은 `sym_out`으로 매핑된다.

### 4.2 VFST 전이 구조

**`/Users/yongseok/oss/finnishNLP/mce/crates/mce-fst/src/transition.rs`**:
```rust
pub struct Transition {
    pub sym_in: u16,   // 입력 심볼 인덱스
    pub sym_out: u16,  // 출력 심볼 인덱스
    pub trans_info: u32, // target_state (24bit) + more_transitions (8bit)
}
```

FST의 전이는 `(sym_in, sym_out, target_state)` 트리플이다. 순회 알고리즘은 `sym_in`을 기준으로 매칭한다.

### 4.3 역방향 조회의 기술적 제약

**역방향 조회 = baseform(output tag)을 입력으로 넣고, surface form(input)을 출력으로 얻는 것.**

이것이 불가능한 이유:

1. **인덱싱 방향**: 전이 테이블은 `sym_in` 기준으로 정렬/인덱싱되어 있다. `sym_out` 기준 역방향 탐색은 전이 테이블 전체를 선형 스캔해야 한다.

2. **Flag Diacritics**: Voikko FST는 flag diacritic 연산(`@P.CASE.NOM@`, `@C.NUM@` 등)을 사용한다. 이 플래그들은 순방향 순회 시 상태 스택에서 체크/설정되며, 역방향으로는 의미가 역전된다(P-flag의 의미가 역으로 바뀜).

3. **다대다 매핑**: 하나의 baseform에서 수십~수백 개의 surface form이 생성될 수 있다(모든 격변화, 시제, 인칭 조합). 역방향 순회는 이 모든 경로를 탐색해야 하므로 조합 폭발이 발생한다.

4. **심볼 테이블의 비대칭성**: `char_to_symbol` 매핑은 단일 문자 → 심볼 인덱스이다. 출력 심볼에는 `[Ln]`, `[Lv]` 같은 multi-char 태그가 포함되어 있어 역방향 입력으로 사용할 수 없다.

5. **API 부재**: `UnweightedTransducer`에는 역방향 순회 메서드가 없다. `prepare()`는 입력 심볼 스택을 설정하고, `next()`는 `sym_in` 매칭을 기반으로 탐색한다. 이 구조를 역방향으로 사용하려면 FST 엔진을 근본적으로 재설계해야 한다.

### 4.4 이론적 대안: 역방향 FST 구축

HFST 등의 도구로 **별도의 역방향 FST**를 컴파일하는 것은 이론적으로 가능하다:
- Omorfi의 `hfst-invert` 명령으로 FST를 뒤집으면 generation FST가 된다
- 이 FST를 VFST 포맷으로 변환하여 MCE에 로드

그러나 이 접근법의 문제:
- **크기**: 역방향 FST는 원본과 비슷한 크기(~3.8MB)이므로 WASM 배포 크기가 ~7MB로 두 배
- **복잡도**: 두 개의 FST를 로드/관리해야 함
- **Voikko FST는 Omorfi FST가 아님**: MCE가 사용하는 `mor.vfst`는 Voikko 프로젝트 자체의 어휘 데이터에서 빌드된 것으로, Omorfi와는 별개의 어휘 DB이다. 역방향 FST를 빌드하려면 Voikko의 빌드 파이프라인을 수정해야 한다
- **30개 동사 문제에 3.8MB 솔루션은 과잉**

### 4.5 결론

FST 역방향 조회는 MCE의 현재 아키텍처에서 **실현 불가능**하며, 실현하더라도 **비용 대비 효과가 극히 낮다**. 불규칙 동사는 수가 적으므로 예외 테이블이 최적이다.

---

## 5. 수학적 모델링 (Comonad 프레임워크 내)

### 5.1 Writer Comonad 복습

MCE의 morphophonological 파이프라인은 `WriterZipper<DeletionSet, char>` 위에서 동작한다.

**`/Users/yongseok/oss/finnishNLP/mce/crates/mce-comonad/src/writer.rs`**:
```rust
pub struct WriterZipper<W: Monoid, A: Clone> {
    pub log: W,          // DeletionSet (삭제 위치 추적)
    pub zipper: Zipper<A>, // 문자 시퀀스 + 초점
}

// coKleisli arrow 타입:
// f: &WriterZipper<W, A> -> (W, B)
```

핵심 연산:
- `extract`: 현재 초점 문자 반환
- `extend(f)`: 모든 위치에서 f를 적용, 결과 합성
- `materialize()`: DeletionSet에 마킹된 위치 제거 후 최종 문자열 생성

### 5.2 coKleisli Arrow가 모델링하는 것

현재 구현된 coKleisli arrow들:

```rust
// gradation_writer: 자음 교체 (pp→p, k→∅, p→v 등)
// harmony_writer: 모음 조화 (A→a/ä, O→o/ö)
// possessive_writer: 소유접미사 모음 복사 (V→선행 모음)
```

이 arrow들은 모두 **문자 단위 위치 변환**이다:
- 입력: 문자 시퀀스의 한 위치 (주변 컨텍스트 참조 가능)
- 출력: 해당 위치의 변환된 문자 + 삭제 마킹

### 5.3 불규칙 동사가 coKleisli Arrow로 표현 불가능한 이유

불규칙 동사 생성의 핵심 변환은 **문자 단위가 아닌 어간 단위**이다:

1. **보충형 (olla → on)**: 이것은 `o-l-e` → `o-n`으로의 변환이 아니다. 완전히 다른 어간이다. Zipper의 어떤 위치에서든 `l→n`으로 바꾸고 `e`를 삭제해도 `on`이 나오지 않는다 (실제로 `one`이 된다). 그리고 이 변환은 `olla`에서만 발생하므로 coKleisli arrow의 **로컬 패턴 매칭으로 표현할 수 없다**.

2. **자음 삽입 (juosta 3sg → juoksee)**: `jouse` → `juokse`로의 변환은 단순한 문자 대체가 아니라 **문자 삽입**이다. WriterZipper의 DeletionSet은 문자 **삭제**만 표현할 수 있고, 삽입은 표현할 수 없다.

3. **과거 모음 축약 (syö → sö)**: `y→ö` 변이는 특정 동사 어간에서만 발생하며, 문자 수준에서는 어떤 컨텍스트 패턴으로도 이 변환의 조건을 정의할 수 없다.

### 5.4 수학적으로는 가능하지만 실용적으로 무의미

이론적으로, Comonad 프레임워크를 다음과 같이 확장할 수 있다:

**확장 1: InsertionMonoid**
```
DeletionSet → TransformationLog = DeletionSet + InsertionMap
```
`InsertionMap<usize, Vec<char>>`로 특정 위치에 문자를 삽입하는 것을 모델링. 그러나 이것은 monoid 구조를 유지하면서도 `materialize()`의 복잡도를 높인다.

**확장 2: ReplacementMonoid**
```
W = Map<Range<usize>, Vec<char>>  (위치 범위 → 대체 문자열)
```
이것은 결국 "예외 테이블"과 동일하다. Comonad으로 감싸는 것은 불필요한 추상화 오버헤드만 추가한다.

**결론**: 불규칙 동사는 coKleisli arrow 합성의 범위 밖에 있다. 이것은 프레임워크의 한계가 아니라, **적용 대상이 다른 것**이다. Comonad는 음운론적 규칙(gradation, harmony)에 최적이고, 어휘적 예외(irregular verbs)는 lookup table의 영역이다.

### 5.5 범주론적 관점: 생성 파이프라인의 올바른 모델

동사 생성 파이프라인을 범주론적으로 정확하게 모델링하면:

```
generate: Verb × Features → String
generate = lookup ⊕ (stem ∘ gradation ∘ suffix ∘ harmony)
```

여기서 `⊕`는 **coproduct** (합집합)이다:
- 첫 번째 인자 `lookup`은 예외 테이블 조회 (불규칙 동사)
- 두 번째 인자는 규칙 기반 파이프라인 (규칙 동사)

이것은 코드에서 단순히 `match` 또는 `HashMap::get().unwrap_or_else(|| ...)`로 구현된다. 수학적으로 깔끔하지만, 실제 코드는 훨씬 더 간단하다.

---

## 6. 해결 전략 비교 매트릭스

### 6.1 전략 (a): 예외 테이블 하드코딩

**구현**:
```rust
// generator.rs에 추가
const IRREGULAR_VERBS: &[(&str, &[IrregularForm])] = &[
    ("olla", &[
        IrregularForm { tense: Present, person: Third, number: Sg, aff: "on", neg: "ole" },
        IrregularForm { tense: Present, person: Third, number: Pl, aff: "ovat", neg: "ole" },
        // ... 기타 불규칙 형태
    ]),
    ("syödä", &[
        IrregularForm { tense: Present, person: Third, number: Sg, aff: "syö", neg: "syö" },
        // ...
    ]),
    // ...
];
```

| 기준 | 평가 |
|------|------|
| 정확도 | **최고**. 수작업 검증된 정답 직접 매핑 |
| 구현 난이도 | **매우 낮음**. ~200-300줄 추가 |
| WASM 크기 | **+1-2KB**. 30개 동사 × ~10 형태 × ~10바이트 |
| 유지보수 | **낮음**. 핀란드어 불규칙 동사 목록은 변하지 않음 |
| 확장성 | 새 불규칙 동사를 발견하면 테이블에 추가 |
| 속도 | **O(1) HashMap lookup**. 규칙 파이프라인보다 빠름 |
| 범주론적 우아함 | 낮음. 순수 데이터 테이블 |

### 6.2 전략 (b): FST 역방향 조회

| 기준 | 평가 |
|------|------|
| 정확도 | 높음 (FST가 커버하는 범위 내) |
| 구현 난이도 | **극히 높음**. FST 엔진 재설계 또는 역방향 FST 빌드 필요 |
| WASM 크기 | **+3-4MB**. 역방향 FST 전체 로드 |
| 유지보수 | 높음. FST 빌드 파이프라인 관리 |
| 확장성 | 좋음 (FST가 모든 형태를 커버) |
| 속도 | 느림. FST 순회 + 결과 필터링 |
| 범주론적 우아함 | 중간. Transducer의 자연스러운 확장 |

### 6.3 전략 (c): Comonad 파이프라인 확장

| 기준 | 평가 |
|------|------|
| 정확도 | 제한적. 보충형은 처리 불가 |
| 구현 난이도 | 높음. 새로운 Monoid + arrow 설계 |
| WASM 크기 | +5-10KB (새 규칙 코드) |
| 유지보수 | 높음. 복잡한 규칙 체계 |
| 확장성 | 낮음. 새 불규칙 패턴마다 새 arrow 필요 |
| 속도 | 보통. 추가 extend 호출 |
| 범주론적 우아함 | 높음. 프레임워크 내 통합 |

### 6.4 전략 (d): 하이브리드 (예외 테이블 + Type 2 규칙 수정 + 규칙 fallback)

**구현**: 3단계 전략

1. **불규칙 테이블 조회** (olla, juosta, tehda, nahda 등 ~10개 진정 불규칙)
2. **Type 2 규칙 수정** (3sg 모음 비연장, 과거 이중모음 축약 -- 준규칙적)
3. **기존 규칙 파이프라인 fallback** (나머지 규칙 동사)

| 기준 | 평가 |
|------|------|
| 정확도 | **최고**. 진정 불규칙 + 준규칙적 패턴 모두 커버 |
| 구현 난이도 | **낮음**. 예외 테이블 + classify_verb 수정 |
| WASM 크기 | **+1-2KB** |
| 유지보수 | **최저**. 테이블은 정적, 규칙은 최소 수정 |
| 확장성 | 최고. 새 패턴 = 테이블 또는 규칙 분기 추가 |
| 속도 | 최고. O(1) lookup + 기존 파이프라인 |
| 범주론적 우아함 | 중간. coproduct 모델 (lookup + rules) |

### 6.5 비교 요약

| 전략 | 정확도 | 난이도 | WASM | 추천도 |
|------|--------|--------|------|--------|
| (a) 예외 테이블 | A+ | A | +1KB | B+ |
| (b) FST 역방향 | A | D | +4MB | D |
| (c) Comonad 확장 | C | C | +8KB | D |
| **(d) 하이브리드** | **A+** | **A** | **+2KB** | **A+** |

---

## 7. 권장 접근법 + 구현 로드맵

### 7.1 권장: 전략 (d) 하이브리드

#### Phase 1: Type 2 동사 3sg 수정 (30분)

**문제**: Type 2 동사의 3sg는 모음을 길게 하지 않는다 (syö → syö, 아닌 syöö).

**수정 위치**: `conjugate_present_affirmative()` (generator.rs:1117-1124)

```rust
// 수정 전:
(VerbPerson::Third, VerbNumber::Singular) => {
    if let Some(v) = last_vowel(&graded) {
        format!("{}{}", graded, v)  // syö → syöö (버그!)
    }
}

// 수정 후:
(VerbPerson::Third, VerbNumber::Singular) => {
    if verb_type == VerbType::Type2 {
        graded.to_string()  // syö → syö (정답)
    } else if let Some(v) = last_vowel(&graded) {
        format!("{}{}", graded, v)
    }
}
```

이 수정만으로 Type 2 동사의 3sg 오류가 모두 해결된다 (syoda, juoda, vieda, tuoda, saada, lyoda, myoda).

**주의**: 이 수정은 `conjugate_present_affirmative`에 `verb_type` 파라미터를 추가해야 한다. 호출 체인(conjugate_affirmative → conjugate_present_affirmative)도 수정 필요.

#### Phase 2: Type 2 과거 시제 이중모음 축약 (1시간)

**문제**: `syödä` past 1sg = `söin` (not `syöin`). 이중모음 `yö`가 `ö`로 축약.

**수정 위치**: `extract_past_stem()` (generator.rs:1003-1007)

```rust
// Type 2 과거 어간 축약 규칙 추가
VerbType::Type2 => {
    let stem: String = chars[..chars.len() - 2].iter().collect();
    // 이중모음 축약: yö→ö, uo→o, ie→e
    contract_diphthong(&stem)
}
```

`contract_diphthong` 함수:
```rust
fn contract_diphthong(stem: &str) -> String {
    let chars: Vec<char> = stem.chars().collect();
    if chars.len() >= 2 {
        let (a, b) = (chars[chars.len()-2], chars[chars.len()-1]);
        match (a, b) {
            ('u', 'o') | ('y', 'ö') | ('i', 'e') => {
                // 첫 번째 모음 삭제: uo→o, yö→ö, ie→e
                let mut result: String = chars[..chars.len()-2].iter().collect();
                result.push(b);
                return result;
            }
            _ => {}
        }
    }
    stem.to_string()
}
```

#### Phase 3: 보충형 예외 테이블 (1시간)

**대상**: olla (유일한 진정 보충형) + juosta, tehda, nahda (자음 삽입형)

```rust
use std::collections::HashMap;

struct IrregularEntry {
    present_stems: Option<PresentStems>,
    past_stem: Option<&'static str>,
    // 완전 보충형 (olla 3sg/3pl만 해당)
    overrides: &'static [(VerbTense, VerbPerson, VerbNumber, VerbPolarity, &'static str)],
}

struct PresentStems {
    strong: &'static str,  // 3sg용
    weak: &'static str,    // 1sg/2sg 등
}

lazy_static! {
    static ref IRREGULAR_VERBS: HashMap<&'static str, IrregularEntry> = {
        let mut m = HashMap::new();
        m.insert("olla", IrregularEntry {
            present_stems: Some(PresentStems { strong: "ole", weak: "ole" }),
            past_stem: Some("ol"),
            overrides: &[
                (Present, Third, Singular, Affirmative, "on"),
                (Present, Third, Plural, Affirmative, "ovat"),
            ],
        });
        m.insert("juosta", IrregularEntry {
            present_stems: Some(PresentStems { strong: "juokse", weak: "juokse" }),
            past_stem: Some("juoks"),
            overrides: &[],
        });
        m.insert("tehdä", IrregularEntry {
            present_stems: Some(PresentStems { strong: "teke", weak: "teke" }),
            past_stem: Some("tek"),
            overrides: &[],
        });
        m.insert("nähdä", IrregularEntry {
            present_stems: Some(PresentStems { strong: "näke", weak: "näke" }),
            past_stem: Some("näk"),
            overrides: &[],
        });
        m.insert("piestä", IrregularEntry {
            present_stems: Some(PresentStems { strong: "piekse", weak: "piekse" }),
            past_stem: Some("pieks"),
            overrides: &[],
        });
        m
    };
}
```

**통합 로직**:
```rust
fn conjugate(infinitive, verb_type, tense, person, number, polarity) -> String {
    // Step 1: 완전 보충형 조회
    if let Some(entry) = IRREGULAR_VERBS.get(infinitive) {
        for &(t, p, n, pol, form) in entry.overrides {
            if t == tense && p == person && n == number && pol == polarity {
                return form.to_string();
            }
        }
        // Step 2: 불규칙 어간으로 규칙 파이프라인 실행
        // (present_stems/past_stem 사용)
    }
    // Step 3: 기존 규칙 파이프라인
    // ...
}
```

#### Phase 4: 테스트 추가 (30분)

```rust
#[test]
fn olla_3sg_on() {
    let g = MorphGenerator::new();
    let form = g.generate_verb("olla", Present, Third, Sg, Aff);
    assert_eq!(form, Some("on".to_string()));
}

#[test]
fn syoda_3sg_syo() {
    let g = MorphGenerator::new();
    let form = g.generate_verb("syödä", Present, Third, Sg, Aff);
    assert_eq!(form, Some("syö".to_string()));
}

#[test]
fn juosta_3sg_juoksee() {
    let g = MorphGenerator::new();
    let form = g.generate_verb("juosta", Present, Third, Sg, Aff);
    assert_eq!(form, Some("juoksee".to_string()));
}

#[test]
fn syoda_past_1sg_soin() {
    let g = MorphGenerator::new();
    let form = g.generate_verb("syödä", Past, First, Sg, Aff);
    assert_eq!(form, Some("söin".to_string()));
}
```

### 7.2 구현 타임라인

| Phase | 작업 | 예상 시간 | 테스트 추가 |
|-------|------|----------|------------|
| 1 | Type 2 3sg 수정 | 30분 | +6 tests |
| 2 | Type 2 과거 이중모음 축약 | 1시간 | +12 tests |
| 3 | 보충형 예외 테이블 (olla + 자음삽입형 4개) | 1시간 | +20 tests |
| 4 | 통합 테스트 + WASM 검증 | 30분 | +5 tests |
| **합계** | | **3시간** | **+43 tests** |

### 7.3 점진적 릴리스 전략

- **v0.3.1**: Phase 1 (Type 2 3sg 수정) -- 가장 빈번한 오류 수정
- **v0.4.0**: Phase 2-3 (과거 시제 + 예외 테이블) -- Kotus 통합과 함께
- 각 Phase는 독립적으로 배포 가능

---

## 8. WASM 영향 분석

### 8.1 현재 WASM 크기

- WASM 바이너리: **~395KB**
- 총 배포: **~9.2MB** (WASM + dictionary + model)
- CI 예산: 420KB

### 8.2 전략별 WASM 크기 영향

| 전략 | 추가 코드 크기 (소스) | WASM 바이너리 증가 예상 | 데이터 증가 |
|------|---------------------|----------------------|------------|
| (a) 예외 테이블 | ~200줄 Rust | +0.5-1KB | 없음 (코드 내 const) |
| (b) FST 역방향 | ~500줄 Rust | +2-3KB | +3.8MB (역방향 FST) |
| (c) Comonad 확장 | ~400줄 Rust | +2-4KB | 없음 |
| **(d) 하이브리드** | **~300줄 Rust** | **+1-2KB** | **없음** |

### 8.3 하이브리드 전략의 크기 상세 분석

1. **Phase 1** (Type 2 3sg 수정): 기존 코드 수정, 새 코드 ~10줄 → WASM 증가 거의 없음
2. **Phase 2** (이중모음 축약): `contract_diphthong()` ~30줄 → +0.2KB
3. **Phase 3** (예외 테이블): 5개 동사 × ~10 형태 × ~20바이트 = ~1KB 데이터 + lookup 코드 ~100줄 → +0.8KB

**총 WASM 증가 예상: ~1-2KB** (~395KB → ~397KB)

CI 예산 420KB 이내로 충분히 여유 있음.

### 8.4 런타임 성능 영향

- 예외 테이블 조회: O(1) HashMap lookup -- 기존 파이프라인(gradate + harmonize)보다 빠름
- 추가 함수 호출 오버헤드: 무시 가능 (조건 분기 1회)
- 메모리: 예외 테이블 상수 ~1KB -- 전체 WASM 메모리 대비 무시 가능

**결론: 하이브리드 전략은 WASM 크기와 성능에 거의 영향을 미치지 않으면서 정확도를 극적으로 개선한다.**

---

## 부록 A: 불규칙 동사 완전 형태 표 (구현 참고용)

### olla (to be) -- 보충형

| 형태 | 1sg | 2sg | 3sg | 1pl | 2pl | 3pl |
|------|-----|-----|-----|-----|-----|-----|
| present aff | olen | olet | **on** | olemme | olette | **ovat** |
| present neg | en ole | et ole | ei ole | emme ole | ette ole | eivat ole |
| past aff | olin | olit | oli | olimme | olitte | olivat |
| conditional | olisin | olisit | olisi | olisimme | olisitte | olisivat |

### syoda (to eat) -- Type 2 축약형

| 형태 | 1sg | 2sg | 3sg | 1pl | 2pl | 3pl |
|------|-----|-----|-----|-----|-----|-----|
| present aff | syon | syot | **syo** | syomme | syotte | syovat |
| past aff | **soin** | **soit** | **soi** | **soimme** | **soitte** | **soivat** |

### juosta (to run) -- 자음 삽입형

| 형태 | 1sg | 2sg | 3sg | 1pl | 2pl | 3pl |
|------|-----|-----|-----|-----|-----|-----|
| present aff | juoksen | juokset | **juoksee** | juoksemme | juoksette | juoksevat |
| past aff | juoksin | juoksit | juoksi | juoksimme | juoksitte | juoksivat |

### tehda (to do) -- 자음 변이형

| 형태 | 1sg | 2sg | 3sg | 1pl | 2pl | 3pl |
|------|-----|-----|-----|-----|-----|-----|
| present aff | teen | teet | **tekee** | teemme | teette | tekevat |
| past aff | tein | teit | teki | teimme | teitte | tekivat |

---

## 부록 B: 관련 파일 경로 참조

| 파일 | 역할 |
|------|------|
| `/Users/yongseok/oss/finnishNLP/mce/crates/mce-fi/src/generator.rs` | 현재 동사 생성기 (수정 대상) |
| `/Users/yongseok/oss/finnishNLP/mce/crates/mce-fi/src/lib.rs` | Finnish 모듈 진입점 |
| `/Users/yongseok/oss/finnishNLP/mce/crates/mce-fst/src/lib.rs` | FST 엔진 Transducer trait |
| `/Users/yongseok/oss/finnishNLP/mce/crates/mce-fst/src/unweighted.rs` | VFST 순회 알고리즘 |
| `/Users/yongseok/oss/finnishNLP/mce/crates/mce-fst/src/transition.rs` | 전이 구조체 |
| `/Users/yongseok/oss/finnishNLP/mce/crates/mce-fst/src/symbols.rs` | 심볼 테이블 파싱 |
| `/Users/yongseok/oss/finnishNLP/mce/crates/mce-fst/src/config.rs` | 순회 설정 스택 |
| `/Users/yongseok/oss/finnishNLP/mce/crates/mce-comonad/src/writer.rs` | Writer Comonad 구현 |
| `/Users/yongseok/oss/finnishNLP/mce/crates/mce-comonad/src/finnish.rs` | 핀란드어 morphophonology arrows |
| `/Users/yongseok/oss/finnishNLP/mce/crates/mce-wasm/src/lib.rs` | WASM API (generate_verb_form) |
