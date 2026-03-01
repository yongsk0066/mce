//! MCE CLI — interactive testing tool for the Morphological Computation Engine.
//!
//! Provides subcommands for morphological analysis, spell-checking, compound
//! word analysis, sentence-level disambiguation, grammar checking, hyphenation,
//! evaluation against UD treebanks, and performance benchmarking using the VFST
//! dictionary.
//!
//! # Usage
//!
//! Set `MCE_DICT_PATH` to the directory containing `mor.vfst`:
//!
//! ```bash
//! export MCE_DICT_PATH=~/oss/corevoikko/voikko-fi/vvfst
//! mce-cli analyze koira
//! mce-cli spell koirra
//! mce-cli compound rautatieasema
//! mce-cli sentence "koira juoksee"
//! mce-cli grammar "Koira koira juoksee pihalla."
//! mce-cli hyphenate suomalainen rautatieasema
//! mce-cli hyphenate-text "Koira juoksee pihalla nopeasti."
//! mce-cli info
//! mce-cli eval --conllu fi_tdt-ud-dev.conllu
//! mce-cli benchmark --iterations 500
//! ```

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::Instant;

use mce_core::analysis::{
    Analysis, ATTR_BASEFORM, ATTR_CLASS, ATTR_COMPARISON, ATTR_MOOD, ATTR_NEGATIVE, ATTR_NUMBER,
    ATTR_PARTICIPLE, ATTR_PERSON, ATTR_SIJAMUOTO, ATTR_STRUCTURE, ATTR_TENSE, ATTR_WORDBASES,
};
use mce_core::token::TokenType;
use mce_disambig::{Disambiguator, ViterbiDisambiguator};
use mce_eval::conllu::parse_conllu_file;
use mce_eval::pipeline::EvalPipeline;
use mce_fi::compound::FinnishCompoundAnalyzer;
use mce_fi::hyphenation::FinnishHyphenator;
use mce_fi::morphology::{Analyzer, FinnishAnalyzer};
use mce_fi::spellcheck::FinnishSpellChecker;
use mce_fst::unweighted::UnweightedTransducer;
use mce_grammar::finnish::FinnishGrammarChecker;
use mce_grammar::GrammarChecker;
use mce_speller::SpellResult;
use mce_tokenizer::next_token;

// ---------------------------------------------------------------------------
// Dictionary loading
// ---------------------------------------------------------------------------

/// Load the VFST dictionary bytes from `MCE_DICT_PATH/mor.vfst`.
fn load_dictionary() -> Vec<u8> {
    let dir = match env::var("MCE_DICT_PATH") {
        Ok(p) => PathBuf::from(p),
        Err(_) => {
            eprintln!("error: MCE_DICT_PATH environment variable is not set.");
            eprintln!();
            eprintln!("Set it to the directory containing mor.vfst:");
            eprintln!("  export MCE_DICT_PATH=~/oss/corevoikko/voikko-fi/vvfst");
            process::exit(1);
        }
    };

    let path = dir.join("mor.vfst");
    match fs::read(&path) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", path.display(), e);
            process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// Subcommands
// ---------------------------------------------------------------------------

/// `mce-cli analyze <word>` -- morphological analysis.
fn cmd_analyze(word: &str) {
    let data = load_dictionary();
    let analyzer = match FinnishAnalyzer::from_bytes(&data) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: failed to load analyzer: {e}");
            process::exit(1);
        }
    };

    let chars: Vec<char> = word.chars().collect();
    let analyses = analyzer.analyze(&chars, chars.len());

    if analyses.is_empty() {
        println!("{word}: no analyses found");
        return;
    }

    println!("{word}: {} analysis(es)", analyses.len());
    for (i, a) in analyses.iter().enumerate() {
        let summary = format_analysis(a);
        println!("  [{}] {}", i + 1, summary);
    }
}

