# Changelog

All notable changes to Lumina are documented here.  
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).  
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

Planned work is tracked in [planning/ROADMAP.md](planning/ROADMAP.md), and the
programme to reach reference quality in [plan/](plan/).

## [0.5.0] — 2026-09-03

The first release published to a registry, and the release that had to change
its own name to get there: `lumina`, `lumina-core`, and `lumina-cli` were all
taken on crates.io — `lumina-core` by an unrelated wgpu renderer. See ADR-0014.

Behaviour changed in ways existing scenes will notice: `ease` and `spring` are
different curves, colour interpolates in OKLab, `draw_fraction` means one thing
rather than three, and the server's defaults are closed rather than open.

### Changed
- **`spring` is now solved rather than integrated.** It was 100 fixed steps of
  semi-implicit Euler indexed by `(t / dt).round()` — which made it *quantised*
  to 101 distinct output values, resolution-dependent, and O(100) per property
  per frame, while its documentation claimed RK4. It is now the closed-form
  damped-harmonic solution: exact, continuous, O(1), and correct in all three
  damping regimes. It also lands exactly on 1, where the old curve stopped at
  1.0000196 and left animated objects fractionally off their keyframe.
- **`spring` is tunable** via `easing_params`: `stiffness`, `damping`, `mass`,
  any subset, defaulting to the previous behaviour.
- **`ease` now is the CSS `ease` curve.** It was documented as
  `cubic-bezier(0.25, 0.1, 0.25, 1.0)` and implemented as `ease_in_out_sine`, a
  visibly different curve. Scenes using `ease` will animate slightly
  differently — correctly.
- `cubic_bezier` inversion uses Newton–Raphson with a bisection fallback,
  converging in a handful of iterations rather than a fixed 32.
- **Particle positions shift slightly.** `hash01` divided by `u32::MAX`, which
  f32 rounds *up* to 2^32, so large inputs produced exactly `1.0` despite the
  documented `[0, 1)` range. It now divides by 2^32 using the top 24 bits.
- **Colours interpolate in OKLab** rather than CIELAB. Both are perceptual, but
  CIELAB's hue lines bend — blue to white drifts purple through the midpoint —
  while OKLab was fitted so straight lines look straight. Endpoints are
  unchanged; midpoints of a fade will look different, and better.
- **Colour interpolation is alpha-aware.** `#RGBA` and `#RRGGBBAA` previously
  failed to parse, so a fade between two eight-digit colours *snapped* to the
  destination instead of blending. Alpha blends linearly, which is correct — it
  is a coverage fraction, not a perceptual quantity. A colour written without
  alpha still comes back without alpha.
- Interpolation never yields `null`. `Value::from` maps non-finite floats to
  `Value::Null`, which made the property disappear from the state map so the
  renderer silently used its default.
- **Shapes morph between different vertex counts by arc length.** Animating a
  point list to one of a different length padded the shorter list by repeating
  its last element, so a four-point square becoming a sixty-four-point circle
  mapped sixty-one of the circle's vertices onto one corner: the square
  collapsed into that corner and unfolded, rather than flowing. Both outlines
  are now resampled at even spacing around their perimeters, so every vertex
  travels a comparable distance.

  Lists of equal length are untouched — matching counts mean the author
  intended vertex *i* to become vertex *i*, and resampling would override that
  silently. Arrays that are not point lists (gradient stops, Bézier control
  parameters) still pad, because resampling them by "arc length" would be
  meaningless.
- **Plot sampling is shared by both backends** (`common/plot.rs`), so they draw
  the same curve from the same points rather than from two similar loops. The
  expression is parsed **once per plot** instead of once per sample per frame —
  720 000 parses of one constant string over a minute of 60 fps output (TD-04).
- **`asin`, `acos`, and `atan` now work in plot expressions.** Namespace
  rewriting used `str::replace("sin(", "math::sin(")`, which turned `asin(x)`
  into `amath::sin(x)` — not a function, so the plot silently drew nothing. It
  is tokenised now. Mixing `math::sin(x) + cos(x)` in one expression also
  works; previously the presence of `math::` anywhere disabled normalisation
  for the whole string.
- **Plot domains are sampled in `f64`.** f32 gives ~7 significant digits, so a
  domain like `[0, 1e6]` lost resolution before the renderer saw it.
- **`draw_fraction` on a Plot no longer changes the curve's resolution.** It
  scaled the sample count, so the curve visibly *resolved* as it drew; it now
  narrows the domain and samples at full detail throughout.
- **Tick positions are computed by index, not accumulated.** `t += step` drifts
  linearly in the tick count — the `+ 1e-4` guard on the loop was the symptom.
  Shared between backends and bounded, so an unvalidated caller cannot hang the
  renderer.

