"""
Generate a Lumina animation with Claude, validate it, and render it to MP4.

This shows the round-trip the engine is designed for: an LLM writes the
declarative scene JSON, Lumina validates it (returning structured, fixable
errors), and renders deterministically.

Prerequisites:
    pip install anthropic
    maturin develop            # from sdks/python/, builds the `lumina` module
    export ANTHROPIC_API_KEY=...
"""

import json
import os

import lumina  # the maturin-built extension module

try:
    import anthropic
except ImportError:
    anthropic = None


SYSTEM_PROMPT = """You generate Lumina Scene Format (LSF) JSON animations.
Rules:
- Objects go in an "objects" map; each has a "type" and "properties".
- Timeline entries have: time (float seconds), object (id), state (object), easing (string).
- Colors are hex strings; fills may also be a gradient: {"type":"linear","stops":[[0,"#.."],[1,"#.."]],"angle":0}.
- Supported types: Circle, Rectangle, Polygon, Path, Line, Arrow, Text, LaTeX, MathML,
  Image, SVG, Group, NumberLine, Axes, Plot, BezierCurve, Particles.
Return ONLY valid JSON, no markdown fences."""


def generate_scene(prompt: str) -> dict:
    client = anthropic.Anthropic()
    msg = client.messages.create(
        model="claude-sonnet-4-6",
        max_tokens=4096,
        system=SYSTEM_PROMPT,
        messages=[{"role": "user", "content": prompt}],
    )
    return json.loads(msg.content[0].text)


def main() -> None:
    if anthropic and os.environ.get("ANTHROPIC_API_KEY"):
        scene = generate_scene(
            "Animate a sine wave drawing itself onto coordinate axes over 3 seconds."
        )
    else:
        # Offline fallback: a tiny hand-written scene so the example always runs.
        print("No ANTHROPIC_API_KEY / anthropic package — using a built-in demo scene.")
        scene = {
            "version": "1.0",
            "meta": {"title": "Fade In", "author": "demo", "created_at": "2026-05-25"},
            "canvas": {"width": 640, "height": 360, "fps": 30, "duration": 2.0, "background": "#0F0F1A"},
            "objects": {
                "dot": {
                    "type": "Circle",
                    "properties": {
                        "cx": 320, "cy": 180, "radius": 60,
                        "fill": {"type": "radial", "stops": [[0, "#F78166"], [1, "#3D1A12"]], "radius": 0.9},
                        "opacity": 0.0, "z_index": 1,
                    },
                }
            },
            "timeline": [
                {"time": 0.0, "object": "dot", "state": {"opacity": 0.0, "radius": 10}, "easing": "linear"},
                {"time": 1.5, "object": "dot", "state": {"opacity": 1.0, "radius": 90}, "easing": "ease_out_elastic"},
            ],
        }

    report = lumina.validate(scene)
    if not report["valid"]:
        print("Scene invalid:")
        for err in report["errors"]:
            print(f"  [{err['code']}] {err['message']}  → {err['fix_suggestion']}")
        return

    lumina.render(scene, "out.mp4", format="mp4")
    print("Rendered out.mp4")


if __name__ == "__main__":
    main()