/// `mce-cli spell <word>` -- spell-checking with suggestions.
fn cmd_spell(word: &str) {
    let data = load_dictionary();
    let mut checker = match FinnishSpellChecker::from_bytes(&data) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to load spell checker: {e}");
            process::exit(1);
        }
    };

    let result = checker.check(word);

    match result {
        SpellResult::Ok => {
            println!("{word}: OK");
        }
        SpellResult::CapitalizeFirst => {
            println!("{word}: OK (capitalize first letter)");
        }
        SpellResult::CapitalizationError => {
            println!("{word}: CAPITALIZATION ERROR");
            print_suggestions(&checker, word);
        }
        SpellResult::Failed => {
            println!("{word}: MISSPELLED");
            print_suggestions(&checker, word);
        }
    }
}

/// Print spelling suggestions for a misspelled word.
fn print_suggestions(checker: &FinnishSpellChecker, word: &str) {
    for max_d in 1..=2 {
        let suggestions = checker.suggest(word, max_d);
        if !suggestions.is_empty() {
            println!("  Suggestions (d<={}): {}", max_d, suggestions.join(", "));
            return;
        }
    }
    // Try unfiltered as fallback.
    let unfiltered = checker.suggest_unfiltered(word, 2);
    if !unfiltered.is_empty() {
        println!("  Candidates (unfiltered, d<=2): {}", unfiltered.join(", "));
    } else {
        println!("  No suggestions found.");
    }
}

/// `mce-cli compound <word>` -- compound word analysis.
fn cmd_compound(word: &str) {
    let data = load_dictionary();
    let analyzer = match FinnishCompoundAnalyzer::from_bytes(&data) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: failed to load compound analyzer: {e}");
            process::exit(1);
        }
    };

    let splits = analyzer.analyze(word);

    if splits.is_empty() {
        println!("{word}: not a compound word (or single dictionary word)");
        return;
    }

    println!("{word}: compound word ({} split(s))", splits.len());
    for (i, split) in splits.iter().enumerate() {
        let word_parts: Vec<&str> = split
            .word_parts()
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        println!(
            "  Split {} (penalty {}): {}",
            i + 1,
            split.penalty,
            word_parts.join(" + ")
        );

        // Show linking elements if any.
        let linking: Vec<&str> = split
            .parts
            .iter()
            .filter(|p| p.is_linking)
            .map(|p| p.surface.as_str())
            .collect();
        if !linking.is_empty() {
            println!("    linking: {}", linking.join(", "));
        }
    }
}

/// `mce-cli sentence <text>` -- tokenize, analyze, and disambiguate.
fn cmd_sentence(text: &str) {
    let data = load_dictionary();
    let analyzer = match FinnishAnalyzer::from_bytes(&data) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: failed to load analyzer: {e}");
            process::exit(1);
        }
    };

    // Tokenize the input text.
    let chars: Vec<char> = text.chars().collect();
    let text_len = chars.len();
    let mut pos = 0;
    let mut words: Vec<String> = Vec::new();
    let mut word_analyses: Vec<Vec<Analysis>> = Vec::new();

    while pos < text_len {
        let (token_type, token_len) = next_token(&chars, text_len, pos);
        if token_len == 0 {
            break;
        }

        if token_type == TokenType::Word {
            let word_str: String = chars[pos..pos + token_len].iter().collect();
            let word_chars: Vec<char> = word_str.chars().collect();
            let analyses = analyzer.analyze(&word_chars, word_chars.len());
            words.push(word_str);
            word_analyses.push(analyses);
        }

        pos += token_len;
    }

    if words.is_empty() {
        println!("No words found in input.");
        return;
    }

    // Disambiguate if there is ambiguity.
    let disambiguator = ViterbiDisambiguator::with_finnish_defaults();
    let best = disambiguator.disambiguate(&word_analyses);

    // Print results.
    for (i, word_str) in words.iter().enumerate() {
        let analysis_count = word_analyses[i].len();

        if i < best.len() {
            let a = &best[i];
            let class = a.get(ATTR_CLASS).unwrap_or("?");
            let detail = format_analysis_short(a);

            if analysis_count > 1 {
                println!(
                    "[{}] {} -> {} ({}) [1/{} readings]",
                    i + 1,
                    word_str,
                    class,
                    detail,
                    analysis_count,
                );
            } else if analysis_count == 1 {
                println!("[{}] {} -> {} ({})", i + 1, word_str, class, detail);
            } else {
                println!("[{}] {} -> UNKNOWN", i + 1, word_str);
            }
        } else {
            // No disambiguation result (should not happen, but be safe).
            println!("[{}] {} -> UNKNOWN", i + 1, word_str);
        }
    }
}

