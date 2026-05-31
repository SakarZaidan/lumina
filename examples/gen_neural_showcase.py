#!/usr/bin/env python3
"""
Generate the flagship Lumina showcase: a ~2.5 minute neural-network /
backpropagation explainer at 1920x1080. Emits examples/showcase_neural_network.lsf.

It exercises the engine's full feature set: gradient + shadowed rounded panels,
an SVG logo, radial-gradient "glowing" neurons, draw-on connections, particle
activation bursts, a live loss curve, LaTeX formulas, camera moves and LAB color
transitions.

Run:  python3 examples/gen_neural_showcase.py
"""
import json
import math
import os

FONT_REG = "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"
FONT_BOLD = "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf"
LOGO = os.path.join(os.path.dirname(__file__), "assets", "lumina_node.svg")

# Authoring resolution is 1920x1080. SCALE renders the scene at a lower output
# resolution while preserving the exact layout (every pixel-space value is scaled
# by the same factor). 1.0 = 1080p, 2/3 = 720p. Override with LUMINA_SHOWCASE_SCALE.
SCALE = float(os.environ.get("LUMINA_SHOWCASE_SCALE", 2.0 / 3.0))
W, H, FPS, DURATION = 1920, 1080, 30, 150.0
CY = 540.0

# Keys whose numeric values are pixel-space and must scale with the canvas.
# Deliberately excludes data-space keys (x_range, y_step, count, lifetime,
# spread, rotation, opacity, ...) and the gradient `radius` fraction, which is
# never reached because we don't recurse into fill/stroke values.
PIXEL_KEYS = {"x", "y", "cx", "cy", "x1", "y1", "x2", "y2", "width", "height",
              "radius", "font_size", "stroke_width", "scale", "emitter_x",
              "emitter_y", "speed", "size", "letter_spacing", "rx", "ry"}

objects = {}
timeline = []


def scale_props(d):
    """Scale pixel-space values in a props/state dict by SCALE, leaving colors,
    gradients, data ranges, and other non-geometric fields untouched."""
    out = {}
    for k, v in d.items():
        if k in PIXEL_KEYS and isinstance(v, (int, float)):
            out[k] = v * SCALE
        elif k in ("from", "to") and isinstance(v, list):
            out[k] = [c * SCALE if isinstance(c, (int, float)) else c for c in v]
        elif k == "shadow" and isinstance(v, dict):
            out[k] = {kk: (vv * SCALE if kk in ("blur", "dx", "dy") and isinstance(vv, (int, float)) else vv)
                      for kk, vv in v.items()}
        else:
            out[k] = v
    return out


def add(obj_id, otype, props):
    objects[obj_id] = {"type": otype, "properties": scale_props(props)}


def kf(t, obj_id, state, easing="ease_out_cubic"):
    timeline.append({"time": round(t, 3), "object": obj_id, "state": scale_props(state), "easing": easing})


def fade(obj_id, t0, t1, a0=0.0, a1=1.0, easing="ease_out_cubic"):
    kf(t0, obj_id, {"opacity": a0}, "linear")
    kf(t1, obj_id, {"opacity": a1}, easing)


# ── Layer geometry ────────────────────────────────────────────────────────────
LAYERS = [
    {"name": "in", "n": 4, "x": 540.0, "color": "#58A6FF", "label": "Input"},
    {"name": "hid", "n": 6, "x": 960.0, "color": "#F778BA", "label": "Hidden"},
    {"name": "out", "n": 3, "x": 1380.0, "color": "#3FB950", "label": "Output"},
]
GAP = 132.0
R = 30.0


def neuron_pos(layer):
    n = layer["n"]
    return [(layer["x"], CY + (i - (n - 1) / 2.0) * GAP) for i in range(n)]


for layer in LAYERS:
    layer["pos"] = neuron_pos(layer)


def grad(c_center, c_edge):
    return {"type": "radial", "stops": [[0.0, c_center], [1.0, c_edge]], "radius": 0.95}


# ── Background ──────────────────────────────────────────────────────────────
add("bg_panel", "Rectangle", {
    "x": 60, "y": 60, "width": W - 120, "height": H - 120, "rx": 36, "ry": 36, "z_index": 0,
    "fill": {"type": "linear", "stops": [[0.0, "#0D1117"], [1.0, "#161B2E"]], "angle": 90},
    "stroke": "#21304A", "stroke_width": 2, "opacity": 0.0,
})
fade("bg_panel", 0.0, 2.0, 0.0, 1.0)

