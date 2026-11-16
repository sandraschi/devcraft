---
name: devcraft-sops
description: Use when checking craft-app freshness or queueing assfix, gapfind, drift-scan, quality-check, or doc-sync.
---

# DevCraft SOPs and craft freshness

## Freshness

`devcraft_freshness` is read-only. For each craft checkout it reports whether a release exe, a debug exe, `CHANGELOG.md`, a help doc, and File/Help menu strings are present. It does not build.

`devcraft_craft_build` with `profile` `debug` or `release` runs `cargo build` in that checkout. It needs `confirm: true`. `devcraft_craft_smoke` runs `start.ps1 -Smoke`.

## Queued SOPs

`devcraft_workflow` writes a request under `.devcraft/workflows`. It does not edit the target repo. An agent runs the SOP. `kind` is one of:

- `craft-assess`: `patterns/CRAFT_APP_ASSESS_AND_FIX_SOP.md`
- `assfix`: `patterns/repo-assess-and-fix.md`
- `gapfind`: `patterns/GAPFIND_AUDIT_SOP.md`
- `drift-scan`: `patterns/DRIFT_SCAN_SOP.md`
- `quality-check`: `patterns/QUALITY_CHECK_SOP.md`
- `doc-sync`: `patterns/DOC_SYNC_SOP.md`

Pass the checkout name as `repo` and `confirm: true`.

Time machine snapshots use `devcraft_checkpoint` or `devcraft_checkpoint_repo`. A sandbox command uses `devcraft_sandbox_run` and also needs confirm.
