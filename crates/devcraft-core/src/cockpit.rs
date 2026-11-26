//! The cockpit: one state object for the window, the CLI, and MCP.
//! Git, GitHub, and Docker are read from the machine. Missing sources stay empty.

use crate::adn;
use crate::backup::BackupEngine;
use crate::board::{self, Facts, Move};
use crate::digest::{self, Digest};
use crate::freshness;
use crate::handover;
use crate::ops::{self, OpsSnap};
use crate::ports::{LockFinding, PortAuditReport, PortSentinel};
use crate::processes;
use crate::sandbox::ContainerSandbox;
use crate::services::ServiceSentinel;
use crate::skills;
use crate::todos::{self, Todo};
use crate::workflows;
use crate::{clean_display_path, resolve_workspace_root};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const SNOOZE_SECS: u64 = 24 * 3600;
/// Claims older than this threshold (hours) are flagged stale.
pub const STALE_CLAIM_HOURS: u64 = 72;
const EXTRA_CRAFTS: &[&str] = &["craft-deck", "craft-biz", "craft-sandbox-vbox"];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Claim {
    holder: String,
    since: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimInfo {
    pub repo: String,
    pub holder: String,
    pub since: String,
    pub age_hours: Option<u64>,
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportRow {
    pub id: String,
    pub title: String,
    pub path: String,
    pub present: bool,
    pub age_hours: Option<u64>,
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftRow {
    pub name: String,
    pub path: String,
    pub tagline: String,
    pub version: String,
    pub has_binary: bool,
    pub has_handover: bool,
}

/// One Storytold craft clone under `_upstream`. Read from the local git history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamRow {
    pub name: String,
    pub head: String,
    pub subject: String,
    pub date: String,
    pub tag: String,
    pub commits: u32,
    pub delta: String,
    pub subjects: Vec<String>,
    pub changelog: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerRow {
    pub name: String,
    pub image: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerReport {
    pub up: bool,
    pub version: String,
    pub containers: Vec<ContainerRow>,
    pub note: String,
}

/// One checkout for a background git read. The window scans these off the UI thread.
#[derive(Clone)]
pub struct ScanJob {
    pub name: String,
    pub path: PathBuf,
    pub holder: Option<String>,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CardView {
    pub id: String,
    pub column: String,
    pub title: String,
    pub key: String,
    pub detail: String,
    pub child: bool,
    pub holder: Option<String>,
    pub craft: bool,
    pub blocked: Option<String>,
    pub github: bool,
}

pub struct Cockpit {
    pub root: PathBuf,
    repos: Vec<Facts>,
    digest: Digest,
    reports: Vec<ReportRow>,
    crafts: Vec<CraftRow>,
    upstream: Vec<UpstreamRow>,
    upstream_loaded: bool,
    docker: Option<DockerReport>,
    ports: Option<PortAuditReport>,
    claims: BTreeMap<String, Claim>,
    tasks: BTreeMap<String, String>,
    todos: Vec<Todo>,
    pins: BTreeSet<String>,
    snooze: BTreeMap<String, u64>,
    scanned: bool,
    lock: Vec<LockFinding>,
    ops: OpsSnap,
}

impl Cockpit {
    pub fn open(root: impl Into<PathBuf>) -> Result<Cockpit, String> {
        let root = root.into();
        let root = root.canonicalize().unwrap_or(root);
        let mut pit = Cockpit {
            root,
            repos: Vec::new(),
            digest: Digest::missing("not loaded"),
            reports: Vec::new(),
            crafts: Vec::new(),
            upstream: Vec::new(),
            upstream_loaded: false,
            docker: None,
            ports: None,
            claims: BTreeMap::new(),
            tasks: BTreeMap::new(),
            todos: Vec::new(),
            pins: BTreeSet::new(),
            snooze: BTreeMap::new(),
            scanned: false,
            lock: Vec::new(),
            ops: OpsSnap::pending(),
        };
        pit.reload_light()?;
        pit.ensure_registry();
        Ok(pit)
    }

    pub fn from_env() -> Result<Cockpit, String> {
        let root = match std::env::var("DEVCRAFT_ROOT") {
            Ok(p) if !p.trim().is_empty() => PathBuf::from(p),
            _ => resolve_workspace_root(),
        };
        Self::open(root)
    }

    pub fn reload_light(&mut self) -> Result<(), String> {
        self.claims = read_json(&self.state_dir().join("claims.json")).unwrap_or_default();
        self.tasks = read_json(&self.state_dir().join("tasks.json")).unwrap_or_default();
        self.todos = todos::load(&self.state_dir());
        self.pins = read_json(&self.state_dir().join("pins.json")).unwrap_or_default();
        self.snooze = read_json(&self.state_dir().join("snooze.json")).unwrap_or_default();
        self.drop_expired_snooze();
        let digest_path = self
            .root
            .join("mcp-central-docs")
            .join("operations")
            .join("fleet-status.md");
        self.digest = match fs::read_to_string(&digest_path) {
            Ok(text) => {
                let mut parsed = digest::parse(&text);
                parsed.age_hours = mtime_hours(&digest_path);
                parsed
            }
            Err(_) => Digest::missing(format!("missing {}", digest_path.display())),
        };
        self.lock = PortSentinel::lock_findings(&self.root);
        self.reports = report_rows(&self.root);
        self.crafts = craft_rows(&self.root);
        self.upstream.clear();
        self.upstream_loaded = false;
        self.apply_claims_to_repos();
        Ok(())
    }

    pub fn refresh(&mut self) -> Result<Value, String> {
        self.reload_light()?;
        let now = SystemTime::now();
        let mut repos = Vec::new();
        let entries = fs::read_dir(&self.root).map_err(|e| e.to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
                continue;
            };
            if name.starts_with('.') || name.starts_with('_') || name.contains(' ') {
                continue;
            }
            if crate::mirrored_upstream(&self.root, &name) {
                continue;
            }
            repos.push(self.facts_for(&name, &path, now));
        }
        repos.sort_by_key(|a| a.name.to_lowercase());
        self.repos = repos;
        self.scanned = true;
        Ok(json!({
            "scanned": self.repos.len(),
            "shown": self.cards().len(),
        }))
    }

    pub fn cards(&self) -> Vec<CardView> {
        let now = unix_now();
        let mut cards = Vec::new();
        for facts in &self.repos {
            if self.snooze.get(&facts.id).is_some_and(|until| *until > now) {
                continue;
            }
            if !board::shown(facts) {
                continue;
            }
            let column = board::column(facts);
            let detail = if facts.handover_next.is_empty() {
                facts.branch.clone()
            } else {
                truncate(&facts.handover_next, 90)
            };
            cards.push(CardView {
                id: facts.id.clone(),
                column: column.to_string(),
                title: facts.name.clone(),
                key: facts.task_key.clone().unwrap_or_default(),
                detail,
                child: false,
                holder: facts.claimed_by.clone(),
                craft: facts.craft,
                blocked: board::block_reason(facts),
                github: false,
            });
            for tree in &facts.worktrees {
                cards.push(CardView {
                    id: format!("{}#{}", facts.id, tree.name),
                    column: column.to_string(),
                    title: tree.name.clone(),
                    key: String::new(),
                    detail: format!("{} / {}", facts.name, tree.branch),
                    child: true,
                    holder: None,
                    craft: false,
                    blocked: None,
                    github: false,
                });
            }
        }
        for (i, line) in self.digest.needs_reply.iter().enumerate() {
            let id = format!("gh:{i}");
            if self.snooze.get(&id).is_some_and(|until| *until > now) {
                continue;
            }
            let title = if line.title.is_empty() {
                line.repo.clone()
            } else {
                truncate(&line.title, 72)
            };
            cards.push(CardView {
                id,
                column: board::INBOX.to_string(),
                title,
                key: "GH".into(),
                detail: truncate(
                    &format!("{} {} #{}", line.repo, line.kind, line.number),
                    110,
                ),
                child: false,
                holder: None,
                craft: false,
                blocked: None,
                github: true,
            });
        }
        cards
    }

    pub fn quiet_count(&self) -> usize {
        self.repos
            .iter()
            .filter(|f| board::column(f) == board::QUIET)
            .count()
    }

    pub fn claim_details(&self) -> Vec<ClaimInfo> {
        let mut rows: Vec<ClaimInfo> = self
            .claims
            .iter()
            .map(|(repo, claim)| {
                let age_hours = claim_age_hours(&claim.since);
                let stale = age_hours.is_some_and(|h| h >= STALE_CLAIM_HOURS);
                ClaimInfo {
                    repo: repo.clone(),
                    holder: claim.holder.clone(),
                    since: claim.since.clone(),
                    age_hours,
                    stale,
                }
            })
            .collect();
        rows.sort_by_key(|a| a.repo.to_lowercase());
        rows
    }

    pub fn stale_claims_count(&self, max_age_hours: u64) -> usize {
        self.claims
            .values()
            .filter(|c| claim_age_hours(&c.since).is_some_and(|h| h >= max_age_hours))
            .count()
    }

    pub fn status(&self) -> Value {
        let todos_open = self.todos.iter().filter(|t| !t.done).count();
        let digest_stale = self.digest.age_hours.is_some_and(|h| h > 26);
        json!({
            "version": crate::APP_VERSION,
            "transport": "stdio",
            "root": clean_display_path(&self.root),
            "serve_port": crate::CONTROL_PORT,
            "scanned": self.scanned,
            "repos": self.repos.len(),
            "shown": self.cards().len(),
            "quiet": self.quiet_count(),
            "claims": self.claims.len(),
            "stale_claims": self.stale_claims_count(STALE_CLAIM_HOURS),
            "todos_open": todos_open,
            "crafts": self.crafts.len(),
            "digest": self.digest.generated,
            "digest_present": self.digest.present,
            "digest_age_hours": self.digest.age_hours,
            "digest_stale": digest_stale,
            "digest_note": self.digest.note,
            "open_prs": self.digest.open_prs,
            "stale_prs": self.digest.stale_pr_count,
            "open_issues": self.digest.open_issues,
            "dirty_worktrees": self.digest.dirty_worktrees,
            "sync_drift": self.digest.sync_drift,
        })
    }

    pub fn call(&mut self, tool: &str, args: &Value) -> Result<Value, String> {
        match tool {
            "devcraft_status" => Ok(self.status()),
            "devcraft_refresh" => self.refresh(),
            "devcraft_board" => Ok(json!({
                "cards": self.cards(),
                "quiet": self.quiet_count(),
                "scanned": self.scanned,
            })),
            "devcraft_git" => {
                let rows: Vec<Value> = self.repos.iter().map(facts_json).collect();
                Ok(json!({ "repos": rows, "scanned": self.scanned }))
            }
            "devcraft_digest" => serde_json::to_value(&self.digest).map_err(|e| e.to_string()),
            "devcraft_reports" => serde_json::to_value(&self.reports).map_err(|e| e.to_string()),
            "devcraft_crafts" => serde_json::to_value(&self.crafts).map_err(|e| e.to_string()),
            "devcraft_upstream" => {
                self.load_upstream();
                serde_json::to_value(&self.upstream).map_err(|e| e.to_string())
            }
            "devcraft_brief" => {
                let repo = required_str(args, "repo")?;
                let facts = self.repo(repo)?;
                Ok(json!({
                    "repo": facts.name,
                    "next": facts.handover_next,
                    "column": board::column(facts),
                    "holder": facts.claimed_by,
                }))
            }
            "devcraft_claim" => {
                let repo = required_str(args, "repo")?;
                let holder = args
                    .get("holder")
                    .and_then(Value::as_str)
                    .unwrap_or("cursor");
                self.claim(repo, holder)
            }
            "devcraft_release" => {
                let repo = required_str(args, "repo")?;
                self.release(repo)
            }
            "devcraft_release_all" => self.release_all(),
            "devcraft_release_stale_claims" => {
                let hours = args
                    .get("max_age_hours")
                    .and_then(Value::as_u64)
                    .unwrap_or(STALE_CLAIM_HOURS);
                self.release_stale_claims(hours)
            }
            "devcraft_processes" => {
                let hours = args
                    .get("max_age_hours")
                    .and_then(Value::as_u64)
                    .unwrap_or(processes::DEFAULT_STALE_HOURS);
                serde_json::to_value(processes::inspect_devcraft_processes(hours))
                    .map_err(|e| e.to_string())
            }
            "devcraft_kill_stale_processes" => {
                let hours = args
                    .get("max_age_hours")
                    .and_then(Value::as_u64)
                    .unwrap_or(processes::DEFAULT_STALE_HOURS);
                processes::kill_stale_processes(hours)
            }
            "devcraft_task" => {
                let repo = required_str(args, "repo")?;
                let key = args.get("key").and_then(Value::as_str).unwrap_or("");
                self.set_task(repo, key)
            }
            "devcraft_todo_add" => {
                let title = required_str(args, "title")?;
                let due = args.get("due").and_then(Value::as_str).unwrap_or("");
                let detail = args.get("detail").and_then(Value::as_str).unwrap_or("");
                let source = args.get("source").and_then(Value::as_str).unwrap_or("mcp");
                let repo = args.get("repo").and_then(Value::as_str).unwrap_or("");
                self.todo_add(title, due, detail, source, repo)
            }
            "devcraft_todo_list" => {
                let show_done = args
                    .get("show_done")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                Ok(self.todo_list(show_done))
            }
            "devcraft_todo_done" => {
                let id = required_str(args, "id")?;
                self.todo_done(id)
            }
            "devcraft_todo_clear" => self.todo_clear(),
            "devcraft_snapshots" => serde_json::to_value(BackupEngine::list_snapshots(&self.root))
                .map_err(|e| e.to_string()),
            "devcraft_checkpoint" => self.checkpoint_state(),
            "devcraft_sandbox" => Ok(json!({
                "docker": ContainerSandbox::is_docker_available(),
            })),
            "devcraft_sandbox_run" => {
                let repo = required_str(args, "repo")?;
                let command = required_str(args, "command")?;
                let confirm = match args.get("confirm") {
                    Some(Value::Bool(flag)) => *flag,
                    Some(Value::String(text)) => text == "true",
                    _ => false,
                };
                Self::sandbox_run_at(&self.root, repo, command, confirm)
            }
            "devcraft_checkpoint_repo" => {
                let repo = required_str(args, "repo")?;
                self.checkpoint_repo(repo)
            }
            "devcraft_craft_smoke" => {
                let repo = required_str(args, "repo")?;
                Self::craft_smoke_at(&self.root, repo)
            }
            "devcraft_tasks" | "devcraft_logs" | "devcraft_telemetry" => {
                self.ensure_ops();
                match tool {
                    "devcraft_tasks" => {
                        serde_json::to_value(&self.ops.tasks).map_err(|e| e.to_string())
                    }
                    "devcraft_logs" => {
                        serde_json::to_value(&self.ops.logs).map_err(|e| e.to_string())
                    }
                    _ => serde_json::to_value(&self.ops.telemetry).map_err(|e| e.to_string()),
                }
            }
            "devcraft_analyze" => self.analyze_id(required_str(args, "id")?),
            "devcraft_lock" => serde_json::to_value(&self.lock).map_err(|e| e.to_string()),
            "devcraft_skill_read" => {
                let name = args.get("name").and_then(Value::as_str).unwrap_or("");
                if name.is_empty() {
                    Ok(json!({
                        "skills": skills::skills().iter().map(|skill| json!({
                            "name": skill.name,
                            "description": skill.description,
                            "uri": format!("skill://{}/SKILL.md", skill.name),
                        })).collect::<Vec<_>>()
                    }))
                } else {
                    skills::skill(name)
                        .map(|skill| json!({"name": skill.name, "body": skill.body}))
                        .ok_or_else(|| format!("unknown skill: {name}"))
                }
            }
            "devcraft_graphs" => {
                serde_json::to_value(crate::graphs::measure(&self.root)).map_err(|e| e.to_string())
            }
            "devcraft_freshness" => {
                let rows: Vec<_> = self
                    .crafts
                    .iter()
                    .map(|craft| freshness::inspect(Path::new(&craft.path), &craft.name))
                    .collect();
                serde_json::to_value(rows).map_err(|e| e.to_string())
            }
            "devcraft_workflow" => {
                let kind = required_str(args, "kind")?;
                let repo = required_str(args, "repo")?;
                let confirm = match args.get("confirm") {
                    Some(Value::Bool(flag)) => *flag,
                    Some(Value::String(text)) => text == "true",
                    _ => false,
                };
                workflows::queue(&self.root, kind, repo, confirm)
            }
            "devcraft_craft_build" => {
                let repo = required_str(args, "repo")?;
                let profile = args
                    .get("profile")
                    .and_then(Value::as_str)
                    .unwrap_or("debug");
                let confirm = match args.get("confirm") {
                    Some(Value::Bool(flag)) => *flag,
                    Some(Value::String(text)) => text == "true",
                    _ => false,
                };
                Self::craft_build_at(&self.root, repo, profile, confirm)
            }
            "devcraft_service" => {
                let name = required_str(args, "name")?;
                serde_json::to_value(ServiceSentinel::query_service(name))
                    .map_err(|e| e.to_string())
            }
            "devcraft_restart" => {
                let name = required_str(args, "name")?;
                if name
                    .chars()
                    .any(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                {
                    return Err("service name has unexpected characters".into());
                }
                serde_json::to_value(ServiceSentinel::restart_service_safely(name, None))
                    .map_err(|e| e.to_string())
            }
            "devcraft_pin" => {
                let repo = required_str(args, "repo")?;
                self.pin(repo)
            }
            "devcraft_move" => {
                let card = required_str(args, "card")?;
                let to = required_str(args, "to")?;
                let holder = args
                    .get("holder")
                    .and_then(Value::as_str)
                    .unwrap_or("cursor")
                    .to_string();
                self.move_card(card, to, &holder)
            }
            "devcraft_docker" => {
                let report = probe_docker();
                let value = serde_json::to_value(&report).map_err(|e| e.to_string())?;
                self.docker = Some(report);
                Ok(value)
            }
            "devcraft_ports" => {
                let report = PortSentinel::audit_ports(&self.root);
                let value = serde_json::to_value(&report).map_err(|e| e.to_string())?;
                self.ports = Some(report);
                Ok(value)
            }
            "devcraft_open" => {
                let repo = required_str(args, "repo")?;
                let ide = args.get("ide").and_then(Value::as_str).unwrap_or("cursor");
                self.open_ide(repo, ide)
            }
            "devcraft_reveal" => {
                let path = required_str(args, "path")?;
                self.reveal(path)
            }
            "devcraft_focus" => {
                let repo = required_str(args, "repo")?;
                let holder = args
                    .get("holder")
                    .and_then(Value::as_str)
                    .unwrap_or("cursor");
                let ide = args.get("ide").and_then(Value::as_str).unwrap_or(holder);
                self.focus(repo, holder, ide)
            }
            "devcraft_find" => {
                let query = args.get("query").and_then(Value::as_str).unwrap_or("");
                Ok(self.find(query))
            }
            "devcraft_recent" => {
                let hours = args.get("hours").and_then(Value::as_u64).unwrap_or(24);
                Ok(self.recent(hours))
            }
            "devcraft_reveal_repo" => {
                let repo = required_str(args, "repo")?;
                self.reveal_repo(repo)
            }
            "devcraft_doctor" => Ok(self.doctor()),
            "devcraft_adn_status" => Ok(adn::status(&self.root)),
            "devcraft_adn_search" => {
                let query = args.get("query").and_then(Value::as_str).unwrap_or("");
                let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(8) as usize;
                Ok(adn::search(query, limit))
            }
            other => Err(format!("unknown tool: {other}")),
        }
    }

    pub fn digest(&self) -> &Digest {
        &self.digest
    }
    pub fn todos(&self) -> &[Todo] {
        &self.todos
    }
    pub fn reports(&self) -> &[ReportRow] {
        &self.reports
    }
    pub fn crafts(&self) -> &[CraftRow] {
        &self.crafts
    }
    pub fn upstream(&self) -> &[UpstreamRow] {
        &self.upstream
    }

    /// Read `_upstream` craft clones. Does not fetch, merge, or write.
    pub fn load_upstream(&mut self) {
        if self.upstream_loaded {
            return;
        }
        self.upstream = upstream_rows(&self.root);
        self.upstream_loaded = true;
    }
    pub fn repos(&self) -> &[Facts] {
        &self.repos
    }
    pub fn docker(&self) -> Option<&DockerReport> {
        self.docker.as_ref()
    }
    pub fn ports(&self) -> Option<&PortAuditReport> {
        self.ports.as_ref()
    }
    pub fn lock_findings(&self) -> &[LockFinding] {
        &self.lock
    }
    pub fn ops(&self) -> &OpsSnap {
        &self.ops
    }
    pub fn store_ops(&mut self, snap: OpsSnap) {
        self.ops = snap;
    }
    pub fn scanned(&self) -> bool {
        self.scanned
    }

    pub fn ensure_registry(&mut self) {
        if self.ports.is_none() {
            self.ports = Some(PortSentinel::registry(&self.root));
        }
    }

    pub fn scan_jobs(&self) -> Vec<ScanJob> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut jobs = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
                continue;
            };
            if name.starts_with('.') || name.starts_with('_') || name.contains(' ') {
                continue;
            }
            if crate::mirrored_upstream(&self.root, &name) {
                continue;
            }
            jobs.push(ScanJob {
                name: name.clone(),
                path,
                holder: self.claims.get(&name).map(|c| c.holder.clone()),
                pinned: self.pins.contains(&name),
            });
        }
        jobs.sort_by_key(|job| job.name.to_lowercase());
        jobs
    }

    pub fn inspect(job: &ScanJob, now: SystemTime) -> Facts {
        let path = &job.path;
        let has_git = path.join(".git").exists();
        let mut branch = "unknown".to_string();
        let mut dirty = 0;
        let mut untracked = 0;
        let mut ahead = 0;
        let mut unmerged = false;
        let mut last_commit_age_hours = None;
        if has_git {
            if let Some(text) = git(path, &["status", "-sb"]) {
                let parsed = board::parse_status_sb(&text);
                branch = parsed.0;
                ahead = parsed.1;
                dirty = parsed.2;
                untracked = parsed.3;
                unmerged = parsed.4;
            }
            if let Some(text) = git(path, &["log", "-1", "--format=%ct"]) {
                if let Ok(secs) = text.trim().parse::<u64>() {
                    let now_s = unix_of(now);
                    last_commit_age_hours = Some(now_s.saturating_sub(secs) / 3600);
                }
            }
        }
        let mut commits_14 = [0u16; 14];
        let mut worktrees = Vec::new();
        if has_git {
            if let Some(text) = git(path, &["log", "--since=14 days ago", "--pretty=%cs"]) {
                commits_14 = board::tally_iso_days(&text, &iso_day(now));
            }
            if let Some(text) = git(path, &["worktree", "list", "--porcelain"]) {
                worktrees = board::child_worktrees(&text, path);
            }
        }
        let handover_path = path.join("HANDOVER.md");
        let handover_next = fs::read_to_string(&handover_path)
            .map(|t| handover::next_step(&t))
            .unwrap_or_default();
        let handover_age_hours = if handover_path.is_file() {
            handover::age_hours(&handover_path, now)
        } else {
            None
        };
        Facts {
            id: job.name.clone(),
            name: job.name.clone(),
            path: path.display().to_string(),
            has_git,
            branch,
            dirty,
            untracked,
            ahead,
            unmerged,
            index_lock: path.join(".git").join("index.lock").is_file(),
            has_agents: path.join("AGENTS.md").is_file(),
            has_claude: path.join("CLAUDE.md").is_file(),
            handover_next,
            handover_age_hours,
            last_commit_age_hours,
            commits_14,
            worktrees,
            task_key: None,
            claimed_by: job.holder.clone(),
            pinned: job.pinned,
            craft: is_craft(&job.name),
        }
    }

    pub fn clear_live(&mut self) {
        self.repos.clear();
        self.scanned = false;
    }

    pub fn push_repo(&mut self, mut facts: Facts) {
        facts.task_key = self.tasks.get(&facts.id).cloned();
        if let Some(slot) = self.repos.iter_mut().find(|row| row.id == facts.id) {
            *slot = facts;
        } else {
            self.repos.push(facts);
        }
    }

    pub fn finish_scan(&mut self) {
        self.repos.sort_by_key(|row| row.name.to_lowercase());
        self.scanned = true;
    }

    pub fn load_docker() -> DockerReport {
        probe_docker()
    }

    pub fn store_docker(&mut self, report: DockerReport) {
        self.docker = Some(report);
    }

    fn move_card(&mut self, card: &str, to: &str, holder: &str) -> Result<Value, String> {
        if card.contains('#') {
            return Err("A worktree follows its checkout.".into());
        }
        let github = card.starts_with("gh:");
        let effect = board::allow(to, github)?;
        match effect {
            Move::Snooze => self.snooze_card(card),
            Move::Pin => self.pin(card),
            Move::Claim => self.claim(card, holder),
            Move::Release => self.release(card),
        }
    }

    fn claim(&mut self, repo: &str, holder: &str) -> Result<Value, String> {
        if !board::holder_ok(holder) {
            return Err("holder is cursor, opencode, agy, or claude".into());
        }
        if repo.starts_with("gh:") || !valid_repo_name(repo) {
            return Err("claim a repository name".into());
        }
        if !self.root.join(repo).is_dir() {
            return Err(format!(
                "{repo} is not a directory under {}",
                self.root.display()
            ));
        }
        self.claims.insert(
            repo.to_string(),
            Claim {
                holder: holder.to_string(),
                since: chrono_now(),
            },
        );
        self.write_state()?;
        self.apply_claims_to_repos();
        Ok(json!({ "claimed": repo, "holder": holder }))
    }

    fn release(&mut self, repo: &str) -> Result<Value, String> {
        let removed = self.claims.remove(repo).is_some();
        self.write_state()?;
        self.apply_claims_to_repos();
        Ok(json!({ "released": removed, "repo": repo }))
    }

    fn release_all(&mut self) -> Result<Value, String> {
        let released = self.claims.len();
        self.claims.clear();
        self.write_state()?;
        self.apply_claims_to_repos();
        Ok(json!({ "released": released }))
    }

    fn release_stale_claims(&mut self, max_age_hours: u64) -> Result<Value, String> {
        let stale_repos: Vec<String> = self
            .claims
            .iter()
            .filter(|(_, c)| claim_age_hours(&c.since).is_some_and(|h| h >= max_age_hours))
            .map(|(repo, _)| repo.clone())
            .collect();
        for repo in &stale_repos {
            self.claims.remove(repo);
        }
        if !stale_repos.is_empty() {
            self.write_state()?;
            self.apply_claims_to_repos();
        }
        Ok(json!({
            "released": stale_repos.len(),
            "repos": stale_repos,
            "max_age_hours": max_age_hours,
        }))
    }

    fn checkpoint_state(&self) -> Result<Value, String> {
        let dir = self.state_dir();
        let files = [
            "claims.json",
            "pins.json",
            "snooze.json",
            "tasks.json",
            "todos.json",
        ]
        .into_iter()
        .map(|name| dir.join(name))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
        if files.is_empty() {
            return Err("nothing in .devcraft to snapshot yet".into());
        }
        let record = BackupEngine::create_snapshot(&self.root, &files, "cockpit", "cockpit state")?;
        Ok(json!({ "id": record.id, "files": record.backed_up_files.len() }))
    }

    fn checkpoint_repo(&self, repo: &str) -> Result<Value, String> {
        if !valid_repo_name(repo) || !self.root.join(repo).is_dir() {
            return Err("snapshot a repository that exists".into());
        }
        let dir = self.root.join(repo);
        let files = ["HANDOVER.md", "CHANGELOG.md", "README.md"]
            .into_iter()
            .map(|name| dir.join(name))
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        if files.is_empty() {
            return Err("no HANDOVER.md, CHANGELOG.md, or README.md".into());
        }
        let record = BackupEngine::create_snapshot(&self.root, &files, repo, "repo checkpoint")?;
        Ok(json!({ "id": record.id, "files": record.backed_up_files.len() }))
    }

    pub fn sandbox_run_at(
        root: &Path,
        repo: &str,
        command: &str,
        confirm: bool,
    ) -> Result<Value, String> {
        if !confirm {
            return Err("confirm the sandbox run".into());
        }
        if !valid_repo_name(repo) || !root.join(repo).is_dir() {
            return Err("run the sandbox in a repository that exists".into());
        }
        let command = command.trim();
        if command.is_empty()
            || command.len() > 180
            || command
                .chars()
                .any(|c| matches!(c, '\n' | '\r' | '&' | '|'))
        {
            return Err("command must be one short line".into());
        }
        let result = ContainerSandbox::execute(&root.join(repo), None, command)?;
        serde_json::to_value(result).map_err(|e| e.to_string())
    }

    pub fn craft_smoke_at(root: &Path, repo: &str) -> Result<Value, String> {
        if !valid_repo_name(repo) {
            return Err("bad repository name".into());
        }
        let script = root.join(repo).join("start.ps1");
        if !script.is_file() {
            return Err(format!("{repo} has no start.ps1"));
        }
        let output = Command::new("powershell.exe")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&script)
            .arg("-Smoke")
            .output()
            .map_err(|e| e.to_string())?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let tail = stdout
            .lines()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        Ok(json!({
            "repo": repo,
            "exit": output.status.code().unwrap_or(-1),
            "tail": tail,
        }))
    }

    pub fn craft_build_at(
        root: &Path,
        repo: &str,
        profile: &str,
        confirm: bool,
    ) -> Result<Value, String> {
        if !confirm {
            return Err("confirm the build".into());
        }
        if !valid_repo_name(repo) || !root.join(repo).is_dir() {
            return Err("build a checkout that exists".into());
        }
        if profile != "debug" && profile != "release" {
            return Err("profile is debug or release".into());
        }
        let manifest = root.join(repo).join("Cargo.toml");
        if !manifest.is_file() {
            return Err(format!("{repo} has no Cargo.toml"));
        }
        let mut cmd = Command::new("cargo");
        cmd.arg("build").arg("--manifest-path").arg(&manifest);
        if profile == "release" {
            cmd.arg("--release");
        }
        let output = cmd.output().map_err(|e| e.to_string())?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail = stderr
            .lines()
            .rev()
            .take(12)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        Ok(json!({
            "repo": repo,
            "profile": profile,
            "exit": output.status.code().unwrap_or(-1),
            "tail": tail,
        }))
    }

    fn ensure_ops(&mut self) {
        if !self.ops.ready {
            self.ops = ops::load(&self.root);
        }
    }

    fn analyze_id(&self, id: &str) -> Result<Value, String> {
        let path = if let Some(name) = id.strip_prefix("task:") {
            self.ops
                .tasks
                .iter()
                .find(|row| row.name == name)
                .map(|row| row.report_path.clone())
                .filter(|path| !path.is_empty())
        } else if let Some(index) = id
            .strip_prefix("log:")
            .and_then(|text| text.parse::<usize>().ok())
        {
            self.ops.logs.get(index).map(|file| file.path.clone())
        } else {
            None
        };
        let Some(path) = path else {
            return Err("that id has no file to read".into());
        };
        let shown = ops::show_file(Path::new(&path));
        Ok(json!({
            "path": path,
            "findings": shown.findings,
            "excerpt": shown.excerpt,
        }))
    }

    fn todo_add(
        &mut self,
        title: &str,
        due: &str,
        detail: &str,
        source: &str,
        repo: &str,
    ) -> Result<Value, String> {
        let todo = todos::add(&mut self.todos, title, due, detail, source, repo)?;
        self.write_state()?;
        serde_json::to_value(&todo).map_err(|e| e.to_string())
    }

    fn todo_list(&self, show_done: bool) -> Value {
        let today = todos::today_local();
        let rows: Vec<Value> = todos::sorted(&self.todos)
            .iter()
            .filter(|t| show_done || !t.done)
            .map(|t| {
                json!({
                    "id": t.id,
                    "title": t.title,
                    "due": todos::due_label(t, &today),
                    "done": t.done,
                    "repo": t.repo,
                })
            })
            .collect();
        json!({ "todos": rows, "open": self.todos.iter().filter(|t| !t.done).count() })
    }

    fn todo_done(&mut self, id: &str) -> Result<Value, String> {
        let todo = todos::complete(&mut self.todos, id)?;
        self.write_state()?;
        serde_json::to_value(&todo).map_err(|e| e.to_string())
    }

    fn todo_clear(&mut self) -> Result<Value, String> {
        let cleared = todos::clear_done(&mut self.todos);
        self.write_state()?;
        Ok(json!({ "cleared": cleared }))
    }

    fn set_task(&mut self, repo: &str, key: &str) -> Result<Value, String> {
        if !valid_repo_name(repo) || !self.root.join(repo).is_dir() {
            return Err("set a key on a repository that exists".into());
        }
        let key = key.trim();
        if key.is_empty() {
            self.tasks.remove(repo);
        } else if !board::task_key_ok(key) {
            return Err("a TaskCraft key looks like BC-12".into());
        } else {
            self.tasks.insert(repo.to_string(), key.to_string());
        }
        self.write_state()?;
        self.apply_claims_to_repos();
        Ok(json!({ "repo": repo, "key": self.tasks.get(repo) }))
    }

    fn pin(&mut self, repo: &str) -> Result<Value, String> {
        if !valid_repo_name(repo) || !self.root.join(repo).is_dir() {
            return Err("pin a repository that exists".into());
        }
        let pinned = if self.pins.contains(repo) {
            self.pins.remove(repo);
            false
        } else {
            self.pins.insert(repo.to_string());
            true
        };
        self.write_state()?;
        self.apply_claims_to_repos();
        Ok(json!({ "repo": repo, "pinned": pinned }))
    }

    fn snooze_card(&mut self, card: &str) -> Result<Value, String> {
        let until = unix_now().saturating_add(SNOOZE_SECS);
        self.snooze.insert(card.to_string(), until);
        self.write_state()?;
        Ok(json!({ "snoozed": card, "until": until }))
    }

    fn reveal(&self, path: &str) -> Result<Value, String> {
        if path.starts_with("https://") || path.starts_with("http://") {
            if path.chars().any(|c| c.is_control() || c == '"') {
                return Err("refused link".into());
            }
            return match Command::new("explorer").arg(path).spawn() {
                Ok(_) => Ok(json!({ "opened": path })),
                Err(e) => Err(format!("explorer did not start: {e}")),
            };
        }
        let target = PathBuf::from(path);
        let root = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        let canon = target.canonicalize().map_err(|e| e.to_string())?;
        if !canon.starts_with(&root) {
            return Err("that path is outside the workspace".into());
        }
        match Command::new("explorer").arg(&canon).spawn() {
            Ok(_) => Ok(json!({ "opened": canon.display().to_string() })),
            Err(e) => Err(format!("explorer did not start: {e}")),
        }
    }

    fn open_ide(&self, repo: &str, ide: &str) -> Result<Value, String> {
        if !board::holder_ok(ide) {
            return Err("ide is cursor, opencode, agy, or claude".into());
        }
        let path = self.root.join(repo);
        if !path.is_dir() {
            return Err(format!("{repo} is not a directory"));
        }
        let program = match ide {
            "cursor" => "cursor",
            "opencode" => "opencode",
            "agy" => "agy",
            "claude" => "claude",
            _ => return Err("unknown ide".into()),
        };
        let mut cmd = Command::new(program);
        match ide {
            "cursor" => {
                cmd.arg(path.as_os_str());
            }
            _ => {
                cmd.arg(path.as_os_str());
            }
        }
        match cmd.spawn() {
            Ok(_) => Ok(json!({ "opened": repo, "ide": ide })),
            Err(e) => Err(format!("{program} did not start: {e}")),
        }
    }

    fn focus(&mut self, repo: &str, holder: &str, ide: &str) -> Result<Value, String> {
        if !board::holder_ok(holder) {
            return Err("holder is cursor, opencode, agy, or claude".into());
        }
        if !board::holder_ok(ide) {
            return Err("ide is cursor, opencode, agy, or claude".into());
        }
        if !valid_repo_name(repo) || !self.root.join(repo).is_dir() {
            return Err(format!(
                "{repo} is not a directory under {}",
                self.root.display()
            ));
        }
        self.claims.insert(
            repo.to_string(),
            Claim {
                holder: holder.to_string(),
                since: chrono_now(),
            },
        );
        self.pins.insert(repo.to_string());
        self.write_state()?;
        self.apply_claims_to_repos();
        let opened = self.open_ide(repo, ide).unwrap_or(json!({"opened": false}));
        Ok(json!({ "focused": repo, "holder": holder, "ide": ide, "opened": opened }))
    }

    fn find(&self, query: &str) -> Value {
        let q = query.trim().to_lowercase();
        let mut rows: Vec<Value> = self
            .repos
            .iter()
            .filter(|f| q.is_empty() || f.name.to_lowercase().contains(&q))
            .map(|f| {
                json!({
                    "name": f.name,
                    "branch": f.branch,
                    "column": board::column(f),
                    "holder": f.claimed_by,
                    "path": clean_display_path(Path::new(&f.path)),
                })
            })
            .collect();
        rows.sort_by_key(|v| {
            let name = v
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase();
            (if name.starts_with(&q) { 0 } else { 1 }, name)
        });
        rows.truncate(30);
        json!({ "query": query, "matches": rows })
    }

    fn recent(&self, hours: u64) -> Value {
        let mut rows: Vec<Value> = self
            .repos
            .iter()
            .filter(|f| f.last_commit_age_hours.is_some_and(|h| h <= hours))
            .map(|f| {
                json!({
                    "name": f.name,
                    "branch": f.branch,
                    "last_commit_age_hours": f.last_commit_age_hours,
                    "dirty": f.dirty,
                    "column": board::column(f),
                    "holder": f.claimed_by,
                })
            })
            .collect();
        rows.sort_by_key(|v| {
            v.get("last_commit_age_hours")
                .and_then(Value::as_u64)
                .unwrap_or(u64::MAX)
        });
        json!({ "hours": hours, "repos": rows })
    }

    fn reveal_repo(&self, repo: &str) -> Result<Value, String> {
        if !valid_repo_name(repo) {
            return Err("bad repository name".into());
        }
        let path = self.root.join(repo);
        if !path.is_dir() {
            return Err(format!("{repo} is not a directory"));
        }
        match Command::new("explorer").arg(&path).spawn() {
            Ok(_) => Ok(json!({ "opened": clean_display_path(&path) })),
            Err(e) => Err(format!("explorer did not start: {e}")),
        }
    }

    fn doctor(&self) -> Value {
        let bins = ["git", "cargo", "bun", "uv", "node", "powershell"];
        let rows: Vec<Value> = bins
            .iter()
            .map(|bin| {
                let found = Command::new("where")
                    .arg(bin)
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                json!({ "bin": bin, "found": found })
            })
            .collect();
        json!({
            "root": clean_display_path(&self.root),
            "root_present": self.root.join("mcp-central-docs").is_dir(),
            "scanned": self.scanned,
            "repos": self.repos.len(),
            "digest_present": self.digest.present,
            "digest_age_hours": self.digest.age_hours,
            "bins": rows,
        })
    }

    fn facts_for(&self, name: &str, path: &Path, now: SystemTime) -> Facts {
        let mut facts = Cockpit::inspect(
            &ScanJob {
                name: name.to_string(),
                path: path.to_path_buf(),
                holder: self.claims.get(name).map(|c| c.holder.clone()),
                pinned: self.pins.contains(name),
            },
            now,
        );
        facts.task_key = self.tasks.get(name).cloned();
        facts
    }

    fn apply_claims_to_repos(&mut self) {
        for facts in &mut self.repos {
            facts.claimed_by = self.claims.get(&facts.id).map(|c| c.holder.clone());
            facts.pinned = self.pins.contains(&facts.id);
            facts.task_key = self.tasks.get(&facts.id).cloned();
        }
    }

    fn repo(&self, name: &str) -> Result<&Facts, String> {
        self.repos
            .iter()
            .find(|f| f.id == name)
            .ok_or_else(|| format!("{name} is not in the last scan. Refresh the fleet."))
    }

    fn state_dir(&self) -> PathBuf {
        self.root.join(".devcraft")
    }

    fn write_state(&self) -> Result<(), String> {
        let dir = self.state_dir();
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        write_json(&dir.join("claims.json"), &self.claims)?;
        write_json(&dir.join("tasks.json"), &self.tasks)?;
        todos::save(&dir, &self.todos)?;
        write_json(&dir.join("pins.json"), &self.pins)?;
        write_json(&dir.join("snooze.json"), &self.snooze)?;
        Ok(())
    }

    fn drop_expired_snooze(&mut self) {
        let now = unix_now();
        self.snooze.retain(|_, until| *until > now);
    }
}

fn facts_json(f: &Facts) -> Value {
    json!({
        "name": f.name,
        "branch": f.branch,
        "dirty": f.dirty,
        "untracked": f.untracked,
        "ahead": f.ahead,
        "column": board::column(f),
        "holder": f.claimed_by,
        "task": f.task_key,
        "worktrees": f.worktrees.iter().map(|tree| json!({
            "name": tree.name,
            "branch": tree.branch,
        })).collect::<Vec<_>>(),
        "next": f.handover_next,
        "craft": f.craft,
        "agents": f.has_agents,
        "claude": f.has_claude,
    })
}

pub fn tool_read_only(name: &str) -> bool {
    matches!(
        name,
        "devcraft_status"
            | "devcraft_board"
            | "devcraft_git"
            | "devcraft_digest"
            | "devcraft_reports"
            | "devcraft_crafts"
            | "devcraft_upstream"
            | "devcraft_brief"
            | "devcraft_snapshots"
            | "devcraft_docker"
            | "devcraft_service"
            | "devcraft_tasks"
            | "devcraft_logs"
            | "devcraft_telemetry"
            | "devcraft_analyze"
            | "devcraft_lock"
            | "devcraft_skill_read"
            | "devcraft_freshness"
            | "devcraft_graphs"
            | "devcraft_sandbox"
            | "devcraft_todo_list"
            | "devcraft_processes"
            | "devcraft_find"
            | "devcraft_recent"
            | "devcraft_doctor"
            | "devcraft_adn_status"
            | "devcraft_adn_search"
    )
}

pub fn tool_specs() -> Vec<Value> {
    vec![
        spec(
            "devcraft_status",
            "Cockpit version, root, and whether a scan is loaded",
            &[],
        ),
        spec(
            "devcraft_refresh",
            "Scan sibling repositories and rebuild the board",
            &[],
        ),
        spec("devcraft_board", "Cards on the fleet board", &[]),
        spec(
            "devcraft_git",
            "Scanned repositories with branch and dirty counts",
            &[],
        ),
        spec(
            "devcraft_digest",
            "Totals from the morning digest file",
            &[],
        ),
        spec(
            "devcraft_reports",
            "Fleet reports with age and a stale mark",
            &[],
        ),
        spec(
            "devcraft_crafts",
            "Craft app checkouts next to this repo",
            &[],
        ),
        spec(
            "devcraft_upstream",
            "Storytold craft clones under _upstream, last 24 hours",
            &[],
        ),
        spec(
            "devcraft_brief",
            "Handover next step for one repository",
            &[("repo", "Repository name")],
        ),
        spec(
            "devcraft_claim",
            "Hold a repository for one IDE",
            &[
                ("repo", "Repository name"),
                ("holder", "cursor, opencode, agy, or claude"),
            ],
        ),
        spec(
            "devcraft_release",
            "Release a claim",
            &[("repo", "Repository name")],
        ),
        spec(
            "devcraft_release_all",
            "Release every claim. Pins stay.",
            &[],
        ),
        spec(
            "devcraft_release_stale_claims",
            "Release claims older than max_age_hours (default 72).",
            &[("max_age_hours", "Age threshold in hours (default 72)")],
        ),
        spec(
            "devcraft_processes",
            "Inspect running DevCraft processes and flag stale or duplicate MCP workers.",
            &[("max_age_hours", "Stale age threshold in hours (default 12)")],
        ),
        spec(
            "devcraft_kill_stale_processes",
            "Terminate stale or duplicate devcraft-mcp processes (protects current PID).",
            &[("max_age_hours", "Stale age threshold in hours (default 12)")],
        ),
        spec(
            "devcraft_todo_add",
            "Add a personal todo. This does not create a TaskCraft issue.",
            &[
                ("title", "Todo title, max 280 chars"),
                ("due", "today, tomorrow, YYYY-MM-DD, or empty"),
                ("detail", "Optional detail"),
                ("source", "desktop, ipad, mcp, or cli"),
                ("repo", "Optional checkout name"),
            ],
        ),
        spec(
            "devcraft_todo_list",
            "Personal todos, open first",
            &[("show_done", "true to include finished todos")],
        ),
        spec(
            "devcraft_todo_done",
            "Mark a personal todo done by full id or unique prefix",
            &[("id", "Todo id")],
        ),
        spec("devcraft_todo_clear", "Drop finished personal todos", &[]),
        spec(
            "devcraft_task",
            "Put a TaskCraft key on a card, or clear it. This does not create an issue.",
            &[
                ("repo", "Repository name"),
                ("key", "Key such as BC-12, or empty to clear"),
            ],
        ),
        spec(
            "devcraft_snapshots",
            "Time machine snapshots under .devcraft",
            &[],
        ),
        spec(
            "devcraft_checkpoint",
            "Snapshot claims, pins, and snoozes",
            &[],
        ),
        spec("devcraft_sandbox", "Whether Docker can run a sandbox", &[]),
        spec(
            "devcraft_sandbox_run",
            "Run one short command for a checkout. Needs confirm.",
            &[
                ("repo", "Repository name"),
                ("command", "One short command line"),
                ("confirm", "true to run"),
            ],
        ),
        spec(
            "devcraft_checkpoint_repo",
            "Snapshot HANDOVER.md and CHANGELOG.md for one checkout",
            &[("repo", "Repository name")],
        ),
        spec(
            "devcraft_craft_smoke",
            "Run start.ps1 -Smoke for one craft checkout",
            &[("repo", "Repository name")],
        ),
        spec(
            "devcraft_tasks",
            "Scheduled fleet tasks and their reports",
            &[],
        ),
        spec("devcraft_logs", "Fleet and IDE MCP logs", &[]),
        spec("devcraft_telemetry", "Unified monitoring stack", &[]),
        spec(
            "devcraft_analyze",
            "Extract failure lines from a task report or a log",
            &[("id", "task: or log: id from the tasks or logs page")],
        ),
        spec(
            "devcraft_lock",
            "Port lock findings from the last fleet lock build",
            &[],
        ),
        spec(
            "devcraft_skill_read",
            "List bundled skills, or return one skill body",
            &[("name", "Skill name. Empty lists them.")],
        ),
        spec(
            "devcraft_freshness",
            "Read-only craft check: release exe, debug exe, changelog, help, menu strings",
            &[],
        ),
        spec(
            "devcraft_graphs",
            "Commit days, source lines, languages, and tags for the fleet",
            &[],
        ),
        spec(
            "devcraft_workflow",
            "Queue an SOP request for one checkout. Does not edit that checkout.",
            &[
                (
                    "kind",
                    "craft-assess, assfix, gapfind, drift-scan, quality-check, or doc-sync",
                ),
                ("repo", "Checkout name"),
                ("confirm", "true to write the request"),
            ],
        ),
        spec(
            "devcraft_craft_build",
            "cargo build for one craft checkout. Needs confirm.",
            &[
                ("repo", "Checkout name"),
                ("profile", "debug or release"),
                ("confirm", "true to build"),
            ],
        ),
        spec(
            "devcraft_service",
            "Query one Windows service",
            &[("name", "Service name")],
        ),
        spec(
            "devcraft_restart",
            "Restart one Windows service with sc.exe",
            &[("name", "Service name")],
        ),
        spec(
            "devcraft_pin",
            "Pin or unpin a repository on the board",
            &[("repo", "Repository name")],
        ),
        spec(
            "devcraft_move",
            "Move a board card. Computed columns are refused.",
            &[
                ("card", "Card id"),
                ("to", "Column id"),
                ("holder", "IDE when moving to Active"),
            ],
        ),
        spec(
            "devcraft_docker",
            "Docker daemon and running containers",
            &[],
        ),
        spec("devcraft_ports", "Registered ports against listeners", &[]),
        spec(
            "devcraft_open",
            "Open a checkout in Cursor, OpenCode, Claude, or Antigravity",
            &[
                ("repo", "Repository name"),
                ("ide", "cursor, opencode, agy, or claude"),
            ],
        ),
        spec(
            "devcraft_reveal",
            "Open a fleet report or an https link",
            &[("path", "Report path or https link")],
        ),
        spec(
            "devcraft_focus",
            "Claim + pin + open a repo in one step for the current holder",
            &[
                ("repo", "Repository name"),
                ("holder", "cursor, opencode, agy, or claude"),
                ("ide", "cursor, opencode, agy, or claude"),
            ],
        ),
        spec(
            "devcraft_find",
            "Fuzzy-find repos by name substring",
            &[("query", "Substring to match, empty lists scanned repos")],
        ),
        spec(
            "devcraft_recent",
            "Repos with commits inside the last N hours",
            &[("hours", "Window in hours (default 24)")],
        ),
        spec(
            "devcraft_reveal_repo",
            "Open a checkout folder in Explorer",
            &[("repo", "Repository name")],
        ),
        spec(
            "devcraft_doctor",
            "Env check: root, scan state, digest, tool bins",
            &[],
        ),
        spec(
            "devcraft_adn_status",
            "advanced-memory-mcp daemon health + workspace file probe (read-only, port 10732)",
            &[],
        ),
        spec(
            "devcraft_adn_search",
            "Search advanced-memory notes via the daemon (read-only, needs daemon up)",
            &[("query", "Search text"), ("limit", "Max hits (default 8)")],
        ),
    ]
}

fn spec(name: &str, description: &str, fields: &[(&str, &str)]) -> Value {
    let mut properties = serde_json::Map::new();
    for (key, desc) in fields {
        properties.insert(
            (*key).to_string(),
            json!({ "type": "string", "description": desc }),
        );
    }
    json!({
        "name": name,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties }
    })
}

