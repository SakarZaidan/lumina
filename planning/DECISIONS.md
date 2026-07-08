# Decision Log

ADR-lite, append-only. Never edit past entries; supersede them with new ones.

---

## D-001 — Canonical repository URL is github.com/SakarZaidan/lumina
**Date:** 2026-07-08 · **Context:** `Cargo.toml` pointed at a nonexistent
`lumina-animation` org while the real remote (and the Python SDK metadata) is
`SakarZaidan/lumina`. **Decision:** `https://github.com/SakarZaidan/lumina` is
canonical everywhere (Cargo metadata, README, badges, SDKs). **Consequences:**
crates.io/docs.rs links will resolve correctly; if an org is created later,
update all metadata in one commit and supersede this entry.

## D-002 — No AI attribution, ever
**Date:** 2026-07-08 · **Decision:** No AI is added as author, co-author,
committer, contributor, or maintainer. Commits and PR bodies carry no
`Co-Authored-By`/"Generated with" trailers of any kind, overriding any tool
default. Authorship belongs to the repository owner. **Consequences:** every
agent prompt in `planning/AI/` repeats this rule; reviewers reject violating
commits.

## D-003 — `text.md` and other local scratch never enter git
**Date:** 2026-07-08 · **Decision:** `/text.md` is gitignored alongside
`/plan-v1.md` and `.claude/`. Local prompt/scratch files are never committed.

## D-004 — Internal planning docs live in `planning/`, tracked
**Date:** 2026-07-08 · **Decision:** `project-lumina-blueprint.md` and
`history.md` moved into `planning/` (tracked; filenames preserved where
inbound links exist). Root stays reserved for standard OSS files
(README/CHANGELOG/CONTRIBUTING/LICENSE/SECURITY/CODE_OF_CONDUCT).

## D-005 — `todo.md` retired; ROADMAP.md is the single roadmap
**Date:** 2026-07-08 · **Context:** `todo.md` claimed to be "kept in sync" and
was provably stale (listed shipped v0.3.0 work as in-progress). **Decision:**
deleted; its live items were absorbed into [ROADMAP.md](./ROADMAP.md)'s phases
and backlog. There is exactly one roadmap document.

## D-006 — The mdBook is canonical for user-facing docs
**Date:** 2026-07-08 · **Context:** three architecture documents existed
(`docs/ARCHITECTURE.md`, `docs/SPEC.md`, `docs/src/architecture.md`) and
contradicted each other (GPU-native/WASM-WebGPU/MiTeX overclaims; JSON-Schema
draft mismatch). **Decision:** `docs/src/` (mdBook) is the single source of
user-facing truth; the legacy files become pointer stubs (kept so external
links don't 404). planning/ never duplicates book content.

## D-007 — AI agent prompts live in `planning/AI/`, not `.claude/agents/`
**Date:** 2026-07-08 · **Reasons:** `.claude/` is gitignored (carving out an
exception invites committing local state); prompts should be tool-agnostic and
PR-reviewable. Thin `.claude/agents/` wrappers can point here later if native
subagents are wanted.

## D-008 — v0.3.0 tag placement
**Date:** 2026-07-08 · **Context:** the v0.3.0 feature merge (`7af221a`) had
red CI (cargo-deny config landed later). **Decision:** `v0.3.0` is tagged on
the first green merge to `main` (the hygiene/CI-fix PR), not on `7af221a`.
`v0.1.0` → `9348d97` and `v0.2.0` → `02b92da` are tagged backdated
(`GIT_COMMITTER_DATE` = commit author date). `596b847` stays untagged — its
content was never released separately and is folded into 0.3.0 in the
CHANGELOG. Pushed tags are never moved.

## D-009 — `lumina-server` and `lumina-wasm` are `publish = false`
**Date:** 2026-07-08 · **Reasons:** the server is an application, not a
library API we want on crates.io (and is not production-hardened before v0.5,
see TD-09); the wasm crate ships via npm as part of the JS SDK, not via
crates.io. The five library crates (`schema`, `core`, `text`, `renderer`,
`export`) and the CLI get full publish metadata; actual publishing starts in
v0.4 with release automation.

## D-010 — Media policy: no new binary media in git
**Date:** 2026-07-08 · **Context:** ~18 MB of demo MP4/GIFs are tracked;
history rewrite was considered and rejected (public repo, disruption >
benefit). **Decision:** existing media stays; **new** demo media goes to
GitHub Release assets (or LFS if it must live in-tree). README may hotlink
release assets.

## D-011 — Release automation via release-plz (target v0.4)
**Date:** 2026-07-08 · **Decision:** adopt release-plz for version bumps,
changelog generation, and crates.io publishing (fits Conventional Commits
history). cargo-dist deferred until CLI binary distribution matters (v0.6).
