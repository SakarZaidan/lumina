# Current State

## Health dashboard

Updated with every entry below (and re-verified at every release). 🟢 healthy
· 🟡 known gaps, tracked · 🔴 broken/blocked.

| Area | | Notes |
|---|---|---|
| CI on `main` | 🟢 | 10 jobs green on ubuntu/macos/windows + MSRV + wasm |
| Tests | 🟢 | 117 native + 3 wasm passing; zero flakes |
| Coverage | 🟡 | still not measured — retargeted to v0.5 (`AAA-TEST-06`) |
| Benchmarks | 🟡 | exist, manual only; not in CI (TD-14 remainder, `AAA-TEST-07`) |
| Docs (book + rustdoc) | 🟢 | book live on Pages; every public item documented, lint-enforced |
| Examples | 🟢 | portable on any OS; CI renders none of them yet (`AAA-TEST-09`) |
| Security | 🟡 | server unhardened pre-v0.5 by design (TD-09); five audited DoS vectors open (`AAA-SEC-01..05`) |
| Backend parity | 🟢 | full visual parity, 16-fixture pixel-diff suite gating in CI; Windows probe suppressed (TD-20) |
| Release | 🟢 | v0.4.0 tagged and released |
| Distribution | 🔴 | nothing published to crates.io, PyPI, or npm — the largest open gap (`AAA-REL-*`) |
| Dependencies | 🟢 | deny green; 386 locked crates (mitex removed); rustybuzz tracked as TD-22 |

Rolling log, newest first. One dated entry per work session; ≤ 10 lines each.
For the release-by-release story see [HISTORY.md](./HISTORY.md).

---

## 2026-09-03 — crates.io names, and two blockers nobody had checked

- The owner supplied a crates.io token, and the first thing it revealed was
  that **the project cannot publish under its own names**. `lumina`,
  `lumina-core`, and `lumina-cli` are all taken.
- `lumina-core` is the bad one: an unrelated GUI framework described as *"wgpu
  rendering, Taffy layout"*. Same domain, adjacent in every search — and a real
  technical collision, since two crates cannot both offer `use lumina_core::`
  in one dependency graph. Publishing as `luminafx-core` while keeping the
  `lumina_core` library target would have shipped that collision rather than
  avoided it, so the library targets moved too.
- Checked all three registries before proposing anything. `lumina-engine` was
  free on crates.io but taken on npm *and* PyPI; **`luminafx`** was the only
  short candidate free on all three. Owner chose it. ADR-0014.
- **Second blocker, found by the same check:** internal dependencies were
  path-only, and `cargo publish` rejects a dependency with no `version`. Every
  one of them now carries `version = "0.4.0"`.
- **Third, nearly shipped silently:** a binary's name defaults to its package
  name, so renaming `lumina-cli` to `luminafx-cli` would have renamed the
  command every doc and example invokes. Now pinned in an explicit `[[bin]]`
  with the reason written beside it.
- Nothing else moves: repository, docs, book, scene format, and source
  directories all stay `lumina`. `luminafx` is a registry identity.
- `luminafx-schema` dry-run publishes clean. The rest cannot be dry-run until
  their dependencies exist on the registry — expected, and why `release.yml`
  publishes in dependency order and skips versions already up, so a partial
  release is resumable rather than stuck.
- Token is stored as the `CARGO_REGISTRY_TOKEN` repository secret. **Nothing
  has been published**; the first real publish is the owner's call, since it
  cannot be undone, only yanked.

## 2026-09-03 (last) — EXR, and what it honestly buys

- `AAA-OUT-07`. `--format exr` writes an OpenEXR sequence in linear light with
  associated alpha. Zero new dependencies: `exr` was already in the tree via
  `image`'s default features, compiled and unused.
- **Stated plainly in the rustdoc and pinned by a test: it adds no precision.**
  The rasteriser has one pixel type, 8-bit sRGB (the same wall that blocks
  `AAA-OUT-01`), so every value in an EXR frame is one of 256 decoded levels.
  A test asserts exactly that, so the float channels cannot be mistaken for a
  claim of float rendering — and so it fails the day a deeper buffer lands,
  which is when the docs need rewriting.
- What it does buy: nothing is lost *after* this point. No second quantisation,
  no guessed gamma, and alpha in the convention OpenEXR specifies.
- That alpha convention forced a real change. EXR wants *associated*
  (premultiplied) alpha, which is the renderer's own convention — so
  `render_blurred` now takes an `AlphaMode` rather than always converting.
  Round-tripping through straight alpha would have divided by an 8-bit alpha
  and amplified quantisation savagely at low coverage.
- The tests are about the values, not the file: mid-grey must land at 0.216 and
  not 0.502 (linear, not sRGB reinterpreted as float), and must land *below*
  the sRGB value, since applying the transfer function backwards also produces
  a plausible image.
- Tests 275 → 280. Wave 4 complete.

## 2026-09-03 (later) — Text stops snapping, and TD-18 closes

- `AAA-OUT-09` and `AAA-OUT-10`, which turned out to be one fix.
- **Measured first.** Sweeping a two-glyph run across a single pixel produced
  **four** distinct frames: `Pixmap::draw_pixmap` snaps a translation to whole
  pixels whatever the filter quality (setting `FilterQuality::Bilinear` changed
  not one byte — checked). So each glyph jumped a whole pixel at a time, at its
  own moment, which is worse than a uniform snap: the spacing between letters
  wobbled while the word moved.
- The sub-pixel remainder is baked into the glyph coverage; only the whole-pixel
  part reaches the transform, which is all the compositor would have honoured.
  Ten positions inside a pixel now give ten distinct frames, and the centroid
  travels 0.9px when x does.
- That same change closed **TD-18**. Layout had been written twice and the GPU
  backend resampled a whole-string bitmap — glyphs sampled twice against the CPU
  backend's once. Layout is now `common::text`, the GPU backend draws one image
  per glyph, and both composite the identical bitmap at the identical integer
  offset. `04_text` dropped from `TEXT_TOL` to `DEFAULT_TOL`; `TEXT_TOL`
  survives only for the combined fixture, tightened from 1.5% pixels / 1.5 mean
  to 0.15% / 0.8, the mean borrowed from `GRADIENT_TOL` because the fixture
  contains gradients rather than because it fits.
- Cost: text-heavy scenes are ~4% slower (`text_render/40x40` 3.61 → 3.76 ms).
  The gate allows 25%, and the trade is a genuine quality gain in both places.
- 84 lines of the old whole-string path deleted; the duplication gate now
  guards the glyph placement arithmetic too.
- Tests 272 → 275.

## 2026-09-03 — Audio

- `AAA-OUT-08`. `assets.audio`: a list of files with `start` (seconds, negative
  to begin part-way in) and linear `gain`. Mixed into every video format; PNG
  sequences and GIFs ignore it.
- Placed by declaration rather than by reference. Sound is a property of the
  scene, not of anything drawn in it, so there is no object to attach it to —
  which is also why `AudioAsset.id` exists only to name the track in
  diagnostics.
- **The exporter never sees the scene's path strings.** ffmpeg needs a
  filesystem path, not bytes, so an exporter that read `scene.assets.audio`
  itself would let any caller name any file — and one caller is an HTTP server
  taking scenes off the network. `Exporter::set_audio` takes already-resolved
  paths; the CLI resolves against the working directory, the server against
  `LUMINA_ASSET_ROOT`, rejecting anything outside it. A structural test asserts
  `render_blocking` routes every audio path through the check, and it was
  verified to fail with the check removed.
