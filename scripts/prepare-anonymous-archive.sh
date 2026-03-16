#!/bin/bash
# scripts/prepare-anonymous-archive.sh
# Paper-3 SCiL 2026 submission — anonymous supplementary archive
#
# Creates a sanitized zip archive with no identifying information.
# Pipeline: EXTRACT → SANITIZE → VERIFY → PACKAGE
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TMPDIR_BASE=$(mktemp -d)
ARCHIVE_DIR="$TMPDIR_BASE/anonymous-mce"
DATESTAMP=$(date +%Y-%m-%d)
COMMIT_SHA=$(git -C "$PROJECT_ROOT" rev-parse --short HEAD 2>/dev/null || echo "nogit")
if [ "$COMMIT_SHA" != "nogit" ] && ! git -C "$PROJECT_ROOT" diff --quiet HEAD 2>/dev/null; then
  echo "WARNING: Working tree has uncommitted changes (commit SHA may not match archive contents)"
fi
OUTPUT_ZIP="$PROJECT_ROOT/supplementary-code.zip"

# Crates to include (paper-relevant only)
INCLUDE_CRATES=(
  mce-core
  mce-fst
  mce-tokenizer
  mce-comonad
  mce-disambig
  mce-speller
  mce-fi
  mce-eval
)

# Sanitization sed patterns (identifying info → anonymous)
SANITIZE_PATTERNS=(
  # GitHub username/URLs
  's|yongsk0066\.github\.io/mce/|anonymous.example.com|g'
  's|github\.com/yongsk0066/mce|github.com/anonymous/anonymous-mce|g'
  's|github\.com/yongsk0066/corevoikko|github.com/anonymous/anonymous-voikko|g'
  's|@yongsk0066/mce|@anonymous/mce|g'
  's|yongsk0066|anonymous|g'

  # Real name
  's|Yongseok Jang|[Anonymous]|g'
  's|YongSeok Jang|[Anonymous]|g'
  's|장용석|[Anonymous]|g'

  # Emails
  's|yongsk0066@naver\.com|anonymous@example.com|g'
  's|yongsk0066@gmail\.com|anonymous@example.com|g'

  # Local paths
  's|/Users/yongseok/[^ ]*||g'

  # corevoikko project link (same author)
  's|corevoikko|anonymous-voikko|g'
)

# Forbidden patterns for leak verification
FORBIDDEN_PATTERNS=(
  "yongsk0066"
  "yongseok"
  "Yongseok"
  "YongSeok"
  "장용석"
  "github.com/yongsk0066"
  "yongsk0066.github.io"
  "@yongsk0066"
  "naver.com"
  "corevoikko"
  "/Users/yongseok"
)

cleanup() {
  rm -rf "$TMPDIR_BASE"
}
trap cleanup EXIT

# ============================================================
echo "=== Phase 1: EXTRACT ==="
# ============================================================

mkdir -p "$ARCHIVE_DIR/crates"

# Copy included crates
for crate in "${INCLUDE_CRATES[@]}"; do
  if [ ! -d "$PROJECT_ROOT/crates/$crate" ]; then
    echo "ERROR: crate $crate not found at $PROJECT_ROOT/crates/$crate"
    exit 1
  fi
  cp -R "$PROJECT_ROOT/crates/$crate" "$ARCHIVE_DIR/crates/$crate"
done

# Copy Cargo.lock for reproducibility
cp "$PROJECT_ROOT/Cargo.lock" "$ARCHIVE_DIR/Cargo.lock"

# Create workspace Cargo.toml (only included crates, no repository/homepage)
cat > "$ARCHIVE_DIR/Cargo.toml" << 'WORKSPACE_EOF'
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
version = "0.0.0"
edition = "2024"
rust-version = "1.86"
license = "Apache-2.0"
keywords = ["nlp", "finnish", "morphology"]
categories = ["text-processing"]

[workspace.dependencies]
criterion = { version = "0.8", features = ["html_reports"] }
proptest = "1"
thiserror = "2"
bytemuck = { version = "1", features = ["derive"] }
hashbrown = "0.16"

# Internal crates
mce-core = { path = "crates/mce-core" }
mce-fst = { path = "crates/mce-fst" }
mce-tokenizer = { path = "crates/mce-tokenizer" }
mce-speller = { path = "crates/mce-speller" }
mce-disambig = { path = "crates/mce-disambig" }
mce-comonad = { path = "crates/mce-comonad" }
mce-fi = { path = "crates/mce-fi" }
mce-eval = { path = "crates/mce-eval" }

