# ADR-0007 — AI agent prompts live in planning/AI/, not .claude/agents/

- **Status:** Accepted · **Date:** 2026-07-08

## Reasons
`.claude/` is gitignored (carving out an exception invites committing local
state); prompts should be tool-agnostic and PR-reviewable. Thin
`.claude/agents/` wrappers can point here later if native subagents are
wanted.