/// `mce-cli grammar <text>` -- check grammar of input text.
fn cmd_grammar(text: &str) {
    let data = load_dictionary();
    let checker = match FinnishGrammarChecker::new(&data) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to load grammar checker: {e}");
            process::exit(1);
        }
    };

    println!("Checking: {:?}", text);
    println!();

    let errors = checker.check(text);

    if errors.is_empty() {
        println!("No errors found.");
        return;
    }

    for error in &errors {
        let span = &text[error.start..error.end];
        println!("Error at {}..{}: {}", error.start, error.end, error.code);
        println!("  {:?} \u{2014} {}", span, error.message);
        if !error.suggestions.is_empty() {
            println!("  Suggestion: {}", error.suggestions.join(", "));
        }
        println!();
    }

    println!("{} error(s) found.", errors.len());
}

/// `mce-cli hyphenate <word>...` -- hyphenate individual words.
fn cmd_hyphenate(words: &[String]) {
    let hyphenator = FinnishHyphenator::new();

    for word in words {
        let result = hyphenator.hyphenate_word(word);
        println!("{} \u{2192} {}", word, result);
    }
}

/// `mce-cli hyphenate-text <text>` -- hyphenate running text.
fn cmd_hyphenate_text(text: &str) {
    let hyphenator = FinnishHyphenator::new();
    let chars: Vec<char> = text.chars().collect();
    let text_len = chars.len();
    let mut result = String::with_capacity(text.len() * 2);
    let mut pos = 0;

    while pos < text_len {
        let (token_type, token_len) = next_token(&chars, text_len, pos);

        if token_len == 0 {
            break;
        }

        let token_str: String = chars[pos..pos + token_len].iter().collect();

        if token_type == TokenType::Word {
            result.push_str(&hyphenator.hyphenate_word(&token_str));
        } else {
            result.push_str(&token_str);
        }

        pos += token_len;
    }

    println!("{}", result);
}

/// `mce-cli info` -- show dictionary metadata.
fn cmd_info() {
    let data = load_dictionary();

    let transducer = match UnweightedTransducer::from_bytes(&data) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: failed to load transducer: {e}");
            process::exit(1);
        }
    };

    let symbols = transducer.symbols();
    let total_symbols = symbols.symbol_strings.len();
    let normal_chars = symbols.first_multi_char as usize - symbols.first_normal_char as usize;
    let multi_chars = total_symbols - symbols.first_multi_char as usize;
    let flag_features = symbols.flag_feature_count;

    println!("MCE Dictionary Info");
    println!("-------------------");
    println!(
        "File size:        {} bytes ({:.1} MB)",
        data.len(),
        data.len() as f64 / 1_048_576.0
    );
    println!("Total symbols:    {}", total_symbols);
    println!("  Normal chars:   {}", normal_chars);
    println!("  Multi-char:     {}", multi_chars);
    println!("  Flag features:  {}", flag_features);
    println!("Transducer:       {:?}", transducer);
}

