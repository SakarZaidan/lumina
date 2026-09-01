# 09 — Features

## Current state

Seventeen object types, 33 easings, two backends, gradients, drop shadows,
rounded rectangles, images and animated GIFs and SVG, particles, LaTeX and
MathML by Unicode substitution, an event bus, semantic scene patching, a WASM
runtime with hit-testing, a headless server, and live reload. That is a great
deal of engine.

The gaps are not "more object types". They are the places where an author hits
a wall and has to leave the tool.

| Wall | Why it matters |
|---|---|
| No audio | An explainer video needs narration; today that means a second tool |
| No alpha output | The result cannot be composited into other footage |
| No 3D | Card flips and perspective reveals are staple motion-design moves |
| No `tween_to` interpolation | The event action applies its target instantly (backlog) |
| No Lottie or SVG *output* | Cannot hand a scene to a web player or a designer |
| No asset optimisation | A 4 MB PNG is decoded at full size every render |
| Nothing importable | No template library, no starting point but a blank file |
| No self-correction loop | The validator emits `fix_suggestion` and nothing consumes it |

Two schema fields also promise things the engine does not do: `LineProps.dash`
is implemented by neither backend (TD-19), and `Plot.draw_fraction` changes
sample resolution rather than revealing the curve
([05](05-animation-motion.md)).

## Target

Every schema field does what it says; the walls above have doors; and the
capability ladder is honest about which rung it is on.

## The ladder

### Rung 1 — Make what exists true (Wave 1–2)

Nothing new. `LineProps.dash` implemented, `draw_fraction` trimming by arc
length, the full SVG path grammar including arcs, adaptive plot sampling,
camera easing parameters honoured. An author who reads the schema reference
gets what it describes. This rung is worth more than any feature below it.

### Rung 2 — Finish the output story (Wave 4)

Audio tracks (`AAA-OUT-08`), alpha output (`AAA-OUT-06`), 10-bit and ProRes
(`AAA-OUT-05`, `07`), motion blur (`AAA-MOT-06`), camera rotation
(`AAA-MOT-05`). These are what turn a rendering engine into something an
editor accepts.

### Rung 3 — Make it reachable (Wave 5)

Published to crates.io, PyPI, and npm; prebuilt CLI binaries. A feature nobody
can install is not a feature — see [11](11-release-distribution.md).

### Rung 4 — Correctness by construction (Wave 6)

Typed properties (TD-07) so a typo is an error, LSF v2 with migration, and
`tween_to` interpolating through the timeline rather than snapping. Plus the
self-correction loop: the CLI already has structured errors with
`fix_suggestion` and does nothing with them; a `--fix` mode that applies
suggestions and re-validates closes the product's own core loop.

### Rung 5 — Reach beyond the engine (Wave 7+)

- **3D transform layer** — perspective plus `rotateX/Y/Z`, card-flip first.
  This is the largest single feature on the list and needs an RFC.
- **Lottie export** — the geometric subset to Lottie JSON, so scenes play in
  every existing web player. Strategically the most valuable export, because
  it makes LSF an *input* format to an ecosystem that already exists.
- **Template library** — a curated set of scenes to start from, which is what
  most people actually want from an animation tool.
- **Asset pipeline** — decode and downsample images once at load, cache by
  target size the way SVG already is (`skia_backend.rs:48`).
- **Bundled encoder** — optional in-process encoding so ffmpeg stops being a
  hard runtime dependency. Backlog, not dogma (DESIGN.md).
- **WASM WebGPU** — run the Vello backend in the browser, which is the
  original promise of having two backends at all.

## What stays out, permanently

Restating VISION.md so the ladder cannot creep: no CSS-animation replacement,
no video editor, no general-purpose game engine, no Python-only niche tool.
And no logic in the format — a capability that seems to need loops or
conditionals belongs in a generator that emits LSF, never in LSF.

## Examples as a feature

ENGINEERING_PRINCIPLES #12 says examples are production quality and a broken
example is a broken build. Nothing in CI renders one. Every rung above ships
with an example, and CI renders all of them —
`AAA-TEST-09` in [10](10-testing-verification.md).

## Metrics moved

Examples (85 → 96), and Ecosystem jointly with
[12](12-community-governance.md) and [14](14-playground-tooling.md).

## Sequencing

Rungs map to waves directly. Nothing on rung 5 starts before rung 1 is
complete: shipping a 3D layer on top of a schema field that renders solid
where it promises dashes would be building on sand.
