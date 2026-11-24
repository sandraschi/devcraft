# DevCraft Help & Operations Guide

DevCraft (`v0.2.0`) is the local fleet cockpit for one developer and four IDEs: **Cursor**, **OpenCode**, **Antigravity**, and **Claude**. It unifies repository health, a live Kanban board, git and GitHub digests, `_upstream` Storytold tracking, port and Windows NSSM service sentinels, time-machine snapshots, Docker sandboxes, craft-app freshness checks, and an embedded Ollama chat dock (`Ctrl+J`) into one pure-Rust desktop workstation (`#![forbid(unsafe_code)]`), CLI, and MCP `stdio` server.

---

## 1. What DevCraft Is & Does & Get Started

DevCraft reads your fleet workspace (`d:\Dev\repos` or `DEVCRAFT_ROOT`) directly from disk and local tools. The desktop window (`devcraft-app`) calls `devcraft-core` in-process, while `devcraft-mcp` exposes the same cockpit tools over CLI and MCP 2026-07-28 `stdio`. Nothing listens on a TCP port.

### 5-Step First-Run Tour

1. **Launch & Inspect Today**: Start `devcraft` (`cargo run -p devcraft-app` or the installed binary). The **Today** page immediately parses `mcp-central-docs/operations/fleet-status.md` into charts and tables. Any digest older than 26 hours is flagged stale.
2. **Scan Sibling Checkouts**: Click **Refresh** in the top bar (or run `File > Refresh fleet` / `devcraft_refresh`) to scan all sibling repositories on a background thread. Git branch, dirty/untracked counts, ahead/behind, `.git/index.lock`, worktrees, and `HANDOVER.md` next steps populate **Board**, **Git**, and **Graphs**.
3. **Triage on the Fleet Board**: Open **Board** to see checkouts grouped into `Inbox`, `Ready`, `Active`, `Review`, `Blocked`, `Done`, and `Quiet`. Click any card to open the detail drawer, attach a TaskCraft key (e.g., `BC-12`), pin, snooze, or claim it for an IDE.
4. **Open in Your IDE & Checkpoint**: In the detail drawer, click **Open** (Cursor, OpenCode, Antigravity, or Claude) to claim the checkout in `.devcraft/claims.json` and launch the editor. Before risky edits, use **Time machine** (`devcraft_checkpoint`) to snapshot modified files.
5. **Ask the Fleet Copilot (`Ctrl+J`)**: Press `Ctrl+J` (or **Window > Chat**) to open the right-hand Ollama chat dock. It is pre-loaded with DevCraft's bundled skills (`devcraft-workflow`, `devcraft-sops`) and live fleet status, and asks for confirmation before running any mutating tool.

---

## 2. Functions

DevCraft is organized into three crates:

- **`devcraft-core`**: Pure-Rust domain engine (`#![forbid(unsafe_code)]`):
  - **`cockpit`**: Unified state and tool dispatcher (`devcraft_status`, `devcraft_refresh`, `devcraft_board`, `devcraft_git`, `devcraft_digest`, `devcraft_reports`, `devcraft_crafts`, `devcraft_upstream`, `devcraft_brief`, `devcraft_claim`, `devcraft_release`, `devcraft_release_all`, `devcraft_release_stale_claims`, `devcraft_processes`, `devcraft_kill_stale_processes`, `devcraft_task`, `devcraft_todo_add`, `devcraft_todo_list`, `devcraft_todo_done`, `devcraft_todo_clear`, `devcraft_snapshots`, `devcraft_checkpoint`, `devcraft_checkpoint_repo`, `devcraft_sandbox`, `devcraft_sandbox_run`, `devcraft_tasks`, `devcraft_logs`, `devcraft_telemetry`, `devcraft_analyze`, `devcraft_lock`, `devcraft_skill_read`, `devcraft_freshness`, `devcraft_graphs`, `devcraft_workflow`, `devcraft_craft_build`, `devcraft_craft_smoke`, `devcraft_service`, `devcraft_restart`, `devcraft_pin`, `devcraft_move`, `devcraft_docker`, `devcraft_ports`, `devcraft_open`, `devcraft_reveal`).
  - **`processes`**: Detects running DevCraft processes and terminates duplicate or >=12h stale `devcraft-mcp` workers while protecting the current PID.
  - **`todos`**: Personal todo storage in `.devcraft/todos.json` with due-date sorting and prefix completion.
  - **`board`**: Computes Kanban columns (`inbox`, `ready`, `active`, `review`, `blocked`, `done`, `quiet`) from working-tree facts and enforces drag/move rules.
  - **`fleet` & `graphs`**: Scans workspace checkouts, counts 26-week commit activity, source lines, languages, tags, authors, and weekday distributions.
  - **`ports`**: Reconciles `mcp-central-docs/operations/WEBAPP_PORTS.md` and `fleet-start.config.ps1` files against live TCP listeners (`127.0.0.1`).
  - **`services`**: Queries Windows services via `sc.exe` and performs safe NSSM restarts (`sc.exe stop` -> `sc.exe start` -> verify port listener).
  - **`backup`**: Atomic file snapshots stored in `.devcraft` (Time Machine).
  - **`sandbox`**: Executes commands inside Docker containers or isolated local subprocesses with explicit confirmation.
  - **`freshness` & `workflows`**: Audits `*craft` apps for release/debug binaries, `CHANGELOG.md`, `HELP.md`/docs, and menu bar compliance, and queues SOP requests under `.devcraft/workflows/`.
  - **`skills`**: Embeds `skills/devcraft-workflow/SKILL.md` and `skills/devcraft-sops/SKILL.md` for both MCP `resources/read` and the `Ctrl+J` chat preprompt.
