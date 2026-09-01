# 05 — Animation and motion design

Accuracy ([04](04-math-physics-accuracy.md)) is whether a number is right.
This is whether the result *moves well* — the difference between a technically
correct interpolation and animation someone wants to watch.

## Current state

The vocabulary is already strong: 33 registered easings including the Manim
family (`smooth`, `rush_into`, `rush_from`, `there_and_back`), CSS aliases,
parameterised `cubic_bezier` and monotone `spline`, plus draw-on, path
morphing, group transforms, and a camera with its own timeline. Very few
engines ship that range.

What is missing is at the edges, and the edges are what read as quality.

**The camera silently ignores its own easing parameters.** `get_camera_at`
calls `get_easing_fn(&k1.easing)` (`timeline.rs:163`) — the name-only lookup —
rather than `eval_easing`, which is the function that accepts
`easing_params`. A camera keyframe using `cubic_bezier` or `spline` therefore
gets the *default* shape of that easing, not the one the author specified,
with no warning. Camera moves are the most visible motion in a scene.

**The camera has no rotation.** `CameraState { x, y, zoom }`
(`schema/src/lib.rs:591`). Pan and zoom only.

**Draw-on is faked with a dash pattern.** `common/stroke.rs:5` returns
`vec![length * frac, length * 2.0]` — a dash whose gap is longer than the
path. Accuracy then depends on the rasteriser's dash-phase handling, which is
why fixture `09_dash_fraction` needs its own tolerance. On `Plot` it is worse:
`draw_fraction` truncates the *sample count* (`skia_backend.rs:805`), so as
the value animates 0 → 1 the curve's resolution changes — it resolves rather
than draws.

**Path morphing bunches vertices.** `interpolator.rs:27-40` pads the shorter
point list by repeating its last element. Correct as a definition, wrong as
motion: every extra vertex piles onto one point of the source shape and the
morph collapses toward it instead of flowing.

**No motion blur.** For an engine whose frames are independent and analytically
evaluable at any *t*, temporal supersampling is unusually cheap here — and it
is the single largest perceived-quality difference between "rendered" and
"animated".

**`LineProps.dash` is implemented by neither backend** (TD-19). The schema
field exists; dashed-line scenes render solid.

**No easing preview, and no motion vocabulary above the keyframe.** There is
no way to see what `elastic_out` looks like without rendering a scene, and no
higher-level constructs (stagger, follow-through, overlap) that motion
designers reach for.

## Target

Motion that a designer would sign off on: easings that do what they claim,
draw-on that draws, morphs that flow, camera moves that honour their curve,
and optional motion blur. Plus the tooling to *see* a curve before committing
to it.

## Work items

| ID | Item | Acceptance |
|---|---|---|
| `AAA-MOT-01` | Camera evaluation routed through `eval_easing` with params | A `cubic_bezier` camera keyframe matches the same curve on an object property |
| `AAA-MOT-02` | Arc-length draw-on: trim the path, do not dash it | `draw_fraction: 0.5` on a curve of known length ends at the halfway point within a pixel |
| `AAA-MOT-03` | Plot draw-on samples at full resolution and trims by arc length | Curve resolution is constant across the reveal |
| `AAA-MOT-04` | Morph by arc-length resampling with correspondence | A 4-point square morphs to a 64-point circle without vertex bunching |
| `AAA-MOT-05` | Camera rotation as a first-class animated property | Schema field, both backends, parity fixture |
| `AAA-MOT-06` | Motion blur via temporal supersampling, opt-in per scene | N sub-samples per frame, weighted; determinism preserved |
| `AAA-MOT-07` | Implement `LineProps.dash` on both backends (TD-19) | Parity fixture; the schema stops lying |
| `AAA-MOT-08` | Stagger and delay on timeline entries | One entry animating a group of objects with a per-child offset |
| `AAA-MOT-09` | `lumina-cli inspect --easing <name>` renders the curve | ASCII or PNG plot of any registered easing, no scene required |
| `AAA-MOT-10` | Easing gallery page in the book, generated from `EASING_NAMES` | Cannot drift from the registry — generated, not hand-written |

## Notes on determinism

`AAA-MOT-06` is the only item here that touches the determinism guarantee, and
it does not break it: sub-samples are taken at fixed offsets within the frame
interval, so the same frame always produces the same set of samples. It must
ship with a parity fixture proving both backends agree, and a golden test
proving two runs are byte-identical.

## Metrics moved

Motion design — a new scorecard dimension, 72 → 95.

## Sequencing

`AAA-MOT-01` in Wave 1 (it is a one-line correctness fix with a visible
effect). `02`, `03`, `04`, `07` in Wave 4 with the rest of the fidelity work.
`05`, `06` in Wave 4 as well but behind them, since both add schema surface.
`08` in Wave 6 with the typed schema. `09`, `10` in Wave 7 with the tooling.
