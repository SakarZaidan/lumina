# PROJECT LUMINA — Complete Engineering Blueprint v2.0
### A Production-Grade, AI-Native, Cross-Platform Animation Engine

---

## TABLE OF CONTENTS

1. [Vision & Positioning](#1-vision--positioning)
2. [Competitive Landscape & How Lumina Wins](#2-competitive-landscape--how-lumina-wins)
3. [Target Users & Monetization Model](#3-target-users--monetization-model)
4. [Tech Stack Decision (with full reasoning)](#4-tech-stack-decision-with-full-reasoning)
5. [Full System Architecture](#5-full-system-architecture)
6. [The Lumina Scene Format (LSF) — Complete Schema Spec](#6-the-lumina-scene-format-lsf--complete-schema-spec)
7. [The AI-Native Design System (Properly Defined)](#7-the-ai-native-design-system-properly-defined)
8. [Renderer Architecture & Honest Performance Claims](#8-renderer-architecture--honest-performance-claims)
9. [Text, LaTeX & Math Rendering Pipeline](#9-text-latex--math-rendering-pipeline)
10. [The Easing & Interpolation Library](#10-the-easing--interpolation-library)
11. [Asset Pipeline (SVG, Images, Fonts)](#11-asset-pipeline-svg-images-fonts)
12. [Export Format Pipeline](#12-export-format-pipeline)
13. [WASM & Browser Runtime (Honest Scope)](#13-wasm--browser-runtime-honest-scope)
14. [Interactive Event System](#14-interactive-event-system)
15. [Diff/Patch Incremental Update Model](#15-diffpatch-incremental-update-model)
16. [AI Headless Rendering Server](#16-ai-headless-rendering-server)
17. [Error Model & Validation System](#17-error-model--validation-system)
18. [Phase-by-Phase Roadmap](#18-phase-by-phase-roadmap)
19. [MVP Definition — What Ships First](#19-mvp-definition--what-ships-first)

---

## 1. VISION & POSITIONING

### The One-Line Pitch
**Lumina is the animation engine for the AI era: declarative by design, GPU-native by architecture, and runnable everywhere humans and machines need motion.**

### What Problem It Actually Solves

Every existing animation library has a fundamental mismatch with how software is built in 2025+:

| Problem | Who Suffers |
|---|---|
| Imperative APIs require stateful reasoning that LLMs hallucinate | AI agents building animations |
| CPU-bound rendering can't hit 60fps with complex math scenes | Real-time web/app developers |
| No unified format runs both offline (video) and online (interactive) | Educators, SaaS builders |
| LaTeX/math rendering is bolted on as an afterthought | Math/science content creators |
| No event system means animations are passive, not interactive | Web developers |
| No schema means no validation means broken outputs at runtime | Everyone |

Lumina solves all six simultaneously with a single coherent architecture.

### What It Is NOT
- Not a replacement for CSS animations (wrong layer)
- Not a video editor (wrong audience)
- Not a general-purpose game engine (wrong scope)
- Not another Python-only tool for math professors

---

## 2. COMPETITIVE LANDSCAPE & HOW LUMINA WINS

### Honest Competitive Analysis

| Library | Strengths | Fatal Weaknesses | Lumina's Answer |
|---|---|---|---|
| **Manim** | Math-accurate, beloved by educators | CPU-only, imperative Python, no real-time, no web | GPU renderer + declarative API |
| **Lottie/Airbnb** | Lightweight, plays anywhere | Adobe After Effects dependency, no programmatic API, no math | Code-first, no design tool needed |
| **Rive** | Stunning real-time, state machine | Proprietary editor, closed format, no AI/programmatic access | Open JSON format, pure code |
| **Theatre.js** | Beautiful timeline editor | JS-only, no rendering, just a choreography layer | Full stack: schema + engine + render |
| **Motion Canvas** | Great DX, TypeScript-native | TypeScript imperative, no AI compatibility, no WASM | Declarative JSON solves this |
| **Three.js / Babylon** | 3D powerhouses | 3D-first, 2D math scenes are painful, no AI-native | 2D-first, 3D as optional layer |
| **GSAP** | Battle-tested, huge community | JS-only, imperative, no video export, no AI usage | Video export + AI-native JSON |

### Where Lumina Has No Competition
The combination of:
1. **Declarative open format** (AI can write it)
2. **GPU-native 2D rendering** (not bolted-on)
3. **Same scene renders to video AND runs in browser**
4. **First-class LaTeX/math support**
5. **Interactive event system baked in**
6. **Headless AI server mode**

No single library does all six. This is the moat.

---

## 3. TARGET USERS & MONETIZATION MODEL

### Primary Target User (MVP Focus)
**AI-agent developers and backend engineers** who need to programmatically generate educational or explainer animations — either as video exports or embedded interactive components — without touching a GUI tool.

Example: A startup building an AI math tutor that generates custom video explanations per student.

### Secondary Target Users (Phase 2+)
- **Frontend/React developers** embedding interactive math visualizations in web apps
- **Data scientists** building animated data story presentations
- **Math/science educators** replacing Manim with a faster, web-deployable tool

### Monetization Strategy

**Tier 1 — Open Source Core (Free Forever)**
Everything in this blueprint up to Phase 2. The Rust core, Python bindings, CLI, and offline video export. MIT licensed. This builds community and trust.

**Tier 2 — Lumina Cloud API ($49–$299/mo)**
- Hosted headless rendering endpoint
- AI agent integration (accept prompts, return video/component)
- SLA-backed, autoscaling
- No infrastructure management for the user

**Tier 3 — Lumina Studio (one-time or subscription)**
- Visual timeline editor (built on the open format)
- Team collaboration
- Brand kit / template library

**Why this works:** The open-source core is the acquisition funnel. The cloud API is the revenue engine. The studio is the enterprise upsell.

---

## 4. TECH STACK DECISION (WITH FULL REASONING)

### Decision: Rust (not C++)

This is not a stylistic preference. Here is the engineering reasoning:

| Factor | C++ | Rust | Winner |
|---|---|---|---|
| Memory safety (no GC pauses at frame boundaries) | Manual, error-prone | Compiler-enforced | **Rust** |
| Python bindings | pybind11 (mature but verbose) | PyO3 + Maturin (modern, ergonomic) | **Rust** |
| WASM compilation | Emscripten (heavy, complex) | wasm-pack (first-class support) | **Rust** |
| Build system | CMake (painful) | Cargo (excellent) | **Rust** |
| Concurrency safety | Threading bugs are silent | Compiler prevents data races | **Rust** |
| GPU library ecosystem | bgfx, SFML (fragmented) | Vello/wgpu (modern, unified) | **Rust** |
| Hiring/community 2025+ | Declining interest | Growing rapidly | **Rust** |

**C++ is only better if you have an existing C++ codebase to integrate with. You don't.**

### The Full Stack

```
┌─────────────────────────────────────────────────────┐
│                    USER INTERFACES                    │
│  Python (pyo3)  │  JS/TS (wasm)  │  CLI (clap)      │
├─────────────────────────────────────────────────────┤
│                  LUMINA CORE (Rust)                   │
│  Scene Graph  │  Timeline Engine  │  Interpolator    │
│  Diff Engine  │  Event System     │  Asset Manager   │
├─────────────────────────────────────────────────────┤
│                    RENDERER                           │
│  Vello (GPU)  │  Tiny-Skia (CPU fallback)            │
│  wgpu backend │  WebGPU (browser) │ Metal/Vulkan/DX  │
├─────────────────────────────────────────────────────┤
│                  EXPORT PIPELINE                      │
│  FFmpeg (MP4/WebM/GIF) │ PNG seq │ Interactive HTML  │
├─────────────────────────────────────────────────────┤
│                 TEXT / MATH LAYER                     │
│  Fontdue (text) │ MiTeX (LaTeX→paths) │ resvg (SVG)  │
└─────────────────────────────────────────────────────┘
```

### Key Library Choices Explained

**Vello** — GPU-accelerated 2D vector renderer by Google's Linebender team. Written in Rust. Uses WebGPU (wgpu). Renders paths, gradients, and text on GPU. Works on desktop (Vulkan/Metal/DirectX) and browser (WebGPU). This is the renderer Chrome's next-gen canvas uses. It is the correct choice.

**Tiny-Skia** — Pure-Rust CPU fallback for environments without GPU (headless servers, CI). Same API as Vello, different backend. This ensures the headless render server works without a GPU requirement.

**wgpu** — The WebGPU implementation in Rust. Powers Vello. Also targets native backends. This is what gives us "write once, render everywhere."

**Fontdue** — Pure-Rust font rasterizer. Handles TTF/OTF. No C dependency. Works in WASM.

**MiTeX** — Rust-native LaTeX math parser. Converts LaTeX strings to glyph paths. No Node.js, no server-side KaTeX call needed.

**PyO3 + Maturin** — The standard for Rust→Python bindings. Used by Polars, Pydantic v2, and Ruff. Battle-tested.

**wasm-pack** — Rust→WASM+JS glue. Used by Cloudflare Workers, Figma's plugin runtime.

**FFmpeg (via subprocess/binding)** — For video encoding. No custom video encoder. FFmpeg is the industry standard. Encoding to MP4 takes 2–15 seconds for a 30-second clip. Not milliseconds. This is honest.

---

## 5. FULL SYSTEM ARCHITECTURE

### Component Map

```
lumina/
├── crates/
│   ├── lumina-core/          # Scene graph, timeline, interpolation
│   │   ├── scene/            # SceneGraph, ObjectRegistry, Transform
│   │   ├── timeline/         # KeyframeTrack, Interpolator, Easing
│   │   ├── objects/          # Circle, Rect, Path, Text, Arrow, Group
│   │   ├── events/           # EventBus, HoverEvent, ClickEvent
│   │   └── diff/             # ScenePatch, DiffEngine
│   │
│   ├── lumina-renderer/      # Rendering backends
│   │   ├── vello_backend/    # GPU rendering via Vello/wgpu
│   │   ├── skia_backend/     # CPU fallback via tiny-skia
│   │   └── traits.rs         # Renderer trait (swap backends cleanly)
│   │
│   ├── lumina-text/          # Text & math rendering
│   │   ├── fontdue_text/     # Standard text layout
│   │   ├── mitex_math/       # LaTeX → glyph paths
│   │   └── svg_text/         # SVG text element support
│   │
│   ├── lumina-export/        # Export pipeline
│   │   ├── mp4/              # FFmpeg MP4/WebM
│   │   ├── gif/              # GIF encoder
│   │   ├── png_seq/          # PNG frame sequence
│   │   └── html_bundle/      # Interactive HTML export
│   │
│   ├── lumina-schema/        # LSF schema definition + validation
│   │   ├── types.rs          # All LSF types as Rust structs
│   │   ├── validation.rs     # Schema validator
│   │   └── json_schema/      # JSON Schema files for AI consumption
│   │
│   ├── lumina-py/            # PyO3 Python bindings
│   ├── lumina-wasm/          # wasm-pack WASM/JS bindings
│   └── lumina-server/        # Headless AI render server (Axum)
│
├── tools/
│   ├── lumina-cli/           # CLI: lumina render scene.lsf -o video.mp4
│   └── lumina-validator/     # Standalone LSF file validator
│
└── sdks/
    ├── python/               # lumina-py wheel + Python helpers
    └── javascript/           # lumina-js npm package + React component
```

### Data Flow: Scene Definition → Output

```
[LSF JSON / Python API / JS API]
         │
         ▼
   Schema Validator ──── returns structured errors if invalid
         │
         ▼
   SceneGraph Builder ── constructs object tree from LSF
         │
         ▼
   Timeline Compiler ─── resolves keyframes, bakes easing curves
         │
         ▼
   DiffEngine (optional) tracks incremental changes from previous state
         │
         ├──── [Offline Mode] ──► FrameRenderer ──► FFmpeg ──► MP4/GIF/WebM
         │
         └──── [Runtime Mode] ──► EventLoop ──► wgpu/WebGPU ──► Screen/Canvas
```

---

## 6. THE LUMINA SCENE FORMAT (LSF) — COMPLETE SCHEMA SPEC

This is the section the original blueprint completely skipped. The LSF is the most critical design decision in the entire project. Get this wrong and everything breaks.

### Design Principles
1. **Declarative only** — No functions, no loops, no conditionals. Pure data.
2. **Self-describing** — Every object declares its type and all valid properties upfront.
3. **Conflict-free** — Rules for what happens when two keyframes collide are explicit.
4. **Versionable** — A `version` field ensures forward/backward compatibility.
5. **Validatable** — A published JSON Schema (draft-07) lets AI validate before submitting.

### Full LSF Example

```json
{
  "version": "1.0",
  "meta": {
    "title": "Pythagorean Theorem Proof",
    "author": "lumina-ai-agent",
    "created_at": "2025-06-01T12:00:00Z"
  },
  "canvas": {
    "width": 1920,
    "height": 1080,
    "fps": 60,
    "duration": 12.0,
    "background": "#0F0F1A"
  },
  "assets": {
    "fonts": [
      { "id": "math_font", "path": "./fonts/STIX2Math.otf" }
    ],
    "images": [
      { "id": "diagram_bg", "path": "./assets/grid.svg" }
    ]
  },
  "objects": {
    "triangle": {
      "type": "Polygon",
      "z_index": 1,
      "properties": {
        "points": [[0, 0], [300, 0], [0, 400]],
        "fill": "#1E3A5F",
        "stroke": "#4A90D9",
        "stroke_width": 2,
        "opacity": 0
      }
    },
    "hypotenuse_label": {
      "type": "LaTeX",
      "z_index": 2,
      "properties": {
        "expression": "c^2 = a^2 + b^2",
        "font_size": 48,
        "color": "#FFFFFF",
        "x": 960,
        "y": 200,
        "opacity": 0
      }
    },
    "side_a": {
      "type": "Arrow",
      "z_index": 2,
      "properties": {
        "from": [0, 0],
        "to": [300, 0],
        "color": "#E74C3C",
        "stroke_width": 3,
        "label": "a",
        "opacity": 0
      }
    },
    "proof_group": {
      "type": "Group",
      "z_index": 3,
      "children": ["triangle", "side_a"],
      "properties": {
        "x": 600,
        "y": 300,
        "scale": 1.0,
        "rotation": 0
      }
    }
  },
  "timeline": [
    {
      "time": 0.0,
      "object": "proof_group",
      "state": { "scale": 0.0, "opacity": 0 },
      "easing": "linear"
    },
    {
      "time": 1.0,
      "object": "proof_group",
      "state": { "scale": 1.0, "opacity": 1 },
      "easing": "spring",
      "easing_params": { "stiffness": 200, "damping": 20 }
    },
    {
      "time": 2.5,
      "object": "hypotenuse_label",
      "state": { "opacity": 1 },
      "easing": "ease_out_cubic"
    },
    {
      "time": 4.0,
      "object": "hypotenuse_label",
      "state": { "opacity": 1, "scale": 1.2 },
      "easing": "ease_in_out_sine"
    }
  ],
  "events": [
    {
      "object": "triangle",
      "trigger": "click",
      "action": {
        "type": "jump_to_time",
        "value": 0.0
      }
    },
    {
      "object": "side_a",
      "trigger": "hover_enter",
      "action": {
        "type": "set_property",
        "target": "side_a",
        "property": "color",
        "value": "#F39C12"
      }
    },
    {
      "object": "side_a",
      "trigger": "hover_exit",
      "action": {
        "type": "set_property",
        "target": "side_a",
        "property": "color",
        "value": "#E74C3C"
      }
    }
  ],
  "camera": {
    "timeline": [
      { "time": 0.0, "state": { "x": 0, "y": 0, "zoom": 1.0 } },
      { "time": 5.0, "state": { "x": -200, "y": 0, "zoom": 1.3 }, "easing": "ease_in_out_quad" }
    ]
  }
}
```

### Object Type Registry (Complete)

| Type | Required Properties | Optional Properties |
|---|---|---|
| `Circle` | `cx, cy, radius` | `fill, stroke, stroke_width, opacity, x, y` |
| `Rectangle` | `x, y, width, height` | `fill, stroke, rx, ry, opacity` |
| `Polygon` | `points: [[x,y],...]` | `fill, stroke, stroke_width, opacity` |
| `Path` | `d: "SVG path string"` | `fill, stroke, stroke_width, opacity` |
| `Line` | `x1, y1, x2, y2` | `stroke, stroke_width, opacity` |
| `Arrow` | `from: [x,y], to: [x,y]` | `color, stroke_width, label, tip_size, opacity` |
| `Text` | `content, x, y` | `font_id, font_size, color, align, opacity` |
| `LaTeX` | `expression, x, y` | `font_size, color, opacity, scale` |
| `MathML` | `markup, x, y` | `font_size, color, opacity` |
| `Image` | `asset_id, x, y` | `width, height, opacity` |
| `SVG` | `asset_id, x, y` | `width, height, opacity` |
| `Group` | `children: [ids]` | `x, y, scale, rotation, opacity` |
| `NumberLine` | `start, end, step, x, y` | `length, tick_labels, color, opacity` |
| `Axes` | `x_range, y_range` | `x_label, y_label, grid, color` |
| `Plot` | `function: "x^2+1", axes_id` | `color, stroke_width, sample_count` |
| `BezierCurve` | `p0, p1, p2, p3` | `stroke, stroke_width, opacity` |
| `Particles` | `count, emitter_x, emitter_y` | `lifetime, speed, spread, color` |

### Conflict Resolution Rules (Explicit)

**Rule 1 — Same time, same object, same property:** Last declaration in the array wins. Validation emits a WARNING (not error).

**Rule 2 — Overlapping interpolation windows:** If object A has keyframes at t=1.0 and t=3.0, and a second keyframe also targets t=2.0 (midpoint), the t=2.0 keyframe acts as a "hard stop" — it snaps to that value and restarts interpolation toward t=3.0.

**Rule 3 — Child vs parent transforms:** A child's `x,y` is always relative to its parent Group's transform. If both parent and child animate simultaneously, the child's final world position is `parent_transform * child_transform`. This is documented and the AI must be told this explicitly.

**Rule 4 — Missing property at t=0:** If a property appears in the timeline but not in the object's initial `properties` block, the engine uses the type's default value (typically 0 or 1 for opacity). Validation emits a WARNING.

**Rule 5 — `priority` field:** Any keyframe can set `"priority": N` (integer). Higher priority wins conflicts. Default is 0.

---

## 7. THE AI-NATIVE DESIGN SYSTEM (PROPERLY DEFINED)

### What "AI-Native" Actually Means Architecturally

The original blueprint used "AI-Native" as a vibe. Here is what it means in practice, implemented as concrete systems:

### System 1: The Published JSON Schema

A machine-readable `lumina-schema.json` (JSON Schema draft-07) is published with every Lumina release. AI agents use this to:
1. Validate their generated LSF before sending it
2. Autocomplete properties in IDE/agent tooling
3. Understand which properties are animatable vs static

This schema is generated directly from the Rust type definitions (via `schemars` crate), so it is always in sync with the engine.

### System 2: The Object Model Prompt Fragment

When an AI agent is asked to generate a Lumina animation, the orchestrating system injects this into the AI's context:

```
You are generating a Lumina Scene Format (LSF) JSON file.

RULES:
1. All objects must be declared in the "objects" section before referenced in "timeline".
2. Object IDs must be snake_case strings. No spaces.
3. "time" values are in seconds (float). Duration is {DURATION}s.
4. Group children inherit parent transforms. Set group x/y for positioning; children use relative coordinates.
5. LaTeX expressions use standard LaTeX math mode syntax (no $ delimiters).
6. All colors are hex strings (#RRGGBB or #RRGGBBAA).
7. The timeline array must be sorted by "time" ascending.
8. Do not reference asset IDs that aren't declared in "assets".

AVAILABLE OBJECT TYPES AND THEIR REQUIRED PROPERTIES:
{inject lumina-schema-summary.md here}

Respond with ONLY the JSON. No explanation, no markdown fences.
```

This prompt fragment is part of the official Lumina documentation and SDK — not something each user has to write themselves.

### System 3: The AI Validation Endpoint

The headless server exposes a `/validate` endpoint. An AI agent can POST its LSF, receive structured errors with line numbers and fix suggestions, and retry before submitting for rendering. This validation loop is the error recovery mechanism.

### System 4: AI-Optimized Error Messages

Lumina's validator returns errors designed to be re-fed to the AI for self-correction:

```json
{
  "valid": false,
  "errors": [
    {
      "code": "UNKNOWN_OBJECT_ID",
      "timeline_index": 3,
      "message": "Timeline entry at index 3 references object 'circle_2', but no object with this ID exists in the 'objects' block. Did you mean 'circle_1'?",
      "fix_suggestion": "Add 'circle_2' to the 'objects' block, or change the timeline reference to 'circle_1'."
    }
  ]
}
```

The `fix_suggestion` field is literally a string the orchestrator can inject back to the AI to guide correction.

### System 5: Scene Introspection API

```python
scene = lumina.Scene.from_file("scene.lsf")
print(scene.object_ids())        # ['triangle', 'hypotenuse_label', 'side_a']
print(scene.animatable_properties('triangle'))  # ['x', 'y', 'opacity', 'scale', 'rotation', 'fill', ...]
print(scene.duration)            # 12.0
print(scene.conflicts())         # [] or list of ConflictWarning
```

This lets the AI query the state of a scene it previously generated and extend it incrementally.

---

## 8. RENDERER ARCHITECTURE & HONEST PERFORMANCE CLAIMS

### The Two-Backend Strategy

**Backend A: Vello (GPU, wgpu)**
Used for: Real-time playback, interactive browser mode, preview rendering.
- Renders via WebGPU (browser) or Vulkan/Metal/DirectX12 (native)
- GPU path rendering via compute shaders (no CPU path tessellation)
- Expected real-world throughput: **500–2000 complex vector paths at 60fps** on mid-range GPU
- Text rendering: Fontdue rasterizes glyphs to atlas, Vello composites

**Backend B: Tiny-Skia (CPU, pure Rust)**
Used for: Headless video rendering on servers without GPUs, CI/CD, testing.
- Pure Rust, no C dependencies, compiles to WASM
- Expected throughput: **50–200 complex paths at 60fps** on modern CPU
- For video export, frame order doesn't matter (parallelized with Rayon)

### Honest Performance Claims

| Scenario | Realistic Expectation |
|---|---|
| 50 objects, real-time browser | 60fps, GPU backend, mid-range device |
| 500 objects, real-time browser | 45–60fps, GPU backend, modern device |
| 2000 objects, real-time browser | Not recommended. Batch into groups |
| Headless render: 30s clip @1080p60 | 45–120 seconds CPU, 10–30 seconds GPU |
| Headless render: 10s clip @720p30 | 8–25 seconds CPU |
| Video encoding via FFmpeg | 2–15 seconds additional on top of render |

"Milliseconds" was removed from the vocabulary. Lumina is fast; it is not magic.

### The Frame Renderer Loop

```
for each frame f at time t:
    1. Query timeline: compute interpolated state of all objects at t
    2. Traverse scene graph (DFS): apply parent transforms to children
    3. Sort by z_index
    4. Submit draw calls to renderer backend (batched)
    5. Flush frame to surface / frame buffer
    6. [Offline mode] write frame to PNG, collect for FFmpeg
```

Scene graph traversal with 1000 nodes: ~0.1ms on modern CPU (competitive programmer territory, well within budget).

---

## 9. TEXT, LATEX & MATH RENDERING PIPELINE

This was completely absent from the original blueprint. It is one of the hardest problems. Here is the full solution.

### Layer 1: Standard Text

**Library:** Fontdue (pure Rust)
**Pipeline:**
1. Load TTF/OTF font into Fontdue at startup
2. Shape text string into glyph sequence
3. Rasterize each glyph into a glyph atlas texture (cached)
4. Submit textured quads to renderer

**Supported:** Unicode, RTL (via unicode-bidi), font fallback chains.

### Layer 2: LaTeX Math

**Library:** MiTeX (Rust-native LaTeX parser, no JS dependency)
**Pipeline:**
1. Parse LaTeX expression → MathML AST
2. Layout engine positions glyphs using STIX2 or Latin Modern Math font
3. Output: a set of positioned glyph draws + rule (line) draws
4. These are submitted to the renderer as standard vector paths

**What MiTeX supports:** All standard AMS math. Fractions, integrals, sums, matrices, cases, aligned environments.

**What it doesn't (yet):** TikZ diagrams, custom LaTeX packages. These are deferred.

**Fallback pipeline (for complex LaTeX):**
1. Call KaTeX server (if available) → SVG string
2. Parse SVG → Lumina Path objects
3. Render via standard path renderer

This fallback is documented, not hidden. Users who need complex LaTeX run the KaTeX sidecar.

### Layer 3: MathML

Some users prefer MathML over LaTeX. MiTeX accepts both. Same pipeline applies.

### Animating Math Expressions

Because LaTeX expressions compile to a set of **named glyph groups**, individual symbols can be animated:

```json
{
  "type": "LaTeX",
  "id": "formula",
  "expression": "E = mc^2",
  "animate_parts": true
}
```

With `animate_parts: true`, the renderer exposes `formula.E`, `formula.m`, `formula.c`, `formula.exp_2` as individually animatable sub-objects. This is the **Write-On effect** (drawing math symbol by symbol) that Manim is famous for.

---

## 10. THE EASING & INTERPOLATION LIBRARY

The original blueprint mentioned "easing" once. Here is the complete specification.

### Built-in Easing Functions (30+)

**Standard (CSS-compatible):**
`linear`, `ease`, `ease_in`, `ease_out`, `ease_in_out`

**Polynomial:**
`ease_in_quad`, `ease_out_quad`, `ease_in_out_quad`
`ease_in_cubic`, `ease_out_cubic`, `ease_in_out_cubic`
`ease_in_quart`, `ease_out_quart`, `ease_in_out_quart`

**Special:**
`ease_in_sine`, `ease_out_sine`, `ease_in_out_sine`
`ease_in_expo`, `ease_out_expo`
`ease_in_circ`, `ease_out_circ`

**Elastic / Bounce / Spring:**
`ease_in_elastic`, `ease_out_elastic`, `ease_in_out_elastic`
`ease_in_bounce`, `ease_out_bounce`
`spring` (requires `easing_params: { stiffness, damping, mass }`)

**Manim-compatible:**
`smooth` (Manim's default), `rush_into`, `rush_from`, `there_and_back`

### Custom Easing: Cubic Bezier
```json
{
  "easing": "cubic_bezier",
  "easing_params": { "p1x": 0.25, "p1y": 0.1, "p2x": 0.25, "p2y": 1.0 }
}
```

### Custom Easing: Spline Keyframes
```json
{
  "easing": "spline",
  "easing_params": {
    "keypoints": [[0.0, 0.0], [0.3, 0.8], [0.7, 0.2], [1.0, 1.0]]
  }
}
```

### Interpolation Types by Property

| Property Type | Default Interpolation |
|---|---|
| Numbers (x, y, radius, opacity) | Linear → easing function applied |
| Colors (hex strings) | LAB colorspace interpolation (perceptually uniform) |
| Points arrays (polygon vertices) | Per-vertex linear interpolation |
| SVG path `d` string | Morphing via path normalization + vertex matching |
| Rotation | Shortest-path angular interpolation |
| Scale | Multiplicative (geometric) interpolation |

**Color interpolation in LAB colorspace** is a significant advantage over every JS library that interpolates in RGB (which produces ugly grey midpoints for complementary colors).

---

## 11. ASSET PIPELINE (SVG, IMAGES, FONTS)

### Asset Declaration
All external assets are declared in the `assets` block and given a stable `id`. The engine loads and caches them at scene load time, not at render time.

### SVG Import
- Library: `resvg` (Rust-native SVG renderer)
- SVGs are rasterized to a texture at their display resolution or kept as paths for GPU rendering
- SVG animations (SMIL) are stripped — use Lumina's timeline instead
- Individual SVG elements can be extracted by `id` and treated as Lumina Path objects

### Raster Image Import
- Formats: PNG, JPEG, WebP
- Library: `image` crate
- Images are uploaded to GPU texture atlas
- Supports: `opacity`, `scale`, `rotation`, `x`, `y` animation

### Font Import
- Formats: TTF, OTF, WOFF2
- Custom fonts referenced by `id` in `assets.fonts`
- System fonts available by name: `"font_id": "system:Arial"`
- Math fonts (STIX2, Latin Modern Math) bundled with Lumina

---

## 12. EXPORT FORMAT PIPELINE

### MP4 (H.264)
- Render frames to RGBA byte arrays → pipe to FFmpeg stdin
- FFmpeg flags: `-crf 18 -preset fast -pix_fmt yuv420p`
- Realistic time for 30s @1080p60: 60–180 seconds total
- **This is not milliseconds. Document this honestly.**

### WebM (VP9)
- Same pipeline, different FFmpeg flags
- Smaller file size, better for web
- ~20% longer encode time than H.264

### GIF
- Library: `gif` crate (pure Rust, no FFmpeg)
- Frame deduplication and palette optimization
- Max recommended: 720p, 30fps (GIFs get large fast)
- Dithering: Floyd-Steinberg (produces high quality output)

### PNG Sequence
- Every frame saved as numbered PNG
- Useful for post-processing in other tools
- Fastest "export" option (no encoding)

### Interactive HTML Bundle
- Compiles the scene + WASM runtime + LSF file into a single `.html` file
- No server required, embeds everything
- Uses the WASM renderer (tiny-skia initially, Vello/WebGPU when available)
- Output: ~500KB–2MB depending on scene complexity
- Supports all interactive events

### Lottie JSON (Phase 3)
- Export to Lottie format for backwards compatibility with existing Lottie players
- Limited to features Lottie supports (no LaTeX, no events)

---

## 13. WASM & BROWSER RUNTIME (HONEST SCOPE)

### What We Are Building (Phase 2, not Phase 1)

The original blueprint implied WASM real-time 60fps GPU rendering was straightforward. Here is the honest engineering truth and the plan to get there:

**Phase 2A: WASM + CPU Renderer (tiny-skia)**
- Compile Lumina core + tiny-skia to WASM via wasm-pack
- Run in Web Worker (off main thread, no jank)
- Render each frame to an OffscreenCanvas
- Transfer frame to main thread via ImageBitmap (zero-copy)
- Expected: 60fps for simple scenes (<100 objects), 20–30fps for complex ones
- This ships in Phase 2 and is a real, working product

**Phase 2B: WASM + GPU Renderer (WebGPU)**
- WebGPU is available in Chrome 113+, Firefox 122+, Safari 18+
- Compile Vello + wgpu (WebGPU backend) to WASM
- Render directly to a `<canvas>` via WebGPU API
- Expected: 60fps for <1000 objects, matching native performance
- Fallback: detect WebGPU availability, fall back to Phase 2A if absent

**What "60fps in browser" requires:**
1. Scene must not change every frame (static scenes are cached)
2. Objects >1000 should be grouped and transformed as units
3. LaTeX pre-rendered to textures (not re-laid-out per frame)
4. Event handlers debounced properly

### The JavaScript/React API

```typescript
import { LuminaPlayer } from '@lumina/react';

export function App() {
  return (
    <LuminaPlayer
      scene={sceneJson}
      width={1280}
      height={720}
      autoplay={true}
      controls={true}
      onObjectClick={(objectId) => console.log(objectId)}
    />
  );
}
```

The `LuminaPlayer` component handles: WASM loading, Web Worker setup, canvas management, playback controls, and event forwarding. It is a single npm install.

---

## 14. INTERACTIVE EVENT SYSTEM

### Event Types

| Trigger | Condition | Available For |
|---|---|---|
| `click` | Pointer up within object bounds | All objects |
| `double_click` | Two clicks within 300ms | All objects |
| `hover_enter` | Pointer enters object bounds | All objects |
| `hover_exit` | Pointer leaves object bounds | All objects |
| `drag_start` | Pointer down + move | Objects with `draggable: true` |
| `drag` | Pointer move while dragging | Draggable objects |
| `drag_end` | Pointer up after drag | Draggable objects |
| `timeline_reached` | Playhead hits a time | Global |
| `animation_complete` | Object finishes a keyframe tween | Object-specific |

### Action Types

| Action | Effect |
|---|---|
| `jump_to_time` | Seek playhead to specified time |
| `set_property` | Immediately set a property value |
| `play_from` | Start playing from a time |
| `pause` | Pause playback |
| `tween_to` | Animate an object to a new state on-demand |
| `show_tooltip` | Display a text overlay |
| `emit_custom` | Fire a named custom event for the host app |

### The Draggable Vector Example (What the Blueprint Promised)

```json
{
  "objects": {
    "vec_a": {
      "type": "Arrow",
      "properties": { "from": [400, 540], "to": [700, 300], "color": "#E74C3C" },
      "draggable": true,
      "drag_handle": "tip"
    },
    "magnitude_label": {
      "type": "LaTeX",
      "properties": { "expression": "|\\vec{a}| = 0", "x": 800, "y": 200 }
    }
  },
  "events": [
    {
      "object": "vec_a",
      "trigger": "drag",
      "action": {
        "type": "emit_custom",
        "event_name": "vector_moved",
        "payload": { "from": "$drag.from", "to": "$drag.to" }
      }
    }
  ]
}
```

The host React app listens to `vector_moved`, computes the new magnitude, and calls `scene.update({ magnitude_label: { expression: "|\\vec{a}| = " + magnitude }})`. This is the live-updating formula demo — fully achievable.

---

## 15. DIFF/PATCH INCREMENTAL UPDATE MODEL

### Problem Being Solved
When an AI agent wants to modify an existing scene (add an object, change a keyframe), it should not have to resend the entire LSF. It should send a minimal patch.

### The ScenePatch Format

```json
{
  "base_scene_id": "scene_abc123",
  "patches": [
    {
      "op": "add_object",
      "id": "new_circle",
      "type": "Circle",
      "properties": { "cx": 500, "cy": 300, "radius": 80, "fill": "#FF0000" }
    },
    {
      "op": "add_keyframe",
      "object": "new_circle",
      "keyframe": { "time": 1.5, "state": { "radius": 120 }, "easing": "spring" }
    },
    {
      "op": "update_property",
      "object": "hypotenuse_label",
      "property": "font_size",
      "value": 64
    },
    {
      "op": "remove_object",
      "id": "old_decoration"
    }
  ]
}
```

### Patch Operations

| Op | Effect |
|---|---|
| `add_object` | Add a new object to the registry |
| `remove_object` | Remove object and all its timeline entries |
| `update_property` | Change a static property value |
| `add_keyframe` | Add a keyframe to an object's timeline track |
| `remove_keyframe` | Remove a specific keyframe by time |
| `update_keyframe` | Replace a keyframe's state at a given time |
| `add_event` | Add an event listener |
| `remove_event` | Remove an event listener |
| `update_canvas` | Change canvas duration, fps, background |

### How the Diff Engine Works
The engine maintains a scene hash. When a patch is applied, only the affected subtree of the scene graph is invalidated and re-compiled. Unchanged frames are served from the render cache. For video export, only changed frames are re-rendered.

---

## 16. AI HEADLESS RENDERING SERVER

### Technology: Axum (Rust async web framework)

The headless server is a Rust Axum service, not Node.js. It is embedded in the Lumina crate tree (`lumina-server`), meaning it shares the same core as the library. No language boundary, no serialization overhead between web server and renderer.

### API Endpoints

**POST /render**
Accept LSF JSON → render → return video/html
```http
POST /render
Content-Type: application/json

{
  "scene": { ...LSF JSON... },
  "output": {
    "format": "mp4",
    "resolution": "1080p",
    "fps": 30
  }
}

→ 200 OK
Content-Type: video/mp4
[binary video stream]
```

**POST /validate**
Accept LSF JSON → validate → return structured errors
```http
POST /validate
→ { "valid": true, "warnings": [...] }
→ { "valid": false, "errors": [...], "warnings": [...] }
```

**POST /patch**
Accept base scene ID + patch → return updated scene + re-render
```http
POST /patch
{ "base_scene_id": "abc123", "patches": [...] }
→ { "scene_id": "xyz456", "render_url": "/results/xyz456.mp4" }
```

**GET /schema**
Return the current JSON Schema for LSF (always up-to-date)
```http
GET /schema
→ { ...JSON Schema draft-07 document... }
```

**GET /objects**
Return the full object type registry with all valid properties
```http
GET /objects
→ { "Circle": { "required": ["cx","cy","radius"], "optional": [...], "animatable": [...] } }
```

### Render Time Reality

For the headless cloud server with GPU access (Tesla T4 class):

| Scene | Resolution | Duration | Estimated Render + Encode Time |
|---|---|---|---|
| Simple (< 50 obj) | 720p | 30s | 8–15 seconds |
| Medium (50–200 obj) | 1080p | 60s | 25–60 seconds |
| Complex (200+ obj) | 1080p | 120s | 90–240 seconds |

For interactive HTML bundle export (no video encode): 1–5 seconds regardless of scene complexity.

**The honest pitch:** Lumina is fast for an animation engine. It is not a streaming API. Set user expectations correctly.

### AI Agent Integration Example (Python)

```python
import lumina_cloud
import anthropic

client = anthropic.Anthropic()
lumina = lumina_cloud.Client(api_key="...")

# Step 1: Generate LSF with Claude
schema = lumina.get_schema()

message = client.messages.create(
    model="claude-opus-4-5",
    max_tokens=4096,
    system=f"""You generate Lumina Scene Format JSON animations.
Schema: {schema}
Return ONLY valid JSON. No markdown.""",
    messages=[{
        "role": "user",
        "content": "Create a 10-second animation explaining the dot product of two vectors."
    }]
)

scene_json = message.content[0].text

# Step 2: Validate
validation = lumina.validate(scene_json)
if not validation.valid:
    # Re-prompt Claude with errors (self-correction loop)
    ...

# Step 3: Render
video_bytes = lumina.render(scene_json, format="mp4", resolution="1080p")
with open("dot_product.mp4", "wb") as f:
    f.write(video_bytes)
```

---

## 17. ERROR MODEL & VALIDATION SYSTEM

### Validation Levels

**ERROR** — Rendering cannot proceed. Must be fixed.
- Unknown object IDs in timeline
- Invalid property types (string where float expected)
- Circular group references
- Asset IDs not declared

**WARNING** — Rendering proceeds, but output may be unexpected.
- Duplicate keyframes at same timestamp
- Object has timeline entries but no initial property declaration
- Timeline entries beyond canvas duration
- Very high z_index values (potential performance issue)

**INFO** — Informational hints for optimization.
- Scene has >500 objects (recommend grouping)
- No easing specified (defaulting to linear)
- Font not found (using bundled fallback)

### Error Message Structure (AI-Optimized)

Every error includes:
- `code` — Machine-readable string for programmatic handling
- `path` — JSONPath to the offending element (`$.timeline[3].object`)
- `message` — Human/AI-readable description
- `fix_suggestion` — Specific actionable fix the AI can apply
- `context` — Surrounding data that helps identify the problem

---

## 18. PHASE-BY-PHASE ROADMAP

### Phase 1: The Solid Foundation (Months 1–6)
**Goal:** Ship a working Python library that does one thing excellently: declarative JSON → MP4/PNG/GIF.

**Deliverables:**
- [ ] Rust core: SceneGraph, KeyframeTrack, Interpolator (all 30 easing functions)
- [ ] All 16 object types implemented
- [ ] Vello GPU renderer (native)
- [ ] Tiny-skia CPU renderer (headless fallback)
- [ ] Text rendering (Fontdue)
- [ ] LaTeX rendering (MiTeX)
- [ ] SVG import (resvg)
- [ ] LSF schema (full spec above)
- [ ] Schema validator with AI-optimized errors
- [ ] Python bindings (PyO3/Maturin)
- [ ] Export: MP4, GIF, PNG sequence
- [ ] CLI: `lumina render scene.lsf -o output.mp4`
- [ ] Published JSON Schema at `schema.lumina.dev`
- [ ] Documentation site

**Success metric:** Claude can generate a 30-second animated math explanation LSF that renders correctly first try, 80% of the time.

### Phase 2: The Web Layer (Months 7–12)
**Goal:** Run Lumina in the browser. Real interactivity. React SDK.

**Deliverables:**
- [ ] WASM build (wasm-pack, CPU renderer)
- [ ] Web Worker integration
- [ ] Interactive event system (all event/action types)
- [ ] Draggable objects
- [ ] React component (`<LuminaPlayer>`)
- [ ] npm package `@lumina/react`
- [ ] Interactive HTML bundle export
- [ ] WebGPU renderer (Vello → wasm)
- [ ] Diff/Patch engine + API
- [ ] Headless Axum server (local mode)

**Success metric:** `npm install @lumina/react` and a 500-object interactive math scene runs at 55+ fps on a 2022 MacBook Chrome.

### Phase 3: The AI Cloud (Months 13–18)
**Goal:** SaaS API. AI agents can render animations without infrastructure.

**Deliverables:**
- [ ] Cloud render API (Lumina Cloud)
- [ ] `/render`, `/validate`, `/patch`, `/schema`, `/objects` endpoints
- [ ] AI self-correction loop SDK (Python + JS)
- [ ] Lottie export
- [ ] 3D transform layer (perspective, rotateX/Y/Z)
- [ ] Particle system
- [ ] Template library (30+ starter animations)
- [ ] Visual schema explorer at `schema.lumina.dev`
- [ ] Pricing and billing

### Phase 4: The Studio (Months 19–24)
**Goal:** Visual timeline editor built on top of the open format.

**Deliverables:**
- [ ] Browser-based timeline editor
- [ ] Import/export LSF
- [ ] Team collaboration
- [ ] Brand kit (colors, fonts, templates)
- [ ] Lumina Studio desktop app (Tauri)

---

## 19. MVP DEFINITION — WHAT SHIPS FIRST

### The Minimum Viable Lumina

One Python command:
```python
import lumina

scene = lumina.Scene.from_json("pythagorean.lsf")
scene.render("pythagorean.mp4")
```

One CLI command:
```bash
lumina render pythagorean.lsf -o pythagorean.mp4
```

That's it. No browser. No WASM. No cloud. No interactivity. Just: declarative JSON in, high-quality GPU-rendered math animation out.

**Why this is the MVP:**
1. It proves the core thesis (AI-writable declarative format works)
2. It directly competes with Manim on Manim's home turf and wins on speed
3. It creates the open format that everything else is built on
4. It can ship in 3–4 months with one skilled Rust developer

**What makes even the MVP better than every competitor:**
- LaTeX rendering is first-class, not an afterthought
- GPU-accelerated (10x faster than Manim for complex scenes)
- AI-validated schema with self-correction support
- All 30 easing functions including spring physics
- LAB colorspace interpolation
- A published JSON Schema that IDEs and AI agents consume
- Path morphing between polygon shapes
- Group transforms with clean child inheritance

---

## APPENDIX A: Why Lumina Beats Each Competitor at MVP

| Competitor | What Lumina MVP Does Better |
|---|---|
| **Manim** | 10x faster render, declarative (no Python state bugs), WASM-ready format, better AI integration |
| **Lottie** | No After Effects dependency, programmatic API, math/LaTeX support, open format |
| **Motion Canvas** | Video export, AI-native schema, LaTeX, no TypeScript required |
| **Theatre.js** | Includes a renderer, not just a timeline. Full stack. |
| **GSAP** | Video export, AI-native, LaTeX, runs headlessly |

---

## APPENDIX B: Naming Note

**Lumina** is clean, memorable, and available on PyPI/npm at time of writing. The `.dev` TLD (`lumina.dev`) is available. Alternatives if taken: **Kinema**, **Motif**, **Vega** (check against existing viz library), **Axiom**.

---

*Blueprint version 2.0. All performance figures are estimates based on comparable library benchmarks. Validate against your hardware before publishing claims.*