# ── Act 1: Title (0–8s) ────────────────────────────────────────────────────
add("logo", "SVG", {"asset_id": "node", "x": 860, "y": 250, "width": 200, "height": 200, "z_index": 5, "opacity": 0.0})
kf(0.5, "logo", {"opacity": 0.0, "rotation": -40}, "linear")
kf(3.0, "logo", {"opacity": 1.0, "rotation": 0}, "ease_out_elastic")

add("title", "Text", {"content": "How a Neural Network Learns", "x": 960, "y": 540, "align": "center",
                       "font_id": "bold", "font_size": 86, "color": "#F0F6FC", "z_index": 6, "opacity": 0.0})
fade("title", 1.0, 3.5)
add("subtitle", "Text", {"content": "forward pass  ·  loss  ·  backpropagation", "x": 960, "y": 620,
                          "align": "center", "letter_spacing": 4, "font_id": "sans", "font_size": 38,
                          "color": "#8B98A9", "z_index": 6, "opacity": 0.0})
fade("subtitle", 2.0, 4.0)

# Title exit
for oid in ("title", "subtitle", "logo"):
    kf(7.5, oid, {"opacity": objects[oid]["properties"].get("opacity", 1.0) or 1.0}, "linear")
    kf(8.5, oid, {"opacity": 0.0}, "ease_in_cubic")
kf(8.0, "title", {"opacity": 1.0}, "linear")
kf(9.0, "title", {"opacity": 0.0}, "ease_in_cubic")

# ── Layer labels ────────────────────────────────────────────────────────────
for layer in LAYERS:
    lid = f"label_{layer['name']}"
    add(lid, "Text", {"content": layer["label"], "x": layer["x"], "y": 230, "align": "center",
                      "font_id": "bold", "font_size": 34, "color": layer["color"], "z_index": 4, "opacity": 0.0})
    fade(lid, 9.0, 11.0)

# ── Neurons (pop-in 10–22s, staggered by layer) ──────────────────────────────
layer_start = {"in": 10.5, "hid": 13.5, "out": 16.5}
for layer in LAYERS:
    base = layer_start[layer["name"]]
    for i, (x, y) in enumerate(layer["pos"]):
        nid = f"n_{layer['name']}_{i}"
        add(nid, "Circle", {
            "cx": x, "cy": y, "radius": R, "z_index": 3,
            "fill": grad("#FFFFFF", layer["color"]),
            "shadow": {"color": layer["color"], "blur": 22, "dx": 0, "dy": 0, "opacity": 0.0},
            "opacity": 0.0,
        })
        t = base + i * 0.18
        kf(t, nid, {"radius": 0.0, "opacity": 0.0}, "linear")
        kf(t + 0.9, nid, {"radius": R, "opacity": 1.0}, "ease_out_elastic")

# ── Connections (draw-on 22–40s) ─────────────────────────────────────────────
conns = []
for li in range(len(LAYERS) - 1):
    a_layer, b_layer = LAYERS[li], LAYERS[li + 1]
    for ai, (ax, ay) in enumerate(a_layer["pos"]):
        for bi, (bx, by) in enumerate(b_layer["pos"]):
            cid = f"c_{a_layer['name']}{ai}_{b_layer['name']}{bi}"
            conns.append((cid, li))
            add(cid, "Line", {"x1": ax, "y1": ay, "x2": bx, "y2": by, "z_index": 1,
                              "stroke": "#30475E", "stroke_width": 2.0, "draw_fraction": 0.0, "opacity": 0.85})

draw_start = {0: 22.5, 1: 30.0}
for cid, li in conns:
    t = draw_start[li]
    kf(t, cid, {"draw_fraction": 0.0}, "linear")
    kf(t + 6.0, cid, {"draw_fraction": 1.0}, "ease_out_cubic")

# ── Forward-pass formula panel (40–46s) ──────────────────────────────────────
add("fwd_panel", "Rectangle", {"x": 660, "y": 840, "width": 600, "height": 120, "rx": 20, "ry": 20, "z_index": 7,
                                "fill": {"type": "linear", "stops": [[0, "#1B2436"], [1, "#243049"]], "angle": 90},
                                "stroke": "#58A6FF", "stroke_width": 2,
                                "shadow": {"color": "#000000", "blur": 16, "dx": 0, "dy": 8, "opacity": 0.5},
                                "opacity": 0.0})
fade("fwd_panel", 40.0, 41.5)
add("fwd_eq", "LaTeX", {"expression": "a = σ(Wx + b)", "x": 960, "y": 915, "align": "center",
                        "font_id": "bold", "font_size": 54, "color": "#E6EDF3", "z_index": 8,
                        "draw_fraction": 0.0, "opacity": 1.0})
kf(41.5, "fwd_eq", {"draw_fraction": 0.0}, "linear")
kf(45.0, "fwd_eq", {"draw_fraction": 1.0}, "linear")

