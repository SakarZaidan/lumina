# Governance

How decisions get made in Lumina, who makes them, and how that changes over
time. This document describes the project's *operation*;
[VISION.md](VISION.md) describes what it is for and
[ENGINEERING_PRINCIPLES.md](ENGINEERING_PRINCIPLES.md) describes how it is
built.

## Current structure

Lumina is a **maintainer-led** project with a single maintainer
([MAINTAINERS.md](MAINTAINERS.md)). That is a description, not an ambition:
the project is young, and pretending to a committee it does not have would be
its own kind of dishonesty.

The structure below is written now, while it is easy, so that it does not have
to be invented under pressure later.

## Decision rights

| Decision | Who | How |
|---|---|---|
| Bug fixes, tests, docs, refactors | Any contributor | Pull request, one maintainer approval |
| New features within an accepted roadmap phase | Any contributor | Pull request; a design comment on the issue first is welcome |
| Changes to the LSF schema, the `Renderer` trait, server endpoints, or SDK surfaces | Maintainers | **RFC required** — [planning/RFCS/](planning/RFCS/) — before implementation |
| Anything contradicting [VISION.md](VISION.md)'s permanent principles | Nobody | Those five principles are fixed. Changing one is forking |
| Roadmap phase contents and ordering | Maintainers | [planning/ROADMAP.md](planning/ROADMAP.md), re-evaluated at each release boundary |
| Architectural decisions with lasting consequences | Maintainers | ADR in [planning/ADR/](planning/ADR/), indexed in DECISIONS.md |
| Releases and version numbers | Release manager | [planning/AI/WORKFLOW.md](planning/AI/WORKFLOW.md) |
| Security advisories and disclosure | Maintainers | [SECURITY.md](SECURITY.md) — privately, first |

When a decision is contested and consensus does not emerge, the maintainer
decides, and writes down why. An unrecorded decision is not a decision — it is
something the next person has to rediscover.

## Becoming a maintainer

There is no quota and no timetable. The path is:

1. **Sustained, reviewed contribution.** Roughly five or more merged pull
   requests of substance, across more than one area of the codebase.
2. **Demonstrated judgement.** Reviews that catch real problems; issues scoped
   well enough that someone else could pick them up; PRs that update
   `TECH_DEBT.md` and `STATUS.md` without being asked.
3. **Alignment with the principles.** Not agreement on everything — the
   ability to argue a position and then implement the decision that was made.
4. **Nomination and invitation.** An existing maintainer proposes it in a
   public issue; existing maintainers agree; the invitation is offered.

Maintainers who step back are moved to an emeritus section in
`MAINTAINERS.md` with thanks, and lose write access. This is administrative,
not a judgement, and it is reversible.

## What a maintainer commits to

- Reviewing pull requests within roughly a week, or saying they cannot.
- Keeping the planning system honest — `STATUS.md`, `TECH_DEBT.md`, and
  `METRICS.md` updated by the change that affects them (principle #13).
- Not merging their own non-trivial work without a second pair of eyes, when a
  second pair of eyes exists.
- Enforcing the [Code of Conduct](CODE_OF_CONDUCT.md).

## Branch protection

`main` is protected: required status checks, linear history, no force-pushes,
no deletions, enforcement including administrators, and one required approving
review.

That last rule is deliberate and it has a cost: while the project has a single
maintainer, it means **the maintainer cannot merge their own work
unassisted**. The alternative — protection that exempts the person most able
to bypass it — is protection in name only, and this project's principles are
meant to bind rather than decorate.

### Breaking glass

There are legitimate reasons to lift protection: an incident, a release that
must ship, a solo maintainer with no reviewer available. The procedure is
documented rather than hidden, so that using it leaves a trace.

```bash
# Inspect the current protection before changing anything.
gh api repos/SakarZaidan/lumina/branches/main/protection > /tmp/protection-backup.json

# Lift the review requirement only (status checks stay enforced).
gh api -X DELETE repos/SakarZaidan/lumina/branches/main/protection/required_pull_request_reviews

# ... merge the thing that needed merging ...

# Restore it.
gh api -X PATCH repos/SakarZaidan/lumina/branches/main/protection/required_pull_request_reviews \
  -F required_approving_review_count=1 \
  -F dismiss_stale_reviews=true \
  -F require_code_owner_reviews=false
```

Whoever breaks glass records it in `planning/STATUS.md` with the reason. Never
disable status checks — they are the only thing standing between `main` and a
regression, and no deadline is worth them.

## Changing this document

Through a pull request, like anything else, with maintainer approval. Changes
to decision rights or the maintainer path also need an ADR, because they are
exactly the kind of decision whose reasoning the next person will need.
