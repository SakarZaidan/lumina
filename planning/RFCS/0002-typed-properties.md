# RFC-0002: Typed properties without a format change

- **Status:** Accepted
- **Author:** —
- **Created:** 2026-09-17
- **Related:** TD-07, `AAA-AI-04`, `plan/00-master.md` Wave 6, `plan/01-architecture.md`, RFC process (`GOVERNANCE.md`)

## Problem

A mistake in a scene is supposed to be a validation error with a path and a
fix. For property names and values it is not. Three failures, each reproduced
against `main` on 2026-09-17 with `lumina-cli validate` and a single rendered
frame of a 64×64 scene whose red circle animates `radius` from 10 to 20:

| Scene | `validate` | Rendered at t = 2 s |
|---|---|---|
| control: `"radius": 20` in the timeline | `ok` | radius ≈ 19.7 — correct |
| `"raduis": 20` in the timeline | **`ok`** | radius ≈ 9.6 — **the animation silently did nothing** |
| `"radius": "big"` in the timeline | **`ok`** | **0 red pixels — the circle vanished** |
| `"opacty": 0.5` in `properties` | **`ok`** | opacity 1.0 — the field was silently dropped |

None of these produce a warning anywhere. The validator reports success, the
render completes, and the output is wrong in a way the author has to notice by
eye. For a format whose stated audience includes language models writing
scenes in a loop — send, read the errors, fix, resend — a silent success is the
worst possible answer, because there is nothing in the response to act on.

### Why it happens

The schema is **already typed**. `CircleProps` has `radius: f32`, and a wrong
type *inside `properties`* is rejected at parse time. The types are then thrown
away:

1. **Unknown fields are ignored.** No props struct carries
   `#[serde(deny_unknown_fields)]`, so `"opacty"` is dropped during
   deserialisation and the validator never sees it existed.
2. **Timeline state is `serde_json::Value`.** `TimelineEntry.state` is untyped,
   so `"raduis"` becomes its own track, is interpolated every frame, and is
   never read by anything.
3. **The renderer reads strings, not types.** `Timeline::from_scene` serialises
   each typed object back into JSON and every draw call reads it back out as
   `state["radius"].as_f64().unwrap_or(0.0)` — **216 such reads** across the two
   backends (122 CPU, 94 GPU). A string where a number belongs becomes `None`,
   and `None` becomes the fallback.

There is a fourth consequence the reads hide: **defaults are defined twice, and
disagree.** `CircleProps.stroke_width` defaults to `0.0` in the schema; the
renderer reads `state["stroke_width"].as_f64().unwrap_or(1.0)`. On the normal
path the schema value is always present, so the renderer's `1.0` is dead code —
until some path produces state without it (an interactive `set_property`
override, a hand-built state map in an embedder), at which point the two
sources of truth quietly diverge.

## Proposal

Two stages, **neither of which changes the file format.** LSF stays at
`version: "1.0"`; every valid scene written today stays valid and renders the
same pixels.

### Stage 1 — Validate against the structs (v0.6)

Make property names and types part of validation, with the same
`code`/`path`/`message`/`fix_suggestion` envelope every other error uses.

- **Source of truth: the structs themselves.** The permitted properties and
  their JSON types for each object type come from `schemars::schema_for!` over
  the real `*Props` structs — not from a hand-maintained table. The existing
  `object_registry()` is a readable summary for agents and stays that; it does
  not become a second source of truth that can drift.
