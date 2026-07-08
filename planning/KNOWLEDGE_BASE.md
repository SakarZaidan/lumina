# Knowledge Base

Repo facts for contributors and agents. If a command or fact here goes stale,
fixing this file is part of the change that staled it.

## What Lumina is

A Rust animation engine: declarative JSON scenes (`.lsf`, "Lumina Scene
Format") in; deterministic, scrubbable animations out (MP4/WebM/GIF/PNG) via a
CPU backend (tiny-skia) and a GPU backend (vello/wgpu). Designed to be driven
by AI systems: JSON-Schema introspection, structured validation errors with
`fix_suggestion`s, semantic scene-patch ops, HTTP server, wasm player, Python
and JS SDKs.

## Crate map

Dependency chain: `lumina-schema` → `lumina-core` / `lumina-text` →
`lumina-renderer` → `lumina-export`. LOC/tests as of the 2026-07-08 audit.

| Crate | Purpose | ~LOC | Tests |
|---|---|---|---|
| `crates/lumina-schema` | Pure serde/schemars data model: `Scene`, 17-variant `Object` enum, `Paint`, timeline/event/camera types, all defaults | 595 | 0 |
| `crates/lumina-core` | Runtime: `SceneGraph`, keyframe `Timeline`, 28 easings (+`cubic_bezier`, `spline`), CIELAB interpolation, `EventBus`, semantic `scene_patch` | 2 460 | 51 |
| `crates/lumina-text` | `TextEngine` over fontdue: font loading, per-char fallback, measurement | 123 | 0 |
| `crates/lumina-renderer` | `Renderer` trait; `SkiaRenderer` (reference, full coverage) + `VelloRenderer` (GPU; parity gaps → TD-01); shared `raster.rs` (glyphs/particles) | 4 328 | 25 |
| `crates/lumina-export` | `Exporter<R>`: PNG sequence via `image`; MP4/WebM/GIF by piping RGBA to an **external `ffmpeg`** subprocess | 390 | 6 |
| `crates/lumina-server` | Axum REST: `/health /schema /objects /validate /patch /scene_patch /render`; strong structured validation | ~720 | 10 |
| `crates/lumina-wasm` | `LuminaEngine` via wasm-bindgen: render_frame, events, 17-object hit-testing (CPU renderer) | 570 | 3 (wasm) |
| `crates/lumina-bench` | Criterion benches: timeline_eval, skia_render, easing. `publish = false` | — | — |
| `tools/lumina-cli` | Clap CLI: scene → png/mp4/webm/gif, `--backend skia|vello`, `--watch` preview | ~215 | 0 |
| `sdks/python` | PyO3/maturin, in-process `validate`/`render`/`schema` (own workspace, excluded from `cargo test --workspace`) | — | 0 |
| `sdks/javascript` | React/vanilla player over the wasm engine. **Unbuildable until TD-12** | — | 0 |

## Commands (canonical)

```bash
cargo fmt --all --check
RUSTFLAGS="-D warnings" cargo clippy --workspace --exclude lumina-wasm --all-targets
cargo test --workspace --exclude lumina-wasm --exclude lumina-bench   # what CI runs
cargo test --workspace                  # full local run (needs ffmpeg; wasm crate needs wasm target)
RUSTFLAGS="-D warnings" cargo doc --workspace --no-deps
wasm-pack build crates/lumina-wasm --target web
wasm-pack test --node crates/lumina-wasm          # wasm tests (not yet in CI → TD-14)
cargo bench -p lumina-bench                        # manual; not in CI
mdbook build docs                                  # book output → docs/book/ (gitignored)
docs/scripts/gen-schema.sh                         # regenerates docs/src/generated/ schema
cargo run -p lumina-cli -- --scene examples/hello.lsf --output out --format mp4
```

## Environment

- **ffmpeg** on PATH — hard runtime requirement for MP4/WebM/GIF export and
  the export tests (they skip gracefully if absent).
- **wasm-pack** for the wasm crate; **mdBook v0.4.40** (pinned in CI) for the
  book; **maturin** for the Python SDK.
- CI: single workflow `.github/workflows/ci.yml` — fmt, clippy, test, docs,
  wasm build, cargo-deny, mdBook build, Pages deploy (main only).

## Conventions

- **Conventional Commits**; scopes = crate short names (`core`, `renderer`,
  `schema`, `server`, `wasm`, `export`, `text`, `cli`), `book`, `planning`,
  `sdk-js`, `sdk-py`, `ci`, `examples`. Branches: `feat/…`, `fix/…`, `docs/…`,
  `ci/…`. PRs merge with merge commits. Full rules: [AI/WORKFLOW.md](./AI/WORKFLOW.md).
- **No AI attribution anywhere** (ADR-0002): no `Co-Authored-By` trailers, no
  "generated with" footers in commits or PR bodies.
- **No panics in production code**: no `unwrap`/`expect`/`panic!`/`unsafe`
  outside tests (the codebase is currently clean — keep it that way; the PR
  template checks it).
- **Media policy** (ADR-0010): no new binary media committed to git; use GitHub
  Release assets (or LFS if unavoidable). Existing `media/` stays; no history
  rewrite.
- **Docs**: mdBook (`docs/src/`) is canonical for user-facing docs (ADR-0006).
  JSON Schema is draft-07 (what schemars 0.8 emits).

## Gotchas

- Example `.lsf` files hardcode Linux font paths (`/usr/share/fonts/...`) —
  see `examples/README.md` for per-OS substitutions (real fix TD-16).
- The Vello backend lacks gradients/shadows/rounded-rects/dashes until v0.4
  (TD-01); scenes using them render differently on `--backend vello`. The
  book's architecture chapter carries the parity table.
- `lumina-server` must not be exposed to untrusted networks before v0.5
  (TD-09; see SECURITY.md).
- Unknown easing names silently fall back to `linear` until v0.4 (TD-08).
- `Timeline` reconstructs object state by serde round-trip and depends on the
  schema's `#[serde(tag = "type", content = "properties")]` layout — changing
  that attribute is a cross-crate breaking change.
- Tag pushes do **not** trigger CI (push trigger is `branches: [main]`).
- wgpu tests run with the CPU fallback adapter (`use_cpu`) for determinism.
