# DevCraft PRD

Status: 14 November 2026. Version 0.2.0.
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
- Lists six reports: the fleet daily page, the morning digest, the session log, the AIWatcher log, the port registry, and `STATUS.md`. A report is stale after 26 hours, or when the file is missing.
- Opens a checkout in Cursor, OpenCode, Antigravity, or Claude, and claims it for that holder.
- Shows Docker when `docker info` answers, and the port registry from `WEBAPP_PORTS.md`. Audit checks which of those ports accept a bind.
- Exposes the same operations on `devcraft-mcp tool <name>` and on MCP over stdio.

## What it does not do

- Listen on a port. CraftDeck still installs apps. TaskCraft still tracks issues.
- Call `gh`. GitHub rows are the digest file.
- Draw a 14-day flow, a commit heatmap, or swimlanes by holder.
- Put the time-machine, sandbox, and service-restart tools on a page.
- Treat a green smoke JSON as proof the window looks right. Someone still has to open it.

## Done when

`cargo run -p devcraft-app -- --smoke` prints version 0.2.0, `transport: stdio`, a present digest, and menu parity for File, Edit, View, Fleet, Window, Help. `cargo test -p devcraft-core -p devcraft-app` passes. Today shows digest tables on open, before the git scan finishes.
