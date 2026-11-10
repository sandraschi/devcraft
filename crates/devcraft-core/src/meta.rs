//! Meta-MCP capability search and tool discovery engine.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredServer {
    pub name: String,
    pub path: String,
    pub has_mcp_config: bool,
    pub description: String,
}

pub struct MetaMcpDepot;

impl MetaMcpDepot {
    /// Discover MCP servers in the fleet workspace and search by keyword.
    pub fn discover_servers(workspace_root: &Path, query: Option<&str>) -> Vec<DiscoveredServer> {
        let mut results = Vec::new();
        let entries = match std::fs::read_dir(workspace_root) {
            Ok(rd) => rd,
            Err(_) => return results,
        };

        let q = query.unwrap_or("").to_lowercase();

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = match path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => continue,
            };

            let is_mcp = name.ends_with("-mcp") || name == "meta_mcp" || path.join("fleet-start.config.ps1").exists();
            if !is_mcp {
                continue;
            }

            let mut description = String::new();
            let readme_path = path.join("README.md");
            if let Ok(content) = std::fs::read_to_string(&readme_path) {
                if let Some(first_line) = content.lines().find(|l| !l.trim().is_empty()) {
                    description = first_line.trim_start_matches('#').trim().to_string();
                }
            }

            if !q.is_empty() {
                let matches_name = name.to_lowercase().contains(&q);
                let matches_desc = description.to_lowercase().contains(&q);
                if !matches_name && !matches_desc {
                    continue;
                }
            }

            results.push(DiscoveredServer {
                name,
                path: path.to_string_lossy().to_string(),
                has_mcp_config: path.join("fleet-start.config.ps1").exists(),
                description,
            });
        }

        results.sort_by(|a, b| a.name.cmp(&b.name));
        results
    }
}
