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

- no authentication or rate limiting,
- permissive CORS.

The v0.4 safety minimum is in place: request bodies are capped at 8 MiB,
`/render` asset paths are confined to `LUMINA_ASSET_ROOT` (default: the
server's working directory; traversal and absolute paths outside it are
rejected with 400), and bind/serve/response failures return errors instead
of panicking.

Run it only locally or behind a trusted reverse proxy. Full hardening
(auth, rate limiting, CORS allowlist, structured request logging) is
scheduled for v0.5 (see `planning/ROADMAP.md`); reports about these known
items are welcome but will be tracked against that plan rather than treated
as new advisories.

The CLI and library crates process untrusted scene files defensively (no
panics on malformed input); crashes or resource-exhaustion issues triggered by
crafted `.lsf` files are in scope and appreciated.
