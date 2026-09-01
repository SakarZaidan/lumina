# The AAA Program

This directory holds one thing: the design and execution spec for taking
Lumina from *good* to *reference-quality*. It is a program, not a process —
it has a beginning, eight waves, and an end (v1.0).

## How this relates to `planning/`

They are not two planning systems. The split is deliberate and recorded in
[ADR-0013](../planning/ADR/0013-aaa-program-location.md):

| | `plan/` | `planning/` |
|---|---|---|
| Answers | *What would world-class look like, and how do we get there?* | *What are we doing, and what is it costing us?* |
| Lifetime | Ends at v1.0 | Permanent |
| Shape | 14 subplans, one per dimension | Roadmap, status log, debt register, ADRs, RFCs |
| Authority | Design rationale and target state | **Schedule of record** |

[`planning/ROADMAP.md`](../planning/ROADMAP.md) remains the single roadmap
(ADR-0005). Nothing here schedules work by itself: every item in a subplan
lands in the roadmap as a versioned entry, or in
[`planning/TECH_DEBT.md`](../planning/TECH_DEBT.md) as a `TD-nn`, before it is
worked. If a subplan and the roadmap ever disagree, **the roadmap is right**
and the subplan is stale — fix it in the same change.

## The documents

Start with [`00-master.md`](00-master.md). It carries the waves, the gates
between them, and the scorecard targets. The rest are reference: read the one
that covers what you are about to change.

| | | |
|---|---|---|
| [01](01-architecture.md) | Architecture | Layering, error model, typed schema, extension points |
| [02](02-performance.md) | Performance | Hot paths, measured baselines, targets |
| [03](03-security.md) | Security | Threat model, resource bounds, supply chain |
| [04](04-math-physics-accuracy.md) | Math, physics, accuracy | Springs, sampling, colour, transforms |
| [05](05-animation-motion.md) | Animation and motion | Easing, draw-on, morphing, motion blur |
| [06](06-render-output-fidelity.md) | Render and output fidelity | Linear light, encoding, colour management |
| [07](07-ui-ux-dx.md) | UI, UX, DX | CLI, diagnostics, docs, onboarding |
| [08](08-code-quality.md) | Code quality | Lints, idioms, duplication, dead weight |
| [09](09-features.md) | Features | The capability ladder to v1.0 and past it |
| [10](10-testing-verification.md) | Testing and verification | Property tests, fuzzing, determinism, coverage |
| [11](11-release-distribution.md) | Release and distribution | Publishing to three ecosystems |
| [12](12-community-governance.md) | Community and governance | Contribution rules, roles, on-ramp |
| [13](13-ai-native-interface.md) | AI-native interface | LSF as interchange; the agent loop; MCP |
| [14](14-playground-tooling.md) | Playground and tooling | Web playground, editor extension, SDKs |

## The shape of a subplan

Every one follows the same five sections, so they can be skimmed in any
order:

1. **Current state** — what the code does today, with `file:line` evidence.
   No claim appears here that has not been read in the source.
2. **Target** — what "AAA" means for this dimension specifically.
3. **Work items** — each with a stable id (`AAA-SEC-01`), a rationale, and an
   acceptance test. An item without a way to prove it is done is not an item.
4. **Metrics moved** — which rows of [`planning/METRICS.md`](../planning/METRICS.md)
   this dimension is accountable for.
5. **Sequencing** — which wave, and what it depends on.

## Rules that apply to every item here

These are not new; they are [`ENGINEERING_PRINCIPLES.md`](../ENGINEERING_PRINCIPLES.md)
applied to this program, and they are what keeps it from becoming a wish list.

- **Nothing ships without proof.** A performance item lands with before/after
  benchmark numbers. A rendering item lands with a pixel assertion. A security
  item lands with an adversarial fixture that fails without the fix.
- **Truth before features.** Where a document and the code disagree, the
  document is a bug and it is fixed first — even if the feature it describes
  is then still missing. An honest gap beats a false claim.
- **Determinism is never traded away.** Not for speed, not for parallelism,
  not for a nicer API. Same scene, same time, same pixels — on every backend,
  every platform, forever.
- **No item is "done" by writing it down.** Items close by linking the PR.
