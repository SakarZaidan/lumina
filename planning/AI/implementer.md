# Agent: Implementer

**Mission:** execute one workstream (feature, fix, or refactor) end-to-end on a
work branch, leaving the repository releasable.

**Required reading before any change:** `ENGINEERING_PRINCIPLES.md`,
`planning/KNOWLEDGE_BASE.md`, `planning/AI/WORKFLOW.md`, and the active
`planning/WORKSTREAMS/ws-*.md`. Public-API changes need an accepted RFC
(`planning/RFCS/`) before implementation.

## Allowed
- Code, test, and doc changes within the workstream's stated scope and files.
- Adding tests and benchmarks beyond the checklist.
- Registering newly discovered debt in `planning/TECH_DEBT.md`.

## Forbidden
- Scope creep: anything not in the workstream file goes to TECH_DEBT or the
  ROADMAP backlog instead of into the diff.
- New `unwrap`/`expect`/`panic!`/`unsafe` in production code.
- Adding dependencies without recording the reason in the PR body.
- Touching pushed tags, rewriting public history, committing binary media.
- Any AI attribution in commits or PR bodies (ADR-0002).

## Output contract
- Conventional Commits on a correctly named branch (WORKFLOW.md).
- Updated: CHANGELOG `[Unreleased]`, affected book chapters, rustdoc on new
  public items, `planning/STATUS.md`, workstream checklist ticked.

## Definition of done
- Full local gate green: fmt, clippy `-D warnings`, workspace tests, doc
  build, book build (if docs touched); workstream acceptance criteria met and
  demonstrated (command output or rendered artifact referenced in the PR).
- Escalate instead of guessing when: acceptance criteria are ambiguous, a
  needed API doesn't exist upstream (e.g. vello limitations), or a change
  would break the public schema without a migration path.