- Three ffmpeg details that each look right and are not:
  - `-shortest` alone truncates the video to a short track; `apad` alone
    stretches it to fill a long one. Both together give the animation's length
    in either direction, and the tests check both directions.
  - `amix` normalises by input count unless told `normalize=0`, so declaring a
    second track would silently halve the first.
  - `adelay` without `all=1` delays only the first channel, turning a stereo
    track into one channel of silence against one of sound.
- Tests 265 → 272.

## 2026-09-02 (night, last) — A crate nothing lints is a crate nothing checks

- Clippy now covers `lumina-wasm`, in `cargo xtask ci` and in CI. It had been
  excluded in both (TD-24), and adding it immediately reported a live bug.
- `hit_test` sorted candidates by z-index with a **stable** sort over
  `HashMap` keys. Ties therefore resolved in map iteration order, which Rust
  randomises per process — the same click on the same scene could name
  different objects between runs. This is TD-25 exactly, present a second time
  because the WASM engine reimplemented the ordering instead of sharing it.
- Fixed to descending `(z_index, id)`, the precise reverse of the renderer's
  ascending draw order, so "tested first" and "drawn last" name one object.
  Group children had the same disagreement in a quieter form: ties were stable
  but in *author* order while the renderer draws them in id order, so a hit
  could report the object underneath the one visibly on top.
- TD-24 is now mostly closed. `cargo test` still excludes the crate, because
  those tests need the wasm target and run under `wasm-pack test --node`.
- The general lesson is TD-25's, restated: the ordering rule lives in
  `lumina-renderer::common::scene` and is crate-private, so every other
  consumer reimplements it. Sharing it is TD-21's blocker too.

## 2026-09-02 (night, latest) — Alpha output, and the bug under it

- `AAA-OUT-06`. `--format webm-alpha` (VP9, `yuva420p`) and `--format mov`
  (ProRes 4444, 10-bit 4:4:4 with 16-bit alpha).
- The flags were the easy half. **Frames were leaving the engine
  premultiplied** and PNG, ffmpeg's `rgba` input, and the WASM canvas all store
  straight alpha, so a half-opaque pure red was written as a dark red at half
  opacity. It had never been visible because at `a = 255` the two encodings are
  identical bytes and every background so far was opaque — the feature and the
  bug could only be found together.
- `render_frame` still returns premultiplied, documented on the trait. That is
  deliberate: motion blur is correct averaging premultiplied values and wrong
  averaging straight ones, and demultiplying at the renderer would force a
  lossy 8-bit round trip through every blurred frame. `demultiply_in_place` is
  called once per boundary instead — inside `render_blurred` after the average,
  and in the WASM `render_frame`.
- Two things the tests get right by accident of being written last:
  - The VP9 check is a **decode round trip**, not an `ffprobe` field. WebM
    reports `pix_fmt=yuv420p` for a file with a full alpha plane and signals
    the plane in an `alpha_mode` tag, so the obvious probe reports "no alpha"
    on a correct file. The first version of the test failed for that reason.
  - "Still red" is asserted as channel dominance. A saturated primary does not
    survive an RGB → BT.709 → RGB round trip exactly (ProRes returns green 25
    where the source had 0), and pinning absolute values would test ffmpeg's
    rounding rather than ours.
- An opaque scene is byte-for-byte what it was; a test asserts it.
- Tests 259 → 265.
- **The benchmark gate cried wolf and was fixed rather than ignored.** It
  failed this branch on `timeline_eval/2000 +52.9%` — for a function
  (`get_state_at`) byte-identical to `main`, on a bench that never touches the
  camera. One run on a shared runner cannot separate that from a real
  regression by threshold alone, so a regression must now corroborate across
  the sizes of its own family: shared code slows at every size, a bad
  neighbour on the host moves one. Uncorroborated ones are still printed
  loudly. Verified against three cases — noise, a genuine across-the-board
  regression, and a single-member family, which is judged on the threshold as
  before.

## 2026-09-02 (night, later) — The camera can turn

- `AAA-MOT-05`. `camera.timeline[].state.rotation`, degrees about the canvas
  centre, positive clockwise to match `GroupProps.rotation`.
- Free on both backends: `common::scene::camera_transform` builds the matrix
  once and each backend concatenates it, so there was one place to change and
  no way for them to disagree. Parity fixture 20 asserts that anyway — the
  claim is cheap to make and cheap to check.
- **Interpolated as a plain angle, not by shortest arc.** Shortest-arc is the
  usual choice and it is wrong here: `0 → 350` would turn 10 degrees backwards,
  reversing the direction the author stated, and a full revolution would become
  inexpressible.
- **Zero is skipped, not composed.** The field is `#[serde(default)]`, so every
  camera ever written now carries `rotation: 0.0`. A test asserts the rendered
  bytes are *identical* with the field absent and with it explicitly zero —
  concatenating an approximate identity would have drifted every existing
  golden image by a fraction of a pixel.
- Found while wiring it: a non-finite camera component blanked the whole video
  silently, since the camera transform reaches every object. Now
  `CAMERA_STATE_NOT_FINITE`.
- Tests 252 → 259.

## 2026-09-02 (night) — Morphing flows instead of collapsing

- `AAA-MOT-04`. Interpolating a point list to one of a different length padded
  the shorter with copies of its last element. Correct as a definition of
  "same length", wrong as motion: a four-point square becoming a sixty-four
  point circle mapped sixty-one of the circle's vertices onto one corner, so
  the square collapsed into that corner and unfolded out of it.
- Both outlines are resampled at even arc-length spacing around their closed
  perimeters, so each vertex has a comparable distance to travel and the shape
  deforms rather than folds.
- Two deliberate limits, both cheaper to solve where they arise:
  - **Equal lengths are left alone.** Matching counts are how an author says
    "vertex *i* becomes vertex *i*"; resampling would silently override that.
  - **No rotational alignment.** Finding the best starting offset is O(n²) per
    property per frame, and the shapes it fixes are ones the author can rotate
    once at authoring time.
- `as_point_list` is strict on purpose — every element a two-number array — so
  gradient stops (`[[0.0, "#hex"], …]`) and Bézier parameter arrays keep the
  padding path, where "arc length" would be meaningless.
- Tests assert the property, not the mechanism: no cluster of vertices may
  coincide at the midpoint. The padding implementation fails that and passes an
  endpoints check.

## 2026-09-02 (evening, later) — `draw_fraction` finally means one thing

- `AAA-MOT-02`. It meant three different things depending on the object:
  - **`Line`**: a dash pattern. Exact for a straight line; for anything else
    the dash phase is measured by the rasteriser's own flattening, so the
    answer depended on which backend was drawing.
  - **`BezierCurve`**: a cut at curve *parameter*. Exact arithmetic measuring
    the wrong quantity — a cubic traversed at uniform `t` does not move at
    uniform speed, so a steady `draw_fraction` produced a reveal that visibly
    accelerated and slowed along the curve.
  - **`Path`**: nothing at all. The field is in `PathProps` and the renderer
    never read it, so a Path with a reveal animation simply appeared whole.
    Same class as `LineProps.dash` — a schema promise with no implementation
    behind it.
