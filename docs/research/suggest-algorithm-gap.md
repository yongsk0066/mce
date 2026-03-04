---
title: suggest() Algorithm Gap Analysis
created: 2026-03-05
commit: 37462bf
status: active
relates-to: []
---

# suggest() 알고리즘 누락 후보 심층 분석

## 1. Executive Summary

MCE의 `suggest("tallö", 1)`이 `"talo"`를 반환하지 못하는 근본 원인은 **바이트 레벨 Levenshtein 거리 계산**이다.

현재 MCE의 전체 suggest 파이프라인 -- Succinct Trie의 `fuzzy_search()`, `SpellChecker`의 `suggest()`, 그리고 `edit_distance()` 유틸리티 함수 -- 은 모두 UTF-8 바이트 시퀀스에서 편집 거리를 계산한다. 핀란드어 특수 문자 `ö`와 `ä`는 UTF-8에서 2바이트(각각 `[0xC3, 0xB6]`, `[0xC3, 0xA4]`)로 인코딩되지만, ASCII 대응 문자 `o`와 `a`는 1바이트(`0x6F`, `0x61`)이다.

이 바이트 길이 비대칭으로 인해:
- `ö` -> `o` 치환은 **문자 레벨 1 edit**이지만 **바이트 레벨 2 edits** (2바이트 삭제 + 1바이트 삽입, 또는 동등한 경로)
- `tallö` -> `talo`는 문자 레벨 2 edits (삭제 `l` + 치환 `ö`->`o`)이지만 **바이트 레벨 3 edits**

`suggest("tallö", 1)` 호출 시 auto-escalation 로직이 `max_edits=2`까지 확장하지만, 이때 `talla`, `talli`, `tallo` (byte dist=2)가 먼저 발견되어 결과가 반환된다. `talo` (byte dist=3)는 검색되지 않는다.

**영향 범위**: `ä`/`ö`를 포함하거나 `ä`/`ö`가 올바른 교정인 모든 단어에서 edit distance가 과대 계산된다. 핀란드어에서 `ä`/`ö`는 극히 흔한 문자이므로, 이 문제는 suggest 품질에 체계적 영향을 미친다.

---

## 2. 현재 suggest 파이프라인 전체 흐름

### 2.1. WASM API 진입점

파일: `crates/mce-wasm/src/lib.rs`, L373-420

```
MceEngine::suggest(word, max_edits)
  |
  +-- 1. FinnishAnalyzer.analyze(word) -- 유효한 단어면 "[]" 반환
  |
  +-- 2. SpellChecker 파이프라인 (wordlist 로드 시)
  |     |
  |     +-- Auto-escalation: start=max_edits, limit=min(start+1, 3)
  |     |   for dist in start..=limit:
  |     |     results = checker.suggest(word, dist, 10)
  |     |     if !results.is_empty() -> return
  |     |
  |     +-- SpellChecker::suggest() [pipeline.rs L98-129]
  |           |
  |           +-- trie.fuzzy_search(word.as_bytes(), max_edits)
  |           +-- filter_map: is_candidate_valid() (morph validation)
  |           +-- user_dict 후보 추가
  |           +-- truncate(max_suggestions)
  |
  +-- 3. Legacy fallback: raw trie fuzzy search (morph validation 없음)
  |
  +-- 4. Final fallback: suggest_with_context(word, "", max_edits)
```

### 2.2. SpellChecker suggest 내부

파일: `crates/mce-speller/src/pipeline.rs`, L98-129

SpellChecker의 suggest는 3단계:
1. **Trie fuzzy search**: `self.trie.fuzzy_search(word.as_bytes(), max_edits)` -- 바이트 레벨
2. **Morph validation filter**: 각 후보에 대해 `is_candidate_valid()` 호출
3. **User dictionary scan**: 사용자 사전의 모든 단어에 대해 `edit_distance()` 계산 (역시 바이트 레벨)

`suggest_ranked()` (L162-237)도 동일한 구조이며, 추가로 `rank_fn` 콜백과 frequency list 보너스를 적용한다.

### 2.3. Trie fuzzy_search 구현

파일: `crates/mce-core/src/trie/mod.rs`, L44-112