fn report_rows(root: &Path) -> Vec<ReportRow> {
    let docs = root.join("mcp-central-docs");
    let specs = [
        ("fleet-daily", "Fleet daily", "operations/daily/index.html"),
        (
            "morning-digest",
            "Morning digest",
            "operations/fleet-status.md",
        ),
        ("session-log", "Session log", "operations/session-log"),
        (
            "aiwatcher",
            "AIWatcher log",
            "operations/session-log/aiwatcher",
        ),
        (
            "port-registry",
            "Port registry",
            "operations/WEBAPP_PORTS.md",
        ),
        ("status", "Fleet status", "STATUS.md"),
    ];
    specs
        .into_iter()
        .map(|(id, title, rel)| {
            let path = docs.join(rel);
            let age = mtime_hours(&path);
            ReportRow {
                id: id.into(),
                title: title.into(),
                path: path.display().to_string(),
                present: path.exists(),
                age_hours: age,
                stale: age.is_some_and(|h| h > 26) || !path.exists(),
            }
        })
        .collect()
}

fn craft_rows(root: &Path) -> Vec<CraftRow> {
    let mut rows = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return rows;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
            continue;
        };
        if !is_craft(&name) || crate::mirrored_upstream(root, &name) {
            continue;
        }
        let readme = fs::read_to_string(path.join("README.md")).unwrap_or_default();
        let cargo = fs::read_to_string(path.join("Cargo.toml")).unwrap_or_default();
        let release = path.join("target").join("release");
        let has_binary = release.join(format!("{name}.exe")).is_file()
            || release.join(format!("{name}-app.exe")).is_file();
        rows.push(CraftRow {
            name,
            path: path.display().to_string(),
            tagline: tagline(&readme),
            version: cargo_version(&cargo),
            has_binary,
            has_handover: path.join("HANDOVER.md").is_file(),
        });
    }
    rows.sort_by_key(|a| a.name.to_lowercase());
    rows
}

