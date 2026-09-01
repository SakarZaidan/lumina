# 11 — Release and distribution

## Current state

This is the largest single gap in the project, and the cheapest to close.

**Nothing is published anywhere.** Not crates.io — so no docs.rs, no version
badge, no `cargo add lumina-core`. Not PyPI, despite a working PyO3 module.
Not npm, despite `README.md:474` telling people to run
`npm install @lumina/sdk`. The only installation path a visitor has is
`git clone && cargo build --release`, which additionally requires ffmpeg on
their PATH.

**Release automation is decided and unbuilt.** ADR-0011 accepts `release-plz`;
there is no `release-plz.toml` and no release workflow. `planning/STATUS.md`
records it as blocked on owner tokens. It is the only unchecked box left in
the v0.4 roadmap scope.

**Publish metadata is ready.** The five library crates plus the CLI carry full
metadata; `lumina-server` and `lumina-wasm` are `publish = false` by design
(ADR-0009). `Cargo.lock` is committed. MSRV is declared. The groundwork is
done — nothing has pulled the trigger.

**Supply-chain assurance is thin.** `cargo-deny` runs on push and PR only, so
a RUSTSEC advisory published during a quiet period goes unseen until someone
opens a PR. That is not theoretical: two advisories landed during seven idle
weeks and surfaced only because the v0.4 stack happened to run. There is no
SBOM, no provenance attestation, no signed tags, and GitHub Actions are pinned
to floating tags (`actions/checkout@v4`) rather than commit SHAs.

**No API stability machinery.** ENGINEERING_PRINCIPLES #10 promises semver
discipline and no silent breaking changes. Nothing checks:
`cargo-semver-checks` is not configured and there is no `cargo-public-api`
snapshot, so the promise rests entirely on reviewer attention.

**The JS SDK cannot build from a clean checkout** (TD-12): the `../wasm/`
import does not exist because nothing wires `wasm-pack` output into the
package build.

## Target

Three ecosystems, one command each, published automatically from a green tag,
with a supply chain someone auditing the project can verify.

## Work items

| ID | Item | Acceptance |
|---|---|---|
| `AAA-REL-01` | `release-plz.toml` + release workflow (ADR-0011) | A merge to main opens a release PR; merging it publishes |
| `AAA-REL-02` | First crates.io publish of the five libraries and the CLI | `cargo add lumina-core` works; docs.rs builds |
| `AAA-REL-03` | Repair the JS SDK build (TD-12): wire `wasm-pack` into the package, add a CI job | `npm pack` works from a clean checkout |
| `AAA-REL-04` | Publish `@lumina/sdk` to npm | The README instruction becomes true |
| `AAA-REL-05` | Python SDK to PyPI via maturin; expose webm/gif (TD-13) | `pip install lumina` works on three platforms |
| `AAA-REL-06` | `cargo-dist` for prebuilt CLI binaries | Linux/macOS/Windows binaries attached to each release |
| `AAA-REL-07` | Scheduled `cargo-deny` and `osv-scanner`, opening an issue on failure | An advisory on a quiet week is noticed within a day |
| `AAA-REL-08` | SBOM, SLSA provenance, signed tags, SHA-pinned actions | Verifiable from outside the project |
| `AAA-REL-09` | CodeQL and OpenSSF Scorecard, badge in the README | Score published, not just claimed |
| `AAA-REL-10` | `cargo-semver-checks` + `cargo-public-api` snapshot in CI | A breaking change cannot merge without a version bump |
| `AAA-REL-11` | `cargo-llvm-cov` + Codecov badge | Coverage is a number, not "n/a" |
| `AAA-REL-12` | Versioned documentation deploys | v0.4 docs stay reachable after v0.5 ships |

## What the owner has to do

Two things cannot be automated from inside the repository, and both should be
done early because everything downstream waits on them:

1. A crates.io API token in repository secrets, for `release-plz`.
2. A PyPI trusted-publisher configuration and an npm token.

Until those exist, `AAA-REL-01` can be built and tested but not exercised.

## Metrics moved

CI / release engineering (80 → 97), and Ecosystem — which sits at 30 almost
entirely because of this document.

## Sequencing

`AAA-REL-01` and `02` immediately after the v0.4 tag: publishing is what turns
the work already done into something people can use, and it does not depend on
any later wave. The rest through Wave 5. `AAA-REL-10` should land before the
LSF v2 schema change in Wave 6, so the breaking change is caught by machinery
rather than by memory.
