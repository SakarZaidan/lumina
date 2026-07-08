# ADR-0008 — Historical tag placement

- **Status:** Accepted (executed) · **Date:** 2026-07-08

## Context
The v0.3.0 feature merge (`7af221a`) had red CI (cargo-deny config landed
later), and the CHANGELOG's version sections did not map one-to-one onto
commits (two `[0.1.0]` blocks; `[0.2.x]` items that first shipped in 0.3.0).

## Decision
After merging the duplicate `[0.1.0]` blocks and folding `[0.2.x]` into
`[0.3.0]`: `v0.1.0` → `02b92da` (2026-04-30; its tree contains everything the
merged 0.1.0 section describes), `v0.2.0` → `596b847` (2026-05-09; the
CHANGELOG 0.2.0 core features), both backdated via `GIT_COMMITTER_DATE` =
commit author date. `v0.3.0` → `9c35474`, the first green merge to `main`,
not red `7af221a`.

## Consequences
Pushed tags are never moved. The v0.3.0 tag's tree includes the hygiene docs
— intentional; do not "fix" the tag.
