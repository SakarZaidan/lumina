# Repository Metrics

Updated **at every release** by the release manager (it's a checklist step in
[AI/release-manager.md](AI/release-manager.md)) — a stale metrics file is
worse than none. One column per release so improvement (or regression) is
visible at a glance. Methodology for each metric is defined below the table;
change the methodology only with a note, or the columns stop being
comparable.

## Snapshot

| Metric | v0.3.0 (2026-07-08) |
|---|---|
| Rust LOC (all workspace + sdks src) | 9 885 |
| — lumina-renderer / core / server | 4 347 / 2 482 / 744 |
| Tests passing (native + wasm) | 92 + 3 |
| Test coverage | n/a — tooling lands v0.4 (`cargo-llvm-cov`) |
| Benchmarks (criterion groups) | 3 (timeline_eval, skia_render, easing) — manual, not in CI |
| Locked dependencies | 428 |
| `unsafe` blocks in production code | **0** |
| `unwrap()`/`expect()`/`panic!` in production code | **0** |
| Public API items (approx, grep) | 129 |
| Rustdoc `///` lines / crate-level `//!` entry points | 171 / 9 of 9 |
| Cold `cargo check --workspace` | 14.7 s |
| Release build, CLI (`cargo build --release -p lumina-cli`) | 33.4 s |
| CLI binary size (release, unstripped) | 22.9 MB |
| CI wall time (longest job, release run) | 2 m 35 s (Tests) |
| CI job spread | fmt 16s · deny 25s · book 5s · docs 39s · clippy 2m27s · tests 2m35s · wasm 1m55s |
| MSRV | 1.88 (lockfile-dependent; probed) |
| Releases to date / cadence | 3 (Apr 30, May 9, Jul 8 2026) |
| Memory profile | n/a — tooling planned v0.5 (heaptrack on export run) |

## Methodology

- **LOC**: `find crates tools sdks -name "*.rs" -not -path "*/target/*" | xargs wc -l`
  (includes in-crate test files; excludes generated/target).
- **Tests**: sum of `cargo test --workspace --exclude lumina-wasm --exclude
  lumina-bench` results, plus `wasm-pack test --node` count.
- **unsafe / panics**: `grep -rn` over `crates/*/src tools/*/src` excluding
  `#[cfg(test)]` files — the PR gate keeps these at zero
  ([ENGINEERING_PRINCIPLES](../ENGINEERING_PRINCIPLES.md) #2).
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

| Dimension | v0.3.0 | Why |
|---|---|---|
| Architecture | 90 | Clean crate layering, zero panics/unsafe; held back by backend code duplication (TD-02) |
| API design | 75 | Small deliberate surface, good errors; untyped `Value` properties (TD-07) and silent easing fallback (TD-08) |
| Documentation | 85 | Book current, crate docs everywhere, honest parity table; per-item rustdoc still sparse (TD-15) |
| Testing | 78 | 92 deterministic tests incl. golden pixels; zero tests in text/schema/CLI (TD-10), no cross-backend diff (TD-11), no coverage numbers |
| Benchmarks | 60 | 3 criterion groups exist but never run in CI, no GPU/export benches (TD-14) |
| Performance | 65 | Deterministic and adequate, but unmeasured in CI, single-threaded export, known per-frame allocation churn (TD-03/04/05) |
| Security | 55 | Library crates defensive; server unhardened by design pre-v0.5 (TD-09) — honestly documented in SECURITY.md |
| Developer experience | 82 | One-command build/test, good CLI errors, examples indexed; stringly CLI args, font-path portability (TD-16) |
| Examples | 85 | 9 scenes + 2 generators, all indexed with commands; not portable off Linux yet |
| CI / release engineering | 80 | 8-job CI, deny, dependabot, tagged releases with assets; Linux-only, no MSRV job, no release automation yet (ADR-0011) |

**Overall trajectory target:** every dimension ≥ 85 by v1.0; nothing below 70
after v0.5. The lowest three scores (Security, Benchmarks, Performance) map
exactly to the v0.4–v0.5 roadmap phases — by design.
