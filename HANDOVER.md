---
updated: 2026-11-15T00:41:49+01:00
status: tier 4 checked; installer staged in dist; not published
tags: [devcraft, cockpit, egui, craft-biz-ui]
---

# Handover - DevCraft

## What was done

- v0.2.0 replaces the v0.1.0 multi-panel studio. The window is the craft shell: File, Edit, View, Fleet, Window, Help.
- Thirteen pages: Today, Board, Git, GitHub, Docker, Reports, Craft apps, Upstream, Claims, Ports, Time machine, Sandbox, Services.
- Upstream shows the Storytold clones in `_upstream`: commits and the diff from the last 24 hours, the newest tag, and the top of `CHANGELOG.md`. It does not fetch.
- Today, Git, and GitHub draw the morning digest (`mcp-central-docs/operations/fleet-status.md`) as bar charts and tables before git finishes: replies, stale pull requests, stale issues, dirty trees, drift.
- A background thread reads sibling checkouts (`git status -sb` and the last commit) and fills the board. Docker is probed on its own thread. Smoke does not scan.
- Claims, pins, and 24-hour snoozes live in `{workspace}/.devcraft`. Holders are cursor, opencode, agy, and claude.
- Drag is a command. Review, Blocked, and Quiet are computed and a drag onto them is refused. Quiet cards stay hidden. The count is on the column chart.
- Ports opens on `WEBAPP_PORTS.md`. Audit is the step that checks listeners.
- The same functions are `devcraft-mcp tool <name>` and MCP 2024-11-05 over stdio. The desktop package lists 23 tools (17 cockpit tools, including `devcraft_upstream`, plus the 6 older MCP tools). There is no listening port.
- Commits `14d3349` and `5e61dbf` on `main` are the cockpit. On 2026-11-14 after the Upstream page: `cargo test -p devcraft-core -p devcraft-app` was 20 tests, clippy `-D warnings` was clean, and `devcraft-mcp tool devcraft_upstream` listed 20 Storytold clones. Smoke does not read `_upstream`.

## Last concrete action

`scripts\release.ps1` with no `-Publish`. Fmt, clippy, and the workspace tests were green (app 3, core 21). `dist` holds `devcraft.exe`, `devcraft-mcp.exe`, `devcraft-0.2.0-setup.exe` (4.8 MB), the portable zip, `devcraft-v0.2.0.mcpb`, and `SHA256SUMS.txt`. The window had already been confirmed. One copy of `devcraft-mcp.exe` that was running from `target\release` was stopped so the link could replace it. The installed copy and the Claude extension copy were left running.

## Next step

Publish is a separate step: `scripts\release.ps1 -Publish`. It needs `CHANGELOG_LATEST.md`, a version bump if `v0.2.0` should not be reused, and a clean pushed `main`. Nothing in the tier plan is still open.

## Deliberately not done

- No TaskCraft issue create, edit, or status. The key is a label only.
- `devcraft_checkpoint_worktree` and `devcraft_sandbox_run` stay CLI and MCP tools. The new pages snapshot `.devcraft` state and only check whether Docker answers.
- The old port constants remain in `lib.rs`. Nothing binds them.
