#!/usr/bin/env bash
# bump-version.sh — Update workspace version across all MCE crates
#
# Usage: ./scripts/bump-version.sh 0.4.0
#
# What it does:
#   1. Validates semver format
#   2. Updates [workspace.package] version in root Cargo.toml
#   3. Runs cargo check --workspace to regenerate Cargo.lock
#   4. Verifies all 11 crates resolve to the new version
#   5. Updates CHANGELOG.md with new version header and comparison link
#   6. Prints next steps

set -euo pipefail

# --- Constants ---
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ROOT_CARGO="$REPO_ROOT/Cargo.toml"
CHANGELOG="$REPO_ROOT/CHANGELOG.md"
EXPECTED_CRATES=(
  mce-core mce-fst mce-tokenizer mce-speller mce-disambig
  mce-comonad mce-fi mce-grammar mce-eval mce-cli mce-wasm
)

# --- Helpers ---
die() { echo "ERROR: $*" >&2; exit 1; }
info() { echo "==> $*"; }

# --- Argument validation ---
if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <version>"
  echo "  e.g. $0 0.4.0"
  exit 1
fi

NEW_VERSION="$1"

# Validate semver (major.minor.patch, optional pre-release)
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
  die "Invalid semver format: '$NEW_VERSION' (expected X.Y.Z or X.Y.Z-pre.N)"
fi

# --- Read current version ---
CURRENT_VERSION=$(grep -m1 '^version' "$ROOT_CARGO" | sed 's/version = "//;s/"//')
if [[ -z "$CURRENT_VERSION" ]]; then
  die "Could not read current version from $ROOT_CARGO"
fi

if [[ "$CURRENT_VERSION" == "$NEW_VERSION" ]]; then
  die "Version is already $NEW_VERSION"
fi

info "Bumping version: $CURRENT_VERSION -> $NEW_VERSION"

# --- Step 1: Update root Cargo.toml ---
info "Updating $ROOT_CARGO"
sed -i '' "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" "$ROOT_CARGO"

# Verify the change took effect
VERIFY=$(grep -m1 '^version' "$ROOT_CARGO")
if [[ "$VERIFY" != "version = \"$NEW_VERSION\"" ]]; then
  die "Failed to update root Cargo.toml (got: $VERIFY)"
fi

# --- Step 2: cargo check to regenerate Cargo.lock ---
info "Running cargo check --workspace"
(cd "$REPO_ROOT" && cargo check --workspace 2>&1) || die "cargo check failed"

# --- Step 3: Verify all 11 crates have the new version in Cargo.lock ---
info "Verifying all crates resolve to $NEW_VERSION"
LOCK_FILE="$REPO_ROOT/Cargo.lock"
MISSING=()
for crate in "${EXPECTED_CRATES[@]}"; do
  if ! grep -A1 "name = \"$crate\"" "$LOCK_FILE" | grep -q "version = \"$NEW_VERSION\""; then
    MISSING+=("$crate")
  fi
done

if [[ ${#MISSING[@]} -gt 0 ]]; then
  die "These crates did not resolve to $NEW_VERSION: ${MISSING[*]}"
fi
info "All 11 crates verified at $NEW_VERSION"

# --- Step 4: Update CHANGELOG.md ---
info "Updating $CHANGELOG"
TODAY=$(date +%Y-%m-%d)

# Insert new version section after [Unreleased]
sed -i '' "/^## \[Unreleased\]/a\\
\\
## [$NEW_VERSION] - $TODAY
" "$CHANGELOG"

# Update comparison links at the bottom
# Change [Unreleased] link to point from new version
sed -i '' "s|\[Unreleased\]: \(.*\)/compare/v${CURRENT_VERSION}\.\.\.HEAD|[Unreleased]: \1/compare/v${NEW_VERSION}...HEAD|" "$CHANGELOG"

# Insert new version comparison link before the old current version link
sed -i '' "/^\[${CURRENT_VERSION}\]:/i\\
[$NEW_VERSION]: https://github.com/yongsk0066/mce/compare/v${CURRENT_VERSION}...v${NEW_VERSION}
" "$CHANGELOG"

info "CHANGELOG.md updated with [$NEW_VERSION] - $TODAY"

# --- Step 5: Summary ---
echo ""
echo "=========================================="
echo "  Version bumped: $CURRENT_VERSION -> $NEW_VERSION"
echo "=========================================="
echo ""
echo "Files modified:"
echo "  - Cargo.toml (workspace version)"
echo "  - Cargo.lock (regenerated)"
echo "  - CHANGELOG.md (new section + links)"
echo ""
echo "Next steps:"
echo "  1. Fill in the CHANGELOG.md [$NEW_VERSION] section"
echo "  2. git add Cargo.toml Cargo.lock CHANGELOG.md"
echo "  3. git commit -m 'chore: bump version to $NEW_VERSION'"
echo "  4. git tag v$NEW_VERSION"
echo "  5. git push origin main --tags"
echo ""