[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
strip = true
panic = "abort"
WORKSPACE_EOF

# Create anonymous README.md
cat > "$ARCHIVE_DIR/README.md" << 'README_EOF'
# Supplementary Code: Comonadic Morphophonology

Anonymous supplementary material for paper submission.

## Build

```bash
cargo build --release -p mce-eval
```

## Run evaluation (requires data files)

```bash
# Download UD Finnish-TDT v2.14 (CC-BY-SA 4.0)
# Place in vendor/ud-finnish-tdt/

# Download Voikko mor.vfst (GPL-3.0)
# Place in data/mor.vfst

# Download suffix_tagger.bin (see data/README.md)
# Place in data/suffix_tagger.bin

cargo test -p mce-comonad   # Comonad law verification (44 tests)
cargo test -p mce-disambig  # Suffix tagger tests
cargo test -p mce-eval      # Evaluation pipeline
```

## Structure

```
crates/
  mce-comonad/   — Writer Comonad, coKleisli arrows, CG-lite rules
  mce-disambig/  — Suffix Tagger, Viterbi disambiguation
  mce-eval/      — UD evaluation pipeline (UPOS, Lemma)
  mce-core/      — Shared types, character classification, LOUDS trie
  mce-fst/       — FST engine (VFST format, flag diacritics)
  mce-tokenizer/ — Text tokenizer
  mce-speller/   — Spell checking pipeline
  mce-fi/        — Finnish language module (analysis, generation)
```

## Key Results

| Metric | Value |
|--------|-------|
| UPOS (CG + Suffix Tagger) | 94.66% |
| UPOS (rule-only) | 83.92% |
| Lemma | 93.09% |
| Coverage | 99.35% |
| Speed | 84,973 tokens/sec |

Evaluated on UD Finnish-TDT v2.14 dev set (gold tokenization).
README_EOF

# Create anonymous data/README.md
mkdir -p "$ARCHIVE_DIR/data"
cat > "$ARCHIVE_DIR/data/README.md" << 'DATA_EOF'
# data/

Runtime data files required for evaluation. These files are not included
in the archive due to size and licensing constraints.

## Required Files

- **`mor.vfst`** (~3.8 MB) — Voikko Finnish morphological FST dictionary.
  License: GPL-3.0. Obtain from the Voikko project (voikko-fi/vvfst/mor.vfst).

- **`suffix_tagger.bin`** (~5.0 MB) — Trained suffix-based logistic regression
  model for POS tagging. Binary format (MCET v1). Train with the script in
  experiments/suffix-tagger/ using UD Finnish-TDT (CC BY-SA 4.0).

- **`lemma_dict.tsv`** (~1.2 MB) — Lemma dictionary extracted from UD Finnish
  treebanks. TSV format: `form<TAB>UPOS<TAB>lemma`. Extract from UD data using
  the provided extraction script.

## UD Treebanks

Place UD treebank repositories in `vendor/`:

- `vendor/ud-finnish-tdt/` — UD Finnish-TDT v2.14 (CC BY-SA 4.0)
- `vendor/ud-finnish-ood/` — UD Finnish-OOD (CC BY-SA 4.0)
- `vendor/ud-finnish-pud/` — UD Finnish-PUD (CC BY-SA 4.0)

Available from: https://universaldependencies.org/
DATA_EOF

# Create anonymous LICENSE
cat > "$ARCHIVE_DIR/LICENSE" << 'LICENSE_EOF'
                                 Apache License
                           Version 2.0, January 2004
                        http://www.apache.org/licenses/

   TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION

   [Standard Apache 2.0 terms apply — see http://www.apache.org/licenses/LICENSE-2.0]

   Copyright 2026 [Anonymous]

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
LICENSE_EOF

echo "  Extracted ${#INCLUDE_CRATES[@]} crates to $ARCHIVE_DIR"

# ============================================================
echo "=== Phase 2: SANITIZE ==="
# ============================================================

# Remove per-crate files that may contain identifying info
find "$ARCHIVE_DIR/crates" -name "CLAUDE.md" -delete 2>/dev/null || true
find "$ARCHIVE_DIR/crates" -name "LEARNING.md" -delete 2>/dev/null || true
find "$ARCHIVE_DIR/crates" -name "CHANGELOG.md" -delete 2>/dev/null || true
find "$ARCHIVE_DIR/crates" -name "ARCHITECTURE.md" -delete 2>/dev/null || true
find "$ARCHIVE_DIR/crates" -name "README.md" -delete 2>/dev/null || true

# Remove test files that import excluded crates (mce-grammar)
rm -f "$ARCHIVE_DIR/crates/mce-fi/tests/full_pipeline_integration.rs"
rm -f "$ARCHIVE_DIR/crates/mce-fi/tests/npm_consumer_tests.rs"

# Remove dev-dependencies on excluded crates (mce-grammar, mce-wasm, mce-cli)
# mce-fi has mce-grammar as dev-dependency
while IFS= read -r -d '' cargo_file; do
  sed -i '' '/^mce-grammar\.workspace/d' "$cargo_file"
  sed -i '' '/^mce-wasm\.workspace/d' "$cargo_file"
  sed -i '' '/^mce-cli\.workspace/d' "$cargo_file"
done < <(find "$ARCHIVE_DIR/crates" -name 'Cargo.toml' -print0)

# Build sed expression from patterns
SED_ARGS=()
for pattern in "${SANITIZE_PATTERNS[@]}"; do
  SED_ARGS+=(-e "$pattern")
done

# Apply sanitization to text files (skip binaries and Cargo.lock)
SANITIZED_COUNT=0
while IFS= read -r -d '' file; do
  if sed -i '' "${SED_ARGS[@]}" "$file" 2>/dev/null; then
    SANITIZED_COUNT=$((SANITIZED_COUNT + 1))
  fi
done < <(find "$ARCHIVE_DIR" \
  \( -name '*.rs' -o -name '*.toml' -o -name '*.md' -o -name '*.txt' -o -name '*.yml' -o -name '*.yaml' \) \
  ! -name 'Cargo.lock' \
  -print0)

# Remove repository/homepage from per-crate Cargo.toml files
# (some crates may inherit workspace fields or have their own)
while IFS= read -r -d '' cargo_file; do
  sed -i '' \
    -e '/^repository[[:space:]]*=/d' \
    -e '/^homepage[[:space:]]*=/d' \
    "$cargo_file"
done < <(find "$ARCHIVE_DIR/crates" -name 'Cargo.toml' -print0)

echo "  Sanitized $SANITIZED_COUNT files"

# ============================================================
echo "=== Phase 3: VERIFY ==="
# ============================================================

LEAKED=0
for pattern in "${FORBIDDEN_PATTERNS[@]}"; do
  MATCHES=$(grep -rl "$pattern" "$ARCHIVE_DIR" \
    --include='*.rs' --include='*.toml' --include='*.md' \
    --include='*.txt' --include='*.yml' --include='*.yaml' 2>/dev/null || true)
  if [ -n "$MATCHES" ]; then
    echo "  LEAK: '$pattern' found in:"
    echo "$MATCHES" | sed 's/^/    /'
    LEAKED=1
  fi
done

if [ "$LEAKED" -eq 1 ]; then
  echo ""
  echo "FAILED: Identifying information detected in archive."
  echo "Review the files above and update SANITIZE_PATTERNS."
  # Don't clean up — let user inspect
  trap - EXIT
  echo "Archive left at: $ARCHIVE_DIR"
  exit 1
fi
echo "  No identifying information found"

# Compile check
echo "  Checking compilation..."
CHECK_OUTPUT=$(cd "$ARCHIVE_DIR" && cargo check --workspace 2>&1) || {
  echo "$CHECK_OUTPUT" | tail -10
  echo ""
  echo "FAILED: Archive does not compile."
  trap - EXIT
  echo "Archive left at: $ARCHIVE_DIR"
  exit 1
}
echo "  Archive compiles successfully"

# Test check (comonad laws are the most paper-relevant)
echo "  Running key tests..."
COMONAD_OUTPUT=$(cd "$ARCHIVE_DIR" && cargo test -p mce-comonad 2>&1) && {
  echo "$COMONAD_OUTPUT" | tail -3
  echo "  Comonad tests pass"
} || {
  echo "$COMONAD_OUTPUT" | tail -5
  echo "  WARNING: mce-comonad tests failed (may need data files)"
}

DISAMBIG_OUTPUT=$(cd "$ARCHIVE_DIR" && cargo test -p mce-disambig 2>&1) && {
  echo "$DISAMBIG_OUTPUT" | tail -3
  echo "  Disambig tests pass"
} || {
  echo "$DISAMBIG_OUTPUT" | tail -5
  echo "  WARNING: mce-disambig tests failed (may need model file)"
}

# ============================================================
echo "=== Phase 4: PACKAGE ==="
# ============================================================

# Remove target/ if cargo check created it
rm -rf "$ARCHIVE_DIR/target"

(cd "$TMPDIR_BASE" && zip -r "$OUTPUT_ZIP" "anonymous-mce" \
  -x '*/target/*' '*.vfst' '*.bin' '*.tsv' '*/.DS_Store' > /dev/null)

echo ""
echo "Done: $OUTPUT_ZIP"
ls -lh "$OUTPUT_ZIP"
