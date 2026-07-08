# Engineering Workflow

**Rule zero: never add an AI as author, co-author, committer, contributor, or
maintainer. No `Co-Authored-By` or "generated with" trailers in any commit
message or PR body — strip tool defaults. All work is attributed to the
repository owner. (D-002)**

## Branches

- `main` — always releasable; protected in spirit: only merge green PRs.
- Work branches: `feat/<slug>`, `fix/<slug>`, `docs/<slug>`, `ci/<slug>`,
  `perf/<slug>`, `release/<version>` (release prep), `hotfix/<slug>` (branched
  from the release tag, merged back to `main`).

## Commits — Conventional Commits

`<type>(<scope>): <imperative summary ≤ 72 chars>`

- Types: `feat`, `fix`, `docs`, `test`, `perf`, `refactor`, `ci`, `chore`,
  `bench`.
- Scopes: crate short names (`core`, `renderer`, `schema`, `server`, `wasm`,
  `export`, `text`, `cli`), `book`, `planning`, `sdk-js`, `sdk-py`,
  `examples`. Omit when repo-wide.
- Breaking changes: `!` after the scope + a `BREAKING CHANGE:` footer.
- One logical change per commit; keep `git mv` and heavy content edits in
  separate commits to preserve rename detection.

## Pull requests

- Target `main`; merge with a **merge commit** (matches existing history).
- Fill `.github/pull_request_template.md`. Before requesting review, all of:

```bash
cargo fmt --all --check
RUSTFLAGS="-D warnings" cargo clippy --workspace --exclude lumina-wasm --all-targets
cargo test --workspace --exclude lumina-wasm --exclude lumina-bench
RUSTFLAGS="-D warnings" cargo doc --workspace --no-deps
mdbook build docs        # when docs/src changed
```

- Update in the same PR: `CHANGELOG.md` (`[Unreleased]`), affected book
  chapters, `planning/STATUS.md`, and `planning/TECH_DEBT.md` (close items by
  linking the PR; register new debt found).
- No new `unwrap`/`expect`/`panic!`/`unsafe` in production code.
- No new binary media in git (D-010).

## Releases

1. Ensure `[Unreleased]` in CHANGELOG.md is complete; retitle it to the
   version + date; add the compare link in the footer.
2. Bump `[workspace.package] version` (and `sdks/python/pyproject.toml` — keep
   them in lockstep).
3. PR → green CI → merge.
4. On freshly pulled `main`: `git tag -a vX.Y.Z -m "Lumina vX.Y.Z — <headline>"`,
   `git push origin vX.Y.Z`.
5. `gh release create vX.Y.Z` with the CHANGELOG section as notes; attach
   demo media here rather than committing it (D-010).
6. Add a `planning/STATUS.md` entry; revise `planning/ROADMAP.md` phases.
7. Never move a pushed tag (D-008). From v0.4: release-plz automates 1–5
   (D-011).

## Review gates

Every PR is judged on: correctness (tests prove the change), architecture
(respects crate layering; no upward deps), quality (no panic paths, no
duplication across backends without a TD entry), docs (book + rustdoc updated),
performance (no per-frame allocations added to hot paths without measurement),
and security (server input handling, path access).
