#!/bin/bash
set -e

cd "$(dirname "$0")/.."

echo "Building mce-wasm with wasm-pack..."
wasm-pack build --target web --out-dir demo/pkg crates/mce-wasm

echo ""
echo "Build complete!"
echo ""
echo "To run the demo:"
echo "  1. Copy the dictionary:  cp ~/oss/corevoikko/voikko-fi/vvfst/mor.vfst demo/"
echo "  2. Start a server:       cd demo && python3 -m http.server 8080"
echo "  3. Open browser:         http://localhost:8080"
