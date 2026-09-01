# ADR-0013 — The AAA program lives in plan/, the roadmap stays in planning/

- **Status:** Accepted · **Date:** 2026-09-01

## Context
A program was commissioned to take the project to reference quality across
every dimension — architecture, performance, security, accuracy, motion,
output, tooling, community. It produces fourteen dimension-level design
documents.

Those documents do not fit `planning/`. ADR-0004 scopes that directory to
long-term engineering memory, and ADR-0005 is explicit that `ROADMAP.md` is
the single roadmap. Fourteen documents each proposing work would either drown
the memory system or quietly become a second schedule — the exact failure
ADR-0005 was written to prevent.

## Decision
`plan/` holds the AAA program: one master plan and fourteen subplans, each
covering one dimension. It is a **design and rationale** directory with a
finite life, ending at v1.0.

`planning/` is unchanged and remains authoritative:

- `ROADMAP.md` is still the single schedule of record. No item is worked
  until it appears there as a versioned entry or in `TECH_DEBT.md` as a
  `TD-nn`.
- `STATUS.md`, `METRICS.md`, the ADRs, and the RFC gate all keep their roles.

Where a subplan and the roadmap disagree, the roadmap is correct and the
subplan is stale — repaired by the change that caused the drift, like every
other planning document (ENGINEERING_PRINCIPLES #13).

## Consequences
- Two directories, one schedule. `plan/README.md` states the contract in both
  directions so the distinction cannot quietly erode.
- The program is legible as a whole — someone can read what "world-class"
  means for accuracy or for output fidelity without reconstructing it from a
  roadmap bullet and a debt row.
- `plan/` is deleted or archived at v1.0. Anything still live by then belongs
  in `planning/`, and moving it is part of the v1.0 checklist.
- The risk is drift: fourteen documents can rot. Mitigated by the rule above,
  and by subplans carrying `file:line` evidence rather than prose — evidence
  goes stale visibly, opinions do not.
