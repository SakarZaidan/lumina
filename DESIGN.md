# Lumina — Design Philosophy

[VISION.md](VISION.md) says why Lumina exists. This document explains **why it
is shaped the way it is** — the reasoning behind the load-bearing design
choices, so future decisions stay consistent with past ones. The current
implementation is described in the [architecture chapter](docs/src/architecture.md).

## Why declarative?

Because the primary author is often not human. An imperative API requires the
author to reason about mutable state over time — the exact failure mode of
language models (and of tired humans). A declarative scene is a *claim about
what should be true at time t*; it can be validated before execution, diffed,
patched, stored, and regenerated. Everything AI-native about Lumina
(schema introspection, structured errors with `fix_suggestion`, semantic
scene patches) is only possible because scenes are data.

**Corollary:** when a feature seems to need logic in the format, the answer is
a generator that *emits* LSF (see `examples/gen_*.py`), never logic *in* LSF.

## Why a schema-first format?

The JSON Schema is generated from the Rust types (`lumina-schema`), so the
validator, the engine, and the documentation cannot drift apart. Every new
field defaults (`#[serde(default)]`) so old scenes never break. The schema is
also the AI interface: paste it (or the `/objects` registry) into a model's
context and generated scenes are correct on the first try far more often.

## Why deterministic — even the particles?

An animation engine that renders frame N differently on two runs cannot be
scrubbed, cached, resumed, tested against golden pixels, or trusted in CI.
Lumina's particles are analytic (a hash of seed × time, not stepped RNG
state) precisely so that *any* frame can be computed in isolation. This is
also what makes frame-parallel export possible later.

## Why Rust?

Compiler-enforced memory safety with no GC pauses at frame boundaries;
first-class WASM via wasm-pack; ergonomic Python bindings via PyO3/maturin;
Cargo instead of CMake; a modern unified GPU stack (vello/wgpu); and data-race
prevention at compile time. C++ wins only when integrating an existing C++
codebase — which we don't have.

## Why two renderer backends?

CPU (`tiny-skia`) is the reference: it runs anywhere — CI, headless servers,
WASM — with zero GPU dependency, and its output defines correctness. GPU
(`vello`/`wgpu`) exists for throughput and the future browser story. The rule
that keeps this honest: **Skia defines the pixels; Vello must match them**
(within tolerance), verified by cross-backend diff tests. When a feature can't
be on both backends yet, the gap is documented in the parity table — never
silent.

## Why scenes render from state maps, not typed objects?

The timeline serializes each object's properties and evaluates a per-frame
`state` map. This is deliberate looseness in one place: new schema fields
flow through the timeline to the renderer with no core changes, which kept
iteration fast in the 0.x era. Its cost (typos degrade to defaults silently)
is known, and the plan is to tighten it — a typed property layer at the
validation boundary (v0.6) — *without* giving up the flow-through property.

## Why an external ffmpeg instead of an in-process encoder?

Encoding is a deep, separately-maintained problem. Piping raw RGBA to ffmpeg
keeps Lumina's dependency tree sane, its licenses clean, and its output
formats current, at the cost of a runtime dependency — a documented,
gracefully-failing one. An optional bundled encoder is backlog, not dogma.

## Why the server speaks "AI loop" natively?

`/validate` returns machine-actionable errors (`code`, `path`,
`fix_suggestion`), `/schema` and `/objects` exist for context-window
injection, and `/scene_patch` applies semantic edits and re-validates in one
round trip. The generate → validate → fix → render loop is the product's
core usage pattern, so the API is shaped around it rather than around CRUD.
