# Lumina — Python SDK

Python bindings for the [Lumina](../../README.md) animation engine. Write a
declarative scene as a Python `dict`, validate it, and render to MP4 or a PNG
sequence — all in-process, no subprocess shell-out.

## Install (from source)

```bash
pip install maturin
cd sdks/python
maturin develop --release      # builds the `lumina` extension into your venv
```

(`ffmpeg` must be on `PATH` for MP4 export.)

## Usage

```python
import luminafx

scene = {
    "version": "1.0",
    "meta": {"title": "Hello", "author": "you", "created_at": "2026-05-25"},
    "canvas": {"width": 1280, "height": 720, "fps": 60, "duration": 2.0, "background": "#0F0F1A"},
    "objects": {
        "dot": {"type": "Circle", "properties": {"cx": 640, "cy": 360, "radius": 80, "fill": "#F78166"}}
    },
    "timeline": [
        {"time": 0.0, "object": "dot", "state": {"opacity": 0.0}, "easing": "linear"},
        {"time": 1.5, "object": "dot", "state": {"opacity": 1.0}, "easing": "ease_out_cubic"},
    ],
}

report = luminafx.validate(scene)      # {"valid": True, "errors": [], "warnings": [...]}
assert report["valid"], report["errors"]

luminafx.render(scene, "hello.mp4", format="mp4")   # or format="png" → frame dir
schema = luminafx.schema()             # the LSF JSON Schema as a dict
```

See [`examples/from_anthropic.py`](examples/from_anthropic.py) for the
LLM → validate → render round-trip.

## API

| Function | Description |
|---|---|
| `luminafx.validate(scene: dict) -> dict` | Semantic validation; returns `{valid, errors, warnings}` with `fix_suggestion` strings. |
| `luminafx.render(scene: dict, output_path: str, format="mp4")` | Render to MP4 or a PNG sequence directory. |
| `luminafx.schema() -> dict` | The LSF JSON Schema, for IDE/agent autocompletion and pre-validation. |