pub fn parse_docker_ps(text: &str) -> Vec<ContainerRow> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let name = parts.next()?.trim();
            if name.is_empty() {
                return None;
            }
            Some(ContainerRow {
                name: name.to_string(),
                image: parts.next().unwrap_or("").trim().to_string(),
                status: parts.next().unwrap_or("").trim().to_string(),
            })
        })
        .collect()
}

fn probe_docker() -> DockerReport {
    let version = Command::new("docker")
        .args(["info", "--format", "{{.ServerVersion}}"])
        .output();
    let Ok(version_out) = version else {
        return DockerReport {
            up: false,
            version: String::new(),
            containers: Vec::new(),
            note: "docker is not on PATH".into(),
        };
    };
    if !version_out.status.success() {
        let err = String::from_utf8_lossy(&version_out.stderr);
        return DockerReport {
            up: false,
            version: String::new(),
            containers: Vec::new(),
            note: truncate(err.trim(), 180),
        };
    }
    let version = String::from_utf8_lossy(&version_out.stdout)
        .trim()
        .to_string();
    let ps = Command::new("docker")
        .args(["ps", "--format", "{{.Names}}\t{{.Image}}\t{{.Status}}"])
        .output();
    let containers = match ps {
        Ok(out) if out.status.success() => parse_docker_ps(&String::from_utf8_lossy(&out.stdout)),
        Ok(out) => {
            return DockerReport {
                up: true,
                version,
                containers: Vec::new(),
                note: truncate(String::from_utf8_lossy(&out.stderr).trim(), 180),
            };
        }
        Err(e) => {
            return DockerReport {
                up: true,
                version,
                containers: Vec::new(),
                note: e.to_string(),
            };
        }
    };
    DockerReport {
        up: true,
        version,
        containers,
        note: String::new(),
    }
}

