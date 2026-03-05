# MCE — Morphological Computation Engine
# Task runner for development, testing, evaluation, and release.
#
# Usage:  just <recipe>
# List:   just --list

set dotenv-load := true
set positional-arguments := true

# Default: run the full check suite (fmt + clippy + test + audit)
default: check

# ---------------------------------------------------------------------------
# Core checks
# ---------------------------------------------------------------------------

# Run fmt-check + lint + test + audit
check: fmt-check lint test audit

# Check Rust formatting (no changes)
fmt-check:
    cargo fmt --all --check

# Apply Rust formatting
fmt:
    cargo fmt --all

# Run clippy with all features, deny warnings
lint:
    cargo clippy --all-features -- -D warnings

# Run unit tests (no data/ files required)
test:
    cargo test --all-features

# Run all tests including #[ignore] (requires data/ files)
test-all:
    MCE_DICT_PATH=data cargo test --all-features -- --include-ignored

# Run only #[ignore] integration tests (requires data/ files)
test-integration:
    MCE_DICT_PATH=data cargo test --all-features -- --ignored

# Run tests for a single crate (includes #[ignore])
test-crate crate:
    MCE_DICT_PATH=data cargo test -p {{crate}} -- --include-ignored

# Run cargo-audit for known vulnerabilities
audit:
    cargo audit

# ---------------------------------------------------------------------------
# WASM
# ---------------------------------------------------------------------------

# Build WASM package (web target)
wasm:
    wasm-pack build --target web --out-dir pkg crates/mce-wasm

# Build WASM and report binary size vs 420KB budget
wasm-size: wasm
    #!/usr/bin/env bash
    set -euo pipefail
    WASM="crates/mce-wasm/pkg/mce_wasm_bg.wasm"
    if [[ ! -f "$WASM" ]]; then
        echo "ERROR: $WASM not found"; exit 1
    fi
    SIZE=$(wc -c < "$WASM" | tr -d ' ')
    SIZE_KB=$((SIZE / 1024))
    BUDGET=420
    echo "WASM size: ${SIZE_KB}KB (${SIZE} bytes)"
    echo "Budget:    ${BUDGET}KB (430,080 bytes)"
    if [[ $SIZE -gt 430080 ]]; then
        echo "OVER BUDGET by $((SIZE_KB - BUDGET))KB"
        exit 1
    else
        echo "Within budget ($(( BUDGET - SIZE_KB ))KB headroom)"
    fi

# Run JS integration tests (builds WASM first)
js-test: wasm
    cd tests/integration-js && bun install && npx vitest run

# ---------------------------------------------------------------------------
# Evaluation & benchmarks
# ---------------------------------------------------------------------------

# Evaluate UPOS + Lemma accuracy on dev set (JSON output)
eval:
    MCE_DICT_PATH=data cargo run -p mce-cli --release -- eval \
        --conllu vendor/ud-finnish-tdt/fi_tdt-ud-dev.conllu \
        --model data/suffix_tagger.bin \
        --lemma-dict data/lemma_dict.tsv \
        --format json

# Evaluate UPOS + Lemma accuracy on test set (JSON output)
eval-test:
    MCE_DICT_PATH=data cargo run -p mce-cli --release -- eval \
        --conllu vendor/ud-finnish-tdt/fi_tdt-ud-test.conllu \
        --model data/suffix_tagger.bin \
        --lemma-dict data/lemma_dict.tsv \
        --format json

# Evaluate in table format (human-readable)
eval-table:
    MCE_DICT_PATH=data cargo run -p mce-cli --release -- eval \
        --conllu vendor/ud-finnish-tdt/fi_tdt-ud-dev.conllu \
        --model data/suffix_tagger.bin \
        --lemma-dict data/lemma_dict.tsv \
        --format table

# Run criterion benchmarks
bench:
    cargo bench --all-features

# Run throughput benchmark (1000 iterations, 100 warmup)
bench-throughput:
    MCE_DICT_PATH=data cargo run -p mce-cli --release -- benchmark --iterations 1000 --warmup 100

# Run throughput + per-rule latency benchmarks
bench-rules:
    MCE_DICT_PATH=data cargo run -p mce-cli --release -- benchmark --iterations 1000 --warmup 100 --rules

# ---------------------------------------------------------------------------
# Demo
# ---------------------------------------------------------------------------

# Build WASM, copy assets to demo/, and start local server at :8080
demo: wasm
    #!/usr/bin/env bash
    set -euo pipefail
    cp crates/mce-wasm/pkg/mce_wasm_bg.wasm demo/pkg/
    cp crates/mce-wasm/pkg/mce_wasm.js       demo/pkg/
    cp crates/mce-wasm/pkg/mce_wasm.d.ts     demo/pkg/
    echo "Copying data files to demo/ ..."
    cp data/mor.vfst          demo/
    cp data/suffix_tagger.bin demo/ 2>/dev/null || true
    cp data/wordlist.txt      demo/ 2>/dev/null || true
    echo ""
    echo "Starting demo server at http://localhost:8080"
    echo "Press Ctrl+C to stop."
    cd demo && python3 -m http.server 8080

# ---------------------------------------------------------------------------
# Docs
# ---------------------------------------------------------------------------

# Generate and open Rust API documentation
doc:
    cargo doc --all-features --no-deps --open

# ---------------------------------------------------------------------------
# Release
# ---------------------------------------------------------------------------

# Full pre-release verification: check + wasm-size + js-test
release-check: check wasm-size js-test
    @echo ""
    @echo "All release checks passed."

# Bump workspace version using scripts/bump-version.sh
version ver:
    ./scripts/bump-version.sh {{ver}}

# ---------------------------------------------------------------------------
# Cleanup
# ---------------------------------------------------------------------------

# Remove build artifacts
clean:
    cargo clean
    rm -rf crates/mce-wasm/pkg
    rm -rf tests/integration-js/node_modules

# ---------------------------------------------------------------------------
# Utilities
# ---------------------------------------------------------------------------

# Print project statistics (LOC, tests, data sizes)
stats:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== MCE Project Statistics ==="
    echo ""
    echo "--- Lines of code (Rust) ---"
    find crates -name '*.rs' | xargs wc -l | tail -1
    echo ""
    echo "--- Test count ---"
    RUST_TESTS=$(grep -r '#\[test\]' crates/ | wc -l | tr -d ' ')
    echo "  Rust #[test]: $RUST_TESTS"
    JS_FILES=$(find tests/integration-js/test -name '*.test.ts' 2>/dev/null | wc -l | tr -d ' ')
    echo "  JS test files: $JS_FILES"
    echo ""
    echo "--- Crates ---"
    ls -1d crates/*/ | wc -l | xargs printf "  Count: %s\n"
    echo ""
    echo "--- Data files ---"
    if [[ -d data ]]; then
        ls -lh data/
    else
        echo "  data/ directory not found"
    fi
    echo ""
    echo "--- WASM binary ---"
    WASM="crates/mce-wasm/pkg/mce_wasm_bg.wasm"
    if [[ -f "$WASM" ]]; then
        SIZE=$(wc -c < "$WASM" | tr -d ' ')
        echo "  ${WASM}: $((SIZE / 1024))KB"
    else
        echo "  Not built yet (run 'just wasm')"
    fi

# ---------------------------------------------------------------------------
# Git hooks (lefthook)
# ---------------------------------------------------------------------------

# Install lefthook git hooks
hooks-install:
    lefthook install

# Uninstall lefthook git hooks
hooks-uninstall:
    lefthook uninstall
