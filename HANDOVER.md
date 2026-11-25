---
updated: 2026-11-25T21:46:00+01:00
status: djcraft backdrops and palettes, mobile skins and PWA install
tags: [devcraft, cockpit, egui, craft-biz-ui, skins]
---

# Handover - DevCraft

## What was done

- v0.2.0 replaces the v0.1.0 multi-panel studio. The window is the craft shell: File, Edit, View, Fleet, Window, Help.
- Nineteen pages: Today, Graphs, Board, Git, GitHub, Docker, Reports, Craft apps, Upstream, Claims, Ports, Time machine, Sandbox, Services, Tasks, Logs, Telemetry, Workflows, LLM & Settings. Graphs measures commits, source lines, languages, tags, authors, and weekdays, and draws a night sky. It does not call `gh`. HelmCraft stays the company cockpit. A one-way engineering briefing is the later link, not a shared database.
- Window > Chat is the right dock. Skills `devcraft-workflow` and `devcraft-sops` are the preprompt and MCP `skill://` resources. Workflows scans craft freshness and queues SOP requests. It does not run assfix inside the process.
- Upstream shows the Storytold clones in `_upstream`: commits and the diff from the last 24 hours, the newest tag, and the top of `CHANGELOG.md`. It does not fetch.
- Today, Git, and GitHub draw the morning digest (`mcp-central-docs/operations/fleet-status.md`) as bar charts and tables before git finishes: replies, stale pull requests, stale issues, dirty trees, drift.
- A background thread reads sibling checkouts (`git status -sb` and the last commit) and fills the board. Docker is probed on its own thread. Smoke does not scan.
- Claims, pins, and 24-hour snoozes live in `{workspace}/.devcraft`. Holders are cursor, opencode, agy, and claude.
- Drag is a command. Review, Blocked, and Quiet are computed and a drag onto them is refused. Quiet cards stay hidden. The count is on the column chart.
- Ports opens on `WEBAPP_PORTS.md`, lists lock findings, and audit is the step that checks listeners.
- Tasks, Logs, and Telemetry read the scheduler, the known log files, and ports 12000-12008. The drawer shows the report or the log. Analysis is a line extract, not a model call.
- Time machine can snapshot one checkout. Sandbox runs one confirmed command off the UI thread. Craft apps can run `start.ps1 -Smoke` the same way.
- Today marks a digest file older than 26 hours. The board filters by holder and by the shell search. A column bar opens that column. Quiet stays off the board until its bar is clicked.
- A folder at the fleet root is not scanned when `_upstream/<name>` is a git checkout.
- The readme links `docs/screenshots` for Today, Graphs, and the board.
- The same functions are `devcraft-mcp tool <name>` and MCP 2026-07-28 over stdio. The desktop package lists 43 tools. There is no listening port.
- Commits `14d3349` and `5e61dbf` on `main` are the cockpit. On 2026-11-14 after the Upstream page: `cargo test -p devcraft-core -p devcraft-app` was 20 tests, clippy `-D warnings` was clean, and `devcraft-mcp tool devcraft_upstream` listed 20 Storytold clones. Smoke does not read `_upstream`.

- `scripts\release.ps1` with no `-Publish` staged `dist` (`devcraft.exe`, `devcraft-mcp.exe`, `devcraft-0.2.0-setup.exe`, portable zip, `devcraft-v0.2.0.mcpb`, `SHA256SUMS.txt`). Publish from a clean pushed `main` is still open.
- A personal todos layer outside the TaskCraft tracker: `crates/devcraft-core/src/todos.rs` stores `{workspace}/.devcraft/todos.json`, `devcraft_todo_add/list/done/clear` are MCP tools and CLI flags, and Today shows a You section with an add box and Done buttons. Time machine snapshots `todos.json` too.
- The golden Today test fails on machines without a GPU (`RequestDeviceError(Device Lost)`), with and without these changes. The ops newest-report test flakes when two fixture writes share a Windows mtime tick; it passes alone and on retry.

- The Tailnet companion `devcraft-mcp serve` (stage 2): hand-rolled tokio HTTP, no new deps, binds `127.0.0.1` plus the Tailscale IPv4 on the already-registered port 11259. Token in `.devcraft/mobile.token`, `?token=` or Bearer. `GET /todos` is a touch-sized page for iPad Safari, `/api/todos` is JSON, forms post urlencoded back to HTML. Verified live: JSON add/list/done/clear, form add/done, 401 without token. iPad 9th-gen (`100.120.39.78`) was online for this.
- 2026-11-25: ported DJCraft's six animated backdrops (Sparkle Club, Starfield Warp, Baroque Damask, Carbon Weave, Brushed Metal, Solid) with per-skin pairing, intensity slider, and repaint loop only for animated variants; added Carbon Deck and Shocking Pink palettes (9 total); golden screenshots pin Solid. Mobile companion now themes from CSS `:root` variables mapped to desktop `skin.json` with `?skin=` override and picker, plus `/manifest.json` and Apple metas for home-screen install. Verified: clippy clean, core+mcp 40+1 passed, app 5 passed, live curl of all pages + LCARS/Daylight switching.

## Last concrete action

2026-11-24 (agy): Fixed `resolve_workspace_root()` (`DEVCRAFT_ROOT` -> ancestor walk -> `D:\Dev\repos` fallback) and stripped Windows `\\?\` prefix; added `--root` CLI flag to `devcraft-mcp`; added `processes.rs` (`devcraft_processes`, `devcraft_kill_stale_processes`) and stale claim hygiene (`devcraft_release_stale_claims`, age badges in Claims); enriched `devcraft_status` with digest totals, stale counts, and serve port; added DJCraft-inspired 7-skin engine (`skins.rs`: Battle Console, Cockpit Dark, Club Midnight, Synthwave Outrun, Star Trek LCARS, Baroque Velvet & Gold, Daylight Studio) with 5 card frame decorations, luminous KPI deck, status LEDs, toolbar skin cycler, and Sun/Moon toggle. Verified: `devcraft-core` 40 passed, `devcraft-app` 5 passed (skipping `today_golden`), `cargo clippy --workspace --all-targets -- -D warnings` clean, release binaries installed and smoke-tested.

## Next step

`scripts\release.ps1 -Publish` from a clean pushed `main`. `CHANGELOG_LATEST.md` is the v0.2.0 note. The installer in `dist` is the older build until that command runs.

## Deliberately not done

- No TaskCraft issue create, edit, or status. The key is a label only.
- `devcraft_checkpoint_worktree` still snapshots a git worktree from CLI and MCP. The Time machine page snapshots cockpit state, or `HANDOVER.md` and `CHANGELOG.md` for one named checkout.
- The old port constants remain in `lib.rs`. Nothing binds them.
