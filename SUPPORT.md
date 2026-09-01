# Getting help

## Start here

- **[The book](https://sakarzaidan.github.io/lumina/)** — getting started, the
  scene format, visual effects, events, the schema reference, and the AI
  integration cookbook. This is the canonical documentation (ADR-0006).
- **[`examples/`](examples/)** — nine scenes and two generators, each indexed
  in [examples/README.md](examples/README.md) with the command that renders it.
- **`lumina-cli --check --scene your.lsf`** — validates a scene and prints
  every finding, each with a `fix_suggestion`. Most authoring problems answer
  themselves here.

## Where to ask

| You want to | Go to |
|---|---|
| Ask a question, share a scene, propose an idea | [Discussions](https://github.com/SakarZaidan/lumina/discussions) |
| Report something that does not work as documented | [Open an issue](https://github.com/SakarZaidan/lumina/issues/new/choose) |
| Report a vulnerability | **Not** an issue — see [SECURITY.md](SECURITY.md) |
| Propose a change to the schema, the `Renderer` trait, a server endpoint, or an SDK surface | An RFC — [planning/RFCS/](planning/RFCS/) |
| Contribute code | [CONTRIBUTING.md](CONTRIBUTING.md) |

## What makes a report useful

A minimal `.lsf` scene that reproduces the problem is worth more than any
description of it — scenes are data, so a reproduction is a paste, not a
project. Include your OS, whether you used `--backend skia` or `--backend
vello`, and your Rust version.

If rendering produced wrong pixels rather than an error, say what you expected
to see. "The gradient is solid white" is actionable; "gradients are broken" is
not.

## Response times

This is a small project. Issues and discussions are usually answered within a
week; security reports within a few days. If something has gone quiet for
longer than that, a polite bump on the thread is welcome rather than rude.

## Support scope

The supported version is the latest release, currently the `0.4.x` line. Older
versions receive fixes only for security issues, per [SECURITY.md](SECURITY.md).

Lumina is pre-1.0: the schema can still change between minor versions, always
with a migration note in [CHANGELOG.md](CHANGELOG.md) and never silently
(principle #10).
