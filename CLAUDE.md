# MCE — Morphological Computation Engine

브라우저에서 서버 없이 동작하는 세계 최강 핀란드어 NLP 엔진.

## 목표 스펙

- 배포 크기: ~7.5MB
- 지연: <5ms/문장
- 정확도: UPOS 95%+
- 환경: WASM 브라우저 (오프라인)

## 아키텍처: MCE v3 (4기계)

| 기계 | 역할 | 수학적 기반 | Crate |
|------|------|-----------|-------|
| M1: Succinct Trie | 사전 검색/맞춤법 | LOUDS 인코딩 | `mce-core` (trie 모듈) |
| M2': Comonadic Engine | 형태소 분석 + 규칙 적용 | Comonad (extend/extract) | `mce-comonad` |
| M3: PDT | 복합어 구조 분석 | Pushdown Transducer | `mce-fst` |
| M4': Weighted Lattice + CS | Disambiguation | Viterbi + Compressed Sensing | `mce-disambig` |

## Crate 구조

```
crates/
├── mce-core/       # 공유 타입, 문자 분류, M1 Succinct Trie
├── mce-fst/        # FST 엔진 (포맷 추상화, VFST 순회)
├── mce-tokenizer/  # 텍스트 토크나이저
├── mce-speller/    # 맞춤법/추천 엔진
├── mce-disambig/   # M4' Disambiguation (Weighted Lattice + CS)
├── mce-comonad/    # M2' Comonadic 형태음운 엔진
├── mce-fi/         # Finnish 언어 모듈
├── mce-wasm/       # WASM 바인딩
└── mce-cli/        # CLI 도구
```

## 관련 프로젝트

- **연구 문서**: `~/oss/finnishNLP/mce-research/` (아키텍처, 수학 탐색, 논문 전략)
- **원본 참조**: `~/oss/corevoikko/libvoikko/rust/` (cherry-pick 원본)
- **참조 NLP**: `~/oss/finnishNLP/` (Omorfi, Trankit, UralicNLP, TNPP)

## cherry-pick 출처

corevoikko에서 ~25% cherry-pick. 적응 대상:

| MCE crate | corevoikko 원본 | 내용 |
|-----------|----------------|------|
| `mce-core` | `voikko-core` | Analysis, Token, Character, Case 타입 |
| `mce-fst` | `voikko-fst` | FST 순회 알고리즘, flag diacritics |
