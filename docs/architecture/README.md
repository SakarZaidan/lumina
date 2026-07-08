# Architecture Diagrams

Generated diagrams of Lumina's structure and data flows. **Never edit the
SVGs** — regenerate them:

```bash
docs/architecture/gen-diagrams.sh   # needs graphviz, cargo, python3
```

| Diagram | Source of truth |
|---|---|
| [dependency-graph.svg](dependency-graph.svg) | **Generated from `cargo metadata`** — the real crate edges, not a drawing |
| [scene-pipeline.svg](scene-pipeline.svg) | `scene-pipeline.dot` (hand-maintained) |
| [render-pipeline.svg](render-pipeline.svg) | `render-pipeline.dot` (hand-maintained) |
| [event-pipeline.svg](event-pipeline.svg) | `event-pipeline.dot` (hand-maintained) |
| [export-pipeline.svg](export-pipeline.svg) | `export-pipeline.dot` (hand-maintained) |

The script also copies the SVGs into `docs/src/diagrams/` so the
[book's architecture chapter](../src/architecture.md) embeds them. If a PR
changes crate dependencies or a pipeline, rerunning this script is part of
the change (see [ENGINEERING_PRINCIPLES](../../ENGINEERING_PRINCIPLES.md) #13).
