# Visual Effects & Assets

Lumina renders more than flat shapes. The features below all degrade gracefully:
omit a field and you get the previous behavior, so older scenes render unchanged.

## Gradients

`fill` and `stroke` on closed shapes (`Circle`, `Rectangle`, `Polygon`, `Path`)
accept either a hex string or a gradient object:

```json
"fill": { "type": "linear", "stops": [[0.0, "#F78166"], [1.0, "#1F6FEB"]], "angle": 45 }
"fill": { "type": "radial", "stops": [[0.0, "#FFFFFF"], [1.0, "#0B132B"]], "radius": 0.8 }
```

`angle` is in degrees; `radius` is a fraction of the shape's bounding box. Stops are `[position 0..1, "#hex"]`.

## Drop shadows / glow

Any closed shape may declare an optional `shadow`:

```json
"shadow": { "color": "#000000", "blur": 12, "dx": 0, "dy": 6, "opacity": 0.5 }
```

The shape silhouette is blurred (separable box blur) and composited beneath the
shape. Shadows are opt-in — they cost extra render time only when present.

## Rounded rectangles

```json
{ "type": "Rectangle", "properties": { "x": 100, "y": 100, "width": 400, "height": 200, "rx": 24, "ry": 24, "fill": "#161B22" } }
```

`ry` falls back to `rx` when omitted. `rx: 0` keeps the fast sharp-corner path.

## Text styling

```json
{ "type": "Text", "properties": { "content": "Centered", "x": 960, "y": 200, "align": "center", "letter_spacing": 2, "font_size": 64 } }
```

`align` is `left` (default), `center`, or `right` around `(x, y)`; `letter_spacing` adds pixels between glyphs. The same fields work on `LaTeX` and `MathML`.

## Images, SVG, and animated GIFs

Declare assets, then place them:

```json
"assets": { "images": [{ "id": "logo", "path": "./assets/logo.svg" }, { "id": "spark", "path": "./assets/spark.gif" }] },
"objects": {
  "brand": { "type": "SVG",   "properties": { "asset_id": "logo",  "x": 40, "y": 40, "width": 120, "height": 120 } },
  "fx":    { "type": "Image", "properties": { "asset_id": "spark", "x": 800, "y": 400, "width": 256, "height": 256, "rotation": 0 } }
}
```

- **Raster** (PNG/JPEG/WebP) and **SVG** (rasterized via resvg) are composited with position, resize, `rotation`, and `opacity`, honoring camera/group transforms and z-order.
- **Animated GIFs** advance with the timeline: at each frame the engine selects the GIF frame whose cumulative delay window contains the current time, looping over the total duration.

## Particles

A deterministic emitter — particles are computed analytically from the current
time plus a per-particle seed, so output is reproducible frame to frame.

```json
{ "type": "Particles", "properties": { "count": 300, "emitter_x": 960, "emitter_y": 540, "speed": 160, "spread": 360, "lifetime": 1.5, "size": 4, "color": "#F0A202" } }
```
