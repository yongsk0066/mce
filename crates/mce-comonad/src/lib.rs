//! MCE Comonad — M2' Comonadic 형태음운 엔진.
//!
//! 핀란드어 형태음운 규칙(모음조화, 자음교체)을 comonad의 `extend` 연산으로 형식화.
//! Bimachine cascade를 coKleisli 합성으로 표현하여 규칙 합성의 정확성을 보장.
