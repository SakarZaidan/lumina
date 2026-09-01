# 06 — Render and output fidelity

The engine's job ends at a file someone watches. This dimension is about the
last mile, and it is the least examined part of the codebase — nothing in the
debt register touches it.

## Current state

**Compositing happens in 8-bit non-linear sRGB.** tiny-skia blends
premultiplied sRGB; `anti_alias = true` is set per paint
(`skia_backend.rs:342` and a dozen more), and Vello uses `AaConfig::Area`
(`vello_backend.rs:992`). Neither composites in linear light. The visible
costs are the familiar ones: dark fringing on antialiased edges against light
backgrounds, and gradients that go muddy through their midpoint.

That last one is doubly odd, because the *timeline* interpolates colour in
CIELAB (`interpolator.rs:44`) while the *renderer* interpolates gradient
stops in sRGB. Two colour models, one frame — see
[04](04-math-physics-accuracy.md#colour-is-round-tripped-through-a-hex-string-every-frame).

**The MP4 encode is untagged.** `export/src/lib.rs:175`:

```
-c:v libx264 -preset fast -crf 18 -pix_fmt yuv420p
```

No `-colorspace`, no `-color_primaries`, no `-color_trc`, no `-color_range`.
Players and browsers then guess the transfer characteristics, and different
players guess differently — the same file looks different in QuickTime, VLC,
and Chrome. There is also no `-tune animation` (x264 has a tune built for
exactly this content), no `+faststart` for web playback, and preset and CRF
are hardcoded rather than exposed.

**One 8-bit pixel format, no alpha, no audio.** `yuv420p` only. WebM is
libvpx-vp9 `-b:v 0 -crf 30` (`:186`) without `-row-mt 1` and without alpha.
There is no way to produce a file with a transparent background for
compositing, no 10-bit output, no lossless intermediate, and no audio track —
so an explainer video cannot carry narration without a second tool.

The GIF path is the one that is already right: two-stage
`palettegen=stats_mode=diff` plus `paletteuse=dither=floyd_steinberg` (`:200`).

**Text renders at integer positions and diverges between backends.** Skia
draws glyphs inline; Vello resamples a `raster.rs` string bitmap. Glyph
antialiasing and low-opacity blending differ enough that the text parity
fixture carries a wider tolerance (TD-18). Neither path does sub-pixel
positioning, so text jitters when it moves slowly.

**SVG input drops curves silently.** `common/path.rs` implements M/m, L/l,
H/h, V/v, C/c, Z/z — and nothing else. Real SVG uses Q, S, T, and elliptical
arcs constantly. Worse, one unparseable token returns `None` for the *entire*
path (`path.rs:99-102`) and the caller skips the shape with no diagnostic.

## Target

Output that survives professional scrutiny: correct colour end to end,
correctly tagged, available in the formats an editor expects, with text that
holds still.

## Work items

| ID | Item | Acceptance |
|---|---|---|
| `AAA-OUT-01` | Composite in linear light; dither on the way down to 8-bit | Edge fringing gone; a reference gradient matches a linear-light reference |
| `AAA-OUT-02` | Gradient stops interpolate in the same space as the timeline | Gradient midpoint equals the two-keyframe fade midpoint |
| `AAA-OUT-03` | BT.709 colour tagging on every encode | `ffprobe` reports primaries, transfer, matrix, and range |
| `AAA-OUT-04` | `-tune animation`, `+faststart`, configurable preset/CRF | Same visual quality at a measurably smaller file |
| `AAA-OUT-05` | 10-bit output (`yuv420p10le`) | Banding on slow gradients measurably reduced |
| `AAA-OUT-06` | Alpha output: VP9 with alpha, ProRes 4444 | A scene with a transparent background composites in an editor |
| `AAA-OUT-07` | Lossless intermediates: PNG sequence already exists; add EXR | Float-precision output for VFX pipelines |
| `AAA-OUT-08` | Audio track support in the schema and the encode | A scene declares an audio asset; the MP4 carries it in sync |
| `AAA-OUT-09` | Sub-pixel text positioning | Slow-moving text does not jitter between frames |
| `AAA-OUT-10` | Unify the two text layout paths (TD-18) | The text parity fixture drops to the default tolerance |
| `AAA-OUT-11` | Full SVG path grammar: Q, S, T, A | An SVG with arcs renders identically to a browser |
| `AAA-OUT-12` | A malformed path token reports which token and where | No silent whole-path drop |
| `AAA-OUT-13` | `--quality draft\|standard\|final` presets | One flag trades render time for output quality predictably |

## A note on `AAA-OUT-01`

Moving to linear-light compositing changes almost every pixel in almost every
fixture. It is the single largest golden-image churn in this program, and it
must land as one PR that regenerates every golden with a written justification
(ENGINEERING_PRINCIPLES #1: intentional visual changes update the goldens
explicitly). Doing it piecemeal would make the parity suite useless for the
duration.

## Metrics moved

Output fidelity — a new scorecard dimension, 65 → 95. Also lifts Examples,
since the showcase renders are the shop window.

## Sequencing

Wave 4, and after Wave 3: `AAA-OUT-01` roughly doubles per-pixel work in the
naive implementation, so the performance items land first and the benchmark
gate measures the cost honestly. `AAA-OUT-03` and `04` are small and can move
earlier — they are ffmpeg argument changes with immediate visible benefit.
