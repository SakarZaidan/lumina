# Writing Lumina scenes

You are writing **LSF**, the Lumina Scene Format: one JSON document that
describes an animation. There is no code in it and no order of execution —
objects say what exists, the timeline says what their properties are at which
times, and the engine renders any frame from that alone.

Write the whole document, validate it, apply what the errors tell you, and
render. Validation is microseconds and rendering is seconds, so never skip it.

## The document

```json
{
  "version": "1.0",
  "meta": { "title": "Dot product", "author": "you", "created_at": "2026-01-01T00:00:00Z" },
  "canvas": { "width": 1920, "height": 1080, "fps": 60, "duration": 6.0, "background": "#0F0F1A" },
  "objects": {
    "dot": { "type": "Circle", "properties": { "cx": 960, "cy": 540, "radius": 40, "fill": "#F78166" } }
  },
  "timeline": [
    { "time": 0.0, "object": "dot", "state": { "opacity": 0 } },
    { "time": 1.0, "object": "dot", "state": { "opacity": 1 }, "easing": "ease_out_cubic" }
  ]
}
```

- `version` is `"1.0"`. It is the format's version, not yours.
- Keys of `objects` are ids you choose; use `snake_case` and keep them stable,
  because the timeline, groups, plots and events refer to them.
- Coordinates are pixels, `y` grows downward, and the origin is the top left.
- Times are seconds.

## Objects

Every object is `{ "type": ..., "properties": { ... } }`. The seventeen types,
with what they require and what they also accept:

| Type | Required | Also accepts |
|---|---|---|
| `Circle` | `cx`, `cy`, `radius` | `fill`, `stroke`, `stroke_width`, `shadow`, `opacity`, `z_index` |
| `Rectangle` | `x`, `y`, `width`, `height` | `fill`, `stroke`, `stroke_width`, `rx`, `ry`, `shadow`, `opacity`, `z_index` |
| `Polygon` | `points` | `fill`, `stroke`, `stroke_width`, `shadow`, `opacity`, `z_index` |
| `Path` | `d` | `fill`, `stroke`, `stroke_width`, `draw_fraction`, `shadow`, `opacity`, `z_index` |
| `Line` | `x1`, `y1`, `x2`, `y2` | `stroke`, `stroke_width`, `dash`, `draw_fraction`, `opacity`, `z_index` |
| `Arrow` | `from`, `to` | `color`, `stroke_width`, `label`, `opacity`, `z_index` |
| `Text` | `content`, `x`, `y`, `font_size` | `font_id`, `color`, `align`, `letter_spacing`, `opacity`, `z_index` |
| `LaTeX` | `expression`, `x`, `y`, `font_size` | `font_id`, `color`, `draw_fraction`, `align`, `letter_spacing`, `opacity`, `z_index` |
| `MathML` | `markup`, `x`, `y`, `font_size` | `font_id`, `color`, `align`, `letter_spacing`, `opacity`, `z_index` |
| `Image` | `asset_id`, `x`, `y` | `width`, `height`, `rotation`, `opacity`, `z_index` |
| `SVG` | `asset_id`, `x`, `y` | `width`, `height`, `rotation`, `opacity`, `z_index` |
| `Group` | `children`, `x`, `y` | `scale`, `rotation`, `opacity`, `z_index` |
| `NumberLine` | `start`, `end`, `step`, `x`, `y` | `length`, `color`, `opacity`, `z_index` |
| `Axes` | `x_range`, `y_range`, `x`, `y` | `scale`, `x_step`, `y_step`, `x_label`, `y_label`, `grid`, `color`, `opacity`, `z_index` |
| `Plot` | `function_str`, `axes_id` | `color`, `stroke_width`, `sample_count`, `draw_fraction`, `opacity`, `z_index` |
| `BezierCurve` | `p0`, `p1`, `p2`, `p3` | `stroke`, `stroke_width`, `draw_fraction`, `opacity`, `z_index` |
| `Particles` | `count`, `emitter_x`, `emitter_y` | `lifetime`, `speed`, `spread`, `size`, `color`, `opacity`, `z_index` |

Points are `[x, y]` pairs: `from`, `to`, `p0`–`p3` are one pair each, `points`
is a list of them, `x_range` and `y_range` are `[min, max]`.

**Property names are checked.** A name that is not in the type's list is an
error (`UNKNOWN_PROPERTY`) naming the nearest real one — it is never ignored.
Ask for a type's exact shape with a schema scoped to it rather than guessing.

## The timeline

A timeline entry sets properties of one object at one time:

```json
{ "time": 2.0, "object": "dot", "state": { "cx": 1400 }, "easing": "ease_in_out_cubic" }
```

- Between two entries for the same property, the value is interpolated.
  `easing` belongs to the entry being moved **to**, as in CSS.