/// `mce-cli eval --conllu <path> [--train <path>] [--format table|json]`
///
/// Evaluate UPOS, Lemma, and Coverage against a CoNLL-U gold-standard file.
fn cmd_eval(args: &[String]) {
    let mut conllu_path: Option<PathBuf> = None;
    let mut train_path: Option<PathBuf> = None;
    let mut format = "table".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--conllu" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --conllu requires a value.");
                    process::exit(1);
                }
                conllu_path = Some(PathBuf::from(&args[i]));
            }
            "--train" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --train requires a value.");
                    process::exit(1);
                }
                train_path = Some(PathBuf::from(&args[i]));
            }
            "--format" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --format requires a value (table or json).");
                    process::exit(1);
                }
                match args[i].as_str() {
                    "table" | "json" => format = args[i].clone(),
                    other => {
                        eprintln!("error: unknown format '{other}'. Use 'table' or 'json'.");
                        process::exit(1);
                    }
                }
            }
            other => {
                eprintln!("error: unknown eval option '{other}'.");
                eprintln!(
                    "usage: mce-cli eval --conllu <FILE> [--train <FILE>] [--format table|json]"
                );
                process::exit(1);
            }
        }
        i += 1;
    }

    let conllu_path = conllu_path.unwrap_or_else(|| {
        eprintln!("error: --conllu is required.");
        eprintln!("usage: mce-cli eval --conllu <FILE> [--train <FILE>] [--format table|json]");
        process::exit(1);
    });

    let data = load_dictionary();

    // Build pipeline.
    let pipeline = if let Some(ref tp) = train_path {
        eprintln!("Loading training data from {} ...", tp.display());
        let train_data = match fs::read_to_string(tp) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("error: cannot read {}: {}", tp.display(), e);
                process::exit(1);
            }
        };
        eprintln!("Building corpus-trained bigram model ...");
        match EvalPipeline::from_bytes_with_corpus(&data, &train_data) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("error: failed to initialize pipeline: {e}");
                process::exit(1);
            }
        }
    } else {
        match EvalPipeline::from_bytes(&data) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("error: failed to initialize pipeline: {e}");
                process::exit(1);
            }
        }
    };

    // Parse CoNLL-U file.
    eprintln!("Parsing CoNLL-U file: {} ...", conllu_path.display());
    let sentences = match parse_conllu_file(&conllu_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", conllu_path.display(), e);
            process::exit(1);
        }
    };

    let total_tokens: usize = sentences.iter().map(|s| s.tokens.len()).sum();
    eprintln!(
        "Parsed {} sentences, {} tokens.",
        sentences.len(),
        total_tokens
    );

    // Run evaluation.
    let start = Instant::now();
    let results = pipeline.evaluate(&sentences);
    let elapsed = start.elapsed();

    match format.as_str() {
        "json" => {
            // Build JSON output manually (no serde dependency).
            println!("{{");
            println!("  \"upos_accuracy\": {:.4},", results.upos_accuracy());
            println!("  \"lemma_accuracy\": {:.4},", results.lemma_accuracy());
            println!("  \"coverage\": {:.4},", results.coverage());
            println!("  \"total_tokens\": {},", results.total);
            println!("  \"upos_correct\": {},", results.upos_correct);
            println!("  \"lemma_correct\": {},", results.lemma_correct);
            println!("  \"unknown_count\": {},", results.unknown_count);
            println!("  \"eval_time_secs\": {:.3},", elapsed.as_secs_f64());
            println!(
                "  \"tokens_per_sec\": {:.1},",
                results.total as f64 / elapsed.as_secs_f64()
            );
            println!("  \"sentences\": {},", sentences.len());

            // Per-POS breakdown.
            let tags = results.all_tags();
            println!("  \"per_pos\": {{");
            for (ti, tag) in tags.iter().enumerate() {
                let comma = if ti + 1 < tags.len() { "," } else { "" };
                println!(
                    "    \"{}\": {{\"precision\": {:.4}, \"recall\": {:.4}, \"f1\": {:.4}, \"support\": {}}}{}",
                    tag,
                    results.precision(tag),
                    results.recall(tag),
                    results.f1(tag),
                    results.support(tag),
                    comma,
                );
            }
            println!("  }},");

            // Top confusions.
            let top = results.top_confusions(10);
            println!("  \"top_confusions\": [");
            for (ci, ((gold, pred), count)) in top.iter().enumerate() {
                let comma = if ci + 1 < top.len() { "," } else { "" };
                println!(
                    "    {{\"gold\": \"{}\", \"pred\": \"{}\", \"count\": {}}}{}",
                    gold, pred, count, comma,
                );
            }
            println!("  ]");
            println!("}}");
        }
        _ => {
            // Table format (default).
            println!();
            println!("{}", results);
            println!();
            println!(
                "Evaluation time:  {:.2}s ({:.1} tokens/sec)",
                elapsed.as_secs_f64(),
                results.total as f64 / elapsed.as_secs_f64(),
            );
            println!("Sentences:        {}", sentences.len());
            if train_path.is_some() {
                println!("Training data:    yes (corpus bigrams + emission priors)");
            } else {
                println!("Training data:    none (default bigrams)");
            }
        }
    }
}