# ── Activation waves (46–84s): pulse neurons + particle bursts ───────────────
def pulse_layer(layer, t):
    for i, _ in enumerate(layer["pos"]):
        nid = f"n_{layer['name']}_{i}"
        kf(t, nid, {"shadow": {"color": layer["color"], "blur": 22, "dx": 0, "dy": 0, "opacity": 0.0}}, "linear")
        kf(t + 0.4, nid, {"shadow": {"color": layer["color"], "blur": 40, "dx": 0, "dy": 0, "opacity": 0.9}}, "ease_out_cubic")
        kf(t + 1.2, nid, {"shadow": {"color": layer["color"], "blur": 22, "dx": 0, "dy": 0, "opacity": 0.0}}, "ease_in_cubic")


WAVES = [48.0, 60.0, 72.0]
for w in WAVES:
    for li, layer in enumerate(LAYERS):
        pulse_layer(layer, w + li * 1.4)

# Particle bursts at output neurons during waves.
for oi, (x, y) in enumerate(LAYERS[-1]["pos"]):
    pid = f"spark_{oi}"
    add(pid, "Particles", {"count": 70, "emitter_x": x, "emitter_y": y, "lifetime": 1.2, "speed": 150,
                           "spread": 360, "size": 3.5, "color": "#3FB950", "z_index": 9, "opacity": 0.0})
    for w in WAVES:
        t = w + 2 * 1.4
        kf(t, pid, {"opacity": 0.0}, "linear")
        kf(t + 0.3, pid, {"opacity": 1.0}, "ease_out_cubic")
        kf(t + 1.8, pid, {"opacity": 0.0}, "ease_in_cubic")

# ── Act: Loss curve (84–112s). Camera nudges down-right. ─────────────────────
add("axes", "Axes", {"x_range": [0, 10], "y_range": [0, 1.0], "x": 1180, "y": 880, "scale": 60,
                     "x_step": 2, "y_step": 0.25, "grid": True, "color": "#3D5A80", "z_index": 2,
                     "x_label": "epoch", "y_label": "loss", "opacity": 0.0})
fade("axes", 86.0, 88.0)
add("loss_curve", "Plot", {"function_str": "exp(-x/3) * 0.9 + 0.05", "axes_id": "axes", "color": "#F0A202",
                            "stroke_width": 4, "sample_count": 240, "draw_fraction": 0.0, "z_index": 3, "opacity": 1.0})
kf(88.0, "loss_curve", {"draw_fraction": 0.0}, "linear")
kf(98.0, "loss_curve", {"draw_fraction": 1.0}, "ease_out_cubic")
add("loss_eq", "LaTeX", {"expression": "L = ½ Σ (y − ŷ)²", "x": 1480, "y": 360, "align": "center",
                          "font_id": "bold", "font_size": 50, "color": "#F0A202", "z_index": 8,
                          "draw_fraction": 0.0, "opacity": 1.0})
kf(99.0, "loss_eq", {"draw_fraction": 0.0}, "linear")
kf(103.0, "loss_eq", {"draw_fraction": 1.0}, "linear")

# ── Act: Backprop (112–136s): backward arrows + update rule ──────────────────
for li in range(len(LAYERS) - 1, 0, -1):
    a_layer, b_layer = LAYERS[li - 1], LAYERS[li]
    ax, ay = a_layer["x"], CY
    bx, by = b_layer["x"], CY
    aid = f"back_{li}"
    add(aid, "Arrow", {"from": [bx, by - 200], "to": [ax, ay - 200], "color": "#F778BA",
                       "stroke_width": 5, "z_index": 10, "opacity": 0.0})
    t = 114.0 + (len(LAYERS) - 1 - li) * 2.0
    fade(aid, t, t + 1.2)

add("bp_label", "Text", {"content": "gradients flow backward", "x": 960, "y": 300, "align": "center",
                          "font_id": "sans", "font_size": 36, "color": "#F778BA", "z_index": 10, "opacity": 0.0})
fade("bp_label", 116.0, 118.0)

add("upd_panel", "Rectangle", {"x": 660, "y": 840, "width": 600, "height": 120, "rx": 20, "ry": 20, "z_index": 7,
                                "fill": {"type": "linear", "stops": [[0, "#2A1B2E"], [1, "#3A2440"]], "angle": 90},
                                "stroke": "#F778BA", "stroke_width": 2,
                                "shadow": {"color": "#000000", "blur": 16, "dx": 0, "dy": 8, "opacity": 0.5},
                                "opacity": 0.0})
fade("upd_panel", 122.0, 123.5)
add("upd_eq", "LaTeX", {"expression": "θ ← θ − η ∇L", "x": 960, "y": 915, "align": "center",
                        "font_id": "bold", "font_size": 56, "color": "#F5D0FE", "z_index": 8,
                        "draw_fraction": 0.0, "opacity": 1.0})
