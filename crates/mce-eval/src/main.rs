//! MCE Eval — evaluate UPOS accuracy against UD treebanks.
//!
//! # Usage
//!
//! ```bash
//! export MCE_DICT_PATH=~/oss/corevoikko/voikko-fi/vvfst
//! mce-eval --conllu path/to/fi_tdt-ud-dev.conllu
//! mce-eval --conllu path/to/fi_tdt-ud-dev.conllu --end-to-end
//! mce-eval --conllu path/to/fi_tdt-ud-dev.conllu --max-sentences 100
//! ```

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::Instant;

use mce_eval::conllu::parse_conllu_file;
use mce_eval::pipeline::EvalPipeline;

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

struct Args {
    dict_path: PathBuf,
    conllu_path: PathBuf,
    train_path: Option<PathBuf>,
    end_to_end: bool,
    include_punct: bool,
    max_sentences: Option<usize>,
    enable_cs: bool,
    cs_measurements: usize,
    cs_lambda: f64,
    cs_only: bool,
}

fn parse_args() -> Args {
    let args: Vec<String> = env::args().collect();

    let mut dict_path: Option<PathBuf> = None;
    let mut conllu_path: Option<PathBuf> = None;
    let mut train_path: Option<PathBuf> = None;
    let mut end_to_end = false;
    let mut include_punct = false;
    let mut max_sentences: Option<usize> = None;
    let mut enable_cs = false;
    let mut cs_measurements: usize = 50;
    let mut cs_lambda: f64 = 0.1;
    let mut cs_only = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dict-path" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --dict-path requires a value.");
                    process::exit(1);
                }
                dict_path = Some(PathBuf::from(&args[i]));
            }
            "--conllu" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --conllu requires a value.");
                    process::exit(1);
                }
                conllu_path = Some(PathBuf::from(&args[i]));
            }
            "--end-to-end" => {
                end_to_end = true;
            }
            "--include-punct" => {
                include_punct = true;
            }
            "--train" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --train requires a value.");
                    process::exit(1);
                }
                train_path = Some(PathBuf::from(&args[i]));
            }
            "--max-sentences" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --max-sentences requires a value.");
                    process::exit(1);
                }
                max_sentences = Some(args[i].parse().unwrap_or_else(|_| {
                    eprintln!("error: --max-sentences must be a positive integer.");
                    process::exit(1);
                }));
            }
            "--enable-cs" => {
                enable_cs = true;
            }
            "--cs-only" => {
                cs_only = true;
                enable_cs = true;
            }
            "--cs-measurements" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --cs-measurements requires a value.");
                    process::exit(1);
                }
                cs_measurements = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("error: --cs-measurements must be a positive integer.");
                    process::exit(1);
                });
            }
            "--cs-lambda" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --cs-lambda requires a value.");
                    process::exit(1);
                }
                cs_lambda = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("error: --cs-lambda must be a number.");
                    process::exit(1);
                });
            }
            "--help" | "-h" | "help" => {
                print_usage();
                process::exit(0);
            }
            other => {
                eprintln!("error: unknown argument '{}'.", other);
                eprintln!();
                print_usage();
                process::exit(1);
            }
        }
        i += 1;
    }

    // Resolve dict-path: CLI flag > environment variable.
    let dict_path = dict_path.unwrap_or_else(|| match env::var("MCE_DICT_PATH") {
        Ok(p) => PathBuf::from(p),
        Err(_) => {
            eprintln!("error: dictionary path not specified.");
            eprintln!();
            eprintln!("Use --dict-path or set MCE_DICT_PATH environment variable.");
            eprintln!("The path should point to the directory containing mor.vfst.");
            process::exit(1);
        }
    });

    let conllu_path = conllu_path.unwrap_or_else(|| {
        eprintln!("error: --conllu is required.");
        eprintln!();
        print_usage();
        process::exit(1);
    });

    Args {
        dict_path,
        conllu_path,
        train_path,
        end_to_end,
        include_punct,
        max_sentences,
        enable_cs,
        cs_measurements,
        cs_lambda,
        cs_only,
    }
}

