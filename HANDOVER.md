---
updated: 2026-11-14T22:44:24+01:00
status: tier 3 checked, window not yet looked at by a person
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

Today light and dark goldens are in `crates/devcraft-app/tests/snapshots/`. `today_golden_light_and_dark` passed against them. The full suite was 24 tests. Clippy `-D warnings` was clean. The shots use a fixed digest, so the numbers stay at 3 open PRs and 1 open issue.

## Next step

Open `cargo run -p devcraft-app` from this repo and look at the window. Tier 4 is still open: a person confirms Today, the board, and Help. The `mcpb` manifest still says 22 tools.

## Deliberately not done

- No TaskCraft issue create, edit, or status. The key is a label only.
- `devcraft_checkpoint_worktree` and `devcraft_sandbox_run` stay CLI and MCP tools. The new pages snapshot `.devcraft` state and only check whether Docker answers.
- The old port constants remain in `lib.rs`. Nothing binds them.
