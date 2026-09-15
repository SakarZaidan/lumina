# Repository Metrics

Updated **at every release** by the release manager (it's a checklist step in
[AI/release-manager.md](AI/release-manager.md)) — a stale metrics file is
worse than none. One column per release so improvement (or regression) is
visible at a glance. Methodology for each metric is defined below the table;
change the methodology only with a note, or the columns stop being
comparable.

## Snapshot

| Metric | v0.3.0 (2026-07-08) | v0.4.0 (2026-09-02) | v0.5.0 (prepared 2026-09-04) |
|---|---|---|---|
| Rust LOC (all workspace + sdks src) | 9 885 | 11 548 | 22 064 |
| — lumina-renderer / core / server | 4 347 / 2 482 / 744 | 5 097 / 3 149 / 616 | 7 966 / 5 556 / 1 821 |
| Tests (native + wasm) | 92 + 3 | 117 + 3 | **311** + 3 |
| Test functions (incl. parity fixtures) | — | 120 | 311 |
| Test coverage | n/a — tooling lands v0.4 (`cargo-llvm-cov`) | still n/a — retargeted to v0.5 (`AAA-TEST-06`) | **83.4% lines / 83.8% regions**, measured in CI and gated at an 80% floor that ratchets |
| Benchmarks (criterion groups) | 3 — manual, not in CI | 3 — still not in CI (`AAA-TEST-07`) | 6, **gating in CI** with family-corroborated regression detection |
| Cross-backend parity fixtures | 0 | **16**, gating in CI | **21**, `04_text` now at the default tolerance (TD-18) |
| Locked dependencies | 428 | **386** (mitex removed, ADR-0012) | 409 (+exr/tower/http-body-util; `glam` removed, 12 unused deps deleted) |
| `unsafe` blocks in production code | **0** | **0** | **0**, compiler-enforced (`forbid`) |
| `unwrap()`/`expect()`/`panic!` in production code | 0 *(see note)* | **2**, both provably guarded | 2, unchanged |
| Public API items (approx, grep) | 129 | 264 | 300+ (server config/error, MCP, `demultiply_in_place`) |
| Undocumented public items | 416 | **0** (`missing_docs` + `-D warnings`) | **0** |
| CI jobs / operating systems | 8 / 1 | 10 / 3 | **15** / 3 (+coverage, audit ×3, CodeQL, Scorecard) |
| MSRV | 1.88 (lockfile-dependent; probed) | 1.88, verified by a CI job | 1.88, verified |
| Releases to date / cadence | 3 (Apr 30, May 9, Jul 8 2026) | 4 (+ Sep 2 2026) | 5th prepared; **first to a registry** |
| Registries published to | 0 | 0 | crates.io + PyPI **armed**, npm blocked on TD-12 |
| CLI binary size (release, unstripped) | 22.9 MB | 22.1 MB | not re-measured |
| Memory profile | n/a — tooling planned v0.5 | n/a — `AAA-P-*` baselines, Wave 3 | n/a — still unmeasured |

## Performance baselines

Captured on the maintainer's machine (WSL2) before any Wave 3 optimisation, so
later work has something to be measured against. Comparable release-over-release
only on the same hardware; CI compares a pull request against its own merge base
on one runner instead, for the reason given below.

| Benchmark | Baseline | After buffer reuse | Change |
|---|---|---|---|
| `skia_render/10` (1080p) | 5.21 ms | **0.569 ms** | −89.1% |
| `skia_render/100` | 5.42 ms | **0.744 ms** | −86.3% |
| `skia_render/500` | 6.54 ms | **1.78 ms** | −72.8% |
| `text_render/10x40` (1080p) | 6.27 ms | **1.31 ms** | −79.1% |
| `text_render/40x40` | 8.31 ms | **3.59 ms** | −56.8% |
| `plot_render/1x200` | 5.79 ms | **1.12 ms** | −80.2% |
| `plot_render/8x200` | 7.04 ms | **2.23 ms** | −68.8% |
| `plot_render/8x2000` | 7.88 ms | **2.99 ms** | −61.8% |
| `frame_sequence/30` (720p) | 18.9 ms | **16.5 ms** | −12.8% |
| `frame_sequence/120` (720p) | 75.8 ms | **65.2 ms** | −14.9% |
| `timeline_eval/100` | 97.5 µs | **56.2 µs** | −42.2% |
| `timeline_eval/500` | 558 µs | **352 µs** | −35.8% |
| `timeline_eval/1000` | 1.15 ms | **736 µs** | −36.0% |
| `timeline_eval/2000` | 2.48 ms | **1.60 ms** | −34.6% |
| `scene_walk/100` / `/1000` | 9.49 µs / 102 µs | unchanged | — |
| `scene_walk/timeline_build_100` / `/1000` | 237 µs / 2.34 ms | unchanged | — |
| `easing/get_easing_fn_lookup` | 4.35 ns | unchanged | — |
| `easing/eval_easing_cubic_bezier` | 16.7 ns | unchanged | — |

`frame_sequence` improved a further **9%** on top of the buffer-reuse figure
once timeline evaluation was cut, so the combined effect on the most
realistic measurement is roughly **−22%** from baseline.

Not everything moved the right way. Borrowing rather than cloning in
`sorted_root_ids` left `skia_render/10` and `/100` about **1% slower**
(p = 0.00, so systematic rather than noise) — the benchmark scenes have no
groups, so there was little cloning to remove, and the change is likely code
layout. It is far below the CI gate and far below the timeline win it came
with, but it is recorded rather than omitted.

