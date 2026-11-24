---
name: devcraft-workflow
description: Use when operating DevCraft pages, the fleet board, ports, tasks, logs, or telemetry.
---

# DevCraft workflow

DevCraft is the local fleet cockpit. The window, the CLI, and MCP call the same tools. Nothing listens on a port except the opt-in `devcraft-mcp serve` Tailnet companion (`/todos`, `/api/todos`, public `/api/health`).

## Pages

Today reads `operations/fleet-status.md`. A file older than 26 hours is marked stale.
Board columns are Inbox, Ready, Active, Review, Blocked, Done, and Quiet. Drag is `devcraft_move`. Review, Blocked, and Quiet refuse a drag. A worktree card id contains `#` and follows its checkout.
Git, GitHub, and Docker show the digest and the live scan. `devcraft_refresh` starts the git scan. `devcraft_docker` probes the daemon.
Craft apps lists checkouts whose names end in craft. Upstream reads `_upstream` and does not fetch.
Claims, pins, and TaskCraft keys live in `.devcraft`. A key such as BC-12 is a label. It does not create an issue.
Ports lists `WEBAPP_PORTS.md` and lock findings. `devcraft_ports` is the listener audit.
Tasks, Logs, and Telemetry come from `devcraft_tasks`, `devcraft_logs`, and `devcraft_telemetry`. `devcraft_analyze` extracts failure lines from a `task:` or `log:` id.
Graphs is `devcraft_graphs`. It counts commits, source lines, languages, tags, authors, and weekdays, and draws a star per active checkout. It does not edit a checkout. It does not call `gh`.

## Answering

Call `devcraft_status` before guessing counts. Call `devcraft_skill_read` to list or open a skill. Quote the tool result you used. Say when a source is missing.
