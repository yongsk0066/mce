# Contributing to MCE

Thanks for your interest in contributing to the Morphological Computation Engine.
This guide covers everything you need to get started.

---

## Prerequisites

| Tool | Version | Purpose | Install |
|------|---------|---------|---------|
| Rust | 1.86+ | Compiler (edition 2024) | [rustup.rs](https://rustup.rs) |
| wasm-pack | latest | WASM builds | `cargo install wasm-pack` |
| bun | latest | JS test runner | [bun.sh](https://bun.sh) |
| just | 1.0+ | Task runner | `cargo install just` |
| cargo-audit | latest | Dependency audit | `cargo install cargo-audit` |
| lefthook | 2.1+ | Git hooks | `brew install lefthook` or [GitHub](https://github.com/evilmartians/lefthook) |
| Python 3 | 3.8+ | Demo server | Pre-installed on most systems |

Optional: `cargo-criterion` for extended benchmark HTML reports.

---

## First-time Setup

```bash
# 1. Clone with submodules (vendor/ud-finnish-tdt etc.)
git clone --recurse-submodules https://github.com/yongsk0066/mce.git
cd mce

# 2. Run the full check suite
just check

# 3. Install git hooks (optional but recommended)
just hooks-install
```

If you already cloned without `--recurse-submodules`:

```bash
git submodule update --init --recursive
```

---

## Data Files

The `data/` directory contains runtime assets required by integration tests and
evaluation. These files are **not** checked into Git (tracked via `.gitignore`
or LFS). Obtain them from a team member or build them yourself.

| File | Size | Purpose |
|------|------|---------|
| `data/mor.vfst` | ~3.8MB | VFST morphological dictionary |
| `data/suffix_tagger.bin` | ~5.0MB | Suffix tagger model (MCET v1) |
| `data/lemma_dict.tsv` | ~1.3MB | Lemma dictionary (48K entries) |
| `data/wordlist.txt` | ~500KB | Wordlist for spell suggestions |

`vendor/` contains UD treebank submodules used for evaluation:

| Submodule | Content |
|-----------|---------|
| `vendor/ud-finnish-tdt` | UD Finnish-TDT (train/dev/test CoNLL-U) |
| `vendor/ud-finnish-pud` | UD Finnish-PUD |
| `vendor/ud-finnish-ood` | UD Finnish-OOD |

---

## Task Runner

MCE uses [just](https://github.com/casey/just) as its task runner. All recipes
are defined in the `justfile` at the project root.

```bash
just --list        # Show all available recipes
just               # Runs the default recipe: check
```

### Key Recipes

| Recipe | What it does | CI equivalent |
|--------|-------------|---------------|
| `just check` | fmt-check + lint + test + audit | `ci.yml` |
| `just test` | Unit tests (no data needed) | `ci.yml` |
| `just test-all` | All tests including `#[ignore]` | `perf.yml` |
| `just test-integration` | Only `#[ignore]` tests | `perf.yml` |
| `just js-test` | JS integration tests (375 tests) | `release-candidate.yml` |
| `just eval` | UPOS/Lemma accuracy (dev set) | `perf.yml` |
| `just wasm-size` | Build WASM + check 420KB budget | `perf.yml` |
| `just release-check` | Full pre-release verification | Manual |

---

## Testing

### Unit Tests

```bash
just test          # Fast, no data/ files needed
```

Runs all `#[test]` functions across the workspace. These cover pure logic,
comonad laws, character classification, tokenizer rules, and more.

### Integration Tests

```bash
just test-all          # Unit + integration (needs data/)
just test-integration  # Only #[ignore] tests (needs data/)
just test-crate mce-fi # Single crate, all tests
```

Integration tests are marked `#[ignore]` and require `data/` files. They
exercise the full pipeline: FST loading, analysis, disambiguation, evaluation.

### JS Integration Tests

```bash
just js-test       # Builds WASM, then runs vitest
```

Tests the WASM API from JavaScript. Located in `tests/integration-js/test/`.

| Test file | Focus | Approx. tests |
|-----------|-------|---------------|
| `analyze.test.ts` | Single-word morphological analysis | 8 |
| `compound.test.ts` | Compound word splitting | 6 |
| `coverage.test.ts` | Dictionary coverage on word lists | 5 |
| `generate.test.ts` | Noun/verb paradigm generation | 3 |
| `grammar.test.ts` | Grammar rule checking | 11 |
| `hyphenate.test.ts` | Hyphenation | 3 |
| `sentence.test.ts` | Sentence analysis + disambiguation | 11 |
| `spell-check.test.ts` | Spell checking | 3 |
| `suggest.test.ts` | Spelling suggestions | 3 |

Many tests use parameterized `it.each()`, so the actual test count (~375) is
higher than the number of `it()` calls.

---

## WASM Building

```bash
just wasm          # Build WASM package to crates/mce-wasm/pkg/
just wasm-size     # Build + report size vs 420KB budget
```

The WASM binary budget is **420KB** (430,080 bytes). The `perf.yml` CI workflow
enforces this limit. If your change increases the binary size significantly,
investigate with `twiggy top crates/mce-wasm/pkg/mce_wasm_bg.wasm`.

---

## Demo

```bash
just demo          # Build WASM, copy assets, start localhost:8080
```

Opens a local demo page at `http://localhost:8080` with the interactive MCE
playground. The demo files live in `demo/`.

---

## Making Changes

### Branch Naming

Use a prefix that matches the change type:

| Prefix | Use for |
|--------|---------|
| `feat/` | New features |
| `fix/` | Bug fixes |
| `refactor/` | Code restructuring |
| `perf/` | Performance improvements |
| `docs/` | Documentation only |
| `test/` | Test additions/fixes |
| `chore/` | Tooling, dependencies, CI |
| `release/` | Release preparation |

Example: `feat/verb-generation-type5`, `fix/utf8-edit-distance`

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]
```

Scope is typically one of the 11 crate names without the `mce-` prefix:

```
feat(comonad): add RemoveIfMood CG rule type
fix(fi): handle consonant gradation in -oida verbs
perf(disambig): transpose weight matrix for cache locality
test(wasm): add verb generation JS integration tests
docs(eval): document PUNCT/SYM exclusion in UPOS scoring
chore(ci): add WASM size budget to perf workflow
```

If `lefthook` is installed, the `commit-msg` hook enforces this format.

### Code Style

- **Formatting**: `cargo fmt` (edition 2024). Run `just fmt` before committing.
- **Linting**: `cargo clippy --all-features -- -D warnings`. Zero warnings policy.
- **Doc comments**: All public items (`pub fn`, `pub struct`, `pub trait`) must
  have `///` doc comments explaining purpose and usage.
- **Error handling**: Prefer `Result<T, E>` over `.unwrap()` in library code.
  `.unwrap()` is acceptable in tests and CLI code.
- **Tests**: Place unit tests in `#[cfg(test)]` modules in the same file as
  the code they test.

---

## Adding a CG Rule

CG (Constraint Grammar) rules live in `crates/mce-comonad/src/cg.rs`. Each
rule implements the `CgRule` trait as a coKleisli arrow.

1. **Define a struct** implementing `CgRule`:

```rust
pub struct RemoveIfMyCondition {
    pub target_pos: &'static str,
    pub condition_pos: &'static str,
}

impl CgRule for RemoveIfMyCondition {
    fn apply(&self, readings: &mut ReadingSet, context: &[ReadingSet]) {
        // Your disambiguation logic here.
        // `readings` is the current position's reading set.
        // `context` provides the surrounding positions.
    }

    fn name(&self) -> &str {
        "RemoveIfMyCondition"
    }
}
```

2. **Register it** in the `finnish_disambiguation_rules()` function at the
   bottom of `cg.rs`. Rules are ordered by phase (1-23). Place your rule in
   the appropriate phase.

3. **Add tests** in the `#[cfg(test)]` module at the bottom of `cg.rs`.

4. **Run evaluation** to verify no accuracy regression:

```bash
just eval          # Should show no drop in UPOS %
just test-crate mce-comonad
```

---

## Adding a Grammar Rule

Grammar rules live in `crates/mce-grammar/src/rules/`. Each rule is a separate
file implementing the grammar check trait.

1. **Create a new file** in `crates/mce-grammar/src/rules/`, e.g.,
   `my_new_rule.rs`.

2. **Implement the rule**: Follow the pattern of existing rules like
   `repeated_word.rs` or `double_space.rs`.

3. **Register it** in `crates/mce-grammar/src/rules/mod.rs`.

4. **Add tests** in the same file using `#[cfg(test)]`.

5. **Run the grammar tests**:

```bash
just test-crate mce-grammar
```

---

## Pull Request Process

1. Create a branch from `main` with the appropriate prefix.
2. Make your changes and ensure `just check` passes locally.
3. Update `CHANGELOG.md` under `[Unreleased]` if the change is user-visible.
4. Push and open a PR. CI will run automatically:

   | CI workflow | What it checks |
   |-------------|---------------|
   | `ci.yml` | fmt + clippy + test + audit |
   | `perf.yml` | Evaluation accuracy + WASM size budget |
   | `release-candidate.yml` | Rust + WASM + JS tests (on PRs) |

5. If CI posts a perf report, review the numbers. Any UPOS regression needs
   explanation.
6. Fill in the PR template checklist.

---

## Release Process

1. Run the full pre-release check:

```bash
just release-check     # check + wasm-size + js-test
```

2. Bump the version:

```bash
just version 0.4.0     # Runs scripts/bump-version.sh
```

This updates `Cargo.toml` (workspace), regenerates `Cargo.lock`, and adds
a new section to `CHANGELOG.md`.

3. Create a `release/v0.4.0` branch, push, and open a PR.

4. After merge, tag and push:

```bash
git tag v0.4.0
git push origin main --tags
```

The `npm-publish.yml` workflow handles npm publishing automatically on tags.

---

## Project Structure

```
crates/
├── mce-core/       # Shared types, LOUDS Succinct Trie, character utils
├── mce-fst/        # FST engine (VFST format, flag diacritics)
├── mce-tokenizer/  # Text tokenizer (words, sentences, URLs, emails)
├── mce-speller/    # Spell checking and suggestion engine
├── mce-comonad/    # Writer Comonad + CG rules (morphophonology)
├── mce-disambig/   # POS disambiguation (Viterbi + Suffix Tagger)
├── mce-fi/         # Finnish language module (analysis, generation)
├── mce-grammar/    # Grammar checking (21 rules)
├── mce-eval/       # Evaluation against UD treebanks
├── mce-wasm/       # WASM bindings (22 API methods)
└── mce-cli/        # CLI tools (11 subcommands)
```

---

## License

By contributing, you agree that your contributions will be licensed under the
[Apache-2.0](LICENSE) license.
