## 0.2.1 - 2026-11-26

- **Antigravity SEP-1865 MCP-App Dashboard**: Added `ui://devcraft/dashboard.html` interactive resource and `devcraft_dashboard` tool (`devcraft-mcp`).
- **DJCraft-inspired Skin Engine (`skins.rs`)**: 9 curated studio palettes (including Carbon Deck and Shocking Pink Club, plus animated DJCraft backdrops), 5 card frame styles, luminous KPI deck with vector status LEDs, Today hero console strip, toolbar skin cycler + Sun/Moon toggle, and Settings picker persisted to `%LOCALAPPDATA%\devcraft\skin.json`.
- **Mobile CSS-variable themes & PWA**: Tailnet `devcraft-mcp serve` mobile `/todos` UI inherits the desktop skin palette and ships a PWA manifest.
- **Workspace root auto-discovery & `--root`**: `DEVCRAFT_ROOT` -> ancestor walk -> `D:\Dev\repos` fallback, Windows `\\?\` prefix stripped, global `--root <PATH>` CLI flag on `devcraft-mcp`.
- **Process Watchdog & Stale Claim Hygiene**: `devcraft_processes`, `devcraft_kill_stale_processes` (Services page), `devcraft_release_stale_claims` (>72h, Claims & Today pages), and enriched `devcraft_status`.
- **Personal todos & Tailnet companion**: `.devcraft/todos.json` (`devcraft_todo_add/list/done/clear`) and opt-in `devcraft-mcp serve` on Tailnet port `11259` with public `/api/health`.
