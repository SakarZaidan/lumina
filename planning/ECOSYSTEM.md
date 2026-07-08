# Lumina — Ecosystem Strategy

The repository is the seed, not the tree. This document plans how Lumina
becomes an ecosystem, and — just as important — what the core repo must
provide so that ecosystem pieces can exist *without* forking or entangling
the engine. Product tiers originate in the
[blueprint](project-lumina-blueprint.md); this is the engineering view.

## The layers

**Layer 0 — the open core (this repo, MIT, free forever).**
Engine, format, CLI, server, WASM, SDKs. Everything else builds on it and
nothing here is ever paywalled ([VISION](../VISION.md) principle 4).

**Layer 1 — developer surface (planned repos/packages).**
- `@lumina/sdk` on npm and `lumina-engine` on PyPI (publishing lands v0.5–v0.6).
- **VS Code extension**: LSF language support — schema-driven autocomplete
  and validation (the JSON Schema already exists), inline scene preview via
  the WASM engine, timeline hover info. Mostly glue; high leverage.
- **Template library**: a `lumina-templates` repo of parameterized scene
  generators (bar chart, network diagram, math proof, slide transitions) —
  the "component library" of the ecosystem, and the natural home for
  community contributions that don't belong in `examples/`.

**Layer 2 — products (separate repos, may be commercial).**
- **Lumina Cloud**: hosted render/validate API with auth, quotas, autoscaling
  — the revenue engine per the blueprint. Prerequisite in core: server
  hardening (TD-09) and a stable API contract (RFC'd).
- **Lumina Studio**: visual timeline editor (Tauri + the WASM player) built
  *on the open format* — it reads/writes plain LSF, proving the format's
  editor-friendliness.

**Layer 3 — community.**
- Curated gallery ("made with Lumina") fed by release-asset renders.
- Learning resources: the book grows a cookbook section; template repo
  doubles as tutorials.
- Showcase pipeline: every release attaches demo media (ADR-0010) — those become
  the marketing surface for free.

## What the core repo owes the ecosystem

These are engineering requirements on *this* repo, tracked in the roadmap:

1. **A versioned, stable LSF schema** with migration guides — editors and
   templates die if the format shifts under them (v0.6 typed schema is the
   gate to calling it stable).
2. **Published packages** (crates.io, PyPI, npm) — nobody builds on a git
   clone (v0.4–v0.6).
3. **A plugin seam for renderers.** The `Renderer` trait is already the
   extension point; keeping it minimal and documented is what lets a future
   `lumina-renderer-skia-gpu` or a Lottie-export backend live out-of-tree.
   Any trait change goes through RFC (principle 9).
4. **Asset conventions.** Fonts/images are referenced by id + path today; an
   ecosystem needs a documented resolution order and a bundling story before
   templates can be portable (backlog: asset pipeline).
5. **Conformance material**: the golden-scene suite (v0.4 pixel-diff corpus)
   doubles as a conformance kit for third-party players.

## Sequencing

Ecosystem work must not outrun the core: nothing in Layer 1+ starts before
its prerequisites in the [ROADMAP](ROADMAP.md) land. The likely order:
npm/PyPI publishing → VS Code extension (cheap, high visibility) → template
library → Cloud (after v0.5 hardening) → Studio (after schema stability).
Each new repo starts with the same governance files as this one (LICENSE,
CONTRIBUTING, CoC, CI) — this repo is the template.
