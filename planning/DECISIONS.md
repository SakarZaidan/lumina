# Decision Log — Index

Every accepted decision is a permanent Architecture Decision Record in
[`ADR/`](./ADR/), one file per decision, never edited after acceptance —
reversals get a new ADR that supersedes the old one and links both ways.
Decisions that need design discussion first go through
[`RFCS/`](./RFCS/) and land here on acceptance.

| ADR | Decision | Status |
|---|---|---|
| [0001](ADR/0001-canonical-repository-url.md) | Canonical repo URL is github.com/SakarZaidan/lumina | Accepted |
| [0002](ADR/0002-no-ai-attribution.md) | No AI attribution, ever | Accepted |
| [0003](ADR/0003-local-scratch-never-committed.md) | Local scratch files never enter git | Accepted |
| [0004](ADR/0004-planning-docs-location.md) | Internal planning docs live in planning/, tracked | Accepted |
| [0005](ADR/0005-single-roadmap.md) | todo.md retired; ROADMAP.md is the single roadmap | Accepted |
| [0006](ADR/0006-mdbook-canonical.md) | The mdBook is canonical for user-facing docs | Accepted |
| [0007](ADR/0007-ai-prompts-location.md) | AI agent prompts live in planning/AI/ | Accepted |
| [0008](ADR/0008-historical-tag-placement.md) | Historical tag placement (v0.1.0–v0.3.0) | Accepted (executed) |
| [0009](ADR/0009-publish-flags.md) | lumina-server and lumina-wasm are publish = false | Accepted |
| [0010](ADR/0010-media-policy.md) | No new binary media in git; use Release assets | Accepted |
| [0011](ADR/0011-release-automation.md) | Release automation via release-plz (v0.4) | Accepted, pending |
| [0012](ADR/0012-latex-unicode-substitution.md) | LaTeX renders by Unicode substitution; mitex dropped | Accepted |
| [0013](ADR/0013-aaa-program-location.md) | The AAA program lives in plan/; the roadmap stays in planning/ | Accepted |

To add a decision: copy the format of an existing ADR
(`Status/Date/Context/Decision/Consequences`), take the next number, add a row
here. Historical note: ADRs 0001–0011 were originally sections D-001…D-011 of
this file before the split.
