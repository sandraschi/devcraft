//! DevCraft Core Engine: Fleet governance, atomic backups, port/process sentinels,
//! and container sandbox execution.
//!
//! Enforces zero-panic, zero-unwrap containment across all modules.

pub mod backup;
pub mod fleet;
pub mod meta;
pub mod ports;
pub mod sandbox;
pub mod services;

pub const APP_VERSION: &str = "0.1.0";
pub const FRONTEND_PORT: u16 = 11258;
pub const CONTROL_PORT: u16 = 11259;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Resolve the fleet workspace root (`d:\Dev\repos`) whether called from root or inside `devcraft`.
pub fn resolve_workspace_root() -> PathBuf {
    if Path::new("mcp-central-docs").exists() {
        PathBuf::from(".")
    } else if Path::new("../mcp-central-docs").exists() {
        PathBuf::from("..")
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
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
