//! MCE Core — 공유 타입 및 기초 자료 구조.
//!
//! MCE(Morphological Computation Engine)의 모든 crate가 공유하는 기본 타입을 제공한다.
//!
//! - [`analysis`]: 형태소 분석 결과 (key-value 속성 집합)
//! - [`character`]: 문자 분류 및 Unicode 유틸리티
//! - [`case`]: 대소문자 패턴 감지 및 변환
//! - [`compound`]: M3 복합어 분석 (Pushdown Transducer)
//! - [`frequency`]: 단어 빈도 목록 (CoNLL-U 기반)
//! - [`string_utils`]: 문자열 유틸리티 (Levenshtein distance 등)
//! - [`token`]: 토큰 및 문장 경계 타입
//! - [`trie`]: M1 Succinct Trie (LOUDS 인코딩 사전)

pub mod analysis;
pub mod case;
pub mod character;
pub mod compound;
pub mod frequency;
pub mod string_utils;
pub mod token;
pub mod trie;
