---
updated: 2026-11-16T23:55:07+01:00
status: nineteen pages; readme shows the window; v0.2.0 ready to publish
tags: [devcraft, cockpit, egui, craft-biz-ui]
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
- The same functions are `devcraft-mcp tool <name>` and MCP 2024-11-05 over stdio. The desktop package lists 43 tools. There is no listening port.
- Commits `14d3349` and `5e61dbf` on `main` are the cockpit. On 2026-11-14 after the Upstream page: `cargo test -p devcraft-core -p devcraft-app` was 20 tests, clippy `-D warnings` was clean, and `devcraft-mcp tool devcraft_upstream` listed 20 Storytold clones. Smoke does not read `_upstream`.

## Last concrete action

`scripts\release.ps1` with no `-Publish`. Fmt, clippy, and the workspace tests were green (app 3, core 21). `dist` holds `devcraft.exe`, `devcraft-mcp.exe`, `devcraft-0.2.0-setup.exe` (4.8 MB), the portable zip, `devcraft-v0.2.0.mcpb`, and `SHA256SUMS.txt`. The window had already been confirmed. One copy of `devcraft-mcp.exe` that was running from `target\release` was stopped so the link could replace it. The installed copy and the Claude extension copy were left running.

## Next step

`scripts\release.ps1 -Publish` from a clean pushed `main`. `CHANGELOG_LATEST.md` is the v0.2.0 note. The installer in `dist` is the older build until that command runs.

## Deliberately not done

- No TaskCraft issue create, edit, or status. The key is a label only.
- `devcraft_checkpoint_worktree` still snapshots a git worktree from CLI and MCP. The Time machine page snapshots cockpit state, or `HANDOVER.md` and `CHANGELOG.md` for one named checkout.
- The old port constants remain in `lib.rs`. Nothing binds them.