/// `mce-cli benchmark [--iterations N] [--warmup W]`
///
/// Run performance benchmarks on the full MCE pipeline.
fn cmd_benchmark(args: &[String]) {
    let mut iterations: usize = 1000;
    let mut warmup: usize = 100;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--iterations" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --iterations requires a value.");
                    process::exit(1);
                }
                iterations = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("error: --iterations must be a positive integer.");
                    process::exit(1);
                });
            }
            "--warmup" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --warmup requires a value.");
                    process::exit(1);
                }
                warmup = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("error: --warmup must be a positive integer.");
                    process::exit(1);
                });
            }
            other => {
                eprintln!("error: unknown benchmark option '{other}'.");
                eprintln!("usage: mce-cli benchmark [--iterations N] [--warmup W]");
                process::exit(1);
            }
        }
        i += 1;
    }

    // Standard Finnish test sentences (varying complexity).
    let sentences: &[(&str, &str)] = &[
        ("short (5 words)", "Koira juoksee pihalla nopeasti tänään."),
        (
            "medium (15 words)",
            "Suomen tasavallan presidentti piti tänään puheen eduskunnassa ja korosti kansainvälisen yhteistyön merkitystä tulevaisuudessa kaikille kansalaisille.",
        ),
        (
            "long (30 words)",
            "Helsingin yliopiston tutkijat ovat kehittäneet uuden menetelmän joka mahdollistaa luonnollisen kielen käsittelyn entistä tehokkaammin ja tarkemmin erityisesti suomen kielen monimutkaisessa morfologisessa järjestelmässä jossa sanojen taivutusmuodot ovat erittäin moninaisia ja vaihtelevia.",
        ),
        (
            "compounds",
            "Rautatieaseman lippuautomaatti oli epäkunnossa ja junaliikenteen aikataulunäyttö näytti väärää kellonaikaa.",
        ),
        (
            "foreign words",
            "Machine learning -algoritmi ja deep learning -malli käyttävät transformer-arkkitehtuuria.",
        ),
    ];

    let data = load_dictionary();

    // Initialize all components.
    let analyzer = match FinnishAnalyzer::from_bytes(&data) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: failed to load analyzer: {e}");
            process::exit(1);
        }
    };
    let disambiguator = ViterbiDisambiguator::with_finnish_defaults();
    let grammar_checker = match FinnishGrammarChecker::new(&data) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to load grammar checker: {e}");
            process::exit(1);
        }
    };

    println!("MCE Performance Benchmark");
    println!("=========================");
    println!("Iterations: {}", iterations);
    println!("Warmup:     {}", warmup);
    println!();

    let mut total_tokens = 0usize;
    let mut total_pipeline_ns = 0u128;

    for (label, text) in sentences {
        // Tokenize once to count tokens.
        let chars: Vec<char> = text.chars().collect();
        let text_len = chars.len();
        let (words, _) = tokenize_text(&chars, text_len, &analyzer);
        let token_count = words.len();
        total_tokens += token_count * iterations;

        println!("--- {} ---", label);
        println!("  Text: {:?}", text);
        println!("  Tokens: {}", token_count);

        // Phase 1: Tokenization only.
        let tok_dur = bench_phase(warmup, iterations, || {
            let chars: Vec<char> = text.chars().collect();
            let len = chars.len();
            let mut pos = 0;
            while pos < len {
                let (_, token_len) = next_token(&chars, len, pos);
                if token_len == 0 {
                    break;
                }
                pos += token_len;
            }
        });

        // Phase 2: Tokenization + Analysis.
        let analysis_dur = bench_phase(warmup, iterations, || {
            let chars: Vec<char> = text.chars().collect();
            let len = chars.len();
            tokenize_text(&chars, len, &analyzer);
        });

        // Phase 3: Full pipeline (tokenize + analyze + disambiguate).
        let pipeline_dur = bench_phase(warmup, iterations, || {
            let chars: Vec<char> = text.chars().collect();
            let len = chars.len();
            let (_, word_analyses) = tokenize_text(&chars, len, &analyzer);
            disambiguator.disambiguate(&word_analyses);
        });

        // Phase 4: Grammar check.
        let grammar_dur = bench_phase(warmup, iterations, || {
            grammar_checker.check(text);
        });

        total_pipeline_ns += pipeline_dur.as_nanos() * iterations as u128;

        let ms_per_sent = pipeline_dur.as_secs_f64() * 1000.0;
        let ms_per_tok = if token_count > 0 {
            ms_per_sent / token_count as f64
        } else {
            0.0
        };

        println!(
            "  Tokenization:      {:>8.3} ms/sentence",
            tok_dur.as_secs_f64() * 1000.0
        );
        println!(
            "  Analysis:          {:>8.3} ms/sentence",
            analysis_dur.as_secs_f64() * 1000.0
        );
        println!(
            "  Full pipeline:     {:>8.3} ms/sentence  ({:.3} ms/token)",
            ms_per_sent, ms_per_tok
        );
        println!(
            "  Grammar check:     {:>8.3} ms/sentence",
            grammar_dur.as_secs_f64() * 1000.0
        );
        println!();
    }

    // Summary.
    let total_secs = total_pipeline_ns as f64 / 1_000_000_000.0;
    let tokens_per_sec = if total_secs > 0.0 {
        total_tokens as f64 / total_secs
    } else {
        0.0
    };

    println!("=== Summary ===");
    println!(
        "Total tokens processed:  {} (across {} iterations)",
        total_tokens, iterations
    );
    println!("Throughput:              {:.0} tokens/sec", tokens_per_sec);
}