**The glyph atlas returned about a fifth of what the plan predicted.**
`plan/02-performance.md` estimated 2–3 ms on a text-heavy scene; caching
rasterised glyphs delivered **−10.8%** of an already-reduced 4.15 ms, so
roughly **0.45 ms**. The estimate was made before buffer reuse had removed the
allocation that dominated everything, and it assumed outline rasterisation was
the cost. Measured, the remaining per-glyph cost is the temporary `Pixmap`
allocated for each glyph's mask plus the per-pixel colour conversion — both of
which happen per glyph per frame whether or not the outline is cached. That is
the next thing to attack in text, and it is a different change.

All other changes above significant at p = 0.00. `frame_sequence` improves least
because it runs at 720p — a smaller buffer faults in fewer pages — and because
its remaining time is timeline evaluation plus the output copy, which are the
next two targets (`AAA-P-04`, and the `render_into` half of `AAA-P-02`).

**What the first reading of these numbers changed.** `skia_render` costs
5.21 ms for ten objects and 6.54 ms for five hundred — almost flat, so nearly
all of it is fixed per-frame cost rather than drawing. Measured directly, an
8.3 MB `Pixmap` allocated and dropped every frame costs **5.3 ms/frame**, while
reusing one costs **0.57 ms/frame**: the allocator returns the block to the
operating system and every frame faults in fresh pages.

That reorders Wave 3. `plan/02-performance.md` named the glyph atlas
(`AAA-P-01`) the largest single win; it is worth 2–3 ms on a text-heavy scene,
while buffer reuse (`AAA-P-02`) is worth ~4.7 ms on **every** scene. The plan
has been corrected rather than followed.

## Export wall-time

`examples/unit_circle.lsf` — 1 560 frames at 1080p, measured end to end.

| | before | after | change |
|---|---|---|---|
| MP4 (H.264) | 12.86 s | **9.82 s** | −24% |
| PNG sequence | 4.54 s | **3.04 s** | −33% |

Of the 12.86 s MP4, rendering was only **2.33 s** — ffmpeg was the other 82%,
and the two ran back to back rather than overlapping. TD-05's premise that
export is "embarrassingly parallel" holds only for the PNG path; for video,
overlapping the two stages is worth far more than N-way parallel rendering,
which cannot beat the encoder.

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

  The `unsafe` count is genuinely 0, but the grep returned a false positive
  from the word appearing in a comment — which was the argument for
  `AAA-SEC-07`. **Since v0.4.1 this row is no longer measured by grep at all:**
  every crate root carries `#![forbid(unsafe_code)]`, so the value is a
  compiler guarantee. `forbid` cannot be silenced by an `allow` further down,
  so introducing `unsafe` requires deleting that line in a diff a reviewer
  sees.
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

| Dimension | v0.3.0 | v0.4.0 | v0.5.0 | Why it moved |
|---|---|---|---|---|
| Architecture | 90 | 93 | 95 | Text layout unified (TD-18); `object_registry` shared by both front ends. Four error idioms still coexist (`AAA-ARCH-01..04`) |
| API design | 75 | 78 | 90 | One error envelope on every HTTP endpoint *including* `axum`'s own extractor rejections; MCP answers in the same vocabulary. Properties still untyped (TD-07) |
| Documentation | 85 | 93 | 95 | Every behaviour change in this release documented where it is; ADR-0014 records the rename. `AAA-CQ-01` still open |
| Testing | 78 | 86 | 92 | 311 tests, 21 parity fixtures, coverage measured and gated. Still no property tests or fuzzing in CI |
| Benchmarks | 60 | 60 | 85 | Running in CI with a regression gate that requires corroboration across a benchmark family, after a false positive proved the threshold alone was not enough |
| Performance | 65 | 65 | 80 | Wave 3 landed: analytic spring, binary keyframe search, glyph cache, pixmap reuse, pipelined export. Sub-pixel text cost 4% back, knowingly |
| Security | 55 | 68 | 90 | Server hardened (TD-09 closed): auth, rate limiting, CORS allowlist, loopback default. Scheduled advisory scan, SHA-pinned actions, CodeQL, Scorecard. All five audited DoS vectors closed |
| Developer experience | 82 | 86 | 88 | `cargo xtask ci` is one gate command; the CLI is still one flat command until Wave 7 |
| Examples | 85 | 90 | 92 | Rendered by CI now, so a broken example is a broken build (principle #12) |
| CI / release engineering | 80 | 88 | 93 | 15 jobs, coverage gated, release workflows written and armed for crates.io and PyPI. Still nothing actually published — the last step is a merge and a tag |
| Accuracy | — | — | 90 | New dimension. OKLab colour, closed-form spring, Newton–Raphson bezier inversion, adaptive plot sampling, exact tick indexing, full SVG arc grammar |
| Output fidelity | — | — | 85 | New dimension. BT.709 tagging, 10-bit, alpha formats, EXR, straight-alpha correctness at every boundary. Linear-light compositing blocked by the rasteriser (`AAA-OUT-01`) |
| Motion design | — | — | 88 | New dimension. Arc-length draw-on, motion blur, arc-length morphing, camera rotation, sub-pixel text |
| Ecosystem | — | — | 55 | New dimension. MCP server exists and names are settled on all three registries; nothing is installable yet, and the JS SDK does not build (TD-12) |

**Overall trajectory target (revised at v0.4.0):** the AAA programme raises
the bar to **≥ 95 on every dimension by v1.0, nothing below 85 after v0.6**,
and adds four dimensions this scorecard never had — Accuracy, Output fidelity,
Motion design, and Ecosystem. Targets and ownership per dimension are in
[plan/00-master.md](../plan/00-master.md#scorecard-targets).

The lowest three scores are now Benchmarks (60), Performance (65), and
Security (68) — which is precisely the Wave 3 and Wave 1 scope. Ecosystem,
newly scored, starts at **30**: nothing is published to any registry and no
external contributor has ever had an issue to pick up.
