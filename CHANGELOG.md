# Changelog

## 0.3.1 - 2026-11-27

- **Backdrop no longer hides shell chrome**: `paint_backdrop` paints via the central panel Ui (panel rect + panel painter, behind page content) instead of the full-window background layer. Animated styles stay inside the content area; menu bar, nav rail, status bar, and detail dock are untouched.
- **Freeze toggle**: `SkinState.animate_backdrop` (default on, backfilled for existing `skin.json`) + settings checkbox. Uncheck to still animated styles on one frame and stop the 120ms repaint loop.
- **Regression test**: `backdrop_never_paints_outside_central_rect` renders all six styles headless and asserts every emitted shape stays clipped to the central rect.

## 0.3.0 - 2026-11-26

- **Dev-focus QoL pack**: `devcraft_focus` (claim + pin + open in one call), `devcraft_find` (fuzzy repo search), `devcraft_recent` (commits in last N hours), `devcraft_reveal_repo` (open checkout in Explorer), `devcraft_doctor` (root, scan, digest, tool bins). CLI `tool` gains `--query/--hours/--limit`.
- **Memory page (20th view, Ops group)**: Dev-focus buttons, `advanced-memory-mcp` daemon probe (`sc.exe` service state + `GET 127.0.0.1:10732/health` + `memory.json`/`data/` file sizes, never the DB), search box driving `devcraft_adn_search` (streamable-HTTP handshake, `adn_search op={operation:query,text,page_size}`).
- **Help refresh**: HELP.md tool list, 20-view nav, CLI examples, and Memory/ADN tips; in-app guide names the Memory page and focus tools.

## 0.2.1 - 2026-11-26

- **Antigravity SEP-1865 MCP-App Dashboard**: Added `ui://devcraft/dashboard.html` interactive resource and `devcraft_dashboard` tool (`devcraft-mcp`).
- **Animated backdrops**: ported DJCraft's six background underlays (Sparkle Club, Starfield Warp, Baroque Damask, Carbon Weave, Brushed Metal, Solid) with per-skin pairing, intensity slider, and animated repaint only for Sparkle/Warp. Golden screenshots pin Solid for determinism.
- **Two more palettes**: Carbon Deck and Shocking Pink Club (9 total).
- **iPad skins & PWA**: companion pages theme from CSS variables mapped to the desktop `skin.json`, `?skin=` override plus picker on More, `/manifest.json` and Apple meta tags for home-screen install.

## 0.2.0 - 2026-11-24

The window is the craft shell: File, Edit, View, Fleet, Window, Help, navigation rail, command palette, shared kanban, and a DJCraft-inspired skin engine.

