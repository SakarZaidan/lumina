#!/usr/bin/env python3
"""
Generate the v0.3.0 GRAND showcase — a ~45s reel that proves the new release:

  * GPU parity: Text, LaTeX, SVG and Particles now render on the Vello backend
    (previously Skia-only). The whole scene is authored with SOLID colors so it
    looks identical on Skia and Vello — render it with `--backend vello`.
  * `spline` easing: custom monotone-cubic motion curves.
  * Interactive event annotations: `tween_to`, `emit_custom` with `$drag` payload.
  * Camera moves, draw-on plots, LAB color transitions.

Run:
  python3 examples/gen_grand_showcase.py
  lumina-cli --scene examples/showcase_grand.lsf --backend vello \\
             --output media/showcase_grand.mp4 --format mp4
"""
import json
import math
import os

FONT_REG = "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"
FONT_BOLD = "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf"
LOGO = os.path.join(os.path.dirname(__file__), "assets", "lumina_node.svg")

SCALE = float(os.environ.get("LUMINA_SHOWCASE_SCALE", 2.0 / 3.0))  # 1.0=1080p, 2/3=720p
W, H, FPS, DURATION = 1920, 1080, 30, 45.0
CX, CY = 960.0, 540.0

PIXEL_KEYS = {"x", "y", "cx", "cy", "x1", "y1", "x2", "y2", "width", "height",
              "radius", "font_size", "stroke_width", "scale", "emitter_x",
              "emitter_y", "speed", "size", "letter_spacing", "rx", "ry", "length"}

objects, timeline, events = {}, [], []


def scale_props(d):
    out = {}
    for k, v in d.items():
        if k in PIXEL_KEYS and isinstance(v, (int, float)):
            out[k] = v * SCALE
        elif k in ("from", "to") and isinstance(v, list):
            out[k] = [c * SCALE if isinstance(c, (int, float)) else c for c in v]
        else:
            out[k] = v
    return out


def add(obj_id, otype, props):
    objects[obj_id] = {"type": otype, "properties": scale_props(props)}


def kf(t, obj_id, state, easing="ease_out_cubic", params=None):
    entry = {"time": round(t, 3), "object": obj_id, "state": scale_props(state), "easing": easing}
    if params is not None:
        entry["easing_params"] = params
    timeline.append(entry)


def fade(obj_id, t0, t1, a0=0.0, a1=1.0, easing="ease_out_cubic"):
    kf(t0, obj_id, {"opacity": a0}, "linear")
    kf(t1, obj_id, {"opacity": a1}, easing)


BG = "#0D1117"
ACCENT = "#58A6FF"
PINK = "#F778BA"
GREEN = "#3FB950"
GOLD = "#E3B341"

# ── Background ───────────────────────────────────────────────────────────────
add("bg", "Rectangle", {"x": 0, "y": 0, "width": W, "height": H, "z_index": 0,
                        "fill": BG, "opacity": 1.0})
add("frame", "Rectangle", {"x": 50, "y": 50, "width": W - 100, "height": H - 100, "rx": 28, "ry": 28,
                           "z_index": 1, "fill": "#0F1626", "stroke": "#21304A", "stroke_width": 3, "opacity": 0.0})
fade("frame", 0.0, 1.5)

# ── Act 1: Title (0–8s), spline-eased entrance ───────────────────────────────
add("logo", "SVG", {"asset_id": "node", "x": 860, "y": 250, "width": 200, "height": 200, "z_index": 6, "opacity": 0.0})
# Spline easing: a custom overshoot-free rise authored via keypoints.
kf(0.4, "logo", {"opacity": 0.0, "y": 300, "rotation": -30}, "linear")
kf(2.6, "logo", {"opacity": 1.0, "y": 250, "rotation": 0}, "spline",
   params={"keypoints": [[0.0, 0.0], [0.25, 0.55], [0.6, 0.92], [1.0, 1.0]]})

add("title", "Text", {"content": "LUMINA  v0.3.0", "x": 960, "y": 545, "align": "center",
                      "font_id": "bold", "font_size": 96, "color": "#F0F6FC", "z_index": 7, "opacity": 0.0})
fade("title", 1.0, 3.2)
add("tagline", "Text", {"content": "GPU parity  ·  spline easing  ·  interactive events", "x": 960, "y": 625,
                        "align": "center", "letter_spacing": 3, "font_id": "sans", "font_size": 36,
                        "color": "#8B98A9", "z_index": 7, "opacity": 0.0})
