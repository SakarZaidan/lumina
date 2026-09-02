# Technical Debt Register

Sourced from the 2026-07-08 full-repo audit. Items are closed **only** by
linking the fixing PR. New debt gets the next TD-id immediately when found.

| ID | Title | Severity | Blast radius | Target | Status |
|----|-------|----------|--------------|--------|--------|
| TD-01 | Vello backend missing gradients/shadows/rounded-rects/dashes | High | Silent CPU↔GPU visual divergence | v0.4 | **Closed** ([#13](https://github.com/SakarZaidan/lumina/pull/13), [#14](https://github.com/SakarZaidan/lumina/pull/14)) |
| TD-02 | SVG-path + color parsers duplicated across backends | High | Every parity fix implemented twice | v0.4 | **Closed** ([#12](https://github.com/SakarZaidan/lumina/pull/12)) |
| TD-03 | `Timeline::get_state_at` clones every property every frame | Medium | Render/export throughput | v0.5 | Partly addressed ([#68](https://github.com/SakarZaidan/lumina/pull/68) — one build instead of two, binary keyframe search, −34–42%). The per-property `Value` clone remains: eliminating it needs caching across frames, which is the larger half |
| TD-04 | evalexpr context built inside Plot sample loop | Medium | Plot-heavy scene throughput | v0.5 | **Closed** ([#63](https://github.com/SakarZaidan/lumina/pull/63) — expression parsed once per plot in shared `common/plot.rs`) |
| TD-05 | `rayon` declared-but-unused; export fully serial | Medium | Export wall-time | v0.5 | **Closed** ([#71](https://github.com/SakarZaidan/lumina/pull/71) — render/encode pipelined; PNG compression on a rayon pool. The "embarrassingly parallel" framing was wrong: video export is ffmpeg-bound, so N-way parallel rendering buys almost nothing there) |
| TD-06 | `mitex` declared-but-unused; LaTeX is Unicode substitution | Medium | Math-typesetting fidelity; honest docs | v0.6 | **Closed** ([#19](https://github.com/SakarZaidan/lumina/pull/19) — dependency removed, ADR-0012) |
| TD-07 | Untyped `serde_json::Value` properties silently default on typos | High | Authoring correctness; breaking schema change | v0.6 | Open |
| TD-08 | Unknown easing names silently fall back to `linear` | Low | Authoring correctness | v0.4 | **Closed** ([#15](https://github.com/SakarZaidan/lumina/pull/15)) |
| TD-09 | Server not production-safe: permissive CORS, no auth/rate/body limits, asset-path arbitrary file read, `.unwrap()` on bind/serve/response | High | Anyone deploying `lumina-server` publicly | Safety minimum v0.4; full hardening v0.5 | **Closed** — part 1 ([#16](https://github.com/SakarZaidan/lumina/pull/16)): allowlist, 8 MiB cap, no panics. Part 2 (v0.5): bearer auth with a constant-time comparison, per-client rate limiting, CORS allowlist (was `permissive`), loopback default bind (was `0.0.0.0`), graceful shutdown, and one error envelope across every endpoint including `axum`'s own extractor rejections. Remaining and documented: the limiter is per-process and keyed by peer address |
| TD-10 | Zero tests: `lumina-text`, `lumina-schema`, `lumina-cli` | Medium | Font fallback, serde contract, CLI regressions | v0.6 | Mostly addressed — `lumina-text` ([#69](https://github.com/SakarZaidan/lumina/pull/69)); `lumina-cli` now has a library target and 13 tests, taking its logic from 0% to 82% coverage. `lumina-schema` still has none, though it is exercised throughout by every scene fixture |
| TD-11 | No cross-backend pixel-diff test | High | Parity is unverifiable; blocks safe refactors | v0.4 | **Closed** ([#11](https://github.com/SakarZaidan/lumina/pull/11), [#14](https://github.com/SakarZaidan/lumina/pull/14) — 16-fixture suite in CI; raster-Image asset fixture deferred with TD-18) |
| TD-12 | JS SDK unbuildable (`../wasm/` import missing, no wasm-pack wiring); `node_modules` committed | High | JS SDK unusable from clean checkout | v0.5 | Open |
| TD-13 | Python SDK: version drift, no tests, unpublished; webm/gif not exposed | Medium | Python users | drift fixed 2026-07-08; rest v0.6 | Open |
| TD-14 | CI gaps: no MSRV/`rust-version`, Linux-only, wasm tests + benches never run, no release automation, no dependabot, no concurrency-cancel | Medium | Regressions land undetected; manual releases | v0.4 (dependabot: hygiene batch) | MSRV job, 3-OS matrix, concurrency-cancel, wasm tests done ([#17](https://github.com/SakarZaidan/lumina/pull/17)); benches-in-CI done ([#65](https://github.com/SakarZaidan/lumina/pull/65)); release automation blocked on owner |
| TD-15 | Rustdoc: most `pub` items lack `///`; no `missing_docs` lint | Medium | docs.rs quality; API discoverability | Crate-level `//!` in hygiene batch; full fill v0.4 | **Closed** ([#18](https://github.com/SakarZaidan/lumina/pull/18) — 416 items documented, lint enforced via `-D warnings`) |
| TD-16 | Examples hardcode Linux font paths (`/usr/share/fonts/...`) | Medium | macOS/Windows users can't run examples | v0.4 (bundle OFL font) | **Closed** ([#10](https://github.com/SakarZaidan/lumina/pull/10)) |
| TD-17 | `ttf-parser` 0.21 unmaintained (RUSTSEC-2026-0192) yet parses untrusted font files; pinned via fontdue 0.9 / resvg 0.42 | Medium | Font-parsing bugs won't get upstream fixes | resvg done v0.4 ([#19](https://github.com/SakarZaidan/lumina/pull/19)); fontdue → v0.5 with TD-18 | SVG path clean (ttf-parser 0.25); fontdue 0.9.3 (latest) still pins 0.21 — close via fontdue release or rasterizer swap |
| TD-18 | Text layout duplicated: Skia draws glyphs inline, Vello resamples a `raster.rs` string bitmap — glyph AA and low-opacity blending diverge (text parity fixture needs a wider tolerance) | Medium | Cross-backend text fidelity, esp. under camera zoom | v0.5 | **Closed** — layout moved to `common::text`, and the GPU backend draws one image per glyph rather than resampling a whole-string bitmap. `04_text` now runs at `DEFAULT_TOL`; the duplication gate guards the layout arithmetic |
| TD-19 | `LineProps.dash` schema field implemented by neither backend (docs previously claimed CPU support) | Low | Dashed-line scenes silently render solid | v0.5 | **Closed** ([#76](https://github.com/SakarZaidan/lumina/pull/76) — both backends, shared normaliser, parity fixture 18) |
| TD-26 | No way to express "no fill" — a shape must have one, and `"none"` is now an error rather than a silent white | Low | Stroke-only shapes need an alpha-zero fill | v0.5 | Open — see #74 |
| TD-20 | Vello adapter probing aborts the process on Windows CI (DX12-WARP), so the graceful skip is never reached | Medium | Backend parity unverified on Windows | v0.5 | Open — probe suppressed via `LUMINA_DISABLE_VELLO=1` ([#17](https://github.com/SakarZaidan/lumina/pull/17)) |
| TD-21 | WASM `hit_test` applies only a group's translation, not its scale or rotation | Low | Mis-hits inside scaled/rotated groups | v0.5 | Open — needs `lumina-renderer`'s crate-private `common::scene::group_transform` shared (RFC) |
| TD-25 | Golden-pixel and parity suites both render in a single process, so they cannot detect ordering that varies *between* processes | Medium | A determinism break can pass every test — and did | v0.5 | Open — `draw_order.rs` covers the specific case found; the general gap remains |
| TD-24 | `lumina-wasm` was excluded from the workspace lint/test commands, so 8 public items went undocumented and nothing linted it | Low | The browser-facing API drifts from the rest | v0.5 | Mostly closed — clippy now covers the crate in both `xtask ci` and CI, which immediately surfaced a live hit-test determinism bug (the TD-25 defect, present a second time). `cargo test` still excludes it: those tests need the wasm target and run under `wasm-pack test --node` |
| TD-23 | `ParticlesProps.emitter_x`/`emitter_y` lack `#[serde(default)]`, unlike every other optional prop | Low | A Particles object cannot be written without them; contradicts the documented schema convention | v0.5 | Open — found writing the resource-bound fixtures |
| TD-22 | `rustybuzz` unmaintained (RUSTSEC-2026-0206) via usvg/resvg 0.47, and it shapes text inside untrusted SVG assets | Medium | SVG text-shaping bugs won't get upstream fixes | v0.5 | Open — resvg 0.47 is current; revisit on its next release |

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
- **TD-20/TD-21** were both surfaced by turning previously-unrun jobs on:
  the 3-OS matrix and the `wasm-bindgen` suite had never executed before
  #17. TD-20 is an environment defect, not a Lumina one — the probe is
  suppressed rather than the tests deleted, so parity coverage on Linux is
  unchanged and re-enabling it is a one-line revert once a Windows runner
  offers an adapter that fails cleanly.
