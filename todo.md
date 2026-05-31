# Future Improvements (todo.md)

This document tracks the Lumina engine roadmap. Items are kept in sync with the
README roadmap and CHANGELOG.

## Done (shipped in 0.2.0)

- [x] **Vello CLI Integration** — GPU backend selectable via `--backend vello`.
- [x] **Path Morphing** — vertex-matching interpolation for paths of differing lengths.
- [x] **LAB Color Interpolation** — perceptually uniform color transitions.
- [x] **Spring Physics** — RK4 spring solver (`easing: "spring"`).
- [x] **Complete Easing Library** — 27 easings + parameterised `cubic_bezier`.
- [x] **Bezier Easings** — CSS-spec `cubic_bezier(x1,y1,x2,y2)`.
- [x] **Interactive Events** — event bus + full `hit_test` across all object types.
- [x] **Live Preview** — `lumina-cli --watch` re-renders on file change.
- [x] **LaTeX Parts Animation** — `draw_fraction` write-on for LaTeX.
- [x] **MathML Support** — `MathML` object type via the Unicode text pipeline.
- [x] **Font Fallbacks** — per-character fallback through loaded fonts.
- [x] **Image / SVG / GIF compositing** — raster, SVG (resvg) and animated GIF
      assets composited into frames with position/scale/rotation/opacity.
- [x] **Visual effects** — gradients (linear/radial), drop shadows/glow,
      rounded rectangles, text alignment + letter-spacing.
- [x] **Particles** — deterministic, time-reproducible particle emitter.
- [x] **`/objects` endpoint** — object-type registry for agent introspection.

## In progress / next

- [ ] **WASM WebGPU**: Port the Vello backend to WebGPU in the browser player.
- [ ] **Lottie Export**: Convert LSF scenes to Lottie JSON for legacy players.
- [ ] **Self-Correction Loop**: CLI-based validate→fix→retry helper around the
      schema validator's `fix_suggestion` output.
- [ ] **Asset Pipeline**: Automatic optimization for imported SVGs and images.
- [ ] **GPU text/image**: Bring text, image and particle rendering to the Vello
      backend (currently CPU/Skia only).
- [ ] **MiTeX layout**: Use `mitex` for true math layout instead of the current
      Unicode substitution fallback.
