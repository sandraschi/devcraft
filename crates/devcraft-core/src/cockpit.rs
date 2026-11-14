//! The cockpit: one state object for the window, the CLI, and MCP.
//! Git, GitHub, and Docker are read from the machine. Missing sources stay empty.

use crate::board::{self, Facts, Move};
use crate::digest::{self, Digest};
use crate::handover;
use crate::ports::{PortAuditReport, PortSentinel};
use crate::resolve_workspace_root;
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
    docker: Option<DockerReport>,
    ports: Option<PortAuditReport>,
    claims: BTreeMap<String, Claim>,
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
            docker: None,
            ports: None,
            claims: BTreeMap::new(),
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
            let key = facts
                .claimed_by
                .as_deref()
                .map(board::holder_mark)
                .unwrap_or("")
                .to_string();
            let detail = if facts.handover_next.is_empty() {
                facts.branch.clone()
            } else {
                truncate(&facts.handover_next, 90)
            };
            cards.push(CardView {
                id: facts.id.clone(),
                column: column.to_string(),
                title: facts.name.clone(),
                key,
                detail,
                holder: facts.claimed_by.clone(),
                craft: facts.craft,
                blocked: board::block_reason(facts),
                github: false,
            });
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
                detail: truncate(&format!("{} {} #{}", line.repo, line.kind, line.number), 110),
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
                let ide = args
                    .get("ide")
                    .and_then(Value::as_str)
                    .unwrap_or("cursor");
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
            claimed_by: job.holder.clone(),
            pinned: job.pinned,
            craft: is_craft(&job.name),
        }
    }

    pub fn clear_live(&mut self) {
        self.repos.clear();
        self.scanned = false;
    }

    pub fn push_repo(&mut self, facts: Facts) {
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
            return Err(format!("{repo} is not a directory under {}", self.root.display()));
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
        let root = self.root.canonicalize().unwrap_or_else(|_| self.root.clone());
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
        Cockpit::inspect(
            &ScanJob {
                name: name.to_string(),
                path: path.to_path_buf(),
                holder: self.claims.get(name).map(|c| c.holder.clone()),
                pinned: self.pins.contains(name),
            },
            now,
        )
    }

    fn apply_claims_to_repos(&mut self) {
        for facts in &mut self.repos {
            facts.claimed_by = self.claims.get(&facts.id).map(|c| c.holder.clone());
            facts.pinned = self.pins.contains(&facts.id);
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
        "next": f.handover_next,
        "craft": f.craft,
        "agents": f.has_agents,
        "claude": f.has_claude,
    })
}

pub fn tool_specs() -> Vec<Value> {
    vec![
        spec("devcraft_status", "Cockpit version, root, and whether a scan is loaded", &[]),
        spec("devcraft_refresh", "Scan sibling repositories and rebuild the board", &[]),
        spec("devcraft_board", "Cards on the fleet board", &[]),
        spec("devcraft_git", "Scanned repositories with branch and dirty counts", &[]),
        spec("devcraft_digest", "Totals from the morning digest file", &[]),
        spec("devcraft_reports", "Fleet reports with age and a stale mark", &[]),
        spec("devcraft_crafts", "Craft app checkouts next to this repo", &[]),
        spec("devcraft_brief", "Handover next step for one repository", &[("repo", "Repository name")]),
        spec("devcraft_claim", "Hold a repository for one IDE", &[("repo", "Repository name"), ("holder", "cursor, opencode, agy, or claude")]),
        spec("devcraft_release", "Release a claim", &[("repo", "Repository name")]),
        spec("devcraft_pin", "Pin or unpin a repository on the board", &[("repo", "Repository name")]),
        spec("devcraft_move", "Move a board card. Computed columns are refused.", &[("card", "Card id"), ("to", "Column id"), ("holder", "IDE when moving to Active")]),
        spec("devcraft_docker", "Docker daemon and running containers", &[]),
        spec("devcraft_ports", "Registered ports against listeners", &[]),
        spec("devcraft_open", "Open a checkout in Cursor, OpenCode, Claude, or Antigravity", &[("repo", "Repository name"), ("ide", "cursor, opencode, agy, or claude")]),
        spec("devcraft_reveal", "Open a fleet report or an https link", &[("path", "Report path or https link")]),
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
        ("morning-digest", "Morning digest", "operations/fleet-status.md"),
        ("session-log", "Session log", "operations/session-log"),
        ("aiwatcher", "AIWatcher log", "operations/session-log/aiwatcher"),
        ("port-registry", "Port registry", "operations/WEBAPP_PORTS.md"),
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
    let version = String::from_utf8_lossy(&version_out.stdout).trim().to_string();
    let ps = Command::new("docker")
        .args(["ps", "--format", "{{.Names}}\t{{.Image}}\t{{.Status}}"])
        .output();
    let containers = match ps {
        Ok(out) if out.status.success() => {
            parse_docker_ps(&String::from_utf8_lossy(&out.stdout))
        }
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
    let output = Command::new("git").arg("-C").arg(path).args(args).output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
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
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
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
        fs::write(root.join("celcraft").join("HANDOVER.md"), "## Next step\n- paint the board\n").unwrap();
        let mut pit = Cockpit::open(&root).unwrap();
        pit.refresh().unwrap();
        let claimed = pit.call("devcraft_claim", &json!({"repo": "celcraft", "holder": "claude"})).unwrap();
        assert_eq!(claimed["holder"], "claude");
        let brief = pit.call("devcraft_brief", &json!({"repo": "celcraft"})).unwrap();
        assert_eq!(brief["next"], "paint the board");
        assert_eq!(brief["column"], "active");
        pit.call("devcraft_move", &json!({"card": "celcraft", "to": "done"})).unwrap();
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
        assert_eq!(cargo_version("version.workspace = true\nversion = \"0.2.0\"\n"), "0.2.0");
        assert_eq!(tagline("# DevCraft\n\n![badge](x)\n\nSovereign cockpit.\n"), "DevCraft");
    }

    #[test]
    fn refuses_a_bad_holder() {
        let root = scratch("holder");
        fs::create_dir_all(root.join("celcraft")).unwrap();
        let mut pit = Cockpit::open(&root).unwrap();
        let err = pit
            .call("devcraft_claim", &json!({"repo": "celcraft", "holder": "vim"}))
            .unwrap_err();
        assert!(err.contains("holder"));
        let _ = fs::remove_dir_all(&root);
    }
}
