# 08 — Code quality

## Current state

The baseline is unusually good and should be stated before the criticism:
`cargo clippy --all-targets --all-features` is **clean, zero warnings**. There
is **no `unsafe` anywhere**. There are no `todo!`, `unimplemented!`, or
`unreachable!` macros. Of every `unwrap`/`expect`/`panic!` in the workspace,
exactly two are outside test code, and both are provably guarded
(`validation.rs:303`, `skia_backend.rs:1437`). Every integer index checked has
an explicit length guard. Every float `==` is an intentional boundary test.

That is the bar this document is trying to protect, not raise.

## Documentation divergences

The project's own VISION principle 5 and ENGINEERING_PRINCIPLES #8 say a doc
that overclaims is a bug. These are the open bugs.

| Claim | Where | Reality |
|---|---|---|
| "RK4 integration" | `easing.rs:456` | Semi-implicit Euler, 100 fixed steps |
| CSS `ease` = `cubic-bezier(.25,.1,.25,1)` | `easing.rs:511` | Calls `ease_in_out_sine` |
| GPU rasteriser | `README.md` §Rendering | `force_fallback_adapter: true, use_cpu: true` (`vello_backend.rs:66,84`) |
| "27 easings" / "28 easings" | `README.md:307` / `:128` | Registry holds 33 |
| `npm install @lumina/sdk` | `README.md:474` | Unpublished; TD-12 calls it unbuildable |
| `pip install` | `README.md:507` | Unpublished |
| "92 tests, 0 failures" | `README.md:340` | 120 test functions |
| Phase 1–8 roadmap | `README.md:552` | Conflicts with ADR-0005; delete and link |
| "refresh root membership" | `core/src/scene.rs:42,47` | `root_objects` never touched |
| "No binary media in git" | `CONTRIBUTING.md:61` | 19 MB tracked; ADR-0010 permits it |
| Vello parity "pending" | `README.md:71,401` | Closed by #13/#14 |
| System font prerequisite | `README.md:158` | Bundled by #10 |
| `[0,1)` | `raster.rs:184` | Can return exactly 1.0 |

Each is one commit, each citing the line it corrects. This table is the Wave 1
checklist and should be struck through as it empties.

## Structural issues

**Four error idioms.** Covered in [01](01-architecture.md); it is as much a
code-quality problem as an architectural one.

**The largest duplication is not gated.** `common/` and
`tests/duplication_gate.rs` cover parsing, geometry, and ordering. They do not
cover *emit* code, and `draw_leaf_object` (`skia_backend.rs:259-969`, ~710
lines) is mirrored by `draw_leaf` (`vello_backend.rs:229-947`). Every new
object type is written twice by hand, and the gate cannot tell.

**Dead weight found by hand, not by tooling.** `mitex` was declared and unused
from v0.1 until an advisory forced the question (ADR-0012). `rayon` has been
declared and unimported for as long. `glam` is declared by `lumina-text` and
`lumina-renderer` with no visible use. `cargo-machete` would have flagged all
three mechanically, years of `cargo-deny` runs did not, and the manual audit
that found them was expensive.

**Swallowed errors.** `tools/lumina-cli/src/main.rs:279-286,303-310` and
`sdks/python/src/lib.rs:54,59` both do `let _ = load_font(...)`.
`timeline.rs:42` drops objects that fail to serialise with `Err(_) => continue`.
`vello_backend.rs:1043` does `sender.send(result).ok()`.

**Backends disagree on malformed input.** `skia_backend.rs:461` returns `Err`
for a malformed `Arrow` and aborts the whole export; `vello_backend.rs:392`
does `_ => return` and renders the frame without it. The parity suite cannot
catch this, because it only compares frames that both backends produced.

**Lint configuration is copy-pasted.** `#![warn(missing_docs)]` appears in six
crates; there is no `[workspace.lints]` table, no `#![forbid(unsafe_code)]`
anywhere, and the zero-`unsafe` metric is enforced by `grep` in a methodology
note rather than by the compiler.

## Work items

| ID | Item | Acceptance |
|---|---|---|
| `AAA-CQ-01` | Every row of the divergence table fixed | Table struck through; each row links a commit |
| `AAA-CQ-02` | `[workspace.lints]` with a pedantic subset; `unwrap_used`/`expect_used` denied outside tests | Lint config lives in one place |
| `AAA-CQ-03` | `#![forbid(unsafe_code)]` in all nine crates | Zero-unsafe becomes a compile error, not a grep |
| `AAA-CQ-04` | `cargo-machete` and `cargo-udeps` in CI | An unused dependency fails the build |
| `AAA-CQ-05` | Extract `draw_leaf_object` into per-object modules; widen the duplication gate | A new object type is implemented once |
| `AAA-CQ-06` | Backends agree on malformed input; behavioural parity test | Both return `Err` for the same bad scene |
| `AAA-CQ-07` | No silently swallowed errors on any load path | Every `let _ =` either logs or propagates |
| `AAA-CQ-08` | `cargo-semver-checks` and a `cargo-public-api` snapshot in CI | Principle #10 becomes enforceable |
| `AAA-CQ-09` | Commit-message linting and a pre-commit hook config | Conventional Commits enforced, not just documented |

## Metrics moved

Architecture, API design, and the `unsafe`/panic rows of
[`planning/METRICS.md`](../planning/METRICS.md) — which stop being grep counts
and become compiler guarantees.

## Sequencing

`AAA-CQ-01`, `02`, `03`, `04` in Wave 1. `06`, `07` in Wave 1 as well —
`06` is one of the five fix-first security items. `05` in Wave 4, `08` and
`09` in Wave 5 with the release machinery.
