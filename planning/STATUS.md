# Current State

## Health dashboard

Updated with every entry below (and re-verified at every release). 🟢 healthy
· 🟡 known gaps, tracked · 🔴 broken/blocked.

| Area | | Notes |
|---|---|---|
| CI on `main` | 🟢 | 10 jobs green on ubuntu/macos/windows + MSRV + wasm |
| Tests | 🟢 | 117 native + 3 wasm passing; zero flakes |
| Coverage | 🟡 | still not measured — retargeted to v0.5 (`AAA-TEST-06`) |
| Benchmarks | 🟡 | exist, manual only; not in CI (TD-14 remainder, `AAA-TEST-07`) |
| Docs (book + rustdoc) | 🟢 | book live on Pages; every public item documented, lint-enforced |
| Examples | 🟢 | portable on any OS; CI renders none of them yet (`AAA-TEST-09`) |
| Security | 🟡 | server unhardened pre-v0.5 by design (TD-09); five audited DoS vectors open (`AAA-SEC-01..05`) |
| Backend parity | 🟢 | full visual parity, 16-fixture pixel-diff suite gating in CI; Windows probe suppressed (TD-20) |
| Release | 🟢 | v0.4.0 tagged and released |
| Distribution | 🔴 | nothing published to crates.io, PyPI, or npm — the largest open gap (`AAA-REL-*`) |
| Dependencies | 🟢 | deny green; 386 locked crates (mitex removed); rustybuzz tracked as TD-22 |

Rolling log, newest first. One dated entry per work session; ≤ 10 lines each.
For the release-by-release story see [HISTORY.md](./HISTORY.md).

---

## 2026-09-02 (later) — Wave 1: scenes are now bounded computations

- `AAA-SEC-01/02/04` landed. Every limit sits in `lumina_core::validation`,
  the one chokepoint server, CLI, and both SDKs already call, so none can be
  bypassed by reaching the renderer another way.
- Bounded: canvas dimension, fps, duration, **the product** `duration x fps`
  (each factor can be reasonable while the product is not), `sample_count`,
  `function_str` length, `Particles.count`, derived tick counts, and group
  nesting depth. Non-positive and non-finite tick steps rejected outright —
  `x_step: 0.0` produced `inf as i32`, saturating to `i32::MAX`.
- Group depth mattered most: a **straight** chain trips no cycle check, and
  8 MiB of JSON encodes ~150 000 levels. That overflowed the stack during
  *validation*, aborting the process before any render limit applied. Both
  renderers carry the limit independently since `lumina-renderer` is a public
  API callable without validating first.
- 12 adversarial fixtures added, each asserting a specific new error code.
  Tests 117 → 129. The whole scene corpus — 9 examples, 16 parity fixtures —
  still validates, so no false positives.
- Cycle detection also stopped allocating a `String` per node visited
  (`path.contains(&id.to_string())` was O(n²) allocations); Vello's scene walk
  gained a `WalkCtx` so recursion carries only what varies.
- New debt: TD-23, `ParticlesProps` emitter fields lack `#[serde(default)]`,
  contradicting the documented schema convention. Found writing the fixtures.

## 2026-09-02 — **v0.4.0 released**; the AAA programme opens

- The ten-PR stack landed. Merging it collapsed the base chain — `--delete-branch`
  removed each PR's parent, so several merged into their parent branch and the
  odd-numbered ones auto-closed. Recovered by opening #22 from the surviving,
  CI-green stack head; every individual commit is preserved, so the per-TD
  trail and `git bisect` both still work. Lesson recorded: never
  `--delete-branch` while merging a stack.
- `main` went from 31 to 78 commits and now carries everything v0.4 promised:
  backend parity, the 16-fixture pixel-diff suite, the server safety minimum,
  the 3-OS matrix, and 416 documented public items.
- Repository made findable at last: description, homepage, and 20 topics set;
  Discussions on; the unused Wiki and Projects off; branch deletion on merge.
- METRICS refreshed with a v0.4.0 column, and one honest correction: the
  v0.3.0 "0 panicking calls in production" row was wrong. There are 2, both
  provably guarded. `AAA-CQ-02` replaces the grep with a lint.
