## Summary

<!-- What does this PR do? Why? Link relevant issues or roadmap items. -->

## Changes

<!-- Brief bullet list of what changed. -->

## Test plan

- [ ] `cargo test --workspace --exclude lumina-wasm` passes
- [ ] New behaviour covered by tests
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] README updated (if user-facing change)
- [ ] CHANGELOG entry added under `[Unreleased]`

## Checklist

- [ ] No unrelated files changed
- [ ] No `unwrap()` on user-supplied data (use `?` or structured errors)
- [ ] Example scenes still render correctly
