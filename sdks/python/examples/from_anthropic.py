"""
Generate a Lumina animation with Claude, validate it, and render it to MP4.

This shows the round-trip the engine is designed for: an LLM writes the
declarative scene JSON, Lumina validates it (returning structured, fixable
errors), and renders deterministically.

Prerequisites:
    pip install anthropic
    maturin develop            # from sdks/python/, builds the `luminafx` module
    export ANTHROPIC_API_KEY=...
"""

import json
import os

import luminafx  # the maturin-built extension module

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
    msg = client.beta.messages.create(
        model="claude-opus-5",
        max_tokens=16000,
        system=SYSTEM_PROMPT,
        # A declined request is re-run server-side on the recommended fallback
        # model instead of coming back as a refusal.
        betas=["server-side-fallback-2026-07-01"],
        fallbacks="default",
        messages=[{"role": "user", "content": prompt}],
    )
    if msg.stop_reason in ("refusal", "max_tokens"):
        raise RuntimeError(f"no scene: stop_reason={msg.stop_reason}")
    # The reply can open with a thinking block, so read the text blocks rather
    # than assuming the first block is text.
    return json.loads("".join(b.text for b in msg.content if b.type == "text"))


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

    report = luminafx.validate(scene)
    if not report["valid"]:
        # Misspelled names with a single near match are repaired without asking
        # the model again; only what is left needs another round trip.
        fixed = luminafx.fix(scene)
        for applied in fixed["applied"]:
            print(f"  fixed [{applied['code']}] {applied['path']}: {applied['fix_suggestion']}")
        scene, report = fixed["scene"], fixed["remaining"]
    if not report["valid"]:
        print("Scene invalid:")
        for err in report["errors"]:
            print(f"  [{err['code']}] {err['message']}  → {err['fix_suggestion']}")
        return

    luminafx.render(scene, "out.mp4", format="mp4")
    print("Rendered out.mp4")


if __name__ == "__main__":
    main()
