# Lumina Planning System

This directory is the project's long-term engineering memory: roadmap, current
state, debt register, decision log, and workstream briefs. It exists so that any
contributor — human or AI agent — can resume work without re-auditing the whole
repository.

User-facing documentation lives in the mdBook (`docs/src/`); nothing in
`planning/` duplicates it. When the two would overlap, the book wins and
planning documents link to it.

## Where to look

| I want to know… | Read |
|---|---|
| Why Lumina exists, what never changes | [VISION.md](../VISION.md) *(read first)* |
| Why it's designed the way it is | [DESIGN.md](../DESIGN.md) |
| The engineering constitution every PR is judged by | [ENGINEERING_PRINCIPLES.md](../ENGINEERING_PRINCIPLES.md) |
| What we're building next, and why | [ROADMAP.md](./ROADMAP.md) |
| The state of the repo right now (health dashboard + log) | [STATUS.md](./STATUS.md) |
| How the codebase is laid out, commands, conventions, gotchas | [KNOWLEDGE_BASE.md](./KNOWLEDGE_BASE.md) |
| Known weaknesses and their target fix release | [TECH_DEBT.md](./TECH_DEBT.md) |
| Why a decision was made | [DECISIONS.md](./DECISIONS.md) → [ADR/](./ADR/) |
| How to propose a major change | [RFCS/](./RFCS/) |
| Measured repo metrics + quality scorecard, per release | [METRICS.md](./METRICS.md) |
| How Lumina grows beyond this repo | [ECOSYSTEM.md](./ECOSYSTEM.md) |
| The scope/checklist of an active piece of work | [WORKSTREAMS/](./WORKSTREAMS/) |
| How to branch, commit, review, and release | [AI/WORKFLOW.md](./AI/WORKFLOW.md) |
| Role prompts for AI agents working on the repo | [AI/](./AI/) |
| The product vision / positioning narrative (long form) | [project-lumina-blueprint.md](./project-lumina-blueprint.md) |
| The release-by-release project story | [HISTORY.md](./HISTORY.md) |

## Maintenance contract

These documents are only useful if they stay true. The rules:

- **STATUS.md** — add a dated entry at the top after every meaningful work
  session (what landed, CI state, next action). Keep entries ≤ 10 lines.
- **ROADMAP.md** — revised at each release boundary: close the shipped phase,
  re-evaluate the next one against the repo as it now is, not as it was planned.
- **TECH_DEBT.md** — items are only closed by linking the PR that fixed them.
  New debt discovered during any work gets an ID and a row immediately.
- **DECISIONS.md / ADR/** — one file per accepted decision, never edited
  after acceptance; reversals get a new ADR that supersedes the old one and
  links both ways. Major changes go through **RFCS/** first.
- **METRICS.md** — refreshed at every release (measured values + quality
  scorecard); the health dashboard at the top of STATUS.md is re-verified at
  the same time.
- **WORKSTREAMS/** — one file per active workstream, created from the template
  in `WORKSTREAMS/ws-01-hygiene-v0.3.1.md`. Completed workstreams stay in place
  (marked Done) as executable history.
- No AI is ever credited as author, co-author, or contributor anywhere in this
  repository — see [AI/WORKFLOW.md](./AI/WORKFLOW.md).