fn git(path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

const UPSTREAM_EXTRA: &[&str] = &["craft-anything", "craft-launcher"];

fn is_upstream_craft(name: &str) -> bool {
    if name.contains(' ') || name.starts_with('.') {
        return false;
    }
    name.ends_with("craft") || UPSTREAM_EXTRA.contains(&name)
}

/// Local history only. No fetch.
pub fn upstream_rows(root: &Path) -> Vec<UpstreamRow> {
    let Ok(entries) = fs::read_dir(root.join("_upstream")) else {
        return Vec::new();
    };
    let mut jobs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || !path.join(".git").exists() {
            continue;
        }
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
            continue;
        };
        if is_upstream_craft(&name) {
            jobs.push((name, path));
        }
    }
    let mut rows = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for (name, path) in jobs {
            handles.push(scope.spawn(move || read_upstream(name, path)));
        }
        handles
            .into_iter()
            .filter_map(|handle| handle.join().ok())
            .collect::<Vec<_>>()
    });
    rows.sort_by(|a, b| {
        b.commits
            .cmp(&a.commits)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    rows
}

fn read_upstream(name: String, path: PathBuf) -> UpstreamRow {
    let head_line = git(&path, &["log", "-1", "--format=%h%x09%as%x09%s"]).unwrap_or_default();
    let mut parts = head_line.trim().splitn(3, '\t');
    let head = parts.next().unwrap_or("").trim().to_string();
    let date = parts.next().unwrap_or("").trim().to_string();
    let subject = parts.next().unwrap_or("").trim().to_string();
    let tag = git(
        &path,
        &[
            "for-each-ref",
            "--count=1",
            "--sort=-creatordate",
            "--format=%(refname:short)",
            "refs/tags",
        ],
    )
    .unwrap_or_default()
    .trim()
    .to_string();
    let commits = git(&path, &["rev-list", "--count", "--since=24.hours", "HEAD"])
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0);
    let subjects = git(
        &path,
        &["log", "-n", "8", "--since=24.hours", "--format=%s"],
    )
    .unwrap_or_default()
    .lines()
    .map(|line| line.trim().to_string())
    .filter(|line| !line.is_empty())
    .collect();
    let delta = if commits == 0 {
        String::new()
    } else {
        upstream_delta(&path)
    };
    let changelog_text = fs::read_to_string(path.join("CHANGELOG.md")).unwrap_or_default();
    UpstreamRow {
        name,
        head,
        subject,
        date,
        tag,
        commits,
        delta,
        subjects,
        changelog: changelog_tail(&changelog_text),
    }
}

