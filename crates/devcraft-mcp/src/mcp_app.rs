//! SEP-1865 MCP App support for Antigravity (`ui://devcraft/dashboard.html`).

use devcraft_core::cockpit::Cockpit;
use serde_json::{json, Value};

pub const APP_URI: &str = "ui://devcraft/dashboard.html";
pub const APP_MIME: &str = "text/html;profile=mcp-app";
pub const TOOL_NAME: &str = "devcraft_dashboard";

const EMBEDDED_HTML: &str = include_str!("dashboard.html");
const MAX_CARDS: usize = 200;

pub fn resource_entry() -> Value {
    json!({
        "uri": APP_URI,
        "name": "DevCraft Dashboard",
        "description": "Interactive DevCraft fleet cockpit UI for Antigravity.",
        "mimeType": APP_MIME,
        "_meta": {
            "ui": {
                "prefersBorder": false
            }
        }
    })
}

pub fn load_html() -> String {
    if let Ok(path) = std::env::var("DEVCRAFT_APP_HTML") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            if let Ok(html) = std::fs::read_to_string(trimmed) {
                return html;
            }
        }
    }
    EMBEDDED_HTML.to_string()
}

pub fn resource_read() -> Value {
    json!({
        "contents": [
            {
                "uri": APP_URI,
                "mimeType": APP_MIME,
                "text": load_html(),
                "_meta": {
                    "ui": {
                        "prefersBorder": false
                    }
                }
            }
        ]
    })
}

pub fn tool_spec() -> Value {
    json!({
        "name": TOOL_NAME,
        "title": "DevCraft",
        "description": "Open the interactive DevCraft fleet cockpit dashboard (status KPIs, board, personal todos, claims, and port audit).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "scan": {
                    "type": "boolean",
                    "description": "When true, scan sibling git checkouts before building the snapshot."
                }
            }
        },
        "_meta": {
            "ui": {
                "resourceUri": APP_URI
            },
            "com.google.antigravity/ui": {
                "entrypoints": [
                    { "type": "aux_pane", "title": "DevCraft" },
                    { "type": "global", "title": "DevCraft" }
                ]
            }
        }
    })
}

pub fn call_dashboard(pit: &mut Cockpit, args: &Value) -> Value {
    let scan = args.get("scan").and_then(Value::as_bool).unwrap_or(false);
    if scan {
        let _ = pit.call("devcraft_refresh", &json!({}));
    }

    let status = pit
        .call("devcraft_status", &json!({}))
        .unwrap_or_else(|e| json!({ "error": e }));
    let mut board = pit
        .call("devcraft_board", &json!({}))
        .unwrap_or_else(|e| json!({ "cards": [], "error": e }));
    let todos = pit
        .call("devcraft_todo_list", &json!({}))
        .unwrap_or_else(|e| json!({ "open": 0, "todos": [], "error": e }));

    if let Some(cards) = board.get_mut("cards").and_then(Value::as_array_mut) {
        if cards.len() > MAX_CARDS {
            cards.truncate(MAX_CARDS);
        }
    }

    let dirty = status
        .get("dirty_worktrees")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let prs = status.get("open_prs").and_then(Value::as_u64).unwrap_or(0);
    let claims = status.get("claims").and_then(Value::as_u64).unwrap_or(0);
    let open_todos = todos.get("open").and_then(Value::as_u64).unwrap_or(0);
    let summary = format!(
        "DevCraft cockpit ready: {dirty} dirty worktrees, {prs} open PRs, {claims} active claims, {open_todos} open todos."
    );

    let structured = json!({
        "status": status,
        "board": board,
        "todos": todos
    });

    json!({
        "content": [
            {
                "type": "text",
                "text": summary
            }
        ],
        "structuredContent": structured
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_app_metadata_matches_sep_1865() {
        let res = resource_entry();
        assert_eq!(res["uri"], APP_URI);
        assert_eq!(res["mimeType"], APP_MIME);

        let tool = tool_spec();
        assert_eq!(tool["name"], TOOL_NAME);
        assert_eq!(tool["_meta"]["ui"]["resourceUri"], APP_URI);
        let entrypoints = tool["_meta"]["com.google.antigravity/ui"]["entrypoints"]
            .as_array()
            .expect("entrypoints array");
        assert_eq!(entrypoints.len(), 2);

        let read = resource_read();
        let html = read["contents"][0]["text"].as_str().expect("html text");
        assert!(html.contains("<!doctype html>"));
        assert!(html.len() < 10 * 1024 * 1024);
    }
}