LOUDS 인코딩 Succinct Trie 위에서 DFS + 행별 DP(Dynamic Programming)로 Levenshtein automaton을 시뮬레이션한다.

핵심 코드 (L72-106):
```rust
while let Some((node, parent_row, key)) = stack.pop() {
    let ch = *key.last().unwrap();           // <- 바이트 레벨
    let mut current_row = Vec::with_capacity(query_len + 1);
    current_row.push(parent_row[0] + 1);

    for j in 1..=query_len {
        let cost = if query[j - 1] == ch { 0 } else { 1 };  // <- 바이트 비교
        let val = (parent_row[j] + 1)
            .min(current_row[j - 1] + 1)
            .min(parent_row[j - 1] + cost);
        current_row.push(val);
    }
    // ...prune, collect, recurse...
}
```

**모든 비교가 바이트(`u8`) 레벨이다.** Trie 자체도 바이트 키(`Vec<u8>`)를 저장하며, `TrieBuilder::insert()`는 `impl Into<Vec<u8>>`를 받는다. `load_wordlist()`에서 `word.as_bytes().to_vec()`으로 삽입한다.

### 2.4. edit_distance 유틸리티 함수

`pipeline.rs` L286-303, `spellcheck.rs` L437-456

두 파일 모두 동일한 바이트 레벨 Levenshtein:
```rust
fn edit_distance(a: &[u8], b: &[u8]) -> usize {
    // ...standard DP on byte slices...
    let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
}
```

### 2.5. FinnishSpellChecker (mce-fi)

파일: `crates/mce-fi/src/spellcheck.rs`, L217-231

`FinnishSpellChecker::suggest()`는 `SpellChecker::suggest_ranked()`를 호출하며, frequency list 기반 랭킹을 추가한다. 핵심 경로는 결국 동일한 바이트 레벨 fuzzy search를 사용한다.

---

## 3. "tallö" -> "talo" 실패 원인 분석

### 3.1. UTF-8 바이트 표현

```
tallö = [116, 97, 108, 108, 195, 182]   (6 bytes)
                              ^^^  ^^^
                              ö의 2바이트 UTF-8 인코딩

talo  = [116, 97, 108, 111]             (4 bytes)
                       ^^^
                       o의 1바이트 ASCII 인코딩
```

### 3.2. 바이트 레벨 DP 테이블

```
         _   t    a    l    o
         (116)(97)(108)(111)
  _      0    1    2    3    4
  t(116) 1    0    1    2    3
  a(97)  2    1    0    1    2
  l(108) 3    2    1    0    1
  l(108) 4    3    2    1    1
  ö1(195)5    4    3    2    2
  ö2(182)6    5    4    3    3   <-- 최종 바이트 거리 = 3
```

문자 레벨에서는 `tallö` -> `talo`가 2 edits (l 삭제 + ö->o 치환)이지만, 바이트 레벨에서는 3 edits이다.

### 3.3. 실행 추적

`suggest("tallö", 1)` 호출 시:

1. `FinnishAnalyzer.analyze("tallö")` -> 빈 결과 (유효하지 않은 단어)
2. SpellChecker 파이프라인 진입
3. Auto-escalation 루프:
   - `dist=1`: `trie.fuzzy_search("tallö".as_bytes(), 1)` -> 빈 결과 (최소 byte dist=2)
   - `dist=2`: `trie.fuzzy_search("tallö".as_bytes(), 2)` -> `[talla, talli, tallo]` (byte dist=2)
   - **결과가 비어있지 않으므로 루프 종료** -- `talo` (byte dist=3)는 검색되지 않음

### 3.4. 대조군: "koirra" -> "koira" 성공 이유

```
koirra = [107, 111, 105, 114, 114, 97]  (6 bytes, 순수 ASCII)
koira  = [107, 111, 105, 114, 97]       (5 bytes, 순수 ASCII)
```

순수 ASCII 단어이므로 바이트 레벨 = 문자 레벨. 편집 거리 = 1 (r 하나 삭제). `max_edits=1`에서 즉시 발견된다.

---

## 4. Edit Distance 계산 상세

### 4.1. 이론적 정의

