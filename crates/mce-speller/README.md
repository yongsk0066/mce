# mce-speller

Spell checking and suggestion engine for MCE.

## Purpose

This crate provides the spell-checking pipeline: checking whether a word is correctly spelled, generating correction suggestions, caching results, and managing user dictionaries. Adapted from corevoikko's speller and suggestion modules.

## Key Types

- `Speller` trait — spell-check interface returning `SpellResult`
- `SpellResult` — result enum: `Ok`, `CapitalizeFirst`, `CapitalizationError`, `Failed`
- `cache` — LRU cache for spell-check results
- `pipeline` — suggestion generation pipeline
- `status` — spell status tracking
- `user_dict` — user dictionary management

## Dependencies

Uses: `mce-core`, `mce-fst`

Used by: `mce-fi`, `mce-wasm`, `mce-cli`