/// Tokenize text and analyze each word. Returns words and their analyses.
fn tokenize_text(
    chars: &[char],
    text_len: usize,
    analyzer: &FinnishAnalyzer,
) -> (Vec<String>, Vec<Vec<Analysis>>) {
    let mut pos = 0;
    let mut words = Vec::new();
    let mut word_analyses = Vec::new();

    while pos < text_len {
        let (token_type, token_len) = next_token(chars, text_len, pos);
        if token_len == 0 {
            break;
        }
        if token_type == TokenType::Word {
            let word_str: String = chars[pos..pos + token_len].iter().collect();
            let word_chars: Vec<char> = word_str.chars().collect();
            let analyses = analyzer.analyze(&word_chars, word_chars.len());
            words.push(word_str);
            word_analyses.push(analyses);
        }
        pos += token_len;
    }

    (words, word_analyses)
}

/// Run a benchmark phase: warmup, then measure average duration per iteration.
fn bench_phase<F: FnMut()>(warmup: usize, iterations: usize, mut f: F) -> std::time::Duration {
    // Warmup.
    for _ in 0..warmup {
        f();
    }

    // Measure.
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();

    elapsed / iterations as u32
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Format a full analysis as a single-line summary string.
fn format_analysis(a: &Analysis) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(v) = a.get(ATTR_CLASS) {
        parts.push(format!("CLASS={v}"));
    }
    if let Some(v) = a.get(ATTR_SIJAMUOTO) {
        parts.push(format!("SIJAMUOTO={v}"));
    }
    if let Some(v) = a.get(ATTR_NUMBER) {
        parts.push(format!("NUMBER={v}"));
    }
    if let Some(v) = a.get(ATTR_PERSON) {
        parts.push(format!("PERSON={v}"));
    }
    if let Some(v) = a.get(ATTR_MOOD) {
        parts.push(format!("MOOD={v}"));
    }
    if let Some(v) = a.get(ATTR_TENSE) {
        parts.push(format!("TENSE={v}"));
    }
    if let Some(v) = a.get(ATTR_NEGATIVE) {
        parts.push(format!("NEGATIVE={v}"));
    }
    if let Some(v) = a.get(ATTR_PARTICIPLE) {
        parts.push(format!("PARTICIPLE={v}"));
    }
    if let Some(v) = a.get(ATTR_COMPARISON) {
        parts.push(format!("COMPARISON={v}"));
    }
    if let Some(v) = a.get(ATTR_BASEFORM) {
        parts.push(format!("BASEFORM={v}"));
    }
    if let Some(v) = a.get(ATTR_STRUCTURE) {
        parts.push(format!("STRUCTURE={v}"));
    }
    if let Some(v) = a.get(ATTR_WORDBASES) {
        parts.push(format!("WORDBASES={v}"));
    }

    if parts.is_empty() {
        "(empty)".to_string()
    } else {
        parts.join(" ")
    }
}

