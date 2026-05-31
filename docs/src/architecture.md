# Architecture

Lumina is a modular Rust workspace. Each crate has one job, and the renderer is
backend-agnostic behind a single trait.

```
lumina/
├── crates/
│   ├── lumina-schema/    LSF types + JSON Schema generation (schemars)
│   ├── lumina-core/      Scene graph, timeline evaluator, easings, LAB interp, event bus
│   ├── lumina-renderer/  Renderer trait → SkiaRenderer (CPU) + VelloRenderer (GPU)
│   ├── lumina-text/      Fontdue TTF rasterization + per-character font fallback
│   ├── lumina-export/    PNG sequence + MP4 (FFmpeg stdin pipe)
│   ├── lumina-server/    Axum HTTP: /render /validate /patch /schema /objects
│   ├── lumina-wasm/      wasm-bindgen: render_frame, hit_test, process_event
│   └── lumina-bench/     Criterion benchmarks
├── sdks/{javascript,python}/
└── tools/lumina-cli/
```

## Data flow

```
LSF JSON
  → Schema validator (structured errors with fix_suggestion)
  → Scene graph + timeline (keyframe evaluation, easing, LAB color)
  → Renderer (Skia CPU  | Vello GPU)
  → Export (PNG sequence | FFmpeg → MP4)   or   WASM canvas
```

## The Renderer trait

A backend implements `render_frame`, `load_font`, and optionally `load_image` /
`set_time`. The `Exporter<R: Renderer>` and the WASM engine are generic over it,
so adding a backend never touches the scene/timeline code. `set_time` is what
lets time-dependent assets (animated GIFs) and the particle simulator pick the
right state per frame.

## Key design choices

- **Declarative first** — scenes are data; nothing for an LLM to mis-sequence.
- **State, not types, drives rendering** — the timeline serializes each object's
  properties to JSON and rebuilds a per-frame `state` map; new `#[serde(default)]`
  fields flow to the renderer with no core changes.
- **Deterministic** — identical inputs yield identical pixels (including particles).
