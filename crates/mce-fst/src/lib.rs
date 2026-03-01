//! MCE FST Engine — FST 로딩 및 순회, 포맷 추상화.
//!
//! corevoikko의 voikko-fst에서 cherry-pick한 FST 순회 알고리즘을 기반으로,
//! VFST 바이너리 포맷의 로딩과 순회를 제공한다.
//!
//! # 모듈
//!
//! - [`format`]: VFST 바이너리 헤더 파싱
//! - [`symbols`]: 심볼 테이블 (char↔index 매핑)
//! - [`transition`]: 전이 구조체 (zero-copy)
//! - [`flags`]: Flag diacritic 연산 (P/C/U/R/D)
//! - [`config`]: 순회 상태 스택 (unweighted/weighted)
//! - [`unweighted`]: 비가중 FST 순회
//! - [`weighted`]: 가중 FST 순회

pub mod config;
pub mod flags;
pub mod format;
pub mod symbols;
pub mod transition;
pub mod unweighted;
pub mod weighted;

/// VFST 파싱 및 로딩 에러.
#[derive(Debug, thiserror::Error)]
pub enum VfstError {
    #[error("invalid magic number in VFST header")]
    InvalidMagic,
    #[error("file too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
    #[error("type mismatch: expected weighted={expected}, got weighted={actual}")]
    TypeMismatch { expected: bool, actual: bool },
    #[error("invalid symbol table: {0}")]
    InvalidSymbolTable(String),
    #[error("invalid flag diacritic: {0}")]
    InvalidFlagDiacritic(String),
    #[error("transition table alignment error")]
    AlignmentError,
}

/// 순회의 최대 반복 횟수 (무한 루프 방지).
pub const MAX_LOOP_COUNT: u32 = 100_000;

/// FST 순회 추상 인터페이스.
pub trait Transducer {
    type Config;

    /// 입력 문자열로 순회를 준비한다.
    fn prepare(&self, config: &mut Self::Config, input: &[char]) -> bool;

    /// 다음 출력을 생성한다.
    fn next(&self, config: &mut Self::Config, output: &mut String) -> bool;
}