MCE가 사용하는 것은 표준 **Levenshtein distance** (삽입, 삭제, 치환 각 비용 1). Damerau-Levenshtein (전위 포함)이나 Optimal String Alignment은 아니다.

세 가지 정의 비교:

| 알고리즘 | 연산 | tallö->talo (char) | tallö->talo (byte) |
|---------|------|-------------------|-------------------|
| Levenshtein | ins/del/sub | 2 | 3 |
| Damerau-Levenshtein | ins/del/sub/transposition | 2 | 3 |
| OSA | ins/del/sub/adjacent swap | 2 | 3 |

이 케이스에서는 전위(transposition)가 관련되지 않으므로 세 알고리즘의 결과가 동일하다.

### 4.2. 바이트 vs 코드포인트 비용 비대칭

UTF-8에서 한 문자의 바이트 수:

| 범위 | 바이트 수 | 예시 |
|------|----------|------|
| U+0000-U+007F | 1 | a, o, z (ASCII) |
| U+0080-U+07FF | 2 | ä (C3 A4), ö (C3 B6), å (C3 A5) |
| U+0800-U+FFFF | 3 | 한국어, 일본어, CJK |
| U+10000-U+10FFFF | 4 | 이모지, 고대 문자 |

핀란드어에서 중요한 비대칭:

| 변환 | 문자 레벨 비용 | 바이트 레벨 비용 | 비율 |
|------|-------------|-------------|------|
| ö -> o | 1 | 2 | 2x |
| ä -> a | 1 | 2 | 2x |
| å -> a | 1 | 2 | 2x |
| ö -> ä | 1 | 1 | 1x (동일 바이트 수) |
| a -> o | 1 | 1 | 1x (동일 바이트 수) |
| a -> e | 1 | 1 | 1x (동일 바이트 수) |

`ä`<->`a`, `ö`<->`o` 변환은 핀란드어 맞춤법 교정에서 **가장 빈번한 패턴** 중 하나이다 (모음 조화 오류, 방언 표기, 외래어 적응 등). 이 변환의 비용이 2배로 과대 계산되는 것은 치명적이다.

---

## 5. UTF-8 vs 코드포인트 레벨 처리 분석

### 5.1. 현재 구현의 일관성

MCE는 전체 파이프라인에서 **일관되게 바이트 레벨**을 사용한다:

| 컴포넌트 | 파일 | 레벨 |
|---------|------|------|
| TrieBuilder::insert() | trie/mod.rs | `Vec<u8>` |
| SuccinctTrie::contains() | trie/mod.rs L29 | `&[u8]` |
| SuccinctTrie::fuzzy_search() | trie/mod.rs L44 | `&[u8]` -> 바이트 DP |
| SpellChecker::suggest() | pipeline.rs L99 | `word.as_bytes()` |
| edit_distance() | pipeline.rs L286 | `&[u8]` |
| edit_distance_str() | spellcheck.rs L437 | `a.as_bytes()` |
| load_wordlist() | lib.rs L167 | `word.as_bytes().to_vec()` |

바이트 레벨 선택의 장점:
- LOUDS 인코딩과 자연스럽게 호환 (바이트 = edge label)
- ASCII 전용 언어에서는 정확
- 메모리 효율적 (char 변환 불필요)
- 구현이 간단

바이트 레벨 선택의 단점:
- 다바이트 문자의 edit distance 과대 계산
- 핀란드어 ä/ö 관련 교정 품질 저하
- 사용자 기대(문자 단위 거리)와 불일치

### 5.2. Trie 구조의 제약

현재 Succinct Trie는 바이트를 edge label로 사용한다. 이는 LOUDS 인코딩의 label 배열(`labels: Vec<u8>`)이 `u8` 타입이기 때문이다. 코드포인트 레벨 trie로 전환하려면:
- label 타입을 `u32` (또는 `char`)로 변경해야 함
- LOUDS 인코딩의 공간 효율성이 일부 감소 (label당 4바이트 vs 1바이트)
- 또는 바이트 trie를 유지하면서 fuzzy search만 코드포인트 인식으로 변경

---

## 6. 다른 잠재적 실패 패턴

### 6.1. 단일 ö/ä 치환 (가장 흔한 패턴)

