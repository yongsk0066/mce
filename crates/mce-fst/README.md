# mce-fst

FST engine for loading and traversing finite-state transducers in the VFST binary format. Supports flag diacritics, weighted/unweighted traversal, and symbol table management. Cherry-picked and adapted from corevoikko's `voikko-fst`.

## Architecture

The FST engine provides the core dictionary lookup mechanism for MCE. A VFST binary encodes Finnish morphological rules as state transitions. The traversal algorithms walk the transducer to produce all valid analyses for an input word.

## Key Types

```rust
use mce_fst::unweighted::UnweightedTransducer;
use mce_fst::weighted::WeightedTransducer;
use mce_fst::Transducer;

// Load an unweighted VFST transducer from bytes
let transducer = UnweightedTransducer::from_bytes(&vfst_data).unwrap();

// Prepare traversal for an input word
let mut config = transducer.new_config();
let input: Vec<char> = "koira".chars().collect();
transducer.prepare(&mut config, &input);

// Iterate over all output strings (analyses)
let mut output = String::new();
while transducer.next(&mut config, &mut output) {
    println!("Analysis: {output}");
    output.clear();
}
```

## Modules

| Module | Description |
|--------|-------------|
| `format` | VFST binary header parser (`VfstHeader`) |
| `symbols` | Character-to-index and index-to-character mapping (`SymbolTable`) |
| `transition` | Zero-copy transition struct with input/output symbol and target state |
| `flags` | Flag diacritic operations: P (positive set), C (clear), U (unification), R (require), D (disallow) |
| `config` | Traversal state stack for backtracking during FST walk |
| `unweighted` | Unweighted FST traversal (used for morphological analysis) |
| `weighted` | Weighted FST traversal (used for ranked suggestions) |

## VFST Format

The VFST binary format encodes:
- **Header**: magic number, type flag (weighted/unweighted), symbol count, transition count
- **Symbol table**: maps integer indices to character strings (including multi-character symbols like `[BOUNDARY]`)
- **Transition table**: packed array of (input, output, target, flag) tuples
- **Flag diacritics**: encoded as special symbols controlling feature unification during traversal

## Flag Diacritics

Flag diacritics enforce long-distance morphological constraints without expanding the state space:

| Operation | Meaning |
|-----------|---------|
| `@P.feat.val@` | Set feature to value (positive) |
| `@C.feat@` | Clear feature |
| `@U.feat.val@` | Unify: set if unset, fail if set to different value |
| `@R.feat.val@` | Require feature to have value |
| `@D.feat.val@` | Disallow feature value |

## Error Handling

```rust
use mce_fst::VfstError;

// VfstError variants:
// - InvalidMagic: bad file header
// - TooShort: truncated file
// - TypeMismatch: expected weighted but got unweighted (or vice versa)
// - InvalidSymbolTable: corrupt symbol data
// - InvalidFlagDiacritic: malformed flag string
// - AlignmentError: transition table misaligned
```

## Dependencies

Uses: `mce-core`, `thiserror`, `bytemuck`, `hashbrown`

Used by: `mce-fi`, `mce-speller`, `mce-grammar`, `mce-eval`, `mce-wasm`, `mce-cli`
