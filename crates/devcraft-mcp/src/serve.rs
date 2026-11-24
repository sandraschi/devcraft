//! Tailnet-only mobile companion: the full cockpit on iPad Safari.
//!
//! Opt-in and off by default. Binds localhost plus the Tailscale IPv4 only,
//! never 0.0.0.0 and never Funnel. One persistent Cockpit lives in this
//! process: a background thread runs the git scan on startup (and on demand),
//! so board and git pages are live without rescanning per request. Todos and
//! mutations go through the same `todos.json` the desktop window uses.
//! Auth is a token in `{workspace}/.devcraft/mobile.token`, created on first
//! run, passed as `?token=` or `Authorization: Bearer`.
//! `GET /api/health` stays public (liveness only) for fleet health checks.

use devcraft_core::cockpit::Cockpit;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::mobile;
use std::path::Path;

static PIT: OnceLock<Mutex<Cockpit>> = OnceLock::new();

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

/// Run a closure against the shared Cockpit.
fn with_pit<T>(f: impl FnOnce(&mut Cockpit) -> T) -> Result<T, String> {
    let pit = PIT.get().ok_or("cockpit not started")?;
    let mut guard: MutexGuard<'_, Cockpit> = pit.lock().map_err(|e| e.to_string())?;
    Ok(f(&mut guard))
}

/// Call a cockpit tool, turning every failure into an `{"error": ...}` value.
fn call_tool(tool: &str, args: &Value) -> Value {
    match with_pit(|pit| pit.call(tool, args)) {
        Ok(Ok(value)) => value,
        Ok(Err(err)) | Err(err) => json!({"error": err}),
    }
}

/// Kick a fleet rescan off the async runtime.
fn kick_refresh() {
    std::thread::spawn(|| {
        let _ = with_pit(|pit| pit.refresh());
    });
}

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

fn is_json(body: &str) -> bool {
    body.trim_start().starts_with('{')
}

fn json_body(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or(json!({}))
}

/// Render the todos page from the shared cockpit.
fn todos_html(token: &str) -> Vec<u8> {
    match with_pit(|pit| mobile::todos(pit, token)) {
        Ok(html) => response(200, "text/html", &html),
        Err(err) => response(500, "text/plain", &err),
    }
}

/// Render the today page from the shared cockpit.
fn today_html(token: &str) -> Vec<u8> {
    match with_pit(|pit| {
        let open = pit.todos().iter().filter(|t| !t.done).count();
        mobile::today(pit, token, open)
    }) {
        Ok(html) => response(200, "text/html", &html),
        Err(err) => response(500, "text/plain", &err),
    }
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
    // Public liveness probe for fleet health checks. No auth: it reveals
    // nothing but the service name and version (same as the MCP handshake).
    if method == "GET" && (path == "/api/health" || path == "/health") {
        let body = format!(
            r#"{{"service":"devcraft-mcp-serve","version":"{}","ok":true}}"#,
            env!("CARGO_PKG_VERSION")
        );
        return response(200, "application/json", &body);
    }
    if !authorized(query, headers, token) {
        return unauthorized();
    }
    match (method, path) {
        ("GET", "/") => today_html(token),
        ("GET", "/board") => match with_pit(|pit| mobile::board(pit, token)) {
            Ok(html) => response(200, "text/html", &html),
            Err(err) => response(500, "text/plain", &err),
        },
        ("GET", "/todos") => todos_html(token),
        ("GET", "/git") => match with_pit(|pit| mobile::git(pit, token)) {
            Ok(html) => response(200, "text/html", &html),
            Err(err) => response(500, "text/plain", &err),
        },
        ("GET", "/more") => match with_pit(|pit| mobile::more(pit, token)) {
            Ok(html) => response(200, "text/html", &html),
            Err(err) => response(500, "text/plain", &err),
        },
        ("GET", "/github") => match with_pit(|pit| mobile::github(pit, token)) {
            Ok(html) => response(200, "text/html", &html),
            Err(err) => response(500, "text/plain", &err),
        },
        ("GET", "/crafts") => match with_pit(|pit| mobile::crafts(pit, token)) {
            Ok(html) => response(200, "text/html", &html),
            Err(err) => response(500, "text/plain", &err),
        },
        ("GET", "/tasks") => {
            let value = call_tool("devcraft_tasks", &json!({}));
            response(200, "text/html", &mobile::tasks(token, &value))
        }
        ("GET", "/reports") => match with_pit(|pit| mobile::reports(pit, token)) {
            Ok(html) => response(200, "text/html", &html),
            Err(err) => response(500, "text/plain", &err),
        },
        ("GET", "/api/status") => {
            let value = call_tool("devcraft_status", &json!({}));
            response(200, "application/json", &value.to_string())
        }
        ("GET", "/api/digest") => {
            let value = call_tool("devcraft_digest", &json!({}));
            response(200, "application/json", &value.to_string())
        }
        ("GET", "/api/board") => {
            let value = call_tool("devcraft_board", &json!({}));
            response(200, "application/json", &value.to_string())
        }
        ("GET", "/api/git") => {
            let value = call_tool("devcraft_git", &json!({}));
            response(200, "application/json", &value.to_string())
        }
        ("GET", "/api/crafts") => {
            let value = call_tool("devcraft_crafts", &json!({}));
            response(200, "application/json", &value.to_string())
        }
        ("POST", "/api/refresh") => {
            kick_refresh();
            if is_json(body) {
                response(200, "application/json", r#"{"refresh":"started"}"#)
            } else {
                today_html(token)
            }
        }
        ("GET", "/api/todos") => {
            let value = call_tool("devcraft_todo_list", &json!({}));
            response(200, "application/json", &value.to_string())
        }
        ("POST", "/api/todos") => {
            let (title, due) = if is_json(body) {
                let value = json_body(body);
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
            let value = call_tool(
                "devcraft_todo_add",
                &json!({"title": title, "due": due, "source": "ipad"}),
            );
            if !is_json(body) {
                return todos_html(token);
            }
            response(200, "application/json", &value.to_string())
        }
        ("POST", "/api/todos/done") => {
            let id = if is_json(body) {
                json_body(body)
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            } else {
                form_field(body, "id")
            };
            let value = call_tool("devcraft_todo_done", &json!({"id": id}));
            if !is_json(body) {
                return todos_html(token);
            }
            response(200, "application/json", &value.to_string())
        }
        ("POST", "/api/todos/clear") => {
            let value = call_tool("devcraft_todo_clear", &json!({}));
            if !is_json(body) {
                return todos_html(token);
            }
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
    let pit = Cockpit::from_env()?;
    let _ = PIT.set(Mutex::new(pit));
    // First scan runs off the async runtime so the server answers immediately.
    kick_refresh();
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
        eprintln!("  http://{addr}:{PORT}/?token={token}");
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
