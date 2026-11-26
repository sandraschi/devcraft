//! DevCraft core: fleet board, claims, digest, reports, and the host probes.
#![forbid(unsafe_code)]

pub mod backup;
pub mod adn;
pub mod board;
pub mod cockpit;
pub mod digest;
pub mod fleet;
pub mod freshness;
pub mod graphs;
pub mod handover;
pub mod meta;
pub mod ops;
pub mod ports;
pub mod processes;
pub mod sandbox;
pub mod services;
pub mod skills;
pub mod todos;
pub mod workflows;

pub const APP_VERSION: &str = "0.2.1";
pub const FRONTEND_PORT: u16 = 11258;
pub const CONTROL_PORT: u16 = 11259;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Strip Windows verbatim prefix (`\\?\`) for human-readable display and JSON output.
pub fn clean_display_path(path: &Path) -> String {
    let raw = path.display().to_string();
    raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_string()
}

/// Resolve the fleet workspace root (`D:\Dev\repos`) whether called from root,
/// inside a sibling repo, or launched from an IDE install directory.
pub fn resolve_workspace_root() -> PathBuf {
    if let Ok(env_root) = std::env::var("DEVCRAFT_ROOT") {
        let trimmed = env_root.trim();
        if !trimmed.is_empty() {
            let candidate = PathBuf::from(trimmed);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        for ancestor in cwd.ancestors() {
            if ancestor.join("mcp-central-docs").is_dir() {
                return ancestor.to_path_buf();
            }
        }
    }
    let windows_fleet = PathBuf::from(r"D:\Dev\repos");
    if windows_fleet.join("mcp-central-docs").is_dir() {
        return windows_fleet;
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// A Storytold checkout lives under `_upstream`. A same-named folder at the fleet root is a copy
/// and is not one of our repos.
pub fn mirrored_upstream(root: &Path, name: &str) -> bool {
    root.join("_upstream").join(name).join(".git").exists()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevCraftStatus {
    pub version: String,
    pub control_port: u16,
    pub frontend_port: u16,
    pub active_subsystems: Vec<String>,
}

impl Default for DevCraftStatus {
    fn default() -> Self {
        Self {
            version: APP_VERSION.to_string(),
            control_port: CONTROL_PORT,
            frontend_port: FRONTEND_PORT,
            active_subsystems: vec![
                "fleet_governance".to_string(),
                "time_machine_backup".to_string(),
                "port_sentinel".to_string(),
                "service_watchdog".to_string(),
                "container_sandbox".to_string(),
                "meta_mcp_depot".to_string(),
            ],
        }
    }
}
