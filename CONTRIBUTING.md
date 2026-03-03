# Contributing to MCE

Thanks for your interest in contributing to MCE!

## Development Setup

**Prerequisites:** Rust 1.86+, wasm-pack (for WASM builds)

```bash
git clone --recurse-submodules https://github.com/yongsk0066/mce.git
cd mce

# Run the full check suite
cargo fmt --all --check
cargo clippy --all-features -- -D warnings
cargo test --all-features

# Build WASM
wasm-pack build --target web --out-dir pkg crates/mce-wasm

# Run accuracy evaluation (requires data/ files)
MCE_DICT_PATH=data cargo run -p mce-cli --release -- eval \
  --conllu vendor/ud-finnish-tdt/fi_tdt-ud-dev.conllu \
  --model data/suffix_tagger.bin \
  --lemma-dict data/lemma_dict.tsv \
  --format json
```

## Pull Request Process

1. Create a branch from `main`
2. Make your changes
3. Ensure all checks pass: `cargo fmt`, `cargo clippy`, `cargo test`
4. Update `CHANGELOG.md` under `[Unreleased]` if applicable
5. Open a PR -- CI will run automatically and post a performance report

## Code Style

- Follow standard `rustfmt` formatting (edition 2024)
- All public items should have doc comments
- Prefer explicit error handling over `.unwrap()` in library code
- Tests go in the same file as the code they test (`#[cfg(test)]` modules)

## Project Structure

```
crates/
├── mce-core/       # Shared types, LOUDS Succinct Trie
├── mce-fst/        # FST engine (VFST format)
├── mce-tokenizer/  # Text tokenizer
├── mce-speller/    # Spell checking
├── mce-comonad/    # Writer Comonad + CG rules
├── mce-disambig/   # POS disambiguation (Viterbi + Suffix Tagger)
├── mce-fi/         # Finnish language module
├── mce-grammar/    # Grammar checking
├── mce-eval/       # Evaluation against UD treebanks
├── mce-wasm/       # WASM bindings
└── mce-cli/        # CLI tools
```

## Reporting Bugs

Use the [bug report template](https://github.com/yongsk0066/mce/issues/new?template=bug_report.yml).

## License

By contributing, you agree that your contributions will be licensed under the Apache-2.0 license.
