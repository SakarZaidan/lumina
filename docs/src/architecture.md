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
│   ├── lumina-export/    PNG sequence + MP4/WebM/GIF (FFmpeg stdin pipe)
│   ├── lumina-server/    Axum HTTP: /render /validate /patch /scene_patch /schema /objects
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

## Backend parity status

Skia (CPU) is the reference backend with full feature coverage. Vello
(GPU/wgpu, headless) reached object-type parity in v0.3.0 — text, LaTeX,
MathML, images, SVG and particles render on the GPU via a shared
rasterization module, so those are pixel-identical across backends. The
remaining gaps are tracked for v0.4:

| Feature | Skia (CPU) | Vello (GPU) |
|---|---|---|
| All 17 object types | ✅ | ✅ |
| Text / LaTeX / MathML / Image / SVG / Particles | ✅ | ✅ (shared rasterizer) |
| Linear & radial gradients | ✅ | ❌ solid fallback |
| Drop shadows / glow | ✅ | ❌ not drawn |
| Rounded rectangles (`rx`/`ry`) | ✅ | ❌ square corners |
| Dashed lines (`dash`) | ✅ | ❌ solid stroke |

A scene that uses the features in the lower rows will render differently on
`--backend vello`. The v0.4 workstream closes these gaps behind a
cross-backend pixel-diff test suite
(see `planning/WORKSTREAMS/ws-02-backend-parity.md`).

## Key design choices

- **Declarative first** — scenes are data; nothing for an LLM to mis-sequence.
- **State, not types, drives rendering** — the timeline serializes each object's
  properties to JSON and rebuilds a per-frame `state` map; new `#[serde(default)]`
  fields flow to the renderer with no core changes.
- **Deterministic** — identical inputs yield identical pixels (including particles).
