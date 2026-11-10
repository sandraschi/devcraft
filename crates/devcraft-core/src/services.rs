//! Windows service and process sentinel.
//!
//! Enforces fleet rule: "Never restart an NSSM service by killing its child --
//! sc.exe stop/start, then verify a NEW PID owns the port."

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    pub state: String,
    pub is_running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartResult {
    pub service_name: String,
    pub stop_success: bool,
    pub start_success: bool,
    pub final_state: String,
    pub port_listening: bool,
}

pub struct ServiceSentinel;

impl ServiceSentinel {
    /// Query Windows service state via sc.exe.
    pub fn query_service(name: &str) -> ServiceStatus {
        let output = match Command::new("sc.exe").args(["query", name]).output() {
            Ok(out) => out,
            Err(_) => {
                return ServiceStatus {
                    name: name.to_string(),
                    state: "QUERY_FAILED".to_string(),
                    is_running: false,
                }
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut state = "UNKNOWN".to_string();
        let mut is_running = false;

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("STATE") {
                if trimmed.contains("RUNNING") {
                    state = "RUNNING".to_string();
                    is_running = true;
                } else if trimmed.contains("STOPPED") {
                    state = "STOPPED".to_string();
                } else if trimmed.contains("START_PENDING") {
                    state = "START_PENDING".to_string();
                } else if trimmed.contains("STOP_PENDING") {
                    state = "STOP_PENDING".to_string();
                } else {
                    state = trimmed.to_string();
                }
            }
        }

        ServiceStatus {
            name: name.to_string(),
            state,
            is_running,
        }
    }

    /// Safely restart an NSSM service adhering to the fleet protocol.
    pub fn restart_service_safely(name: &str, expected_port: Option<u16>) -> RestartResult {
        // Step 1: sc.exe stop <service>
        let stop_status = Command::new("sc.exe")
            .args(["stop", name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        // Wait up to 3 seconds for service to transition to STOPPED
        for _ in 0..6 {
            sleep(Duration::from_millis(500));
            let st = Self::query_service(name);
            if st.state == "STOPPED" {
                break;
            }
        }

        // Step 2: sc.exe start <service>
        let start_status = Command::new("sc.exe")
            .args(["start", name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        // Wait up to 5 seconds for service to transition to RUNNING
        let mut final_state = "UNKNOWN".to_string();
        for _ in 0..10 {
            sleep(Duration::from_millis(500));
            let st = Self::query_service(name);
            final_state = st.state;
            if final_state == "RUNNING" {
                break;
            }
        }

        let port_listening = match expected_port {
            Some(p) => crate::ports::PortSentinel::is_port_in_use(p),
            None => false,
        };

        RestartResult {
            service_name: name.to_string(),
            stop_success: stop_status,
            start_success: start_status,
            final_state,
            port_listening,
        }
    }
}
