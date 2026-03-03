# mce-tokenizer

Language-agnostic text tokenizer with sentence boundary detection.

## Purpose

This crate splits raw text into tokens (words, whitespace, punctuation, URLs, emails) and detects sentence boundaries. It preserves byte offsets for downstream grammar checking and error reporting. Adapted from corevoikko's tokenizer.

## Key Types

- `tokenize()` — splits text into `(TokenType, &str)` pairs with byte offsets
- `detect_sentences()` — identifies sentence boundaries (probable/possible)
- `TokenType` — word, whitespace, punctuation, URL, email, unknown (re-exported from `mce-core`)
- `SentenceType` — none, probable, possible boundary markers (re-exported from `mce-core`)

## Dependencies

Uses: `mce-core`

Used by: `mce-grammar`, `mce-eval`, `mce-wasm`, `mce-cli`
