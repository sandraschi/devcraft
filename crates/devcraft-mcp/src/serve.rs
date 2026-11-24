//! Tailnet-only mobile companion: todos on iPad Safari.
//!
//! Opt-in and off by default. Binds localhost plus the Tailscale IPv4 only,
//! never 0.0.0.0 and never Funnel. Every request opens a fresh Cockpit, so
//! the desktop window, MCP, and this server all see the same `todos.json`.
//! Auth is a token in `{workspace}/.devcraft/mobile.token`, created on first
//! run, passed as `?token=` or `Authorization: Bearer`.

use devcraft_core::cockpit::Cockpit;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const PORT: u16 = 11259;

fn workspace_root() -> PathBuf {
    if Path::new("mcp-central-docs").exists() {
        PathBuf::from(".")
    } else if Path::new("../mcp-central-docs").exists() {
        PathBuf::from("..")
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}

use std::path::Path;

/// Read the mobile token, creating it on first run.
fn mobile_token(root: &Path) -> Result<String, String> {
    let dir = root.join(".devcraft");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("mobile.token");
    if let Ok(text) = std::fs::read_to_string(&path) {
        let token = text.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let token = format!("{:x}{:x}", nanos, std::process::id());
    std::fs::write(&path, &token).map_err(|e| e.to_string())?;
    Ok(token)
}

/// Tailscale IPv4 from `tailscale ip -4`, empty when Tailscale is down.
fn tailscale_ip() -> String {
    let output = std::process::Command::new("tailscale")
        .arg("ip")
        .arg("-4")
        .output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => String::new(),
    }
}

fn unauthorized() -> Vec<u8> {
    response(401, "text/plain", "bad or missing token")
}

fn response(status: u16, content_type: &str, body: &str) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        400 => "Bad request",
        401 => "Unauthorized",
        404 => "Not found",
        _ => "Error",
    };
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn authorized(query: &str, headers: &HashMap<String, String>, token: &str) -> bool {
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("token=") {
            if value == token {
                return true;
            }
        }
    }
    if let Some(auth) = headers.get("authorization") {
        if let Some(bearer) = auth.strip_prefix("Bearer ") {
            if bearer.trim() == token {
                return true;
            }
        }
    }
    false
}