- **`devcraft-mcp`**: Universal CLI (`status`, `audit`, `snapshot`, `ports`, `discover`, `skills`, `tool`) and MCP 2026-07-28 `stdio` JSON-RPC server (`--mcp`).
- **`devcraft-app`**: 19-view `egui` desktop workstation built on `craft-biz-ui` (`Today`, `Graphs`, `Board`, `Git`, `GitHub`, `Docker`, `Reports`, `Craft apps`, `Upstream`, `Claims`, `Ports`, `Time machine`, `Sandbox`, `Services`, `Tasks`, `Logs`, `Telemetry`, `Workflows`, `LLM & Settings`).

---

## 3. Usage (GUI, CLI & MCP)

### Desktop GUI (`devcraft`)

- **Navigation Rail**: Switch across all 19 views. **Fleet** and **View** menus also jump directly to any page.
- **Command Palette (`Ctrl+K`)**: Fuzzy-search any cockpit tool or UI action and execute it with schema-generated forms.
- **Chat Dock (`Ctrl+J` / `Window > Chat`)**: Select a local Ollama model in **LLM & Settings**, ask questions about stale repos, failing logs, or port conflicts, and approve/deny any write tool call inline.
- **Headless Smoke & Screenshots**:
  ```powershell
  cargo run -p devcraft-app -- --smoke
  cargo run -p devcraft-app -- --screenshot target\shots\today.png --view 0
  ```

### CLI (`devcraft-mcp`)

Run subcommands directly from PowerShell or scripts with JSON output:

```powershell
# Workspace & port audits
cargo run -p devcraft-mcp -- status
cargo run -p devcraft-mcp -- audit
cargo run -p devcraft-mcp -- ports
cargo run -p devcraft-mcp -- discover mcp

# Cockpit tools
cargo run -p devcraft-mcp -- tool devcraft_status
cargo run -p devcraft-mcp -- tool devcraft_refresh
cargo run -p devcraft-mcp -- tool devcraft_upstream
cargo run -p devcraft-mcp -- tool devcraft_claim --repo devcraft --holder cursor

# Bundled skills
cargo run -p devcraft-mcp -- skills list
cargo run -p devcraft-mcp -- skills show devcraft-workflow
```

### MCP `stdio` Server (`devcraft-mcp --mcp`)

Configure your IDE (`Cursor`, `Antigravity`, `Claude`, `OpenCode`) to launch `devcraft-mcp.exe --mcp` over `stdio`:

- **Protocol**: MCP `2026-07-28` (`initialize`, `tools/list`, `tools/call`, `resources/list`, `resources/read`).
- **Stdout discipline**: `stdout` is 100% JSON-RPC; all diagnostics go to `stderr`.
- **Resources**: Exposes `skill://devcraft-workflow/SKILL.md` and `skill://devcraft-sops/SKILL.md`.

---

## 4. Tips & Tricks

