# Performance

Lumina is fast for an animation engine — it is not magic, and it is honest about
where time goes. Video rendering is bounded by per-frame rasterization plus
FFmpeg encoding, not by the timeline math (scene-graph evaluation of a
2000-object scene is sub-millisecond).

## Benchmarks

Run them yourself:

```bash
cargo bench -p lumina-bench
```

The suite covers:

- `timeline_eval` — `Timeline::get_state_at` on synthetic 100 / 1000 / 2000-object scenes.
- `render_frame` — a single Skia frame at 1080p.
- `easing_dispatch` — overhead of the easing lookup (incl. `cubic_bezier`).

Record the numbers from your hardware in your fork's README rather than quoting
someone else's machine.

## Honest expectations

| Scenario | Realistic expectation |
|---|---|
| Headless 1080p60 render | seconds-to-minutes depending on object count + duration |
| FFmpeg encode | a few seconds on top of rendering |
| Browser playback (WASM, CPU) | 60 fps for simple scenes; fewer for heavy ones |
| Interactive HTML | no video encode → near-instant |

## Tips

- Group static sub-trees so transforms apply once.
- Keep `shadow` and SVG rasterization opt-in; both are cached/bounded but cost more than flat fills.
- For previews, use `--watch` (renders a single mid-point frame) instead of full MP4 exports.
