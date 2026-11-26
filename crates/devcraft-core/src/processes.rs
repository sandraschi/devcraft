//! Process inspector and stale-process cleanup for DevCraft.
//!
//! Detects duplicate or long-lived `devcraft-mcp` instances (for example
//! orphaned stdio servers from restarted IDE sessions) while always protecting
//! the currently executing process.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Command;

/// Processes older than this threshold with duplicate paths or orphaned parents are flagged stale.
pub const DEFAULT_STALE_HOURS: u64 = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessEntry {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub path: String,
    pub age_hours: Option<u64>,
    pub stale: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessReport {
    pub current_pid: u32,
    pub total: usize,
    pub stale_count: usize,
    pub entries: Vec<ProcessEntry>,
}

/// Classify raw process rows into stale vs active entries.
///
/// Never marks `current_pid` as stale. Marks an entry stale when:
/// - Another process with the same executable path is already running (keeps newest or `current_pid`), or
/// - Its `age_hours` exceeds `max_age_hours` and it is a `devcraft-mcp` stdio worker.
pub fn classify_processes(
    mut entries: Vec<ProcessEntry>,
    current_pid: u32,
    max_age_hours: u64,
) -> ProcessReport {
    // Count executable paths (lowercased on Windows).
    let mut by_path: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, e) in entries.iter().enumerate() {
        if e.name.to_lowercase().contains("devcraft-mcp") {
            let key = e.path.to_lowercase();
            by_path.entry(key).or_default().push(idx);
        }
    }

    // For each duplicated path, keep current_pid if present, otherwise keep the youngest (smallest age_hours).
    for indices in by_path.values() {
        if indices.len() <= 1 {
            continue;
        }
        let keeper = indices
            .iter()
            .copied()
            .find(|&i| entries[i].pid == current_pid)
            .unwrap_or_else(|| {
                *indices
                    .iter()
                    .min_by_key(|&&i| entries[i].age_hours.unwrap_or(u64::MAX))
                    .unwrap_or(&indices[0])
            });
        for &i in indices {
            if i != keeper && entries[i].pid != current_pid {
                entries[i].stale = true;
                entries[i].reason = "duplicate devcraft-mcp instance".into();
            }
        }
    }

    for e in &mut entries {
        if e.pid == current_pid {
            e.stale = false;
            e.reason = "current process".into();
            continue;
        }
        if !e.stale
            && e.name.to_lowercase().contains("devcraft-mcp")
            && e.age_hours.is_some_and(|h| h >= max_age_hours)
        {
            e.stale = true;
            e.reason = format!("running >= {}h", max_age_hours);
        }
    }

    let stale_count = entries.iter().filter(|e| e.stale).count();
    ProcessReport {
        current_pid,
        total: entries.len(),
        stale_count,
        entries,
    }
}

/// Inspect running DevCraft processes on the host.
pub fn inspect_devcraft_processes(max_age_hours: u64) -> ProcessReport {
    let current_pid = std::process::id();
    let entries = query_host_processes();
    classify_processes(entries, current_pid, max_age_hours)
}

/// Terminate stale `devcraft-mcp` processes (excluding the current process).
pub fn kill_stale_processes(max_age_hours: u64) -> Result<Value, String> {
    let report = inspect_devcraft_processes(max_age_hours);
    let mut killed: Vec<u32> = Vec::new();
    let mut failed: Vec<String> = Vec::new();

    for entry in &report.entries {
        if !entry.stale || entry.pid == report.current_pid {
            continue;
        }
        if stop_pid(entry.pid) {
            killed.push(entry.pid);
        } else {
            failed.push(format!("PID {}", entry.pid));
        }
    }

    Ok(json!({
        "killed_count": killed.len(),
        "killed_pids": killed,
        "failed": failed,
        "remaining_total": report.total.saturating_sub(killed.len()),
    }))
}

#[cfg(target_os = "windows")]
fn query_host_processes() -> Vec<ProcessEntry> {
    let script = r#"$now = Get-Date; $rows = @(Get-CimInstance Win32_Process -Filter "Name like '%devcraft%'" -ErrorAction SilentlyContinue | ForEach-Object { $age = if ($_.CreationDate) { [math]::Floor(($now - $_.CreationDate).TotalHours) } else { $null }; [pscustomobject]@{ pid = [uint32]$_.ProcessId; parent_pid = [uint32]$_.ParentProcessId; name = [string]$_.Name; path = [string]$_.ExecutablePath; age_hours = $age } }); if ($rows.Count -eq 0) { '[]' } else { ConvertTo-Json -InputObject @($rows) -Compress }"#;
    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", script])
        .output();
    let Ok(out) = out else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    parse_process_json(text.trim())
}

#[cfg(not(target_os = "windows"))]
fn query_host_processes() -> Vec<ProcessEntry> {
    Vec::new()
}

#[cfg(target_os = "windows")]
fn stop_pid(pid: u32) -> bool {
    Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
        .is_ok_and(|o| o.status.success())
}

#[cfg(not(target_os = "windows"))]
fn stop_pid(_pid: u32) -> bool {
    false
}

fn parse_process_json(text: &str) -> Vec<ProcessEntry> {
    if text.is_empty() {
        return Vec::new();
    }
    let Ok(val) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let arr = match val {
        Value::Array(items) => items,
        Value::Object(_) => vec![val],
        _ => return Vec::new(),
    };
    arr.into_iter()
        .filter_map(|item| {
            let pid = item.get("pid")?.as_u64()? as u32;
            let parent_pid = item.get("parent_pid").and_then(Value::as_u64).unwrap_or(0) as u32;
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let path = item
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let age_hours = item.get("age_hours").and_then(Value::as_u64);
            Some(ProcessEntry {
                pid,
                parent_pid,
                name,
                path,
                age_hours,
                stale: false,
                reason: String::new(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_mcp_processes_keep_youngest_and_protect_self() {
        let rows = vec![
            ProcessEntry {
                pid: 100,
                parent_pid: 1,
                name: "devcraft-mcp.exe".into(),
                path: r"C:\Apps\DevCraft\devcraft-mcp.exe".into(),
                age_hours: Some(20),
                stale: false,
                reason: String::new(),
            },
            ProcessEntry {
                pid: 200,
                parent_pid: 1,
                name: "devcraft-mcp.exe".into(),
                path: r"C:\Apps\DevCraft\devcraft-mcp.exe".into(),
                age_hours: Some(1),
                stale: false,
                reason: String::new(),
            },
            ProcessEntry {
                pid: 300,
                parent_pid: 1,
                name: "devcraft-mcp.exe".into(),
                path: r"C:\Other\devcraft-mcp.exe".into(),
                age_hours: Some(25),
                stale: false,
                reason: String::new(),
            },
        ];
        let report = classify_processes(rows, 200, 12);
        assert_eq!(report.total, 3);
        assert_eq!(report.stale_count, 2);
        assert!(report.entries[0].stale);
        assert!(!report.entries[1].stale);
        assert!(report.entries[2].stale);
    }

    #[test]
    fn parses_single_or_array_json() {
        let one = parse_process_json(
            r#"{"pid":42,"parent_pid":7,"name":"devcraft-mcp.exe","path":"C:\\x.exe","age_hours":3}"#,
        );
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].pid, 42);
        assert_eq!(one[0].age_hours, Some(3));
    }
}