사용자가 `max_edits=1`로 호출할 때, 순수 `ä->a` 또는 `ö->o` 치환(문자 레벨 1 edit)이 바이트 레벨 2 edits로 계산되어, `max_edits=1`에서 발견되지 않는다.

```
käsi -> kasi:  char_dist=1, byte_dist=2  (auto-escalation으로 찾을 수 있음)
talö -> talo:  char_dist=1, byte_dist=2  (auto-escalation으로 찾을 수 있음)
söi  -> soi:   char_dist=1, byte_dist=2  (auto-escalation으로 찾을 수 있음)
```

auto-escalation이 `max_edits+1`까지 확장하므로, 단일 ä/ö 치환은 대부분 발견된다. 그러나 결과 수가 많아지면 진짜 원하는 후보가 밀릴 수 있다.

### 6.2. ö/ä 치환 + 다른 1 edit (tallö -> talo 유형)

문자 레벨 2 edits이지만 바이트 레벨 3 edits. auto-escalation의 cap(min(start+1, 3))에 걸리거나, 바이트 거리 2인 다른 후보가 먼저 발견되면 누락된다.

```
tallö  -> talo:   char=2, byte=3  *** MISSED
kassä  -> kasa:   char=2, byte=3  *** MISSED (if kassa found at byte=2)
perrö  -> pero:   char=2, byte=3  *** MISSED
kunnä  -> kuna:   char=2, byte=3  *** MISSED
```

### 6.3. 이중 ö/ä (양쪽 모두 교정 필요)

문자 레벨 2 edits이지만 바이트 레벨 4 edits. `max_edits=3` cap에서도 발견 불가.

```
söpö  -> sopo:   char=2, byte=4  *** UNREACHABLE at any practical max_edits
häntä -> hanta:  char=2, byte=4  *** UNREACHABLE
pöytä -> poyta:  char=2, byte=4  *** UNREACHABLE
mökkö -> mokko:  char=2, byte=4  *** UNREACHABLE
```

### 6.4. 역방향: ASCII -> ö/ä (삽입 방향)

사용자가 ASCII로 입력하고 올바른 핀란드어가 ö/ä를 포함하는 경우:

```
talo -> talö:   char=1, byte=2  (auto-escalation으로 찾을 수 있음)
kasi -> käsi:   char=1, byte=2  (auto-escalation으로 찾을 수 있음)
sopo -> söpö:   char=2, byte=4  *** UNREACHABLE
```

### 6.5. 영향 범위 추정

핀란드어 텍스트에서 `ä`와 `ö`의 출현 빈도:
- `ä`: 핀란드어 텍스트의 ~3.3% (13번째로 흔한 문자)
- `ö`: 핀란드어 텍스트의 ~0.4% (23번째로 흔한 문자)

핀란드어 어휘에서 `ä` 또는 `ö`를 포함하는 단어의 비율은 약 30-40%로 추정된다. 이 모든 단어에 대해 suggest 품질이 저하된다.

---

## 7. 개선 전략

### 전략 A: 코드포인트 레벨 fuzzy search (근본적 해결)

Trie의 `fuzzy_search()`를 코드포인트 단위로 동작하도록 변경한다.

**방법 A1: Trie를 코드포인트 레벨로 변환**
- label 타입을 `u8` -> `u32`로 변경
- 삽입 시 `word.chars()`로 코드포인트 단위 삽입
- fuzzy_search query도 `word.chars()`로 변환
- 장점: 가장 정확한 edit distance
- 단점: LOUDS 공간 효율성 감소 (label 4x), 기존 직렬화 포맷 호환성 파괴

**방법 A2: 바이트 Trie 유지 + 코드포인트 인식 fuzzy search**
- Trie 구조는 바이트 레벨 유지
- `fuzzy_search()`만 수정: 다바이트 UTF-8 시퀀스를 인식하여 연속 바이트를 하나의 "문자 단위"로 처리
- DP 행 계산 시 다바이트 문자를 그룹으로 비교
- 장점: 직렬화 호환성 유지, 공간 효율성 유지
- 단점: 구현 복잡도 증가, DFS 탐색에서 다바이트 경로를 그룹화해야 함

### 전략 B: Hybrid 보정 (실용적 타협)

