# Future Improvements (todo.md)

This document outlines the roadmap and technical enhancements planned for the Lumina engine.

## 1. Renderer Enhancements

- [ ] **Vello CLI Integration**: Finish the implementation of the Vello (GPU) backend in the CLI tool.
- [ ] **Path Morphing**: Implement smooth vertex-matching interpolation for SVG paths of differing lengths.
- [ ] **LAB Color Interpolation**: Move from RGB to LAB colorspace for perceptually uniform color transitions.

## 2. Animation & Easing

- [ ] **Spring Physics**: Add a dedicated spring solver for physical-feeling animations.
- [ ] **Complete Easing Library**: Implement all 30+ functions defined in the blueprint (Elastic, Bounce, etc.).
- [ ] **Bezier Easings**: Support custom cubic-bezier easing curves.

## 3. Formats & Interactivity

- [ ] **WASM Runtime**: Complete the browser-based player with WebGPU support.
- [ ] **Interactive Events**: Fully implement the event bus for click, hover, and drag interactions in the WASM player.
- [ ] **Lottie Export**: Build a converter to export LSF scenes to Lottie JSON for legacy compatibility.

## 4. AI & Tooling

- [ ] **Self-Correction Loop**: Integrate the schema validator into a CLI-based AI feedback tool.
- [ ] **Live Preview**: Create a lightweight watcher that re-renders the scene on file change.
- [ ] **Asset Pipeline**: Add automatic optimization for imported SVGs and raster images.

## 5. Math & Text

- [ ] **LaTeX Parts Animation**: Allow individual symbols in a LaTeX expression to be animated independently (Write-on effect).
- [ ] **MathML Support**: Direct rendering of MathML structures.
- [ ] **Font Fallbacks**: Better handling of missing glyphs using system-wide fallback chains.