- **Hit-testing is deterministic.** The WASM engine sorted candidate objects by
  z-index with a *stable* sort over `HashMap` keys, so two overlapping objects
  sharing a z-index resolved to whichever the map happened to yield first —
  and Rust randomises that per process, so the same click on the same scene
  could report different objects between runs. Ties now break by id, in exactly
  the reverse of the renderer's draw order, so the object reported is the
  object visibly on top. Inside a group the ordering was stable but still
  disagreed with the renderer, which could name the object *underneath*.

  This is the defect fixed in the renderer as TD-25, present a second time
  because the ordering was reimplemented rather than shared. It survived
  because `lumina-wasm` was excluded from clippy; it is not any more, in both
  `cargo xtask ci` and CI.
- **Transparency leaves the engine undarkened.** Frames were handed to PNG,
  ffmpeg, and the WASM canvas with **premultiplied** alpha, and all three store
  straight alpha. A half-opaque pure red left the rasteriser as
  `(127, 0, 0, 127)` and was written as a *dark* red at half opacity rather
  than a bright one — every semi-transparent pixel darkened by its own alpha.

  It had never mattered: at `a = 255` the two encodings are the same bytes, and
  until now every background was opaque. Opaque output is unchanged, byte for
  byte. `Renderer::render_frame` still returns premultiplied values, which is
  what compositing and frame averaging need — motion blur is correct on them
  and wrong on straight alpha — and `demultiply_in_place` is called once at
  each boundary where pixels leave the engine.

### Added
- **Full SVG path grammar.** `Q`/`q`, `S`/`s`, `T`/`t` and elliptical arcs
  `A`/`a` are now understood; previously the parser handled `M L H V C Z` and
  silently ignored the rest, so curves from any vector editor were dropped
  without a word. Arcs are converted to cubic Béziers via the SVG 1.1
  endpoint-to-centre parameterisation.
- **Repeated coordinate sets.** `L 1 1 2 2 3 3` draws three lines, and a repeat
  after `M` becomes an implicit line, as the specification requires. Both are
  ubiquitous in real files and neither worked.
- Numbers may run together without separators — `M0 0-1-1` and `1.5.5` are
  legal SVG and both broke the previous whitespace-splitting lexer.
- **Path errors name the offending token and its offset** rather than
  discarding the whole shape.
- **Colour-space tagging on every video.** Output carries BT.709 primaries,
  transfer, matrix and `tv` range. Without them a player guesses, and players
  guess differently — the same file looked different in QuickTime, VLC and
  Chrome.
- **`--quality draft|standard|final`.** Trades encoder effort and bit depth,
  never pixels: `final` produces 10-bit output to keep banding out of
  gradients. `-tune animation` and `+faststart` are on by default, and VP9
  gains `-row-mt`.
- **`draw_fraction` now trims by arc length**, and means the same thing on
  every object. It previously meant three different things: a dash pattern on a
  `Line` (exact for a straight line, rasteriser-dependent otherwise), a cut at
  curve *parameter* on a `BezierCurve` (exact arithmetic measuring the wrong
  quantity — a cubic traversed at uniform `t` does not move at uniform speed,
  so the reveal accelerated and slowed while the fraction climbed steadily),
  and **nothing at all on a `Path`**, whose schema had the field and whose
  renderer never read it.

  At 0.5, half the ink is on the canvas. Multi-subpath shapes reveal as one
  continuous drawing rather than several racing each other. Both backends share
  one implementation, covered by parity fixture 19.
- **Text moves continuously instead of a pixel at a time.**
  `Pixmap::draw_pixmap` snaps a translation to whole pixels whatever the filter
  quality, so text had one position per pixel: a caption drifting across the
  screen jumped a full pixel at a time, and because each glyph crossed its own
  boundary at a different moment, the spacing between letters visibly wobbled
  as the word moved. Sweeping a two-glyph run across one pixel produced four
  distinct frames; it now produces a different frame at every step.

  The sub-pixel remainder is baked into the glyph coverage and only the
  whole-pixel part reaches the transform — which is all `draw_pixmap` would
  have honoured anyway. Text-heavy scenes render about 4% slower for it.
- **Both backends composite text identically.** Glyph layout was written twice
  and the GPU backend resampled a bitmap of the whole string, so glyphs were
  sampled twice where the CPU backend sampled once, and their anti-aliasing
  diverged (TD-18). Layout now lives in one place and the GPU backend draws one
  image per glyph, so both composite the same bitmap at the same offset. The
  text parity fixture runs at the default tolerance rather than one an order of
  magnitude wider.
- **The Python SDK publishes to PyPI** as `luminafx`: wheels for Linux, macOS
  and Windows plus an sdist fallback, built by maturin and uploaded by the
  release workflow.

  The module is `luminafx` too, not `lumina`. Module names are not
  registry-controlled, so two packages may both install one — and PyPI's
  `lumina` (an unrelated object-detection library) installs a top-level
  `Lumina`, which is a distinct name on Linux and the *same* name on macOS and
  Windows, where filesystems are case-insensitive. Shipping a collision that
  appears on two platforms out of three is worse than not shipping one.
