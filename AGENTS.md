# DevCraft — Sovereign Developer Cockpit & Fleet Hygiene Workstation

## Overview
DevCraft is a local-first developer cockpit and compound FastMCP server that consolidates fleet governance, git multi-repo monitoring, atomic backup snapshots, container sandbox runners, and host OS/port sentinels into a single native Rust + egui 0.37 desktop studio.

It serves as the reference implementation of the "Craft Bundler App" pattern, replacing micro-daemon sprawl with cohesive, intent-based developer workflows.

## Architecture & Crates
- `crates/devcraft-core`:
  - `fleet`: Recursive multi-repo workspace scanner, dirty working tree detector, and documentation conformance auditor (`AGENTS.md`, `CLAUDE.md`).
  - `backup`: Content-addressed time machine and timestamped `.bak` rollback ledger.
  - `ports`: Real-time TCP socket auditor cross-referenced with `mcp-central-docs/operations/WEBAPP_PORTS.md`.
  - `services`: Windows NSSM service and process sentinel enforcing safe restart protocols.
  - `sandbox`: Ephemeral Docker/Podman container test runner.
  - `meta`: Fleet-wide MCP capability search and tool discovery depot.
- `crates/devcraft-mcp`: FastMCP stdio JSON-RPC 2.0 server (`devcraft-mcp --mcp`) exposing compound agent tools.
- `crates/devcraft-app`: Native immediate-mode egui 0.37 desktop application with Fleet Radar, Time Machine, Sandbox Runner, and Port Sentinel panes.

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
