# Lumina Roadmap

**North star:** the declarative, deterministic animation engine that AI systems
and humans share — JSON scene in, pixel-identical animation out, on CPU, GPU,
and the web. Reference-quality Rust: layered crates, no panics in production
code, every claim in the docs true of the code.

This is the single roadmap document (see ADR-0005). Phases are re-evaluated at
each release boundary against the repository as it then is. Debt IDs reference
[TECH_DEBT.md](./TECH_DEBT.md).

---

## v0.4.0 — Correctness, Parity & Foundations

**Goal:** the two backends provably render the same pixels; the project becomes
publishable and contributable.

**Scope**
- [x] Extract `lumina-renderer/src/common/`: shared SVG-path parser, color
  parser, scene-walk/z-sort helpers (TD-02, #12) — prerequisite for everything below.
- [x] Vello parity: gradients, drop shadows, rounded rectangles, dashed lines
  (TD-01, #13/#14).
- [x] Cross-backend pixel-diff golden harness with per-channel tolerance; runs
  in CI on a curated scene set (TD-11, #11/#14). This is the acceptance gate for parity
  and the safety net for all later refactors.
- [x] Unknown easing name → validation error instead of silent `linear`
  (TD-08, #15; mildly breaking — done while the scene corpus is small).
- [x] Server safety minimum (TD-09 part 1, #16): asset-root allowlist for
  `/render`, remove `.unwrap()` on bind/serve/response, request body-size limit.
- [ ] CI foundations (TD-14): `rust-version` in workspace + MSRV job,
  ubuntu/macos/windows test matrix, concurrency-cancel, run
  `wasm-bindgen-test` suite.
- [ ] Rustdoc fill: `///` on all public items, `#![warn(missing_docs)]`
  per crate (TD-15).
- [ ] Release automation: adopt release-plz; first crates.io publish of the
  five library crates + CLI (ADR-0009, ADR-0011).
- [x] Examples portability: bundle an OFL-licensed font under
  `examples/assets/`, stop hardcoding `/usr/share/fonts` (TD-16, #10).

**Non-goals:** performance work, server auth, typed schema.

**Exit criteria:** pixel-diff suite green on both backends across the scene
set; CI matrix green on 3 OSes; `v0.4.0` on crates.io via release-plz.

**Risks:** vello 0.2 API limits (gradient/blur support) may force a vello/wgpu
upgrade first; wgpu on macOS/Windows CI runners can be flaky — pin the CPU
fallback adapter for tests.

---

## v0.5.0 — Performance & Production Server

**Goal:** measurably faster rendering/export, and a server you may expose.

**Scope**
- [ ] Timeline state caching — stop cloning every property per frame (TD-03).
- [ ] Hoist evalexpr context out of the Plot sample loop (TD-04).
- [ ] Rayon frame-parallel export (TD-05) — finally justify the dependency.
- [ ] Benchmark expansion: GPU render + export pipeline benches; criterion in
  CI as an informational (non-gating) job (TD-14 remainder).
- [ ] Server hardening (TD-09 part 2): bearer-token auth, rate limiting,
  configurable bind address, CORS allowlist, structured request logging,
  HTTP-level integration tests via `tower::ServiceExt::oneshot`.
- [ ] JS SDK repair (TD-12): wire `wasm-pack` output into the package build,
  untrack `node_modules`, add a CI build job.

**Sequencing rationale:** performance rewrites of the hot path come *after*
v0.4's pixel-diff harness and benchmarks exist as regression gates.

**Exit criteria:** documented before/after benchmark deltas; server deployable
behind a reverse proxy with a written threat model; JS SDK builds from a clean
checkout in CI.

---

## v0.6.0 — Typed Schema, Real LaTeX & SDK Publishing

**Goal:** correctness by construction; installable SDKs.

**Scope**
- [ ] Typed property system replacing raw `serde_json::Value` flow (TD-07).
  Breaking schema change with a migration guide — must precede 1.0, and comes
  after parity so one validation layer serves both proven-equivalent backends.
- [ ] LaTeX decision (TD-06): implement real mitex-based typesetting **or**
  drop the dependency and document the Unicode-substitution approach honestly.
  Recorded in DECISIONS.md either way.
- [ ] Test-debt closure (TD-10): property tests (proptest) for
  interpolator/easing; unit suites for `lumina-text`, `lumina-schema`, CLI.
- [ ] Publish Python SDK to PyPI (maturin CI) and JS SDK to npm; expose
  webm/gif in the Python API (TD-13); cargo-dist for CLI binaries (ADR-0011).

**Exit criteria:** schema v2 documented with migration guide; `pip install`
and `npm install` work from public registries.

---

## v1.0.0 — Stability

No new features. API/semver audit; deprecation removals; external-style
security review of the server; docs completeness pass; cross-platform render
determinism verified in CI; support statement.

---

## Unallocated backlog

Absorbed from the retired `todo.md` plus deferred items; pulled into a phase
when it earns priority.

- **WASM WebGPU** — run the Vello backend in the browser player (today wasm
  uses the CPU renderer).
- **Interpolated `tween_to`** — runtime keyframe blending (currently applies
  the target immediately via the override channel).
- **Lottie export** — geometric subset → Lottie JSON for legacy players.
- **3D transform layer** — perspective + rotateX/Y/Z (card-flip first).
- **Asset pipeline** — automatic SVG/image optimization on import.
- **Self-correction loop** — CLI validate→fix→retry helper around the
  validator's `fix_suggestion` output.
- **Bundled encoder** — optional in-process encoding to drop the hard external
  ffmpeg requirement.
- **CLI ergonomics** — clap `ValueEnum` for `--format`/`--backend`,
  width/height/fps overrides, watch mode honoring `--format` and asset files.
