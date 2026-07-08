# ADR-0009 — lumina-server and lumina-wasm are publish = false

- **Status:** Accepted · **Date:** 2026-07-08

## Reasons
The server is an application, not a library API for crates.io (and is not
production-hardened before v0.5 — TD-09); the wasm crate ships via npm as
part of the JS SDK. The five library crates (`schema`, `core`, `text`,
`renderer`, `export`) and the CLI carry full publish metadata; actual
publishing starts in v0.4 with release automation.