/// Format a short disambiguation summary (parenthesized details).
fn format_analysis_short(a: &Analysis) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(v) = a.get(ATTR_SIJAMUOTO) {
        parts.push(v.to_string());
    }
    if let Some(v) = a.get(ATTR_MOOD) {
        parts.push(v.to_string());
    }
    if let Some(v) = a.get(ATTR_TENSE) {
        parts.push(v.to_string());
    }
    if let Some(v) = a.get(ATTR_NUMBER) {
        parts.push(v.to_string());
    }
    if let Some(v) = a.get(ATTR_PERSON) {
        parts.push(format!("{v} person"));
    }
    if let Some(v) = a.get(ATTR_COMPARISON) {
        parts.push(v.to_string());
    }
    if let Some(v) = a.get(ATTR_PARTICIPLE) {
        parts.push(v.to_string());
    }
    if let Some(v) = a.get(ATTR_NEGATIVE) {
        parts.push(format!("negative={v}"));
    }
    if let Some(v) = a.get(ATTR_BASEFORM) {
        parts.push(format!("base: {v}"));
    }

    if parts.is_empty() {
        "no details".to_string()
    } else {
        parts.join(", ")
    }
}

// ---------------------------------------------------------------------------
// Help / usage
// ---------------------------------------------------------------------------