- **An MCP server** (`lumina-mcp`), so any MCP-capable agent can drive the
  engine with no glue code: `lumina_objects` to learn the format,
  `lumina_validate` to check a scene, `lumina_patch` to repair it, and
  `lumina_render` to produce a file. Newline-delimited JSON-RPC on stdio — no
  port, no credentials.

  It answers in the **same error vocabulary as the HTTP server**, which is the
  point: an agent that has learned `SCHEMA_MISMATCH` through one door knows it
  in the other. `object_registry` moved into `lumina-core` so both front ends
  answer from one table rather than two that drift.

  `lumina_schema` takes an `objects` argument and returns only those
  definitions plus everything they reference transitively — a scoped schema
  with a dangling `$ref` is worse than a large one, because a model will either
  invent the missing type or refuse.

  `lumina_render` validates first and refuses an invalid scene, so a failure
  from it is a real rendering problem rather than a document the agent can
  repair. Output paths are confined to `LUMINA_ASSET_ROOT`: this tool runs on a
  developer's machine with their permissions, and "render to
  `../../.ssh/authorized_keys`" must not be something it can be talked into.
- **The server is hardened** (TD-09 part 2). Bearer authentication via
  `LUMINA_API_TOKEN`, per-client rate limiting via `LUMINA_RATE_LIMIT`, a CORS
  allowlist via `LUMINA_CORS_ORIGINS`, a configurable bind address, a request
  timeout, and graceful shutdown so an in-flight render is not killed
  mid-encode.

  Three defaults changed, and each was a way to be exposed without deciding to
  be:
  - **Binds `127.0.0.1`, not `0.0.0.0`.** Starting the server used to publish a
    render endpoint on every interface, so "I started the dev server" and "I
    offered this machine's CPU to the network" were indistinguishable.
  - **Cross-origin is closed unless opened.** It shipped as
    `CorsLayer::permissive()`, which let any website a developer happened to
    visit drive their local render server.
  - **Rate limited by default**, at 60 requests per client per minute.

  Tokens are compared without leaking their contents through timing.
- **One error envelope across the whole HTTP surface.** `/validate` answered
  with `code` / `path` / `message` / `fix_suggestion`; `/patch`, `/scene_patch`,
  and `/render` answered with bare strings. That gap matters more here than in
  most APIs — the surface exists so an agent can send a scene, read what is
  wrong, fix it and resend, and half the answers were prose it would have had
  to parse with a regex.

  Converting the handlers turned out not to be enough: `axum` rejects a
  malformed body *before* any handler runs, so the endpoint whose job is to
  explain what is wrong with your JSON answered the most common mistake in
  plain text. Every route now takes an extractor that answers with the same
  envelope, distinguishing `MALFORMED_JSON` from `SCHEMA_MISMATCH` from
  `MISSING_CONTENT_TYPE` — three different fixes that used to look alike.
- **The advisory scan runs on a schedule**, not only when someone opens a pull
  request. Two advisories landed while the v0.4 stack sat unmerged for seven
  weeks and surfaced only because that stack happened to run CI again; another
  quiet month and neither would have been noticed. A failure now opens a
  labelled issue rather than only turning a badge red on a repository nobody is
  looking at, and reuses the open one instead of filing a duplicate every
  morning.
- **`osv-scanner` and `cargo-machete` run alongside `cargo-deny`**, and machete
  immediately found **twelve unused dependencies** across seven crates —
  including `glam`, which is now gone from the dependency graph entirely. An
  unused-dependency check is an advisory check by another route: `mitex`, which
  carried RUSTSEC-2026-0235 into this workspace, was imported by nothing.
- **Every GitHub Action is pinned to a commit SHA**, so a compromised or
  retagged action cannot silently change what CI runs. `dtolnay/rust-toolchain`
  derives its toolchain from the ref it is used at, so each use gained an
  explicit `toolchain:` at the same time — pinning it alone would have quietly
  changed which compiler builds the project.
- **CodeQL and OpenSSF Scorecard** run on a schedule. Scorecard is deliberately
  an outside opinion: the project grades itself in `planning/METRICS.md`, and a
  metric you grade yourself is one you can talk yourself into.
- **The crates are named `luminafx-*` on crates.io**, and a release workflow
  publishes them. `lumina`, `lumina-core`, and `lumina-cli` were all already
  taken — `lumina-core` by an unrelated *wgpu* renderer, which would have sat
  beside ours in every search and, worse, would have collided outright: two
  crates cannot both offer `use lumina_core::` in one dependency graph.
  `luminafx` is free on crates.io, npm, and PyPI. See ADR-0014.

  The project is still Lumina — repository, docs, book, and scene format are
  unchanged, and the command is still `lumina-cli`, now pinned in an explicit
  `[[bin]]` so the package rename could not silently rename it. Internal path
  dependencies also gained the `version` fields `cargo publish` requires and
  never had.
- **`--format exr`**: an OpenEXR frame sequence in linear light with associated
  (premultiplied) alpha — the intermediate a compositor wants, with a documented
  colour space so nothing downstream has to guess a transfer function or
  re-quantise on the way in.

  It does **not** add precision the renderer did not have. The CPU rasteriser
  has one pixel type, 8-bit sRGB, so these floats carry 8-bit values converted
  exactly; a test pins that, and will start failing the day a deeper buffer
  lands. What it buys is that nothing is lost *after* this point. No new
  dependency — `exr` was already in the tree through `image`'s defaults, unused.
