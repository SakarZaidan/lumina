# 04 — Math, physics, accuracy

## Current state

Several things here are done better than the field standard and should be
said plainly, because the rest of this document is critical.

- **Monotone cubic splines** use Fritsch–Carlson (`easing.rs:44-98`), not
  Catmull–Rom, with the correct τ = 3/√s limiter at `:83-87`. That is the
  right algorithm and it prevents overshoot past the surrounding keypoints.
- **Colour interpolation is CIELAB** (`interpolator.rs:44-55`) with a proper
  sRGB gamma expand → XYZ D65 → Lab pipeline and a correct inverse, including
  the linear-segment handling in both directions. Most engines lerp sRGB.
- **De Casteljau subdivision** clips Bézier curves exactly at parameter *t*
  for draw-on (`skia_backend.rs:630-646`) rather than approximating.
- **Particles are analytic** (`raster.rs:135-174`) — closed-form position from
  time, no integrator. Any frame is computable in isolation, which is what
  makes scrubbing and frame-parallel export possible at all.
- **Transforms are computed once, in f32, via tiny-skia** (`common/scene.rs:70-115`)
  so the matrix is bit-identical on both backends, with kurbo's different
  coefficient order handled explicitly and documented at `:108`.
- `total_cmp` for keyframe sorting (`timeline.rs:83`) instead of
  `partial_cmp().unwrap()`, so NaN times cannot panic the sort.

### The spring is not what it says it is

`easing.rs:456` documents "RK4 integration". The loop at `:459-477` is
semi-implicit Euler:

```rust
let a = -STIFFNESS * (x - 1.0) - DAMPING * v;
v += a * DT;
x += v * DT;
```

Three consequences. It is **quantised**: `n = (t / DT).round()` with 100
steps, so `spring(0.001)` and `spring(0.004)` return the identical value — the
curve is a 100-level staircase, not a curve. It is **resolution-dependent**:
the shape changes if `STEPS` changes. And it costs **O(100) per property per
frame**.

The system is `m = 1, k = 200, c = 20`, giving ζ = c/(2√(km)) ≈ 0.707 —
underdamped, with a closed-form solution. There is no reason to integrate it
numerically at all.

### The CSS `ease` curve is not the CSS `ease` curve

`ease_css` (`easing.rs:511`) is documented as `cubic-bezier(0.25, 0.1, 0.25, 1.0)`
and implemented as `ease_in_out_sine`. The file already contains a correct
`cubic_bezier_easing` that would produce the real curve. This is a one-line
fix and a documentation claim that is currently false.

### Unvalidated preconditions in the easing solvers

`cubic_bezier_easing` (`:311-323`) bisects on `bezier_x(t)`, which is only
valid when `x1, x2 ∈ [0,1]` — the CSS constraint. `validation.rs:258-260`
checks only `arr.len() >= 4`, so out-of-range control points yield a silently
wrong curve rather than an error. Similarly `spline_easing` assumes sorted
keypoints; `easing.rs:61` clamps a negative interval to `1e-9`, so unsorted
input produces tangents around 1e9 and garbage output, while
`validation.rs:262-266` checks only `len() >= 2`.

### Non-finite values disappear instead of erroring

`interpolate_value` (`interpolator.rs:27`) builds results with
`Value::from(f32)`, and serde_json maps NaN and ±∞ to `Value::Null`. The
property then vanishes from the state map and the renderer silently falls back
to its `unwrap_or` default. Paired with `as_f64().unwrap_or(0.0)` at `:24-25`,
an out-of-range number becomes 0.0 with no diagnostic.

### Plot sampling is uniform

`skia_backend.rs:841-842` walks a fixed grid, 200 points by default. Steep
regions facet visibly; flat regions are oversampled. Poles are handled by
dropping non-finite points and breaking the polyline (`:854-859`), with no
bisection to locate the asymptote, so curves stop short of where they should
go. Adaptive subdivision on chord deviation is the standard answer.

### Accumulated float error in tick loops

`skia_backend.rs:671-681` runs `let mut t = start; while t <= end + 1e-4 { t += step; }`.
Error grows linearly in the tick count, and the `+ 1e-4` fudge at `:672` is
the symptom. `start + i as f32 * step` over an integer loop is exact.

### Colour is round-tripped through a hex string every frame

`interpolator.rs:41-53` parses `#RRGGBB`, converts to Lab, interpolates, and
formats back with `format!("#{:02X}{:02X}{:02X}")`. That is one allocation and
one 8-bit quantisation **per colour property, per frame** — visible as banding
on slow fades — and there is no alpha channel: `#RRGGBBAA` is not parsed here.

There is also an inconsistency worth naming: the timeline interpolates colour
in Lab while the renderer interpolates gradient stops in sRGB. Two colour
models in one frame.

## Target

Every numerical routine either exact or adaptively accurate to a stated
tolerance; every precondition validated rather than assumed; no silent
degradation of a value the author wrote down.

## Work items

| ID | Item | Acceptance |
|---|---|---|
| `AAA-ACC-01` | Closed-form damped-harmonic spring; parameterise via `easing_params` | `spring(t)` is continuous and resolution-independent; property test asserts monotone approach and correct overshoot |
| `AAA-ACC-02` | `ease_css` routed through `cubic_bezier_easing(0.25, 0.1, 0.25, 1.0)` | Matches the CSS spec curve to 1e-6 at 100 sample points |
| `AAA-ACC-03` | Newton–Raphson inversion with bisection fallback | Same accuracy in 4–8 iterations rather than 32 |
| `AAA-ACC-04` | Validate bezier control points in `[0,1]` and spline keypoints sorted/distinct | Both rejected with a structured error and a fix suggestion |
| `AAA-ACC-05` | Non-finite interpolation results become errors, not `null` | A NaN keyframe pair is a validation error |
| `AAA-ACC-06` | Adaptive plot sampling with chord-deviation subdivision and pole bisection | `tan(x)` renders to the asymptote; `sin(50x)` does not facet |
| `AAA-ACC-07` | Exact tick sequences (`start + i * step`) | No `1e-4` fudge; tick positions exact at 1e6 ticks |
| `AAA-ACC-08` | OKLab interpolation in float, alpha-aware, formatted once at the edge | No per-frame allocation, no 8-bit round trip, `#RRGGBBAA` supported |
| `AAA-ACC-09` | One colour model: gradients interpolate where the timeline does | Gradient midpoints match a two-keyframe fade of the same colours |
| `AAA-ACC-10` | `hash01` returns a true `[0,1)` | Property test over the whole `u32` domain |
| `AAA-ACC-11` | Tokenise plot expressions instead of substring rewriting | `asin(x)` evaluates correctly (today it becomes `amath::sin(x)`) |
| `AAA-ACC-12` | f64 through the plot domain rather than narrowing at the JSON boundary | Large `x_range` values keep more than ~7 significant digits |

## Metrics moved

Accuracy — a new scorecard dimension, 70 → 96.

## Sequencing

Wave 2 entirely. `AAA-ACC-02`, `04`, `05`, `10`, `11` are small and land
first; `01`, `03`, `06`, `08` are the substantive ones. All of them are gated
by the property-test suite in [10](10-testing-verification.md), which is
written alongside — several of these defects are exactly what a twenty-line
proptest would have found.
