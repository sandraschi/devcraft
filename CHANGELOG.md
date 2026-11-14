# Changelog

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
- The `mcpb` manifest lists the 30 tools the server returns from `tools/list`.
- A card can carry a TaskCraft key such as BC-12. The key is stored in `.devcraft/tasks.json`. It does not create an issue.
- `git worktree list` adds a child card for each linked checkout. A drag on that card is refused.
- Today has golden screenshots in light and dark. `today_golden_light_and_dark` compares them.
