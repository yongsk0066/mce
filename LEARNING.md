# Learning Guide

This document helps newcomers build the background knowledge needed to work on MCE
(Morphological Computation Engine). Read this before diving into the code.

The topics are ordered so that each section motivates the next: first the problem,
then the mathematical solution, then the implementation details.


## 1. Agglutinative Languages and Finnish Morphology

Before looking at any code, understand the problem this project solves.

### What you need to know

Most English spell checkers use a word list: look up the word, and if it is in the
list, it is correct. This works because English has relatively few forms per word
(run, runs, running, ran -- maybe 4-5 forms).

Finnish is an **agglutinative language**. Words are built by chaining a root with
suffixes -- case markers, plural markers, possessive suffixes, clitics -- in sequence.
A single noun like "talo" (house) has over 2,000 valid inflected forms. Verbs have
even more. Compound words multiply this further: "lentokonesuihkuturbiinimoottori" =
jet turbine engine, and each part inflects independently.

Finnish has **15 grammatical cases**. A noun like "talo" appears as: talo, talon,
taloa, talossa, talosta, taloon, talolla, talolta, talolle, talona, taloksi, talotta,
taloineen... Each form conveys a different grammatical meaning (location, direction,
possession, etc.). Suffixes can also stack: "talossanikinko" = talo (house) + ssa (in)
+ ni (my) + kin (also) + ko (question) -- "in my house too?"

Other agglutinative languages (Turkish, Hungarian, Estonian, Korean) face the same
challenge. The techniques in MCE apply broadly, though the specific rules are Finnish.

### Why it matters

A word list approach would need tens of millions of entries and still miss valid
compound words. Instead, MCE uses **morphological analysis**: a word is correct if it
can be decomposed into a valid root + valid suffixes according to Finnish grammar rules.
Every feature builds on this insight -- spell checking, lemmatization, POS tagging, and
grammar checking all depend on correctly decomposing words into morphemes.

### How to learn

```
Explain Finnish noun inflection. Finnish has 15 grammatical cases -- list them with
their Finnish names (nimento, omanto, osanto, etc.), their linguistic names (nominative,
genitive, partitive, etc.), and what each one expresses. Use "talo" (house) as the
example and show each inflected form in singular.
```

```
What is an agglutinative language? How does it differ from isolating languages (like
English or Chinese) and fusional languages (like Russian or Latin)? Why do agglutinative
languages make dictionary-based spell checking impractical?
```

