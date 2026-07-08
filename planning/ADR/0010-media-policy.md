# ADR-0010 — Media policy: no new binary media in git

- **Status:** Accepted · **Date:** 2026-07-08

## Context
~18 MB of demo MP4/GIFs are tracked; history rewrite was considered and
rejected (public repo, disruption > benefit).

## Decision
Existing media stays; **new** demo media goes to GitHub Release assets (or
LFS if it must live in-tree). README may hotlink release assets.

## Consequences
The v0.3.0 release carries the showcase videos as assets — the pattern for
every future release.
