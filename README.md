# DevCraft 🛠️

![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)
![Rust 1.90+](https://img.shields.io/badge/rust-1.90%2B-orange?logo=rust)
![MCP stdio](https://img.shields.io/badge/MCP-stdio-purple)
![ArtCraft harness](https://img.shields.io/badge/ArtCraft-harness-green)

**Sovereign Developer Cockpit & Compound FastMCP Server for Fleet Governance.**

Part of the **Sovereign Craft Suite** and registered in [CraftDeck](file:///d:/Dev/repos/craft-deck).

---

## What is DevCraft?

DevCraft is the first reference implementation of a **Craft Bundler App**. Instead of forcing AI agents and developers to coordinate across 6+ disparate micro-MCP servers (`git-github-mcp`, `docker-mcp`, `multi-backup-mcp`, `meta_mcp`, `sysinternals-mcp`, `repomix-mcp`), DevCraft bundles these capabilities into:

1. **A Native Desktop Studio (`crates/devcraft-app`)**:
   - Built in **Rust + egui 0.37**.
   - **Fleet Radar**: Live monitoring of 280+ repositories, uncommitted changes, and `AGENTS.md` documentation coverage.
   - **Time Machine**: Atomic file and worktree snapshot ledger with rolling restore points.
   - **Container Sandbox**: Isolated Docker/Podman test execution with instant stdout/stderr inspection.
   - **Port Sentinel**: Live monitoring of TCP ports `10700–15000` cross-checked against `WEBAPP_PORTS.md`.
   - **NSSM Service Sentinel**: Safe Windows service control enforcing fleet restart protocols.
2. **A Compound FastMCP Server (`crates/devcraft-mcp`)**:
   - Implements MCP 2024-11-05 standard over stdio.
   - High-level intent-based tools (`devcraft_audit_fleet`, `devcraft_checkpoint_worktree`, `devcraft_sandbox_run`, `devcraft_restart_service`, `devcraft_audit_ports`, `devcraft_discover_tools`).

---

## Ports & Fleet Configuration

- **Frontend Port**: `11258`
- **Backend / MCP Port**: `11259`
- Registered in: `mcp-central-docs/operations/WEBAPP_PORTS.md`

---

## Quick Start

```powershell
# Headless smoke test
cargo run -p devcraft-app -- --smoke

# Run the native desktop studio
cargo run -p devcraft-app

# Run as FastMCP stdio server
cargo run -p devcraft-mcp -- --mcp

# Run CLI commands
cargo run -p devcraft-mcp -- status
cargo run -p devcraft-mcp -- audit
cargo run -p devcraft-mcp -- ports
cargo run -p devcraft-mcp -- discover audio
```

---

## Architecture

```
devcraft/
├── Cargo.toml                    # Rust workspace
├── AGENTS.md                     # Per-directory agent instructions
├── README.md                     # Documentation
├── fleet-start.config.ps1        # Fleet port configuration
├── justfile                      # Task automation
└── crates/
    ├── devcraft-core/            # Shared engine (fleet, backup, ports, services, sandbox, meta)
    ├── devcraft-mcp/             # MCP 2024-11-05 stdio server & CLI
    └── devcraft-app/             # egui 0.37 native desktop studio
```
