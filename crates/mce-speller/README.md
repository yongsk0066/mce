# mce-speller

Spell checking and suggestion engine for MCE. Combines M1 Succinct Trie dictionary lookup with morphological validation, LRU caching, user dictionaries, and frequency-ranked suggestions. Adapted from corevoikko's speller and suggestion modules.

## Architecture

The spell checker operates in stages:

1. **Cache lookup** -- fast path for recently checked words (LRU, words <= 10 chars)
2. **User dictionary** -- custom word list always accepted
3. **Trie lookup** -- exact match in M1 Succinct Trie (LOUDS encoding)
4. **Morphological fallback** -- validates compounds and inflected forms via `MorphValidator`

Suggestions use Levenshtein fuzzy search on the trie, filtered by morphological validity and ranked by edit distance + frequency.

## Usage

```rust
use mce_speller::pipeline::{SpellCheckerBuilder, SpellChecker};
use mce_speller::SpellResult;
use mce_core::trie::TrieBuilder;

// Build a dictionary trie
let mut builder = TrieBuilder::new();
builder.insert(b"koira".to_vec());
builder.insert(b"kissa".to_vec());
builder.insert(b"talo".to_vec());
let trie = builder.build();

// Create spell checker with morphological validator
let mut checker = SpellCheckerBuilder::new()
    .trie(trie)
    .morph_validator(|word: &[char], len: usize| {
        // Your morphological validation logic
        len > 0 && word[0] == 'k'
    })
    .cache_size(0)
    .build();

// Check spelling
assert_eq!(checker.check("koira"), SpellResult::Ok);
assert_eq!(checker.check("xyzzy"), SpellResult::Failed);

// Generate suggestions (fuzzy search + morph filter)
let suggestions = checker.suggest("koirb", 1, 5);
assert!(suggestions.contains(&"koira".to_string()));
```

### User Dictionary

```rust
use mce_speller::user_dict::UserDictionary;

let ud = UserDictionary::from_words(["MCE", "WASM"]);
let mut checker = SpellCheckerBuilder::new()
    .trie(trie)
    .morph_validator(|_: &[char], _: usize| false)
    .user_dict(ud)
    .build();

assert_eq!(checker.check("MCE"), SpellResult::Ok);  // in user dict
```

### Frequency-Ranked Suggestions

```rust
use mce_core::frequency::FrequencyList;

// Attach frequency list for ranking common words higher
let fl = FrequencyList::from_conllu(conllu_text);
let checker = SpellCheckerBuilder::new()
    .trie(trie)
    .morph_validator(morph_fn)
    .frequency_list(fl)
    .build();

// Suggestions ranked by: edit distance (asc), then frequency (desc)
let ranked = checker.suggest_ranked("koirb", 1, 5, None::<fn(&str) -> f64>);
```

## Key Types

| Type | Description |
|------|-------------|
| `SpellResult` | `Ok`, `CapitalizeFirst`, `CapitalizationError`, `Failed` |
| `Speller` trait | Spell-check interface (`fn spell(&[char], usize) -> SpellResult`) |
| `SpellChecker<M>` | Full checker: trie + morph validator + cache + user dict + frequency |
| `SpellCheckerBuilder<M>` | Builder pattern for constructing `SpellChecker` |
| `SpellerCache` | Hash-based LRU cache for words <= 10 characters |
| `UserDictionary` | Custom word list (always accepted as correct) |
| `MorphValidator` trait | Morphological validation callback |

## Modules

| Module | Description |
|--------|-------------|
| `cache` | Hash-based spell result cache with configurable size |
| `pipeline` | `SpellChecker` and `SpellCheckerBuilder` with staged lookup |
| `status` | Spell status tracking utilities |
| `user_dict` | User dictionary management |

## Dependencies

Uses: `mce-core`, `mce-fst`

Used by: `mce-fi`, `mce-wasm`, `mce-cli`