fn upstream_delta(path: &Path) -> String {
    let Some(anchor) = git(
        path,
        &["rev-parse", "--verify", "--quiet", "HEAD@{24.hours.ago}"],
    ) else {
        return String::new();
    };
    let anchor = anchor.trim();
    if anchor.is_empty() {
        return String::new();
    }
    let spec = format!("{anchor}..HEAD");
    shortstat(&git(path, &["diff", "--shortstat", &spec]).unwrap_or_default())
}

pub fn changelog_tail(text: &str) -> String {
    let mut lines = Vec::new();
    let mut seen_heading = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("<!--") {
            continue;
        }
        if trimmed.starts_with("# ") {
            continue;
        }
        if trimmed.starts_with("## ") {
            if seen_heading {
                break;
            }
            seen_heading = true;
        }
        lines.push(trimmed.to_string());
        if lines.len() >= 8 {
            break;
        }
    }
    lines.join("\n")
}

pub fn shortstat(text: &str) -> String {
    let mut files = String::new();
    let mut ins = String::from("0");
    let mut del = String::from("0");
    let mut any = false;
    for part in text.split(',') {
        let trimmed = part.trim();
        if let Some(n) = trimmed
            .strip_suffix(" files changed")
            .or_else(|| trimmed.strip_suffix(" file changed"))
        {
            files = n.trim().to_string();
            any = true;
        } else if let Some(n) = trimmed
            .strip_suffix(" insertions(+)")
            .or_else(|| trimmed.strip_suffix(" insertion(+)"))
        {
            ins = n.trim().to_string();
            any = true;
        } else if let Some(n) = trimmed
            .strip_suffix(" deletions(-)")
            .or_else(|| trimmed.strip_suffix(" deletion(-)"))
        {
            del = n.trim().to_string();
            any = true;
        }
    }
    if !any {
        return String::new();
    }
    format!("{files} files, +{ins} / -{del}")
}

