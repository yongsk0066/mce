//! MCE Speller — 맞춤법 검사 및 추천 엔진.
//!
//! LRU 캐시, 편집 거리 기반 후보 생성, 우선순위 큐 수집.
//! M1(SuccinctTrie)과 M2(FST)를 조합하여 추천을 생성한다.
