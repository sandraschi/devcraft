# DevCraft

![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)
![Rust 1.90+](https://img.shields.io/badge/rust-1.90%2B-orange?logo=rust)
![MCP stdio](https://img.shields.io/badge/MCP-stdio-purple)
![ArtCraft harness](https://img.shields.io/badge/ArtCraft-harness-green)

Local fleet cockpit for one person and four IDEs: Cursor, OpenCode, Antigravity, and Claude.

Status: **v0.2.0**. The window is the craft shell. It calls `devcraft-core` in-process. MCP stays on stdio. Nothing listens on a port.

CraftDeck installs apps. This checkout is the view of the fleet. TaskCraft keeps issues.

## Pages

Today, Board, Git, GitHub, Docker, Reports, Craft apps, Claims, Ports.

Today draws the morning digest (`mcp-central-docs/operations/fleet-status.md`) as charts and tables as soon as the window opens. Git status and Docker fill in behind that. Claims, pins, and snoozes are stored in `{workspace}/.devcraft`.

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
cargo run -p devcraft-mcp -- tool devcraft_status
```

`devcraft_refresh` scans sibling checkouts. `devcraft_status` does not.

## Layout

```text
devcraft/
  crates/devcraft-core/   board, digest, claims, ports, docker
  crates/devcraft-mcp/    CLI and MCP 2024-11-05 over stdio
  crates/devcraft-app/    egui window on craft-biz-ui
  docs/                   PRD and tier plan
```
