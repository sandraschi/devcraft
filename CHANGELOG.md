# Changelog

## 0.2.0 - 2026-11-14

The window is the craft shell: File, Edit, View, Fleet, Window, Help, navigation rail, command palette, and the shared kanban.

- Fleet board computed from git status, handover age, claims, and the morning digest.
- Claims, pins, and 24-hour snoozes live in `.devcraft` at the workspace root.
- Pages for Today, Git, GitHub, Docker, reports, craft apps, claims, and ports.
- CLI `devcraft-mcp tool <name>` and the stdio server call the same cockpit functions.
- Today, Git, and GitHub draw the morning digest as bar charts and tables (replies, stale PRs, stale issues, dirty trees, drift) before the git scan finishes.
- The git scan and the Docker probe run off the UI thread and fill the board as they arrive.
- Ports opens on the registry table. Audit marks which of those ports are listening.
- Fleet menu lists every page. Help includes a short guide and the changelog.
- PRD, tier plan, and handover describe this cockpit. The readme no longer advertises listening ports.