- **`plan/`** opens the AAA programme: a master plan and fourteen dimension
  subplans, each citing `file:line` evidence. `planning/ROADMAP.md` remains
  the single schedule of record (ADR-0013).

## 2026-09-01 (later) — Fresh advisories triaged; mitex dropped

- Seven idle weeks produced two new RUSTSEC hits, both caught only because a
  PR happened to run — the case for a scheduled audit rather than a
  push-triggered one.
- RUSTSEC-2026-0235 reached the workspace through exactly one path, and that
  path was `mitex` — declared since v0.1, imported by nothing (TD-06). Removed
  rather than ignored: the vulnerability is gone, not suppressed, and the
  locked tree fell **428 → 386 crates**. Decision recorded as ADR-0012;
  TD-06 closed.
- RUSTSEC-2026-0206 (`rustybuzz`, via usvg/resvg 0.47) has no upgrade to take —
  resvg 0.47 is current. Ignored with reasoning and registered as TD-22, since
  it shapes text inside untrusted SVG assets.
- Also dropped a stale ignore: RUSTSEC-2025-0057 (fxhash) no longer matches
  any crate in the tree.
- `cargo deny check`: advisories ok, bans ok, licenses ok, sources ok.

## 2026-09-01 — First matrix run triaged; two latent bugs fixed

- The 3-OS matrix and the wasm suite had never run before this PR, and both
  failed on their first outing. Windows: DX12-WARP aborts the renderer test
  process (exit 2173, two tests never reporting) instead of returning an error
  the skip path can catch — probe now suppressed by `LUMINA_DISABLE_VELLO=1`,
  registered as TD-20.
- WASM: scenes were built with `serde_wasm_bindgen::to_value`, which emits a
  JS `Map`; `Scene` reads every field as absent from one, so all three tests
  died on `missing field version`. Tests now use `JSON.parse` — the path the
  JS SDK actually takes (`new LuminaEngine(scene as object)`).
- That exposed a real engine bug behind it: `hit_test` tested every object in
  world space and let a `Group` answer for its own children, so **nothing
  inside a group was ever clickable**. It now walks roots only, descends
  through groups, and returns the deepest object. Group scale/rotation still
  unapplied to the hit point (TD-21).
- Local gate green: fmt, clippy `-D warnings`, 117 native tests under both CI
  env configurations, 3 wasm tests, 16-fixture parity suite under
  `LUMINA_REQUIRE_VELLO=1`.

## 2026-07-13 (evening) — Dep bumps; v0.4 code work complete

