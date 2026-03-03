# mce-grammar

Finnish grammar checking engine with 21 modular rules.

## Purpose

This crate detects grammar errors in Finnish text by combining tokenization, morphological analysis, and disambiguation with a set of composable grammar rules. Each rule is an independent implementation of the `GrammarRule` trait, operating on annotated tokens with byte offsets for precise error location reporting.

## Key Types

- `GrammarChecker` trait — check raw text for grammar errors
- `GrammarRule` trait — individual rule operating on annotated tokens
- `GrammarError` — error with byte offsets, error code, message, and suggestions
- `AnnotatedToken` — surface form + position + optional morphological analysis
- `finnish::FinnishGrammarChecker` — full pipeline (tokenize -> analyze -> disambiguate -> check)

## Rules (21)

Includes: repeated word, capitalization, subject-verb agreement, double space, quotation marks, comma before conjunction, compound spacing, number agreement, negation agreement, double negation, missing/extra space around punctuation, partitive object, postposition case, comparative partitive, missing main verb, sentence-initial lowercase, excessive exclamation, comma in subordinate clause, possessive suffix.

## Dependencies

Uses: `mce-core`, `mce-fst`, `mce-fi`, `mce-tokenizer`, `mce-disambig`

Used by: `mce-wasm`, `mce-cli`
