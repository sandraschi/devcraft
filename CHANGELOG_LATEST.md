## 0.2.0 - 2026-11-14

The window is the craft shell: File, Edit, View, Fleet, Window, Help, navigation rail, command palette, and the shared kanban.

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
