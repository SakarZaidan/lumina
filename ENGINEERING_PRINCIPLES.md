# Lumina — Engineering Principles

The project's constitution. Every PR is judged against these; when a principle
must be violated, the violation is stated in the PR description and recorded
in [planning/TECH_DEBT.md](planning/TECH_DEBT.md) — never slipped in silently.
([VISION.md](VISION.md) covers product principles; this covers engineering.)

1. **Determinism first.** Same inputs → same pixels, every backend, every
   platform. Any change that could alter rendering must pass the golden-pixel
   tests; intentional visual changes update the goldens explicitly.

2. **No panics in production code.** No `unwrap`/`expect`/`panic!` outside
   tests; malformed input degrades gracefully or returns a structured error.
   `unsafe` requires written justification and safety comments. Current
   count of both: zero. Keep it there.

3. **Architecture before features.** Respect the layering
   (`schema` → `core` → `renderer` → `export`); no upward dependencies, no
   logic in the data crate, no pixels outside the renderer. A feature that
   doesn't fit the layers triggers a design conversation, not a workaround.

4. **No duplicate implementations.** Logic needed by both renderer backends
   lives in a shared module. Duplication that can't be avoided immediately
   gets a TECH_DEBT entry with a target release (this is how the backend
   parity debt is being paid down — see TD-01/TD-02).

5. **Measure before optimizing.** Performance work starts with a benchmark or
   a profile, and lands with before/after numbers in the PR. No speculative
   `#[inline]`, no clever code justified by vibes.

6. **Every feature tested; every subsystem benchmarked.** New behavior ships
   with tests that fail without it (pixel assertions preferred for
   rendering). Hot paths have criterion benches. Tests are deterministic —
   a flaky test is a bug with priority.

7. **Zero warnings.** `cargo clippy` with `-D warnings` and `cargo fmt
   --check` gate every merge. Suppressions (`#[allow]`) are rare, local, and
   justified in a comment.

8. **Everything documented, truthfully.** Public items carry rustdoc; user
   behavior lives in the book; docs describe what code does *now*. Known
   limitations are documented as prominently as features (parity table,
   SECURITY.md). A doc that overclaims is treated as a bug.

9. **Public APIs require an RFC.** Changes to the LSF schema, the `Renderer`
   trait, server endpoints, or SDK surfaces go through
   [planning/RFCS/](planning/RFCS/) before implementation. Small additive
   changes (a defaulted schema field) need a paragraph, not a ceremony — but
   they still get written down.

10. **No silent breaking changes.** Semver is honored; schema changes are
    backward-compatible (`#[serde(default)]`) or ship with a migration guide
    and a major/minor bump per pre-1.0 convention. A scene that rendered
    yesterday renders identically today, or the CHANGELOG says why.

11. **Every release reproducible and recorded.** Releases follow the
    documented procedure ([WORKFLOW](planning/AI/WORKFLOW.md)): green CI →
    CHANGELOG → annotated tag → GitHub Release; committed `Cargo.lock`;
    metrics and quality scorecard updated
    ([planning/METRICS.md](planning/METRICS.md)).

12. **Examples are production quality.** Every example renders with the
    documented command, is indexed in `examples/README.md`, and demonstrates
    something specific. A broken example is a broken build.

13. **The planning system is part of the codebase.** STATUS, ROADMAP,
    TECH_DEBT, ADRs, and METRICS are updated by the change that affects them
    — stale planning docs are treated like failing tests.
