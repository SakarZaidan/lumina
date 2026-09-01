# 07 — UI, UX, developer experience

Lumina has no GUI, and that is a deliberate product decision (VISION.md). Its
interface surfaces are therefore the CLI, the errors it prints, the book, the
schema, and the player SDK. "UI/UX" here means those, treated with the care a
GUI would get.

## Current state

### The CLI is a single flat command

`tools/lumina-cli/src/main.rs` is 331 lines with six flags: `--scene`,
`--output`, `--format`, `--backend`, `--watch`, `--verbose`, `--check`.
`--format` and `--backend` are plain strings, not `ValueEnum`, so a typo is a
runtime error rather than a completion candidate. There is no `new`, no
`inspect`, no `fmt`, no way to print the schema, and no shell completions or
man page. Width, height, and fps cannot be overridden without editing the
scene.

Watch mode always writes a single mid-point PNG (`main.rs:233`), ignoring
`--format`, and it swallows failures: `if let Ok(data) = fs::read(...) { let _ = load_font(...) }`
at `:279-286`. A missing or corrupt font produces a preview with no text and
**no message** — while the non-watch path hard-errors on exactly the same
condition (`:120`). Two behaviours for one fault.

### Errors are excellent in one place and absent everywhere else

`/validate` returns `code`, `path`, `message`, and `fix_suggestion` — a genuine
machine contract, and the reason the AI loop works. But a validation error
pointing at `$.timeline[2].object` is a JSONPath string, not a location: the
user still has to find line 340 of their scene by hand. Nothing renders a
source span the way `rustc` underlines the offending token.

### Onboarding costs more than it should

`CONTRIBUTING.md:32-38` lists five commands a contributor must retype before
every PR. There is no `justfile`, no `Makefile`, no `xtask`, no pre-commit
config, and no devcontainer for a toolchain that spans ffmpeg, wasm-pack,
mdbook, and maturin. There is no `rust-toolchain.toml`, which matters a lot
when CI sets `RUSTFLAGS: -D warnings` globally — a new stable release can turn
a fresh lint into a red build on a repository nobody touched.

### The book is good and static

Ten chapters, current, honest about gaps. But it has no versioned deploy (only
"latest"), no recipes or cookbook section, no searchable schema reference
beyond the generated dump, and no way to see a scene without cloning the repo
and installing ffmpeg. The rustdoc does not include the README via
`#![doc = include_str!]`, so docs.rs and GitHub tell slightly different stories.

### The README overclaims installability

`npm install @lumina/sdk` (`README.md:474`) for a package TD-12 describes as
unbuildable from a clean checkout, and a `maturin develop` path for a Python
SDK that is not on PyPI. It also carries a second "Phase 1–8" roadmap
(`:552`) that conflicts with ADR-0005, counts 27 easings in one place and 28
in another against a registry of 33, and reports "92 tests" where there are
120.

## Target

A CLI that feels like a modern Rust tool, diagnostics that point at the
problem, one command to run the full gate, and documentation that lets someone
succeed without cloning anything.

## Work items

| ID | Item | Acceptance |
|---|---|---|
| `AAA-DX-01` | `cargo xtask ci` runs fmt, clippy, tests, wasm, book, and examples exactly as CI does | One command in `CONTRIBUTING.md` replaces five |
| `AAA-DX-02` | `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `.editorconfig`, `taplo.toml` | A fresh clone formats and lints identically to CI |
| `AAA-DX-03` | CLI subcommands: `render`, `validate`, `new`, `preview`, `schema`, `inspect`, `fmt` | `lumina --help` reads like a tool, not a flag list |
| `AAA-DX-04` | `ValueEnum` for `--format`/`--backend`; `--width/--height/--fps` overrides | Invalid values rejected at parse time with the valid set listed |
| `AAA-DX-05` | Shell completions and a man page, generated in the build | `lumina render --f<TAB>` completes |
| `AAA-DX-06` | Progress reporting on long renders | Frame count, rate, and ETA on a TTY; silent when piped |
| `AAA-DX-07` | Source-span diagnostics into the scene file | A validation error shows the offending JSON line with a caret |
| `AAA-DX-08` | Watch mode honours `--format`, watches asset files, and reports load failures | The two font-failure behaviours become one |
| `AAA-DX-09` | `--json` machine output on every command | An agent parses CLI output without scraping prose |
| `AAA-DX-10` | Every README claim verified; the second roadmap deleted | See [08](08-code-quality.md#documentation-divergences) |
| `AAA-DX-11` | Cookbook chapter: ten recipes from "fade in text" to "animated chart" | Each recipe is a runnable `.lsf` in `examples/` |
| `AAA-DX-12` | Versioned book deploys; `#![doc = include_str!("../README.md")]` on crate roots | docs.rs and the book agree |
| `AAA-DX-13` | Devcontainer covering ffmpeg, wasm-pack, mdbook, maturin | A contributor is productive without local installs |

`AAA-DX-07` is the highest-leverage item here. The schema is the contract and
the errors are already structured; turning a JSONPath into a line, column, and
caret is the difference between a good error and an actionable one — and it
serves humans and agents equally.

## Metrics moved

Developer experience (82 → 96), Documentation (85 → 97).

## Sequencing

`AAA-DX-01`, `02`, `10` in Wave 1 — they are the floor everything else stands
on. `03`–`09` in Wave 7 as one coherent CLI release, so the interface changes
once rather than six times. `11`–`13` alongside.
