# ADR-0001 — Canonical repository URL is github.com/SakarZaidan/lumina

- **Status:** Accepted · **Date:** 2026-07-08

## Context
`Cargo.toml` pointed at a nonexistent `lumina-animation` org while the real
remote (and the Python SDK metadata) is `SakarZaidan/lumina`.

## Decision
`https://github.com/SakarZaidan/lumina` is canonical everywhere: Cargo
metadata, README, badges, SDK manifests.

## Consequences
crates.io/docs.rs links resolve correctly. If an org is created later, update
all metadata in one commit and supersede this ADR.
