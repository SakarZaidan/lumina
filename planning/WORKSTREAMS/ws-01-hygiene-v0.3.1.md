# WS-01 — Repository Hygiene & v0.3.0 Release

**Status:** In progress (2026-07-08) · **Priority:** P0 · **Effort:** 1 session
**Linked debt:** TD-13 (drift part), TD-14 (dependabot part), TD-15 (crate-level part)

> This file doubles as the workstream template: copy its section structure for
> new workstreams.

## Goal

Make the repository's claims true and its release real: planning system in
place, metadata/docs/CHANGELOG corrected, community health files added, mdBook
current for v0.3.0, CI green on `main`, and v0.1.0–v0.3.0 tagged. **No code
changes** — code work is phased in ROADMAP.md.

## Tasks

- [x] `.gitignore` — add `/text.md` (D-003)
- [x] `planning/` system: README, ROADMAP (absorbs todo.md), STATUS,
      KNOWLEDGE_BASE, TECH_DEBT, DECISIONS, WORKSTREAMS, AI prompts;
      move blueprint + history.md under planning/
- [x] Cargo metadata: canonical repo URL (D-001), authors, per-crate
      description/keywords/categories/readme, `publish = false` on
      server+wasm (D-009); pyproject 0.2.0→0.3.0; MSRV probed = **1.88**
      (1.78 and 1.85 fail on locked deps), `rust-version` declared;
      JS SDK package.json URL/version aligned too
- [x] Crate-level `//!` rustdoc, all crates + CLI
- [x] README: live CI badge, easing-count fix (28), Pages URL, canonical links
- [x] CHANGELOG: merged duplicate [0.1.0] blocks, folded [0.2.x] into
      [0.3.0], compare links, seeded [Unreleased]
- [x] CONTRIBUTING unified; SECURITY.md, CODE_OF_CONDUCT.md, CODEOWNERS,
      dependabot.yml, issue-template config.yml
- [x] examples/README.md with portability notes
- [x] mdBook v0.3.0 refresh: events chapter, flag updates, parity table;
      legacy ARCHITECTURE.md/SPEC.md → pointer stubs (D-006)
- [ ] PR → green CI on main → merge
- [ ] Tags v0.1.0 (02b92da, backdated), v0.2.0 (596b847, backdated),
      v0.3.0 (new green merge, D-008); GitHub Release for v0.3.0
- [ ] GitHub Pages must be enabled by the repo owner (Settings → Pages →
      Source: GitHub Actions) before the deploy-docs job can succeed

## Acceptance criteria

- CI fully green on `main`; Pages serves the refreshed book incl. the Events
  chapter.
- `git tag` shows the three tags; `gh release view v0.3.0` works.
- No tracked file references `lumina-animation`, `todo.md`, or the old
  root locations of blueprint/history.
- `git check-ignore text.md` matches; `text.md` absent from all history.

## Verification

```bash
cargo fmt --all --check && RUSTFLAGS="-D warnings" cargo clippy --workspace --exclude lumina-wasm --all-targets
cargo test --workspace --exclude lumina-wasm --exclude lumina-bench
RUSTFLAGS="-D warnings" cargo doc --workspace --no-deps
mdbook build docs
grep -rn "lumina-animation\|todo\.md" --include="*.md" --include="*.toml" . | grep -v target | grep -v planning/DECISIONS
```

## Risks

- CHANGELOG [0.2.x] folding only safe pre-tags — same PR as the tag plan.
- `rust-version` claim must be probe-verified before shipping.
- Backdated tags need `GIT_COMMITTER_DATE` set to the commit author date.
