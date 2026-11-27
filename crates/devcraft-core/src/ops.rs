//! Scheduled tasks, the logs those jobs and the IDEs write, and the unified
//! monitoring stack. Parsing stays pure. The scheduler query is the only spawn.

use serde::Deserialize;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

const READ_CAP: u64 = 256_000;

#[derive(Clone, Copy)]
struct Spec {
    name: &'static str,
    summary: &'static str,
    report: ReportAt,
}

#[derive(Clone, Copy)]
enum ReportAt {
    None,
    File(&'static str),
    Latest {
        dir: &'static str,
        prefix: &'static str,
    },
    Local(&'static str),
    Absolute(&'static str),
}

const CATALOG: &[Spec] = &[
    Spec {
        name: "Fleet-Nightly-Gate",
        summary: "Nightly port contract, launcher check, health, and one cold-start shard.",
        report: ReportAt::File("mcp-central-docs/fleet-gate/state/DASHBOARD.md"),
    },
    Spec {
        name: "Fleet-Install-Matrix",
        summary: "Sunday naked-PC install of one quarter of the fleet.",
        report: ReportAt::Latest {
            dir: "mcp-central-docs/fleet-gate/state",
            prefix: "install-matrix-",
        },
    },
    Spec {
        name: "GitHub-Fleet-Morning-Digest",
        summary: "Daily GitHub digest, mirrored to fleet-status.md.",
        report: ReportAt::File("mcp-central-docs/operations/fleet-status.md"),
    },
    Spec {
        name: "GitHub-Fleet-Weekly-Suite",
        summary: "Sunday maintainer suite. The file copy is under LocalAppData.",
        report: ReportAt::Local("git-github-mcp/morning-digest.md"),
    },
    Spec {
        name: "Fleet-mcd-rag-sync",
        summary: "Incremental RAG index of mcp-central-docs. Full re-index on Sunday.",
        report: ReportAt::Latest {
            dir: "mcp-central-docs/logs",
            prefix: "rag-sync-",
        },
    },
    Spec {
        name: "Fleet-mcd-shed-sync",
        summary: "Copy mcp-central-docs into the documentation shed. Does not commit.",
        report: ReportAt::Latest {
            dir: "mcp-central-docs/logs",
            prefix: "shed-sync-",
        },
    },
    Spec {
        name: "Fleet-NSSM-boot-check",
        summary: "After logon, record which NSSM services are stopped.",
        report: ReportAt::Absolute(r"C:\ProgramData\nssm-boot-check.log"),
    },
    Spec {
        name: "Fleet-Nightly-OrphanSid-Scan",
        summary: "Nightly report of orphan-SID paths. Does not delete repos.",
        report: ReportAt::Latest {
            dir: "mcp-central-docs/operations/session-log",
            prefix: "fritz-orphan-sweep-",
        },
    },
    Spec {
        name: "Fleet-CI-Failure-Triage",
        summary: "Every two hours, classify new GitHub Actions failures.",
        report: ReportAt::Latest {
            dir: "mcp-central-docs/operations/daily",
            prefix: "ci-triage-",
        },
    },
    Spec {
        name: "Fleet-fleet-overview",
        summary: "Sunday Discord post of the active-repo index.",
        report: ReportAt::File("mcp-central-docs/projects/FLEET_INDEX.md"),
    },
    Spec {
        name: "StartsSweep",
        summary: "Daily click-test of every starts launcher, then the ports are torn down.",
        report: ReportAt::Latest {
            dir: "mcp-central-docs/reports",
            prefix: "starts-scan-",
        },
    },
    Spec {
        name: "WipSnapshot",
        summary: "Nightly WIP snapshot under refs/wip. It does not commit a branch.",
        report: ReportAt::None,
    },
    Spec {
        name: "WipSweep",
        summary: "Old sweep that committed WIP onto the branch. Left disabled on purpose.",
        report: ReportAt::Latest {
            dir: "mcp-central-docs/operations/session-log",
            prefix: "wip-sweep-",
        },
    },
    Spec {
        name: "Fleet-orphan-reaper",
        summary: "Every 10 minutes, reap orphan processes left by fleet services.",
        report: ReportAt::None,
    },
];

#[derive(Debug, Clone, Deserialize)]
struct RawTask {
    name: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    schedule: String,
    #[serde(default, rename = "lastRun")]
    last_run: String,
    #[serde(default, rename = "nextRun")]
    next_run: String,
    result: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskRow {
    pub id: String,
    pub name: String,
    pub state: String,
    pub schedule: String,
    pub last_run: String,
    pub next_run: String,
    pub result: String,
    pub summary: String,
    pub report_path: String,
    pub report_present: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LogFile {
    pub id: String,
    pub source: String,
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TeleRow {
    pub id: String,
    pub name: String,
    pub url: String,
    pub port: u16,
    pub role: String,
    pub listening: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OpsSnap {
    pub ready: bool,
    pub note: String,
    pub tasks: Vec<TaskRow>,
    pub logs: Vec<LogFile>,
    pub telemetry: Vec<TeleRow>,
}

impl OpsSnap {
    pub fn pending() -> Self {
        Self {
            ready: false,
            note: "Reading scheduled tasks.".into(),
            tasks: Vec::new(),
            logs: Vec::new(),
            telemetry: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Shown {
    pub excerpt: String,
    pub findings: Vec<String>,
}

struct Target {
    name: &'static str,
    port: u16,
    role: &'static str,
}

const STACK: &[Target] = &[
    Target {
        name: "Grafana",
        port: 12000,
        role: "Dashboards",
    },
    Target {
        name: "Prometheus",
        port: 12001,
        role: "Metrics",
    },
    Target {
        name: "Loki",
        port: 12002,
        role: "Log store",
    },
    Target {
        name: "Promtail",
        port: 12003,
        role: "Log shipper",
    },
    Target {
        name: "node-exporter",
        port: 12004,
        role: "Host metrics",
    },
    Target {
        name: "cAdvisor",
        port: 12005,
        role: "Container metrics",
    },
    Target {
        name: "blackbox",
        port: 12006,
        role: "Probe exporter",
    },
    Target {
        name: "observability-mcp",
        port: 12007,
        role: "Monitoring API",
    },
    Target {
        name: "observability web",
        port: 12008,
        role: "Monitoring UI",
    },
];

pub fn load(root: &Path) -> OpsSnap {
    let (live, note) = match query_tasks() {
        Ok(text) => match parse_tasks(&text) {
            Ok(rows) => (rows, String::new()),
            Err(err) => (Vec::new(), err),
        },
        Err(err) => (Vec::new(), err),
    };
    let local = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    OpsSnap {
        ready: true,
        note,
        tasks: assemble(&live, root, &local),
        logs: log_files(root),
        telemetry: probe_stack(),
    }
}

fn parse_tasks(text: &str) -> Result<Vec<RawTask>, String> {
    let text = text.trim().trim_start_matches('\u{feff}');
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(text).map_err(|e| format!("scheduler json: {e}"))?;
    let items = match value {
        Value::Array(items) => items,
        Value::Object(_) => vec![value],
        Value::Null => return Ok(Vec::new()),
        _ => return Err("scheduler json was not a list".into()),
    };
    items
        .into_iter()
        .map(|item| serde_json::from_value(item).map_err(|e| format!("scheduler row: {e}")))
        .collect()
}

fn assemble(live: &[RawTask], root: &Path, local: &Path) -> Vec<TaskRow> {
    let mut rows = Vec::new();
    let mut seen = Vec::new();
    for spec in CATALOG {
        let hit = live.iter().find(|row| row.name == spec.name);
        if let Some(row) = hit {
            seen.push(row.name.clone());
        }
        rows.push(task_row(spec, hit, root, local));
    }
    let mut extra: Vec<&RawTask> = live
        .iter()
        .filter(|row| !seen.iter().any(|name| name == &row.name))
        .collect();
    extra.sort_by(|a, b| a.name.cmp(&b.name));
    for row in extra {
        let summary = if row.name.starts_with("Fleet-") {
            format!(
                "Every 20 minutes, check that {} is up and restart it if it is not.",
                row.name.trim_start_matches("Fleet-")
            )
        } else {
            "Fleet scheduled task.".into()
        };
        rows.push(TaskRow {
            id: format!("task:{}", row.name),
            name: row.name.clone(),
            state: dash(&row.state),
            schedule: dash(&row.schedule),
            last_run: dash(&row.last_run),
            next_run: dash(&row.next_run),
            result: result_label(row.result),
            summary,
            report_path: String::new(),
            report_present: false,
        });
    }
    rows
}

fn task_row(spec: &Spec, live: Option<&RawTask>, root: &Path, local: &Path) -> TaskRow {
    let (report_path, report_present) = resolve(spec.report, root, local);
    let (state, schedule, last_run, next_run, result) = if let Some(row) = live {
        (
            dash(&row.state),
            dash(&row.schedule),
            dash(&row.last_run),
            dash(&row.next_run),
            result_label(row.result),
        )
    } else {
        (
            "not registered".into(),
            "-".into(),
            "-".into(),
            "-".into(),
            "-".into(),
        )
    };
    TaskRow {
        id: format!("task:{}", spec.name),
        name: spec.name.into(),
        state,
        schedule,
        last_run,
        next_run,
        result,
        summary: spec.summary.into(),
        report_path,
        report_present,
    }
}

fn dash(text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        "-".into()
    } else {
        text.into()
    }
}

pub fn result_label(code: Option<i64>) -> String {
    match code {
        None => "-".into(),
        Some(0) => "ok".into(),
        Some(1) => "failed".into(),
        Some(267009) => "running".into(),
        Some(267011) => "not run yet".into(),
        Some(other) => format!("result {other}"),
    }
}

fn resolve(report: ReportAt, root: &Path, local: &Path) -> (String, bool) {
    match report {
        ReportAt::None => (String::new(), false),
        ReportAt::File(rel) => {
            let path = root.join(rel);
            (path.display().to_string(), path.is_file())
        }
        ReportAt::Absolute(path) => {
            let path = PathBuf::from(path);
            (path.display().to_string(), path.is_file())
        }
        ReportAt::Local(rel) => {
            let path = local.join(rel);
            (path.display().to_string(), path.is_file())
        }
        ReportAt::Latest { dir, prefix } => {
            let dir = root.join(dir);
            match newest(&dir, prefix) {
                Some(path) => (path.display().to_string(), true),
                None => (format!("{}\\{prefix}*", dir.display()), false),
            }
        }
    }
}

pub fn newest(dir: &Path, prefix: &str) -> Option<PathBuf> {
    let mut best: Option<(SystemTime, String, PathBuf)> = None;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !name.starts_with(prefix) {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|(when, best_name, _)| (modified, &name) > (*when, best_name))
        {
            best = Some((modified, name, path));
        }
    }
    best.map(|(_, _, path)| path)
}

pub fn log_files(root: &Path) -> Vec<LogFile> {
    let mut files = Vec::new();
    push_newest(
        &mut files,
        "Fleet gate",
        &root.join("mcp-central-docs/fleet-gate/state"),
        ".log",
        8,
    );
    push_newest(
        &mut files,
        "Sync",
        &root.join("mcp-central-docs/logs"),
        ".log",
        8,
    );
    push_file(
        &mut files,
        "NSSM",
        Path::new(r"C:\ProgramData\nssm-boot-check.log"),
    );
    push_file(
        &mut files,
        "NSSM",
        Path::new(r"C:\ProgramData\nssm-boot-repair.log"),
    );
    if let Ok(appdata) = std::env::var("APPDATA") {
        push_mcp(
            &mut files,
            "Cursor MCP",
            &PathBuf::from(appdata).join("Cursor/logs"),
        );
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let home = PathBuf::from(profile);
        push_mcp(&mut files, "Claude MCP", &home.join(".claude"));
        push_mcp(&mut files, "Codex MCP", &home.join(".codex"));
        push_mcp(
            &mut files,
            "OpenCode MCP",
            &home.join(".local/share/opencode"),
        );
    }
    for (idx, file) in files.iter_mut().enumerate() {
        file.id = format!("log:{idx}");
    }
    files
}

fn push_newest(out: &mut Vec<LogFile>, source: &str, dir: &Path, suffix: &str, cap: usize) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !path.is_file() || !name.ends_with(suffix) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let when = meta.modified().unwrap_or(UNIX_EPOCH);
        found.push((when, path, meta.len()));
    }
    found.sort_by_key(|row| std::cmp::Reverse(row.0));
    found.truncate(cap);
    for (_, path, len) in found {
        out.push(LogFile {
            id: String::new(),
            source: source.into(),
            path: path.display().to_string(),
            bytes: len,
        });
    }
}

fn push_file(out: &mut Vec<LogFile>, source: &str, path: &Path) {
    if let Ok(meta) = fs::metadata(path) {
        if meta.is_file() {
            out.push(LogFile {
                id: String::new(),
                source: source.into(),
                path: path.display().to_string(),
                bytes: meta.len(),
            });
        }
    }
}

fn push_mcp(out: &mut Vec<LogFile>, source: &str, dir: &Path) {
    if !dir.is_dir() {
        return;
    }
    let mut found = Vec::new();
    for entry in WalkDir::new(dir)
        .max_depth(5)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !name.contains("mcp") || !(name.ends_with(".log") || name.ends_with(".txt")) {
            continue;
        }
        let when = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(UNIX_EPOCH);
        let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
        found.push((when, entry.into_path(), len));
        if found.len() > 60 {
            break;
        }
    }
    found.sort_by_key(|row| std::cmp::Reverse(row.0));
    found.truncate(6);
    for (_, path, len) in found {
        out.push(LogFile {
            id: String::new(),
            source: source.into(),
            path: path.display().to_string(),
            bytes: len,
        });
    }
}

pub fn probe_stack() -> Vec<TeleRow> {
    STACK
        .iter()
        .map(|target| TeleRow {
            id: format!("tele:{}", target.port),
            name: target.name.into(),
            url: format!("http://127.0.0.1:{}/", target.port),
            port: target.port,
            role: target.role.into(),
            listening: TcpStream::connect_timeout(
                &SocketAddr::from(([127, 0, 0, 1], target.port)),
                Duration::from_millis(200),
            )
            .is_ok(),
        })
        .collect()
}

pub fn show_file(path: &Path) -> Shown {
    let tail = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("log"));
    let text = read_capped(path, tail);
    let findings = analyze(&text);
    let excerpt = if findings.is_empty() {
        text.lines().take(80).collect::<Vec<_>>().join("\n")
    } else {
        findings.join("\n")
    };
    Shown { excerpt, findings }
}

pub fn analyze(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !interesting(trimmed) {
            continue;
        }
        out.push(trimmed.to_string());
        if out.len() == 40 {
            break;
        }
    }
    out
}

fn interesting(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if lower.contains("fail=0") || lower.contains("0 errors") || lower.contains("0 new") {
        return false;
    }
    lower.contains("exit 1")
        || lower.contains("[fail]")
        || lower.contains("not clean")
        || lower.contains("new since baseline")
        || lower.contains("timeout")
        || lower.contains(" error")
        || lower.contains("failed")
}

fn read_capped(path: &Path, tail: bool) -> String {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return String::new(),
    };
    let len = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    if tail && len > READ_CAP {
        let _ = file.seek(SeekFrom::End(-(READ_CAP as i64)));
    }
    let mut buf = Vec::new();
    let _ = file.take(READ_CAP).read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

fn query_tasks() -> Result<String, String> {
    let exe = if Path::new(r"C:\Program Files\PowerShell\7\pwsh.exe").is_file() {
        r"C:\Program Files\PowerShell\7\pwsh.exe"
    } else {
        "powershell.exe"
    };
    let output = Command::new(exe)
        .args(["-NoProfile", "-NonInteractive", "-Command", SCHEDULER])
        .output()
        .map_err(|err| format!("scheduler query did not start: {err}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("scheduler query failed: {}", err.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

const SCHEDULER: &str = r#"
$ErrorActionPreference = 'Continue'
function Format-When($trg) {
  $kind = $trg.CimClass.CimClassName
  $clock = ''
  if ($trg.StartBoundary) { $clock = ([datetime]$trg.StartBoundary).ToString('HH:mm') }
  $label = 'scheduled'
  if ($kind -eq 'MSFT_TaskDailyTrigger') { $label = "daily $clock" }
  elseif ($kind -eq 'MSFT_TaskWeeklyTrigger') { $label = "weekly $($trg.DaysOfWeek) $clock" }
  elseif ($kind -eq 'MSFT_TaskLogonTrigger') { $label = 'at logon' }
  elseif ($kind -eq 'MSFT_TaskBootTrigger') { $label = 'at startup' }
  elseif ($clock -ne '') { $label = "at $clock" }
  if ($trg.Delay) { $label = "$label +$($trg.Delay)" }
  if ($trg.Repetition -and $trg.Repetition.Interval) { $label = "$label every $($trg.Repetition.Interval)" }
  return $label.Trim()
}
$script:seen = @{}
$script:bag = New-Object System.Collections.Generic.List[object]
function Add-Task($t) {
  if (-not $t) { return }
  $key = [string]$t.TaskPath + '|' + [string]$t.TaskName
  if ($script:seen.ContainsKey($key)) { return }
  $script:seen[$key] = $true
  $null = $script:bag.Add($t)
}
foreach ($t in @(Get-ScheduledTask -TaskPath '\Fleet\' -ErrorAction SilentlyContinue)) { Add-Task $t }
foreach ($n in @('Fleet-CI-Failure-Triage','Fleet-fleet-overview','Fleet-Install-Matrix','Fleet-Nightly-Gate','Fleet-Nightly-OrphanSid-Scan','GitHub-Fleet-Morning-Digest','GitHub-Fleet-Weekly-Suite')) {
  Add-Task (Get-ScheduledTask -TaskName $n -ErrorAction SilentlyContinue)
}
$rows = New-Object System.Collections.Generic.List[object]
foreach ($t in $script:bag) {
  $info = Get-ScheduledTaskInfo -TaskName $t.TaskName -TaskPath $t.TaskPath
  $last = ''
  $next = ''
  if ($info.LastRunTime -and $info.LastRunTime.Year -gt 2000) { $last = $info.LastRunTime.ToString('yyyy-MM-dd HH:mm') }
  if ($info.NextRunTime -and $info.NextRunTime.Year -gt 2000) { $next = $info.NextRunTime.ToString('yyyy-MM-dd HH:mm') }
  $sched = @()
  foreach ($trg in @($t.Triggers)) { $sched += (Format-When $trg) }
  $null = $rows.Add([ordered]@{
    name = [string]$t.TaskName
    state = [string]$t.State
    schedule = ($sched -join ', ')
    lastRun = $last
    nextRun = $next
    result = [int]$info.LastTaskResult
  })
}
if ($rows.Count -eq 0) { '[]' } else { ConvertTo-Json -InputObject @($rows.ToArray()) -Compress -Depth 4 }
"#;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_one_object_and_a_list() {
        let one = r#"{"name":"Fleet-Nightly-Gate","state":"Ready","schedule":"daily 03:30","lastRun":"2026-11-14 02:30","nextRun":"2026-11-15 02:30","result":1}"#;
        let rows = parse_tasks(one).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(result_label(rows[0].result), "failed");
        let list = r#"[{"name":"A","result":0},{"name":"B"}]"#;
        let rows = parse_tasks(list).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(result_label(rows[1].result), "-");
    }

    #[test]
    fn catalog_row_points_at_the_newest_report() {
        let dir = std::env::temp_dir().join(format!(
            "devcraft-ops-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let state = dir
            .join("mcp-central-docs")
            .join("fleet-gate")
            .join("state");
        fs::create_dir_all(&state).unwrap();
        fs::write(state.join("install-matrix-2026-11-01.json"), "old").unwrap();
        fs::write(state.join("install-matrix-2026-11-08.json"), "new").unwrap();
        let live = vec![RawTask {
            name: "Fleet-Install-Matrix".into(),
            state: "Ready".into(),
            schedule: "weekly Sunday 04:30".into(),
            last_run: "2026-11-08 04:30".into(),
            next_run: "2026-11-15 03:30".into(),
            result: Some(1),
        }];
        let rows = assemble(&live, &dir, Path::new("."));
        let row = rows
            .iter()
            .find(|r| r.name == "Fleet-Install-Matrix")
            .unwrap();
        assert!(row.report_present);
        assert!(row.report_path.ends_with("install-matrix-2026-11-08.json"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_fleet_task_is_a_watchdog() {
        let live = vec![RawTask {
            name: "Fleet-example-mcp".into(),
            state: "Ready".into(),
            schedule: "every PT20M".into(),
            last_run: "2026-11-14 12:09".into(),
            next_run: "2026-11-14 12:29".into(),
            result: Some(0),
        }];
        let row = assemble(&live, Path::new("."), Path::new("."))
            .into_iter()
            .find(|r| r.name == "Fleet-example-mcp")
            .unwrap();
        assert!(row.summary.contains("example-mcp"));
        assert!(!row.report_present);
    }

    #[test]
    fn analysis_keeps_failures_and_skips_a_clean_count() {
        let text =
            "0 errors (0 known, 0 NEW)\n[fail] artcraft\nexit 1\nnot clean\nfail=0 is fine\n";
        let found = analyze(text);
        assert!(found.iter().any(|line| line.contains("[fail]")));
        assert!(found.iter().any(|line| line.contains("exit 1")));
        assert!(!found
            .iter()
            .any(|line| line.contains("0 errors") || line.contains("fail=0")));
    }
}
