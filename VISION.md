# Lumina — Vision

> Read this before anything else. The [ROADMAP](planning/ROADMAP.md) says what
> we build next; the [architecture chapter](docs/src/architecture.md) says how
> it works today. This document says **why Lumina exists and what must stay
> true** no matter how the code evolves.

## Why Lumina exists

**Lumina is the animation engine for the AI era: declarative by design,
GPU-capable by architecture, and runnable everywhere humans and machines need
motion.**

Every established animation tool has a fundamental mismatch with how software
is built now:

- Imperative APIs demand stateful reasoning that LLMs hallucinate.
- CPU-bound rendering can't hit real-time with complex math scenes.
- No single format runs both offline (video) and online (interactive).
- LaTeX/math rendering is bolted on as an afterthought.
- Animations are passive because no event system exists.
- Without a schema there is no validation, so broken output surfaces at
  runtime.

Lumina answers all six with one coherent design: a **validatable, declarative
JSON scene format** (LSF) that a human, a program, or a language model can
write, and a **deterministic engine** that renders that scene to video, to a
browser canvas, or to an interactive component — pixel-identically.

## Why developers should choose it

No other tool combines: an open declarative format AI can write · GPU-capable
2D rendering · one scene that exports to video *and* runs interactively in the
browser · first-class math/LaTeX · a built-in event system · a headless
validation/render server designed for agent loops. Each exists somewhere;
the combination is the moat.

## Who it serves

1. **AI-agent developers and backend engineers** generating educational or
   explainer animations programmatically — the primary user.
2. Frontend developers embedding interactive math/data visualizations.
3. Data scientists animating data stories.
4. Educators who want Manim's rigor with web deployment and speed.

## Principles that never change

1. **Scenes are data.** No scripting, no loops, no imperative escape hatch in
   the format. If a capability needs logic, it belongs in a generator or the
   host — never in LSF.
2. **Determinism is sacred.** The same scene at the same time yields the same
   pixels, on every backend, forever. Scrubbing, caching, testing, and trust
   all depend on it.
3. **The schema is the contract.** Every scene is validatable before render;
   errors are structured and machine-correctable. Silent acceptance of
   invalid input is a bug.
4. **The format stays open and the core stays MIT.** Commercial layers may
   exist around Lumina ([ecosystem strategy](planning/ECOSYSTEM.md)); the
   format and engine are never the paid wall.
5. **Truth in documentation.** We document what the code does, not what we
   wish it did. Known gaps are stated (see the backend parity table), never
   papered over.

## Out of scope — permanently

- A CSS-animation replacement (wrong layer).
- A video editor (wrong audience).
- A general-purpose game engine (wrong scope).
- A Python-only tool for one niche (wrong reach).

## What success looks like in five years

- LSF is a recognized interchange format: LLMs emit it reliably from a schema
  in their context window, and third-party players/editors consume it.
- The engine is the default answer to "how do I programmatically generate an
  explainer animation" in Rust, Python, and JS ecosystems.
- A scene authored in year one still renders identically in year five
  (versioned schema, migration guides, no silent breakage).
- An ecosystem exists around the core — editor tooling, template libraries,
  a hosted render API — sustained by the open format
  ([ECOSYSTEM.md](planning/ECOSYSTEM.md)).
- Engineers study the repository as an example of how to build and maintain
  production Rust ([ENGINEERING_PRINCIPLES.md](ENGINEERING_PRINCIPLES.md)).
