# Security Policy

## Reporting a vulnerability

Please report vulnerabilities privately via
[GitHub Security Advisories](https://github.com/SakarZaidan/lumina/security/advisories/new)
— do **not** open a public issue. You should receive a response within a few
days. Include a minimal reproduction where possible.

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.3.x   | ✅        |
| < 0.3   | ❌        |

## Known limitations — `lumina-server`

The HTTP server (`crates/lumina-server`) is **not hardened for untrusted
networks** in the current release, by design and documented intent:

- ~~no authentication or rate limiting~~ — both exist as of v0.5:
  `LUMINA_API_TOKEN` requires a bearer token (compared without leaking its
  contents through timing), and `LUMINA_RATE_LIMIT` caps requests per client
  per minute.
- ~~permissive CORS~~ — cross-origin access is now closed unless
  `LUMINA_CORS_ORIGINS` lists an origin. It previously used
  `CorsLayer::permissive()`, which let any website a developer happened to
  visit drive their local render server.
- The default bind address is `127.0.0.1`, not `0.0.0.0`. Starting the server
  used to publish a render endpoint to every interface, which made "I started
  the dev server" and "I exposed this machine's CPU to the network"
  indistinguishable.

What still stands: the rate limiter is per-process and keyed by peer address,
so it does not survive a restart and cannot see through a proxy that does not
set the peer. Anything multi-node belongs behind a gateway that does this
properly.

The v0.4 safety minimum is in place: request bodies are capped at 8 MiB,
`/render` asset paths are confined to `LUMINA_ASSET_ROOT` (default: the
server's working directory; traversal and absolute paths outside it are
rejected with 400), and bind/serve/response failures return errors instead
of panicking.

Since v0.4.1 a scene is also a **bounded** computation. The body cap limited
how much a request could *say*; it did not limit how much work a request could
*ask for* — `{"duration": 1e9, "fps": 240}` is thirty bytes and describes
2.4 x 10^11 frames. `lumina_core::validation` now rejects, with a structured
error, any scene exceeding:

| Bound | Limit |
|---|---|
| Canvas dimension | 16 384 px per side |
| Frame rate | 240 fps |
| Duration | 86 400 s |
| Total frames (`duration x fps`) | 1 000 000 |
| `Plot.sample_count` | 100 000 |
| `Plot.function_str` | 4 096 bytes |
| `Particles.count` | 1 000 000 |
| Derived tick count (`Axes`, `NumberLine`) | 100 000 |
| Group nesting depth | 256 |

Non-positive and non-finite tick steps are rejected outright: `x_step: 0.0`
previously produced `inf as i32`, which saturates to `i32::MAX` and ran a
stroked-path loop 2.1 billion times per frame on both backends.

Group nesting is bounded because the depth check runs during *validation*,
before any render limit could apply. A straight chain of groups contains no
cycle, so cycle detection alone never terminated it, and 8 MiB of JSON encodes
roughly 150 000 levels — enough to overflow the stack and abort the process.
The renderers carry the same limit independently, since `lumina-renderer` is a
public API that can be called without validating first.

These limits are enforced in `lumina-core`, so every consumer inherits them:
the server, the CLI, and both SDKs.

Hardening landed in v0.5 (TD-09): bearer authentication, per-client rate
limiting, a CORS allowlist, a loopback default bind, and graceful shutdown.
What remains is documented rather than implied away — the rate limiter is
per-process and keyed by peer address, so it does not survive a restart and
cannot see through a proxy that does not set the peer. Anything multi-node
belongs behind a gateway that already does this properly.

The CLI and library crates process untrusted scene files defensively (no
panics on malformed input); crashes or resource-exhaustion issues triggered by
crafted `.lsf` files are in scope and appreciated.

## Verifying a release

Every binary attached to a release carries **signed build provenance**: a
statement, recorded in GitHub's public transparency log, that those exact bytes
came out of this repository's release workflow at a named commit. You do not
have to trust the download, the mirror it came through, or us:

```bash
gh attestation verify lumina-cli-v0.5.0-x86_64-unknown-linux-gnu.tar.gz \
  --repo SakarZaidan/lumina
```

A checksum sits beside each archive for the simpler case:

```bash
sha256sum -c lumina-cli-v0.5.0-x86_64-unknown-linux-gnu.tar.gz.sha256
```

Each release also carries an **SBOM** (`lumina-<version>.spdx.json`) listing
every crate that went into it, so when an advisory lands you can answer "am I
affected?" by reading a file rather than by rebuilding the workspace to find
out.

This is the half of supply-chain security that points outward. Pinned actions,
`cargo-deny`, `osv-scanner` and `cargo-machete` protect what goes *into* a
build; provenance and the SBOM are what let somebody else check what came out.

### Signed tags — not yet

Tags are **not** currently signed. `git verify-tag v0.5.0` reports
`no signature found`, and saying otherwise here would be the kind of claim this
project's own rules forbid. Signing needs a key the maintainer holds and this
repository cannot create for itself; when one exists, `git config
user.signingkey` plus `git tag -s` is the whole change, and this section will
say so with a version it is true of.

In the meantime the **provenance attestation** above is the stronger guarantee
anyway: it is produced by the workflow rather than by a person, it is recorded
in a public transparency log, and verifying it needs nobody's public key.
