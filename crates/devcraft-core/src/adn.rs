//! Read-only connector to advanced-memory-mcp.
//!
//! The daemon owns SQLite + LanceDB. DevCraft never opens the DB file.
//! It probes `http://127.0.0.1:10732/health` over a plain TCP socket
//! (no new deps) and reports service state + workspace file sizes.
//! Search does the MCP streamable-HTTP handshake (initialize,
//! notifications/initialized, tools/call) for `adn_search` and returns
//! the daemon JSON. When the daemon is down it returns `up: false`.

use serde_json::{json, Value};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

pub const ADN_HOST: &str = "127.0.0.1";
pub const ADN_PORT: u16 = 10732;
const TIMEOUT_MS: u64 = 2500;

fn addr() -> SocketAddr {
    format!("{ADN_HOST}:{ADN_PORT}")
        .parse()
        .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 10732)))
}

fn raw_post(path: &str, session: Option<&str>, body: &str) -> Option<(u16, Vec<String>, String)> {
    let sock = addr();
    let timeout = Duration::from_millis(TIMEOUT_MS);
    let mut stream = TcpStream::connect_timeout(&sock, timeout).ok()?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let mut head = format!(
        "POST {path} HTTP/1.1\r\nHost: {ADN_HOST}:{ADN_PORT}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nMCP-Protocol-Version: 2025-11-25\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(s) = session {
        let clean: String = s
            .chars()
            .filter(|c| !c.is_control() && *c != '\r' && *c != '\n')
            .collect();
        if !clean.is_empty() {
            head.push_str(&format!("Mcp-Session-Id: {clean}\r\n"));
        }
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).ok()?;
    stream.write_all(body.as_bytes()).ok()?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw).ok()?;
    let mut parts = raw.splitn(2, "\r\n\r\n");
    let header_text = parts.next().unwrap_or("");
    let body_text = parts.next().unwrap_or("").to_string();
    let status = header_text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);
    let headers = header_text.lines().skip(1).map(|l| l.to_string()).collect();
    Some((status, headers, body_text))
}

fn probe_tcp(path: &str, method: &str, body: Option<&str>) -> Option<(u16, String)> {
    if method == "POST" {
        return raw_post(path, None, body.unwrap_or("")).map(|(c, _, b)| (c, b));
    }
    let sock = addr();
    let timeout = Duration::from_millis(TIMEOUT_MS);
    let mut stream = TcpStream::connect_timeout(&sock, timeout).ok()?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: {ADN_HOST}:{ADN_PORT}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw).ok()?;
    let status = raw
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);
    let body_text = raw.split("\r\n\r\n").last().unwrap_or("").to_string();
    Some((status, body_text))
}

fn session_id(headers: &[String]) -> String {
    for line in headers {
        let lower = line.to_lowercase();
        if let Some(rest) = lower.strip_prefix("mcp-session-id:") {
            return rest.trim().to_string();
        }
    }
    String::new()
}

/// Unwrap a streamable-HTTP SSE body (`data: {...}` lines) into JSON.
fn sse_json(body: &str) -> Value {
    let mut last = Value::Null;
    for line in body.lines() {
        let t = line.trim();
        let data = if let Some(rest) = t.strip_prefix("data:") {
            rest.trim()
        } else if t.starts_with('{') {
            t
        } else {
            continue;
        };
        if let Ok(v) = serde_json::from_str::<Value>(data) {
            last = v;
        }
    }
    if last.is_null() {
        serde_json::from_str::<Value>(body).unwrap_or(json!({"raw": body}))
    } else {
        last
    }
}

fn service_state() -> Value {
    #[cfg(target_os = "windows")]
    {
        let out = std::process::Command::new("sc.exe")
            .args(["query", "advanced-memory-mcp-daemon"])
            .output();
        match out {
            Ok(o) => {
                let text = String::from_utf8_lossy(&o.stdout).to_string();
                let running = text.contains("RUNNING");
                let stopped = text.contains("STOPPED");
                let state = if running {
                    "RUNNING"
                } else if stopped {
                    "STOPPED"
                } else if text.trim().is_empty() {
                    "NOT_INSTALLED"
                } else {
                    "UNKNOWN"
                };
                json!({"service": "advanced-memory-mcp-daemon", "state": state})
            }
            Err(e) => {
                json!({"service": "advanced-memory-mcp-daemon", "state": "QUERY_FAILED", "note": e.to_string()})
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        json!({"service": "advanced-memory-mcp-daemon", "state": "NON_WINDOWS_HOST"})
    }
}

fn workspace_files(root: &Path) -> Value {
    let adv = root.join("advanced-memory-mcp");
    let memory_json = adv.join("memory.json");
    let data_dir = adv.join("data");
    let mem_size = fs::metadata(&memory_json).map(|m| m.len()).unwrap_or(0);
    let data_files = fs::read_dir(&data_dir).map(|it| it.count()).unwrap_or(0);
    json!({
        "adv_path": adv.display().to_string(),
        "memory_json_bytes": mem_size,
        "data_files": data_files,
        "memory_json_present": memory_json.is_file(),
    })
}

/// `devcraft_adn_status` payload. Read-only, never touches the DB.
pub fn status(root: &Path) -> Value {
    let svc = service_state();
    let files = workspace_files(root);
    match probe_tcp("/health", "GET", None) {
        Some((code, body)) => {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
            json!({
                "up": (200..300).contains(&code),
                "http_code": code,
                "health": parsed,
                "service": svc,
                "files": files,
                "endpoint": format!("http://{ADN_HOST}:{ADN_PORT}/mcp"),
            })
        }
        None => json!({
            "up": false,
            "http_code": 0,
            "note": "daemon not reachable on 127.0.0.1:10732. Start the NSSM service advanced-memory-mcp-daemon.",
            "service": svc,
            "files": files,
            "endpoint": format!("http://{ADN_HOST}:{ADN_PORT}/mcp"),
        }),
    }
}

/// `devcraft_adn_search` payload. Handshake + `adn_search` tools/call.
pub fn search(query: &str, limit: usize) -> Value {
    let q = query.trim();
    if q.is_empty() {
        return json!({"up": false, "error": "give a search query"});
    }
    let init = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "devcraft", "version": crate::APP_VERSION}
        }
    })
    .to_string();
    let (code, headers, body) = match raw_post("/mcp", None, &init) {
        Some(v) => v,
        None => {
            return json!({"up": false, "query": q, "note": "daemon not reachable on 127.0.0.1:10732"})
        }
    };
    if !(200..300).contains(&code) {
        return json!({"up": false, "http_code": code, "query": q, "result": sse_json(&body)});
    }
    let sid = session_id(&headers);
    if sid.is_empty() {
        return json!({"up": false, "query": q, "error": "daemon gave no session id", "result": sse_json(&body)});
    }
    let ack = json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string();
    let _ = raw_post("/mcp", Some(&sid), &ack);
    let rpc = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {"name": "adn_search", "arguments": {"op": {"operation": "query", "text": q, "page_size": limit}}}
    })
    .to_string();
    match raw_post("/mcp", Some(&sid), &rpc) {
        Some((call_code, _, call_body)) => {
            let parsed = sse_json(&call_body);
            json!({"up": (200..300).contains(&call_code), "http_code": call_code, "query": q, "result": parsed})
        }
        None => json!({"up": false, "query": q, "note": "daemon dropped the search call"}),
    }
}