**Recommended reading:**
- Wikipedia: [Agglutinative language](https://en.wikipedia.org/wiki/Agglutinative_language)
- Wikipedia: [Finnish grammar](https://en.wikipedia.org/wiki/Finnish_grammar)
- Pirinen 2019, "Building Finnish NLP resources -- neural vs. rule-based" (in `papers/`)


## 2. Finite State Transducers (FST/VFST)

This is the data structure that makes morphological analysis fast enough for real-time
spell checking.

### What you need to know

A **Finite State Automaton (FSA)** is a directed graph that recognizes strings. You start
at an initial state, follow edges labeled with characters, and check if you end at an
accepting state. This is the foundation of regular expressions.

A **Finite State Transducer (FST)** extends the FSA by adding an *output* label to each
edge alongside the input label. When you traverse the graph, you collect output symbols.
So an FST does not just say "yes, this string is valid" -- it also produces a result
describing *how* it is valid:

```
Input:  k-i-s-s-o-j-a
Output: [Ln][Xp]kissa[X]kissoja[Spar][Nm]
```

This output tells us: it is a noun (`Ln`), the base form is "kissa" (`[Xp]kissa[X]`),
in partitive case (`Spar`), plural (`Nm`).

MCE uses **VFST** (Voikko FST), a compact binary format. A VFST file contains a 16-byte
header, a symbol table mapping indices to UTF-8 strings, and a transition table. The
entire Finnish lexicon fits in roughly 3.8MB because shared prefixes and suffixes share
graph nodes.

**Flag diacritics** are control symbols on edges that constrain which paths are valid.
Five operations (P/C/U/R/D) enforce constraints like "this path is only valid if
CASE=NOM was set earlier," preventing impossible suffix combinations.

### How to learn

```
Explain finite state automata and finite state transducers in simple terms. How does
an FST map input strings to output strings? Give a small example showing state
transitions step by step.
```

```
What are flag diacritics in finite state transducers? Explain the five operations:
P (positive set), C (clear), U (unification), R (require), D (disallow).
```

**Recommended reading:**
- Beesley & Karttunen, *Finite State Morphology* (2003)
- The [foma documentation](https://fomafst.github.io/)

**Key files:** `crates/mce-fst/src/` -- VFST format, symbols, traversal


## 3. Morphophonology: Consonant Gradation, Vowel Harmony

When suffixes attach to stems in Finnish, the sounds at the boundary change
systematically. These changes are the core problem MCE's comonadic engine solves.

### What you need to know

Three main morphophonological processes:

**Consonant gradation (astevaihtelu):** Stem-final consonants alternate between
"strong" and "weak" grades depending on syllable structure. Eleven patterns exist:

| Strong | Weak | Example |
|--------|------|---------|
| pp | p | kaappi -> kaapi |
| tt | t | matto -> mato |
| kk | k | kukka -> kuka |
| p | v | tapa -> tava |
| t | d | katu -> kadu |
| k | (deleted) | puku -> puu |
| mp | mm | kampa -> kamma |
| nt | nn | ranta -> ranna |
| nk | ng | kenka -> kenga |
| lt | ll | kulta -> kulla |
| rt | rr | parta -> parra |

**Vowel harmony (vokaalisointu):** Suffix vowels must agree with stem vowels.
Finnish has back vowels (a, o, u), front vowels (a, o, y), and neutral vowels (e, i).
Archiphonemic characters (A, O, U) in morphological representations resolve based on
the last non-neutral vowel to the left: talo + ssA -> talossa (back), poyda + ssA ->
poydassa (front).

**Possessive vowel copying:** The archiphoneme V copies the immediately preceding
vowel: talo + Vn -> taloon.

### Why it matters

In traditional FST systems (Omorfi), these three processes are handled by pre-compiled
**continuation classes** -- every combination of gradation pattern, harmony class, and
possessive variant needs a separate class. Omorfi manages 874 such classes. Adding a
new rule means modifying hundreds of combinations. MCE uses comonads to reduce this to
13 independent arrows (a 67:1 compression).

### How to learn

```
Explain Finnish consonant gradation (astevaihtelu). What triggers the alternation
between strong and weak grades? Give the 11 gradation patterns with examples.
```

```
What is vowel harmony in Finnish? How do back vowels (a, o, u), front vowels
(a, o, y), and neutral vowels (e, i) interact? How are archiphonemes resolved?
```

**Key files:** `crates/mce-comonad/src/finnish.rs` -- all rules as coKleisli arrows


## 4. Comonads: The Mathematical Framework

This is the central mathematical idea of the MCE project. Where monads abstract over
*effects*, comonads abstract over *context* -- exactly what morphophonological rules need.

### What you need to know

A **comonad** is a structure from category theory with two operations:

- **`extract`**: Get the focused element from a context. Like asking "what is the
  current character?"
- **`extend`**: Given a local rule that reads context and produces a value, apply it at
  every position simultaneously. Like cellular automata: each cell inspects its neighbors
  and updates.

A **coKleisli arrow** is a function `W A -> B` that reads from a comonadic context and
produces a value. In MCE, each morphophonological rule is a coKleisli arrow:
`&Zipper<char> -> char`. It inspects the focused character plus its left/right neighbors,
and returns the transformed character.

**coKleisli composition** (`>=>`) chains arrows sequentially:

```
pipe = consonant_gradation >=> vowel_harmony >=> possessive_copying
```

This composition is **associative** -- grouping does not matter -- and `extract` is the
identity. These are the three comonad laws that guarantee correctness.

**The Zipper** (list zipper) is MCE's primary comonad. It represents a sequence with a
focused element and bidirectional context:

```
left context   focus   right context
[k, a, a]       p      [p, i]
```

**The Writer Comonad** solves the deletion problem. Consonant gradation pattern 6
(puku -> puu) *deletes* a character. But a coKleisli arrow `Zipper<char> -> char` must
return exactly one character per position. The Writer Comonad pairs the Zipper with a
`DeletionSet` (a monoid under set union). Instead of deleting immediately, arrows mark
positions for deletion: `(DeletionSet::singleton(pos), original_char)`. After all rules
run, `materialize()` applies all deletions at once. This preserves pure coKleisli
composition -- no intermediate string reconstruction needed.

**Result:** 13 coKleisli arrows replace 874 FST continuation classes (67:1 compression).

### How to learn

```
What is a comonad? Compare it to a monad. While a monad wraps a value with effects
(Maybe, IO, List), a comonad wraps a value with context. Explain extract and extend
with examples. What are the comonad laws?
```

```
What is a list zipper? How does it provide O(1) access to a focused element and its
neighbors? How does extend apply a local function at every position?
```

```
What is the Writer comonad? How does pairing a comonad with a monoid accumulator
allow side-information (like deletion markers) without breaking coKleisli composition?
```

**Recommended reading:**
- Uustalu & Vene (2005), "The Essence of Dataflow Programming"
- Capobianco & Uustalu (2010), "A Categorical Outlook on Cellular Automata"
- Orchard (2012), "Should I use a Monad or a Comonad?"

**Key files:**
- `crates/mce-comonad/src/zipper.rs` -- Zipper (extract, extend)
- `crates/mce-comonad/src/writer.rs` -- Writer Comonad (DeletionSet, materialize)
- `crates/mce-comonad/src/finnish.rs` -- coKleisli arrows for Finnish


## 5. Constraint Grammar (CG) for Disambiguation

After the FST produces morphological analyses, many words are ambiguous -- a word may
have 2-10 valid readings. CG rules narrow them down.

### What you need to know

**Constraint Grammar** (CG) is a rule-based framework for disambiguating morphological
analyses. Each rule inspects the sentence context around an ambiguous word and removes
unlikely readings or selects likely ones.

In MCE, CG rules are expressed as **coKleisli arrows** over `Zipper<ReadingSet>`, where
`ReadingSet = Vec<Analysis>` is the set of candidate analyses at one sentence position.
This is the same comonadic abstraction used for morphophonology, operating at the
sentence level instead of the character level:

| Level | Zipper contents | coKleisli arrow | Purpose |
|-------|----------------|-----------------|---------|
| Character | `Zipper<char>` | gradation/harmony | Letter transformation |
| Sentence | `Zipper<ReadingSet>` | CG rules | Disambiguation |

MCE implements **CG-lite** -- a simplified CG with 62 active rules (85 total) targeting
the most common UPOS confusions: ADJ/NOUN, ADV/NOUN, NOUN/PROPN, NOUN/VERB,
PRON/NOUN, ADP/ADV, VERB/AUX, and more.

Rule types include:
- `RemoveIfPreceded`: remove a reading if the previous word has a certain class
- `SelectIfFollowed`: keep only readings matching a class if the next word matches
- `RemoveIfSandwiched`: remove if both neighbors match conditions
- `SelectByBaseform`: select by dictionary form

**Safety invariant:** A rule must never remove the last reading at any position. All
implementations enforce this.

### How to learn

```
What is Constraint Grammar (CG)? How does it differ from statistical disambiguation?
Explain the basic operations REMOVE and SELECT with examples. What is the safety
invariant that prevents removing all readings?
```

**Recommended reading:**
- Karlsson (1990), "Constraint Grammar as a Framework for Parsing Running Text"
- Bick & Didriksen (2015), "CG-3 -- Beyond Classical Constraint Grammar"

**Key file:** `crates/mce-comonad/src/cg.rs` -- CG-lite rules as coKleisli arrows


## 6. Statistical POS Tagging (Suffix Tagger)

CG rules are high-precision but limited in coverage. The suffix tagger adds a
statistical layer that pushes UPOS accuracy from 82.71% (rule-only) to 95.56%.

### What you need to know

The **suffix tagger** is a sparse logistic regression classifier that predicts UPOS
tags from surface form features. It operates in two phases:

1. **Feature extraction**: For each word, extract ~20-30 sparse features -- character
   suffixes (1-8 chars), prefixes (1-5 chars), word shape (capitalization, digits,
   hyphens), and context suffixes from neighboring words.

2. **Sparse dot product + softmax**: Compute class logits via sparse matrix-vector
   multiply against quantized (i8) weights, then log-softmax for per-tag probabilities.

The tagger does **not** replace FST analysis. Instead it provides emission
log-probabilities that re-rank FST-generated candidates. The full pipeline is:

```
FST analysis -> CG-lite rules -> Suffix tagger scoring -> Viterbi decoding
```

The model is trained offline (Python/sklearn) and stored as a compact binary file
(~5MB). The binary format uses quantized int8 weights for minimal memory footprint.

### Why it matters

Pure rule-based disambiguation (CG-lite alone) achieves 82.71% UPOS -- comparable to
Omorfi's 83.88% but with 67x fewer rules. Adding the suffix tagger jumps to 95.56%,
within 1.4 percentage points of neural systems like TurkuNLP (96.91%). This
demonstrates that lightweight statistical methods can effectively complement rule-based
systems in a browser-deployable package.

### How to learn

```
What is logistic regression for text classification? How do suffix features help
predict part-of-speech tags? Why are suffix features particularly effective for
morphologically rich languages?
```

```
What is the Viterbi algorithm? How does it find the globally optimal tag sequence
given per-word emission probabilities and bigram transition probabilities?
```

**Key files:**
- `crates/mce-disambig/src/suffix_tagger.rs` -- feature extraction and model loading
- `crates/mce-disambig/src/viterbi.rs` -- Viterbi sequence decoding
- `crates/mce-disambig/src/lattice.rs` -- weighted lattice construction


## 7. Tensor Train Decomposition and Bond Rank

This is the mathematical framework behind MCE's typological analysis tool, which
measures how morphologically complex a language is.

### What you need to know

A **morphological paradigm** is the table of all inflected forms of a word. For Finnish
nouns: 15 cases x 2 numbers = 30 cells. This table can be encoded as a
multi-dimensional array (tensor) where axes correspond to grammatical features (case,
number) and character positions.

**Tensor Train (TT) decomposition** factorizes this large tensor into a chain of small
"core" tensors:

```
[large tensor]  =  [core1] --r1-- [core2] --r2-- [core3]
  case x num x pos    case          number         position
```

The sizes r1, r2 of the connections between cores are called **bond ranks**. Bond rank
measures the number of independent interaction patterns between grammatical features.

The **bond rank profile** (r1, r2, ...) serves as a **morphological fingerprint** for
a language:

- **High bond rank** = many distinct inflected forms = agglutinative languages
  (Finnish: Bond 1 = 8.32)
- **Low bond rank** = many forms overlap (syncretism) = fusional languages
  (German: Bond 1 = 2.14)

MCE's experiments across 12 languages (4 language families + 1 isolate) show that bond
rank profiles separate agglutinative from fusional languages with high statistical
significance (Kruskal-Wallis H=270.69, p < 10^-60), and correlate strongly with
syncretism rates (Spearman rho=-0.743).

A key finding: Russian verbal Bond 2 = 3 (not the expected 4) because the past tense
uses gender instead of person agreement, collapsing one dimension. Finnish verbal
Bond 2 = 5 matches its 4 moods x 2 tenses structure exactly. These results demonstrate
that bond rank captures genuine linguistic structure, not statistical artifacts.

### How to learn

```
What is Tensor Train decomposition? Explain it as factorizing a large multi-dimensional
array into a chain of smaller matrices. What does "bond rank" measure?
```

```
What is syncretism in morphology? How does it relate to the distinction between
agglutinative and fusional languages? Give examples from Finnish and Russian.
```

**Recommended reading:**
- Oseledets (2011), "Tensor-Train Decomposition"
- Ackerman & Malouf (2013), "Morphological Organization"

**Key files:** The TT analysis is implemented in Python (NumPy) and documented in
`mce-research/papers/paper-2-tt-rank/`


## 8. WebAssembly Deployment

MCE compiles to WebAssembly so it runs in the browser with zero server dependencies.

### What you need to know

**WebAssembly (WASM)** is a binary instruction format that runs in web browsers at
near-native speed. Rust compiles to WASM via the `wasm32-unknown-unknown` target.

**wasm-bindgen** generates JavaScript glue code that handles type conversion between
Rust and JavaScript. The `#[wasm_bindgen]` attribute marks functions and types for
export. For complex return types, MCE uses **serde-wasm-bindgen** to serialize Rust
structs to JS objects.

MCE's WASM binary is **225KB** after optimization. Combined with the FST dictionary
and suffix tagger model, the total deployment size is approximately **9.1MB** (the
model compresses to 1.03MB with gzip). The WASM module exposes 20 API methods
including `load_model`, `has_model`, and `analyze_sentence`.

### Why it matters

Most NLP tools for morphologically rich languages require a server (GPU for neural
models, or HFST/foma for rule-based systems). MCE runs entirely in the browser,
enabling offline spell checking, morphological analysis, and grammar checking from a
CDN-served package. No server, no installation, no Python dependencies.

### How to learn

```
Explain how wasm-bindgen works in Rust. How does the #[wasm_bindgen] attribute
transform Rust functions for JavaScript consumption? What happens to Rust types
like String and Vec when they cross the WASM boundary?
```

```
What is serde-wasm-bindgen and when would you use it instead of plain wasm-bindgen?
Compare the two approaches for returning a Vec<MyStruct> from Rust to JavaScript.
```

**Key file:** `crates/mce-wasm/src/lib.rs` -- WASM bindings and exported API


## 9. The MCE Pipeline: How All 4 Machines Work Together

MCE is not a single algorithm but an orchestra of four specialized machines, each
handling a different computational task.

### What you need to know

The MCE v3 architecture consists of four machines:

**M1: Succinct Trie** -- Dictionary lookup and spell checking. Uses LOUDS (Level-Order
Unary Degree Sequence) encoding for compact storage. Given a word, M1 answers: "Is this
word in the dictionary?" and provides the starting point for morphological analysis.
Crate: `mce-core` (trie module).

**M2': Comonadic Engine** -- Morphophonological transformation and morphological
analysis. Implements Finnish consonant gradation, vowel harmony, and possessive copying
as composable coKleisli arrows. Also handles CG-lite disambiguation at the sentence
level. This is the mathematical heart of MCE. Crate: `mce-comonad`.

**M3: Pushdown Transducer (PDT)** -- Compound word analysis. Finnish freely forms
compound words ("talonrakentaja" = house builder). A PDT can handle the recursive
structure of compounds that a flat FST cannot. Crate: `mce-fst`.

**M4': Weighted Lattice** -- Disambiguation and sequence optimization. Combines CG-lite
rule output with suffix tagger emission scores and bigram transition probabilities.
Uses Viterbi decoding to find the globally optimal tag sequence. Crate: `mce-disambig`.

### The data flow

```
Input text
    |
    v
[Tokenizer] -- split into words and sentences
    |
    v
[M1: Trie] -- dictionary lookup per word
    |
    v
[M3: PDT/FST] -- morphological analysis, compound decomposition
    |
    v
[M2': Comonad] -- morphophonological pipeline (gradation, harmony, possessive)
    |                + CG-lite rules (sentence-level disambiguation)
    v
[M4': Lattice] -- suffix tagger scoring + Viterbi decoding
    |
    v
Output: disambiguated analyses (lemma, UPOS, features) per token
```

### Performance

- **Speed:** 42,090 tokens/second (24 microseconds/token)
- **UPOS accuracy:** 95.56% (CG + suffix tagger), 82.71% (rule-only)
- **Lemma accuracy:** 86.24%
- **Coverage:** 99.64% of tokens receive at least one analysis
- **WASM binary:** 225KB
- **Total deployment:** ~9.1MB (gzip: model 1.03MB)

### How to learn

```
What is a pushdown transducer and why is it more powerful than a finite state
transducer? How does the extra stack help with recursive structures like compound words?
```

```
How does Viterbi decoding combine local emission scores with global sequence
constraints? Walk through a small example with 3 words and 4 possible tags each.
```

**Key files:**
- `crates/mce-core/` -- shared types, character classification, trie
- `crates/mce-comonad/` -- comonadic engine (zipper, writer, Finnish rules, CG)
- `crates/mce-fst/` -- FST engine (VFST format, traversal)
- `crates/mce-disambig/` -- disambiguation (suffix tagger, Viterbi, lattice)
- `crates/mce-fi/` -- Finnish language module (tag parsing, morphology)
- `crates/mce-grammar/` -- grammar checking rules (21 rules)
- `crates/mce-tokenizer/` -- text tokenization
- `crates/mce-wasm/` -- WASM bindings
- `crates/mce-eval/` -- evaluation against UD treebanks
- `crates/mce-cli/` -- command-line interface


## Learning Path

Follow this order. Stop when you have enough context for your task.

1. **The problem** -- why word lists fail for Finnish (Section 1)
2. **FST** -- the data structure that analyzes morphology (Section 2)
3. **Morphophonology** -- the sound changes that make Finnish hard (Section 3)
4. **Comonads** -- how MCE solves it mathematically (Section 4)
5. **Read the code** -- `mce-comonad/src/` then `mce-fst/src/`
6. **CG** -- if working on disambiguation rules (Section 5)
7. **Suffix tagger** -- if working on statistical scoring (Section 6)
8. **Tensor Train** -- if working on typological analysis (Section 7)
9. **WASM** -- if working on browser deployment (Section 8)
10. **Pipeline** -- the full picture of how machines cooperate (Section 9)

Most contributors need Sections 1-4 and then the code. The rest is on-demand.
