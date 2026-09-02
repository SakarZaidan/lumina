# Fuzz targets

Four targets over the inputs Lumina accepts from somewhere it does not
control. Each one is a parser reachable from a scene file or an HTTP request.

| Target | Input | Why it is here |
|---|---|---|
| `scene_json` | Arbitrary bytes → `Scene` → validation → timeline | The front door. A server accepts this from the network |
| `svg_path` | SVG path data | Index-walked token stream — the shape that panics on truncation. Reachable from a scene *and* from an untrusted SVG asset |
| `latex_unicode` | LaTeX source | Reads brace groups and scripts by index; unbalanced input is the interesting case |
| `plot_expression` | `Plot.function_str` | Reaches a recursive-descent parser straight from the scene |

`scene_json` also builds the timeline and scene graph for anything that
validates, because both run before a renderer ever sees the scene — a panic
there is reachable from exactly the same input.

## Running

This is a **separate workspace**: `libfuzzer-sys` needs a nightly toolchain and
sanitizer flags, so it is excluded from the root `Cargo.toml` and never seen by
`cargo check --workspace`.

```bash
cargo install cargo-fuzz
cargo +nightly fuzz run scene_json -- -max_total_time=60
cargo +nightly fuzz list          # all targets
```

## What counts as a finding

A panic, a hang, or an out-of-memory. Returning `None`, an `Err`, or a
validation error for malformed input is **correct behaviour**, not a finding —
these parsers are supposed to reject things.

Resource limits deserve a note. `validate_scene_data` enforces bounds on canvas
size, frame count, sample counts, tick counts, and group depth (see
`SECURITY.md`). A fuzz input that trips one of those and returns an error is
the system working. A fuzz input that *reaches* one of those limits and hangs
or dies anyway is a real finding, and the reason `scene_json` runs validation
rather than stopping at deserialisation.

## Corpus

Seed corpora live in `corpus/<target>/` and are not committed — they grow
large, and ADR-0010 keeps binary artefacts out of git. Seed a run from the
repository's own scenes:

```bash
mkdir -p corpus/scene_json
cp ../examples/*.lsf ../crates/lumina-renderer/tests/fixtures/*.lsf corpus/scene_json/
```

Starting from real scenes rather than random bytes gets past the "is this even
JSON" stage immediately, so the fuzzer spends its time on the logic instead of
on the parser's front gate.

## Crashes

A reproducer is written to `artifacts/<target>/`. Minimise it, then commit it
as a regression test in the crate that owns the code — not here. A fuzz finding
that only lives in `fuzz/artifacts/` is a finding that will be reintroduced.
