# mce-comonad

Writer Comonad + CG-lite for Finnish morphophonology and disambiguation.

## Purpose

Finnish morphophonological rules -- consonant gradation, vowel harmony, possessive vowel copying -- all share a computational pattern: each rule inspects **local context** around a focus position and produces a transformed output. The comonad abstraction captures this pattern precisely. Each rule becomes a **coKleisli arrow** (`&Zipper<A> -> B`), and the `extend` combinator lifts it into a global transformation. The comonad laws (identity, associativity) guarantee that composed rules behave predictably.

The same abstraction applies to CG disambiguation: each CG rule is a coKleisli arrow over sentence-level reading sets, filtering unlikely analyses based on neighboring context.

## Writer Comonad

The core innovation is using a **Writer Comonad** to handle character deletions algebraically. Finnish consonant gradation deletes characters (e.g., `kaappi` -> `kaapi`, `puku` -> `puu`), which traditionally requires sentinel characters (`'\0'`) that break pure composition. The Writer Comonad solves this:

- **`DeletionSet`** (the monoid): tracks which positions to delete via set union
- **`WriterZipper<W, A>`**: pairs a `Zipper<A>` with a monoid accumulator
- **`extend`**: applies a coKleisli arrow at every position, combining monoid contributions
- **`materialize`**: applies all accumulated deletions once at the end

This restores pure coKleisli composition -- each arrow operates on the full character sequence with correct positions, and deletions accumulate algebraically rather than destructively.

### Morphophonological rules

11 consonant gradation patterns are implemented as coKleisli arrows:

| Pattern | Strong | Weak | Example |
|---------|--------|------|---------|
| Geminate | pp, tt, kk | p, t, k | `kaappi` -> `kaapi` |
| Cluster | mp, nt, nk, lt, rt | mm, nn, ng, ll, rr | `kampa` -> `kamma` |
| Qualitative | p, t, k | v, d, (deleted) | `tapa` -> `tava` |

Vowel harmony resolves archiphonemes (`A` -> `a`/`ä`, `O` -> `o`/`ö`, `U` -> `u`/`y`) by scanning left context for the nearest back or front vowel. Possessive vowel copying resolves the `V` archiphoneme by copying the preceding vowel.

## CG-lite

Constraint Grammar disambiguation removes unlikely morphological readings at each sentence position based on context. The CG engine uses the same coKleisli architecture:

- **85 rules** across **24 phases**, targeting top UPOS confusions
- **24 rule types**: `RemoveIfPreceded`, `SelectIfFollowed`, `SelectIfSandwiched`, `RemoveByBaseformList`, `SelectAtSentenceStart`, and 19 others
- **Safety invariant**: a rule never removes the last reading at any position

Rule types include context-based (preceded/followed), baseform-based, attribute-based, multi-context (sandwiched), and positional (sentence start) patterns.

## Example

```rust
use mce_comonad::writer::{WriterZipper, DeletionSet, gradation_writer, harmony_writer};
use mce_comonad::finnish::Grade;
use mce_comonad::Zipper;

// Compose gradation + harmony via extend chaining
let zipper = Zipper::new("pukussA".chars().collect()).unwrap();
let writer = WriterZipper::<DeletionSet, char>::new(zipper);

let result = writer
    .extend(|wz| gradation_writer(wz, Grade::Weak))  // puku -> puu (k deleted)
    .extend(harmony_writer);                           // A -> a (back harmony)

assert_eq!(result.materialize_string(), "puussa");
```

```rust
use mce_comonad::cg::{apply_cg_rules, finnish_disambiguation_rules};

// Apply 85 CG rules over a sentence of reading sets
let rules = finnish_disambiguation_rules();
let disambiguated = apply_cg_rules(&sentence, &rules);
```

## Key Types

| Type | Module | Role |
|------|--------|------|
| `Zipper<T>` | `zipper` | List zipper comonad with `extract`, `extend`, `peek_left`/`peek_right` |
| `WriterZipper<W, A>` | `writer` | Writer Comonad pairing a zipper with a monoid accumulator |
| `DeletionSet` | `writer` | Monoid tracking positions to delete (set union) |
| `Monoid` | `writer` | Trait for associative binary operation with identity |
| `Grade` | `finnish` | Strong/Weak consonant gradation selector |
| `GradationPattern` | `finnish` | Two-character window pattern for gradation matching |
| `CgRule` | `cg` | Trait for CG disambiguation rules (coKleisli arrows) |
| `ReadingSet` | `cg` | `Vec<Analysis>` -- candidate analyses at one sentence position |

## Source Files

| File | LOC | Content |
|------|-----|---------|
| `writer.rs` | ~980 | Writer Comonad, DeletionSet, morphophonological pipeline |
| `cg.rs` | ~2000+ | 24 CG rule types, 85 Finnish rules, `apply_cg_rules` |
| `finnish.rs` | -- | 11 gradation patterns, vowel harmony, possessive copying |
| `zipper.rs` | ~240 | List zipper comonad with comonad law tests |
| `bench.rs` | -- | Per-rule latency benchmarking (coKleisli + CG) |

## Dependencies

**Uses**: `mce-core` (Analysis, attribute constants)

**Used by**: `mce-fi`, `mce-disambig`, `mce-eval`, `mce-wasm`, `mce-cli`