fn is_craft(name: &str) -> bool {
    if name.contains(' ') || name.starts_with('.') || name.starts_with('_') {
        return false;
    }
    name.ends_with("craft") || EXTRA_CRAFTS.contains(&name)
}

fn tagline(readme: &str) -> String {
    for line in readme.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("![") || t.starts_with("<!--") {
            continue;
        }
        if let Some(rest) = t.strip_prefix('#') {
            let title = rest.trim_start_matches('#').trim();
            if !title.is_empty() {
                return truncate(title, 80);
            }
            continue;
        }
        return truncate(t, 110);
    }
    String::new()
}

fn cargo_version(toml: &str) -> String {
    for line in toml.lines() {
        let t = line.trim();
        if t.starts_with("version.workspace") || t.starts_with('#') {
            continue;
        }
        if let Some(rest) = t.strip_prefix("version") {
            if let Some(q) = rest.split('"').nth(1) {
                if !q.is_empty() {
                    return q.to_string();
                }
            }
        }
    }
    String::new()
}

fn valid_repo_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['/', '\\', ':', '.'])
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn mtime_hours(path: &Path) -> Option<u64> {
    let meta = if path.is_dir() {
        newest_child(path)?
    } else {
        path.metadata().ok()?
    };
    let modified = meta.modified().ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let then = modified.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(now.saturating_sub(then) / 3600)
}

