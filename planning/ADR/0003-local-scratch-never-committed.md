# ADR-0003 — Local scratch files never enter git

- **Status:** Accepted · **Date:** 2026-07-08

## Decision
`/text.md` is gitignored alongside `/plan-v1.md` and `.claude/`. Local
prompt/scratch files are never committed.