- All three now trim by **arc length** through one shared helper in
  `common/path.rs`: flatten to a polyline with adaptive subdivision, accumulate
  distance, cut where the fraction lands. At 0.5, half the ink is drawn.
- Multi-subpath shapes measure across all subpaths, so two strokes reveal one
  after the other as a single drawing rather than racing each other at
  independent rates.
- Parity fixture 19 covers curve and path reveals on both backends; 20 fixtures
  now. Tests 241 → 247.
- The test that matters asserts *proportionality*, not endpoints: eight equal
  steps of the fraction must each add an eighth of the length, on a curve whose
  control points are deliberately bunched at one end. Parameter-space cutting
  passes an endpoints check and fails that one.

## 2026-09-02 (evening) — Motion blur

- `AAA-MOT-06`. `canvas.motion_blur_samples` and `canvas.shutter`; each frame
  is rendered several times across the shutter interval and averaged.
- Cheap here for a specific reason: particles are analytic and the timeline is
  pure, so any instant can be evaluated in isolation. Temporal supersampling is
  just several renders averaged — no per-object velocity, no reprojection.
- Three decisions worth recording:
  - **The shutter is centred on the frame's instant**, not opened at it, so a
    blurred object straddles where the timeline says it is rather than trailing
    behind. A test asserts the centroid does not shift.
  - **Averaging is on premultiplied values.** That is what `render_frame`
    returns and what makes the average correct — averaging straight-alpha
    colours weights a nearly-transparent sample as heavily as an opaque one and
    haloes every moving edge.
  - **Rounding, not truncation.** Truncating biases every channel down, which
    dims a blurred frame relative to a sharp one. A test compares mean
    luminance for exactly that.
- Off by default and bit-identical when off, which is also tested — a default
  that changed existing renders would be a far bigger deal than the feature.
- Bounded at validation like every other per-frame cost (`MAX_MOTION_BLUR_SAMPLES`
  = 64): it multiplies the render count on top of the frame count.
- Tests 236 → 241.

## 2026-09-02 (later, still) — Gradients blend perceptually; linear light is blocked

- **`AAA-OUT-01` (linear-light compositing) cannot be done**, and the obstacle
  is earlier than the effort. `tiny-skia` exposes exactly one pixel type,
  `PremultipliedColorU8` — 8-bit, sRGB, premultiplied. No linear buffer, no f32
  surface, no blend-space option, so there is nothing to composite *into*. It
  would mean replacing the CPU rasteriser, which abandons "Skia defines the
  pixels" (DESIGN.md) and is a much larger decision than an output item.
  Recorded in the plan with the reasoning rather than quietly dropped: the
  programme proposed something the chosen dependency cannot express, and
  nothing but reading the pixel type would have shown it.
- **`AAA-OUT-02` was reachable and is done.** The timeline blended colours in
  `OKLab` while both rasterisers interpolated gradient stops linearly in sRGB —
  two colour models in one frame, so red to blue dipped through a dark muddy
  purple in a gradient and a bright one in a fade.
- Neither backend lets a caller choose an interpolation space, but both accept
  arbitrarily many stops. So the perceptual curve is sampled and the samples
  handed over as intermediate stops: the right answer through an API that
  cannot be asked for it directly. Eight segments per pair keeps the
  approximation error under one 8-bit step.
- The mixing function is **exposed from `lumina-core` and shared**, so a
  gradient and a fade agree by construction rather than by two implementations
  happening to match. `interpolate_value` now calls it too.
- Tests assert the actual claim — gradient midpoint equals fade midpoint across
  four transitions — plus that the author's own stops survive refinement, and
  that the result is *not* indistinguishable from a naive sRGB blend, which
  would silently pass if the refinement did nothing.

## 2026-09-02 (later, more) — Wave 4: two schema promises finally kept

- **`LineProps.dash` implemented** on both backends (TD-19 closed). It had sat
  in the schema since v0.1 with neither backend drawing it, so a dashed line
  rendered solid — the docs even claimed CPU support at one point. Both now
  read the pattern through one shared normaliser: odd-length patterns repeat to
  make them even (SVG and Canvas both do this, so `[5]` is five on, five off),
  and a malformed pattern draws solid rather than something the two backends
  might disagree about. `draw_fraction` still wins when both are set, because
  it reveals by dashing too and a line being revealed should reveal, not
  reveal-and-dash.
- Parity fixture 18 added — 19 fixtures now. It passed first time, which is the
  useful outcome: the shared normaliser means there is only one interpretation
  of a pattern to get wrong.
