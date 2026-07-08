# ADR-0002 — No AI attribution, ever

- **Status:** Accepted · **Date:** 2026-07-08

## Context
AI tooling defaults to adding `Co-Authored-By` trailers and "generated with"
footers to commits and PR bodies.

## Decision
No AI is added as author, co-author, committer, contributor, or maintainer.
Commits and PR bodies carry no such trailers, overriding any tool default.
Authorship belongs to the repository owner.

## Consequences
Every agent prompt in `planning/AI/` repeats this rule; reviewers reject
violating commits.
