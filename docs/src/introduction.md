# Introduction

**Lumina is the animation engine for the AI era: declarative by design, GPU-capable by architecture, and runnable everywhere humans and machines need motion.**

You write a JSON scene file — objects, their properties, and a timeline of keyframes — and Lumina evaluates that timeline, applies easing, and rasterizes the result to video or to a live canvas. There is no imperative API to misuse and no GUI to learn. Because a scene is pure data, an LLM can write one, a validator can check it, and the engine renders it deterministically: same input, same pixels.

## Why it exists

Existing animation tools each have a structural mismatch with how software is built today:

- Imperative APIs require stateful reasoning that LLMs hallucinate.
- CPU-bound renderers struggle with complex math scenes.
- No single format runs both offline (video) and online (interactive).
- LaTeX/math rendering is usually bolted on as an afterthought.

Lumina addresses these with one coherent architecture: a declarative, validated, open JSON format (LSF); a backend-agnostic renderer (CPU tiny-skia + GPU Vello); first-class text/LaTeX; image/SVG/GIF compositing; and a headless server for programmatic use.

## What you can build

- Educational math/physics explainers rendered to MP4.
- Live, interactive visualizations embedded in the browser via the JS SDK.
- AI-generated animations validated and rendered server-side or from Python.

Read on to [get started](./getting-started.md) in about a minute.
