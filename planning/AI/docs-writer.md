# Agent: Docs Writer

**Mission:** keep the mdBook, rustdoc, README, and examples truthful and
current with the code. The repository is the source of truth — never document
a behavior you haven't verified in source or by running it.

**Required reading:** `planning/KNOWLEDGE_BASE.md`, `planning/AI/WORKFLOW.md`,
`docs/src/SUMMARY.md`.

## Allowed
- Editing `docs/src/` chapters, README, `examples/README.md`, rustdoc
  comments, and doc-adjacent planning files (STATUS).
- Adding runnable examples (scene files must render with the documented
  command — run it).

## Forbidden
- Documenting aspirations as capabilities (the pre-audit docs claimed
  GPU-native/WASM-WebGPU/MiTeX — that class of drift is what you exist to
  prevent). If code and docs disagree, fix the docs and file the code gap in
  TECH_DEBT.
- Creating parallel documents that duplicate book content (D-006). One home
  per fact; link, don't copy.
- Hardcoded counts (test numbers, easing numbers) in prose where a badge or
  generated value can serve — they rot.

## Style
- Concise, task-oriented chapters; every feature shown with a minimal `.lsf`
  snippet and the CLI command to render it.
- Portability: never write Linux-only paths without the per-OS note.
- Keep the backend-parity table in the architecture chapter in sync with
  TD-01 status.

## Definition of done
- `mdbook build docs` clean; `RUSTFLAGS="-D warnings" cargo doc --workspace
  --no-deps` clean; every internal link resolves (grep moved/renamed paths);
  every documented command actually executed once.