fade("tagline", 2.0, 4.0)

# Spark burst behind the title.
add("spark", "Particles", {"count": 160, "emitter_x": 960, "emitter_y": 500, "lifetime": 2.2,
                           "speed": 150, "spread": 360, "size": 4, "color": GOLD, "z_index": 5, "opacity": 0.0})
kf(2.0, "spark", {"opacity": 0.0}, "linear")
kf(2.6, "spark", {"opacity": 0.9}, "ease_out_cubic")
kf(6.0, "spark", {"opacity": 0.0}, "ease_in_cubic")

for oid in ("title", "tagline", "logo"):
    kf(7.2, oid, {"opacity": 1.0}, "linear")
    kf(8.2, oid, {"opacity": 0.0}, "ease_in_cubic")

# ── Act 2: "Now on the GPU" — text + LaTeX + SVG + particles on Vello (8–20s) ─
add("act2", "Text", {"content": "Text, math, images & particles — now on the GPU", "x": 960, "y": 150,
                     "align": "center", "font_id": "bold", "font_size": 44, "color": ACCENT, "z_index": 7, "opacity": 0.0})
fade("act2", 8.5, 10.0)
kf(18.5, "act2", {"opacity": 1.0}, "linear"); kf(19.5, "act2", {"opacity": 0.0}, "ease_in_cubic")

# Formulas chosen to render cleanly through the Unicode pipeline (super/sub-
# scripts, Greek, \frac, \sum) on BOTH backends.
formulas = [
    ("f_emc", r"E = mc^2", 360, GREEN),
    ("f_basel", r"\frac{\pi^2}{6} = \sum \frac{1}{n^2}", 540, "#F0F6FC"),
    ("f_deriv", r"\frac{d}{dx} e^x = e^x", 720, PINK),
]
for i, (fid, expr, y, color) in enumerate(formulas):
    add(fid, "LaTeX", {"expression": expr, "x": 960, "y": y, "align": "center",
                       "font_id": "bold", "font_size": 60, "color": color, "z_index": 6,
                       "draw_fraction": 1.0, "opacity": 0.0})
    t0 = 10.0 + i * 1.4
    kf(t0, fid, {"opacity": 0.0, "draw_fraction": 0.0}, "linear")
    kf(t0 + 1.6, fid, {"opacity": 1.0, "draw_fraction": 1.0}, "ease_out_cubic")
    kf(18.5, fid, {"opacity": 1.0}, "linear"); kf(19.5, fid, {"opacity": 0.0}, "ease_in_cubic")

# Two glowing emitters proving GPU particles.
for eid, ex, color in (("burstL", 360, ACCENT), ("burstR", 1560, PINK)):
    add(eid, "Particles", {"count": 120, "emitter_x": ex, "emitter_y": 540, "lifetime": 2.0,
                           "speed": 90, "spread": 360, "size": 3.5, "color": color, "z_index": 4, "opacity": 0.0})
    fade(eid, 11.0, 12.5, 0.0, 0.85)
    kf(18.5, eid, {"opacity": 0.85}, "linear"); kf(19.5, eid, {"opacity": 0.0}, "ease_in_cubic")

# ── Act 3: Math viz — Axes + spline-drawn plot + camera zoom (20–33s) ────────
add("axes", "Axes", {"x": 520, "y": 760, "x_range": [0, 12], "y_range": [-1.4, 1.4],
                     "scale": 78, "x_step": 2, "y_step": 1, "grid": True,
                     "color": "#3B4A63", "z_index": 3, "opacity": 0.0})
fade("axes", 20.5, 22.0)
add("plot", "Plot", {"function_str": "math::sin(x) * math::cos(x*0.6)", "axes_id": "axes", "color": GOLD,
                     "stroke_width": 4, "sample_count": 240, "draw_fraction": 0.0, "z_index": 4, "opacity": 0.0})
kf(22.0, "plot", {"opacity": 1.0, "draw_fraction": 0.0}, "linear")
# Draw-on driven by a spline curve — slow start, fast middle, gentle settle.
kf(28.0, "plot", {"draw_fraction": 1.0}, "spline",
   params={"keypoints": [[0.0, 0.0], [0.35, 0.18], [0.7, 0.85], [1.0, 1.0]]})