fn print_usage() {
    eprintln!("MCE Eval -- UPOS Evaluation against UD Treebanks");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("    mce-eval --conllu <FILE> [OPTIONS]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("    --conllu <FILE>          Path to CoNLL-U file (required)");
    eprintln!("    --dict-path <DIR>        Directory containing mor.vfst");
    eprintln!("                             (or set MCE_DICT_PATH env var)");
    eprintln!("    --train <FILE>           CoNLL-U training file for corpus bigrams");
    eprintln!("    --end-to-end             Use MCE tokenizer instead of gold tokens");
    eprintln!("    --include-punct          Include PUNCT/SYM tokens in evaluation");
    eprintln!("    --max-sentences <N>      Evaluate only the first N sentences");
    eprintln!("    --enable-cs              Enable Compressed Sensing (FISTA) scorer");
    eprintln!("    --cs-only                Use CS scorer without emission priors");
    eprintln!("    --cs-measurements <N>    Number of CS measurements (default: 50)");
    eprintln!("    --cs-lambda <F>          CS L1 regularization (default: 0.1)");
    eprintln!("    --help, -h               Show this help message");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("    export MCE_DICT_PATH=~/oss/corevoikko/voikko-fi/vvfst");
    eprintln!("    mce-eval --conllu fi_tdt-ud-dev.conllu");
    eprintln!("    mce-eval --conllu fi_tdt-ud-test.conllu --max-sentences 50");
    eprintln!("    mce-eval --conllu fi_tdt-ud-dev.conllu --end-to-end");
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let args = parse_args();

    // Load dictionary.
    let dict_file = args.dict_path.join("mor.vfst");
    eprintln!("Loading dictionary from {} ...", dict_file.display());

    let dict_data = match fs::read(&dict_file) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", dict_file.display(), e);
            process::exit(1);
        }
    };

    eprintln!(
        "Dictionary loaded: {:.1} MB",
        dict_data.len() as f64 / 1_048_576.0
    );

    // Build pipeline.
    let mut pipeline = if let Some(ref train_path) = args.train_path {
        eprintln!("Loading training data from {} ...", train_path.display());
        let train_data = match fs::read_to_string(train_path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("error: cannot read {}: {}", train_path.display(), e);
                process::exit(1);
            }
        };
        if args.cs_only {
            eprintln!("Building corpus-trained bigram model (no emission priors) ...");
            match EvalPipeline::from_bytes_with_corpus_no_emission(&dict_data, &train_data) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error: failed to initialize pipeline: {}", e);
                    process::exit(1);
                }
            }
        } else {
            eprintln!("Building corpus-trained bigram model ...");
            match EvalPipeline::from_bytes_with_corpus(&dict_data, &train_data) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error: failed to initialize pipeline: {}", e);
                    process::exit(1);
                }
            }
        }
    } else {
        match EvalPipeline::from_bytes(&dict_data) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("error: failed to initialize pipeline: {}", e);
                process::exit(1);
            }
        }
    };

    // Enable Compressed Sensing scorer if requested.
    if args.enable_cs {
        eprintln!(
            "Enabling Compressed Sensing (FISTA) scorer: m={}, lambda={}{}",
            args.cs_measurements,
            args.cs_lambda,
            if args.cs_only {
                " (CS-only, no emission priors)"
            } else {
                ""
            }
        );
        pipeline.enable_cs(args.cs_measurements, args.cs_lambda);
    }

    // Parse CoNLL-U file.
    eprintln!("Parsing CoNLL-U file: {} ...", args.conllu_path.display());

    let mut sentences = match parse_conllu_file(&args.conllu_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", args.conllu_path.display(), e);
            process::exit(1);
        }
    };

    eprintln!("Parsed {} sentences.", sentences.len());

    // Apply max-sentences limit.
    if let Some(max) = args.max_sentences {
        if max < sentences.len() {
            sentences.truncate(max);
            eprintln!("Evaluating first {} sentences.", max);
        }
    }

    // Count tokens for progress reporting.
    let total_tokens: usize = sentences.iter().map(|s| s.tokens.len()).sum();
    eprintln!("Total tokens: {}", total_tokens);

    // Run evaluation.
    let mode = if args.end_to_end {
        "end-to-end (MCE tokenizer)"
    } else {
        "gold tokenization"
    };
    eprintln!("Running evaluation ({}) ...", mode);

    let start = Instant::now();

    let results = if args.end_to_end {
        pipeline.evaluate_end_to_end(&sentences)
    } else {
        pipeline.evaluate_filtered(&sentences, !args.include_punct)
    };

    let elapsed = start.elapsed();

    // Print results.
    println!();
    println!("{}", results);
    println!();
    println!(
        "Evaluation time:  {:.2}s ({:.1} tokens/sec)",
        elapsed.as_secs_f64(),
        results.total as f64 / elapsed.as_secs_f64(),
    );
    println!("Sentences:        {}", sentences.len(),);
    println!("Mode:             {}", mode);
    if args.enable_cs {
        println!(
            "CS scorer:        enabled (m={}, lambda={}){}",
            args.cs_measurements,
            args.cs_lambda,
            if args.cs_only { " [CS-ONLY]" } else { "" }
        );
    } else {
        println!("CS scorer:        disabled");
    }
}
