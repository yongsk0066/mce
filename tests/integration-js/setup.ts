/**
 * WASM engine shared initialization for all integration tests.
 *
 * Loads the MCE WASM module synchronously (Node.js) and initializes
 * the engine with dictionary, model, and wordlist.
 */

import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { initSync, MceEngine } from '../../crates/mce-wasm/pkg/mce_wasm.js'

// WASM load (Node.js sync)
const wasmPath = join(__dirname, '../../crates/mce-wasm/pkg/mce_wasm_bg.wasm')
initSync({ module: readFileSync(wasmPath) })

// Dictionary load
const dictPath = join(__dirname, '../../data/mor.vfst')
export const engine = MceEngine.load(new Uint8Array(readFileSync(dictPath)))

// Optional: suffix tagger model
try {
  const modelPath = join(__dirname, '../../data/suffix_tagger.bin')
  engine.load_model(new Uint8Array(readFileSync(modelPath)))
} catch {
  // Model not available -- disambiguation tests will use rule-only mode
}

// Optional: wordlist for trie-based suggestions
try {
  const wordlistPath = join(__dirname, '../../data/wordlist.txt')
  engine.load_wordlist(new Uint8Array(readFileSync(wordlistPath)))
} catch {
  // Wordlist not available -- suggest() will use brute-force fallback
}
