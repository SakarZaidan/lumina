# ADR-0011 — Release automation via release-plz (target v0.4)

- **Status:** Accepted (not yet implemented) · **Date:** 2026-07-08

## Decision
Adopt release-plz for version bumps, changelog generation, and crates.io
publishing (fits the Conventional Commits history). cargo-dist is deferred
until CLI binary distribution matters (v0.6).
