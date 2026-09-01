# 01 — Architecture

## Current state

The layering is genuinely clean and worth protecting:

```
lumina-schema → lumina-core → lumina-renderer → lumina-export → {server, cli, py}
                    lumina-text ↗                                lumina-wasm ↗
```

No upward dependencies, no logic in the data crate, no pixels outside the
renderer. `crates/lumina-renderer/tests/duplication_gate.rs` is a source-text
assertion that fails the build if deduplicated parser code reappears in a
backend — an architectural rule enforced by a test, which is rare and right.

Four things weaken it.

**Four error idioms that do not compose.** `thiserror` in core and renderer,
`Result<(), String>` in `lumina-text/src/lib.rs:37`, library-level `anyhow` in
`lumina-export`, and `io::Error` plus `format!` in the server. A font failure
starts as a `String`, is wrapped into `RendererError::Failed`
(`skia_backend.rs:1006`), flattened into `anyhow::anyhow!` (`export/lib.rs:63`),
and finally rendered as prose by an HTTP handler. Structure is lost at every
hop, and `RendererError`'s dominant variant is itself `Failed(String)`.

**Half the server speaks a machine contract, half does not.** `/validate`
returns `code` / `path` / `message` / `fix_suggestion`. `/render`, `/patch`,
and `/scene_patch` return bare strings (`server/src/lib.rs:151,179,285,312`).
The AI loop is the product's core usage pattern (DESIGN.md), and it degrades
to prose on three of four endpoints.

**Untyped properties flow end to end.** The `Renderer` trait takes
`states: &HashMap<String, Value>` (`renderer/src/lib.rs:39`) and backends read
`state["cx"].as_f64().unwrap_or(0.0)`. DESIGN.md defends this as deliberate
looseness that kept 0.x iteration fast, and names its cost: a typo degrades to
a default in silence. That is TD-07, the largest pre-1.0 breaking change.

**Shared geometry is crate-private.** `common::scene::{Mat2x3, group_transform,
camera_transform}` is `pub(crate)` to `lumina-renderer`. `lumina-wasm` needs
exactly that transform for hit-testing and cannot have it, which is why hit
tests apply translation only (TD-21).

**One piece of dead state.** `SceneGraph::add_object` / `remove_object`
(`core/src/scene.rs:42,47`) document that they "refresh root membership"; they
never touch `root_objects`. It is harmless only because renderers recompute
roots from scratch every frame.

## Target

One error type per crate, all `thiserror`, all composing without losing
structure. One error envelope on every server endpoint. Properties typed at
the validation boundary without giving up flow-through. Shared geometry shared.
No documented behaviour that does not happen.

## Work items

| ID | Item | Acceptance |
|---|---|---|
| `AAA-ARCH-01` | `lumina-text` gets a `thiserror` `TextError`; `Result<(), String>` removed | No `Result<_, String>` in any library crate |
| `AAA-ARCH-02` | `lumina-export` drops library-level `anyhow` for `ExportError` | `anyhow` appears only in binaries |
| `AAA-ARCH-03` | `RendererError` gains real variants; `Failed(String)` becomes the genuine fallback, not the default | Callers can match on font/asset/geometry/backend failure |
| `AAA-ARCH-04` | One `ApiError` envelope with `code`/`path`/`message`/`fix_suggestion` on every endpoint | A client parses failures from `/render` the same way as from `/validate` |
| `AAA-ARCH-05` | Promote `common::scene` to a documented public module under an RFC | `lumina-wasm` uses the same transform the renderer does; TD-21 closes |
| `AAA-ARCH-06` | Typed property layer at the validation boundary (TD-07), LSF v2 | An unknown property is an error, not a silent default; v1 scenes migrate |
| `AAA-ARCH-07` | Delete `SceneGraph`'s dead root bookkeeping or make it true | Documented behaviour matches observed behaviour |
| `AAA-ARCH-08` | `render_into(&mut [u8])` on the `Renderer` trait | See [02](02-performance.md); the trait stops forcing an allocation |
| `AAA-ARCH-09` | Extract the ~710-line `draw_leaf_object` match into per-object modules | Both backends shrink; the duplication gate widens to cover emit code |

## Extension points to design deliberately

Today a new object type must be added in the schema enum, the Skia z-index
match, the Skia draw match, the Vello match, and the WASM `hit_test` and
`get_z_index` — six places, listed in `CONTRIBUTING.md` because the compiler
cannot enforce them all. `AAA-ARCH-09` should reduce that to one trait
implementation per object, checked exhaustively by the compiler.

## Metrics moved

Architecture (90 → 97), API design (75 → 96).

## Sequencing

`AAA-ARCH-01..04` and `07` in Wave 1 (they are small and unblock the error
story). `05` and `08` in Wave 3, since both are prerequisites for performance
work. `06` in Wave 6, after the parity suite and the test suite are mature
enough to catch a breaking schema change. `09` in Wave 4.
