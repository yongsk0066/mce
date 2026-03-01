//! MCE FST Engine — FST 로딩 및 순회, 포맷 추상화.
//!
//! corevoikko의 voikko-fst에서 cherry-pick한 FST 순회 알고리즘을 기반으로,
//! VFST/HFST 등 다양한 FST 포맷을 추상화한다.
//!
//! # 모듈
//!
//! - `config`: 순회 상태 스택 (unweighted/weighted)
//! - `flags`: Flag diacritic 연산 (P/C/U/R/D)
//! - `unweighted`: 비가중 FST 순회
//! - `weighted`: 가중 FST 순회

// TODO: cherry-pick traversal modules from corevoikko
// pub mod config;
// pub mod flags;
// pub mod format;
// pub mod symbols;
// pub mod transition;
// pub mod unweighted;
// pub mod weighted;

/// FST 순회의 최대 반복 횟수 (무한 루프 방지).
pub const MAX_LOOP_COUNT: u32 = 100_000;

/// FST 순회 추상 인터페이스.
pub trait Transducer {
    type Config;

    /// 입력 문자열로 순회를 준비한다.
    /// 모든 입력 문자가 알려진 심볼이면 `true`.
    fn prepare(&self, config: &mut Self::Config, input: &[char]) -> bool;

    /// 다음 출력을 생성한다.
    /// 출력이 있으면 `true`, 더 이상 없으면 `false`.
    fn next(&self, config: &mut Self::Config, output: &mut String) -> bool;
}
