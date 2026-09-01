# 10 — Testing and verification

## Current state

120 test functions, deterministic, no flakes. The centrepiece is genuinely
sophisticated: `crates/lumina-renderer/tests/backend_parity.rs` renders 16
fixtures on both backends and compares them with an antialiasing-aware
comparator — a 3×3 neighbourhood rescue for edge pixels, plus a mean-delta
check that catches systematic gamma or blend tints the neighbourhood match
would forgive. Failures write both frames and a heat map to
`target/parity-failures/`. Per-fixture tolerance structs carry four
independent knobs.

`tests/duplication_gate.rs` asserts on source text to keep deduplicated logic
deduplicated — an architectural invariant enforced as a test.

`LUMINA_REQUIRE_VELLO=1` turns "no wgpu adapter" from a silent skip into a
hard failure, so parity cannot quietly stop being checked.

That is a strong foundation with five holes in it.

**Whole crates have no tests.** `lumina-text` (the font fallback walk and
`measure_width` — the hottest path in the engine), `lumina-schema` (the entire
public wire format), `tools/lumina-cli`, and `sdks/python` — which is not even
a workspace member, so `cargo clippy --workspace` has never seen it. TD-10
tracks the first three. `lumina-wasm`'s ~200 lines of hit-test geometry
(`point_in_polygon`, `point_to_segment_dist`, the SVG-path bbox walker at
`:484-510`) have three external tests and no unit tests.

**No property testing.** No `proptest`, `quickcheck`, or `arbitrary`. This is
the clearest gap in the whole program, because the domain is unusually
well-suited: easing functions have crisp invariants (`f(0) == 0`, `f(1) == 1`,
monotonicity for the non-elastic family, boundedness for `spline`), and
`parse_svg_path`, `latex_to_unicode`, and `interpolate_value` are total
functions over adversarial input. The unsorted-keypoint and out-of-range-bezier
defects in [04](04-math-physics-accuracy.md) would both fall out of a
twenty-line proptest.

**No fuzzing.** No `fuzz/` directory. The engine parses three untrusted
formats — LSF JSON, SVG path data, TTF font files — and carries a live
unmaintained-parser advisory on the last of them (TD-17).

**No coverage measurement.** `planning/METRICS.md` records "n/a — tooling
lands v0.4". It has not.

**Benchmarks exist and never run.** Three criterion groups, zero CI
invocations, so no hot path has a regression gate (TD-14 remainder).

**Nothing renders an example.** Principle #12 calls a broken example a broken
build; nothing checks.

**Determinism is asserted, not proven across platforms.** There are
deterministic-render tests within a run. There is no test that the same scene
produces identical bytes on Linux, macOS, and Windows — which is the actual
promise in VISION.md.

## Target

Every invariant the project claims is checked by something that fails when it
stops being true.

## Work items

| ID | Item | Acceptance |
|---|---|---|
| `AAA-TEST-01` | `proptest` on easing: boundary values, monotonicity, boundedness, parameter domains | The four solver defects in [04](04-math-physics-accuracy.md) are caught by tests, not by reading |
| `AAA-TEST-02` | `proptest` on `interpolate_value`: totality, NaN behaviour, array padding, colour round trips | No input produces a panic or a silent `null` |
| `AAA-TEST-03` | Fuzz targets: `parse_svg_path`, `latex_to_unicode`, `Scene` deserialisation | Running in CI on a time budget; corpus committed |
| `AAA-TEST-04` | Unit suites for `lumina-text`, `lumina-schema`, `lumina-cli` (TD-10) | Font fallback order, serde contract, and CLI argument handling all covered |
| `AAA-TEST-05` | `sdks/python` joins the workspace or gets its own CI job | Linted and tested like everything else |
| `AAA-TEST-06` | `cargo-llvm-cov` in CI with a Codecov badge; ≥ 85% gate | The `n/a` row in METRICS becomes a number |
| `AAA-TEST-07` | Criterion in CI as a regression gate, > 5% fails | Hot paths cannot silently regress |
| `AAA-TEST-08` | Adversarial fixtures for every `AAA-SEC-*` bound | Each fails without its fix |
| `AAA-TEST-09` | CI renders every example | Principle #12 becomes enforceable |
| `AAA-TEST-10` | Cross-platform determinism test: identical bytes on three OSes | The core promise of VISION.md is verified, not asserted |
| `AAA-TEST-11` | `insta` for the structured outputs (validation responses, schema, `/objects`) | Contract changes are visible in review as a diff |
| `AAA-TEST-12` | Behavioural parity: backends agree on *errors*, not just pixels | Closes the class of bug the pixel suite structurally cannot see |

`AAA-TEST-12` deserves emphasis. The parity suite compares frames that both
backends produced; when one backend errors and the other silently skips
(`skia_backend.rs:461` vs `vello_backend.rs:392`), there is no frame to
compare and the divergence is invisible. Parity must cover the error path.

## Metrics moved

Testing (78 → 96), Benchmarks (60 → 95), and the coverage row.

## Sequencing

`AAA-TEST-01`, `02`, `03` in Wave 2, written *before* the fixes in
[04](04-math-physics-accuracy.md) so they fail first. `08` in Wave 1 with the
security bounds. `06`, `07`, `09` in Wave 3. `04`, `05`, `11`, `12` in Wave 5.
`10` in Wave 8, where cross-platform determinism is the v1.0 exit criterion.