- **Camera keyframes accept `easing_params`** (`AAA-MOT-01`, #67). The field
  did not exist, so `cubic_bezier` and `spline` on a camera passed validation —
  both are registered names — and then animated linearly. The test asserts the
  camera follows the *same* curve an object property would with the same
  parameters, and a second test checks the midpoint specifically, because
  falling back to linear still hits both endpoints.
- Tests 224 → 232.

## 2026-09-02 (later) — Wave 4 opens: output fidelity

- **Encode quality** (`AAA-OUT-03/04/05/13`). Every video now carries BT.709
  primaries, transfer, matrix and `tv` range — without them a player guesses,
  and players guess differently, so the same file looked different in
  QuickTime, VLC and Chrome. Plus `-tune animation` (x264 has a tune built for
  flat regions and hard edges), `+faststart` so a browser can play before the
  download finishes, and `-row-mt` for VP9.
- `--quality draft|standard|final`. Trades encoder effort and bit depth, never
  pixels. `final` is 10-bit. Verified from the produced file with `ffprobe`
  rather than from the arguments passed — ffmpeg is free to ignore a flag.
- **The SVG path parser was missing most of the grammar** (`AAA-OUT-11/12`).
  It handled `M L H V C Z` and silently ignored `Q S T A`, so curves from any
  vector editor were dropped. It also never implemented **repeated coordinate
  sets**, so `L 1 1 2 2` drew one line instead of two — which truncates
  essentially every real path. Rewritten with a proper lexer (numbers may run
  together: `M0 0-1-1` and `1.5.5` are legal), full command set, arcs converted
  via the SVG 1.1 endpoint-to-centre parameterisation, and errors that name the
  offending token and offset instead of discarding the shape.
- Found while testing that with a hand-written SVG: **`fill="none"` rendered as
  opaque white.** `parse_rgba8` falls back to 255 for anything unrecognised, so
  a typo and an SVG habit both produced a solid white shape silently. Now an
  `INVALID_COLOR` validation error with a specific suggestion for `"none"`.
  The whole corpus was checked first — every colour in it is a valid hex
  literal, so nothing breaks.
- That leaves a real gap: there is **no way to express "no fill"** at all. TD-26
  and #74, worth deciding alongside TD-19 (`LineProps.dash`) since both are the
  schema promising drawing behaviour the renderer does not provide.
- Tests 207 → 224.

## 2026-09-02 (end, final) — Wave 3 done: three items dropped, one RFC rejected

Wave 3 finishes with more items *not* done than done, and each refusal has a
number behind it.

- **`AAA-P-12`** done: particles reserve capacity. One line, per emitter per
  frame.
- **`AAA-P-07`** dropped. No scene uses `spline` and no benchmark exercises it,
  so there is nothing to measure and principle #5 forbids optimising on a
  hypothesis.
- **`AAA-P-08`** dropped after measuring. A new `latex_render` group put a
  LaTeX object at ~60 µs per frame including its glyphs; on a realistic
  one-formula scene that is **0.5% of export**. Not worth a cache and its
  invalidation risk. The benchmark stays so the decision can be revisited.
- **`AAA-P-10`/`AAA-P-11`** deferred, not skipped (#72). There is no Vello
  benchmark, so there is no baseline. Assuming the CPU result transfers would
  be the same guessing that measuring disproved twice this wave.
- **RFC-0001 (`render_into`) rejected — by its own measurement.** The proposal
  was written from a real number: the output copy is 0.394 ms at 1080p, ~70% of
  the fixed per-frame cost once the buffer is reused. Implementing it made
  export **slower** (9.82 s → 10.52 s).
  The reason was already in the RFC's Alternatives section, arguing against a
  borrowed slice: "the borrow would have to end before the next `render_frame`,
  which is exactly what a pipeline cannot promise." That applies to the
  proposal too, and was missed. Every current caller needs *owned* bytes — the
  export channel, `ImageBuffer::from_raw`, and the JavaScript boundary.
  Kept as a written "no", which is what `planning/RFCS/README.md` asks for. The
  first RFC in the repository, and it is a rejection.

## 2026-09-02 (end) — Wave 3 closes: export pipelined, TD-05 shut

- Export: MP4 **12.86 s → 9.82 s** (−24%), PNG **4.54 s → 3.04 s** (−33%) on a
  1 560-frame 1080p scene.
- TD-05's premise was wrong and the measurement said so before any code was
  written. Rendering was **2.33 s of the 12.86 s** — ffmpeg was the other 82%,
  and the two stages ran back to back rather than overlapping. So the win is
  *pipelining*, not N-way parallel rendering, which cannot beat the encoder.
  For PNG, where there is no encoder to wait on, compression genuinely does
  parallelise, and that is where `rayon` finally earns its place after being
  declared and unimported since v0.1.
- The whole determinism investigation started here: the parallel PNG path
  appeared to change pixels with queue depth. It did not. The **base** was
  non-deterministic, and finding that (see the previous entry) mattered far
  more than the speedup being chased. Re-tested on the fixed base: 8 runs one
  hash, and byte-identical to sequential across all 1 560 frames.
- Recorded because it nearly went the other way: the honest option at the time
  was to ship only the part that was verified and file the rest. Refusing to
  ship the unexplained half is what turned up the real bug.
- Also fixed en route: the glyph cache used `Rc`, quietly making the renderer
  `!Send` and blocking any future threading. `Arc` now, with a test asserting
  `Send` so it cannot regress silently again.

## 2026-09-02 (late, critical) — **Rendering was not deterministic.** Fixed.

- Found while investigating why a parallel-export experiment changed pixels.
  It turned out the *base* was unstable: **12 exports of `unit_circle.lsf` from
  released v0.4.0 produced two different results**, 7 one way and 5 the other,
  diverging from frame 991 onward.
- Cause: objects sharing a `z_index` were ordered by a **stable** sort over a
  `HashMap`. Rust randomises `HashMap` iteration order per process, so a stable
  sort faithfully preserved an order that differed between runs. Tied objects
  drew in a different sequence each run and, where they overlapped, produced
  different pixels. `unit_circle` has five z-index values with ties.
- Fix: sort on `(z_index, id)` — a total order over what the scene *contains*
  rather than over how it happens to be stored. Verified: 12 runs, one hash;
  the full 1 560-frame export byte-identical across processes.
- **No existing test could have caught this**, and that is the more important
  finding. The golden-pixel suite and the 18-fixture cross-backend parity suite
  both render inside a single process, where a map's iteration order is fixed
  for its lifetime. The divergence only exists *between* processes. Registered
  as TD-25; `tests/draw_order.rs` asserts the property directly by building the
  same scene with different insertion orders, which does catch it in-process.
- This contradicted VISION.md principle 2 ("Determinism is sacred") and
  ENGINEERING_PRINCIPLES #1 — the guarantee the project rests scrubbing,
  caching, golden-pixel testing and frame-parallel export on. It shipped in
  v0.4.0.
- Worth recording how it surfaced: only by comparing two full exports byte for
  byte. Every cheaper check — the suite, the parity fixtures, rendering a frame
  twice in one process — reported success.

## 2026-09-02 (late, end) — Wave 3: glyph cache, and an estimate that was 5x off

- Rasterised glyphs are cached per `(font index, character, exact size)`. The
  cache stores **coverage, not colour**, so one entry serves every colour a
  character is ever drawn in — which is what makes it worth having on a scene
  that fades text. Bounded, because an animated `font_size` produces a new key
  every frame.
- Measurement and drawing now read the same cache, so a string cannot measure
  one width and draw another.
- Result: `text_render` **−9.5%** and **−10.8%**, and `skia_render` −1.6% to
  −2.9%, which incidentally recovers the ~1% regression from the previous PR.
- **The plan overestimated this by about five times.** It predicted 2–3 ms on a
  text-heavy scene; the measured saving is ~0.45 ms. The estimate was written
  before buffer reuse removed the allocation that dominated everything, and it
  assumed outline rasterisation was the cost. It is not: the remaining per-glyph
  cost is the temporary `Pixmap` allocated for each glyph's mask plus the
  per-pixel colour conversion, both of which happen per glyph per frame whether
  or not the outline is cached. Recorded in METRICS; that is the next thing to
  attack in text, and it is a different change.
- Second time this wave a plan estimate has been wrong in a way only measuring
  could show — the first reordered the whole wave. The pattern is worth naming:
  estimates written before the dominant cost was removed are estimates of the
  wrong system.
- `lumina-text` has its first tests ever (TD-10 partly addressed): cached glyph
  identical to fresh, sizes not sharing entries, measurement agreeing with
  drawing, font reload invalidating, and the cache staying bounded.

## 2026-09-02 (late, last) — Wave 3: timeline evaluation 34–42% faster

- `get_state_at` built a nested `HashMap` and then rebuilt it into
  `serde_json::Map`s, walking and reallocating every property twice per frame.
  Built once now.
- Keyframe lookup is a binary search. The tracks were already sorted at
  construction and the scan ran once per property, per object, per frame
  (`AAA-P-05`). Camera lookup likewise. Boundary semantics kept exactly: the
  explicit clamps stay, so a keyframe's own value is still returned at its own
  time rather than interpolated toward the next — which matters for properties
  that snap rather than blend.
- `sorted_root_ids` borrows instead of cloning every id twice per frame.
- Measured: `timeline_eval` **−42% / −36% / −36% / −35%** at 100/500/1000/2000
  objects; `frame_sequence` a further **−9%** on top of buffer reuse, so about
  **−22%** from baseline on the most realistic measurement.
- **Not everything improved.** `skia_render/10` and `/100` came out ~1% slower
  (p = 0.00, so systematic rather than noise) — those scenes have no groups, so
  there was little cloning for the borrow change to remove. Recorded in METRICS
  rather than omitted; it is far below the gate and far below the win it came
  with, but reporting only the favourable half is how benchmark suites stop
  being believed.
- Found while rewriting `get_camera_at`: `CameraTimelineEntry` has no
  `easing_params` field at all, so a camera keyframe naming `cubic_bezier` or
  `spline` validates cleanly and then animates **linearly**. Filed as #67 with
  a comment at the call site, rather than fixed inside a performance change —
  it needs a defaulted schema field.

## 2026-09-02 (late, latest) — Wave 3: rendering is 3.5–9x faster

- `AAA-P-02` first half landed: the renderer keeps its frame buffer between
  renders instead of allocating a fresh 8.3 MB `Pixmap` every frame.
- Measured, on the baselines recorded yesterday: **-89%** on a 1080p frame with
  10 objects, **-86%** with 100, **-73%** with 500, **-77%** on text, **-80%**
  on a plot. All at p = 0.00. `frame_sequence` improves least (-13%) because it
  runs at 720p and its remaining cost is timeline evaluation plus the output
  copy — the next two targets.
- Output is byte-identical, and that is tested rather than assumed. Reuse is
  exactly the kind of change that fails as a faint ghost of a previous frame in
  one corner of a video months later, not as a crash. Four tests: a reused
  buffer against a fresh one, shrinking the frame, eight repeated renders, and
  the error path — a failed render must put the buffer back, or the next frame
  quietly pays the allocation the change exists to avoid.
- The GPU backend allocates a texture and staging buffer per frame too
  (`AAA-P-10`), and is deliberately **not** changed here. wgpu may pool
  internally; assuming the CPU result transfers would be the same guessing the
  baselines just disproved. It gets its own measurement.

## 2026-09-02 (late, later) — Wave 3 opens: baselines, and the plan was wrong

- Four new benchmark groups, because the existing three could not see the
  engine's largest costs: nothing drew a character, nothing plotted a function,
  and nothing rendered frames in sequence.
- **The first reading contradicted the plan.** `skia_render` costs 5.21 ms for
  ten objects and 6.54 ms for five hundred — nearly flat, so almost none of it
  is drawing. Measured directly: allocating an 8.3 MB `Pixmap` per frame costs
  **5.3 ms**, reusing one costs **0.57 ms**. The allocator hands the block back
  to the OS and every frame faults in fresh pages.
- So `AAA-P-02` (buffer reuse, ~4.7 ms on *every* scene) outranks `AAA-P-01`
  (glyph atlas, 2–3 ms on text-heavy scenes), which `plan/02-performance.md`
  had called the largest single win. The plan is corrected, with the numbers.
  This is what principle #5 is for — the plan was wrong about its own priority
  until something measured it.
- One false start worth recording: the first profile showed 1 object rendering
  7× *faster* than 0 objects. That was not a cost curve, it was warm-up — the
  first two allocations in a process reuse a warm free list. Repeating each
  measurement three times made it obvious. A single-pass measurement would have
  produced a confident, wrong conclusion.
- Benchmarks now run in CI (TD-14's remaining item) comparing a pull request
  against **its own merge base, on the same runner, in the same job**. A stored
  cross-job baseline would report double-digit swings on identical code.
- The gate is 25%, not the 5% the programme proposed: same-runner comparison
  removes most variance but not all, and a gate that fires on noise gets
  disabled. 25% is above the noise floor and far below the size of the
  regressions it exists to catch.

## 2026-09-02 (late) — Wave 2 closes: fuzzing, and why it is not only fuzzing

- Four `cargo-fuzz` targets over every parser that reads untrusted input:
  scene JSON (the server's front door), SVG path data, LaTeX, and plot
  expressions. `scene_json` continues past deserialisation into validation and
  timeline construction, because all of that runs before a renderer sees the
  scene — a panic there is reachable from the same request body.
- **They are not the main safety net, and that is the interesting part.**
  `libfuzzer` needs nightly plus sanitizer flags, so a fuzz target alone runs
  only when somebody remembers. On a repository that went quiet for seven
  weeks, that is never — the same failure mode that let two RUSTSEC advisories
  sit unnoticed.
- So the same entry points also get **corpus-driven tests that run on stable,
  in CI, on every commit**: unbalanced braces, truncated path commands, 200
  levels of nesting, a 10 000-deep group chain, keyframes that change a
  property's type mid-animation, degenerate and infinite plot ranges, and every
  registered easing driven end to end through a real scene.
- Those tests assert bounds, not just absence of panics: a transliterator must
  not turn a bounded input into an unbounded one, and namespace rewriting must
  stay proportional to its input. A quadratic blow-up is reachable straight
  from a scene file.
- `fuzz/` is a separate workspace, excluded from the root, so
  `cargo check --workspace` never tries to build libfuzzer.
- proptest regression seeds are committed and explicitly un-ignored. The two
  saved seeds are the ones that found the quantised spring.
- Tests 179 → 188. **Wave 2 complete** apart from `AAA-ACC-09` (gradient stops
  still interpolate in sRGB while the timeline uses OKLab), which belongs with
  the linear-light compositing work in Wave 4.

## 2026-09-02 (night, latest) — Wave 2: plot sampling shared and adaptive

- Plot sampling moved into `common/plot.rs`. Both backends had near-identical
  loops; deciding *where* to sample is a rendering decision, so it belongs in
  the shared layer (TD-02, principle #4). The duplication gate now guards it.
- Sampling is **adaptive**: refined by chord deviation where the curve bends,
  seeded uniformly first so high-frequency features cannot be missed. A pole
  splits the curve instead of drawing a false vertical, and the curve is
  bisected toward the break so it reaches its asymptote.
- The expression is parsed **once per plot** rather than once per sample per
  frame — 720 000 parses of one constant string over a minute of output.
  **TD-04 closed.**
- `asin`/`acos`/`atan` were broken: `str::replace("sin(", "math::sin(")` turned
  `asin(x)` into `amath::sin(x)`. Tokenised now. Mixing namespaced and bare
  calls in one expression also works — the old code abandoned normalisation
  entirely if `math::` appeared anywhere.
- Sampling is f64 throughout, narrowing only at the screen transform. A domain
  like `[0, 1e6]` previously lost resolution before the renderer saw it.
- `draw_fraction` scaled the *sample count*, so a Plot visibly changed
  resolution as it drew. It narrows the domain now and samples at full detail.
- Tick positions come from an index rather than `t += step`. The `+ 1e-4` guard
  on the old loop was the accumulated-error symptom. Shared and bounded, so an
  unvalidated caller cannot hang the renderer.
- One test was reframed rather than satisfied: "a straight line takes under 100
  points" was pinning the seed size, not a property. The real claim — a flat
  curve costs a fraction of its budget — is what it asserts now, and the
  adaptivity claim lives in the neighbouring wiggly-vs-flat test.
- All 18 parity fixtures still match after both backends changed.

## 2026-09-02 (night, later) — Wave 2: colour and interpolation

- Colour moved from CIELAB to **OKLab**. Both are perceptual; CIELAB's hue
  lines bend, so blue to white drifts purple at the midpoint, while OKLab was
  fitted so straight lines look straight. For an engine whose main job is
  watching one colour become another, that is the whole point.
- **Alpha was silently broken.** `#RGBA` and `#RRGGBBAA` did not parse, so a
  fade between two eight-digit colours *snapped* to the destination rather than
  blending. Alpha now blends linearly — it is a coverage fraction, not a
  perceptual quantity.
- The property that caught it passed first time for the wrong reason: it only
  asserted the result was nine characters, which a snap to the destination also
  satisfies. Asserting the midpoint differs from **both** endpoints is what
  actually distinguishes interpolation from a snap. Second false-positive test
  this wave — worth noticing as a pattern.
- Interpolation can no longer yield `null`. `Value::from` maps non-finite
  floats to `Value::Null`, so the property vanished from the state map and the
  renderer substituted its default.
- Root cause validated too: `1e39` parses as f64 and becomes `inf` as f32.
  New `NUMBER_NOT_REPRESENTABLE` error, checked recursively since point lists
  and gradient stops are arrays.
- No golden pixels moved: goldens render at endpoints, where colour
  round-trips exactly, and the parity suite compares two backends that share
  the interpolator. Fade *midpoints* do change, which is the intended effect.

## 2026-09-02 (night) — Wave 2 opens: easing accuracy, property tests first

- Property tests written **before** the fixes, as the plan requires, and they
  failed immediately: `spring` was quantised and did not pin its endpoint.
- The quantisation property went through two wrong formulations before a right
  one. Comparing adjacent inputs fails on a *converged tail* — a settled spring
  legitimately repeats f32 values near 1. Counting distinct outputs over 10 000
  samples is what actually separates a staircase from a curve, and it reported
  the old spring's level count exactly: **101**.
- `spring` replaced with the closed-form damped-harmonic solution — exact,
  continuous, O(1) instead of O(100) per property per frame, correct in all
  three damping regimes, and tunable via `easing_params`. It lands exactly on
  1; the old curve stopped at 1.0000196, leaving objects fractionally off.
- `ease_css` now calls the curve it documents. It claimed
  `cubic-bezier(0.25, 0.1, 0.25, 1.0)` and called `ease_in_out_sine`. The exact
  solver was already in the same file.
- `cubic_bezier` inversion: Newton–Raphson with bisection fallback.
- Two unchecked preconditions now validated — bezier x control points in
  `[0, 1]` (both solvers need monotonic x) and strictly increasing spline
  keypoint times. y control points outside `[0, 1]` stay legal; that is
  overshoot, and there is a test asserting it.
- `hash01` divided by `u32::MAX`, which f32 rounds up to 2^32, so it could
  return exactly 1.0 against a documented `[0, 1)`. Swept 2 million values
  across the domain to confirm the fix.
- Nothing in the repository uses `spring` or `ease`, so no golden pixels moved.
  Particle positions do shift; nothing asserts exact positions, which is itself
  a coverage gap worth noting.

## 2026-09-02 (evening) — Wave 1 complete: the lint floor and one gate command

- `#![forbid(unsafe_code)]` on all nine crate roots. The zero-unsafe metric
  stops being a grep — which had started returning a false positive from the
  word in a comment — and becomes a compiler guarantee that cannot be silenced
  by an `allow` further down.
- `[workspace.lints]` replaces `#![warn(missing_docs)]` copy-pasted into six
  crates. Adds `unwrap_used`/`expect_used`/`panic`, `unreachable_pub`,
  `rust_2018_idioms`, and a handful of clarity lints.
- That surfaced ~130 warnings. `clippy.toml` (`allow-*-in-tests`) accounted for
  most legitimately; `cargo clippy --fix` took the 45 uninlined format args and
  19 doc-backtick items; the rest were fixed by hand. Two suppressions remain,
  both local and justified in a comment, per principle #7.
- One real improvement fell out: `latex_to_unicode` mapped each character
  twice — once to test, once to unwrap. It now collects into `Option<Vec<_>>`,
  mapping once and short-circuiting, so the unwrap is gone.
- `cargo xtask ci` replaces the five-command gate. Defined once in Rust, so it
  cannot drift from CI the way a command list in a README does. `--fast` skips
  the slow steps; missing optional tools are reported as skipped, not failed.
- **CI now checks every example renders** (`cargo xtask examples`). Principle
  #12 has always said a broken example is a broken build; nothing checked it.
  It caught one immediately: `showcase_grand.lsf` and
  `showcase_neural_network.lsf` carried the **absolute path**
  `/home/horux/projects/lumina/examples/assets/lumina_node.svg`, baked in by
  `os.path.dirname(__file__)` in their generators. The two flagship showcase
  scenes had only ever rendered on one machine. Generators and scenes both
  fixed.
- The check draws one frame per scene rather than encoding whole videos: the
  showcase is 4 500 frames, and a gate slow enough to be skipped is not a gate.
  A single frame still proves the scene parses, validates, resolves every
  asset, and draws every object. `--full` encodes properly for a release check.
- That needed a new CLI flag, `--preview [SECONDS]`, and fixing what it was
  built on: preview rendering swallowed asset failures with
  `if let Ok(data) = … { let _ = … }`, so a missing font produced a frame with
  no text and no message — while the full render path hard-errored on the same
  input. One loader now serves both (`AAA-DX-08`).
- The wasm job also went red: `[workspace.lints]` reached `lumina-wasm` for the
  first time and found **8 undocumented public items** on the browser-facing
  API, including `LuminaEngine` itself. Documented; the underlying exclusion of
  that crate from workspace commands is TD-24.
- `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `.editorconfig` pinned.
  This matters because CI sets `RUSTFLAGS: -D warnings` globally: without a
  pinned toolchain, a new stable release introducing a lint turns the build red
  on a repository nobody has touched.

## 2026-09-02 (later still) — Wave 1: runtime and backend agreement

- `AAA-SEC-03`: `/render` now hands its work to `spawn_blocking`. It used to
  render every frame and then block on ffmpeg on a worker thread, so N
  concurrent renders starved the runtime and `/health` stopped answering.
- The regression test for it is **structural**, not timing-based, and that is
  deliberate: a timing test was written first and **passed against the unfixed
  handler**. Starving a runtime deterministically needs a render slow enough to
  make the suite slow; anything faster is a coin flip. A test that passes
  without the fix is worse than no test. The source assertion fails without the
  fix and passes with it — verified both ways — and follows the same technique
  as `duplication_gate.rs`.
- `AAA-SEC-05`: Vello's `draw_leaf` now returns `Result` and rejects the same
  malformed `Arrow` the CPU backend rejects. Previously Skia aborted the export
  and Vello silently skipped the object, so the same scene produced different
  output depending on `--backend`.
- New test category: **behavioural parity**. The pixel suite compares frames
  both backends produced, so when one errors and the other skips there is
  nothing to compare — the divergence was invisible to it by construction.
- `ArrowProps.from`/`to` are `[f32; 2]`, so serde guarantees two elements at
  parse time. A timeline keyframe is the only route by which malformed geometry
  reaches a renderer, because timeline state is untyped (TD-07). That is the
  fixture the new test uses.
- Tests 129 → 132.

## 2026-09-02 (later) — Wave 1: scenes are now bounded computations

- `AAA-SEC-01/02/04` landed. Every limit sits in `lumina_core::validation`,
  the one chokepoint server, CLI, and both SDKs already call, so none can be
  bypassed by reaching the renderer another way.
- Bounded: canvas dimension, fps, duration, **the product** `duration x fps`
  (each factor can be reasonable while the product is not), `sample_count`,
  `function_str` length, `Particles.count`, derived tick counts, and group
  nesting depth. Non-positive and non-finite tick steps rejected outright —
  `x_step: 0.0` produced `inf as i32`, saturating to `i32::MAX`.
- Group depth mattered most: a **straight** chain trips no cycle check, and
  8 MiB of JSON encodes ~150 000 levels. That overflowed the stack during
  *validation*, aborting the process before any render limit applied. Both
  renderers carry the limit independently since `lumina-renderer` is a public
  API callable without validating first.
- 12 adversarial fixtures added, each asserting a specific new error code.
  Tests 117 → 129. The whole scene corpus — 9 examples, 16 parity fixtures —
  still validates, so no false positives.
- Cycle detection also stopped allocating a `String` per node visited
  (`path.contains(&id.to_string())` was O(n²) allocations); Vello's scene walk
  gained a `WalkCtx` so recursion carries only what varies.
- New debt: TD-23, `ParticlesProps` emitter fields lack `#[serde(default)]`,
  contradicting the documented schema convention. Found writing the fixtures.

## 2026-09-02 — **v0.4.0 released**; the AAA programme opens

- The ten-PR stack landed. Merging it collapsed the base chain — `--delete-branch`
  removed each PR's parent, so several merged into their parent branch and the
  odd-numbered ones auto-closed. Recovered by opening #22 from the surviving,
  CI-green stack head; every individual commit is preserved, so the per-TD
  trail and `git bisect` both still work. Lesson recorded: never
  `--delete-branch` while merging a stack.
- `main` went from 31 to 78 commits and now carries everything v0.4 promised:
  backend parity, the 16-fixture pixel-diff suite, the server safety minimum,
  the 3-OS matrix, and 416 documented public items.
- Repository made findable at last: description, homepage, and 20 topics set;
  Discussions on; the unused Wiki and Projects off; branch deletion on merge.
- METRICS refreshed with a v0.4.0 column, and one honest correction: the
  v0.3.0 "0 panicking calls in production" row was wrong. There are 2, both
  provably guarded. `AAA-CQ-02` replaces the grep with a lint.
- **`plan/`** opens the AAA programme: a master plan and fourteen dimension
  subplans, each citing `file:line` evidence. `planning/ROADMAP.md` remains
  the single schedule of record (ADR-0013).

## 2026-09-01 (later) — Fresh advisories triaged; mitex dropped

- Seven idle weeks produced two new RUSTSEC hits, both caught only because a
  PR happened to run — the case for a scheduled audit rather than a
  push-triggered one.
- RUSTSEC-2026-0235 reached the workspace through exactly one path, and that
  path was `mitex` — declared since v0.1, imported by nothing (TD-06). Removed
  rather than ignored: the vulnerability is gone, not suppressed, and the
  locked tree fell **428 → 386 crates**. Decision recorded as ADR-0012;
  TD-06 closed.
- RUSTSEC-2026-0206 (`rustybuzz`, via usvg/resvg 0.47) has no upgrade to take —
  resvg 0.47 is current. Ignored with reasoning and registered as TD-22, since
  it shapes text inside untrusted SVG assets.
- Also dropped a stale ignore: RUSTSEC-2025-0057 (fxhash) no longer matches
  any crate in the tree.
- `cargo deny check`: advisories ok, bans ok, licenses ok, sources ok.

## 2026-09-01 — First matrix run triaged; two latent bugs fixed

- The 3-OS matrix and the wasm suite had never run before this PR, and both
  failed on their first outing. Windows: DX12-WARP aborts the renderer test
  process (exit 2173, two tests never reporting) instead of returning an error
  the skip path can catch — probe now suppressed by `LUMINA_DISABLE_VELLO=1`,
  registered as TD-20.
- WASM: scenes were built with `serde_wasm_bindgen::to_value`, which emits a
  JS `Map`; `Scene` reads every field as absent from one, so all three tests
  died on `missing field version`. Tests now use `JSON.parse` — the path the
  JS SDK actually takes (`new LuminaEngine(scene as object)`).
- That exposed a real engine bug behind it: `hit_test` tested every object in
  world space and let a `Group` answer for its own children, so **nothing
  inside a group was ever clickable**. It now walks roots only, descends
  through groups, and returns the deepest object. Group scale/rotation still
  unapplied to the hit point (TD-21).
- Local gate green: fmt, clippy `-D warnings`, 117 native tests under both CI
  env configurations, 3 wasm tests, 16-fixture parity suite under
  `LUMINA_REQUIRE_VELLO=1`.

## 2026-07-13 (evening) — Dep bumps; v0.4 code work complete

- PR [#19](https://github.com/SakarZaidan/lumina/pull/19) (stacked on #18):
  resvg 0.47 + tiny-skia 0.12 — untrusted-SVG path off ttf-parser 0.21;
  fontdue (latest release) still pins it → TD-17 retargeted to v0.5 with
  TD-18. Parity suite = regression net; all green incl. wasm + deny.
- **v0.4 roadmap code items all done** (7 TD closed, 2 partial-by-design).
  Open PR stack: #10→#11→#12→#13→#14→#15→#16→#17→#18→#19, merge in order.
- Remaining before v0.4.0 release: merge stack, owner enables release-plz
  tokens (ADR-0011), CHANGELOG retitle, tag, METRICS refresh.

## 2026-07-13 (later still) — Rustdoc fill (TD-15)

- PR [#18](https://github.com/SakarZaidan/lumina/pull/18) (stacked on #17):
  416 undocumented public items filled across all six library crates;
  `#![warn(missing_docs)]` per crate + CI `-D warnings` = enforcement.
- lumina-schema fields now state units/semantics — docs.rs becomes a
  real authoring reference. Per-crate commits, dependency order.
- v0.4 code work remaining: dep bumps (TD-17). Then release prep.

## 2026-07-13 (later) — CI foundations (TD-14)

- PR [#17](https://github.com/SakarZaidan/lumina/pull/17) (stacked on #16):
  3-OS test matrix (require-Vello Linux-only for now), MSRV 1.88 job,
  concurrency-cancel, wasm-bindgen tests now actually run (Node).
- First-ever macOS/Windows runs — watch this PR's matrix legs for
  platform surprises; fixes land on the same branch.
- TD-14 remainder: criterion-in-CI (v0.5), release automation (owner).

## 2026-07-13 — Server safety minimum (TD-09 part 1)

- PR [#16](https://github.com/SakarZaidan/lumina/pull/16) (stacked on #15):
  `/render` asset paths confined to `LUMINA_ASSET_ROOT` (canonicalize →
  prefix check; was an arbitrary-file read), 8 MiB body cap (413),
  bind/serve/response no longer panic. SECURITY.md updated.
- Verified live with curl: escape → 400, 9 MB body → 413.
- v0.4 remaining: CI matrix (TD-14), rustdoc (TD-15), dep bumps (TD-17);
  release-plz blocked on owner.

## 2026-07-12 (night, latest) — Easing strictness; **WS-02 complete**

- PR [#15](https://github.com/SakarZaidan/lumina/pull/15) (stacked on #14):
  UNKNOWN_EASING validation error with did-you-mean suggestion; registry
  `EASING_NAMES` + drift-guard test; validation moved to
  `lumina_core::validation` (server re-exports, CLI + SDKs share it).
- CLI validates before every render; new `--check` flag (exit 1 on
  errors). Whole scene corpus passes; typo'd easing verified rejected.
- **WS-02 Done** — all four acceptance criteria met. v0.4 remaining:
  server safety (TD-09p1), CI matrix (TD-14), rustdoc (TD-15), dep bumps
  (TD-17); release-plz blocked on owner tokens.
- PR stack open for review/merge in order: #10 → #11 → #12 → #13 → #14 → #15.

## 2026-07-12 (night, later) — Vello shadows; TD-01 + TD-11 closed

- PR [#14](https://github.com/SakarZaidan/lumina/pull/14) (stacked on #13):
  GPU drop shadows composite the shared blurred silhouette (identical
  bytes to CPU) as a peniko::Image under an opacity layer. No vello/wgpu
  upgrade needed — WS-02 risk resolved negative.
- Fixture set now 16 scenes (adds shadows, plot/axes, SVG asset, combined
  showcase); suite caught + fixed a second real bug: vello axes ticks
  drawn at grid width (1.0) instead of axis width (2.0).
- **Backend parity table complete** — every feature row ✅ on both
  backends. WS-02 scope 1–3 done; only easing strictness (TD-08) remains.

## 2026-07-12 (night) — Vello gradients, rounded rects, dash (TD-01 pt 1)

- PR [#13](https://github.com/SakarZaidan/lumina/pull/13) (stacked on #12):
  GPU backend renders gradient fills+strokes (peniko brushes, shared bbox
  geometry), rx/ry rounded rects (shared quad-arc paths), and
  draw_fraction via kurbo dashes. Fixes gradients silently rendering
  solid white on vello.
- Fixtures 05/06/08/09 added; 12-fixture parity suite green.
- Book parity table updated; found + registered TD-19: `LineProps.dash`
  implemented by neither backend (docs had claimed CPU support).
- Remaining TD-01 gap: drop shadows → next PR closes TD-01 + TD-11.

## 2026-07-12 (later still) — Renderer common/ extraction (TD-02)

- PR [#12](https://github.com/SakarZaidan/lumina/pull/12) (stacked on #11):
  `lumina-renderer/src/common/` now owns color, SVG-path (`PathData`),
  z-order/root-sort, group+camera transforms (`Mat2x3`, f32, bit-identical
  per backend), shadow blur pipeline, fill/gradient resolution, dash.
- One extraction per commit; parity harness + full suite green after each.
- WS-02 scope 1 done; grep gate (`tests/duplication_gate.rs`) enforces it.
- Next: Vello gradients/rounded/dash (TD-01 pt 1) consuming common/.

## 2026-07-12 (later) — Pixel-diff parity harness live (TD-11)

- PR [#11](https://github.com/SakarZaidan/lumina/pull/11) (stacked on #10):
  cross-backend harness renders 8 fixtures on Skia + Vello, AA-aware
  comparator (3×3 neighborhood rescue + mean-delta tint check), failure
  artifacts to `target/parity-failures/` and CI upload.
- First real catch, fixed in-PR: Vello stroked with kurbo's round caps/joins
  vs Skia's butt/miter — all GPU line ends and sharp corners diverged.
- CI test job now installs lavapipe and sets `LUMINA_REQUIRE_VELLO=1`
  (missing adapter = failure, not silent skip).
- New debt TD-18: duplicated text layout paths (Skia inline vs raster.rs
  bitmap); text fixture carries a wider tolerance until unified (v0.5).
- WS-02 → In progress. Next: `common/` extraction (TD-02).

## 2026-07-12 — v0.4 kickoff: bundled OFL font (TD-16)

- v0.4 execution started per ROADMAP/WS-02; PR sequence planned A–J
  (font → parity harness → common/ extraction → vello parity → easing
  strictness → server safety → CI matrix → rustdoc → dep bumps).
- PR [#10](https://github.com/SakarZaidan/lumina/pull/10): Liberation Sans
  2.1.5 (SIL OFL 1.1) bundled at `examples/assets/fonts/`; all scenes/docs
  off `/usr/share/fonts`. Closes TD-16.
- Latent bug found + fixed in-PR: hello/circle_bounce/pythagorean declared no
  font asset — their text never rendered (no system-font fallback exists).
- Local gate green: fmt, clippy `-D warnings`, 92/92 tests, rustdoc, mdBook.

## 2026-07-08 (evening) — Constitution, RFC/ADR system, metrics, diagrams

- Added the constitution set at root: VISION.md, DESIGN.md,
  ENGINEERING_PRINCIPLES.md (linked from README and CONTRIBUTING).
- DECISIONS.md split into per-decision `planning/ADR/0001–0011`; DECISIONS.md
  is now the index. New `planning/RFCS/` process gates public-API changes.
- New `planning/METRICS.md` (measured v0.3.0 snapshot + quality scorecard)
  and the health dashboard above; both are release-checklist duties now.
- New `planning/ECOSYSTEM.md` (layers 0–3, what the core owes the ecosystem).
- `docs/architecture/`: gen-diagrams.sh renders the crate dependency graph
  from `cargo metadata` + 4 hand-maintained pipeline diagrams; embedded in
  the book's architecture chapter.

## 2026-07-08 (later) — v0.3.0 released

- PR #2 merged to `main` as `9c35474`; CI fully green (a fresh RUSTSEC batch
  was fixed in-flight: anyhow → 1.0.103, crossbeam-epoch → 0.9.20,
  ttf-parser unmaintained ignored + registered as TD-17).
- GitHub Pages enabled by the owner; book live at
  <https://sakarzaidan.github.io/lumina/> including the new Events chapter.
- Tags pushed: `v0.1.0` (02b92da), `v0.2.0` (596b847), `v0.3.0` (9c35474);
  GitHub Release v0.3.0 created with showcase media as assets (ADR-0010).
- WS-01 complete. Next up: WS-02 backend parity (v0.4) — see ROADMAP.

## 2026-07-08 — Repo audit, planning system, hygiene batch

- Full three-track engineering audit completed (core crates, tooling/CI, docs/git).
- This planning system created; `todo.md` retired into [ROADMAP.md](./ROADMAP.md);
  blueprint and history moved under `planning/`.
- Hygiene batch in flight on `feat/v0.3.0-enhancements`: metadata fixes, crate
  rustdoc, README/CHANGELOG repair, community health files, mdBook v0.3.0 refresh.
- Git state at session start: `origin/main` = `7af221a` (v0.3.0 merge, **CI red
  6/8** — fixed by the two `ci:` commits on this branch); no tags existed.
- Batch complete: 10 commits; local gate green (fmt, clippy `-D warnings`,
  92/92 tests, rustdoc clean, mdBook 0.4.40 builds).
- MSRV probed: **1.88** (1.78 and 1.85 fail on locked deps `home`/`image`);
  declared as `rust-version` and reflected in the README badge.
- Next: PR → green CI on `main` → tag `v0.1.0` (02b92da) / `v0.2.0` (596b847)
  backdated + `v0.3.0` on the green merge → GitHub Release for v0.3.0.
- Blocked on repo owner: enable GitHub Pages (Settings → Pages → Source:
  GitHub Actions) so the deploy-docs job can publish the book.
- Version: 0.3.0 across workspace and both SDK manifests (drift fixed).
