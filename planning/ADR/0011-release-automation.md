# ADR-0011 — Release automation via release-plz (target v0.4)

- **Status:** Superseded in part by [ADR-0014](./0014-published-crate-names.md) · **Date:** 2026-07-08
- **2026-09-03:** publishing is implemented as an ordered `release.yml` rather
  than release-plz. The names had to be settled first (ADR-0014), and an
  explicit ordered publish is what proves the manifests are correct; release-plz
  can take over version bumps and changelog generation on top of it.

## Decision
Adopt release-plz for version bumps, changelog generation, and crates.io
publishing (fits the Conventional Commits history). cargo-dist is deferred
until CLI binary distribution matters (v0.6).
