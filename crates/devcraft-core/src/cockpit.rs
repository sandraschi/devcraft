//! The cockpit: one state object for the window, the CLI, and MCP.
//! Git, GitHub, and Docker are read from the machine. Missing sources stay empty.

use crate::backup::BackupEngine;
use crate::board::{self, Facts, Move};
use crate::digest::{self, Digest};
use crate::handover;
use crate::ports::{PortAuditReport, PortSentinel};
use crate::resolve_workspace_root;
use crate::sandbox::ContainerSandbox;
use crate::services::ServiceSentinel;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const SNOOZE_SECS: u64 = 24 * 3600;
const EXTRA_CRAFTS: &[&str] = &["craft-deck", "craft-biz", "craft-sandbox-vbox"];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Claim {
    holder: String,
    since: String,
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
    pins: BTreeSet<String>,
    snooze: BTreeMap<String, u64>,
    scanned: bool,
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
            pins: BTreeSet::new(),
            snooze: BTreeMap::new(),
            scanned: false,
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
        self.pins = read_json(&self.state_dir().join("pins.json")).unwrap_or_default();
        self.snooze = read_json(&self.state_dir().join("snooze.json")).unwrap_or_default();
        self.drop_expired_snooze();
        let digest_path = self
            .root
            .join("mcp-central-docs")
            .join("operations")
            .join("fleet-status.md");
        self.digest = match fs::read_to_string(&digest_path) {
            Ok(text) => digest::parse(&text),
            Err(_) => Digest::missing(format!("missing {}", digest_path.display())),
        };
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

    pub fn status(&self) -> Value {
        json!({
            "version": crate::APP_VERSION,
            "transport": "stdio",
            "root": self.root.display().to_string(),
            "scanned": self.scanned,
            "repos": self.repos.len(),
            "shown": self.cards().len(),
            "quiet": self.quiet_count(),
            "claims": self.claims.len(),
            "crafts": self.crafts.len(),
            "digest": self.digest.generated,
            "digest_present": self.digest.present,
            "digest_note": self.digest.note,
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
            "devcraft_task" => {
                let repo = required_str(args, "repo")?;
                let key = args.get("key").and_then(Value::as_str).unwrap_or("");
                self.set_task(repo, key)
            }
            "devcraft_snapshots" => serde_json::to_value(BackupEngine::list_snapshots(&self.root))
                .map_err(|e| e.to_string()),
            "devcraft_checkpoint" => self.checkpoint_state(),
            "devcraft_sandbox" => Ok(json!({
                "docker": ContainerSandbox::is_docker_available(),
            })),
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
            other => Err(format!("unknown tool: {other}")),
        }
    }

    pub fn digest(&self) -> &Digest {
        &self.digest
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

    fn checkpoint_state(&self) -> Result<Value, String> {
        let dir = self.state_dir();
        let files = ["claims.json", "pins.json", "snooze.json", "tasks.json"]
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
        if !is_craft(&name) {
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
        assert_eq!(shot["files"], 4);
        let listed = pit.call("devcraft_snapshots", &json!({})).unwrap();
        assert_eq!(listed.as_array().map(Vec::len), Some(1));
        let bad = pit
            .call("devcraft_restart", &json!({"name": "bad name"}))
            .unwrap_err();
        assert!(bad.contains("unexpected"));
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
}
