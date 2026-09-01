# 12 — Community and governance

## Current state

The written-down parts are strong. `CONTRIBUTING.md` gives setup, MSRV, branch
naming, the exact pre-PR gate, and real conventions — including the six places
a new object type must be handled and the rule that rendering changes need a
pixel test. `CODE_OF_CONDUCT.md` is Contributor Covenant 2.1 with a real
enforcement contact. There are issue templates, a PR template with a genuine
checklist, `CODEOWNERS`, and dependabot. `SECURITY.md` documents limitations
rather than hiding them. An RFC process gates public-API changes.

The lived parts are empty.

**Zero issues have ever been filed.** Not one, open or closed. The roadmap and
nineteen debt items live only in Markdown, so there is no `good first issue`,
no `help wanted`, no milestone, no board. A person who reads the README,
likes it, and wants to help has nothing to pick up.

**One contributor, self-merging.** 73 commits, effectively all from one author
plus dependabot. `planning/AI/WORKFLOW.md` describes `main` as "protected in
spirit" — which is to say not protected. Branch protection has never been
configured.

**Four dependabot PRs sat open for weeks** with no triage, and two of them
were superseded by work in the stack.

**Discovery was impossible until now.** Description, topics, and homepage were
all empty; the book at `sakarzaidan.github.io/lumina` was not linked from the
repository header. One star. (Fixed in Wave 0.)

**Governance documents that a serious project has are missing:** no
`GOVERNANCE.md` (who decides, how a maintainer is added), no `MAINTAINERS.md`,
no `SUPPORT.md`, no `CITATION.cff`, no funding config.

**Contribution has friction the docs cannot remove.** Five commands to retype
before every PR; no `justfile` or `xtask`; no pre-commit hooks; no
devcontainer for a toolchain spanning ffmpeg, wasm-pack, mdbook and maturin.

## Target

Someone who wants to contribute can find something to do in under a minute,
run the full gate with one command, and understand who decides what.

## Work items

| ID | Item | Acceptance |
|---|---|---|
| `AAA-COM-01` | `GOVERNANCE.md`: decision rights, maintainer path, RFC gate, release authority, breaking-glass procedure | A contributor knows how a decision gets made |
| `AAA-COM-02` | `MAINTAINERS.md`, `SUPPORT.md`, `CITATION.cff`, `.github/FUNDING.yml` | The standard set is complete |
| `AAA-COM-03` | Label taxonomy: `good first issue`, `help wanted`, `area/*`, `kind/*`, `difficulty/*` | Every issue is filterable |
| `AAA-COM-04` | Milestones for v0.5, v0.6, v1.0, and a project board | Roadmap phases are visible outside Markdown |
| `AAA-COM-05` | Convert the roadmap and debt register into issues | Nineteen debt items become nineteen tickets |
| `AAA-COM-06` | Seed 15–20 scoped `good first issue` tickets from the subplans | Each has a file path, an acceptance test, and an estimate |
| `AAA-COM-07` | `CONTRIBUTING.md` gains DCO sign-off, review SLA, label guide, first-contribution walkthrough, reviewer checklist mapped to the 13 principles | A first PR can be written without asking anything |
| `AAA-COM-08` | `cargo xtask ci` as the single gate command | Replaces the five-command list |
| `AAA-COM-09` | Commit-message linting and pre-commit config | Conventional Commits enforced |
| `AAA-COM-10` | Devcontainer | Contribution without local toolchain installs |
| `AAA-COM-11` | Enable Discussions; triage dependabot weekly | A question has somewhere to go |
| `AAA-COM-12` | Branch protection on `main` | See below |

## Branch protection

Configured **last**, in Wave 8, once the v0.4 stack has landed — enabling it
earlier would have made those merges impossible. The chosen configuration is
strict: required status checks, linear history, no force-push, no deletions,
`enforce_admins: true`, and one required approving review.

That last setting means the sole maintainer cannot merge their own work
unassisted. That is a deliberate trade — it is the configuration that makes
the repository's rules real rather than advisory — but it needs an escape
hatch, so `GOVERNANCE.md` will carry a clearly-labelled **breaking-glass**
section with the exact `gh api` calls to lift protection and restore it. The
procedure is documented, not hidden, so using it leaves a trace.

## The honest read

Governance here is already better than most projects with a hundred times the
reach. What is missing is not policy — it is *people having something to do*.
`AAA-COM-05` and `AAA-COM-06` matter more than the rest of this document
combined.

## Metrics moved

Ecosystem (30 → 95), jointly with [11](11-release-distribution.md) and
[14](14-playground-tooling.md).

## Sequencing

`AAA-COM-01`–`07` and `11` in Wave 1, immediately after the release: the point
of publishing is to be findable, and the point of being findable is that
someone can help. `08`–`10` in Wave 1 as well. `12` in Wave 8.