- **DJCraft-inspired Skin Engine (`skins.rs`)**: 7 curated palettes (Battle Console, Cockpit Dark, Club Midnight, Synthwave Outrun, Star Trek LCARS, Baroque Velvet & Gold, and high-contrast Daylight Studio), 5 card frame styles (Bold Drop Shadow, Neon Outer Glow, LCARS Starfleet Bracket, Baroque Double Frame, Clean Flat), luminous KPI deck with vector status LEDs, Today hero console strip, toolbar skin cycler + Sun/Moon toggle, and Settings picker persisted to `%LOCALAPPDATA%\devcraft\skin.json`.
- **Workspace root auto-discovery & `--root`**: `resolve_workspace_root()` checks `DEVCRAFT_ROOT`, walks ancestors for `mcp-central-docs`, and falls back to `D:\Dev\repos`; `clean_display_path()` strips the Windows `\\?\` prefix. `devcraft-mcp` adds a global `--root <PATH>` CLI flag.
- **Process Watchdog (`processes.rs`)**: `devcraft_processes` and `devcraft_kill_stale_processes` detect and clean duplicate or >=12h `devcraft-mcp` workers while protecting the current PID; wired into the Services page.
- **Stale Claim Hygiene**: `Claims` shows claim age and flags claims >=72h as stale; `devcraft_release_stale_claims` provides one-click cleanup on both Claims and Today.
- **Richer `devcraft_status`**: Returns `root`, `serve_port`, `crafts`, `stale_claims`, `todos_open`, `digest_age_hours`, `digest_stale`, `open_prs`, `stale_prs`, `open_issues`, `dirty_worktrees`, and `sync_drift` immediately without running a git scan.
- **Personal todos & Tailnet iPad companion**: `.devcraft/todos.json` (`devcraft_todo_add/list/done/clear`) with Today You section, plus opt-in `devcraft-mcp serve` on Tailnet port `11259` with `/todos`, `/api/todos`, and public `/api/health`.
- Fleet board computed from git status, handover age, claims, and the morning digest.
- Claims, pins, and 24-hour snoozes live in `.devcraft` at the workspace root.
- Pages for Today, Git, GitHub, Docker, reports, craft apps, Upstream, claims, and ports.
- Upstream page (`devcraft_upstream`): Storytold craft clones under `_upstream`. Each row is the last 24 hours of commits, the diff against the checkout from 24 hours ago, the newest tag, and the top of `CHANGELOG.md` when that file exists. The page only reads the local clones.
- CLI `devcraft-mcp tool <name>` and the stdio server call the same cockpit functions.
- Today, Git, and GitHub draw the morning digest as bar charts and tables (replies, stale PRs, stale issues, dirty trees, drift) before the git scan finishes.
- The git scan and the Docker probe run off the UI thread and fill the board as they arrive.
- Ports opens on the registry table. Audit marks which of those ports are listening.
- Fleet menu lists every page. Help includes a short guide and the changelog.
- PRD, tier plan, and handover describe this cockpit. The readme no longer advertises listening ports.
- Today adds commit-age bands, holder swimlanes (Cursor, OpenCode, Antigravity, Claude), and a 14-day commit chart filled from `git log --pretty=%cs` during the existing scan.
- Claims has Release every claim. Time machine, Sandbox, and Services are pages. Sandbox only checks Docker when the button is pressed. Services query and restart go through `sc.exe`.
- The `mcpb` manifest lists the 43 tools the server returns from `tools/list`.
- A card can carry a TaskCraft key such as BC-12. The key is stored in `.devcraft/tasks.json`. It does not create an issue.
- `git worktree list` adds a child card for each linked checkout. A drag on that card is refused.
- Today has golden screenshots in light and dark. `today_golden_light_and_dark` compares them.
- Today marks the morning digest when the file is older than 26 hours.
- The board filters by holder and by the shell search. Git search also matches the handover next line.
- A board column bar opens that column. Quiet stays off the board until its bar is clicked. The cards scroll inside the board.
- Ports lists lock findings from `fleet-gate/state/lock-findings.json`.
- Time machine can snapshot one checkout's handover and changelog. Sandbox runs one confirmed short command off the UI thread.
- Craft apps can run `start.ps1 -Smoke` for the selected row and keep the last exit.
- Tasks, Logs, and Telemetry pages. A task or log row shows the file in the drawer, with failure lines pulled out. Telemetry probes the monitoring stack on ports 12000-12008. The app does not call a model.
- Graphs draws a night sky of checkouts, who is shipping, weekday rhythm, commit weather, a code tide, a language ring, the heaviest checkouts, and a release spine. `devcraft_graphs` reads git off the UI thread and does not edit a checkout. GitHub star counts stay in the morning digest.
- Window > Chat (Ctrl+J) is a right-hand dock. The top bar has the same Chat button, because the menu popup is not in the window accessibility tree. Its preprompt is the bundled skills plus live status. Ollama is the provider. A missing model is an error. Read-only tools run. Other tools wait for Allow once. The input line uses the remaining width and does not grow the dock.
- Skills `devcraft-workflow` and `devcraft-sops` are compiled in, listed in Help, and served as `skill://` resources plus `devcraft_skill_read`.
- Workflows scans craft apps for a release exe, a debug exe, changelog, help docs, and File/Help menu strings. Debug and Release build the selected checkout after confirm. SOP buttons queue assfix, craft-assess, gapfind, drift-scan, quality-check, and doc-sync. They write a request. They do not edit the checkout.
- LLM & Settings lists installed Ollama models and the stdio MCP command.
- A folder at the fleet root is not scanned when the same name is a git checkout under `_upstream`.
- The readme shows the Today, Graphs, and Board windows.
