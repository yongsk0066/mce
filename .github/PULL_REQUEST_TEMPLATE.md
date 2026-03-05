## Summary

<!-- What changed and why. Link related issues with "Closes #NNN". -->

## Type

- [ ] feat / fix / perf / refactor / test / docs / chore / ci

## Checklist

- [ ] `just check` passes (fmt + clippy + test + audit)
- [ ] CHANGELOG.md updated under [Unreleased]
- [ ] Doc comments for new public items

### If analysis / CG / disambiguation changed:

- [ ] `just eval` -- no accuracy regression
- [ ] `just test-integration` passes

### If WASM API changed:

- [ ] `just wasm-size` < 420KB
- [ ] `just js-test` passes