fn newest_child(dir: &Path) -> Option<fs::Metadata> {
    let mut best: Option<(u64, fs::Metadata)> = None;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let meta = entry.metadata().ok()?;
        let then = meta
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs();
        if best.as_ref().is_none_or(|b| then > b.0) {
            best = Some((then, meta));
        }
    }
    best.map(|(_, meta)| meta)
}

fn truncate(text: &str, max: usize) -> String {
    let mut out = String::new();
    for (i, ch) in text.chars().enumerate() {
        if i >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn unix_of(now: SystemTime) -> u64 {
    now.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn chrono_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Age of an RFC-3339 claim timestamp in whole hours.
pub fn claim_age_hours(since: &str) -> Option<u64> {
    let parsed = chrono::DateTime::parse_from_rfc3339(since).ok()?;
    let delta = chrono::Utc::now().signed_duration_since(parsed.with_timezone(&chrono::Utc));
    Some(delta.num_hours().max(0) as u64)
}

fn iso_day(now: SystemTime) -> String {
    chrono::DateTime::<chrono::Local>::from(now)
        .format("%Y-%m-%d")
        .to_string()
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{key} is required"))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "devcraft-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn claim_roundtrip_and_board_move() {
        let root = scratch("claim");
        fs::create_dir_all(root.join("celcraft")).unwrap();
        fs::write(
            root.join("celcraft").join("HANDOVER.md"),
            "## Next step\n- paint the board\n",
        )
        .unwrap();
        let mut pit = Cockpit::open(&root).unwrap();
        pit.refresh().unwrap();
        let claimed = pit
            .call(
                "devcraft_claim",
                &json!({"repo": "celcraft", "holder": "claude"}),
            )
            .unwrap();
        assert_eq!(claimed["holder"], "claude");
        let brief = pit
            .call("devcraft_brief", &json!({"repo": "celcraft"}))
            .unwrap();
        assert_eq!(brief["next"], "paint the board");
        assert_eq!(brief["column"], "active");
        pit.call("devcraft_move", &json!({"card": "celcraft", "to": "done"}))
            .unwrap();
        let mut reloaded = Cockpit::open(&root).unwrap();
        reloaded.refresh().unwrap();
        assert!(reloaded.repos().iter().all(|r| r.claimed_by.is_none()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn docker_ps_lines() {
        let rows = parse_docker_ps("web\tnginx:1\tUp 2 hours\n");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "web");
        assert_eq!(rows[0].image, "nginx:1");
    }

    #[test]
    fn craft_version_and_tagline() {
        assert_eq!(
            cargo_version("version.workspace = true\nversion = \"0.2.0\"\n"),
            "0.2.0"
        );
        assert_eq!(
            tagline("# DevCraft\n\n![badge](x)\n\nSovereign cockpit.\n"),
            "DevCraft"
        );
    }

    #[test]
    fn changelog_tail_stops_at_the_next_release() {
        let text = "# Changelog\n\n## 0.2.0\n\n- moved parsers\n\n## 0.1.0\n\n- old\n";
        assert_eq!(changelog_tail(text), "## 0.2.0\n- moved parsers");
        assert_eq!(
            shortstat(" 3 files changed, 1 insertion(+), 4 deletions(-)\n"),
            "3 files, +1 / -4"
        );
    }

    #[test]
    fn a_root_copy_of_an_upstream_craft_is_not_scanned() {
        let root = scratch("mirror");
        fs::create_dir_all(root.join("_upstream").join("soundcraft").join(".git")).unwrap();
        fs::create_dir_all(root.join("soundcraft").join(".git")).unwrap();
        fs::create_dir_all(root.join("devcraft").join(".git")).unwrap();
        let pit = Cockpit::open(&root).unwrap();
        let names: Vec<_> = pit.scan_jobs().into_iter().map(|job| job.name).collect();
        assert!(!names.iter().any(|name| name == "soundcraft"));
        assert!(names.iter().any(|name| name == "devcraft"));
        assert!(craft_rows(&root).iter().all(|row| row.name != "soundcraft"));
        assert_eq!(crate::graphs::measure(&root).repos, 1);
    }

    #[test]
    fn upstream_reads_a_local_clone_only() {
        let root = scratch("upstream");
        let repo = root.join("_upstream").join("pdfcraft");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(root.join("_upstream").join("democraft")).unwrap();
        fs::create_dir_all(root.join("_upstream").join("buzz")).unwrap();
        let run = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        run(&["init"]);
        run(&["config", "user.email", "devcraft@example.com"]);
        run(&["config", "user.name", "devcraft"]);
        fs::write(repo.join("CHANGELOG.md"), "## 0.2.0\n\n- moved parsers\n").unwrap();
        run(&["add", "CHANGELOG.md"]);
        run(&["commit", "-m", "cut over parsers"]);
        let mut pit = Cockpit::open(&root).unwrap();
        let value = pit.call("devcraft_upstream", &json!({})).unwrap();
        let rows = value.as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], "pdfcraft");
        assert_eq!(rows[0]["commits"], 1);
        assert_eq!(rows[0]["subject"], "cut over parsers");
        assert!(rows[0]["changelog"]
            .as_str()
            .unwrap()
            .contains("moved parsers"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_a_bad_holder() {
        let root = scratch("holder");
        fs::create_dir_all(root.join("celcraft")).unwrap();
        let mut pit = Cockpit::open(&root).unwrap();
        let err = pit
            .call(
                "devcraft_claim",
                &json!({"repo": "celcraft", "holder": "vim"}),
            )
            .unwrap_err();
        assert!(err.contains("holder"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn release_all_clears_claims_and_checkpoint_needs_state() {
        let root = scratch("release-all");
        fs::create_dir_all(root.join("celcraft")).unwrap();
        let mut pit = Cockpit::open(&root).unwrap();
        let err = pit.call("devcraft_checkpoint", &json!({})).unwrap_err();
        assert!(err.contains("nothing"));
        pit.call(
            "devcraft_claim",
            &json!({"repo": "celcraft", "holder": "cursor"}),
        )
        .unwrap();
        let cleared = pit.call("devcraft_release_all", &json!({})).unwrap();
        assert_eq!(cleared["released"], 1);
        pit.call(
            "devcraft_claim",
            &json!({"repo": "celcraft", "holder": "opencode"}),
        )
        .unwrap();
        let shot = pit.call("devcraft_checkpoint", &json!({})).unwrap();
        assert_eq!(shot["files"], 5);
        let listed = pit.call("devcraft_snapshots", &json!({})).unwrap();
        assert_eq!(listed.as_array().map(Vec::len), Some(1));
        let bad = pit
            .call("devcraft_restart", &json!({"name": "bad name"}))
            .unwrap_err();
        assert!(bad.contains("unexpected"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn release_stale_claims_removes_old_and_keeps_fresh() {
        let root = scratch("stale-claims");
        fs::create_dir_all(root.join("celcraft")).unwrap();
        fs::create_dir_all(root.join("djcraft")).unwrap();
        let mut pit = Cockpit::open(&root).unwrap();
        pit.claims.insert(
            "celcraft".into(),
            Claim {
                holder: "claude".into(),
                since: "2020-01-01T00:00:00Z".into(),
            },
        );
        pit.claims.insert(
            "djcraft".into(),
            Claim {
                holder: "agy".into(),
                since: chrono_now(),
            },
        );
        pit.write_state().unwrap();
        assert_eq!(pit.stale_claims_count(72), 1);
        let res = pit
            .call(
                "devcraft_release_stale_claims",
                &json!({"max_age_hours": 72}),
            )
            .unwrap();
        assert_eq!(res["released"], 1);
        assert_eq!(pit.claims.len(), 1);
        assert!(pit.claims.contains_key("djcraft"));
        let st = pit.status();
        assert_eq!(st["stale_claims"], 0);
        assert!(!st["root"].as_str().unwrap_or("").starts_with(r"\\?\"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn task_key_shows_on_the_card_and_a_worktree_is_a_child() {
        let root = scratch("task-key");
        fs::create_dir_all(root.join("celcraft")).unwrap();
        let mut pit = Cockpit::open(&root).unwrap();
        pit.push_repo(Facts {
            id: "celcraft".into(),
            name: "celcraft".into(),
            path: root.join("celcraft").display().to_string(),
            has_git: true,
            branch: "main".into(),
            dirty: 0,
            untracked: 0,
            ahead: 0,
            unmerged: false,
            index_lock: false,
            has_agents: true,
            has_claude: false,
            handover_next: String::new(),
            handover_age_hours: None,
            last_commit_age_hours: Some(3),
            commits_14: [0; 14],
            worktrees: vec![board::Worktree {
                name: "celcraft-fix".into(),
                branch: "fix".into(),
            }],
            task_key: None,
            claimed_by: None,
            pinned: false,
            craft: true,
        });
        let bad = pit
            .call(
                "devcraft_task",
                &json!({"repo": "celcraft", "key": "bc-12"}),
            )
            .unwrap_err();
        assert!(bad.contains("BC-12"));
        pit.call(
            "devcraft_task",
            &json!({"repo": "celcraft", "key": "BC-12"}),
        )
        .unwrap();
        let cards = pit.cards();
        let parent = cards.iter().find(|card| card.id == "celcraft").unwrap();
        assert_eq!(parent.key, "BC-12");
        assert!(!parent.child);
        let child = cards
            .iter()
            .find(|card| card.id == "celcraft#celcraft-fix")
            .unwrap();
        assert!(child.child);
        assert_eq!(child.column, parent.column);
        let moved = pit
            .call(
                "devcraft_move",
                &json!({"card": "celcraft#celcraft-fix", "to": "ready"}),
            )
            .unwrap_err();
        assert!(moved.contains("worktree"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn sandbox_refuses_until_confirmed_and_repo_snapshot_copies_handover() {
        let root = scratch("ops-actions");
        fs::create_dir_all(root.join("celcraft")).unwrap();
        fs::write(root.join("celcraft").join("HANDOVER.md"), "next\n").unwrap();
        let mut pit = Cockpit::open(&root).unwrap();
        let err = pit
            .call(
                "devcraft_sandbox_run",
                &json!({"repo": "celcraft", "command": "echo ok", "confirm": false}),
            )
            .unwrap_err();
        assert!(err.contains("confirm"));
        let snap = pit
            .call("devcraft_checkpoint_repo", &json!({"repo": "celcraft"}))
            .unwrap();
        assert!(snap["files"].as_u64().unwrap() >= 1);
        let _ = fs::remove_dir_all(&root);
    }
}
