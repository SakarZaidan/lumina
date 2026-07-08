# Lumina Example Scenes

Every scene here is a plain LSF JSON file — render any of them with the CLI:

```bash
cargo run --release -p lumina-cli -- --scene examples/hello.lsf --output hello --format mp4
```

Formats: `png` (frame sequence), `mp4`, `webm`, `gif` (video formats need
`ffmpeg` on PATH). Add `--backend vello` for the GPU renderer, or `--watch`
for a live PNG preview while you edit the scene. Rendered demo videos of all
of these live in [`media/`](../media/).

## Scenes

| Scene | Duration | Demonstrates |
|---|---|---|
| [`hello.lsf`](hello.lsf) | 3 s | Smallest useful scene: Text + Line, fade-in, `ease_out_sine` |
| [`circle_bounce.lsf`](circle_bounce.lsf) | 4 s | `ease_out_bounce` vs `ease_in_quad` side by side |
| [`pythagorean.lsf`](pythagorean.lsf) | 8 s | Polygon, Arrow, LaTeX, Group; spring scale, label timing |
| [`dataviz_bars.lsf`](dataviz_bars.lsf) | 6 s | Animated bar chart with value labels |
| [`neural_net.lsf`](neural_net.lsf) | 11 s | Group scale-in, draw-on connections, activation pulse |
| [`fourier_series.lsf`](fourier_series.lsf) | 12 s | Four overlapping `Plot` curves converging on a square wave |
| [`unit_circle.lsf`](unit_circle.lsf) | 52 s | Full math video: camera moves, Axes, Plot, draw_fraction, LAB color |
| [`showcase_neural_network.lsf`](showcase_neural_network.lsf) | 150 s | **Flagship** (79 objects): SVG icons, gradients, shadows, particles, LaTeX, camera choreography |
| [`showcase_grand.lsf`](showcase_grand.lsf) | 45 s | **v0.3.0 reel**, rendered on the **Vello GPU** backend: spline easing, GPU text/LaTeX/SVG/particles, events |

## Generators

The two showcase scenes are generated, not hand-written — useful as a starting
point for producing LSF programmatically:

- [`gen_neural_showcase.py`](gen_neural_showcase.py) → `showcase_neural_network.lsf`
- [`gen_grand_showcase.py`](gen_grand_showcase.py) → `showcase_grand.lsf`

```bash
python3 examples/gen_grand_showcase.py   # regenerates the .lsf in place
```

## Font paths (portability note)

The scenes reference a system font by absolute path, e.g.:

```json
"fonts": [{ "id": "main", "path": "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf" }]
```

That path exists on Debian/Ubuntu. On other systems, point it at any TTF you
have:

| OS | Try |
|---|---|
| Fedora | `/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf` |
| macOS | `/System/Library/Fonts/Supplemental/Arial.ttf` (or any font in `~/Library/Fonts`) |
| Windows | `C:\\Windows\\Fonts\\arial.ttf` |

If a font fails to load, text objects simply don't draw — the render still
succeeds. Bundling a portable OFL-licensed font with the examples is planned
(`planning/TECH_DEBT.md` TD-16).