fn form_field(body: &str, name: &str) -> String {
    for pair in body.split('&') {
        if let Some(value) = pair.strip_prefix(&format!("{name}=")) {
            return value
                .replace('+', " ")
                .split('%')
                .enumerate()
                .map(|(i, part)| {
                    if i == 0 {
                        part.to_string()
                    } else if part.len() >= 2 {
                        let hex = &part[..2];
                        let rest = &part[2..];
                        match u8::from_str_radix(hex, 16) {
                            Ok(byte) => format!("{}{rest}", byte as char),
                            Err(_) => format!("%{part}"),
                        }
                    } else {
                        format!("%{part}")
                    }
                })
                .collect();
        }
    }
    String::new()
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn todos_page(token: &str) -> Result<String, String> {
    let mut pit = Cockpit::from_env()?;
    let list = pit.call("devcraft_todo_list", &json!({}))?;
    let empty = Vec::new();
    let rows = list
        .get("todos")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let mut items = String::new();
    for row in rows {
        let id = row.get("id").and_then(Value::as_str).unwrap_or("");
        let title = row.get("title").and_then(Value::as_str).unwrap_or("");
        let due = row.get("due").and_then(Value::as_str).unwrap_or("");
        let done = row.get("done").and_then(Value::as_bool).unwrap_or(false);
        if done {
            continue;
        }
        let due = if due.is_empty() {
            String::new()
        } else {
            format!(" <small>{}</small>", escape_html(due))
        };
        items.push_str(&format!(
            "<li><form method=\"post\" action=\"/api/todos/done?token={token}\">\
             <input type=\"hidden\" name=\"id\" value=\"{id}\">\
             <button type=\"submit\">Done</button>\
             <span>{}{due}</span></form></li>",
            escape_html(title),
        ));
    }
    if items.is_empty() {
        items.push_str("<li>Nothing open. Add the first one below.</li>");
    }
    Ok(format!(
        "<!doctype html><html><head><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>DevCraft todos</title><style>\
         body{{font-family:system-ui;background:#0f172a;color:#e2e8f0;margin:0;padding:16px}}\
         ul{{list-style:none;padding:0}}li{{margin:10px 0}}\
         button,input{{font-size:18px;padding:10px 14px;border-radius:10px;border:0}}\
         button{{background:#38bdf8;color:#082f49;font-weight:700}}\
         input[name=title]{{width:60%;background:#1e293b;color:#e2e8f0}}\
         span{{margin-left:10px}}small{{color:#94a3b8}}\
         </style></head><body><h1>Today</h1><ul>{items}</ul>\
         <form method=\"post\" action=\"/api/todos?token={token}\">\
         <input name=\"title\" placeholder=\"New todo\" autocomplete=\"off\">\
         <input type=\"hidden\" name=\"due\" value=\"today\">\
         <button type=\"submit\">Add today</button></form>\
         <p><a style=\"color:#38bdf8\" href=\"/api/todos?token={token}\">JSON</a></p>\
         </body></html>"
    ))
}

async fn handle(raw: &str, body: &str, headers: &HashMap<String, String>, token: &str) -> Vec<u8> {
    let mut lines = raw.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target, ""),
    };
    if !authorized(query, headers, token) {
        return unauthorized();
    }
    match (method, path) {
        ("GET", "/") | ("GET", "/todos") => match todos_page(token) {
            Ok(html) => response(200, "text/html", &html),
            Err(err) => response(500, "text/plain", &err),
        },
        ("GET", "/api/todos") => {
            let value = Cockpit::from_env()
                .and_then(|mut pit| pit.call("devcraft_todo_list", &json!({})))
                .unwrap_or_else(|err| json!({"error": err}));
            response(200, "application/json", &value.to_string())
        }
        ("POST", "/api/todos") => {
            let (title, due) = if body.trim_start().starts_with('{') {
                let value: Value = serde_json::from_str(body).unwrap_or(json!({}));
                (
                    value
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    value
                        .get("due")
                        .and_then(Value::as_str)
                        .unwrap_or("today")
                        .to_string(),
                )
            } else {
                (form_field(body, "title"), {
                    let due = form_field(body, "due");
                    if due.is_empty() {
                        "today".into()
                    } else {
                        due
                    }
                })
            };
            let value = Cockpit::from_env()
                .and_then(|mut pit| {
                    pit.call(
                        "devcraft_todo_add",
                        &json!({"title": title, "due": due, "source": "ipad"}),
                    )
                })
                .unwrap_or_else(|err| json!({"error": err}));
            if !body.trim_start().starts_with('{') {
                return match todos_page(token) {
                    Ok(html) => response(200, "text/html", &html),
                    Err(err) => response(500, "text/plain", &err),
                };
            }
            response(200, "application/json", &value.to_string())
        }
        ("POST", "/api/todos/done") => {
            let id = if body.trim_start().starts_with('{') {
                let value: Value = serde_json::from_str(body).unwrap_or(json!({}));
                value
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            } else {
                form_field(body, "id")
            };
            let value = Cockpit::from_env()
                .and_then(|mut pit| pit.call("devcraft_todo_done", &json!({"id": id})))
                .unwrap_or_else(|err| json!({"error": err}));
            if !body.trim_start().starts_with('{') {
                return match todos_page(token) {
                    Ok(html) => response(200, "text/html", &html),
                    Err(err) => response(500, "text/plain", &err),
                };
            }
            response(200, "application/json", &value.to_string())
        }
        ("POST", "/api/todos/clear") => {
            let value = Cockpit::from_env()
                .and_then(|mut pit| pit.call("devcraft_todo_clear", &json!({})))
                .unwrap_or_else(|err| json!({"error": err}));
            response(200, "application/json", &value.to_string())
        }
        _ => response(404, "text/plain", "unknown route"),
    }
}

/// Run the companion. `bind` is `tailscale` (default), `localhost`, or an IP.
pub async fn run(bind: &str) -> Result<(), String> {
    let root = workspace_root()
        .canonicalize()
        .unwrap_or_else(|_| workspace_root());
    let token = mobile_token(&root)?;
    let tip = tailscale_ip();
    let mut targets = vec!["127.0.0.1".to_string()];
    if bind == "tailscale" {
        if tip.is_empty() {
            return Err("tailscale ip -4 found no address; is Tailscale running?".into());
        }
        if tip != "127.0.0.1" {
            targets.push(tip.clone());
        }
    } else if bind != "localhost" {
        targets = vec![bind.to_string()];
    }
    let mut listeners = Vec::new();
    for addr in &targets {
        let listener = TcpListener::bind(format!("{addr}:{PORT}"))
            .await
            .map_err(|e| format!("cannot bind {addr}:{PORT}: {e}"))?;
        listeners.push(listener);
    }
    eprintln!("devcraft serve on port {PORT} (Tailnet-only, token in .devcraft/mobile.token)");
    for addr in &targets {
        eprintln!("  http://{addr}:{PORT}/todos?token={token}");
    }
    let acceptors = listeners.into_iter().map(|listener| {
        let token = token.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    continue;
                };
                let token = token.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 65536];
                    let Ok(n) = socket.read(&mut buf).await else {
                        return;
                    };
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    let (head, body) = match text.split_once("\r\n\r\n") {
                        Some((h, b)) => (h.to_string(), b.to_string()),
                        None => (text, String::new()),
                    };
                    let mut headers = HashMap::new();
                    for line in head.lines().skip(1) {
                        if let Some((k, v)) = line.split_once(':') {
                            headers.insert(k.trim().to_lowercase(), v.trim().to_string());
                        }
                    }
                    let out = handle(&head, &body, &headers, &token).await;
                    let _ = socket.write_all(&out).await;
                });
            }
        })
    });
    for task in acceptors {
        let _ = task.await;
    }
    Ok(())
}
