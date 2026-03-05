#![no_main]

use libfuzzer_sys::fuzz_target;
use mce_tokenizer::{next_token, next_sentence};

fuzz_target!(|data: &[u8]| {
    // Only operate on valid UTF-8.
    let text = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    // Tokenize the entire input — must not panic.
    let mut pos = 0;
    let mut iterations = 0;
    while pos < len && iterations < 10_000 {
        let (tok_type, tok_len) = next_token(&chars, len, pos);
        if tok_len == 0 {
            break;
        }
        let _ = tok_type;
        pos += tok_len;
        iterations += 1;
    }

    // Sentence detection — must not panic.
    let mut pos = 0;
    let mut iterations = 0;
    while pos < len && iterations < 10_000 {
        let (sent_type, sent_len) = next_sentence(&chars, len, pos);
        if sent_len == 0 {
            break;
        }
        let _ = sent_type;
        pos += sent_len;
        iterations += 1;
    }
});