kf(123.5, "upd_eq", {"draw_fraction": 0.0}, "linear")
kf(127.0, "upd_eq", {"draw_fraction": 1.0}, "linear")

# ── Outro (136–150s) ─────────────────────────────────────────────────────────
add("outro", "Text", {"content": "Built with Lumina", "x": 960, "y": 560, "align": "center",
                       "font_id": "bold", "font_size": 72, "color": "#F0F6FC", "z_index": 12, "opacity": 0.0})
add("outro_sub", "Text", {"content": "declarative JSON  →  video", "x": 960, "y": 640, "align": "center",
                           "letter_spacing": 3, "font_id": "sans", "font_size": 34, "color": "#8B98A9",
                           "z_index": 12, "opacity": 0.0})
add("outro_logo", "SVG", {"asset_id": "node", "x": 860, "y": 250, "width": 200, "height": 200, "z_index": 12, "opacity": 0.0})
add("outro_spark", "Particles", {"count": 160, "emitter_x": 960, "emitter_y": 540, "lifetime": 2.5, "speed": 130,
                                  "spread": 360, "size": 3, "color": "#F0A202", "z_index": 11, "opacity": 0.0})
fade("outro_logo", 138.0, 140.5)
kf(138.0, "outro_logo", {"rotation": -30}, "linear")
kf(141.0, "outro_logo", {"rotation": 0}, "ease_out_elastic")
fade("outro", 139.0, 141.0)
fade("outro_sub", 140.0, 142.0)
kf(138.0, "outro_spark", {"opacity": 0.0}, "linear")
kf(139.5, "outro_spark", {"opacity": 1.0}, "ease_out_cubic")
kf(149.0, "outro_spark", {"opacity": 0.6}, "linear")

# ── Camera choreography ──────────────────────────────────────────────────────
# Pan offsets are pixel-space (scale them); zoom is a ratio (leave it).
_cam_raw = [
    {"time": 0.0, "state": {"x": 0, "y": 0, "zoom": 1.0}, "easing": "linear"},
    {"time": 10.0, "state": {"x": 0, "y": 0, "zoom": 1.0}, "easing": "ease_in_out_cubic"},
    {"time": 24.0, "state": {"x": 0, "y": 20, "zoom": 1.08}, "easing": "ease_in_out_cubic"},
    {"time": 46.0, "state": {"x": 0, "y": 0, "zoom": 1.0}, "easing": "ease_in_out_cubic"},
    {"time": 88.0, "state": {"x": -120, "y": -40, "zoom": 1.12}, "easing": "ease_in_out_cubic"},
    {"time": 112.0, "state": {"x": 0, "y": 0, "zoom": 1.0}, "easing": "ease_in_out_cubic"},
    {"time": 138.0, "state": {"x": 0, "y": 0, "zoom": 1.0}, "easing": "ease_in_out_cubic"},
    {"time": 144.0, "state": {"x": 0, "y": 0, "zoom": 1.06}, "easing": "ease_in_out_cubic"},
]
for _c in _cam_raw:
    _c["state"]["x"] *= SCALE
    _c["state"]["y"] *= SCALE
camera = {"timeline": _cam_raw}

scene = {
    "version": "1.0",
    "meta": {"title": "How a Neural Network Learns", "author": "lumina-showcase", "created_at": "2026-05-25"},
    "canvas": {"width": round(W * SCALE), "height": round(H * SCALE), "fps": FPS, "duration": DURATION, "background": "#0A0E16"},
    "assets": {
        "fonts": [{"id": "sans", "path": FONT_REG}, {"id": "bold", "path": FONT_BOLD}],
        "images": [{"id": "node", "path": LOGO}],
    },
    "objects": objects,
    "timeline": sorted(timeline, key=lambda e: e["time"]),
    "events": [],
    "camera": camera,
}

out_path = os.path.join(os.path.dirname(__file__), "showcase_neural_network.lsf")
with open(out_path, "w") as f:
    json.dump(scene, f, indent=2)

print(f"Wrote {out_path}")
print(f"  objects: {len(objects)}  keyframes: {len(timeline)}  duration: {DURATION}s @ {FPS}fps")

# Optional: validate if the lumina module is importable.
try:
    import lumina
    report = lumina.validate(scene)
    print(f"  validation: valid={report['valid']} errors={len(report['errors'])} warnings={len(report['warnings'])}")
    for e in report["errors"][:5]:
        print("   ", e["code"], "-", e["message"])
except ImportError:
    print("  (install the `lumina` module to validate: cd sdks/python && maturin develop)")
