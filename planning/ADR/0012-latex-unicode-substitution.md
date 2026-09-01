# ADR-0012 — LaTeX renders by Unicode substitution; mitex is dropped

- **Status:** Accepted · **Date:** 2026-09-01

## Context
TD-06 recorded a dependency declared and never used: `mitex` sat in
`lumina-text`'s manifest while every `LaTeX` and `MathML` object was rendered
by a hand-written Unicode transliterator in the renderer
(`skia_backend::latex_to_unicode`). The debt entry left the question open —
implement real mitex-based typesetting, or drop the dependency and describe
the substitution approach honestly.

RUSTSEC-2026-0235 decided it. The advisory (out-of-bounds reads on malformed
archives) reaches the workspace through exactly one path:
`mitex-spec → mitex-lexer → mitex-parser → mitex → lumina-text`. A live
vulnerability was being carried by code that never executes.

## Decision
Remove `mitex` from `crates/lumina-text/Cargo.toml` and from
`[workspace.dependencies]`. LaTeX and MathML continue to render through
Unicode substitution, and the documentation says so plainly rather than
implying a typesetting engine that was never wired up.

Real typesetting stays on the roadmap as a feature with its own design, not
as an unused dependency waiting to be discovered. If it lands, it will be
chosen on its merits at that time — the removed dependency is not a
commitment either way.

## Consequences
- RUSTSEC-2026-0235 is eliminated rather than suppressed: no `deny.toml`
  ignore, no residual exposure.
- The locked dependency count falls from 428 to 386 — 42 crates, roughly
  10% of the tree, removed with no loss of behaviour.
- TD-06 closes. The honest-documentation half of it becomes a doc change,
  not a pending decision.
- Anyone expecting mitex to be "nearly wired up" is corrected: it never was.
  Real typesetting is greenfield work whenever it is scheduled.
