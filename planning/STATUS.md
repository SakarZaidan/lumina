# Current State

## Health dashboard

Updated with every entry below (and re-verified at every release). 🟢 healthy
· 🟡 known gaps, tracked · 🔴 broken/blocked.

| Area | | Notes |
|---|---|---|
| CI on `main` | 🟢 | all 8 jobs green (release run `9c35474`) |
| Tests | 🟢 | 92 + 3 wasm passing; zero flakes |
| Coverage | 🟡 | not measured yet — tooling v0.4 |
| Benchmarks | 🟡 | exist, manual only; not in CI (TD-14) |
| Docs (book + rustdoc) | 🟢 | book live on Pages, current for v0.3.0 |
| Examples | 🟢 | all render on any OS; OFL font bundled (TD-16 closed, #10) |
| Security | 🟡 | server unhardened pre-v0.5 by design (TD-09, SECURITY.md) |
| Backend parity | 🟡 | vello gaps documented; fix is v0.4 flagship (TD-01) |
| Release | 🟢 | v0.3.0 tagged + GitHub Release with assets |
| Dependencies | 🟢 | deny green; dependabot active (5 PRs pending triage) |

Rolling log, newest first. One dated entry per work session; ≤ 10 lines each.
For the release-by-release story see [HISTORY.md](./HISTORY.md).

---

## 2026-07-12 (later) — Pixel-diff parity harness live (TD-11)

- PR [#11](https://github.com/SakarZaidan/lumina/pull/11) (stacked on #10):
  cross-backend harness renders 8 fixtures on Skia + Vello, AA-aware
  comparator (3×3 neighborhood rescue + mean-delta tint check), failure
  artifacts to `target/parity-failures/` and CI upload.
- First real catch, fixed in-PR: Vello stroked with kurbo's round caps/joins
  vs Skia's butt/miter — all GPU line ends and sharp corners diverged.
- CI test job now installs lavapipe and sets `LUMINA_REQUIRE_VELLO=1`
  (missing adapter = failure, not silent skip).
- New debt TD-18: duplicated text layout paths (Skia inline vs raster.rs
  bitmap); text fixture carries a wider tolerance until unified (v0.5).
- WS-02 → In progress. Next: `common/` extraction (TD-02).

## 2026-07-12 — v0.4 kickoff: bundled OFL font (TD-16)

- v0.4 execution started per ROADMAP/WS-02; PR sequence planned A–J
  (font → parity harness → common/ extraction → vello parity → easing
  strictness → server safety → CI matrix → rustdoc → dep bumps).
- PR [#10](https://github.com/SakarZaidan/lumina/pull/10): Liberation Sans
  2.1.5 (SIL OFL 1.1) bundled at `examples/assets/fonts/`; all scenes/docs
  off `/usr/share/fonts`. Closes TD-16.
- Latent bug found + fixed in-PR: hello/circle_bounce/pythagorean declared no
  font asset — their text never rendered (no system-font fallback exists).
- Local gate green: fmt, clippy `-D warnings`, 92/92 tests, rustdoc, mdBook.

## 2026-07-08 (evening) — Constitution, RFC/ADR system, metrics, diagrams

- Added the constitution set at root: VISION.md, DESIGN.md,
  ENGINEERING_PRINCIPLES.md (linked from README and CONTRIBUTING).
- DECISIONS.md split into per-decision `planning/ADR/0001–0011`; DECISIONS.md
  is now the index. New `planning/RFCS/` process gates public-API changes.
- New `planning/METRICS.md` (measured v0.3.0 snapshot + quality scorecard)
  and the health dashboard above; both are release-checklist duties now.
- New `planning/ECOSYSTEM.md` (layers 0–3, what the core owes the ecosystem).
- `docs/architecture/`: gen-diagrams.sh renders the crate dependency graph
  from `cargo metadata` + 4 hand-maintained pipeline diagrams; embedded in
  the book's architecture chapter.

## 2026-07-08 (later) — v0.3.0 released

- PR #2 merged to `main` as `9c35474`; CI fully green (a fresh RUSTSEC batch
  was fixed in-flight: anyhow → 1.0.103, crossbeam-epoch → 0.9.20,
  ttf-parser unmaintained ignored + registered as TD-17).
- GitHub Pages enabled by the owner; book live at
  <https://sakarzaidan.github.io/lumina/> including the new Events chapter.
- Tags pushed: `v0.1.0` (02b92da), `v0.2.0` (596b847), `v0.3.0` (9c35474);
  GitHub Release v0.3.0 created with showcase media as assets (ADR-0010).
- WS-01 complete. Next up: WS-02 backend parity (v0.4) — see ROADMAP.

## 2026-07-08 — Repo audit, planning system, hygiene batch

- Full three-track engineering audit completed (core crates, tooling/CI, docs/git).
- This planning system created; `todo.md` retired into [ROADMAP.md](./ROADMAP.md);
  blueprint and history moved under `planning/`.
- Hygiene batch in flight on `feat/v0.3.0-enhancements`: metadata fixes, crate
  rustdoc, README/CHANGELOG repair, community health files, mdBook v0.3.0 refresh.
- Git state at session start: `origin/main` = `7af221a` (v0.3.0 merge, **CI red
  6/8** — fixed by the two `ci:` commits on this branch); no tags existed.
- Batch complete: 10 commits; local gate green (fmt, clippy `-D warnings`,
  92/92 tests, rustdoc clean, mdBook 0.4.40 builds).
- MSRV probed: **1.88** (1.78 and 1.85 fail on locked deps `home`/`image`);
  declared as `rust-version` and reflected in the README badge.
- Next: PR → green CI on `main` → tag `v0.1.0` (02b92da) / `v0.2.0` (596b847)
  backdated + `v0.3.0` on the green merge → GitHub Release for v0.3.0.
- Blocked on repo owner: enable GitHub Pages (Settings → Pages → Source:
  GitHub Actions) so the deploy-docs job can publish the book.
- Version: 0.3.0 across workspace and both SDK manifests (drift fixed).
