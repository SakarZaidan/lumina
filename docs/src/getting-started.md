# Getting Started

## Prerequisites

- **Rust** (latest stable, via [rustup](https://rustup.rs))
- **FFmpeg** for MP4 export (`apt install ffmpeg` / `brew install ffmpeg`)
- A TTF font for text rendering (e.g. `fonts-liberation` on Ubuntu)

## Build

```bash
git clone https://github.com/SakarZaidan/lumina.git
cd lumina
cargo build --release
```

## Render a scene

```bash
# MP4 video
./target/release/lumina-cli --scene examples/unit_circle.lsf --output unit_circle.mp4 --format mp4

# PNG frame sequence (no FFmpeg needed)
./target/release/lumina-cli --scene examples/hello.lsf --output frames/ --format png
```

Useful flags:

| Flag | Effect |
|---|---|
| `--backend skia\|vello` | CPU rasterizer (default) or GPU rasterizer. |
| `--format mp4\|png` | Encoded video or a numbered PNG sequence. |
| `--watch` | Re-render a preview frame whenever the scene file changes. |
| `--verbose` | Print render timing at the end. |

## Your first scene

```json
{
  "version": "1.0",
  "meta": { "title": "Fade In", "author": "you", "created_at": "2026-05-25" },
  "canvas": { "width": 1280, "height": 720, "fps": 60, "duration": 2.0, "background": "#0F0F1A" },
  "assets": { "fonts": [{ "id": "sans", "path": "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf" }] },
  "objects": {
    "title": { "type": "Text", "properties": { "content": "Hello, Lumina", "x": 640, "y": 360, "align": "center", "font_id": "sans", "font_size": 96, "color": "#FFFFFF", "opacity": 0.0 } }
  },
  "timeline": [
    { "time": 0.0, "object": "title", "state": { "opacity": 0.0 }, "easing": "linear" },
    { "time": 1.5, "object": "title", "state": { "opacity": 1.0 }, "easing": "ease_out_cubic" }
  ]
}
```

## Other entry points

- **Python**: `pip install maturin && (cd sdks/python && maturin develop)`, then `import lumina`.
- **JavaScript/React**: `npm install @lumina/sdk` and mount `<LuminaPlayer scene={...} />`.
- **HTTP**: `cargo run -p lumina-server` exposes `/render`, `/validate`, `/patch`, `/schema`, `/objects`.
