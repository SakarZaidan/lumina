# The Scene Format (LSF)

LSF is a pure-JSON, declarative description of a scene. It is **declarative only** (no functions, loops, or conditionals), **self-describing** (every object names its type and properties), and **validatable** against a published JSON Schema.

## Top-level shape

```json
{
  "version": "1.0",
  "meta":   { "title": "...", "author": "...", "created_at": "..." },
  "canvas": { "width": 1920, "height": 1080, "fps": 60, "duration": 12.0, "background": "#0F0F1A" },
  "assets": { "fonts": [{ "id": "sans", "path": "..." }], "images": [{ "id": "logo", "path": "..." }] },
  "objects": { "<id>": { "type": "Circle", "properties": { ... } } },
  "timeline": [ { "time": 1.0, "object": "<id>", "state": { ... }, "easing": "ease_out_cubic" } ],
  "events":   [ { "object": "<id>", "trigger": "click", "action": { ... } } ],
  "camera":   { "timeline": [ { "time": 0.0, "state": { "x": 0, "y": 0, "zoom": 1.0, "rotation": 0 } } ] }
}
```

## Object types

| Type | Required | Notable optional |
|---|---|---|
| `Circle` | `cx, cy, radius` | `fill`, `stroke`, `shadow` |
| `Rectangle` | `x, y, width, height` | `fill`, `rx`, `ry`, `shadow` |
| `Polygon` | `points` | `fill`, `stroke`, `shadow` |
| `Path` | `d` (SVG path) | `fill`, `stroke`, `draw_fraction` |
| `Line` | `x1,y1,x2,y2` | `dash`, `draw_fraction` |
| `Arrow` | `from, to` | `label`, `stroke_width` |
| `Text` | `content, x, y, font_size` | `align`, `letter_spacing`, `font_id` |
| `LaTeX` | `expression, x, y, font_size` | `draw_fraction`, `align` |
| `MathML` | `markup, x, y, font_size` | `align` |
| `Image` / `SVG` | `asset_id, x, y` | `width`, `height`, `rotation` |
| `Group` | `children, x, y` | `scale`, `rotation` |
| `NumberLine` | `start, end, step, x, y` | `length` |
| `Axes` | `x_range, y_range, x, y` | `scale`, `grid`, `x_step` |
| `Plot` | `function_str, axes_id` | `sample_count`, `draw_fraction` |
| `BezierCurve` | `p0,p1,p2,p3` | `draw_fraction` |
| `Particles` | `count, emitter_x, emitter_y` | `lifetime`, `speed`, `spread` |

Every type also accepts `z_index` and `opacity`. The full, authoritative list of
properties is in the [Schema Reference](./schema-reference.md) and from the
`/objects` endpoint.

## Timeline & conflict rules

- The timeline is a flat list of keyframes; each targets one object and a set of properties at a `time` (seconds).
- Between two keyframes for the same property, the value is interpolated and the named `easing` is applied (the easing named on the **destination** keyframe wins, CSS-style).
- Colors interpolate in **CIELAB**; point arrays and SVG paths **morph** vertex-by-vertex (padding the shorter one).
- A property that appears in the timeline but not in the object's initial `properties` uses the type default.

## Easings

28 named easing functions: `linear`, the quad/cubic/quart/sine
in/out/in-out families, `ease_in_expo`/`ease_out_expo`,
`ease_in_circ`/`ease_out_circ`, elastic and bounce variants, `spring` (RK4
physics), plus the Manim-style `smooth`, `rush_into`, `rush_from`,
`there_and_back`, and the CSS aliases `ease`, `ease_in`, `ease_out`,
`ease_in_out`. Two are parameterized via `easing_params`:

```json
{ "time": 2.0, "object": "box", "state": { "x": 800 },
  "easing": "cubic_bezier", "easing_params": [0.34, 1.56, 0.64, 1.0] }
```

`cubic_bezier(x1, y1, x2, y2)` implements the CSS spec (binary-search
parametric solver). `spline` interpolates through arbitrary keypoints with
monotone-cubic (Fritsch–Carlson) segments — guaranteed overshoot-free:

```json
{ "time": 4.0, "object": "dot", "state": { "y": 200 },
  "easing": "spline",
  "easing_params": { "keypoints": [[0.0, 0.0], [0.3, 0.9], [0.7, 0.4], [1.0, 1.0]] } }
```

> Since v0.4, an unrecognized easing name is a **validation error**
> (`UNKNOWN_EASING`) with a did-you-mean suggestion — the CLI refuses to
> render such a scene (`lumina-cli --check` validates without rendering),
> and `POST /validate` reports it. `cubic_bezier`/`spline` without
> `easing_params` produce a `MISSING_EASING_PARAMS` warning and fall back
> to the CSS `ease` curve / `linear` respectively.

## Groups & transforms

A child's coordinates are relative to its parent `Group`'s transform. Animate a group's `scale`/`rotation`/`x`/`y` to move many children together; the child's world position is `parent_transform × child_transform`.
