# Lumina Architecture

Lumina is a modular, GPU-native animation engine built in Rust, designed for performance, correctness, and AI agent integration.

## Core Design Philosophy

- **Declarative First**: All scenes are pure data (LSF). No imperative function calls that LLMs might hallucinate.
- **GPU-Native**: Utilizing [Vello](https://github.com/linebender/vello) (wgpu) for path rendering to achieve frame rates unattainable by CPU-based solutions.
- **WASM-First**: Built with `wasm-pack` support for seamless web deployment via WebGPU.

## System Architecture

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
└─────────────────────────────────────────────────────┘
│                 TEXT / MATH LAYER                     │
│  Fontdue (text) │ MiTeX (LaTeX→paths) │ resvg (SVG)  │
└─────────────────────────────────────────────────────┘
```

## Renderer Strategy

Lumina utilizes a two-backend architecture to ensure high performance across all environments:

1. **Vello (GPU/wgpu)**:
    - Target: High-performance real-time playback and browser rendering.
    - Path rendering via compute shaders.
    - Capacity: 500–2,000+ complex vector paths at 60fps on modern GPUs.

2. **Tiny-Skia (CPU/Rust)**:
    - Target: Headless rendering servers (no GPU available), CI/CD pipelines.
    - Performance: Reliable 50–200 paths at 60fps, fully parallelized with `Rayon`.

## Performance Claims

| Scenario | Realistic Expectation |
|---|---|
| 500 objects, browser | 45–60fps (Vello/WebGPU) |
| 30s clip @1080p60 (Headless) | 10–30 seconds render time |
| Video Encoding (FFmpeg) | 2–15 seconds post-render |

*Performance benchmarks are based on optimized builds; raw performance varies by GPU/CPU capability.*
