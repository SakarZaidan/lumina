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
| ~~`AAA-OUT-01`~~ | ~~Composite in linear light~~ | **Blocked by the rasteriser, not by effort.** `tiny-skia` has exactly one pixel type, `PremultipliedColorU8` — 8-bit, sRGB. There is no linear-light or higher-precision buffer to composite into, so this cannot be done without replacing the CPU backend, which would abandon the rule that Skia defines the pixels. Reopen only alongside a rasteriser decision |
| `AAA-OUT-02` | Gradient stops interpolate in the same space as the timeline | Gradient midpoint equals the two-keyframe fade midpoint. **Done** — neither backend exposes an interpolation space, but both accept arbitrarily many stops, so the perceptual curve is sampled and the samples handed over |
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

## `AAA-OUT-01` is blocked by the rasteriser

This document originally called linear-light compositing the flagship item and
warned that it would churn every golden image. The obstacle turned out to be
earlier than that.

`tiny-skia` exposes exactly one pixel type: `PremultipliedColorU8`. Eight bits
per channel, sRGB, premultiplied. There is no linear-light buffer, no f32 or
16-bit surface, and no blend-space option — so there is nothing to composite
*into*. Doing this means replacing the CPU rasteriser, which abandons the rule
that Skia defines the pixels and Vello matches them (DESIGN.md), and that is a
decision far larger than an output-fidelity item.

Recorded rather than quietly dropped, because the reasoning is the useful part:
the plan proposed something the chosen dependency cannot express, and no amount
of effort inside this wave would have surfaced that except reading the pixel
type.

**What is reachable, and was done instead:** gradient stops (`AAA-OUT-02`).
Neither backend lets a caller choose an interpolation space, but both accept
arbitrarily many stops — so sampling the perceptual curve and handing over the
samples gets the right answer through an API that cannot be asked for it
directly. That closes the specific inconsistency worth closing: the timeline
blended colours perceptually while gradients blended them in sRGB, so the same
two colours produced two different midpoints in one frame.

## Metrics moved

Output fidelity — a new scorecard dimension, 65 → 95. Also lifts Examples,
since the showcase renders are the shop window.

## Sequencing

Wave 4, and after Wave 3: `AAA-OUT-01` roughly doubles per-pixel work in the
naive implementation, so the performance items land first and the benchmark
gate measures the cost honestly. `AAA-OUT-03` and `04` are small and can move
earlier — they are ffmpeg argument changes with immediate visible benefit.