바이트 레벨 fuzzy search를 유지하되, 핀란드어 특화 보정을 추가한다.

**방법 B1: ö/ä 정규화 후 이중 검색**
```
suggest(word, max_edits):
  1. 원본 단어로 fuzzy search  (기존 로직)
  2. ö->o, ä->a 정규화된 단어로 fuzzy search (추가)
  3. 두 결과를 합산, 중복 제거, 코드포인트 레벨 거리로 재랭킹
```
- `tallö` -> 정규화: `tallo` -> fuzzy_search("tallo", 1) -> `talo` (byte dist=1) 발견!
- 장점: 기존 코드 최소 변경, 바이트 trie 유지
- 단점: 검색 2회 비용, 정규화로 인한 false positive 가능성

**방법 B2: Auto-escalation 임계값 조정**
```
suggest(word, max_edits):
  // 다바이트 문자가 포함된 경우 추가 여유분 허용
  let multibyte_count = word.chars().filter(|c| c.len_utf8() > 1).count();
  let effective_max = max_edits + multibyte_count;
  let limit = effective_max.min(4);
  // ...기존 auto-escalation 로직...
```
- 장점: 간단한 수정
- 단점: 검색 공간 폭발 가능성, 정확하지 않은 근사

**방법 B3: 코드포인트 기반 edit_distance 재랭킹**
```
suggest_ranked() 내부:
  1. 바이트 레벨 fuzzy search (넉넉한 max_edits)
  2. 코드포인트 레벨 edit distance로 재계산
  3. 코드포인트 거리 기준으로 필터링 및 정렬
```
- 장점: 최종 결과의 거리가 사용자 기대와 일치
- 단점: 넉넉한 max_edits를 얼마로 설정할지 결정 필요

### 전략 C: 코드포인트 레벨 edit_distance만 교체

`edit_distance()` 함수만 코드포인트 레벨로 변경하고, fuzzy_search는 바이트 유지.
- 랭킹과 user dictionary 스캔에서의 거리 계산은 정확해짐
- 그러나 trie fuzzy_search의 후보 생성 자체가 바이트 레벨이므로, 후보가 누락되는 근본 문제는 해결되지 않음

---

## 8. 권장 접근법 + 구현 로드맵

### 권장: 전략 B1 (정규화 이중 검색) + C (코드포인트 edit_distance)

두 전략의 조합이 최적의 비용/효과 비율을 제공한다.

#### Phase 1: 코드포인트 레벨 edit_distance (즉시, 1-2시간)

파일: `crates/mce-speller/src/pipeline.rs`, `crates/mce-fi/src/spellcheck.rs`

```rust
/// Compute Levenshtein edit distance at Unicode codepoint level.
fn edit_distance_codepoint(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let n = a_chars.len();
    let m = b_chars.len();

    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0usize; m + 1];

    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[m]
}
```

- `suggest_ranked()`의 `edit_distance()` 호출을 `edit_distance_codepoint()`로 교체
- user dictionary 스캔의 거리 계산도 동일하게 교체
- 이것만으로는 후보 누락 문제를 해결하지 않지만, 랭킹 정확도가 개선됨

#### Phase 2: 정규화 이중 검색 (2-4시간)

파일: `crates/mce-speller/src/pipeline.rs` 또는 `crates/mce-fi/src/spellcheck.rs`

```rust
/// Normalize Finnish diacritical characters: ö->o, ä->a
fn normalize_finnish(word: &str) -> String {
    word.chars().map(|c| match c {
        'ö' => 'o', 'ä' => 'a', 'å' => 'a',
        'Ö' => 'O', 'Ä' => 'A', 'Å' => 'A',
        _ => c,
    }).collect()
}

// In suggest():
pub fn suggest(&self, word: &str, max_edits: usize, max_suggestions: usize) -> Vec<String> {
    // 1. Original fuzzy search
    let mut raw_candidates = self.trie.fuzzy_search(word.as_bytes(), max_edits);

    // 2. Normalized fuzzy search (if word contains ä/ö/å)
    let normalized = normalize_finnish(word);
    if normalized != word {
        let bonus_edits = /* 다바이트 문자 수 */ ;
        let norm_candidates = self.trie.fuzzy_search(
            normalized.as_bytes(),
            max_edits.saturating_sub(1).max(1)  // 정규화가 1 edit를 소비
        );
        raw_candidates.extend(norm_candidates);
    }

    // 3. Deduplicate and re-rank by codepoint edit distance
    // ...
}
```

