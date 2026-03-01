# MCE Web Demo

Browser-based Finnish NLP demo using the MCE WASM module.

## Prerequisites

- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) installed
- `mor.vfst` dictionary file (from `corevoikko/voikko-fi/vvfst/`)

## Build

```bash
# From the mce/ root directory:
./demo/build.sh

# Or manually:
wasm-pack build --target web --out-dir demo/pkg crates/mce-wasm
```

## Run

```bash
# Copy the dictionary into the demo directory
cp ~/oss/corevoikko/voikko-fi/vvfst/mor.vfst demo/

# Start a local server
cd demo
python3 -m http.server 8080
```

Open http://localhost:8080 in your browser.

## Features

| Button | Description |
|--------|-------------|
| **Analyze** | Tokenize, analyze, and disambiguate the sentence (POS + baseform) |
| **Check Spelling** | Highlight unknown words in red |
| **Compound Split** | Break compound words into parts (e.g. rautatieasema) |
| **Baseforms** | Show the dictionary form of each word |
| **Raw JSON** | Show full analysis JSON output |

Keyboard shortcut: **Ctrl+Enter** runs Analyze.

## File Structure

```
demo/
├── index.html    # UI
├── app.js        # WASM glue + event handlers
├── build.sh      # Build script
├── README.md     # This file
├── mor.vfst      # Dictionary (user-provided, not in git)
└── pkg/          # wasm-pack output (generated, not in git)
    ├── mce_wasm.js
    ├── mce_wasm_bg.wasm
    └── ...
```

## Notes

- The demo runs entirely client-side. No server-side processing.
- Dictionary loading takes a moment (mor.vfst is ~4 MB).
- The `pkg/` directory and `mor.vfst` are not committed to git.
