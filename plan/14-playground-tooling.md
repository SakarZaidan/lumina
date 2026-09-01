# 14 — Playground, editor tooling, SDKs

## Current state

The engine already compiles to WebAssembly and runs in a browser:
`LuminaEngine::new(scene)` then `render_frame(time)` returns RGBA to paint on
a canvas, with `hit_test` and `process_event` for interactivity. The JS SDK
wraps that in a React component, a vanilla `createPlayer`, and a `useLumina`
hook. That is most of a playground already built.

None of it is reachable.

**The JS SDK does not build.** `sdks/javascript/src/createPlayer.ts:1` imports
`../wasm/lumina_wasm`, and nothing wires `wasm-pack` output into the package
build (TD-12). A clean checkout cannot produce a working package.

**There is no playground.** To see what Lumina does, a visitor must clone the
repository, install Rust and ffmpeg, and render a scene. For a project whose
entire premise is "write JSON, get animation", that is the wrong first
experience by a wide margin.

**There is no editor tooling.** LSF is JSON with a generated JSON Schema —
the single easiest language-server story imaginable — and no editor knows
about it. No autocomplete, no inline validation, no preview.

**The book cannot show motion.** It documents an animation engine in static
text. Every example is a code block and a claim.

## Target

Someone who hears about Lumina can see it working in ten seconds, edit a scene
in twenty, and share the result with a URL — without installing anything.

## Work items

### Playground

| ID | Item | Acceptance |
|---|---|---|
| `AAA-PLAY-01` | Repair the JS SDK build (TD-12); CI job that builds it | `npm pack` works from a clean checkout |
| `AAA-PLAY-02` | Playground page on the docs site: editor pane, canvas pane, timeline scrubber | Loads and plays a bundled scene with no network calls after load |
| `AAA-PLAY-03` | Schema-driven editing: autocomplete, inline validation, `fix_suggestion` surfaced as a quick fix | Errors appear as the user types, with the same `code`/`path`/`fix_suggestion` the server returns |
| `AAA-PLAY-04` | Share by URL — scene compressed into the fragment | A link reproduces a scene exactly, with no server |
| `AAA-PLAY-05` | Example gallery: every scene in `examples/` loadable in one click | The showcase becomes interactive rather than a video file |
| `AAA-PLAY-06` | Export from the browser: PNG frame, or a scene file to download | A visitor leaves with something |

The playground is deliberately **static**: WASM plus a text editor on GitHub
Pages, no backend, no accounts, no hosting cost. Video export still requires
ffmpeg and stays a CLI or server operation.

### Editor extension

| ID | Item | Acceptance |
|---|---|---|
| `AAA-PLAY-07` | VS Code extension: schema validation for `.lsf` | Red squiggles on invalid scenes |
| `AAA-PLAY-08` | Inline preview panel driven by the same WASM build | A frame preview beside the file |
| `AAA-PLAY-09` | Render command with output selection | No terminal switch to produce a video |

Publishing the JSON Schema to SchemaStore would give every JSON-aware editor
basic validation for free, before any extension exists. That is the cheapest
item in this document and should be done first.

### SDKs

| ID | Item | Acceptance |
|---|---|---|
| `AAA-PLAY-10` | Python SDK: tests, webm/gif exposure, PyPI publish (TD-13) | `pip install lumina` then render in three lines |
| `AAA-PLAY-11` | JS SDK: types, docs, npm publish | The README instruction stops being false |
| `AAA-PLAY-12` | WASM WebGPU: run the Vello backend in the browser | The second backend earns its existence in the browser too |

## Why this ranks where it does

It is Wave 7 — late — and that is deliberate. A playground built on a schema
that silently swallows typos, or on a renderer whose GPU path is a CPU path,
would advertise the wrong product very effectively. The engine has to be true
before it is made visible.

The one exception is `AAA-PLAY-01`: the JS SDK is broken *now*, and the README
tells people to install it *now*. That is a documentation defect
([08](08-code-quality.md)) and it is fixed in Wave 1 either by repairing the
build or by removing the claim.

## Metrics moved

Ecosystem (30 → 95), jointly with [11](11-release-distribution.md) and
[12](12-community-governance.md). Also Documentation and Examples.

## Sequencing

`AAA-PLAY-01` in Wave 1. SchemaStore submission in Wave 5 with the other
publishing. `02`–`06` in Wave 7. `07`–`09` after the playground, since the
extension reuses its editor integration. `10`, `11` in Wave 5. `12` is
unscheduled backlog — it depends on wgpu's browser story more than on us.