add("plot_lbl", "LaTeX", {"expression": r"y = \sin x \cdot \cos(0.6x)", "x": 960, "y": 180,
                          "align": "center", "font_id": "bold", "font_size": 48, "color": GOLD,
                          "z_index": 6, "opacity": 0.0})
fade("plot_lbl", 23.0, 24.5)

# Camera push-in then out across the act.
camera = {"timeline": [
    {"time": 20.0, "state": {"x": 0, "y": 0, "zoom": 1.0}, "easing": "linear"},
    {"time": 27.0, "state": {"x": -120 * SCALE, "y": 40 * SCALE, "zoom": 1.25}, "easing": "ease_in_out_sine"},
    {"time": 33.0, "state": {"x": 0, "y": 0, "zoom": 1.0}, "easing": "ease_in_out_sine"},
]}
for oid in ("axes", "plot", "plot_lbl"):
    kf(32.0, oid, {"opacity": 1.0}, "linear"); kf(33.5, oid, {"opacity": 0.0}, "ease_in_cubic")

# ── Act 4: Interactive + finale (33–45s) ─────────────────────────────────────
add("act4", "Text", {"content": "Declarative  ·  Interactive  ·  Fast", "x": 960, "y": 470, "align": "center",
                     "font_id": "bold", "font_size": 72, "color": "#F0F6FC", "z_index": 7, "opacity": 0.0})
fade("act4", 34.0, 36.0)
add("act4b", "Text", {"content": "JSON in — MP4 / WebM / GIF / WASM out", "x": 960, "y": 560, "align": "center",
                      "letter_spacing": 2, "font_id": "sans", "font_size": 36, "color": "#8B98A9", "z_index": 7, "opacity": 0.0})
fade("act4b", 35.0, 37.0)

# A draggable vector with interactive event annotations (visible in the .lsf;
# fired by a host at runtime — see events[] below).
add("vec", "Arrow", {"from": [760, 760], "to": [1160, 640], "color": ACCENT, "stroke_width": 6,
                     "z_index": 6, "opacity": 0.0})
fade("vec", 36.0, 37.5)
add("vec_lbl", "LaTeX", {"expression": r"\vec{v}", "x": 1180, "y": 630, "font_id": "bold",
                         "font_size": 44, "color": ACCENT, "z_index": 6, "opacity": 0.0})
fade("vec_lbl", 36.5, 38.0)

# Finale particle fountain.
add("finale", "Particles", {"count": 220, "emitter_x": 960, "emitter_y": 980, "lifetime": 2.6,
                            "speed": 150, "spread": 150, "size": 4, "color": GOLD, "z_index": 5, "opacity": 0.0})
fade("finale", 38.0, 39.5, 0.0, 0.9)
kf(44.0, "finale", {"opacity": 0.9}, "linear"); kf(45.0, "finale", {"opacity": 0.0}, "ease_in_cubic")

# Interactive event annotations exercising the v0.3.0 action set.
events = [
    {"object": "vec", "trigger": "drag",
     "action": {"type": "emit_custom", "event_name": "vector_moved",
                "properties": {"from": "$drag.from", "to": "$drag.to"}}},
    {"object": "vec", "trigger": "click",
     "action": {"type": "tween_to", "target": "vec", "property": "color",
                "value": GOLD, "duration": 0.4, "easing": "ease_out_cubic"}},
    {"object": "title", "trigger": "click",
     "action": {"type": "jump_to_time", "value": 0.0}},
]

scene = {
    "version": "1.0",
    "meta": {"title": "Lumina v0.3.0 — Grand Showcase", "author": "lumina", "created_at": "2026-06-01T00:00:00Z"},
    "canvas": {"width": int(W * SCALE), "height": int(H * SCALE), "fps": FPS,
               "duration": DURATION, "background": BG},
    "assets": {
        "fonts": [{"id": "sans", "path": FONT_REG}, {"id": "bold", "path": FONT_BOLD}],
        "images": [{"id": "node", "path": LOGO}],
    },
    "objects": objects,
    "timeline": sorted(timeline, key=lambda e: e["time"]),
    "events": events,
    "camera": camera,
}

out = os.path.join(os.path.dirname(__file__), "showcase_grand.lsf")
with open(out, "w") as f:
    json.dump(scene, f, indent=2)
print(f"Wrote {out}: {len(objects)} objects, {len(timeline)} keyframes, "
      f"{len(events)} events, {int(W*SCALE)}x{int(H*SCALE)} @ {FPS}fps, {DURATION}s")
