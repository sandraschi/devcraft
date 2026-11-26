## 0.3.0 - 2026-11-26

- **Dev-focus QoL pack**: `devcraft_focus` (claim + pin + open in one call), `devcraft_find` (fuzzy repo search), `devcraft_recent` (commits in last N hours), `devcraft_reveal_repo` (open checkout in Explorer), `devcraft_doctor` (root, scan, digest, tool bins). CLI `tool` gains `--query/--hours/--limit`.
- **Memory page (20th view, Ops group)**: Dev-focus buttons, `advanced-memory-mcp` daemon probe (`sc.exe` service state + `GET 127.0.0.1:10732/health` + `memory.json`/`data/` file sizes, never the DB), search box driving `devcraft_adn_search` (streamable-HTTP handshake, `adn_search op={operation:query,text,page_size}`).
- **Help refresh**: HELP.md tool list, 20-view nav, CLI examples, and Memory/ADN tips; in-app guide names the Memory page and focus tools.