- **Audio.** A scene declares `assets.audio` and video exports carry it, mixed
  and trimmed to the animation's length. Each track takes a `start` in seconds
  (negative begins part-way into the file) and a linear `gain`. MP4 gets AAC,
  WebM Opus, and ProRes MOV uncompressed PCM; PNG sequences and GIFs have
  nowhere to put sound and ignore it.

  Audio is a property of the scene rather than of anything drawn in it, so
  declaring the asset is what places it — there is no object to attach it to.

  Unlike fonts and images, an audio file is handed to ffmpeg as a *path* rather
  than as bytes, so `Exporter` never reads the scene's own path strings: the
  caller resolves them. The CLI resolves against the working directory and the
  server against `LUMINA_ASSET_ROOT`, rejecting anything outside it, so
  `/render` does not become an arbitrary-file read whose result the caller
  downloads.
- **Alpha output**: `--format webm-alpha` (VP9) and `--format mov` (ProRes
  4444, 10-bit 4:4:4). Give a scene a transparent `canvas.background` such as
  `"#00000000"` and it composites over other footage in an editor instead of
  arriving as a rectangle of backdrop.
- **Camera rotation.** `camera.timeline[].state.rotation`, in degrees about the
  canvas centre, positive clockwise, matching `GroupProps.rotation`. It
  interpolates as the angle written rather than by shortest arc, so `0 → 350`
  turns 350 degrees forwards rather than 10 backwards — otherwise a full
  revolution could not be expressed at all.

  The field defaults to `0` and a zero rotation is skipped rather than composed
  into the transform, so every camera authored before it existed renders the
  identical bytes it did before. Parity fixture 20 covers it on both backends.
- A camera state whose `x`, `y`, `zoom`, or `rotation` is not finite is now a
  validation error (`CAMERA_STATE_NOT_FINITE`). The camera is a root transform,
  so a single non-finite component rendered the *entire* video empty with no
  diagnostic anywhere. JSON cannot write `inf`, but a literal past f32's range
  becomes one on the way in.
- **Motion blur**, via `canvas.motion_blur_samples` and `canvas.shutter`. Each
  frame is rendered several times across the shutter interval and averaged, so
  anything moving smears the way a camera's shutter makes it. `shutter` defaults
  to `0.5` — a 180-degree shutter, the film convention.

  The shutter is centred on the frame's own instant rather than opening at it,
  so a blurred object straddles where the timeline says it is instead of
  trailing behind. Samples are taken at fixed offsets, so the result stays
  deterministic. Averaging is done on premultiplied values, which is what makes
  it correct: averaging straight-alpha colours weights a nearly-transparent
  sample as heavily as an opaque one and haloes every moving edge.

  Off by default (`1` sample), and bit-identical to the previous behaviour when
  off. Cost is linear in the rendering.
- **Gradients blend perceptually**, in the same space as keyframe fades. The
  timeline blended colours in `OKLab` while both rasterisers interpolated
  gradient stops linearly in sRGB, so the same two colours produced two
  different midpoints in a single frame — red to blue dipped through a dark
  muddy purple in a gradient and a bright one in a fade. Neither backend
  exposes an interpolation space, so the perceptual curve is sampled and the
  samples are handed over as intermediate stops. The author's own stops are
  preserved exactly; only the path between them changes.
- **`LineProps.dash` is implemented** on both backends (TD-19). The field had
  been in the schema since v0.1 and neither backend drew it, so a dashed line
  rendered solid. Odd-length patterns repeat to make them even, as SVG and
  Canvas do — `[5]` is five on, five off — and a malformed pattern draws solid
  rather than something the two backends might disagree about. Parity fixture
  18 keeps their dash phases in step.
- **Camera keyframes accept `easing_params`.** Without the field, a camera
  keyframe naming `cubic_bezier` or `spline` passed validation — both are
  registered easing names — and then animated **linearly**, because the
  parameterless lookup does not know them. Camera moves are the most visible
  motion in a scene. Defaulted, so existing scenes are unaffected.
- `INVALID_COLOR` validation error. An unparseable colour — `"red"`, a typo, or
  SVG's `"none"` — silently became **opaque white** in the renderer.

### Fixed
- **Rendering was not deterministic between runs.** Objects sharing a `z_index`
  were ordered by a *stable* sort over a `HashMap`, whose iteration order Rust
  randomises per process — so tied objects were drawn in a different sequence in
  different runs, and wherever they overlapped, the pixels differed. Twelve
  exports of `examples/unit_circle.lsf` produced two distinct results, 7 and 5.

  Draw order is now `(z_index, id)`, a total order over the scene's contents
  rather than over how they happen to be stored.

  This violated the project's most load-bearing guarantee — VISION.md principle
  2 and ENGINEERING_PRINCIPLES #1 — and no existing test could see it: the
  golden-pixel and cross-backend parity suites both render inside a single
  process, where a map's iteration order is fixed for its lifetime. The
  divergence only appears *between* processes. `tests/draw_order.rs` asserts
  the property directly instead, by building the same scene with different
  insertion orders.

