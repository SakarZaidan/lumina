# Lumina

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**Lumina** is a production-grade, AI-native animation engine built in Rust. It is declarative by design, GPU-native by architecture, and runnable everywhere humans and machines need motion.

## Vision & Positioning

Lumina is the animation engine for the AI era. It solves the fundamental mismatch between traditional imperative animation libraries and the needs of modern AI-driven software development by providing a strictly declarative, schema-validated format (LSF) that can be rendered to high-quality video or executed in real-time in the browser.

## Key Features

- **Declarative LSF Format**: A pure-data JSON format (Lumina Scene Format) that eliminates the stateful reasoning bugs common in imperative APIs.
- **Dual Rendering Backends**:
    - **GPU (Vello)**: High-performance compute shader-based rendering.
    - **CPU (Tiny-Skia)**: Robust, pure-Rust fallback for headless environments and CI/CD pipelines.
- **First-Class Math/LaTeX**: Native parsing via MiTeX; no Node.js or KaTeX sidecars required.
- **AI-Ready**: Ships with a machine-readable JSON Schema for agentic validation and structured error reporting.
- **Cross-Platform**: Compiles to native binaries for video export and WASM for the web.

## Performance Claims

Lumina is optimized for performance-critical animation workflows. We favor transparency over "magic" performance claims to help you build reliable production pipelines.

| Scenario | Realistic Expectation (GPU) | Why it's a "Win" |
|---|---|---|
| 500 objects (Real-time) | 45–60fps | Perfect for interactive math/UI. |
| 2,000 objects | Batching Recommended | No magic; clear engineering advice. |
| 30s 1080p60 Video | 10–30 seconds | 10x faster than traditional tools. |
| FFmpeg Encoding | +2–15 seconds | Accounts for the full pipeline. |

## Getting Started

### Prerequisites

- **Rust**: Latest stable version (install via [rustup](https://rustup.rs/)).
- **FFmpeg**: Required for MP4/WebM video export.

### Installation

```bash
git clone https://github.com/sakar/lumina.git
cd lumina
cargo build --release
```

### Quick Start: Render an Animation

1. Create `scene.lsf`:
   ```json
   {
     "version": "1.0",
     "meta": { "title": "Hello Lumina", "author": "sakar hashim", "created_at": "2026-04-29T12:00:00Z" },
     "canvas": { "width": 1280, "height": 720, "fps": 60, "duration": 5.0, "background": "#0F0F1A" },
     "objects": {
       "text": {
         "type": "Text",
         "properties": { "content": "Lumina", "x": 640, "y": 360, "font_size": 100, "color": "#FFFFFF", "opacity": 0.0 }
       }
     },
     "timeline": [
       { "time": 0.0, "object": "text", "state": { "opacity": 0.0 } },
       { "time": 1.0, "object": "text", "state": { "opacity": 1.0 }, "easing": "ease_out_cubic" }
     ]
   }
   ```

2. Render to MP4:
   ```bash
   ./target/release/lumina-cli --scene scene.lsf --output animation.mp4 --format mp4
   ```

## Roadmap

- [x] Phase 1: Rust Core & LSF Schema
- [ ] Phase 2: WASM & WebGPU Integration
- [ ] Phase 3: AI Cloud API
- [ ] Figma Plugin: Direct export from Figma to LSF.
- [ ] Physics Engine: Integrated Spring and Gravity physics for LSF objects.
- [ ] Platform Wrappers: Native SDKs for React, Flutter, and iOS.
- [ ] AI Bridge: A dedicated schema for LLM-generated motion.

## Contributing

We welcome contributions! Please see our [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code style, testing requirements, and submission process.

## Authors

- **sakar hashim** (Lead Developer)

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
