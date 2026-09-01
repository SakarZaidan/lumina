# 13 — The AI-native interface

VISION.md names the moat directly: no other tool combines an open declarative
format an AI can write, GPU-capable 2D rendering, one scene that exports to
video *and* runs interactively, first-class math, an event system, and a
headless validation server built for agent loops. Each exists somewhere; the
combination does not. This document is about making the combination real
rather than merely claimed.

## Current state

The design is right and half-built.

**What works.** `/validate` returns structured errors with `code`, `path`,
`message`, and `fix_suggestion` — a genuine machine contract, and the reason
generated scenes converge. `/schema` emits JSON Schema generated from the Rust
types, so validator, engine, and documentation cannot drift. `/objects`
returns a registry of all 17 object types with their required and optional
properties, explicitly for context-window injection. `/scene_patch` applies
semantic operations (`add_object`, `add_keyframe`, `update_property`, …) with
cascade deletes and re-validates in one round trip. Scenes are data, so they
are diffable and patchable. Particles are analytic, so any frame is
reproducible in isolation.

**What is missing or broken.**

*The error contract stops after one endpoint.* `/render`, `/patch`, and
`/scene_patch` return bare strings (`server/src/lib.rs:151,179,285,312`). An
agent gets machine-actionable JSON from `/validate` and prose from everything
else, which means the loop works for authoring and breaks for everything past
it.

*Typos are silent.* Properties flow as untyped `serde_json::Value` to the
renderer (TD-07), so a model that writes `"radius"` where the schema says
`"r"` gets a default rather than an error. This is the single worst property
for AI authorship: the failure is invisible, so the loop has nothing to
correct.

*Nothing consumes `fix_suggestion`.* The validator produces repair advice and
no tool applies it. The generate → validate → fix → render loop that DESIGN.md
calls "the product's core usage pattern" has no implementation of the *fix*
step.

*There is no MCP server.* Every agent integration is bespoke HTTP.

*The schema is large and undifferentiated.* Pasting the whole thing into a
context window works but is wasteful; there is no way to request the subset
relevant to a task.

*No golden corpus.* Nothing measures whether generated scenes actually
validate on the first attempt, so the central product claim is unmeasured.

## Target

An agent can discover the format, write a scene, get told precisely what is
wrong in a form it can act on, repair it, and render — without a human writing
glue code, and with the success rate measured rather than asserted.

## Work items

| ID | Item | Acceptance |
|---|---|---|
| `AAA-AI-01` | One structured error envelope on every endpoint | An agent parses `/render` failures the same way as `/validate` |
| `AAA-AI-02` | MCP server exposing `validate`, `schema`, `objects`, `scene_patch`, `render` as tools | An MCP-capable agent drives the engine with no bespoke code |
| `AAA-AI-03` | `lumina fix` — apply `fix_suggestion`s, re-validate, iterate to a fixpoint | The core loop exists as a command |
| `AAA-AI-04` | Typed properties (TD-07) so unknown fields error | A misspelled property is a structured error with a suggestion |
| `AAA-AI-05` | Scoped schema: `/schema?objects=Circle,Text` and a compact mode | Context cost drops for narrow tasks |
| `AAA-AI-06` | Golden prompt corpus: N prompts → generated scenes → first-attempt validation rate, tracked per release | The "correct on the first try" claim becomes a number in METRICS |
| `AAA-AI-07` | An authoring guide written *for models*, and shipped as a retrievable document | One canonical system prompt, versioned with the schema |
| `AAA-AI-08` | Did-you-mean on every unknown identifier, not just easing names | `suggest_easing`'s Levenshtein approach generalised to object ids, properties, and asset ids |
| `AAA-AI-09` | Streaming render progress over the server | Long renders report progress to an agent instead of timing out |

`AAA-AI-06` is the keystone. Everything else here is a guess about what helps
models until there is a corpus measuring it. It is also the most defensible
marketing claim the project could own: a published, reproducible first-attempt
validity rate, tracked release over release.

## Relationship to the format's principles

Nothing here weakens VISION principle 1: scenes stay data, with no logic in
the format. The fix loop, the MCP server, and the scoped schema are all *host*
capabilities operating on data. A capability that seems to need logic in LSF
belongs in a generator that emits LSF — as `examples/gen_*.py` already
demonstrate.

## Metrics moved

API design, Documentation, and Ecosystem. Adds a first-attempt validity row.

## Sequencing

`AAA-AI-01` and `08` in Wave 5 with the server work. `04` in Wave 6 — it is
TD-07 and gates the rest. `02`, `03`, `05`, `07` in Wave 6 after typing lands,
since an MCP server over an untyped schema would enshrine the wrong contract.
`06` should start early and cheaply in Wave 2, because its value is the trend
line, not any single measurement.