fn print_usage() {
    eprintln!("MCE CLI -- Morphological Computation Engine");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("    mce-cli <COMMAND> [ARGS]");
    eprintln!();
    eprintln!("COMMANDS:");
    eprintln!("    analyze <word>            Morphological analysis of a word");
    eprintln!("    spell <word>              Check spelling and suggest corrections");
    eprintln!("    compound <word>           Analyze compound word structure");
    eprintln!("    sentence <text>           Analyze and disambiguate a sentence");
    eprintln!("    grammar <text>            Check grammar of input text");
    eprintln!("    hyphenate <word>...       Hyphenate words");
    eprintln!("    hyphenate-text <text>     Hyphenate running text");
    eprintln!("    info                      Show dictionary info (symbol count, etc.)");
    eprintln!("    eval [OPTIONS]            Evaluate UPOS/Lemma accuracy against CoNLL-U");
    eprintln!("    benchmark [OPTIONS]       Run performance benchmarks");
    eprintln!();
    eprintln!("EVAL OPTIONS:");
    eprintln!("    --conllu <FILE>           Path to CoNLL-U file (required)");
    eprintln!("    --train <FILE>            CoNLL-U training file for corpus bigrams");
    eprintln!("    --format <table|json>     Output format (default: table)");
    eprintln!();
    eprintln!("BENCHMARK OPTIONS:");
    eprintln!("    --iterations <N>          Number of iterations (default: 1000)");
    eprintln!("    --warmup <W>              Warmup iterations (default: 100)");
    eprintln!();
    eprintln!("ENVIRONMENT:");
    eprintln!("    MCE_DICT_PATH        Directory containing mor.vfst (required for");
    eprintln!("                         analyze, spell, compound, sentence, grammar,");
    eprintln!("                         info, eval, benchmark)");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("    export MCE_DICT_PATH=~/oss/corevoikko/voikko-fi/vvfst");
    eprintln!("    mce-cli analyze koira");
    eprintln!("    mce-cli spell koirra");
    eprintln!("    mce-cli compound rautatieasema");
    eprintln!("    mce-cli sentence \"koira juoksee\"");
    eprintln!("    mce-cli grammar \"Koira koira juoksee pihalla.\"");
    eprintln!("    mce-cli hyphenate suomalainen rautatieasema kissanpentu");
    eprintln!("    mce-cli hyphenate-text \"Koira juoksee pihalla nopeasti.\"");
    eprintln!("    mce-cli info");
    eprintln!("    mce-cli eval --conllu fi_tdt-ud-dev.conllu");
    eprintln!("    mce-cli eval --conllu fi_tdt-ud-dev.conllu --train fi_tdt-ud-train.conllu");
    eprintln!("    mce-cli eval --conllu fi_tdt-ud-dev.conllu --format json");
    eprintln!("    mce-cli benchmark");
    eprintln!("    mce-cli benchmark --iterations 500 --warmup 50");
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    let command = args[1].as_str();

    match command {
        "analyze" => {
            if args.len() < 3 {
                eprintln!("error: 'analyze' requires a word argument.");
                eprintln!("usage: mce-cli analyze <word>");
                process::exit(1);
            }
            cmd_analyze(&args[2]);
        }
        "spell" => {
            if args.len() < 3 {
                eprintln!("error: 'spell' requires a word argument.");
                eprintln!("usage: mce-cli spell <word>");
                process::exit(1);
            }
            cmd_spell(&args[2]);
        }
        "compound" => {
            if args.len() < 3 {
                eprintln!("error: 'compound' requires a word argument.");
                eprintln!("usage: mce-cli compound <word>");
                process::exit(1);
            }
            cmd_compound(&args[2]);
        }
        "sentence" => {
            if args.len() < 3 {
                eprintln!("error: 'sentence' requires a text argument.");
                eprintln!("usage: mce-cli sentence \"text to analyze\"");
                process::exit(1);
            }
            // Join all remaining args in case the user didn't quote the text.
            let text = args[2..].join(" ");
            cmd_sentence(&text);
        }
        "grammar" => {
            if args.len() < 3 {
                eprintln!("error: 'grammar' requires a text argument.");
                eprintln!("usage: mce-cli grammar \"text to check\"");
                process::exit(1);
            }
            let text = args[2..].join(" ");
            cmd_grammar(&text);
        }
        "hyphenate" => {
            if args.len() < 3 {
                eprintln!("error: 'hyphenate' requires at least one word argument.");
                eprintln!("usage: mce-cli hyphenate <word>...");
                process::exit(1);
            }
            cmd_hyphenate(&args[2..]);
        }
        "hyphenate-text" => {
            if args.len() < 3 {
                eprintln!("error: 'hyphenate-text' requires a text argument.");
                eprintln!("usage: mce-cli hyphenate-text \"text to hyphenate\"");
                process::exit(1);
            }
            let text = args[2..].join(" ");
            cmd_hyphenate_text(&text);
        }
        "info" => {
            cmd_info();
        }
        "eval" => {
            cmd_eval(&args[2..]);
        }
        "benchmark" => {
            cmd_benchmark(&args[2..]);
        }
        "--help" | "-h" | "help" => {
            print_usage();
        }
        _ => {
            eprintln!("error: unknown command '{command}'.");
            eprintln!();
            print_usage();
            process::exit(1);
        }
    }
}
