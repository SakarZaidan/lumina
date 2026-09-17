# ADR-0015 — Typed properties without a format change

- **Status:** Accepted · **Date:** 2026-09-17
- **Supersedes:** the "LSF v2 with a migration guide and a `migrate` command"
  clause of Wave 6 in `plan/00-master.md`
- **RFC:** [RFC-0002](../RFCS/0002-typed-properties.md)

## Context

TD-07 records that property typos degrade silently to defaults. Before designing
a fix it was reproduced against `main`: a timeline typo (`"raduis"`), a wrong
type (`"radius": "big"`) and an unknown property (`"opacty"`) all passed
`lumina-cli validate`, and rendered respectively an animation that did nothing,
a circle that vanished, and a field silently dropped.

The master plan assumed the fix required a new format version. The
investigation found the schema is already typed and the types are discarded
after parsing — props structs ignore unknown fields, timeline state is untyped
JSON, and the renderer reads properties back by string at 216 sites, defining
defaults a second time in ways that disagree with the schema.

## Decision

Fix TD-07 in two stages, **with no change to the file format**. LSF stays at
`version: "1.0"`.

1. **v0.6** — validate property names and types against the real structs,
   derived through `schemars` rather than a hand-maintained table. Unknown and
   mistyped properties become `UNKNOWN_PROPERTY` and `PROPERTY_TYPE_MISMATCH`
   errors with a did-you-mean.
2. **v0.7** — typed render state, so a misspelled property inside the engine is
   a compile error and defaults are defined exactly once.

No LSF v2, migration guide or `migrate` command is built.

## Consequences

- No existing scene needs migrating. A scene that fails the new validation was
  already rendering something other than what it says.
- Stage 1 does reject scenes that currently validate. That is observable and is
  called out in the changelog for the release that ships it.
- Stage 2 changes `Renderer::render_frame`'s `states` parameter, a semver-major
  change for `luminafx-renderer` that `cargo-semver-checks` will enforce.
- `version: "2.0"` is reserved for a change that genuinely needs the file to say
  something different. Spending it on a checking problem would have made the
  next real format change harder to explain.
- The Wave 6 gate now tests the behaviour TD-07 is about — typos are errors with
  suggestions, examples render identically — rather than a format migration.
