# WS-02 — Renderer Backend Parity (v0.4 flagship)

**Status:** In progress — harness live ([#11](https://github.com/SakarZaidan/lumina/pull/11)), extraction next · **Priority:** P0 for v0.4 · **Effort:** multi-session
**Linked debt:** TD-01, TD-02, TD-11 (cluster), TD-08

## Goal

The Skia (CPU) and Vello (GPU) backends render the same scene to the same
pixels within tolerance, verified in CI — and stay that way.

## Scope

1. **Extract `crates/lumina-renderer/src/common/`** (do this first — TD-02):
   - `svg_path.rs`: single SVG-path parser emitting a backend-neutral path
     representation, adapted to `tiny_skia::Path` and `kurbo::BezPath`
     (replaces `parse_svg_path` in skia_backend.rs and
     `parse_svg_path_kurbo` in vello_backend.rs).
   - `color.rs`: single color parser (replaces `parse_color` /
     `parse_vello_color`).
   - Scene-walk helpers: z-index sort, root detection, group-transform
     recursion currently duplicated per backend.
2. **Vello parity features** (TD-01): linear/radial gradients, drop shadows
   (match Skia's 3-pass box blur visually), rounded rectangles, dashed lines
   incl. `draw_fraction`.
3. **Pixel-diff harness** (TD-11): render a curated scene set (one scene per
   feature: each object type, gradients, shadows, rounded, dash, camera,
   groups, particles) on both backends; assert per-channel diff within
   tolerance; run in CI (CPU fallback adapter). Failures dump both images as
   artifacts.
4. **Easing strictness** (TD-08): unknown easing → schema-validation error
   with `fix_suggestion` (nearest-name), not silent `linear`.

## Non-goals

Performance (v0.5), typed properties (v0.6), wasm WebGPU (backlog).

## Acceptance criteria

- Pixel-diff suite green on both backends for the full scene set.
- The four feature gaps demonstrably closed (diff scenes exercise them).
- Zero duplicated parser code between backends (grep gate).
- A scene with a misspelled easing fails validation with a helpful message.

## Risks

- vello 0.2 may lack blur/gradient APIs needed → may force vello/wgpu
  upgrade first; scope that as a preliminary task if so.
- GPU rasterization differences (AA, rounding) require tolerance tuning —
  start loose (e.g. ≤2/255 per channel, ≤0.5% differing pixels), tighten
  empirically.