핵심 아이디어: `tallö`를 정규화하면 `tallo`가 되고, `fuzzy_search("tallo", 1)`은 `talo` (byte dist=1)를 포함한다.

#### Phase 3: 코드포인트 인식 fuzzy_search (선택적, 4-8시간)

파일: `crates/mce-core/src/trie/mod.rs`

바이트 Trie 구조를 유지하면서 fuzzy_search만 코드포인트 인식으로 변경하는 방법:

```rust
pub fn fuzzy_search_unicode(&self, query: &str, max_edits: usize) -> Vec<Vec<u8>> {
    let query_chars: Vec<char> = query.chars().collect();
    let query_len = query_chars.len();

    // DFS with codepoint-aware DP
    // 트리 탐색 시 다바이트 UTF-8 시퀀스를 인식하여
    // 연속 바이트를 하나의 코드포인트로 디코딩 후 비교
    // ...
}
```

이 방법은 구현 복잡도가 높으므로, Phase 1+2가 충분한 품질 개선을 제공하는지 평가 후 결정한다.

### 구현 우선순위

| 순서 | 작업 | 예상 시간 | 영향 |
|------|------|---------|------|
| 1 | `edit_distance_codepoint()` 추가 | 1시간 | 랭킹 정확도 개선 |
| 2 | 정규화 이중 검색 | 3시간 | tallö->talo 같은 누락 해결 |
| 3 | auto-escalation에서 codepoint 거리 기반 필터링 | 1시간 | false positive 감소 |
| 4 | (선택) codepoint 인식 fuzzy_search | 6시간 | 근본적 해결 |

### 테스트 계획

```rust
#[test]
fn suggest_finds_talo_from_tallö() {
    // tallö -> talo (char_dist=2, byte_dist=3)
    let suggestions = suggest("tallö", 2);
    assert!(suggestions.contains(&"talo".to_string()));
}

#[test]
fn suggest_finds_kasi_from_käsi() {
    // käsi -> kasi (char_dist=1, byte_dist=2)
    let suggestions = suggest("käsi", 1);
    assert!(suggestions.contains(&"kasi".to_string()));
}

#[test]
fn suggest_handles_double_umlaut() {
    // söpö -> sopo (char_dist=2, byte_dist=4)
    let suggestions = suggest("söpö", 2);
    assert!(suggestions.contains(&"sopo".to_string()));
}
```

### 성능 영향 예측

- Phase 1 (codepoint edit_distance): 약 1.5-2x 느려짐 (char 변환 비용). 그러나 edit_distance 계산은 후보 수에 비례하므로, 전체 suggest 시간의 일부분.
- Phase 2 (이중 검색): fuzzy_search 2회 실행. 최악의 경우 ~2x 느려짐. 그러나 정규화된 쿼리는 보통 더 짧으므로 (2바이트 -> 1바이트), 실제 비용 증가는 ~1.5x 수준.
- WASM 바이너리 크기 영향: 무시할 수 있음 (함수 몇 개 추가).

### 위험 요소

1. **정규화 이중 검색의 false positive**: `ö`와 `o`가 구별되어야 하는 단어 쌍 (예: `tuli` vs `tüli`)에서 불필요한 후보가 생성될 수 있다. 그러나 morph validation이 이를 걸러내므로 실제 문제는 작다.

2. **auto-escalation 상호작용**: 정규화 검색이 추가 결과를 제공하면, auto-escalation이 더 일찍 멈출 수 있다. 이는 의도된 동작이다.

3. **바이트 trie와 코드포인트 거리의 불일치**: 바이트 trie에서 "거리 2 이내"로 검색한 후보가 코드포인트 거리로는 1일 수 있고, 반대로 코드포인트 거리 1인 후보가 바이트 거리 3이어서 검색되지 않을 수 있다. Phase 2의 정규화 이중 검색이 후자의 문제를 완화한다.
