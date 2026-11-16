# DevCraft - fleet cockpit

## Overview

Local cockpit for one person working across the fleet with Cursor, OpenCode, Antigravity, and Claude. Version 0.2.0. The window is the craft shell and calls `devcraft-core` in-process. MCP is stdio. The product does not listen on a port.

Read `HANDOVER.md` before starting, then `docs/PRD.md` and `docs/TIERED_PLAN.md`.

## Architecture and crates

- `crates/devcraft-core`:
  - `cockpit`: one state object for the window, the CLI, and MCP. Claims, pins, and snoozes live in `{workspace}/.devcraft`.
  - `board`: column rules. Review, Blocked, and Quiet refuse a drag.
  - `digest`: parse `operations/fleet-status.md`. This crate does not call `gh`.
  - `handover`: the next step under a heading that starts with "next".
  - `ports`: the registry table, an audit that binds each registered port, and the last lock findings.
  - `ops`: scheduled fleet tasks, fleet and IDE MCP logs, and the monitoring stack on ports 12000-12008.
  - `fleet`, `backup`, `services`, `sandbox`, `meta`: audit, checkpoint, service restart, sandbox, and discovery. Time machine, Sandbox, and Services are pages.
- `crates/devcraft-mcp`: CLI `devcraft-mcp tool <name>` and MCP 2024-11-05 (`--mcp`). Logs go to stderr.
- `crates/devcraft-app`: egui pages Today, Graphs, Board, Git, GitHub, Docker, Reports, Craft apps, Upstream, Claims, Ports, Time machine, Sandbox, Services, Tasks, Logs, Telemetry, Workflows, LLM & Settings. Menus are File, Edit, View, Fleet, Window, Help. Window > Chat is the right-hand dock. Upstream reads `_upstream` craft clones and does not fetch.
- `skills/`: `devcraft-workflow` and `devcraft-sops`, compiled into the binary and served over MCP as `skill://` resources.

## Transport
- The desktop window calls `devcraft_core` in-process.
- MCP and the CLI are stdio. There is no listening port.
- `devcraft-mcp tool devcraft_refresh` scans the fleet. `devcraft_status` does not.

## Commands
```powershell
# Headless smoke test
cargo run -p devcraft-app -- --smoke

# Run Desktop UI
cargo run -p devcraft-app

# Start FastMCP stdio server
cargo run -p devcraft-mcp -- --mcp
```
