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
- permissive CORS,
- no request body-size limit on the CPU-heavy `/render` endpoint,
- scene asset paths are read from the server's filesystem without an
  allowlist.

Run it only locally or behind a trusted reverse proxy. Hardening is scheduled
(see `planning/ROADMAP.md`, v0.4 safety minimum and v0.5 production
hardening); reports about these known items are welcome but will be tracked
against that plan rather than treated as new advisories.

The CLI and library crates process untrusted scene files defensively (no
panics on malformed input); crashes or resource-exhaustion issues triggered by
crafted `.lsf` files are in scope and appreciated.
