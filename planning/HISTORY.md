# Lumina — Project History

A high-level, human-readable narrative of where Lumina has been, what it does
today, and where it's headed. For the precise, release-by-release record see
[`CHANGELOG.md`](../CHANGELOG.md); for what's next see
[`ROADMAP.md`](./ROADMAP.md).

---

## Shipped — v0.1.0 (initial release)

The first cut proved the core thesis: **declarative LSF JSON in, rendered
animation out**.

- Rust workspace: `lumina-core` (scene graph, timeline, interpolation),
  `lumina-renderer`, `lumina-text`, `lumina-schema`, `lumina-export`,
  `lumina-server`, `lumina-cli`.
- Skia (tiny-skia) CPU renderer; first geometric object types.
- Keyframe timeline with easing; MP4 + PNG-sequence export via FFmpeg.
- Schema validation with AI-oriented `fix_suggestion` errors.

## Shipped — v0.2.0

The "make every README claim true" release — hardening, breadth, and ecosystem.

- **All 17 object types** in the schema and the Skia backend: Circle, Rectangle,
  Polygon, Path, Line, Arrow, Text, LaTeX, MathML, Image, SVG, Group,
  NumberLine, Axes, Plot, BezierCurve, Particles.
- **Dual backends** — Skia (CPU) with full coverage; Vello (GPU/wgpu) for
  geometric primitives, selectable via `--backend vello`.
- **27 easings + parameterised `cubic_bezier` + `spring` (RK4)**; LAB-colorspace
  color interpolation; path morphing across differing vertex counts.
- **Visual effects** — linear/radial gradients, drop shadows/glow, rounded
  rectangles, text alignment + letter spacing, `draw_fraction` write-on.
- **Assets** — PNG/JPEG/SVG (resvg) and animated-GIF compositing; per-character
  font fallback; deterministic, scrub-safe particles.
- **Server** — `/health`, `/validate`, `/render`, `/schema`, `/objects`, and an
  RFC-6902 `/patch` endpoint.
- **WASM** — `hit_test` across all 17 object types; interactive event bus.
- **Ecosystem** — JavaScript SDK (React + vanilla), Python SDK (PyO3 + maturin),
  mdBook docs site, criterion benches, CI (fmt/clippy/test/doc/wasm/deny), and
  a 2.5-minute flagship showcase (`showcase_neural_network`).

---

## Shipped — v0.3.0 (this release)

Closed the remaining gaps and added new capability.

- [x] **Vello GPU parity** — Text, LaTeX, MathML, Image, SVG and Particles now
      render on the GPU backend. A shared `raster` module rasterizes glyphs
      (fontdue), SVG (resvg) and images into `peniko::Image`s composited via
      `Scene::draw_image`; particles fill GPU circles. Image/SVG opacity is
      applied via an alpha layer. Reaches parity with Skia (gradients and drop
      shadows remain Skia-only — see deferred).
- [x] **`spline` easing** — monotone-cubic (Fritsch–Carlson) interpolation
      through `easing_params.keypoints`, overshoot-free.
- [x] **WebM (VP9) + GIF export** — `export_webm` / `export_gif` in the export
      crate (shared `stream_frames` + `encode_with_ffmpeg`), wired into the CLI
      (`--format webm|gif`) and the server (`format` field + MIME type).
- [x] **Event system completion** — `jump_to_time` now seeks; added `play_from`,
      `pause`, `tween_to`, `show_tooltip`, `emit_custom` with `$drag.*` payload
      substitution. `EventBus` carries a `PlaybackState` and returns an
      `EventOutcome { actions, current_time, playing, emitted }`.
- [x] **Semantic ScenePatch ops** — `lumina_core::scene_patch` (`add_object`,
      `remove_object` with cascade, `update_property`, add/remove/update keyframe,
      add/remove event, `update_canvas`) plus a `POST /scene_patch` endpoint that
      re-validates.
- [x] **Grand showcase** — `examples/gen_grand_showcase.py` →
      `examples/showcase_grand.lsf`, rendered on the **Vello** backend to
      `media/showcase_grand.{mp4,gif,webm}`, exercising spline easing, GPU text /
      LaTeX / SVG / particles, camera moves and event annotations.
- [x] **Cleanups** — cleared all `lumina-wasm` clippy warnings.

---

## Upcoming / deferred (v0.4+)

- **Vello gradients & drop shadows** — bring the remaining v0.2.0 visual effects
  to the GPU backend (currently Skia-only).
- **WASM WebGPU** — port the Vello backend to run in the browser player.
- **MiTeX layout** — true math layout instead of Unicode-substitution fallback;
  per-symbol `animate_parts`.
- **Interpolated `tween_to`** — runtime keyframe blending (v0.3.0 applies the
  target value immediately via the override channel).
- **Lottie export** — geometric subset → Lottie JSON for legacy players.
- **3D transform layer** — perspective + rotateX/Y/Z (card-flip first).
- **Asset pipeline** — automatic SVG/image optimization.
- **Self-correction loop** — CLI validate→fix→retry around `fix_suggestion`.
