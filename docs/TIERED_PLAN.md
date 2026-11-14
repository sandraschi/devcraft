# DevCraft tier plan

Status: 14 November 2026. Current version: 0.2.0.
The product is a cockpit. Tiers below are what shipped and what is still open. A box is checked only when the window or the tests do that thing.

## Tier 1 - Workspace and compound tools (v0.1.0)

Shipped in the 0.1.0 release.

- [x] Workspace: `devcraft-core`, `devcraft-mcp`, `devcraft-app`. Clippy denies unwrap, expect, panic, todo, and unimplemented.
- [x] Six local tools: fleet audit, worktree checkpoint, port audit, sandbox run, service restart, tool discovery.
- [x] MCP 2024-11-05 over stdio, and a portable zip.
- [x] Listening ports 11258 and 11259 were reserved for that release. v0.2.0 does not bind them.

## Tier 2 - Cockpit (v0.2.0)

Shipped on `main` as `14d3349` and `5e61dbf`.

- [x] Craft shell: File, Edit, View, Fleet, Window, Help, rail, command palette, status bar.
- [x] Nine pages. Fleet lists each one. Help includes the guide, the readme, and this changelog's sibling docs.
- [x] Morning digest parsed into charts and tables. The file is not regenerated here.
- [x] Board computed from git, handover age, claims, pins, and snoozes. Refused drags stay refused.
- [x] Git scan and Docker probe off the UI thread. Smoke does not scan.
- [x] Port registry table on open. Audit marks listeners.
- [x] CLI, MCP, and the window call one cockpit. The desktop package lists 22 tools.
- [x] Gate on 2026-11-14: 18 tests, clippy `-D warnings`, smoke green (digest present, 46 craft apps, both menu languages).

## Tier 3 - Depth

Not started. These were part of the cockpit design and are not in the window.

- [ ] Holder swimlanes, and a chart of commit age. Today's bars are digest totals and column counts.
- [ ] A 14-day activity chart.
- [ ] A TaskCraft key on a card, without turning this into the issue tracker.
- [ ] Git worktree child cards.
- [ ] End-of-day release of every claim.
- [ ] Pages for the time machine, the sandbox, and service restart. The tools already exist.
- [ ] Golden screenshots of Today in light and dark, checked in a test.

## Tier 4 - A person can launch it

- [ ] Someone opens the window and confirms Today, the board, and Help.
- [ ] CraftDeck card points at this checkout.
- [ ] Installer only after the window has been looked at.

## Verification

```powershell
cargo test -p devcraft-core -p devcraft-app -p devcraft-mcp
cargo clippy -p devcraft-core -p devcraft-app -p devcraft-mcp --all-targets -- -D warnings
cargo run -p devcraft-app -- --smoke
```

Smoke must stay fast. It reads the digest and the craft directories. It does not run `git status` on the fleet.
