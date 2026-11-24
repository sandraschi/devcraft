# DevCraft PRD

Status: 16 November 2026. Version 0.2.0.
Repo: `D:\Dev\repos\devcraft`

## Problem

One person works across hundreds of checkouts with Cursor, OpenCode, Antigravity, and Claude. Git status, the morning digest, Docker, the port registry, and the fleet reports live in different files. The v0.1.0 window was a stack of panes and advertised two listening ports.

## Goal

A local cockpit, the same shape as HelmCraft. The window is for the person. The four IDEs call the same stdio tools. The board is computed from the working trees and the digest that already exists. Missing sources stay empty with a note.

## What it does

- Reads `mcp-central-docs/operations/fleet-status.md` and shows the totals, the reply list, stale pull requests, stale issues, unanswered questions, dirty worktrees, and sync drift as charts and tables.
- Scans sibling directories for git status, the last commit, and the next line under a "next" heading in `HANDOVER.md`.
- Places a checkout on the board: index lock, unmerged paths, or a handover older than 7 days on a dirty tree is Blocked. A claim is Active. Dirty files or commits ahead of origin are Review. A commit younger than 24 hours is Done. A pin, or a handover younger than 7 days, is Ready. No commit, or a commit older than 60 days, is Quiet. Quiet cards are hidden.
- Accepts a drag as a command. Inbox snoozes for 24 hours. Ready toggles a pin. Active claims the checkout. Done releases a repo, or snoozes a digest card. Review, Blocked, and Quiet refuse the drag.
- Stores claims, pins, and snoozes in `{workspace}/.devcraft`, outside any project git tree.
- Lists craft checkouts (name ends with "craft", plus craft-deck, craft-biz, and craft-sandbox-vbox).
- Lists the Storytold craft clones under `_upstream` through `devcraft_upstream`: commits and the tree diff from the last 24 hours, the newest tag, and the top of `CHANGELOG.md` when the file exists. The first open of that page reads the clones in parallel. It does not fetch, merge, commit, or push. Smoke does not run this read.
- Lists six reports: the fleet daily page, the morning digest, the session log, the AIWatcher log, the port registry, and `STATUS.md`. A report is stale after 26 hours, or when the file is missing.
- Opens a checkout in Cursor, OpenCode, Antigravity, or Claude, and claims it for that holder.
- Shows Docker when `docker info` answers, and the port registry from `WEBAPP_PORTS.md`. Audit checks which of those ports accept a bind. The Ports page also lists errors and warnings from `fleet-gate/state/lock-findings.json`.
- Draws the 14-day commit bars, commit-age bars, and holder bars. The board can be filtered by holder and by the shell search.
- Graphs measures 26 weeks of commits, lines added and deleted, source lines by language, the largest checkouts, recent tags, the authors on those commits, and which weekdays the fleet commits. The night sky is one star per active checkout. The read is off the UI thread. It does not edit a checkout. GitHub stargazer counts are not fetched.
- Puts time machine, sandbox, and service restart on their own pages. Time machine can snapshot one checkout's handover and changelog. Sandbox runs one confirmed short command off the UI thread.
- Lists scheduled fleet tasks, the report each job writes, fleet and IDE MCP logs, and the monitoring stack on ports 12000-12008. Selecting a task or a log shows the text in the drawer, with failure lines pulled out. The app does not call a model.
- Runs `start.ps1 -Smoke` for the selected craft app, off the UI thread, and keeps the last exit on the row.
- Opens a right-hand chat from Window > Chat. The preprompt is the bundled skills plus live status. Ollama answers. Read-only tools run. Other tools wait for Allow once.
- Workflows checks each craft app for a release exe, a debug exe, a changelog, help docs, and File/Help menu strings, and can build the selected checkout. SOP buttons write a request for craft-assess, assfix, gapfind, drift-scan, quality-check, or doc-sync. They do not edit the checkout.
- Marks the morning digest on Today when the file is older than 26 hours.
- Exposes the same operations on `devcraft-mcp tool <name>` and on MCP over stdio.
- Keeps personal todos in `{workspace}/.devcraft/todos.json`: quick capture with a due of today, tomorrow, a date, or none. The Today page shows a You section with an add box and Done buttons. `devcraft_todo_add/list/done/clear` are the MCP tools. This does not create a TaskCraft issue.
- Serves the same todos to iPad Safari on the Tailnet only: `devcraft-mcp serve` binds localhost plus the Tailscale IPv4 on port 11259 (opt-in, off by default, token in `.devcraft/mobile.token`). No `0.0.0.0`, no Funnel.

## What it does not do

- Listen on a port. CraftDeck still installs apps. TaskCraft still tracks issues. A card key is a label. It does not create or edit an issue.
- Fetch, merge, commit, or push anything under `_upstream`.
- Call `gh`. GitHub rows are the digest file.
- Treat a green smoke JSON as proof the window looks right. Someone still has to open it.

## Done when

`cargo run -p devcraft-app -- --smoke` prints version 0.2.0, `transport: stdio`, a present digest, and menu parity for File, Edit, View, Fleet, Window, Help. `cargo test -p devcraft-core -p devcraft-app` passes. Today shows digest tables on open, before the git scan finishes.
