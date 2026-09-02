# Contributing to Lumina

Contributions are welcome — bug fixes, features, documentation, examples,
better tests. The bar is simple and it does not move: **the workspace stays
green, and new behaviour is covered by a test that fails without it.**

This file is the canonical contribution guide. Read [VISION.md](VISION.md)
first so you know what the project is for, and know that every pull request is
judged against [ENGINEERING_PRINCIPLES.md](ENGINEERING_PRINCIPLES.md) — thirteen
numbered rules that are quoted in review by number.

Who decides what, and how that changes, is in [GOVERNANCE.md](GOVERNANCE.md).
Where to ask a question is in [SUPPORT.md](SUPPORT.md).

---

## Your first contribution

If you want to help and do not have something specific in mind, the issues
labelled [`good first issue`](https://github.com/SakarZaidan/lumina/labels/good%20first%20issue)
are scoped for exactly that: each names the file to change and states how you
will know you are done.

The walkthrough:

1. **Comment on the issue** saying you are taking it, so nobody duplicates
   your work. No permission needed — the comment *is* the claim.
2. **Fork and branch.** Branch names are `feat/<slug>`, `fix/<slug>`,
   `docs/<slug>`, `test/<slug>`, `perf/<slug>`, `refactor/<slug>`, or
   `ci/<slug>`.
3. **Make the change**, following the existing style and the crate layering.
4. **Add or update a test.** For rendering changes, a pixel assertion over a
   known scene is strongly preferred over a description of the intent.
5. **Run the gate** (below). It is the same thing CI runs.
6. **Open the pull request.** Fill in the template — the test plan section is
   the one reviewers read first.

If you get stuck halfway, open the PR as a draft and say where. A half-finished
PR with a clear question is a perfectly good contribution.

---

## Setup

- Latest stable [Rust](https://rustup.rs/). MSRV is **1.88**, enforced by
  `rust-version` in `Cargo.toml` and by a CI job.
- `ffmpeg` on `PATH` — required for MP4/WebM/GIF export and the export tests.
- Optional, per area: `wasm-pack` (the wasm crate), `mdbook` (the docs site),
  `maturin` (the Python SDK).

A software Vulkan driver (`mesa-vulkan-drivers` on Debian/Ubuntu) lets you run
the cross-backend parity suite locally. Without one those tests skip; set
`LUMINA_REQUIRE_VELLO=1` to turn a skip into a failure, as CI does on Linux.

## The gate

One command runs everything CI runs, in the same order:

```bash
cargo xtask ci
```

It stops at the first failure and prints a summary. Steps needing a tool you
do not have — `wasm-pack`, `mdbook`, `cargo-deny` — are reported as skipped
rather than failing, so a partial toolchain still gets you most of the way.

While iterating, skip the slow steps:

```bash
cargo xtask ci --fast      # no wasm, book, or example renders
```

The gate is defined once, in `xtask/src/main.rs`. That matters more than the
convenience: when a command list in a README drifts from what CI runs, the
README loses silently and a contributor finds out at merge time.

Other tasks:

```bash
cargo xtask fmt            # format the workspace
cargo xtask examples       # render every example scene (needs ffmpeg)
```

The test step sets `LUMINA_REQUIRE_VELLO=1`, so a missing wgpu adapter fails
rather than silently skipping the cross-backend parity suite. Install a
software Vulkan driver (`mesa-vulkan-drivers` on Debian/Ubuntu) if you do not
have one. Parity failures write both frames plus a difference heat map to
`target/parity-failures/` — look at them before assuming the tolerance is wrong.

Failures write both frames plus a difference heat map to
`target/parity-failures/` — look at them before assuming the tolerance is
wrong.

For the SDKs:

```bash
(cd sdks/python && maturin develop && python -c "import lumina")
wasm-pack test --node crates/lumina-wasm
```

---

## Conventions

**Commits.** [Conventional Commits](https://www.conventionalcommits.org):
`feat(core): …`, `fix(renderer): …`, `docs(planning): …`. One logical change
per commit. A commit message explains *why*; the diff already shows what.

**No panics in production code.** No `unwrap`, `expect`, or `panic!` outside
tests — malformed input degrades gracefully or returns a structured error. The
two that exist today are provably guarded and documented in
`planning/METRICS.md`; do not add a third. `unsafe` requires written
justification and safety comments; there is none in the codebase and that is
worth keeping.

**Schema compatibility.** Every new schema field takes `#[serde(default)]`, so
scenes written a year ago still load. This is not negotiable pre-1.0 either
(principle #10).

**New object types** must be handled in six places: the schema enum, the Skia
`z_index` match, the Skia draw match, the Vello match, and the WASM `hit_test`
and `get_z_index`. Miss one and the object silently does nothing on that path.
Reducing this to a single trait implementation is tracked as `AAA-CQ-05`.

**Rendering changes need a rendering test.** Pixel assertions over a known
scene. Rendering must stay deterministic — the same scene at the same time
produces the same bytes, on both backends, forever. If your change alters
output intentionally, say so in the PR and regenerate the goldens explicitly
(principle #1).

**No duplicate implementations.** Anything both backends need lives in
`crates/lumina-renderer/src/common/` exactly once.
`tests/duplication_gate.rs` enforces part of this by asserting on source text;
the rest is on review.

**Documentation is part of the change.** Update `CHANGELOG.md` under
`[Unreleased]` and the relevant `docs/src/` chapter when behaviour a user can
observe changes. Public items need `///` — `missing_docs` is a warning and CI
runs `-D warnings`, so an undocumented `pub` fails the build.

**The planning system is part of the codebase.** If your change closes or
creates technical debt, update `planning/TECH_DEBT.md` in the same PR. Debt is
closed only by linking the PR that fixed it. Stale planning docs are treated
like failing tests (principle #13).

**Performance claims need numbers.** Optimisation PRs start from a benchmark
and land with before/after figures in the description. No speculative
`#[inline]`, no clever code justified by intuition (principle #5).

**Public API changes need an RFC.** The LSF schema, the `Renderer` trait,
server endpoints, and SDK surfaces go through [planning/RFCS/](planning/RFCS/)
before implementation. A small additive change — one defaulted schema field —
needs a paragraph, not a ceremony, but it still gets written down.

**No binary media in git.** Demo videos and GIFs are attached to GitHub
Releases. The files already in `media/` are grandfathered by ADR-0010; do not
add to them.

**Authorship.** All work is attributed to its human author. Do not add AI
tools as authors, co-authors, or contributors in commits, pull request bodies,
or anywhere else in the repository (ADR-0002). Use whatever tools help you;
the commit is yours.

---

## Review

You can expect a first response within about a week. Reviews cite principles
by number, and a reviewer asking for a change will say which rule it comes
from — if that is not clear, ask, because an unexplained review comment is a
review bug.

What a reviewer checks:

- [ ] Does it do what the description says, and only that?
- [ ] Is there a test that fails without the change?
- [ ] Does it respect the crate layering (`schema` → `core` → `renderer` → `export`)?
- [ ] Any new panic paths, `unsafe`, or swallowed errors?
- [ ] Does rendering stay deterministic and both backends stay at parity?
- [ ] Are docs, `CHANGELOG.md`, and the planning docs updated in the same PR?
- [ ] For performance work: are there before/after numbers?
- [ ] For public API changes: is there an RFC?

Squash-merging is the default. Your commits are preserved in the PR either way.

---

## Where the work is

- [`planning/ROADMAP.md`](planning/ROADMAP.md) — the single schedule of record.
- [`planning/TECH_DEBT.md`](planning/TECH_DEBT.md) — every known compromise,
  with severity, blast radius, and a target release.
- [`plan/`](plan/) — the programme to reach v1.0: fourteen dimension subplans,
  each with `file:line` evidence and acceptance tests. If you want to
  understand *why* something is on the roadmap, it is explained there.

## Reporting issues

Include a clear description, a **minimal `.lsf` scene** that reproduces the
problem, and your environment (OS, backend, Rust version). Scenes are data, so
a reproduction is a paste rather than a project — please include one.

For security issues see [SECURITY.md](SECURITY.md). Do not open a public issue.
