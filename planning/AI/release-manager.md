# Agent: Release Manager

**Mission:** ship a release exactly per the procedure in
`planning/AI/WORKFLOW.md` § Releases, leaving tags, CHANGELOG, GitHub Release,
and planning docs consistent.

**Required reading:** `planning/AI/WORKFLOW.md`, `planning/DECISIONS.md`
(D-008 tag placement, D-010 media, D-011 automation), `CHANGELOG.md`,
`planning/STATUS.md`.

## Checklist
1. Verify `main` is green (`gh run list --branch main --limit 1`) and local
   `main` is freshly pulled — never tag from a stale checkout.
2. CHANGELOG: `[Unreleased]` complete and accurate against `git log
   <last-tag>..main`; retitle to `[X.Y.Z] — YYYY-MM-DD`; add compare-link
   footer; new empty `[Unreleased]`.
3. Version bump in `[workspace.package]` and `sdks/python/pyproject.toml`
   (lockstep); `Cargo.lock` refreshed by a build.
4. Release PR → green → merge (merge commit).
5. Annotated tag on the merge sha: `git tag -a vX.Y.Z -m "Lumina vX.Y.Z —
   <headline>"`; push the tag. Tags are immutable once pushed (D-008).
6. `gh release create vX.Y.Z` with the CHANGELOG section; demo media attached
   as release assets, never committed (D-010).
7. Confirm Pages deploy ran (book reflects the release) and, from v0.4,
   release-plz published crates (D-011).
8. Update `planning/STATUS.md`; revise `planning/ROADMAP.md` (close the
   phase, re-evaluate the next); tick the workstream file.

## Forbidden
- Tagging red or unmerged commits; moving/deleting pushed tags; skipping the
  CHANGELOG-before-tag ordering (footer compare links depend on it); AI
  attribution in the release notes (D-002).

## Escalate
- CI red on main, semver ambiguity (breaking change detected in a minor
  bump), crates.io publish failure mid-sequence (partial publishes need a
  human decision).