- **Fast Read-Only Status**: Call `devcraft_status` before `devcraft_refresh` when you only need cached counts, claims, stale alerts, and digest age - `devcraft_status` does not walk all sibling git directories.
- **Skins & Daylight Mode**: Use the top-bar `Skin` button + Sun/Moon toggle (or **LLM & Settings**) to switch across 7 DJCraft-inspired studio palettes and 5 card frame decorations.
- **Stale MCP Process Cleanup**: Open **Services** (or run `devcraft-mcp tool devcraft_kill_stale_processes`) to inspect running `devcraft-mcp` instances and stop stale duplicates without affecting the active session.
- **Upstream Never Fetches**: `Upstream` and `devcraft_upstream` read local git history in `_upstream/*` (last 24h commits, diff stats, latest tag, `CHANGELOG.md` tail) without touching the network.
- **Keyboard-First Navigation**:
  - `Ctrl+K`: Open Command Palette
  - `F1`: Open Keyboard Shortcuts cheat sheet
  - `Ctrl+J`: Toggle the right-hand Ollama Chat Dock
  - `Ctrl+,`: Open **LLM & Settings** (Preferences)
  - `Esc`: Close any open modal or Help Center window
- **Log & Task Triage**: In **Tasks** or **Logs**, select any entry and run `devcraft_analyze` (using its `task:` or `log:` id) to extract failure lines and stack traces without opening huge log files manually.
- **Non-Destructive SOP Queueing**: `devcraft_workflow` writes a markdown request to `.devcraft/workflows/` and never edits the target repository directly.

---

## 5. Common Pitfalls & Safety

- **Port Conflicts (`WEBAPP_PORTS.md`)**: Never hardcode a port. Always check the **Ports** page (`devcraft_ports`) and claim ports via `mcp-central-docs/fleet-gate/claim_ports.py`, which cross-checks `WEBAPP_PORTS.md` and every `fleet-start.config.ps1`.
- **NSSM Service Restarts**: Never restart an NSSM-managed Windows service by killing its child process (`taskkill` / `Stop-Process`) — NSSM will respawn or orphan state. Use `Services` (`devcraft_restart`) or `sc.exe stop <name>` followed by `sc.exe start <name>`, then verify a **new PID** owns the expected port.
- **Worktree & Git Hygiene**:
  - Linked git worktrees appear as child cards (`<repo>#<worktree>`). They follow their parent checkout's column and intentionally refuse independent Kanban drags.
  - Columns `Review`, `Blocked`, and `Quiet` are computed from working-tree state (dirty files, unpushed commits, `.git/index.lock`, unmerged paths) and cannot be overridden by dragging.
  - A stale `.git/index.lock` forces a repo into `Blocked` until cleared.
- **Commit Size & Backup Discipline**: Keep commits to a **maximum of 10 files** per commit and per script invocation. When editing 3+ files, create timestamped `.bak` backups first or run `devcraft_checkpoint` / `devcraft_checkpoint_repo`.
- **Safety Gates**: Mutating operations (`devcraft_workflow`, `devcraft_craft_build`, `devcraft_sandbox_run`) require explicit `confirm: true` (and UI arming) before execution. All crates enforce `#![forbid(unsafe_code)]` and deny `unwrap_used`, `expect_used`, and `panic`.

---

## 6. Planned Extensions

- **Live Telemetry Auto-Thresholds**: Configurable alert thresholds on the **Telemetry** view (`12000–12008` stack) with one-click jump to matching **Logs** entries.
- **Multi-Provider Chat Routing**: Optional fallback beyond local Ollama (`127.0.0.1:11434`) in **LLM & Settings** while keeping local-first defaults.
- **Worktree Diff Preview**: Inline diff summary in the detail drawer for `<repo>#<worktree>` child cards before launching an IDE.
- **Batch Freshness Runner**: One-click sequential `--smoke` verification across all stale `*craft` apps with summary export.

---

## 7. Other Crafts

| Craft / Component | Role in Ecosystem | How DevCraft Interacts |
| :--- | :--- | :--- |
| **`CraftDeck`** (`craft-deck`) | Installs and updates Craft desktop apps and MCP bundles | Lists installed apps; DevCraft monitors their freshness |
| **`TaskCraft`** (`taskcraft`) | Issue & task tracker across the fleet | DevCraft attaches TaskCraft keys (e.g. `BC-12`) to board cards without creating duplicate issues |
| **`craft-biz` / `craft-biz-ui`** | Shared `egui` shell, menu bar bookends, command palette, Help Center | `devcraft-app` runs on `craft-biz-ui`'s `Shell` and `SuiteApp` trait |
| **`craft-sandbox-vbox`** | Isolated VM/sandbox execution environment | Complementary to DevCraft's Docker/subprocess **Sandbox** view |
| **`_upstream/*`** | Read-only Storytold reference clones (`celcraft`, `cadcraft`, etc.) | Tracked on **Upstream** (`devcraft_upstream`); never modified directly |
