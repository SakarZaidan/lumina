# 02 — Performance

## Current state

The engine is deterministic and adequate; it is also doing a great deal of
work twice. Nothing here is speculative — each line is a read of the code.

**Rendering allocates twice per frame.** `skia_backend.rs:988` builds a fresh
`Pixmap` and `:1001` copies it with `.to_vec()`. At 1080p that is two 8.3 MB
allocations per frame, ~7 200 per minute of output. The `Renderer` trait
signature (`renderer/src/lib.rs:39`, returning `Result<Vec<u8>, _>`) *requires*
the copy.

**Text is re-rasterised from outlines every frame.** `raster.rs:44` measures,
`:65-70` rasterises, and neither caches. `font_for_char`
(`lumina-text/src/lib.rs:47-68`) linearly walks every loaded font calling
`font.metrics(c, 16.0)` until one reports a nonzero advance — per character,
per measure, per rasterise, per frame. A 50-character title over a 60-second
60 fps render is ~180 000 rasterisations of perhaps 20 distinct glyphs.

**Export is single-threaded, and `rayon` has never been imported.** It is
declared in `crates/lumina-export/Cargo.toml:20` and matched by no `.rs` file
in the workspace. `stream_frames` (`export/src/lib.rs:89-113`) and
`export_png_sequence` (`:36-77`) render strictly in sequence. Frames are
independent by construction — `get_state_at(t)` is pure and particles are
analytic — so this is the cleanest parallelism available anywhere in the
codebase, and `export_png_sequence` has no ordering constraint at all.

**The timeline rebuilds the world every frame.** `get_state_at`
(`timeline.rs:105-124`) clones every object id, every property name, and
deep-clones every `Value`, then re-collects the lot into `serde_json::Map`s
(TD-03). `evaluate_track` (`:193`) then linear-scans a **sorted** keyframe
array; `get_camera_at` (`:158`) does the same.

**Invariant work repeated per frame.** `sorted_root_ids`
(`common/scene.rs:34`) rebuilds a `HashSet<String>` of cloned child ids plus
two `Vec<String>` for a result that cannot change during a render.
`spline_easing` (`easing.rs:58-65`) allocates three vectors and re-solves the
Fritsch–Carlson tangents on every call, per property, per frame.
`latex_to_unicode` (`skia_backend.rs:1134`) runs ~70 chained `String::replace`
passes over input that never changes. The Plot loop rebuilds an evalexpr
context *and re-parses the expression* per sample (`:843-847`, TD-04) —
200 samples × 3 600 frames is 720 000 parses of one constant string.

**The GPU backend runs on the CPU.** `vello_backend.rs:66,84` set
`force_fallback_adapter: true` and `use_cpu: true`. Excellent for CI
determinism; not a GPU path, and the README says it is one. A new texture and
staging buffer are also created per frame (`:961-976`, `:1004-1009`).

**One cache exists in the whole engine:** `SkiaRenderer::svg_cache`
(`skia_backend.rs:48`).

## Target

Measured, not asserted. Every claim in `docs/src/performance.md` backed by a
criterion number produced in CI, and a regression gate that fails the build
rather than a note nobody reads.

## Work items

Baselines are captured **before** any change; ENGINEERING_PRINCIPLES #5 makes
this the entry condition, not the write-up.

| ID | Item | Where | Expected |
|---|---|---|---|
| `AAA-P-01` | Glyph atlas keyed `(char, font_id, size)`; index `font_for_char` instead of walking | `lumina-text`, `raster.rs:44,65` | Largest single win on text-heavy scenes |
| `AAA-P-02` | `render_into(&mut [u8])` on the trait; reuse one framebuffer per export | `renderer/src/lib.rs:39` | Two 8.3 MB allocations per frame removed |
| `AAA-P-03` | Frame-parallel export with `rayon` (TD-05) | `export/src/lib.rs:36-77,89-113` | Near-linear in cores for PNG sequences |
| `AAA-P-04` | Timeline state caching; stop cloning unchanged properties (TD-03) | `timeline.rs:105-124` | Scales with object count |
| `AAA-P-05` | `partition_point` for keyframe and camera lookup | `timeline.rs:193,158` | O(log K) per property |
| `AAA-P-06` | Compute root ordering once per render, not per frame | `common/scene.rs:34,55` | Removes 3 allocations/frame/group |
| `AAA-P-07` | Solve spline tangents in `Timeline::from_scene` | `easing.rs:58-65` | Removes 3 allocations per property per frame |
| `AAA-P-08` | Memoise `latex_to_unicode` per object | `skia_backend.rs:1134` | ~70 string passes → 1 |
| `AAA-P-09` | `build_operator_tree` once per plot; hoist the context (TD-04) | `skia_backend.rs:843` | 720 000 parses → 1 |
| `AAA-P-10` | Retain GPU texture and staging buffer across frames | `vello_backend.rs:961,1004` | Removes 2 GPU allocations/frame |
| `AAA-P-11` | Prefer a hardware adapter; `force_fallback_adapter` only under the CI env | `vello_backend.rs:66,84` | Makes the GPU claim true |
| `AAA-P-12` | `Vec::with_capacity` for particles | `raster.rs:143` | Count is known up front |

## Benchmarks to add

Three criterion groups exist (`timeline_eval`, `skia_render`, `easing`) and CI
runs none of them. The suite needs: a Vello render group, a full export
pipeline group (render + ffmpeg pipe), a text-heavy scene group (the only one
that would surface `AAA-P-01`), and a plot-heavy group for `AAA-P-09`.

## Metrics moved

Performance (65 → 95), Benchmarks (60 → 95). Adds a memory-profile row —
`planning/METRICS.md` currently records "n/a, tooling planned v0.5".

## Sequencing

Wave 3 in full, and deliberately **after** Wave 2. The pixel-diff parity suite
is the regression net for every item here; rewriting hot paths before it is
merged and trusted would be exactly the mistake TD-03/04/05 were sequenced to
avoid.
