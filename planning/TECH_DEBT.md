# Technical Debt Register

Sourced from the 2026-07-08 full-repo audit. Items are closed **only** by
linking the fixing PR. New debt gets the next TD-id immediately when found.

| ID | Title | Severity | Blast radius | Target | Status |
|----|-------|----------|--------------|--------|--------|
| TD-01 | Vello backend missing gradients/shadows/rounded-rects/dashes | High | Silent CPU↔GPU visual divergence | v0.4 | Open |
| TD-02 | SVG-path + color parsers duplicated across backends | High | Every parity fix implemented twice | v0.4 | Open |
| TD-03 | `Timeline::get_state_at` clones every property every frame | Medium | Render/export throughput | v0.5 | Open |
| TD-04 | evalexpr context built inside Plot sample loop | Medium | Plot-heavy scene throughput | v0.5 | Open |
| TD-05 | `rayon` declared-but-unused; export fully serial | Medium | Export wall-time (embarrassingly parallel) | v0.5 | Open |
| TD-06 | `mitex` declared-but-unused; LaTeX is Unicode substitution | Medium | Math-typesetting fidelity; honest docs | v0.6 | Open |
| TD-07 | Untyped `serde_json::Value` properties silently default on typos | High | Authoring correctness; breaking schema change | v0.6 | Open |
| TD-08 | Unknown easing names silently fall back to `linear` | Low | Authoring correctness | v0.4 | Open |
| TD-09 | Server not production-safe: permissive CORS, no auth/rate/body limits, asset-path arbitrary file read, `.unwrap()` on bind/serve/response | High | Anyone deploying `lumina-server` publicly | Safety minimum v0.4; full hardening v0.5 | Open |
| TD-10 | Zero tests: `lumina-text`, `lumina-schema`, `lumina-cli` | Medium | Font fallback, serde contract, CLI regressions | v0.6 | Open |
| TD-11 | No cross-backend pixel-diff test | High | Parity is unverifiable; blocks safe refactors | v0.4 | Open |
| TD-12 | JS SDK unbuildable (`../wasm/` import missing, no wasm-pack wiring); `node_modules` committed | High | JS SDK unusable from clean checkout | v0.5 | Open |
| TD-13 | Python SDK: version drift, no tests, unpublished; webm/gif not exposed | Medium | Python users | drift fixed 2026-07-08; rest v0.6 | Open |
| TD-14 | CI gaps: no MSRV/`rust-version`, Linux-only, wasm tests + benches never run, no release automation, no dependabot, no concurrency-cancel | Medium | Regressions land undetected; manual releases | v0.4 (dependabot: hygiene batch) | Open |
| TD-15 | Rustdoc: most `pub` items lack `///`; no `missing_docs` lint | Medium | docs.rs quality; API discoverability | Crate-level `//!` in hygiene batch; full fill v0.4 | Open |
| TD-16 | Examples hardcode Linux font paths (`/usr/share/fonts/...`) | Medium | macOS/Windows users can't run examples | v0.4 (bundle OFL font) | **Closed** ([#10](https://github.com/SakarZaidan/lumina/pull/10)) |
| TD-17 | `ttf-parser` 0.21 unmaintained (RUSTSEC-2026-0192) yet parses untrusted font files; pinned via fontdue 0.9 / resvg 0.42 | Medium | Font-parsing bugs won't get upstream fixes | v0.4 (bump fontdue/resvg to versions on a maintained fork) | Open |

## Notes per item

- **TD-01/TD-02/TD-11** are one cluster: extract shared
  `lumina-renderer/src/common/` (SVG-path parser, color parser) *first*, then
  implement Vello gradients/shadows/rounded/dash once, gated by a
  tolerance-based pixel-diff harness rendering the same scenes on both
  backends. Risk if deferred: every new visual feature doubles the divergence.
- **TD-03/TD-04/TD-05** are sequenced *after* TD-11 deliberately: performance
  rewrites of the hot path are unsafe without visual regression detection.
- **TD-07** is the largest pre-1.0 breaking change; it gets the most mature
  test suite (post-v0.5) and a schema migration guide.
- **TD-09** interim stance is documented honestly in `SECURITY.md`: do not
  expose `lumina-server` to untrusted networks before v0.5.
- **TD-13**: `pyproject.toml` 0.2.0→0.3.0 drift fixed in the 2026-07-08
  hygiene batch (see STATUS.md); tests/publishing remain open.
