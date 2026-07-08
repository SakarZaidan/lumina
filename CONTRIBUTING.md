# Contributing to Lumina

Thank you for your interest in contributing! Whether it's bug fixes, features,
or documentation, help is welcome. The bar is simple: the workspace stays green
and new behavior is covered by a test.

This file is the canonical contribution guide. Read
[VISION.md](VISION.md) first, and know that every PR is judged against
[ENGINEERING_PRINCIPLES.md](ENGINEERING_PRINCIPLES.md). Repo conventions and
workflow details live in
[`planning/KNOWLEDGE_BASE.md`](planning/KNOWLEDGE_BASE.md) and
[`planning/AI/WORKFLOW.md`](planning/AI/WORKFLOW.md); substantial public-API
changes go through [`planning/RFCS/`](planning/RFCS/) before implementation.

## Setup

- Latest stable [Rust](https://rustup.rs/) (MSRV: 1.88, enforced via
  `rust-version` in `Cargo.toml`).
- `ffmpeg` on PATH — required for MP4/WebM/GIF export and the export tests.
- Optional: `wasm-pack` (wasm crate), `mdbook` (docs site), `maturin`
  (Python SDK).

## Development workflow

1. Fork and clone; create a branch: `feat/<slug>`, `fix/<slug>`, `docs/<slug>`,
   or `ci/<slug>`.
2. Make focused changes that follow the existing style and crate layering
   (`schema` → `core` → `renderer` → `export`).
3. Add or update tests in the relevant crate.
4. Run the full gate before opening a PR:

```bash
cargo fmt --all --check
cargo clippy --workspace --exclude lumina-wasm --all-targets -- -D warnings
cargo test --workspace --exclude lumina-wasm --exclude lumina-bench   # what CI runs
cargo build -p lumina-wasm            # wasm lib compiles
mdbook build docs                     # when you touched docs/src/
```

For the SDKs:

```bash
(cd sdks/python && maturin develop && python -c "import lumina")
```

## Conventions

- **Commits**: [Conventional Commits](https://www.conventionalcommits.org)
  (`feat(core): …`, `fix(renderer): …`). One logical change per commit.
- **No panics in production code**: no `unwrap`/`expect`/`panic!` outside
  tests; degrade gracefully on malformed input. `unsafe` requires a thorough
  justification and safety comments (the codebase currently has none).
- **Schema compatibility**: every new schema field takes `#[serde(default)]`
  so existing `.lsf` files keep working.
- **New object types** must be handled in: the schema enum, the Skia `z_index`
  and draw match, the Vello match, and the WASM `hit_test`/`get_z_index`.
- **Rendering changes** need a renderer test — pixel assertions over a known
  scene are preferred; rendering must stay deterministic.
- **Docs**: update `CHANGELOG.md` (`[Unreleased]`) and the relevant mdBook
  chapter (`docs/src/`) when a change is user-facing.
- **No binary media in git**: attach demo videos/GIFs to GitHub Releases
  instead of committing them.
- **Authorship**: all work is attributed to its human author. Do not add AI
  tools as authors or co-authors in commits or PR bodies.

## Reporting issues

Please include: a clear description, a minimal `.lsf` scene that reproduces
the problem, and your environment (OS, GPU, Rust version). For security
issues, see [SECURITY.md](SECURITY.md) — do not open a public issue.
