# mce-grammar

Finnish grammar checking engine with 21 modular rules. Detects errors by combining tokenization, morphological analysis, and disambiguation with composable rule implementations operating on annotated tokens with byte offsets.

## Architecture

The grammar checker pipeline:

1. **Tokenize** text into word tokens (preserving byte offsets)
2. **Analyze** each word with `FinnishAnalyzer` (FST-based morphology)
3. **Disambiguate** using `ViterbiDisambiguator` (POS bigram model)
4. **Check** each `GrammarRule` against the annotated token stream
5. **Collect** all `GrammarError`s with byte offsets and suggestions

## Rules (21)

| Category | Rules |
|----------|-------|
| **Repetition** | `RepeatedWordRule` -- consecutive identical words |
| **Capitalization** | `CapitalizationRule` -- sentence-initial caps; `SentenceInitialLowercaseRule` -- lowercase after colon/semicolon |
| **Agreement** | `AgreementRule` -- subject-verb number; `SubjectVerbAgreementRule` -- pronoun-verb person; `NumberAgreementRule` -- numeral-noun case/number; `NegationAgreementRule` -- negation verb person |
| **Spacing** | `DoubleSpaceRule` -- multiple spaces; `MissingSpaceAfterPunctuationRule`; `ExtraSpaceBeforePunctuationRule`; `CompoundSpacingRule` -- split compounds |
| **Punctuation** | `QuotationMarkRule` -- unmatched quotes; `ExcessiveExclamationRule` -- `!!!`, `???` |
| **Comma** | `CommaBeforeConjunctionRule` -- subordinating conjunctions; `CommaInSubordinateRule` -- relative pronouns (joka, etc.) |
| **Case** | `PartitiveObjectRule` -- negated sentences require partitive; `PostpositionCaseRule` -- postpositions require genitive; `ComparativePartitiveRule` -- comparative + partitive without "kuin" |
| **Verb** | `MissingMainVerbRule` -- sentence without finite verb (heuristic); `DoubleNegationRule` -- non-standard double negation |
| **Morphology** | `PossessiveSuffixRule` -- redundant possessive pronoun + suffix |

## Usage

```rust
use mce_grammar::{GrammarChecker, GrammarError, GrammarRule, AnnotatedToken};
use mce_grammar::rules::RepeatedWordRule;

// Individual rule on annotated tokens
let rule = RepeatedWordRule::new();
let tokens = vec![
    AnnotatedToken::word("koira", 0, 5, None),
    AnnotatedToken::word("koira", 6, 11, None),
];
let errors = rule.check(&tokens);
assert_eq!(errors.len(), 1);
assert_eq!(errors[0].code, "REPEATED_WORD");
assert_eq!(errors[0].start, 6);
assert_eq!(errors[0].end, 11);
```

### Full Pipeline (with VFST dictionary)

```rust
use mce_grammar::finnish::FinnishGrammarChecker;
use mce_grammar::GrammarChecker;

let checker = FinnishGrammarChecker::new(&vfst_bytes).unwrap();
let errors = checker.check("Koira koira juoksee pihalla.");

for error in &errors {
    println!("[{}..{}] {} -- {}",
        error.start, error.end, error.code, error.message);
    if !error.suggestions.is_empty() {
        println!("  Suggestions: {}", error.suggestions.join(", "));
    }
}
```

## Key Types

| Type | Description |
|------|-------------|
| `GrammarChecker` trait | Check raw text for grammar errors |
| `GrammarRule` trait | Individual rule operating on `&[AnnotatedToken]` |
| `GrammarError` | Error with byte offsets, code, message, and suggestions |
| `AnnotatedToken` | Surface form + position + optional morphological analysis |
| `FinnishGrammarChecker` | Full pipeline: tokenize -> analyze -> disambiguate -> check |

## Implementing a Custom Rule

```rust
use mce_grammar::{GrammarRule, GrammarError, AnnotatedToken};

struct MyRule;

impl GrammarRule for MyRule {
    fn id(&self) -> &'static str { "MY_RULE" }

    fn check(&self, tokens: &[AnnotatedToken]) -> Vec<GrammarError> {
        let mut errors = Vec::new();
        for token in tokens {
            if token.is_word && token.text == "error" {
                errors.push(GrammarError::with_suggestions(
                    token.start, token.end,
                    "MY_RULE", "Example error detected",
                    vec!["correction".to_string()],
                ));
            }
        }
        errors
    }
}
```

## Dependencies

Uses: `mce-core`, `mce-fst`, `mce-fi`, `mce-tokenizer`, `mce-disambig`

Used by: `mce-wasm`, `mce-cli`
