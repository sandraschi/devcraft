//! Port sentinel and registry reconciliation module.

use serde::{Deserialize, Serialize};
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
}

pub struct PortSentinel;

impl PortSentinel {
    /// Test if a local TCP port is currently occupied / listening on 127.0.0.1.
    pub fn is_port_in_use(port: u16) -> bool {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(_) => false,
            Err(_) => true,
        }
    }

    /// Parse `operations/WEBAPP_PORTS.md` from mcp-central-docs and test active sockets.
    pub fn audit_ports(workspace_root: &Path) -> PortAuditReport {
        let ports_doc = workspace_root
            .join("mcp-central-docs")
            .join("operations")
            .join("WEBAPP_PORTS.md");

        let mut registered: HashMap<u16, (String, String)> = HashMap::new();

        if let Ok(content) = std::fs::read_to_string(ports_doc) {
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
        }

        let mut entries = Vec::new();
        let mut active_listeners = 0;

        for (&port, (repo, desc)) in &registered {
            let listening = Self::is_port_in_use(port);
            if listening {
                active_listeners += 1;
            }
            entries.push(PortEntry {
                port,
                repo: repo.clone(),
                description: desc.clone(),
                is_listening: listening,
            });
        }

        entries.sort_by_key(|e| e.port);

        // Check common fleet sample range 11250..11270 for any rogue unreserved listener
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
        }
    }
}