- Before the first entry for a property, and after the last, the value holds.
- An entry at `time: 0` wins over the value written in `objects`, so a fade-in
  is `opacity: 0` at 0 and `opacity: 1` later, with either value authored.
- Only properties that appear in some `state` animate; everything else stays as
  authored.
- Numbers, colours (blended perceptually), and point lists all interpolate.
  Anything else — text, booleans, ids — switches at the next keyframe.
- Integer properties (`z_index`, `count`, `sample_count`) are rounded.
- A value must fit its property: `"radius": "big"` is an error, and so is a
  point list of the wrong shape (`PROPERTY_VALUE_INVALID`).

### Easing

`linear` · `ease` · `ease_in` · `ease_out` · `ease_in_out` ·
`ease_in_quad` · `ease_out_quad` · `ease_in_out_quad` ·
`ease_in_cubic` · `ease_out_cubic` · `ease_in_out_cubic` ·
`ease_in_quart` · `ease_out_quart` · `ease_in_out_quart` ·
`ease_in_sine` · `ease_out_sine` · `ease_in_out_sine` ·
`ease_in_expo` · `ease_out_expo` · `ease_in_circ` · `ease_out_circ` ·
`ease_in_elastic` · `ease_out_elastic` · `ease_in_out_elastic` ·
`ease_in_bounce` · `ease_out_bounce` · `spring` · `smooth` ·
`rush_into` · `rush_from` · `there_and_back` · `cubic_bezier` · `spline`

`cubic_bezier` takes `"easing_params": [x1, y1, x2, y2]`, and `spline` takes
`"easing_params": { "keypoints": [[t, v], ...] }` with `t` increasing. Any
other name is an error with the nearest match.

## Colour and paint

Colours are hex strings: `"#RGB"`, `"#RRGGBB"` or `"#RRGGBBAA"`. There is no
`"none"` — for no fill use an alpha of `00`. A `fill` or `stroke` may also be a
gradient:

```json
{ "type": "linear", "stops": [[0.0, "#F78166"], [1.0, "#3D1A12"]], "angle": 45 }
```

`"type"` is `"linear"` (the default) or `"radial"`; a radial gradient's
`radius` is a fraction of half the shape's larger side. Two stops at least.

## Structure

- **Draw order** is `z_index` ascending, then object id. Groups draw their
  children.
- **A `Group`** moves, scales and rotates its `children`, whose coordinates are
  relative to the group. Every child id must exist, and a group cannot contain
  itself at any depth.
- **`Plot`** needs `axes_id` pointing at an `Axes` object; `function_str` is an
  expression in `x`, e.g. `"sin(x) * 2"`.
- **Assets.** `Image` and `SVG` need `asset_id` declared in
  `assets.images`; `font_id` needs `assets.fonts`. Both are paths on disk.
- **Reveals.** `draw_fraction` between 0 and 1 draws that fraction of a stroke,
  by arc length. On `LaTeX` it reveals characters.

## What to do with errors

Every error carries a `code`, a JSON `path`, a `message` and a
`fix_suggestion`; some also carry a `fix_patch`, an RFC 6902 patch that applies
the fix exactly. Apply those without asking anyone — `lumina-cli fix`, the
`lumina_fix` tool and `POST /patch` all do it — and spend your own effort on
the rest.

The errors worth knowing before you write:

| Code | What it means |
|---|---|
| `UNKNOWN_PROPERTY` | The property is not on that object type. Use the suggested name. |
| `PROPERTY_TYPE_MISMATCH` | Right name, wrong kind of value: a string where a number belongs. |
| `PROPERTY_VALUE_INVALID` | Right kind, wrong shape: a point with one coordinate. |
| `UNKNOWN_OBJECT_ID` | A timeline entry, group child or event names an id that is not in `objects`. |
| `UNKNOWN_EASING` | Not one of the names above. |
| `INVALID_COLOR` | Not a hex colour. `"none"` is not a colour. |
| `UNKNOWN_ASSET_ID` | The asset is not declared under `assets`. |
| `CIRCULAR_GROUP_REFERENCE` | A group contains itself, directly or through another. |
| `TOO_MANY_FRAMES`, `CANVAS_TOO_LARGE`, `FPS_TOO_HIGH` | The scene asks for more work than the engine will do; make it smaller. |

## Habits that produce valid scenes

- Write the objects first, then the timeline. Give every object an id you can
  read in six months.
- Keep the canvas at 1920×1080 and 30 or 60 fps unless asked otherwise, and
  keep `duration` to what the animation needs.
- Animate `opacity`, position and scale; they read well and interpolate
  exactly.
- Reach for `Axes` + `Plot` for anything mathematical rather than approximating
  a curve with a `Path`.
- Use `Group` when several objects move together; it is one keyframe instead of
  many, and it cannot drift out of alignment.
- Do not invent properties to express something the format does not have. If a
  scene needs logic, generate the scene from a program and let LSF stay data.
