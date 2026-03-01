//! MCE CLI — interactive testing tool for the Morphological Computation Engine.
//!
//! Provides subcommands for morphological analysis, spell-checking, compound
//! word analysis, sentence-level disambiguation, grammar checking, and
//! hyphenation using the VFST dictionary.
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
//! ```

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

use mce_core::analysis::{
    Analysis, ATTR_BASEFORM, ATTR_CLASS, ATTR_COMPARISON, ATTR_MOOD, ATTR_NEGATIVE, ATTR_NUMBER,
    ATTR_PARTICIPLE, ATTR_PERSON, ATTR_SIJAMUOTO, ATTR_STRUCTURE, ATTR_TENSE, ATTR_WORDBASES,
};
use mce_core::token::TokenType;
use mce_disambig::{Disambiguator, ViterbiDisambiguator};
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
    eprintln!();
    eprintln!("ENVIRONMENT:");
    eprintln!("    MCE_DICT_PATH        Directory containing mor.vfst (required for");
    eprintln!("                         analyze, spell, compound, sentence, grammar, info)");
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
