# The AAA Program — master plan

**Thesis.** Lumina's governance is already better than most projects with a
hundred times its reach. Its gap is not planning; it is *exposure and proof*.
Work sat unmerged while `main` shipped an older story. Documents described a
GPU backend that runs on the CPU and an RK4 integrator that is Euler's method.
Nothing was installable from any registry. The program below closes that gap
first, then raises every dimension of the engine to a level where the claims
are worth making.

**Definition of AAA, stated so it can be falsified:** every claim in the
documentation is true of the code; every dimension of the quality scorecard is
≥ 95; the engine installs with one command from Rust, Python, and JavaScript;
and the same scene renders byte-identically on three platforms and two
backends, verified in CI on every commit.

---

## Where we started (2026-09-01)

Measured, not estimated. Sources: `planning/METRICS.md`, `git`, `gh`, and a
full read of the workspace.

| | |
|---|---|
| `origin/main` | v0.3.0 — **42 commits behind** the finished v0.4 work |
| Open PRs holding v0.4 | 10, stacked (#10 → #19), none merged |
| Published packages | none — not crates.io, not PyPI, not npm |
| Repository description / topics / homepage | all empty |
| Issues ever filed | 0 |
| Test count | 120 functions; coverage never measured |
| Benchmarks in CI | none (3 criterion groups exist, never run) |
| Untracked defects found by audit | 11, several remotely exploitable |
| Lowest scorecard dimensions | Security 55 · Benchmarks 60 · Performance 65 |

---

## The waves

Each wave has one job and one gate. A wave does not start until the wave
before it is provably complete — the gate is a command someone can run, not a
judgement call.

### Wave 0 — Land what exists

Ten stacked PRs hold every v0.4 deliverable: backend parity, the 16-fixture
pixel-diff harness, the server safety minimum, the 3-OS matrix, 416 rustdoc
items. Until they merge, none of it is real.

The first matrix run had already found two things nobody had seen, because
those jobs had never executed: Windows aborts the renderer test process inside
the Vello adapter probe (TD-20), and the WASM suite failed on
`missing field version` — which, once fixed, exposed that **nothing inside a
`Group` had ever been clickable**. Both are fixed; both are the point. Turning
on a check you have never run is worth more than adding a feature.

**Gate:** `main` green on ubuntu/macos/windows plus wasm; `v0.4.0` tagged and
released; repository description, topics, and homepage populated.

### Wave 1 — Truth, then safety

Close every divergence between what a document says and what the code does —
see [08-code-quality](08-code-quality.md#documentation-divergences) for the
full table. Then land the five defects an audit found that the debt register
did not track: unbounded scene resources, unbounded recursion, a blocking
render handler, an `inf as i32` tick loop, and a backend that errors where its
twin silently skips. Set the lint floor: `#![forbid(unsafe_code)]`,
`[workspace.lints]`, a pinned toolchain, and one `cargo xtask ci` command that
runs exactly what CI runs.

**Gate:** every row of the divergence table struck through; each of the five
security items has an adversarial fixture that fails without its fix; zero
`unsafe` enforced by the compiler rather than by `grep`.

### Wave 2 — Correctness and accuracy

The numerics. An analytic spring instead of a 100-step Euler staircase.
Newton–Raphson bezier inversion instead of fixed bisection. Adaptive plot
sampling instead of a uniform grid that facets steep curves and stops short of
poles. Exact tick sequences instead of accumulated `t += step`. OKLab colour
with alpha instead of a hex-string round trip every frame. Camera keyframes
that honour their easing parameters. The full SVG path grammar, arcs included.

**Gate:** property tests over the easing and interpolation invariants, and
fuzz targets over all three parsers, running in CI.

### Wave 3 — Performance

Twelve items, each with a measured baseline captured before the change. The
two largest are a glyph atlas (the engine currently re-rasterises the same
twenty glyphs a hundred and eighty thousand times per showcase render) and
frame-parallel export (`rayon` has been a declared dependency since v0.1 and
is imported nowhere).

**Gate:** criterion in CI as a regression gate; before/after numbers in every
PR; the parity suite still green afterwards.

### Wave 4 — Output fidelity and motion

Where "AAA" is actually visible. Composite in linear light. Tag the encode
BT.709 so players stop guessing. Ship 10-bit, alpha, and ProRes for people
who take the output into an editor. Add motion blur. Trim draw-on by arc
length instead of faking it with a dash pattern.

**Gate:** visual review against reference renders; determinism unchanged.

### Wave 5 — Server, supply chain, distribution

Auth, rate limiting, a CORS allowlist, structured logging, and one error
envelope across every endpoint instead of structured JSON on `/validate` and
bare prose everywhere else. Then publish: crates.io, PyPI, npm, and prebuilt
CLI binaries, with SBOM, provenance, and signed tags.

**Gate:** `cargo add lumina-core`, `pip install lumina`, and
`npm install @lumina/sdk` all work from a clean machine.

### Wave 6 — Typed schema and the AI-native interface

The largest pre-1.0 breaking change: typed properties replacing the raw
`serde_json::Value` flow, so a typo stops degrading silently to a default.
LSF v2 with a migration guide and a `migrate` command. A decision on LaTeX,
recorded either way. An MCP server, so any agent can drive the engine natively.

**Gate:** schema v2 documented; a v1 scene migrates and renders identically.

### Wave 7 — Playground, extension, CLI

A browser playground on the docs site — edit, preview live, scrub, share by
URL. An editor extension. A CLI with subcommands and diagnostics that point
into the JSON the way `rustc` points into Rust.

**Gate:** playground live on Pages, rendering a scene the visitor typed.

### Wave 8 — Stability

No new features. Semver audit, external-style security review, cross-platform
determinism proven in CI, support statement. Branch protection is enabled
here — last, so it never blocks the merges above.

**Gate:** every scorecard dimension ≥ 95.

---

## Scorecard targets

[`planning/METRICS.md`](../planning/METRICS.md) gains four dimensions this
program is accountable for, and the bar rises from "≥ 85 by v1.0" to **≥ 95 on
every dimension by v1.0, nothing below 85 after v0.6**.

| Dimension | v0.3.0 | v0.5 target | v1.0 target | Owned by |
|---|---|---|---|---|
| Architecture | 90 | 93 | 97 | [01](01-architecture.md) |
| API design | 75 | 85 | 96 | [01](01-architecture.md) |
| Documentation | 85 | 92 | 97 | [07](07-ui-ux-dx.md) |
| Testing | 78 | 90 | 96 | [10](10-testing-verification.md) |
| Benchmarks | 60 | 88 | 95 | [02](02-performance.md) |
| Performance | 65 | 88 | 95 | [02](02-performance.md) |
| Security | 55 | 85 | 96 | [03](03-security.md) |
| Developer experience | 82 | 90 | 96 | [07](07-ui-ux-dx.md) |
| Examples | 85 | 90 | 96 | [09](09-features.md) |
| CI / release engineering | 80 | 92 | 97 | [11](11-release-distribution.md) |
| **Accuracy** *(new)* | 70 | 88 | 96 | [04](04-math-physics-accuracy.md) |
| **Output fidelity** *(new)* | 65 | 85 | 95 | [06](06-render-output-fidelity.md) |
| **Motion design** *(new)* | 72 | 86 | 95 | [05](05-animation-motion.md) |
| **Ecosystem** *(new)* | 30 | 80 | 95 | [12](12-community-governance.md) · [14](14-playground-tooling.md) |

The four new dimensions are scored against the same standard as the rest:
"reference-quality open-source project", not "better than last release".
Ecosystem starts at 30 because nothing is published anywhere and no external
contributor has ever had something to pick up.

## Hard gates, enforced by CI

Not aspirations — build failures.

- Zero `unsafe`, enforced by `#![forbid(unsafe_code)]`, not by `grep`.
- Zero `unwrap`/`expect`/`panic!` outside tests, enforced by a lint.
- Coverage ≥ 85%, reported per PR.
- No criterion regression > 5% without an explicit, justified baseline update.
- Cross-backend pixel parity green on every fixture.
- Every example renders. Principle #12 has always said a broken example is a
  broken build; nothing has ever checked it.
- Documentation builds clean, and every public item is documented.