### Performance
- **Rendering is 3.5–9× faster.** The renderer allocated a fresh frame buffer
  every frame. An 8.3 MB `Pixmap` at 1080p costs **5.3 ms** to allocate and
  drop, because the allocator returns a block that size to the operating system
  and every frame then faults in fresh pages; reusing one costs **0.57 ms**.

  | | before | after |
  |---|---|---|
  | 1080p frame, 10 objects | 5.21 ms | **0.569 ms** |
  | 1080p frame, 500 objects | 6.54 ms | **1.78 ms** |
  | 1080p frame, 1 600 glyphs | 8.31 ms | **4.15 ms** |
  | 1080p frame, 8 plots | 7.04 ms | **2.23 ms** |

  Output is byte-identical. The buffer is cleared to the background before
  anything is drawn — the same operation that always began a frame — and it is
  reallocated whenever the requested frame size changes.
- **Export is 24–33% faster.** Rendering and encoding used to take turns: the
  loop rendered a frame, wrote it to ffmpeg's stdin, and only then rendered the
  next, so a 1 560-frame 1080p scene spent 2.3 s rendering and ~6.6 s encoding
  and took 12.9 s. They overlap now, through a bounded queue that caps memory
  and lets rendering self-throttle to the encoder's rate. PNG compression runs
  on a rayon pool — the dependency has been declared since v0.1 and imported
  nowhere until now (TD-05).

  | | before | after |
  |---|---|---|
  | MP4, 1 560 frames at 1080p | 12.86 s | **9.82 s** |
  | PNG sequence, same | 4.54 s | **3.04 s** |

  Video export is bound by ffmpeg, not by rendering — rendering was 2.3 s of
  the original 12.9 s — so overlapping the stages is worth far more there than
  parallel rendering would be.
- **Text rendering is ~10% faster** on top of the above. Glyphs were rasterised
  from their outlines on every frame, and `font_for_char` walked every loaded
  font per character — twice, once to measure and once to draw. Rasterised
  glyphs are now cached per `(font, character, exact size)`; the cache stores
  coverage rather than colour, so one entry serves every colour a character is
  drawn in, and it is bounded so an animated `font_size` cannot grow it without
  limit. Measurement and drawing read the same cache, so they cannot disagree.
- **Timeline evaluation is 34–42% faster.** Per-frame state was built into a
  nested `HashMap` and then rebuilt into `serde_json::Map`s, walking and
  reallocating every property twice; it is built once now. Keyframe lookup is a
  binary search rather than a linear scan — the tracks were already sorted, and
  the scan ran once per property, per object, per frame. Camera keyframe lookup
  likewise.


### Added
- `NUMBER_NOT_REPRESENTABLE` validation error. `1e39` parses as f64 without
  complaint and becomes `inf` as f32 — the precision the engine renders with —
  so the property would vanish. Checked recursively, since point lists and
  gradient stops are arrays.
- **Fuzz targets** (`fuzz/`, `cargo-fuzz`) over every parser that reads
  untrusted input: scene JSON, SVG path data, LaTeX, and plot expressions.
- **Adversarial corpus tests** covering the same entry points on stable, so
  they run in CI on every commit rather than only when someone runs the fuzzer
  by hand — `libfuzzer` needs a nightly toolchain that CI does not have.
- **Property tests** over interpolation: finiteness, exact endpoints,
  boundedness, totality against mismatched types, and colour round-tripping.
- **Property tests** (`proptest`) over the easing registry: endpoint pinning,
  finiteness, boundedness, monotonicity, determinism, and a distinct-value
  count that distinguishes a real curve from a quantised approximation. The
  last of these is what caught the spring, reporting exactly 101 levels.
- Validation of easing solver **preconditions**, which the parameter-shape
  checks did not cover:
  - `INVALID_CUBIC_BEZIER` — x control points must lie in `[0, 1]`, since both
    solvers require `bezier_x` to be monotonic. y values may still leave the
    interval; that is how overshoot is expressed.
  - `UNSORTED_SPLINE_KEYPOINTS` — keypoint times must be strictly increasing.
    Unsorted input clamped a negative interval to `1e-9`, producing tangents
    around 1e9 and output that then read as `null`.

---

## [0.4.0] — 2026-09-02

**Correctness, backend parity, and foundations.** The two renderer backends
now provably produce the same pixels, verified by a 16-fixture pixel-diff
suite in CI; the project became contributable (3-OS matrix, MSRV job, every
public item documented) and publishable.

