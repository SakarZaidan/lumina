# Current State

Rolling log, newest first. One dated entry per work session; ≤ 10 lines each.
For the release-by-release story see [HISTORY.md](./HISTORY.md).

---

## 2026-07-08 — Repo audit, planning system, hygiene batch

- Full three-track engineering audit completed (core crates, tooling/CI, docs/git).
- This planning system created; `todo.md` retired into [ROADMAP.md](./ROADMAP.md);
  blueprint and history moved under `planning/`.
- Hygiene batch in flight on `feat/v0.3.0-enhancements`: metadata fixes, crate
  rustdoc, README/CHANGELOG repair, community health files, mdBook v0.3.0 refresh.
- Git state at session start: `origin/main` = `7af221a` (v0.3.0 merge, **CI red
  6/8** — fixed by the two `ci:` commits on this branch); no tags existed.
- Next: PR → green CI on `main` → tag `v0.1.0`/`v0.2.0` (backdated) + `v0.3.0`
  → GitHub Release for v0.3.0.
- Version: 0.3.0 across the workspace; Python SDK pyproject drift (0.2.0) fixed
  in this batch.
