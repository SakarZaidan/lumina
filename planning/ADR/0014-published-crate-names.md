# ADR-0014 — Published crate names are `luminafx-*`

- **Status:** Accepted · **Date:** 2026-09-03

## Context

Publishing to crates.io was blocked on a name check that had never been run.
Three of the names this workspace uses are already taken:

| Name | Owner on crates.io |
|---|---|
| `lumina` | an unrelated programming language |
| `lumina-core` | an unrelated GUI framework — *"wgpu rendering, Taffy layout"* |
| `lumina-cli` | the Celestia data-availability node |

`lumina-core` is the awkward one. It is not merely a name collision, it is a
collision in the same domain: a Rust crate that renders with wgpu, which would
sit immediately beside ours in every crates.io search for "lumina renderer".

It is also a *technical* collision, not only a presentational one. Publishing
our package as `luminafx-core` while leaving its library target named
`lumina_core` would mean two different crates both offering `use lumina_core::`,
which cannot coexist in one dependency graph without renaming one of them.

`lumina-engine` was free on crates.io but taken on both npm and PyPI, which
would have given the project two identities across three registries.

## Decision

Published packages take the prefix **`luminafx`**, which is free on crates.io,
npm, and PyPI. Library targets follow, so `luminafx-core` provides
`luminafx_core` — the Rust convention, and the only arrangement that avoids the
collision above.

Three things deliberately do **not** change:

- **The project is still Lumina.** The repository, the documentation, the book,
  the scene format, and the product name are unchanged. `luminafx` is a
  registry identity, not a rename.
- **Source directories stay `crates/lumina-*`.** Cargo does not require a
  directory to match its package name, and renaming nine directories would
  churn every path in CI, `xtask`, and the fuzz harness while moving no user
  closer to installing anything. The manifests state the published names.
- **The CLI binary stays `lumina-cli`.** Binary names are not globally unique,
  so nothing forced this one to change — but the package rename would have
  renamed it silently, because the binary name defaults to the package name.
  `tools/lumina-cli/Cargo.toml` now pins it in an explicit `[[bin]]` section
  with that reason written down.

## The SDKs had the same problem

Checking the crates surfaced two more names that could never have published,
neither of them a Rust crate:

- **PyPI**: `sdks/python/pyproject.toml` declared `lumina-engine`, which is
  taken. Now `luminafx`. The *module* stays `lumina`, so `import lumina` is
  unchanged — `[lib] name` is what a user imports, and module names are not
  globally unique, only distribution names are.
- **npm**: `sdks/javascript/package.json` declared `@lumina/sdk`, a scoped name
  requiring ownership of the `lumina` scope, which the project does not have.
  Now the unscoped `luminafx`. The README and the book advertised
  `npm install @lumina/sdk` for a package that could not have been published
  under that name by us.

## Consequences

- `cargo add luminafx-core` then `use luminafx_core::…`; the command is
  installed with `cargo install luminafx-cli` and invoked as `lumina-cli`.
- Internal path dependencies gained explicit `version` fields, which
  `cargo publish` requires and which path-only dependencies did not have. This
  was a second, independent blocker discovered by the same check.
- Crates must publish in dependency order: `schema` → `text` → `core` →
  `renderer` → `export` → `cli`. `luminafx-schema` is the only one that can be
  dry-run before any of them exist; the others resolve their dependencies from
  the registry and can only be verified once their dependencies are up.
- If the `lumina` name ever becomes available, moving to it is a new ADR and a
  major version, not a quiet rename.
