# Repository Metrics

Updated **at every release** by the release manager (it's a checklist step in
[AI/release-manager.md](AI/release-manager.md)) — a stale metrics file is
worse than none. One column per release so improvement (or regression) is
visible at a glance. Methodology for each metric is defined below the table;
change the methodology only with a note, or the columns stop being
comparable.

## Snapshot

| Metric | v0.3.0 (2026-07-08) | v0.4.0 (2026-09-02) |
|---|---|---|
| Rust LOC (all workspace + sdks src) | 9 885 | 11 548 |
| — lumina-renderer / core / server | 4 347 / 2 482 / 744 | 5 097 / 3 149 / 616 |
| Tests (native + wasm) | 92 + 3 | 117 + 3 |
| Test functions (incl. parity fixtures) | — | 120 |
| Test coverage | n/a — tooling lands v0.4 (`cargo-llvm-cov`) | still n/a — retargeted to v0.5 (`AAA-TEST-06`) |
| Benchmarks (criterion groups) | 3 — manual, not in CI | 3 — still not in CI (`AAA-TEST-07`) |
| Cross-backend parity fixtures | 0 | **16**, gating in CI |
| Locked dependencies | 428 | **386** (mitex removed, ADR-0012) |
| `unsafe` blocks in production code | **0** | **0** |
| `unwrap()`/`expect()`/`panic!` in production code | 0 *(see note)* | **2**, both provably guarded |
| Public API items (approx, grep) | 129 | 264 |
| Undocumented public items | 416 | **0** (`missing_docs` + `-D warnings`) |
| CI jobs / operating systems | 8 / 1 | 10 / 3 |
| MSRV | 1.88 (lockfile-dependent; probed) | 1.88, verified by a CI job |
| Releases to date / cadence | 3 (Apr 30, May 9, Jul 8 2026) | 4 (+ Sep 2 2026) |
| CLI binary size (release, unstripped) | 22.9 MB | 22.1 MB |
| Memory profile | n/a — tooling planned v0.5 | n/a — `AAA-P-*` baselines, Wave 3 |

## Methodology

- **LOC**: `find crates tools sdks -name "*.rs" -not -path "*/target/*" | xargs wc -l`
  (includes in-crate test files; excludes generated/target).
- **Tests**: sum of `cargo test --workspace --exclude lumina-wasm --exclude
  lumina-bench` results, plus `wasm-pack test --node` count.
- **unsafe / panics**: `grep -rn` over `crates/*/src tools/*/src` excluding
  `#[cfg(test)]` files and inline `mod tests`
  ([ENGINEERING_PRINCIPLES](../ENGINEERING_PRINCIPLES.md) #2).

  **Correction, v0.4.0:** the v0.3.0 row read 0 panicking calls in production
  code. The accurate count is **2**, and both are provably safe:
  `lumina-core/src/validation.rs:303` unwraps a `position()` guarded by a
  `contains` check two lines above, and
  `lumina-renderer/src/skia_backend.rs:1437` unwraps inside a
  superscript/subscript mapper whose input domain it has just checked. They
  are still `unwrap`s, and principle #2 says so. `AAA-CQ-02` replaces this
  grep with a `clippy::unwrap_used` deny outside tests, which will force both
  to be rewritten or explicitly allowed with a justification.

  The `unsafe` count is genuinely 0, but the grep now returns a false positive
  from the word appearing in a comment — which is the argument for
  `AAA-SEC-07` (`#![forbid(unsafe_code)]`): a compiler guarantee instead of a
  text search.
- **Public API**: `grep -rEh '^\s*pub (fn|struct|enum|trait|type|const)'` —
  approximate; replace with `cargo public-api` when adopted.
- **Compile times / binary size**: single machine (owner's WSL2 box), after
  `cargo clean`; comparable release-over-release only on the same hardware.
- **CI times**: from the release commit's Actions run on `main`.

## Quality scorecard

Honest, subjective 0–100 per dimension, scored against "reference-quality
open source project", not against the previous release. Re-scored at every
release with a one-line justification; movement matters more than absolute
value.

| Dimension | v0.3.0 | v0.4.0 | Why it moved |
|---|---|---|---|
| Architecture | 90 | 93 | Backend duplication closed (TD-02); four incompatible error idioms remain (`AAA-ARCH-01..04`) |
| API design | 75 | 78 | Easing names validated; properties still untyped (TD-07) and three server endpoints still return prose |
| Documentation | 85 | 93 | 416 public items documented and the lint enforces it; README claims still unverified (`AAA-CQ-01`) |
| Testing | 78 | 86 | 16-fixture cross-backend parity suite gating in CI; no property tests, fuzzing, or coverage yet |
| Benchmarks | 60 | 60 | Unchanged — still three groups, still not run by CI |
| Performance | 65 | 65 | Unchanged by design; v0.4 was non-goal territory. Wave 3 owns it |
| Security | 55 | 68 | Asset-root allowlist, body cap, no panics on bind/serve; a live vulnerability removed with its dead dependency. Five audited DoS vectors still open (`AAA-SEC-01..05`) |
| Developer experience | 82 | 86 | Portable examples, MSRV job, 3-OS matrix; the CLI is still one flat command and the gate is still five |
| Examples | 85 | 90 | All portable off Linux with a bundled OFL font; CI still renders none |
| CI / release engineering | 80 | 88 | 3-OS matrix, MSRV job, wasm suite actually running, concurrency-cancel; nothing published to any registry yet |

**Overall trajectory target (revised at v0.4.0):** the AAA programme raises
the bar to **≥ 95 on every dimension by v1.0, nothing below 85 after v0.6**,
and adds four dimensions this scorecard never had — Accuracy, Output fidelity,
Motion design, and Ecosystem. Targets and ownership per dimension are in
[plan/00-master.md](../plan/00-master.md#scorecard-targets).

The lowest three scores are now Benchmarks (60), Performance (65), and
Security (68) — which is precisely the Wave 3 and Wave 1 scope. Ecosystem,
newly scored, starts at **30**: nothing is published to any registry and no
external contributor has ever had an issue to pick up.
