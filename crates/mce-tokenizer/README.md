# mce-tokenizer

Language-agnostic text tokenizer with sentence boundary detection. Splits raw text into typed tokens while preserving byte offsets for downstream error reporting. Adapted from corevoikko's tokenizer.

## Features

- **Word tokenization**: splits text into word, whitespace, punctuation, and unknown tokens
- **URL/email detection**: recognizes URLs (`http://`, `https://`, `ftp://`) and email addresses as single tokens
- **Sentence boundary detection**: identifies probable and possible sentence boundaries using punctuation heuristics
- **Byte offset preservation**: every token carries its position in the original text

## Usage

```rust
use mce_tokenizer::next_token;
use mce_core::token::TokenType;

let text = "Koira juoksee nopeasti.";
let chars: Vec<char> = text.chars().collect();
let text_len = chars.len();
let mut pos = 0;

while pos < text_len {
    let (token_type, token_len) = next_token(&chars, text_len, pos);
    if token_len == 0 {
        break;
    }

    let token: String = chars[pos..pos + token_len].iter().collect();
    match token_type {
        TokenType::Word => println!("WORD: {token}"),
        TokenType::Whitespace => {} // skip
        TokenType::Punctuation => println!("PUNCT: {token}"),
        _ => println!("OTHER: {token}"),
    }

    pos += token_len;
}
// WORD: Koira
// WORD: juoksee
// WORD: nopeasti
// PUNCT: .
```

### Sentence Detection

```rust
use mce_tokenizer::detect_sentences;
use mce_core::token::SentenceType;

let text = "Koira juoksee. Kissa nukkuu.";
let chars: Vec<char> = text.chars().collect();
let boundaries = detect_sentences(&chars, chars.len(), None);

for (pos, sentence_type) in &boundaries {
    match sentence_type {
        SentenceType::Probable => println!("Sentence boundary at char {pos}"),
        SentenceType::Possible => println!("Possible boundary at char {pos}"),
        SentenceType::None => {}
    }
}
```

## Token Types

| Type | Examples |
|------|---------|
| `Word` | `koira`, `juoksee`, `123` |
| `Whitespace` | spaces, tabs, newlines |
| `Punctuation` | `.`, `,`, `!`, `?`, `:`, `;` |
| `Url` | `https://example.com` |
| `Email` | `user@example.com` |
| `Unknown` | characters not matching other categories |

## Dependencies

Uses: `mce-core`

Used by: `mce-grammar`, `mce-eval`, `mce-wasm`, `mce-cli`