### Changed
- **Unknown easing names are now validation errors** (`UNKNOWN_EASING`, with a
  did-you-mean suggestion) instead of silently animating as `linear`; the CLI
  validates every scene before rendering and gains `--check`. Mildly breaking
  for scenes with typo'd easings — every name in
  `lumina_core::easing::EASING_NAMES` is accepted (TD-08, [#15]).
- **Scene validation moved to `lumina-core`** (`lumina_core::validation`);
  `lumina-server` re-exports it unchanged, and the CLI/SDKs share the same
  rules ([#15]).
- **Shared renderer `common/` module** — color parsing, SVG-path parsing
  (backend-neutral `PathData`), z-ordering, group/camera transform math
  (bit-identical `Mat2x3` on both backends), the drop-shadow blur pipeline,
  fill/gradient resolution, and dash geometry now have exactly one
  implementation consumed by both backends; a grep-gate test keeps it that
  way (TD-02, [#12]).

### Added
- **Vello drop shadows** — the GPU backend composites the same blurred
  silhouette bytes as the CPU backend via the shared blur pipeline; full
  visual feature parity between backends, verified by a 16-scene parity
  suite in CI (TD-01/TD-11 closed, [#14]). Also fixes axes tick marks
  rendering thinner on the GPU backend.
- **Vello gradients, rounded rectangles, and `draw_fraction` dash** — the GPU
  backend now renders linear/radial gradient fills *and strokes*, honors
  `rx`/`ry` corner radii, and reveals lines via the same dash pattern as the
  CPU backend, all through shared `common/` geometry. Fixes gradient fills
  silently rendering solid white on `--backend vello` (TD-01 part 1, [#13]).
- **Cross-backend pixel-diff harness** — every fixture scene in
  `crates/lumina-renderer/tests/fixtures/` renders on both the Skia (CPU) and
  Vello (GPU) backends and must agree within an AA-aware per-pixel tolerance;
  failures dump both frames plus a diff heat map, uploaded as CI artifacts.
  CI now requires a wgpu adapter (lavapipe) so Vello tests can never silently
  skip (TD-11, [#11]).
- **Bundled example font** — Liberation Sans 2.1.5 (Regular + Bold, SIL OFL 1.1)
  under `examples/assets/fonts/`; all example scenes, generator scripts, and
  docs now use repo-relative font paths, so examples render text on macOS and
  Windows too (TD-16, [#10]).

### Documentation
- Every public item across the six library crates is documented (416 items
  filled); `#![warn(missing_docs)]` + CI `-D warnings` make undocumented
  public API a build failure (TD-15, [#18]).

### CI
- Tests now run on ubuntu/macos/windows; new MSRV (1.88) check job;
  concurrent pushes cancel superseded runs; the wasm-bindgen test suite
  actually executes (it previously never ran in CI) (TD-14, [#17]).

### Dependencies
- resvg 0.42 → 0.47 and tiny-skia 0.11 → 0.12: the untrusted-SVG parsing
  path moves off unmaintained ttf-parser 0.21 (RUSTSEC-2026-0192); the sole
  remaining 0.21 consumer is fontdue (latest release), tracked in TD-17
  ([#19]).
- **`mitex` removed.** It had been declared since v0.1 and imported by no
  source file — LaTeX and MathML have always rendered through Unicode
  substitution. RUSTSEC-2026-0235 reached the workspace through that single
  unused path, so dropping the dependency eliminates the advisory rather than
  suppressing it, and takes the locked tree from **428 to 386 crates**
  (TD-06 closed, ADR-0012, [#22]).
- `rustybuzz` (RUSTSEC-2026-0206, via usvg → resvg 0.47) has no upgrade
  available and is ignored with reasoning as TD-22. A stale `fxhash` ignore
  was dropped — no crate in the tree matched it.

### Security
- `lumina-server` v0.4 safety minimum: `/render` asset paths are confined to
  `LUMINA_ASSET_ROOT` (was: arbitrary-file read), request bodies capped at
  8 MiB, and bind/serve/response no longer panic (TD-09 part 1, [#16]).

### Fixed
- **Hit-testing never reached inside a `Group`.** `hit_test` compared every
  object against a world-space point, so a child positioned in group-local
  coordinates could not match, and the `Group` then reported itself as hit —
  meaning no object inside a group could ever be the target of an event. It
  now walks root objects only, descends through groups, and returns the
  deepest object covering the point, with a recursion cap because the WASM
  engine accepts unvalidated scenes from its host. Group scale and rotation
  are still not applied to the hit point (TD-21, [#22]).
- The WASM test suite had never run before this release and failed on
  `missing field version`: scenes were built with
  `serde_wasm_bindgen::to_value`, which emits a JS `Map`, and `Scene` reads
  every field as absent from one. Tests now use `JSON.parse` — the path the
  JS SDK takes ([#22]).
- Windows CI aborted the renderer test process inside the Vello adapter probe
  (DX12-WARP terminates rather than returning an error). `VelloRenderer::new`
  gained a `LUMINA_DISABLE_VELLO` escape hatch, set for Windows only; parity
  remains verified on Linux under `LUMINA_REQUIRE_VELLO=1` (TD-20, [#22]).
- The Vello backend stroked shapes with round caps/joins (kurbo defaults)
  while the Skia backend uses butt caps and miter joins — GPU renders grew
  round nubs at line ends and rounded sharp corners ([#11]).
- `hello.lsf`, `circle_bounce.lsf`, and `pythagorean.lsf` declared no font
  asset, so their Text objects silently never rendered; they now use the
  bundled font ([#10]).

## [0.3.0] — 2026-06-01

### Added
- **Vello GPU parity** — Text, LaTeX, MathML, Image, SVG and Particles now render
  on the GPU backend. New crate-internal `raster` module rasterizes glyphs
  (fontdue), SVG (resvg) and images to straight-alpha RGBA, composited via
  `vello::Scene::draw_image`; particles fill GPU circles. Image/SVG opacity is
  honored through an alpha `push_layer`. `VelloRenderer` now implements
  `load_font` / `load_image` / `set_time`.
- **`spline` easing** — monotone-cubic (Fritsch–Carlson) interpolation through
  `easing_params.keypoints`, overshoot-free.
- **WebM (VP9) and GIF export** — `Exporter::export_webm` and `export_gif`
  (single-pass `palettegen`/`paletteuse`, Floyd–Steinberg dithering). Wired into
  the CLI (`--format webm|gif`) and the `/render` server endpoint (with the
  correct MIME type via the request `format` field).
- **Event system completion** — `jump_to_time` now actually seeks; new actions
  `play_from`, `pause`, `tween_to`, `show_tooltip`, `emit_custom`. `EventBus`
  owns a `PlaybackState` and returns `EventOutcome { actions, current_time,
  playing, emitted }`; `$drag.*` placeholders are substituted from the event
  payload.
- **Semantic ScenePatch ops** — `lumina_core::scene_patch` (`add_object`,
  `remove_object` with timeline/event/group cascade, `update_property`,
  add/remove/update keyframe, add/remove event, `update_canvas`) and a
  `POST /scene_patch` endpoint that applies a patch and re-validates.
- **Grand showcase** — `examples/gen_grand_showcase.py` →
  `examples/showcase_grand.lsf`, rendered on Vello to
  `media/showcase_grand.{mp4,gif,webm}`.

_The items below landed after 0.2.0 and were never released separately;
0.3.0 is their first shipped release:_

- **Image / SVG / animated-GIF compositing** — new `load_image` method on the
  `Renderer` trait; `SkiaRenderer` decodes PNG/JPEG (via `image` 0.25), SVG (via
  `usvg`/`resvg` 0.42, cached by `(asset_id, w, h)`), and animated GIF (frame
  selected by `current_time % total_duration`). Premultiplied alpha compositing
  matches tiny-skia's internal format.
- **Gradient fills** — `Paint` is now an untagged enum of `Solid(hex)` or
  `Gradient { type, stops, angle, radius }`. Linear and radial gradients on all
  closed shapes (Circle, Rectangle, Polygon, Path). Existing hex-string fills
  convert via `impl From<&str> for Paint` — no scene-file migration needed.
- **Drop shadows / glow** — optional `Shadow { color, blur, dx, dy, opacity }`
  on all closed shapes. Rendered as a box-blurred silhouette offset behind the
  shape. Zero overhead for shapes that omit the field.
- **Rounded rectangles** — `rx`/`ry` on `RectangleProps`; quadratic Bézier
  corner path when `rx > 0`; existing `fill_rect` fast path when `rx == 0`.
- **Text alignment + letter spacing** — `align: "left" | "center" | "right"` and
  `letter_spacing: f32` on `Text` and `LaTeX` props; `TextEngine::measure_width`
  computes the offset.
- **`set_time` on `Renderer` trait** — defaulted method propagates the current
  frame time to GIF frame selection without changing `render_frame` signatures.
- **`MathML` object type** — renders markup via unicode-substitution fallback;
  included in WASM `hit_test` and `get_z_index`.
- **`Particles` object type** — deterministic analytical particle simulation
  (`hash01` seed → position/alpha per particle per time). Reproducible across
  CLI, server, and WASM scrubbing. Full hit-test bbox.
- **`/objects` endpoint** — `GET /objects` returns the object-type registry:
  per type, its required, optional, and animatable property lists.
- **Python SDK** (`sdks/python/`) — standalone PyO3 0.22 + maturin workspace;
  exposes `lumina.validate(dict)`, `lumina.render(dict, path, format)`,
  `lumina.schema()`. Ships `from_anthropic.py` LLM loop example.
- **mdBook docs site** (`docs/`) — eight chapters (Introduction, Getting Started,
  Scene Format, Visual Effects, AI Integration, Architecture, Performance,
  Contributing); CI job builds and deploys to GitHub Pages on every `main` push.
- **Flagship showcase video** — `examples/showcase_neural_network.lsf` (2.5 min,
  1920×1080, 79 objects) generated by `examples/gen_neural_showcase.py`.
  Exercises every new visual feature.
- **WASM `hit_test` / `get_z_index`** extended to cover all 17 object types.

### Changed
- Workspace version bumped `0.2.0` → `0.3.0`.
- The CLI Vello backend now loads declared fonts and images (previously skipped),
  enabling GPU text/image rendering end to end.
- `EventBus::process_event` now takes `&mut self` and returns `EventOutcome`
  instead of `Vec<Action>`.

### Fixed
- Cleared all `lumina-wasm` clippy warnings (`get(0)` → `first()`, grouped
  bezier hit-test arguments).
- Image/SVG opacity is now respected on the Vello backend.

## [0.2.0] — 2026-05-10

### Added
- **`cubic_bezier` easing** — parameterised via `easing_params: [x1,y1,x2,y2]`
  on any timeline entry. Binary-search solver matches CSS cubic-bezier spec.
- **Path morphing** — arrays of unequal length now interpolate by padding the
  shorter list with its last vertex, enabling Polygon/Path morph animations.
- **LaTeX `draw_fraction`** — write-on animation for `LaTeX` objects, clipping
  rendered glyphs to the first `N × frac` characters.
- **Font fallback chain** — `TextEngine` walks all loaded fonts in load order
  to find a glyph when the preferred font lacks it.
- **Vello CLI backend** — `--backend vello` now works end-to-end via CPU
  software rasterisation (was previously unimplemented).
- **`--watch` mode** — `lumina-cli --watch` re-renders a preview PNG on every
  scene file change using `notify`.
- **`--verbose` flag** — prints per-render timing after export completes.
- **`/schema` endpoint** — `GET /schema` returns the LSF JSON Schema derived
  from Rust types via `schemars`.
- **`/patch` endpoint** — `POST /patch` applies RFC 6902 JSON Patch to a scene,
  re-validates, and returns the updated scene with validation results.
- **Complete WASM `hit_test`** — all 17 object types now tested: Circle (radius),
  Rectangle (bbox), Polygon (ray cast), Line/Arrow (segment distance),
  BezierCurve (sampled segments), Text/LaTeX/MathML (approx bbox), Image/SVG (bbox),
  Axes/NumberLine (bbox), Plot (axes bbox), Group (recursive children), Path (bbox),
  Particles (emitter bbox).
- **GitHub Actions CI** — `cargo fmt`, `clippy`, `test`, `doc`, WASM build on
  every push and pull request.
- **Benchmarks crate** (`crates/lumina-bench`) — criterion benches for timeline
  evaluation, Skia frame render, and easing dispatch.
- **`cargo deny` config** — licence allow-list, yanked-crate detection.
- **GitHub PR + issue templates**.
- **JavaScript SDK rewrite** — proper TypeScript API with `LuminaPlayer`,
  `useLumina` hook, vanilla `createPlayer`, and `tsup` build pipeline.
- **New examples** — `fourier_series.lsf`, `neural_net.lsf`, `dataviz_bars.lsf`.
- **`LICENSE`** — MIT licence file (was referenced but missing).
- **Server library surface** — `lumina-server` is now a proper crate with a
  `lib.rs` library and a thin binary, enabling reuse in tests and integrations.

### Changed
- `interpolate_value` now accepts `Option<&Value> easing_params`.
- `Keyframe` carries `easing_params: Option<Value>` for parameterised easings.
- `TextEngine::font_for_char` replaces the direct `HashMap::get` lookup,
  enabling transparent fallback to secondary fonts.
- `VelloRenderer::new()` is now synchronous (`pollster::block_on` internally),
  making it callable from non-async contexts.

---

## [0.1.0] — 2026-04-30

### Added
- Initial release: LSF schema, Skia CPU renderer, timeline evaluator, 15 object
  types, PNG/MP4 export, Axum HTTP server (`/validate`, `/render`), WASM runtime,
  `lumina-cli`.
- **Plot function rendering** (`Plot` object, `evalexpr` v11, `math::` namespace).
- **Camera/viewport system** — zoom + pan as a first-class animated property.
- **Axes** — full coordinate system with scale, tick marks, optional grid.
- **27 easing functions** — all CSS aliases, elastic, bounce, spring, smooth.
- **Draw-on animation** — `draw_fraction` on `Line`, `BezierCurve`, `Plot`, `Path`.
- **LAB colorspace interpolation** — hex color transitions via CIELAB pipeline.
- **Font rendering** — TTF fonts loaded from `assets.fonts`; glyph baseline fix.
- **LaTeX Unicode substitution** — `latex_to_unicode()` preprocessor.
- **Vello GPU backend** — first `VelloRenderer` with wgpu CPU software fallback.
- **Demo** — `examples/unit_circle.lsf` (52 s, 1080p, 30 fps).

[#10]: https://github.com/SakarZaidan/lumina/pull/10
[#11]: https://github.com/SakarZaidan/lumina/pull/11
[#12]: https://github.com/SakarZaidan/lumina/pull/12
[#13]: https://github.com/SakarZaidan/lumina/pull/13
[#14]: https://github.com/SakarZaidan/lumina/pull/14
[#15]: https://github.com/SakarZaidan/lumina/pull/15
[#16]: https://github.com/SakarZaidan/lumina/pull/16
[#17]: https://github.com/SakarZaidan/lumina/pull/17
[#18]: https://github.com/SakarZaidan/lumina/pull/18
[#19]: https://github.com/SakarZaidan/lumina/pull/19
[#22]: https://github.com/SakarZaidan/lumina/pull/22
[Unreleased]: https://github.com/SakarZaidan/lumina/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/SakarZaidan/lumina/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/SakarZaidan/lumina/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/SakarZaidan/lumina/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/SakarZaidan/lumina/releases/tag/v0.1.0
