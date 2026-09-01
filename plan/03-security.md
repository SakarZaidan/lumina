# 03 — Security

## Current state

The library crates are genuinely defensive: zero `unsafe`, zero panics outside
tests, malformed input degrading rather than crashing. `resolve_asset_path`
(`server/src/lib.rs:209-227`) canonicalises before its prefix check, so it
resolves `..` and symlinks correctly, and it has dedicated tests. `SECURITY.md`
states the server's limitations rather than hiding them.

Against that, an audit found five reachable resource-exhaustion vectors and
two structural problems that the debt register did not track.

### Nothing bounds the *work* a request can request

`DefaultBodyLimit` caps input at 8 MiB (`server/src/lib.rs:326`).
`validate_scene_data` checks only that width and height are nonzero
(`validation.rs:171-183`). `Canvas` carries `width: u32, height: u32,
fps: u32, duration: f32` with no upper bound. From a sub-kilobyte body:

| Vector | Where | Effect |
|---|---|---|
| Frame count | `export/src/lib.rs:92` — `(duration * fps).ceil() as u32` | `duration: 1e9, fps: 240` runs the loop to `u32::MAX` |
| Pixmap size | `skia_backend.rs:988` | `65535 × 65535` attempts a 17 GB allocation, per frame |
| `sample_count` | `skia_backend.rs:803`, `vello_backend.rs:725` | Unbounded `u64`, evaluated per frame |
| `NumberLine` ticks | `skia_backend.rs:671-681` | `start: 0, end: 1e9, step: 1e-6` → 1e15 stroked paths per frame |
| `Axes` tick count | `skia_backend.rs:748,772` | `x_step: 0.0` → `inf as i32` saturates to `i32::MAX` iterations |

### Unbounded recursion kills the process during *validation*

`detect_group_cycle` (`validation.rs:281-311`) is a recursive DFS. A linear
chain `g0 → g1 → … → gN` contains no cycle, so the `visited` set never trips;
depth is N. At roughly 55 JSON bytes per group entry, an 8 MiB body encodes
~150 000 levels. Stack overflow aborts the process — and it happens *before*
any render limit could apply. `draw_node` (`skia_backend.rs:249`) recurses on
groups with no depth limit either.

serde_json's own 128-level nesting cap does not help: group nesting is a flat
map of id references, not nested JSON.

### `/render` blocks the async runtime

`render_scene` is an `async fn` (`server/src/lib.rs:228`) that synchronously
renders every frame and then blocks on `child.wait()` for ffmpeg
(`export/src/lib.rs:160`). There is no `spawn_blocking` or `block_in_place`
anywhere in the workspace. On the default multi-threaded runtime, as many
concurrent renders as there are cores starve every worker, and `/health`
stops answering.

### Two smaller items

`hash01` (`raster.rs:184`) can return exactly `1.0` — `u32::MAX as f32` rounds
up — despite documenting `[0,1)`. And the CLI and Python SDK deliberately do
not apply the asset-root restriction, which is defensible for local tools but
means rendering an untrusted `.lsf` locally feeds arbitrary paths to the
font and image parsers.

### The evalexpr surface is smaller than it looks

`function_str` flows from request JSON into `eval_number_with_context`
(`skia_backend.rs:846`). Checked against the lockfile: `evalexpr = "11"` is
declared with no features, and in 11.3.1 both `regex_support` and `rand` are
off by default. So there is no ReDoS vector and determinism holds. What
remains is an unbounded expression string parsed by a recursive-descent
parser, re-parsed once per sample per frame — bounded by `AAA-SEC-06` and
`AAA-P-09` together.

## Target

A scene is a bounded computation before it is a picture. Every limit is
enforced at one chokepoint, returns a structured error, and has an adversarial
fixture in the test suite. The server is deployable on a network you do not
control.

## Work items

| ID | Item | Acceptance |
|---|---|---|
| `AAA-SEC-01` | Resource bounds in `validate_scene_data`: canvas ≤ 16384², fps ≤ 240, frame count, `sample_count`, `Particles.count`, derived tick counts | Five adversarial fixtures return 4xx in bounded time |
| `AAA-SEC-02` | Depth limits on `detect_group_cycle` and `draw_node` | A 150 000-deep chain returns an error, not a stack overflow |
| `AAA-SEC-03` | `spawn_blocking` around the render in `render_scene` | `/health` answers while N renders are in flight |
| `AAA-SEC-04` | Reject non-positive `x_step`/`y_step` before the `as i32` | `x_step: 0.0` is a validation error on both backends |
| `AAA-SEC-05` | Align backend behaviour on malformed input (Skia errors, Vello skips) | A behavioural parity test asserts both return `Err` |
| `AAA-SEC-06` | Bound expression length and parse once per plot | A 1 MiB `function_str` is rejected at validation |
| `AAA-SEC-07` | `#![forbid(unsafe_code)]` in all nine crates | The zero-unsafe metric becomes compiler-enforced |
| `AAA-SEC-08` | Server hardening part 2 (TD-09): auth, rate limiting, CORS allowlist, configurable bind, structured logging, request ids | Threat model written; `SECURITY.md` updated to match |
| `AAA-SEC-09` | Fuzz targets on `parse_svg_path`, `latex_to_unicode`, `Scene` deserialisation | Running in CI on a time budget |
| `AAA-SEC-10` | Scheduled advisory scan, not push-triggered | A RUSTSEC advisory published on a quiet week opens an issue |
| `AAA-SEC-11` | SBOM, SLSA provenance, signed tags, SHA-pinned actions, CodeQL, OpenSSF Scorecard | Scorecard badge in the README |

`AAA-SEC-10` is not hypothetical. Two advisories — RUSTSEC-2026-0235 and
RUSTSEC-2026-0206 — landed during seven quiet weeks and were noticed only
because a PR happened to run. One of them was a real vulnerability reaching
the workspace through `mitex`, a dependency nothing imported (ADR-0012).

## Metrics moved

Security (55 → 96). It is the lowest score on the board and the one with the
clearest path.

## Sequencing

`AAA-SEC-01..07` in Wave 1 — they are small, they are reachable today, and
several are one-line. `08`, `10`, `11` in Wave 5 with the rest of the server
and supply-chain work. `09` in Wave 2 alongside the property tests, which
share the same generators.
