# Lumina

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-44%20passing-brightgreen)](#tests)
[![Rust](https://img.shields.io/badge/rust-1.78%2B-orange)](https://rustup.rs)

**Lumina** is a production-grade, AI-native animation engine built in Rust. Declarative by design, GPU-native by architecture, runnable everywhere humans and machines need motion.

---

## Demo

### Pythagorean Theorem — generated from 60 lines of JSON

![Pythagorean theorem animation](media/pythagorean.gif)

### Bounce Easing — `ease_out_bounce` vs `ease_in_quad`

![Circle bounce easing demo](media/circle_bounce.gif)

Rendered with:

```bash
./target/release/lumina-cli --scene examples/pythagorean.lsf --output media/pythagorean.mp4 --format mp4
./target/release/lumina-cli --scene examples/circle_bounce.lsf --output media/circle_bounce.mp4  --format mp4
```

No After Effects. No Python. No GUI. Just JSON in, video out.

---

## What It Solves

Every existing animation library has a fundamental mismatch with AI-driven development:

| Problem                                                              | Who Suffers                   |
| -------------------------------------------------------------------- | ----------------------------- |
| Imperative APIs require stateful reasoning that LLMs hallucinate     | AI agents building animations |
| CPU-bound rendering can't hit 60fps with complex math scenes         | Real-time web/app developers  |
| No unified format runs both offline (video) and online (interactive) | Educators, SaaS builders      |
| LaTeX/math rendering is bolted on as an afterthought                 | Math/science content creators |
| No schema means no validation means broken outputs at runtime        | Everyone                      |

Lumina solves all of these with a single coherent architecture.

---

## Key Features

- **Declarative LSF Format** — Pure-data JSON (Lumina Scene Format). No functions, no loops, no state. An LLM can write it correctly.
- **Dual Rendering Backends** — GPU (Vello/wgpu) for real-time, CPU (Tiny-Skia) for headless video export.
- **27 Easing Functions** — Including spring physics, elastic, bounce, Manim-compatible `smooth`/`rush_into`/`there_and_back`.
- **First-Class LaTeX** — Native MiTeX parsing; no Node.js or KaTeX sidecar required.
- **AI Validation** — Schema-validated LSF with machine-readable structured errors and fix suggestions.
- **Cross-Platform** — Native binaries for video export, WASM for the browser.
- **16 Object Types** — Circle, Rectangle, Polygon, Path, Line, Arrow, Text, LaTeX, Group, Image, SVG, NumberLine, Axes, Plot, BezierCurve, and more.

---

## Performance

| Scenario                     | Expectation                         | Notes                    |
| ---------------------------- | ----------------------------------- | ------------------------ |
| 500 objects, real-time (GPU) | 45–60 fps                           | Mid-range GPU            |
| 2,000 objects, real-time     | Batching recommended                | Use Groups               |
| 30s @ 1080p60, video export  | 10–30 seconds render + 2–15s FFmpeg | Measured on T4-class GPU |
| Headless CPU render (CI/CD)  | 45–120 seconds for 30s @ 1080p      | Tiny-Skia backend        |

---

## Getting Started

### Prerequisites

- **Rust** — latest stable via [rustup](https://rustup.rs)
- **FFmpeg** — required for MP4/WebM export (`apt install ffmpeg` / `brew install ffmpeg`)

### Build

```bash
git clone https://github.com/sakar/lumina.git
cd lumina
cargo build --release
```

### Render Your First Scene

```bash
# PNG frame sequence
./target/release/lumina-cli --scene examples/hello.lsf --output frames/ --format png

# MP4 video
./target/release/lumina-cli --scene examples/pythagorean.lsf --output proof.mp4 --format mp4
```

### Write a Scene

```json
{
  "version": "1.0",
  "meta": {
    "title": "Fade In",
    "author": "you",
    "created_at": "2026-04-30T00:00:00Z"
  },
  "canvas": {
    "width": 1280,
    "height": 720,
    "fps": 60,
    "duration": 2.0,
    "background": "#0F0F1A"
  },
  "objects": {
    "title": {
      "type": "Text",
      "properties": {
        "content": "Hello",
        "x": 540,
        "y": 360,
        "font_size": 96,
        "color": "#FFFFFF",
        "opacity": 0.0
      }
    }
  },
  "timeline": [
    {
      "time": 0.0,
      "object": "title",
      "state": { "opacity": 0.0 },
      "easing": "linear"
    },
    {
      "time": 1.5,
      "object": "title",
      "state": { "opacity": 1.0 },
      "easing": "ease_out_cubic"
    }
  ],
  "events": []
}
```

---

## AI Integration

Lumina is designed to be written by LLMs. The workflow:

```python
import anthropic, lumina_cloud

client = anthropic.Anthropic()
lumina = lumina_cloud.Client(api_key="...")

# 1. Generate LSF with Claude
schema = lumina.get_schema()
msg = client.messages.create(
    model="claude-sonnet-4-6",
    max_tokens=4096,
    system=f"You generate Lumina Scene Format JSON. Schema: {schema}. Return ONLY valid JSON.",
    messages=[{"role": "user", "content": "Animate the dot product of two vectors, 10 seconds."}]
)

# 2. Validate (self-correction loop)
validation = lumina.validate(msg.content[0].text)
# validation.errors contain fix_suggestion strings ready to re-feed to the model

# 3. Render
video = lumina.render(msg.content[0].text, format="mp4", resolution="1080p")
```

### Validation Response

When your LSF has errors, the engine returns structured, AI-re-injectable error messages:

```json
{
  "valid": false,
  "errors": [
    {
      "code": "UNKNOWN_OBJECT_ID",
      "path": "$.timeline[3].object",
      "message": "Timeline entry at index 3 references object 'circle_2', but no such object exists.",
      "fix_suggestion": "Did you mean 'circle_1'? Add 'circle_2' to the 'objects' block, or change the reference."
    }
  ],
  "warnings": []
}
```

---

## Example Scenes

| Scene                                                      | Duration | Objects                      | Demonstrates                        |
| ---------------------------------------------------------- | -------- | ---------------------------- | ----------------------------------- |
| [`examples/hello.lsf`](examples/hello.lsf)                 | 3s       | Text, Line                   | Fade-in, `ease_out_sine`            |
| [`examples/pythagorean.lsf`](examples/pythagorean.lsf)     | 8s       | Polygon, Arrow, LaTeX, Group | Spring scale, label timing          |
| [`examples/circle_bounce.lsf`](examples/circle_bounce.lsf) | 4s       | Circle, Line                 | `ease_out_bounce` vs `ease_in_quad` |

---

## Easing Functions (27)

```
linear
ease_in/out/in_out_quad       ease_in/out/in_out_cubic
ease_in/out/in_out_quart      ease_in/out/in_out_sine
ease_in/out_expo              ease_in/out_circ
ease_in/out/in_out_elastic    ease_in/out_bounce
spring                        smooth (Manim)
rush_into  rush_from          there_and_back
ease  ease_in  ease_out  ease_in_out  (CSS aliases)
```

All implement the contract `f(0.0) == 0.0`, `f(1.0) == 1.0` — verified by the test suite.

---

## Tests

**44 tests, all passing.**

```
lumina-core     25 tests   easing boundaries, timeline interpolation, stress (2000 objects)
lumina-renderer  9 tests   pixel-level correctness, z-index ordering, opacity, background color
lumina-export    4 tests   PNG sequence creation, dimensions, brightness, FFmpeg graceful fail
lumina-server    6 tests   semantic validation: unknown IDs, circular groups, duplicate keyframes
```

Run them:

```bash
cargo test --workspace --exclude lumina-wasm
```

Sample output:

```
test easing_tests::tests::test_all_easings_boundary_conditions ... ok
test easing_tests::tests::test_in_out_variants_are_symmetric ... ok
test easing_tests::tests::test_elastic_out_overshoots_then_settles ... ok
test timeline_tests::tests::test_linear_interpolation_at_midpoint ... ok
test timeline_tests::tests::test_ease_in_quad_is_nonlinear_at_midpoint ... ok
test renderer_tests::tests::test_z_index_determines_draw_order ... ok
test renderer_tests::tests::test_circle_center_pixel_matches_fill ... ok
test renderer_tests::tests::test_background_color_applied ... ok
test tests::test_circular_group_reference_detected ... ok
test tests::test_unknown_object_id_in_timeline ... ok

test result: ok. 44 passed; 0 failed
```

---

## Architecture

```
lumina/
├── crates/
│   ├── lumina-schema/     LSF type definitions + JSON Schema generation (schemars)
│   ├── lumina-core/       Scene graph, timeline engine, 27 easing functions, event bus
│   ├── lumina-renderer/   Renderer trait + Skia (CPU) + Vello (GPU, in progress)
│   ├── lumina-text/       Fontdue text layout + MiTeX LaTeX parsing
│   ├── lumina-export/     PNG sequence, MP4 via FFmpeg
│   ├── lumina-server/     Axum headless render server (/render /validate /patch /schema)
│   └── lumina-wasm/       wasm-bindgen browser runtime
├── tools/
│   └── lumina-cli/        lumina render scene.lsf -o output.mp4
├── sdks/
│   └── javascript/        React <LuminaPlayer> component
├── examples/              hello.lsf  pythagorean.lsf  circle_bounce.lsf
└── media/                 hello.mp4  pythagorean.mp4  circle_bounce.mp4 + GIFs
```

### Data Flow

```
LSF JSON
   │
   ▼
Schema Validator ──── structured errors with fix_suggestion
   │
   ▼
Scene Graph + Timeline ──── keyframe evaluation, easing, overrides
   │
   ├─── [Offline] ──► Skia/Vello Frame Renderer ──► FFmpeg ──► MP4/GIF/PNG
   └─── [Browser] ──► WASM + SkiaRenderer ──► Canvas putImageData ──► 60fps
```

---

## Headless Server

```bash
cargo run -p lumina-server

# Validate an LSF file
curl -X POST http://localhost:3000/validate \
  -H "Content-Type: application/json" \
  -d @examples/pythagorean.lsf

# Render to MP4
curl -X POST http://localhost:3000/render \
  -H "Content-Type: application/json" \
  -d '{"scene": {...}, "format": "mp4"}' \
  --output animation.mp4
```

---

## Roadmap

| Phase                  | Status       | Scope                                                                      |
| ---------------------- | ------------ | -------------------------------------------------------------------------- |
| Phase 1 — Rust Core    | **Complete** | LSF schema, Skia renderer, timeline, easing, export, CLI, server           |
| Phase 2 — WASM & Web   | In Progress  | WebGPU (Vello), interactive events, React SDK, `npm install @lumina/react` |
| Phase 3 — AI Cloud API | Planned      | Hosted render endpoint, AI self-correction SDK, Python bindings            |
| Phase 4 — Studio       | Planned      | Browser timeline editor, team collaboration, Tauri desktop app             |

Tracked in [`todo.md`](todo.md).

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). All PRs require `cargo test --workspace` to pass.

## Authors

**sakar hashim** — Lead Developer

## License

MIT — see [LICENSE](LICENSE).
