//! MCE Eval — UPOS evaluation infrastructure for UD treebank benchmarking.
//!
//! Evaluates MCE's morphological analysis pipeline against Universal
//! Dependencies gold-standard annotations, measuring UPOS accuracy,
//! lemma accuracy, and per-POS precision/recall/F1.
//!
//! # Architecture
//!
//! ```text
//! CoNLL-U file (gold) ──┐
//!                        ├──> Evaluate ──> Metrics
//! MCE Pipeline ─────────┘
//!   (tokenize → analyze → disambiguate → map POS)
//! ```
//!
//! # Modules
//!
//! - [`conllu`]: CoNLL-U format parser (reads `.conllu` files).
//! - [`pos_map`]: MCE Finnish class → UD UPOS tag mapping.
//! - [`metrics`]: Evaluation metrics (accuracy, P/R/F1, confusion matrix).
//! - [`pipeline`]: Evaluation pipeline connecting MCE to gold annotations.
//!
//! # Usage
//!
//! ```no_run
//! use std::path::Path;
//! use mce_eval::conllu::parse_conllu_file;
//! use mce_eval::pipeline::EvalPipeline;
//!
//! let dict_data = std::fs::read("path/to/mor.vfst").unwrap();
//! let pipeline = EvalPipeline::from_bytes(&dict_data).unwrap();
//!
//! let sentences = parse_conllu_file(Path::new("fi_tdt-ud-dev.conllu")).unwrap();
//! let results = pipeline.evaluate(&sentences);
//! println!("{}", results);
//! ```

pub mod conllu;
pub mod lemma_dict;
pub mod metrics;
pub mod pipeline;
pub mod pos_map;
