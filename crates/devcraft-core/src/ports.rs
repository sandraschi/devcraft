//! Port sentinel and registry reconciliation module.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::net::TcpListener;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortEntry {
    pub port: u16,
    pub repo: String,
    pub description: String,
    pub is_listening: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortAuditReport {
    pub total_registered: usize,
    pub active_listeners: usize,
    pub entries: Vec<PortEntry>,
    pub unregistered_listeners: Vec<u16>,
    /// False until each port has been probed. The table is still the registry.
    pub checked: bool,
}

pub struct PortSentinel;

impl PortSentinel {
    /// Test if a local TCP port is currently occupied / listening on 127.0.0.1.
    pub fn is_port_in_use(port: u16) -> bool {
        TcpListener::bind(("127.0.0.1", port)).is_err()
    }

    /// The port table from `WEBAPP_PORTS.md`, with no socket probes.
    pub fn registry(workspace_root: &Path) -> PortAuditReport {
        let registered = read_registered(workspace_root);
        let mut entries: Vec<PortEntry> = registered
            .iter()
            .map(|(port, (repo, desc))| PortEntry {
                port: *port,
                repo: repo.clone(),
                description: desc.clone(),
                is_listening: false,
            })
            .collect();
        entries.sort_by_key(|e| e.port);
        PortAuditReport {
            total_registered: entries.len(),
            active_listeners: 0,
            entries,
            unregistered_listeners: Vec::new(),
            checked: false,
        }
    }

    /// Parse `operations/WEBAPP_PORTS.md` from mcp-central-docs and test active sockets.
    pub fn audit_ports(workspace_root: &Path) -> PortAuditReport {
        let registered = read_registered(workspace_root);
        let mut entries = Vec::new();
        let mut active_listeners = 0;

        for (port, (repo, desc)) in &registered {
            let listening = Self::is_port_in_use(*port);
            if listening {
                active_listeners += 1;
            }
            entries.push(PortEntry {
                port: *port,
                repo: repo.clone(),
                description: desc.clone(),
                is_listening: listening,
            });
        }

        entries.sort_by_key(|e| e.port);

        let mut unregistered_listeners = Vec::new();
        for port in 11250..=11270 {
            if !registered.contains_key(&port) && Self::is_port_in_use(port) {
                unregistered_listeners.push(port);
            }
        }

        PortAuditReport {
            total_registered: entries.len(),
            active_listeners,
            entries,
            unregistered_listeners,
            checked: true,
        }
    }

    /// Errors and warnings from the last fleet lock build. Empty when the file is absent.
    pub fn lock_findings(workspace_root: &Path) -> Vec<LockFinding> {
        let path = workspace_root
            .join("mcp-central-docs")
            .join("fleet-gate")
            .join("state")
            .join("lock-findings.json");
        let Ok(text) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for key in ["errors", "warnings"] {
            let Some(items) = value.get(key).and_then(|v| v.as_array()) else {
                continue;
            };
            for item in items {
                let message = item
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if message.is_empty() {
                    continue;
                }
                out.push(LockFinding {
                    code: item
                        .get("code")
                        .and_then(|v| v.as_str())
                        .unwrap_or(key)
                        .to_string(),
                    severity: item
                        .get("severity")
                        .and_then(|v| v.as_str())
                        .unwrap_or(key)
                        .to_string(),
                    message,
                });
            }
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFinding {
    pub code: String,
    pub severity: String,
    pub message: String,
}

fn read_registered(workspace_root: &Path) -> HashMap<u16, (String, String)> {
    let ports_doc = workspace_root
        .join("mcp-central-docs")
        .join("operations")
        .join("WEBAPP_PORTS.md");
    let mut registered: HashMap<u16, (String, String)> = HashMap::new();
    let Ok(content) = std::fs::read_to_string(ports_doc) else {
        return registered;
    };
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let parts: Vec<&str> = trimmed.split('|').map(|s| s.trim()).collect();
        if parts.len() >= 4 {
            if let Ok(port) = parts[1].parse::<u16>() {
                let repo = parts[2].trim_matches('`').to_string();
                let desc = parts[3].to_string();
                registered.insert(port, (repo, desc));
            }
        }
    }
    registered
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn lock_findings_keep_code_and_message() {
        let dir = std::env::temp_dir().join(format!(
            "devcraft-lock-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let state = dir
            .join("mcp-central-docs")
            .join("fleet-gate")
            .join("state");
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(
            state.join("lock-findings.json"),
            r#"{"errors":[{"code":"E1","severity":"error","message":"10764 claimed twice"}],"warnings":[{"code":"W1","message":"gap"}]}"#,
        )
        .unwrap();
        let found = PortSentinel::lock_findings(&dir);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].code, "E1");
        assert!(found[0].message.contains("10764"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
