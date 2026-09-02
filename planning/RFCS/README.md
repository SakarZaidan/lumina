# Lumina RFCs

Substantial changes go through a lightweight RFC before implementation —
modeled on Rust's process, sized for this project.

## When an RFC is required

- Any change to the **LSF schema** that is not a purely additive
  `#[serde(default)]` field.
- Changes to the **`Renderer` trait** or the renderer backend contract.
- New or changed **server endpoints**, or changes to the validation error
  format (AI loops depend on it).
- **SDK public surfaces** (Python/JS API shape).
- Anything with a **migration impact** on existing `.lsf` scenes.
- New **required dependencies** or runtime requirements (another ffmpeg-class
  decision).

Not required for: bug fixes, internal refactors, docs, additive defaulted
schema fields (a paragraph in the PR description suffices), or anything
already covered by an accepted RFC.

## Process

1. Copy `0000-template.md` to `NNNN-short-slug.md` (next free number).
2. Open a PR containing just the RFC. Discussion happens on the PR.
3. On merge, the RFC is **Accepted**; record the decision as an ADR in
   [`planning/ADR/`](../ADR/) referencing the RFC.
4. Implementation PRs link the RFC. When shipped, edit the RFC header status
   to **Implemented (vX.Y.Z)**.
5. Rejected/withdrawn RFCs are merged too, with that status — a written "no"
   prevents the same debate twice.

## Index

| RFC | Title | Status |
|---|---|---|
| [0001](0001-render-into.md) | `Renderer::render_into` — render without allocating the output | **Rejected** — measured no benefit; every caller needs owned bytes |
