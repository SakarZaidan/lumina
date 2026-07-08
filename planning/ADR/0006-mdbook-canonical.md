# ADR-0006 — The mdBook is canonical for user-facing docs

- **Status:** Accepted · **Date:** 2026-07-08

## Context
Three architecture documents existed (`docs/ARCHITECTURE.md`, `docs/SPEC.md`,
`docs/src/architecture.md`) and contradicted each other (GPU-native /
WASM-WebGPU / MiTeX overclaims; JSON-Schema draft mismatch).

## Decision
`docs/src/` (mdBook) is the single source of user-facing truth; the legacy
files became pointer stubs (kept so external links don't 404). planning/
never duplicates book content.