- **Unknown property in `properties` or a timeline `state`** →
  `UNKNOWN_PROPERTY`, with a did-you-mean from `lumina_core::suggest` (#93),
  e.g. `"raduis" is not a property of Circle — did you mean "radius"?`
- **Wrong JSON type** → `PROPERTY_TYPE_MISMATCH`, naming the expected type:
  `"radius" on Circle is a number; got a string ("big")`.
- **Catching unknown fields in `properties`** needs the raw JSON, because
  deserialisation discards them before the validator runs. A new
  `validate_scene_json(&serde_json::Value)` walks the raw document and then
  calls the existing `validate_scene_data`; the CLI, server and MCP entry
  points — which all parse from text already — call it instead. The typed
  function stays for embedders who build a `Scene` in code.
- **`deny_unknown_fields` is deliberately not used.** It would reject the same
  scenes, but as a serde error with no path, no suggestion, and no
  machine-readable code — trading a silent failure for an unactionable one.

### Stage 2 — Typed render state (v0.7)

Replace the stringly-typed state the renderer consumes with typed per-object
state, so a misspelled property name *inside the engine* is a compile error
rather than a silent default.

- `Timeline::get_state_at` produces typed state (one struct per object type)
  instead of `HashMap<String, serde_json::Value>`. Interpolation becomes
  per-field and type-aware — which also removes the per-frame `Value` clone
  TD-03 still carries.
- The 216 `state["…"].as_f64().unwrap_or(…)` reads become field accesses.
  **Defaults are defined once, in the schema**, and the renderer's copies are
  deleted rather than reconciled.
- `set_property` / `tween_to` event actions resolve the property name against
  the same schema at dispatch time, so an interactive override with a typo is
  an error, not a track nobody reads.
- This changes the `Renderer` trait's `states` parameter, which is a public API
  change in its own right. `cargo-semver-checks` (#98) will require the version
  bump, which is the point of having it.

### What this RFC recommends *not* doing: LSF v2

`plan/00-master.md` frames TD-07 as needing "LSF v2 with a migration guide and a
`migrate` command". The investigation for this RFC found that it does not.
Every failure above is a failure of **checking**, not of the format: the file
already says what the author meant, and the engine discards the information
needed to notice when it doesn't. Typing the engine fixes that without changing
a byte of any scene.

A format version bump would force every existing scene, every example, every
model prompt and every SDK caller through a migration — for no behavioural
gain. `version: "2.0"` should be reserved for a change that genuinely needs the
file to say something different. This RFC proposes amending the Wave 6 gate
accordingly (see **Migration**).

## Alternatives

1. **Do nothing.** Rejected on the evidence above: the validator reports success
   on scenes that render wrong or render nothing.
2. **`#[serde(deny_unknown_fields)]` on every props struct.** One line each, and
   it does reject `"opacty"`. Rejected as the *whole* answer: the error is a
   serde message with no JSON path, no did-you-mean and no error code, and it
   does nothing for timeline `state`, which is `Value` and never reaches serde's
   field matching. Still unhelpful for a model in a loop.
3. **A hand-written property table** (extend `object_registry()` with types).
   Rejected: a second description of the schema drifts from the structs the
   moment someone adds a field to one and not the other. The structs must be the
   only source of truth.
4. **Stage 2 only — go straight to typed render state.** Catches renderer-side
   typos at compile time, but on its own does *not* catch authoring typos:
   unknown keys in a scene still need to be reported to the author, which is
   Stage 1's job. Stage 2 without Stage 1 fixes the engine and leaves the user
   exactly where they are.
5. **LSF v2 with a format change.** Rejected above: it imposes a migration on
   every user to fix a problem that is not in the format.

## Trade-offs

- **Stage 1 rejects scenes that currently validate.** A scene carrying a
  misspelled or unknown property passes today and fails after. That is the
  intended effect — each such scene already renders something other than what
  it says — but it is observable, and a scene written for a *newer* Lumina with
  a property this version does not know will now fail loudly instead of being
  partially ignored. Loud is the correct failure; it is still a change.
- **A second validation entry point** (`validate_scene_json`) alongside the
  typed one. Two functions to keep consistent; mitigated by having the raw one
  call the typed one rather than reimplement it.
- **schemars becomes load-bearing at runtime**, not only for `/schema`. It is
  already a dependency; the risk is behavioural coupling to its output shape,
  which issue #50 (schemars 1.x, held) will have to account for. Stage 1 should
  consume the schema through one small adapter so a schemars upgrade touches
  one file.
- **Stage 2 is large**: 216 read sites across two backends, the timeline, event
  dispatch, the WASM engine, and a public trait change. It needs the parity
  suite green at every step and is sized as its own release for that reason.

## Migration

- **Scenes:** none required. No format change; `version` stays `"1.0"`. A scene
  that fails Stage 1 validation was already rendering incorrectly, and the new
  error names the property and suggests the fix.
- **Rust API (Stage 2):** `Renderer::render_frame`'s `states` parameter changes
  type. Semver-major for `luminafx-renderer`; enforced by `cargo-semver-checks`.
- **Server / MCP / SDK clients:** new error codes (`UNKNOWN_PROPERTY`,
  `PROPERTY_TYPE_MISMATCH`) in the existing envelope. Additive for clients that
  branch on `code`.
- **Plan amendment:** the Wave 6 gate currently reads *"schema v2 documented; a
  v1 scene migrates and renders identically."* This RFC proposes replacing it
  with *"every unknown or mistyped property is a validation error with a
  suggestion; every existing example validates and renders identically."* —
  which tests the behaviour TD-07 is about rather than a format change it does
  not need.

## Performance

- **Stage 1:** one extra walk of the raw JSON at validation time, which happens
  once per scene, not per frame. Expected to be negligible; measured by adding a
  validation case to `luminafx-bench` with the regression gate at its existing
  threshold.
- **Stage 2:** expected to be a **gain**. Typed per-field interpolation removes
  the per-frame `serde_json::Value` clone and map construction that TD-03 still
  carries, and field access replaces a string-keyed map lookup at 216 sites per
  object per frame. Measured with `timeline_eval` and `skia_render`; acceptance
  is no regression beyond noise, with the expectation of an improvement.

## Examples

A model writes a scene with a typo. Today:

```json
{ "time": 1.0, "object": "c", "state": { "raduis": 20 } }
```

```
$ lumina-cli validate scene.lsf
ok
```

After Stage 1:

```
$ lumina-cli validate scene.lsf
error: UNKNOWN_PROPERTY at $.timeline[0].state.raduis
  "raduis" is not a property of Circle.
  fix: Did you mean 'radius'?
1 error(s), 0 warning(s)
```

And the same scene with a wrong type:

```
error: PROPERTY_TYPE_MISMATCH at $.timeline[0].state.radius
  "radius" on Circle is a number; got a string ("big").
  fix: Use a number, e.g. "radius": 20
```

After Stage 2, inside the engine:

```rust
// before — a typo here compiles, and silently draws radius 0
let radius = state["raduis"].as_f64().unwrap_or(0.0) as f32;

// after — a typo here does not compile
let radius = state.radius;
```

## Decision

**Accepted as written** by the maintainer, 2026-09-17. Recorded as
[ADR-0015](../ADR/0015-typed-properties-no-format-change.md).

Both stages proceed with no change to the file format: `version` stays `"1.0"`.
The LSF v2, migration guide and `migrate` command the master plan assumed are
not built. `plan/00-master.md`'s Wave 6 gate is amended as proposed under
**Migration**, in the same change that accepts this RFC.
