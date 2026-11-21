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


## Documentation preservation and hooks

On 2026-11-20 a README "standardization" sweep across the fleet deleted project descriptions,
status sections and user warnings in nine repos. Full rules:
`mcp-central-docs/standards/CONTENT_PRESERVATION_STANDARD.md`.

- Applying any standard or template restructures; it never deletes. Move content (within the file
  or to `docs/` with a link), do not drop it. Never remove what the project is, what it is based on,
  its status (what works, what does not, what is not built) or warnings to users.
- Remove content only when Sandra explicitly asks for that removal, in this repo.
- Never add claims you have not verified (release downloads, versions, test counts, features).
- Only standards with `status: active` may be applied across repos. A change across several repos is
  done on one repo first and approved by Sandra; its commits name the tool and carry a `Sweep-Id:` line.
- `.githooks/commit-msg` blocks a commit that deletes a Markdown file, removes 20% of its lines
  (15 or more) or removes 3 or more headings. A removal Sandra asked for passes with
  `Docs-Removal-Approved: <who and why>` in the message. Enable once per clone:
  `git config core.hooksPath .githooks`. Never use `--no-verify`; if a hook blocks you, stop and ask.
