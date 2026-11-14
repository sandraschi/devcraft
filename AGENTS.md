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
  - `ports`: the registry table, then an audit that binds each registered port.
  - `fleet`, `backup`, `services`, `sandbox`, `meta`: the v0.1.0 tools (audit, checkpoint, service restart, sandbox, discovery). They are callable. They have no page yet.
- `crates/devcraft-mcp`: CLI `devcraft-mcp tool <name>` and MCP 2024-11-05 (`--mcp`). Logs go to stderr.
- `crates/devcraft-app`: egui pages Today, Board, Git, GitHub, Docker, Reports, Craft apps, Claims, Ports. Menus are File, Edit, View, Fleet, Window, Help.

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