- PR [#19](https://github.com/SakarZaidan/lumina/pull/19) (stacked on #18):
  resvg 0.47 + tiny-skia 0.12 — untrusted-SVG path off ttf-parser 0.21;
  fontdue (latest release) still pins it → TD-17 retargeted to v0.5 with
  TD-18. Parity suite = regression net; all green incl. wasm + deny.
- **v0.4 roadmap code items all done** (7 TD closed, 2 partial-by-design).
  Open PR stack: #10→#11→#12→#13→#14→#15→#16→#17→#18→#19, merge in order.
- Remaining before v0.4.0 release: merge stack, owner enables release-plz
  tokens (ADR-0011), CHANGELOG retitle, tag, METRICS refresh.

## 2026-07-13 (later still) — Rustdoc fill (TD-15)

- PR [#18](https://github.com/SakarZaidan/lumina/pull/18) (stacked on #17):
  416 undocumented public items filled across all six library crates;
  `#![warn(missing_docs)]` per crate + CI `-D warnings` = enforcement.
- lumina-schema fields now state units/semantics — docs.rs becomes a
  real authoring reference. Per-crate commits, dependency order.
- v0.4 code work remaining: dep bumps (TD-17). Then release prep.

## 2026-07-13 (later) — CI foundations (TD-14)

- PR [#17](https://github.com/SakarZaidan/lumina/pull/17) (stacked on #16):
  3-OS test matrix (require-Vello Linux-only for now), MSRV 1.88 job,
  concurrency-cancel, wasm-bindgen tests now actually run (Node).
- First-ever macOS/Windows runs — watch this PR's matrix legs for
  platform surprises; fixes land on the same branch.
- TD-14 remainder: criterion-in-CI (v0.5), release automation (owner).

## 2026-07-13 — Server safety minimum (TD-09 part 1)

- PR [#16](https://github.com/SakarZaidan/lumina/pull/16) (stacked on #15):
  `/render` asset paths confined to `LUMINA_ASSET_ROOT` (canonicalize →
  prefix check; was an arbitrary-file read), 8 MiB body cap (413),
  bind/serve/response no longer panic. SECURITY.md updated.
- Verified live with curl: escape → 400, 9 MB body → 413.
- v0.4 remaining: CI matrix (TD-14), rustdoc (TD-15), dep bumps (TD-17);
  release-plz blocked on owner.

## 2026-07-12 (night, latest) — Easing strictness; **WS-02 complete**

- PR [#15](https://github.com/SakarZaidan/lumina/pull/15) (stacked on #14):
  UNKNOWN_EASING validation error with did-you-mean suggestion; registry
  `EASING_NAMES` + drift-guard test; validation moved to
  `lumina_core::validation` (server re-exports, CLI + SDKs share it).
- CLI validates before every render; new `--check` flag (exit 1 on
  errors). Whole scene corpus passes; typo'd easing verified rejected.
- **WS-02 Done** — all four acceptance criteria met. v0.4 remaining:
  server safety (TD-09p1), CI matrix (TD-14), rustdoc (TD-15), dep bumps
  (TD-17); release-plz blocked on owner tokens.
- PR stack open for review/merge in order: #10 → #11 → #12 → #13 → #14 → #15.

## 2026-07-12 (night, later) — Vello shadows; TD-01 + TD-11 closed

- PR [#14](https://github.com/SakarZaidan/lumina/pull/14) (stacked on #13):
  GPU drop shadows composite the shared blurred silhouette (identical
  bytes to CPU) as a peniko::Image under an opacity layer. No vello/wgpu
  upgrade needed — WS-02 risk resolved negative.
- Fixture set now 16 scenes (adds shadows, plot/axes, SVG asset, combined
  showcase); suite caught + fixed a second real bug: vello axes ticks
  drawn at grid width (1.0) instead of axis width (2.0).
- **Backend parity table complete** — every feature row ✅ on both
  backends. WS-02 scope 1–3 done; only easing strictness (TD-08) remains.

## 2026-07-12 (night) — Vello gradients, rounded rects, dash (TD-01 pt 1)

- PR [#13](https://github.com/SakarZaidan/lumina/pull/13) (stacked on #12):
  GPU backend renders gradient fills+strokes (peniko brushes, shared bbox
  geometry), rx/ry rounded rects (shared quad-arc paths), and
  draw_fraction via kurbo dashes. Fixes gradients silently rendering
  solid white on vello.
- Fixtures 05/06/08/09 added; 12-fixture parity suite green.
- Book parity table updated; found + registered TD-19: `LineProps.dash`
  implemented by neither backend (docs had claimed CPU support).
- Remaining TD-01 gap: drop shadows → next PR closes TD-01 + TD-11.

## 2026-07-12 (later still) — Renderer common/ extraction (TD-02)

- PR [#12](https://github.com/SakarZaidan/lumina/pull/12) (stacked on #11):
  `lumina-renderer/src/common/` now owns color, SVG-path (`PathData`),
  z-order/root-sort, group+camera transforms (`Mat2x3`, f32, bit-identical
  per backend), shadow blur pipeline, fill/gradient resolution, dash.
- One extraction per commit; parity harness + full suite green after each.
- WS-02 scope 1 done; grep gate (`tests/duplication_gate.rs`) enforces it.
- Next: Vello gradients/rounded/dash (TD-01 pt 1) consuming common/.

## 2026-07-12 (later) — Pixel-diff parity harness live (TD-11)

- PR [#11](https://github.com/SakarZaidan/lumina/pull/11) (stacked on #10):
  cross-backend harness renders 8 fixtures on Skia + Vello, AA-aware
  comparator (3×3 neighborhood rescue + mean-delta tint check), failure
  artifacts to `target/parity-failures/` and CI upload.
- First real catch, fixed in-PR: Vello stroked with kurbo's round caps/joins
  vs Skia's butt/miter — all GPU line ends and sharp corners diverged.
- CI test job now installs lavapipe and sets `LUMINA_REQUIRE_VELLO=1`
  (missing adapter = failure, not silent skip).
- New debt TD-18: duplicated text layout paths (Skia inline vs raster.rs
  bitmap); text fixture carries a wider tolerance until unified (v0.5).
- WS-02 → In progress. Next: `common/` extraction (TD-02).

## 2026-07-12 — v0.4 kickoff: bundled OFL font (TD-16)

- v0.4 execution started per ROADMAP/WS-02; PR sequence planned A–J
  (font → parity harness → common/ extraction → vello parity → easing
  strictness → server safety → CI matrix → rustdoc → dep bumps).
- PR [#10](https://github.com/SakarZaidan/lumina/pull/10): Liberation Sans
  2.1.5 (SIL OFL 1.1) bundled at `examples/assets/fonts/`; all scenes/docs
  off `/usr/share/fonts`. Closes TD-16.
- Latent bug found + fixed in-PR: hello/circle_bounce/pythagorean declared no
  font asset — their text never rendered (no system-font fallback exists).
- Local gate green: fmt, clippy `-D warnings`, 92/92 tests, rustdoc, mdBook.

## 2026-07-08 (evening) — Constitution, RFC/ADR system, metrics, diagrams

- Added the constitution set at root: VISION.md, DESIGN.md,
  ENGINEERING_PRINCIPLES.md (linked from README and CONTRIBUTING).
- DECISIONS.md split into per-decision `planning/ADR/0001–0011`; DECISIONS.md
  is now the index. New `planning/RFCS/` process gates public-API changes.
- New `planning/METRICS.md` (measured v0.3.0 snapshot + quality scorecard)
  and the health dashboard above; both are release-checklist duties now.
- New `planning/ECOSYSTEM.md` (layers 0–3, what the core owes the ecosystem).
- `docs/architecture/`: gen-diagrams.sh renders the crate dependency graph
  from `cargo metadata` + 4 hand-maintained pipeline diagrams; embedded in
  the book's architecture chapter.

## 2026-07-08 (later) — v0.3.0 released

- PR #2 merged to `main` as `9c35474`; CI fully green (a fresh RUSTSEC batch
  was fixed in-flight: anyhow → 1.0.103, crossbeam-epoch → 0.9.20,
  ttf-parser unmaintained ignored + registered as TD-17).
- GitHub Pages enabled by the owner; book live at
  <https://sakarzaidan.github.io/lumina/> including the new Events chapter.
- Tags pushed: `v0.1.0` (02b92da), `v0.2.0` (596b847), `v0.3.0` (9c35474);
  GitHub Release v0.3.0 created with showcase media as assets (ADR-0010).
- WS-01 complete. Next up: WS-02 backend parity (v0.4) — see ROADMAP.

## 2026-07-08 — Repo audit, planning system, hygiene batch

- Full three-track engineering audit completed (core crates, tooling/CI, docs/git).
- This planning system created; `todo.md` retired into [ROADMAP.md](./ROADMAP.md);
  blueprint and history moved under `planning/`.
- Hygiene batch in flight on `feat/v0.3.0-enhancements`: metadata fixes, crate
  rustdoc, README/CHANGELOG repair, community health files, mdBook v0.3.0 refresh.
- Git state at session start: `origin/main` = `7af221a` (v0.3.0 merge, **CI red
  6/8** — fixed by the two `ci:` commits on this branch); no tags existed.
- Batch complete: 10 commits; local gate green (fmt, clippy `-D warnings`,
  92/92 tests, rustdoc clean, mdBook 0.4.40 builds).
- MSRV probed: **1.88** (1.78 and 1.85 fail on locked deps `home`/`image`);
  declared as `rust-version` and reflected in the README badge.
- Next: PR → green CI on `main` → tag `v0.1.0` (02b92da) / `v0.2.0` (596b847)
  backdated + `v0.3.0` on the green merge → GitHub Release for v0.3.0.
- Blocked on repo owner: enable GitHub Pages (Settings → Pages → Source:
  GitHub Actions) so the deploy-docs job can publish the book.
- Version: 0.3.0 across workspace and both SDK manifests (drift fixed).
