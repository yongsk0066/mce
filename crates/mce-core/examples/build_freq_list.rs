//! Build a word frequency list from UD Finnish-TDT training data.
//!
//! Usage:
//!   cargo run -p mce-core --example build_freq_list -- <path-to-fi_tdt-ud-train.conllu>
//!
//! Example:
//!   cargo run -p mce-core --example build_freq_list -- \
//!     vendor/ud-finnish-tdt/fi_tdt-ud-train.conllu

use mce_core::frequency::FrequencyList;
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path-to-conllu-file>", args[0]);
        eprintln!(
            "Example: {} vendor/ud-finnish-tdt/fi_tdt-ud-train.conllu",
            args[0]
        );
        std::process::exit(1);
    }

    let path = &args[1];
    eprintln!("Reading CoNLL-U file: {path}");

    let content = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Error reading file: {e}");
        std::process::exit(1);
    });

    eprintln!("Parsing word frequencies...");
    let freq_list = FrequencyList::from_conllu(&content);

    eprintln!(
        "Done: {} unique word forms, {} total tokens",
        freq_list.len(),
        freq_list.total()
    );

    // Print top 50 most common words.
    println!("\n=== Top 50 Most Common Finnish Word Forms ===\n");
    println!(
        "{:>4}  {:>8}  {:>10}  {}",
        "Rank", "Count", "Rel.Freq", "Word"
    );
    println!("{}", "-".repeat(42));

    for (i, (word, count)) in freq_list.top_n(50).iter().enumerate() {
        let rel = freq_list.relative_frequency(word);
        println!("{:>4}  {:>8}  {:>10.6}  {}", i + 1, count, rel, word);
    }

    // Verify common Finnish words are present.
    println!("\n=== Verification: Common Finnish Words ===\n");
    let expected = [
        "ja", "on", "ei", "se", "oli", "että", "kun", "myös", "niin", "tai",
    ];
    for word in &expected {
        let freq = freq_list.frequency(word);
        let rank = freq_list.rank(word);
        let rank_str = rank
            .map(|r| format!("#{r}"))
            .unwrap_or_else(|| "NOT FOUND".to_string());
        println!("  {word:<10} freq={freq:<6} rank={rank_str}");
    }

    // Serialize and report size.
    let bytes = freq_list.to_bytes();
    eprintln!(
        "\nSerialized size: {} bytes ({:.1} KB)",
        bytes.len(),
        bytes.len() as f64 / 1024.0
    );

    // Verify roundtrip.
    let restored = FrequencyList::from_bytes(&bytes).expect("roundtrip failed");
    assert_eq!(restored.len(), freq_list.len());
    assert_eq!(restored.total(), freq_list.total());
    eprintln!("Serialization roundtrip: OK");
}
