---
updated: 2026-11-14T11:30:00+01:00
status: cockpit shipped, window not yet looked at by a person this session
tags: [devcraft, cockpit, egui, craft-biz-ui]
---

# Handover - DevCraft

## What was done

- v0.2.0 replaces the v0.1.0 multi-panel studio. The window is the craft shell: File, Edit, View, Fleet, Window, Help.
- Nine pages: Today, Board, Git, GitHub, Docker, Reports, Craft apps, Claims, Ports.
- Today, Git, and GitHub draw the morning digest (`mcp-central-docs/operations/fleet-status.md`) as bar charts and tables before git finishes: replies, stale pull requests, stale issues, dirty trees, drift.
- A background thread reads sibling checkouts (`git status -sb` and the last commit) and fills the board. Docker is probed on its own thread. Smoke does not scan.
- Claims, pins, and 24-hour snoozes live in `{workspace}/.devcraft`. Holders are cursor, opencode, agy, and claude.
- Drag is a command. Review, Blocked, and Quiet are computed and a drag onto them is refused. Quiet cards stay hidden. The count is on the column chart.
- Ports opens on `WEBAPP_PORTS.md`. Audit is the step that checks listeners.
- The same functions are `devcraft-mcp tool <name>` and MCP 2024-11-05 over stdio. The desktop package lists 22 tools. There is no listening port.
- Commits `14d3349` and `5e61dbf` on `main`. On 2026-11-14: 18 tests, clippy `-D warnings`, and `cargo run -p devcraft-app -- --smoke` were green (46 craft apps, digest present, menus in English and German).

## Last concrete action

Docs, changelog, PRD, and tier plan rewritten to this cockpit. The v0.1.0 handover (ports 11258 and 11259, Fleet Radar panes) is stale.

## Next step

Open `cargo run -p devcraft-app` from this repo and look at Today. The digest tables should be there before the git meter finishes.

Then Tier 3 in `docs/TIERED_PLAN.md`: holder swimlanes, a commit-age chart, and pages for the tools that still have no window (time machine, sandbox, service restart).

## Deliberately not done

- No 14-day flow chart, commit heatmap, or swimlanes.
- No TaskCraft key on a card. No git worktree child cards. No end-of-day bulk release.
- No golden screenshots.
- `devcraft_checkpoint_worktree`, `devcraft_sandbox_run`, and `devcraft_restart_service` are CLI and MCP tools. They have no page.
- The old port constants remain in `lib.rs`. Nothing binds them.
