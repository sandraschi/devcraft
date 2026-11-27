# DevCraft

![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)
![Rust 1.90+](https://img.shields.io/badge/rust-1.90%2B-orange?logo=rust)
![MCP stdio](https://img.shields.io/badge/MCP-stdio-purple)
![ArtCraft harness](https://img.shields.io/badge/ArtCraft-harness-green)

## Install

Tell Cursor, Claude, Antigravity, or OpenCode: **Install DevCraft from `https://github.com/sandraschi/devcraft`**

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\install-craft.ps1
```

Installs the Windows app and the CLI (Start Menu shortcut, CLI on the user PATH). `-RegisterClients` also registers the MCP exe with the IDEs that are installed. `-Clients cursor,claude-desktop` limits that list. `-Mcpb` installs the Claude Desktop extension bundle.

## Window

![Today](docs/screenshots/today.png)

Today is the morning digest. The board and the reports are already on disk.

![Graphs](docs/screenshots/graphs.png)

Graphs is the night sky of the fleet. A circle is a checkout. Size is commits over 26 weeks.

![Board](docs/screenshots/board.png)

The board is the live kanban. Click a bar to open that column.

Local fleet cockpit for one person and four IDEs: Cursor, OpenCode, Antigravity, and Claude.

Status: **v0.3.1**. The window is the craft shell. It calls `devcraft-core` in-process. MCP stays on stdio. Nothing listens on a port except the opt-in `devcraft-mcp serve` Tailnet companion.

CraftDeck installs apps. This checkout is the view of the fleet. TaskCraft keeps issues.

## Pages

Today, Graphs, Board, Git, GitHub, Docker, Reports, Craft apps, Upstream, Claims, Ports, Time machine, Sandbox, Services, Tasks, Logs, Telemetry, Workflows, Memory, LLM & Settings.

Window > Chat opens the right-hand dock. The prompt is the bundled skills. Graphs measures commits, source lines, languages, and tags. Workflows checks craft-app freshness and queues SOP requests. Memory probes the advanced-memory daemon (read-only, port 10732) and hosts the dev-focus tools: focus, find, recent, reveal, doctor.

Upstream is the Storytold clones in `_upstream`: commits and the diff from the last 24 hours, the newest tag, and the changelog tail. It does not fetch.

Today draws the morning digest (`mcp-central-docs/operations/fleet-status.md`) as a luminous DJCraft-inspired KPI deck, hero console strip, charts, and tables as soon as the window opens. Git status and Docker fill in behind that. Claims, pins, snoozes, and personal todos are stored in `{workspace}/.devcraft`.

## Skins & Visuals

DevCraft includes a DJCraft-inspired skin engine (`crates/devcraft-app/src/skins.rs`) with 9 curated palettes (**Battle Console**, **Cockpit Dark**, **Club Midnight**, **Carbon Deck**, **Shocking Pink Club**, **Synthwave Outrun**, **Star Trek LCARS**, **Baroque Velvet & Gold**, and high-contrast **Daylight Studio**), 5 card frame decorations (**Bold Drop Shadow**, **Neon Outer Glow**, **LCARS Starfleet Bracket**, **Baroque Double Frame**, **Clean Flat**), and 6 animated background backdrops (**Sparkle Club**, **Starfield Warp**, **Baroque Damask**, **Carbon Weave**, **Brushed Metal**, **Solid**). Switch live from the top bar (`Skin` cycler + Sun/Moon toggle) or **LLM & Settings** (palette, frame, backdrop, intensity).

## Feature Matrix

| Area | Status | Notes |
|---|---|---|
| Morning digest, Today console, luminous KPIs & charts | Implemented | Reads `fleet-status.md` immediately; flags digests >26h |
| Live Fleet Kanban Board (drag, filter, worktrees, snooze) | Implemented | Background git scan; claims/pins/snoozes in `.devcraft/` |
| DJCraft-inspired Skin Engine (9 palettes, 5 frames, 6 backdrops) | Implemented | Persisted in `%LOCALAPPDATA%\devcraft\skin.json`; Daylight >= 7:1 |
| Stale Claim Hygiene & Process Watchdog | Implemented | `devcraft_release_stale_claims`, `devcraft_processes`, `devcraft_kill_stale_processes` |
| Personal Todos & Opt-in Tailnet iPad Companion | Implemented | `.devcraft/todos.json`; `devcraft-mcp serve` on Tailnet port `11259` |
| Upstream Storytold Tracker, Graphs, Ports, Services, Sandbox | Implemented | Read-only `_upstream` scan; `sc.exe` service control; confirmed sandbox |
| Dev-focus QoL tools (focus, find, recent, reveal, doctor) | Implemented | One-call claim+pin+open; fuzzy find; recent commits; Explorer reveal; env check |
| Memory page + read-only ADN connector | Implemented | Daemon health on `127.0.0.1:10732`, `adn_search` via streamable HTTP; never opens the DB |
| Local Ollama Chat Dock (`Ctrl+J`) & Bundled Skills | Implemented | Read-only tools auto-run; mutating tools require confirmation |
| TaskCraft issue create/edit sync | Planned | Card keys are local labels in `.devcraft/tasks.json` |

## Docs

- [PRD](docs/PRD.md)
- [Tier plan](docs/TIERED_PLAN.md)
- [Handover](HANDOVER.md)
- [Changelog](CHANGELOG.md)

## Commands

```powershell
cargo run -p devcraft-app -- --smoke
cargo run -p devcraft-app
cargo run -p devcraft-mcp -- --mcp
cargo run -p devcraft-mcp -- --root D:\Dev\repos tool devcraft_status
cargo run -p devcraft-mcp -- tool devcraft_upstream
cargo run -p devcraft-mcp -- tool devcraft_processes
cargo run -p devcraft-mcp -- tool devcraft_doctor
cargo run -p devcraft-mcp -- tool devcraft_find --query craft
cargo run -p devcraft-mcp -- tool devcraft_recent --hours 24
cargo run -p devcraft-mcp -- tool devcraft_adn_status
cargo run -p devcraft-mcp -- tool devcraft_adn_search --query fleet --limit 2
cargo run -p devcraft-mcp -- serve
```

`devcraft_refresh` scans sibling checkouts. `devcraft_status` does not (it returns cached root, digest totals, stale claims, open todos, and serve port immediately). `devcraft_upstream` reads `_upstream` and does not fetch. `devcraft-mcp serve` is opt-in and binds localhost + Tailscale IPv4 on port `11259`.

## Layout

```text
devcraft/
  crates/devcraft-core/   board, digest, claims, processes, todos, ports, docker, adn
  crates/devcraft-mcp/    CLI, MCP 2026-07-28 over stdio, and Tailnet serve companion
  crates/devcraft-app/    egui window on craft-biz-ui + DJCraft skin engine
  docs/                   PRD and tier plan
```
