//! DevCraft window: fleet board on the shared craft shell.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod chat_dock;
mod graphs_ui;
mod skins;
mod welcome;

use craft_biz::bus::Bus;
use craft_biz_ui::app::{self, AppMenus, Frame, NavEntry, SuiteApp};
use craft_biz_ui::help::{AppInfo, Doc, HelpTab};
use craft_biz_ui::kanban::{self, Column};
use craft_biz_ui::menu::{Item, Menu};
use craft_biz_ui::shell::{self, UiSettings};
use craft_biz_ui::table::{Cell, Col, Table, TableEvent};
use craft_biz_ui::theme::Tone;
use craft_biz_ui::widgets::{self, Kpi};
use devcraft_core::board::{self, Facts};
use devcraft_core::cockpit::{tool_specs, CardView, Cockpit, DockerReport, ScanJob};
use devcraft_core::digest::{DirtyLine, DriftLine, GhLine};
use eframe::egui::{self, RichText, ScrollArea};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, SystemTime};

const APP_NAME: &str = "DevCraft";
const V_TODAY: usize = 0;
const V_GRAPHS: usize = 1;
const V_BOARD: usize = 2;
const V_GIT: usize = 3;
const V_GITHUB: usize = 4;
const V_DOCKER: usize = 5;
const V_REPORTS: usize = 6;
const V_CRAFTS: usize = 7;
const V_UPSTREAM: usize = 8;
const V_CLAIMS: usize = 9;
const V_PORTS: usize = 10;
const V_TIME: usize = 11;
const V_SANDBOX: usize = 12;
const V_SERVICE: usize = 13;
const V_TASKS: usize = 14;
const V_LOGS: usize = 15;
const V_TELEMETRY: usize = 16;
const V_WORKFLOWS: usize = 17;
const V_LLM: usize = 18;
const V_MEMORY: usize = 19;
const VIEWS: usize = 20;

fn settings_db() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("devcraft").join("devcraft.json")
}

fn num(value: Option<u32>) -> String {
    value.map(|n| n.to_string()).unwrap_or_else(|| "n/a".into())
}

enum Live {
    Repo(Box<Facts>),
    Done,
}

struct DevApp {
    pit: Cockpit,
    bus: Bus,
    selected: Option<String>,
    booted: bool,
    scan_rx: Option<Receiver<Live>>,
    scan_done: usize,
    scan_total: usize,
    docker_rx: Option<Receiver<DockerReport>>,
    docker_up: Option<bool>,
    service_name: String,
    service_note: String,
    task_repo: Option<String>,
    task_edit: String,
    todo_input: String,
    holder_filter: Option<String>,
    column_filter: Option<String>,
    smoke_notes: std::collections::HashMap<String, String>,
    smoke_rx: Option<Receiver<(String, String)>>,
    repo_snap: String,
    sandbox_repo: String,
    sandbox_cmd: String,
    sandbox_arm: bool,
    sandbox_note: String,
    sandbox_rx: Option<Receiver<String>>,
    ops_rx: Option<Receiver<devcraft_core::ops::OpsSnap>>,
    chat: chat_dock::ChatState,
    fresh_rows: Vec<devcraft_core::freshness::FreshRow>,
    fresh_rx: Option<Receiver<Vec<devcraft_core::freshness::FreshRow>>>,
    graph: Option<devcraft_core::graphs::GraphSnap>,
    graph_rx: Option<Receiver<devcraft_core::graphs::GraphSnap>>,
    graph_cache_loaded: bool,
    build_note: String,
    build_rx: Option<Receiver<String>>,
    build_arm: bool,
    welcome: welcome::WelcomeState,
    skin: skins::SkinState,
    proc_report: Option<devcraft_core::processes::ProcessReport>,
    proc_note: String,
    adn_query: String,
    adn_out: String,
}

impl DevApp {
    fn open() -> Result<DevApp, String> {
        let pit = Cockpit::from_env()?;
        let bus = Bus::new(pit.root.join(".devcraft"));
        Ok(DevApp {
            pit,
            bus,
            selected: None,
            booted: false,
            scan_rx: None,
            scan_done: 0,
            scan_total: 0,
            docker_rx: None,
            docker_up: None,
            service_name: String::new(),
            service_note: String::new(),
            task_repo: None,
            task_edit: String::new(),
            todo_input: String::new(),
            holder_filter: None,
            column_filter: None,
            smoke_notes: std::collections::HashMap::new(),
            smoke_rx: None,
            repo_snap: String::new(),
            sandbox_repo: String::new(),
            sandbox_cmd: String::new(),
            sandbox_arm: false,
            sandbox_note: String::new(),
            sandbox_rx: None,
            ops_rx: None,
            chat: chat_dock::ChatState::default(),
            fresh_rows: Vec::new(),
            fresh_rx: None,
            graph: None,
            graph_rx: None,
            graph_cache_loaded: false,
            build_note: String::new(),
            build_rx: None,
            build_arm: false,
            welcome: welcome::WelcomeState::default(),
            skin: skins::SkinState::load(),
            proc_report: None,
            proc_note: String::new(),
            adn_query: String::new(),
            adn_out: String::new(),
        })
    }

    fn kick_ops(&mut self) -> Result<Value, String> {
        let root = self.pit.root.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(devcraft_core::ops::load(&root));
        });
        self.ops_rx = Some(rx);
        Ok(json!({ "started": "ops" }))
    }

    fn kick_smoke(&mut self, args: &Value) -> Result<Value, String> {
        let repo = args
            .get("repo")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let root = self.pit.root.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let note = match Cockpit::craft_smoke_at(&root, &repo) {
                Ok(value) => format!(
                    "exit {} {}",
                    value.get("exit").and_then(Value::as_i64).unwrap_or(-1),
                    value.get("tail").and_then(Value::as_str).unwrap_or("")
                ),
                Err(err) => err,
            };
            let _ = tx.send((repo, note));
        });
        self.smoke_rx = Some(rx);
        Ok(json!({ "started": "smoke" }))
    }

    fn kick_sandbox(&mut self, args: &Value) -> Result<Value, String> {
        let confirm = match args.get("confirm") {
            Some(Value::Bool(flag)) => *flag,
            Some(Value::String(text)) => text == "true",
            _ => false,
        };
        if !confirm {
            return Err("confirm the sandbox run".into());
        }
        let repo = args
            .get("repo")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let command = args
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let root = self.pit.root.clone();
        let started = repo.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let note = match Cockpit::sandbox_run_at(&root, &repo, &command, true) {
                Ok(value) => value.to_string(),
                Err(err) => err,
            };
            let _ = tx.send(note);
        });
        self.sandbox_rx = Some(rx);
        Ok(json!({ "started": started }))
    }

    fn kick_freshness(&mut self) -> Result<Value, String> {
        let jobs: Vec<(String, std::path::PathBuf)> = self
            .pit
            .crafts()
            .iter()
            .map(|craft| (craft.name.clone(), std::path::PathBuf::from(&craft.path)))
            .collect();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let rows = jobs
                .iter()
                .map(|(name, path)| devcraft_core::freshness::inspect(path, name))
                .collect();
            let _ = tx.send(rows);
        });
        self.fresh_rx = Some(rx);
        Ok(json!({ "started": "freshness" }))
    }

    fn kick_graphs(&mut self) -> Result<Value, String> {
        let root = self.pit.root.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(devcraft_core::graphs::measure(&root));
        });
        self.graph_rx = Some(rx);
        Ok(json!({ "started": "graphs" }))
    }

    fn kick_build(&mut self, args: &Value) -> Result<Value, String> {
        let confirm = match args.get("confirm") {
            Some(Value::Bool(flag)) => *flag,
            Some(Value::String(text)) => text == "true",
            _ => false,
        };
        if !confirm {
            return Err("confirm the build".into());
        }
        let repo = args
            .get("repo")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let profile = args
            .get("profile")
            .and_then(Value::as_str)
            .unwrap_or("debug")
            .to_string();
        let root = self.pit.root.clone();
        let started = repo.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let note = match Cockpit::craft_build_at(&root, &repo, &profile, true) {
                Ok(value) => value.to_string(),
                Err(err) => err,
            };
            let _ = tx.send(note);
        });
        self.build_rx = Some(rx);
        Ok(json!({ "started": started }))
    }

    fn kick_scan(&mut self) -> Result<Value, String> {
        self.scan_rx = None;
        let jobs = self.pit.scan_jobs();
        let total = jobs.len();
        self.pit.clear_live();
        self.scan_done = 0;
        self.scan_total = total;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || scan_thread(jobs, tx));
        self.scan_rx = Some(rx);
        Ok(json!({ "started": total }))
    }

    fn kick_docker(&mut self) -> Result<Value, String> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(Cockpit::load_docker());
        });
        self.docker_rx = Some(rx);
        Ok(json!({ "started": "docker" }))
    }

    fn pump(&mut self, ctx: &egui::Context) {
        if !self.booted {
            self.booted = true;
            let _ = self.kick_scan();
            let _ = self.kick_docker();
        }
        let mut finished = false;
        let mut batch = 0;
        if let Some(rx) = &self.scan_rx {
            while batch < 8 {
                match rx.try_recv() {
                    Ok(Live::Repo(facts)) => {
                        self.pit.push_repo(*facts);
                        self.scan_done += 1;
                        batch += 1;
                    }
                    Ok(Live::Done) => {
                        finished = true;
                        break;
                    }
                    Err(_) => break,
                }
            }
        }
        if finished {
            self.pit.finish_scan();
            self.scan_rx = None;
        }
        if let Some(rx) = &self.docker_rx {
            if let Ok(report) = rx.try_recv() {
                self.pit.store_docker(report);
                self.docker_rx = None;
            }
        }
        if let Some(rx) = &self.ops_rx {
            if let Ok(snap) = rx.try_recv() {
                self.pit.store_ops(snap);
                self.ops_rx = None;
            }
        }
        if let Some(rx) = &self.smoke_rx {
            if let Ok((repo, note)) = rx.try_recv() {
                self.smoke_notes.insert(repo, note);
                self.smoke_rx = None;
            }
        }
        if let Some(rx) = &self.sandbox_rx {
            if let Ok(note) = rx.try_recv() {
                self.sandbox_note = note;
                self.sandbox_rx = None;
            }
        }
        if let Some(rx) = &self.fresh_rx {
            if let Ok(rows) = rx.try_recv() {
                self.fresh_rows = rows;
                self.fresh_rx = None;
            }
        }
        if let Some(rx) = &self.graph_rx {
            match rx.try_recv() {
                Ok(snap) => {
                    devcraft_core::graphs::save_cache(&self.pit.root, &snap);
                    self.graph = Some(snap);
                    self.graph_rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.graph_rx = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(rx) = &self.build_rx {
            if let Ok(note) = rx.try_recv() {
                self.build_note = note;
                self.build_rx = None;
            }
        }
        if self.scan_rx.is_some()
            || self.docker_rx.is_some()
            || self.ops_rx.is_some()
            || self.smoke_rx.is_some()
            || self.sandbox_rx.is_some()
            || self.fresh_rx.is_some()
            || self.build_rx.is_some()
            || self.graph_rx.is_some()
            || self.chat.busy
        {
            ctx.request_repaint_after(Duration::from_millis(90));
        }
    }

    fn progress(&self, ui: &mut egui::Ui) {
        if self.scan_rx.is_none() && self.docker_rx.is_none() {
            return;
        }
        if self.scan_total > 0 {
            ui.label(
                RichText::new(format!(
                    "Reading git  {} / {}",
                    self.scan_done, self.scan_total
                ))
                .size(13.5),
            );
            widgets::meter(
                ui,
                self.scan_done as f32 / self.scan_total as f32,
                ui.available_width(),
                Tone::Accent,
            );
        }
        if self.docker_rx.is_some() {
            ui.label(RichText::new("Asking Docker").size(13.0));
        }
        ui.add_space(8.0);
    }

    fn nav_entries(&self) -> Vec<NavEntry> {
        let shown = self.pit.cards().len();
        let claims = self
            .pit
            .repos()
            .iter()
            .filter(|r| r.claimed_by.is_some())
            .count();
        vec![
            NavEntry::new("\u{25C6}", "Today").group("Fleet"),
            NavEntry::new("\u{25C8}", "Graphs"),
            NavEntry::new("\u{25A6}", "Board").badge(shown, Tone::Accent),
            NavEntry::new("\u{25CF}", "Git"),
            NavEntry::new("\u{25B2}", "GitHub")
                .badge(self.pit.digest().needs_reply.len(), Tone::Warn),
            NavEntry::new("\u{25A3}", "Docker"),
            NavEntry::new("\u{2261}", "Reports").group("Library"),
            NavEntry::new("\u{00A7}", "Craft apps"),
            NavEntry::new("\u{2191}", "Upstream"),
            NavEntry::new("\u{20AC}", "Claims").badge(claims, Tone::Info),
            NavEntry::new("\u{2699}", "Ports").group("Host"),
            NavEntry::new("\u{23F1}", "Time machine"),
            NavEntry::new("\u{25A1}", "Sandbox"),
            NavEntry::new("\u{21BB}", "Services"),
            NavEntry::new("\u{29D6}", "Tasks").group("Ops"),
            NavEntry::new("\u{2263}", "Logs"),
            NavEntry::new("\u{25CE}", "Telemetry"),
            NavEntry::new("\u{25B6}", "Workflows").group("Craft"),
            NavEntry::new("\u{25C9}", "Memory").group("Ops"),
            NavEntry::new("\u{2699}", "LLM & Settings").group("Chat"),
        ]
    }

    fn header(&mut self, ui: &mut egui::Ui, title: &str, subtitle: &str, f: &mut Frame) {
        widgets::page_header(ui, title, subtitle, |ui| {
            if widgets::primary(ui, "Refresh fleet").clicked() {
                f.run("Fleet refreshed", "devcraft_refresh", json!({}));
            }
        });
    }

    fn view_today(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Today",
            "The morning digest, the live board, and the reports already on disk.",
            f,
        );
        self.progress(ui);
        let palette = self.skin.palette();
        let d = self.pit.digest().clone();
        let stale = d.age_hours.is_some_and(|hours| hours > 26);
        let stale_claims = self
            .pit
            .stale_claims_count(devcraft_core::cockpit::STALE_CLAIM_HOURS);
        let root_display = devcraft_core::clean_display_path(&self.pit.root);
        let health_led = if !d.present || stale || !d.needs_reply.is_empty() {
            palette.accent_b
        } else if stale_claims > 0 || d.stale_pr_count.unwrap_or(0) > 0 {
            palette.accent_warn
        } else {
            palette.accent_ok
        };

        skins::skin_card(ui, &palette, |ui| {
            ui.horizontal_wrapped(|ui| {
                skins::status_led(ui, health_led);
                ui.label(
                    RichText::new(format!("Fleet Console - {}", root_display))
                        .strong()
                        .color(palette.text_primary),
                );
                ui.separator();
                let digest_text = if !d.present {
                    format!("Digest missing ({})", d.note)
                } else if stale {
                    format!("Digest stale ({}h old)", d.age_hours.unwrap_or(0))
                } else {
                    format!("Digest {}", d.generated)
                };
                ui.label(RichText::new(digest_text).color(palette.text_secondary));
                if stale_claims > 0 {
                    ui.separator();
                    if ui
                        .button(format!("Release {} stale claim(s)", stale_claims))
                        .clicked()
                    {
                        f.run(
                            "Released stale claims",
                            "devcraft_release_stale_claims",
                            json!({ "max_age_hours": devcraft_core::cockpit::STALE_CLAIM_HOURS }),
                        );
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Processes").clicked() {
                        f.goto(V_SERVICE);
                    }
                    if ui.button("Copy Status JSON").clicked() {
                        if let Ok(text) = serde_json::to_string_pretty(&self.pit.status()) {
                            ui.ctx().copy_text(text);
                        }
                    }
                });
            });
        });
        ui.add_space(6.0);
        self.todo_section(ui);
        let note = if !d.present {
            d.note.clone()
        } else if stale {
            format!("{} - {}h old", d.generated, d.age_hours.unwrap_or(0))
        } else {
            d.generated.clone()
        };
        let kpis = [
            skins::HeroKpi {
                label: "OPEN PRS",
                value: num(d.open_prs),
                sub: note,
                accent: if d.present && !stale {
                    palette.accent_a
                } else {
                    palette.accent_warn
                },
            },
            skins::HeroKpi {
                label: "OPEN ISSUES",
                value: num(d.open_issues),
                sub: format!("{} stale", num(d.stale_issue_count)),
                accent: palette.accent_a,
            },
            skins::HeroKpi {
                label: "DIRTY TREES",
                value: num(d.dirty_worktrees),
                sub: format!("{} listed", d.dirty.len()),
                accent: palette.accent_warn,
            },
            skins::HeroKpi {
                label: "SYNC DRIFT",
                value: num(d.sync_drift),
                sub: "ahead/behind origin".into(),
                accent: if d.sync_drift.unwrap_or(0) > 0 {
                    palette.accent_b
                } else {
                    palette.accent_ok
                },
            },
            skins::HeroKpi {
                label: "NEEDS REPLY",
                value: d.needs_reply.len().to_string(),
                sub: "first reply queue".into(),
                accent: if d.needs_reply.is_empty() {
                    palette.accent_ok
                } else {
                    palette.accent_b
                },
            },
            skins::HeroKpi {
                label: "STALE PRS",
                value: num(d.stale_pr_count),
                sub: "quiet >= 7d".into(),
                accent: if d.stale_pr_count.unwrap_or(0) > 0 {
                    palette.accent_warn
                } else {
                    palette.accent_ok
                },
            },
            skins::HeroKpi {
                label: "CRAFT APPS",
                value: self.pit.crafts().len().to_string(),
                sub: format!("{} stale claims", stale_claims),
                accent: palette.accent_ok,
            },
            skins::HeroKpi {
                label: "ON BOARD",
                value: self.pit.cards().len().to_string(),
                sub: format!("{} quiet", self.pit.quiet_count()),
                accent: palette.accent_a,
            },
        ];
        skins::luminous_kpi_grid(ui, &palette, &kpis);
        ui.add_space(4.0);
        ui.columns(2, |cols| {
            cols[0].vertical(|ui| {
                widgets::section(ui, "This morning", None);
                skins::skin_card(ui, &palette, |ui| {
                    bars(
                        ui,
                        &[
                            ("PRs", opt_f(d.open_prs), Tone::Info),
                            ("Issues", opt_f(d.open_issues), Tone::Neutral),
                            ("Dirty", opt_f(d.dirty_worktrees), Tone::Warn),
                            ("Drift", opt_f(d.sync_drift), Tone::Danger),
                            ("Stale PRs", opt_f(d.stale_pr_count), Tone::Warn),
                            ("Stale issues", opt_f(d.stale_issue_count), Tone::Warn),
                            ("Needs reply", d.needs_reply.len() as f32, Tone::Danger),
                            ("Discussions", opt_f(d.discussion_count), Tone::Info),
                        ],
                    );
                });
            });
            cols[1].vertical(|ui| {
                widgets::section(ui, "Board columns", Some(self.pit.repos().len()));
                skins::skin_card(ui, &palette, |ui| {
                    if self.pit.repos().is_empty() {
                        ui.label(
                            "Git status is still arriving. The bars fill as each checkout is read.",
                        );
                    } else {
                        column_bars(ui, self.pit.repos());
                    }
                });
            });
        });
        ui.add_space(8.0);
        ui.columns(2, |cols| {
            cols[0].vertical(|ui| {
                widgets::section(ui, "Commit age", Some(self.pit.repos().len()));
                skins::skin_card(ui, &palette, |ui| age_bars(ui, self.pit.repos()));
            });
            cols[1].vertical(|ui| {
                widgets::section(ui, "Holders", None);
                skins::skin_card(ui, &palette, |ui| holder_bars(ui, self.pit.repos()));
            });
        });
        ui.add_space(8.0);
        widgets::section(ui, "14 days", None);
        skins::skin_card(ui, &palette, |ui| activity_bars(ui, self.pit.repos()));
        ui.add_space(8.0);
        self.gh_section(ui, f, "Needs a first reply", "reply", &d.needs_reply);
        self.dirty_section(ui, f, &d);
        self.drift_section(ui, f, &d.drift);
    }

    fn todo_section(&mut self, ui: &mut egui::Ui) {
        use devcraft_core::todos::{due_label, sorted, today_local};
        widgets::section(ui, "You", None);
        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.todo_input);
                if widgets::primary(ui, "Add today").clicked() {
                    let title = self.todo_input.clone();
                    match self.pit.call(
                        "devcraft_todo_add",
                        &json!({"title": title, "due": "today", "source": "desktop"}),
                    ) {
                        Ok(_) => self.todo_input.clear(),
                        Err(err) => {
                            ui.label(err);
                        }
                    }
                }
            });
            let today = today_local();
            let open: Vec<_> = sorted(self.pit.todos())
                .into_iter()
                .filter(|t| !t.done)
                .collect();
            let mut done_id: Option<String> = None;
            for todo in &open {
                ui.horizontal(|ui| {
                    if ui.button("Done").clicked() {
                        done_id = Some(todo.id.clone());
                    }
                    let due = due_label(todo, &today);
                    if due.is_empty() {
                        ui.label(&todo.title);
                    } else {
                        ui.label(format!("{}  ·  {due}", todo.title));
                    }
                });
            }
            if open.is_empty() {
                ui.label("No open todos. Add the first one above.");
            }
            if let Some(id) = done_id {
                if let Err(err) = self.pit.call("devcraft_todo_done", &json!({"id": id})) {
                    ui.label(err);
                }
            }
            let finished = self.pit.todos().iter().filter(|t| t.done).count();
            if finished > 0 {
                ui.horizontal(|ui| {
                    ui.label(format!("{finished} finished"));
                    if ui.button("Clear finished").clicked() {
                        if let Err(err) = self.pit.call("devcraft_todo_clear", &json!({})) {
                            ui.label(err);
                        }
                    }
                });
            }
        });
        ui.add_space(8.0);
    }

    fn view_board(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Board",
            "Click a bar to open that column. Click it again for every column. Drag is a command.",
            f,
        );
        self.progress(ui);
        if !self.pit.repos().is_empty() {
            if let Some(id) = column_chart(ui, self.pit.repos(), self.column_filter.as_deref()) {
                self.column_filter = if self.column_filter.as_deref() == Some(id) {
                    None
                } else {
                    Some(id.to_string())
                };
            }
            ui.add_space(8.0);
        }
        let mut next_holder = self.holder_filter.clone();
        ui.horizontal(|ui| {
            for choice in [
                None,
                Some("cursor"),
                Some("opencode"),
                Some("agy"),
                Some("claude"),
            ] {
                let label = choice.unwrap_or("All");
                if ui
                    .selectable_label(self.holder_filter.as_deref() == choice, label)
                    .clicked()
                {
                    next_holder = choice.map(str::to_string);
                }
            }
        });
        self.holder_filter = next_holder;
        let search = f.search.clone();
        let mut columns = vec![
            Column::new(board::INBOX, "Inbox", Tone::Danger),
            Column::new(board::READY, "Ready", Tone::Info),
            Column::new(board::ACTIVE, "Active", Tone::Accent).wip(3),
            Column::new(board::REVIEW, "Review", Tone::Warn),
            Column::new(board::BLOCKED, "Blocked", Tone::Danger),
            Column::new(board::DONE, "Done today", Tone::Ok),
            Column::new(board::QUIET, "Quiet", Tone::Neutral),
        ];
        if let Some(id) = &self.column_filter {
            columns.retain(|column| column.id == *id);
        }
        let views = if self.column_filter.as_deref() == Some(board::QUIET) {
            cards_for_column(self.pit.repos(), board::QUIET)
        } else {
            let mut views = self.pit.cards();
            if let Some(id) = &self.column_filter {
                views.retain(|card| card.column == *id);
            }
            views
        };
        let cards: Vec<kanban::Card> = views
            .iter()
            .filter(|card| self.card_visible(card, &search))
            .map(to_card)
            .collect();
        let rule = |card: &kanban::Card, to: &str| {
            if card.id.contains('#') {
                return Err("A worktree follows its checkout.".into());
            }
            board::allow(to, card.id.starts_with("gh:")).map(|_| ())
        };
        let wide = (ui.available_width() - 24.0).clamp(280.0, 720.0);
        let mut board = kanban::Board::new("fleet", &columns, &cards)
            .add_buttons(false)
            .rule(&rule)
            .selected(self.selected.as_deref());
        if self.column_filter.is_some() {
            board = board.column_width(wide);
        }
        let events = board.show(ui);
        for event in events {
            match event {
                kanban::Event::Moved { card, to, .. } => {
                    f.run(
                        "Card moved",
                        "devcraft_move",
                        json!({ "card": card, "to": to, "holder": "cursor" }),
                    );
                }
                kanban::Event::Refused { reason, .. } => f.toast(reason, Tone::Warn),
                kanban::Event::Select(id) | kanban::Event::Open(id) => self.selected = Some(id),
                kanban::Event::Add { .. } | kanban::Event::Undo => {}
            }
        }
    }

    fn view_git(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Git",
            "The digest lists dirty trees now. Live status fills the table underneath.",
            f,
        );
        self.progress(ui);
        let d = self.pit.digest().clone();
        widgets::kpi_row(
            ui,
            &[
                Kpi::new("Digest dirty", num(d.dirty_worktrees)),
                Kpi::new("Listed", d.dirty.len().to_string()),
                Kpi::new("Drift", d.drift.len().to_string()),
                Kpi::new("Live repos", self.pit.repos().len().to_string()).note(
                    if self.pit.scanned() {
                        "scan finished"
                    } else {
                        "scan running"
                    },
                    Tone::Accent,
                ),
            ],
        );
        if !self.pit.repos().is_empty() {
            widgets::section(ui, "Live columns", Some(self.pit.repos().len()));
            widgets::card(ui, |ui| column_bars(ui, self.pit.repos()));
        }
        self.dirty_section(ui, f, &d);
        widgets::section(ui, "Live checkouts", Some(self.pit.repos().len()));
        let cols = [
            Col::new("Repo", 1.4),
            Col::new("Branch", 1.0),
            Col::new("Dirty", 0.5),
            Col::new("Ahead", 0.5),
            Col::new("Column", 0.8),
            Col::new("Holder", 0.7),
        ];
        let rows: Vec<(String, Vec<Cell>)> = self
            .pit
            .repos()
            .iter()
            .filter(|r| {
                r.has_git && app::search_hit(&f.search, &[&r.name, &r.branch, &r.handover_next])
            })
            .map(git_row)
            .collect();
        if rows.is_empty() {
            widgets::empty_state(
                ui,
                "Waiting for git",
                "Rows appear as each checkout is read.",
            );
        }
        self.show_table(ui, "git", &cols, &rows);
    }

    fn view_github(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "GitHub",
            "Read from operations/fleet-status.md. This page does not call gh.",
            f,
        );
        let d = self.pit.digest().clone();
        if !d.present {
            widgets::empty_state(ui, "No morning digest", &d.note);
            return;
        }
        ui.label(RichText::new(format!("Generated {}", d.generated)).size(13.5));
        widgets::kpi_row(
            ui,
            &[
                Kpi::new("Repos in the digest", num(d.repos_scanned)),
                Kpi::new("Open PRs", num(d.open_prs)),
                Kpi::new("Open issues", num(d.open_issues)),
                Kpi::new("Discussions", num(d.discussion_count)),
            ],
        );
        widgets::card(ui, |ui| {
            bars(
                ui,
                &[
                    ("PRs", opt_f(d.open_prs), Tone::Info),
                    ("Issues", opt_f(d.open_issues), Tone::Neutral),
                    ("Stale PRs", opt_f(d.stale_pr_count), Tone::Warn),
                    ("Stale issues", opt_f(d.stale_issue_count), Tone::Warn),
                    ("Needs reply", d.needs_reply.len() as f32, Tone::Danger),
                    ("Unanswered", d.unanswered.len() as f32, Tone::Warn),
                ],
            );
        });
        self.gh_section(ui, f, "Needs a first reply", "reply", &d.needs_reply);
        self.gh_section(ui, f, "Stale pull requests", "stalepr", &d.stale_prs);
        self.gh_section(ui, f, "Stale issues", "staleissue", &d.stale_issues);
        self.gh_section(ui, f, "Unanswered questions", "qa", &d.unanswered);
        if !d.errors.is_empty() {
            widgets::section(ui, "Scan errors", Some(d.errors.len()));
            widgets::card(ui, |ui| {
                for line in &d.errors {
                    ui.label(line);
                }
            });
        }
    }

    fn view_docker(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(ui, "Docker", "Daemon version and running containers.", f);
        self.progress(ui);
        if ui.button("Probe Docker again").clicked() {
            f.run("Docker", "devcraft_docker", json!({}));
        }
        let Some(report) = self.pit.docker().cloned() else {
            widgets::empty_state(
                ui,
                "Asking the daemon",
                "The first probe starts with the window.",
            );
            return;
        };
        let tone = if report.up { Tone::Ok } else { Tone::Warn };
        widgets::kpi_row(
            ui,
            &[Kpi::new("Daemon", if report.up { "up" } else { "down" })
                .note(report.version.clone(), tone)],
        );
        if !report.note.is_empty() {
            ui.label(&report.note);
        }
        let cols = [
            Col::new("Name", 1.0),
            Col::new("Image", 1.2),
            Col::new("Status", 1.0),
        ];
        let rows: Vec<(String, Vec<Cell>)> = report
            .containers
            .iter()
            .map(|c| {
                (
                    c.name.clone(),
                    vec![
                        Cell::Strong(c.name.clone()),
                        Cell::Mono(c.image.clone()),
                        Cell::Text(c.status.clone()),
                    ],
                )
            })
            .collect();
        self.show_table(ui, "docker", &cols, &rows);
    }

    fn view_reports(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Reports",
            "The daily page, the digest, and the session log.",
            f,
        );
        let ages: Vec<(String, f32, Tone)> = self
            .pit
            .reports()
            .iter()
            .map(|r| {
                let hours = r.age_hours.unwrap_or(0) as f32;
                let tone = if !r.present {
                    Tone::Danger
                } else if r.stale {
                    Tone::Warn
                } else {
                    Tone::Ok
                };
                (r.title.clone(), hours, tone)
            })
            .collect();
        widgets::section(ui, "Age in hours", Some(ages.len()));
        let age_refs: Vec<(&str, f32, Tone)> = ages
            .iter()
            .map(|(name, hours, tone)| (name.as_str(), *hours, *tone))
            .collect();
        widgets::card(ui, |ui| bars(ui, &age_refs));
        let cols = [
            Col::new("Report", 1.2),
            Col::new("Age (h)", 0.5),
            Col::new("State", 0.6),
            Col::new("Path", 2.0),
        ];
        let rows: Vec<(String, Vec<Cell>)> = self
            .pit
            .reports()
            .iter()
            .map(|r| {
                let age = r
                    .age_hours
                    .map(|h| h.to_string())
                    .unwrap_or_else(|| "-".into());
                let state = if !r.present {
                    Cell::Pill("missing".into(), Tone::Danger)
                } else if r.stale {
                    Cell::Pill("stale".into(), Tone::Warn)
                } else {
                    Cell::Pill("fresh".into(), Tone::Ok)
                };
                (
                    r.id.clone(),
                    vec![
                        Cell::Strong(r.title.clone()),
                        Cell::Text(age),
                        state,
                        Cell::Weak(r.path.clone()),
                    ],
                )
            })
            .collect();
        self.show_table(ui, "reports", &cols, &rows);
        if let Some(id) = self.selected.clone() {
            if let Some(row) = self.pit.reports().iter().find(|r| r.id == id) {
                if ui.button("Open report").clicked() {
                    f.run(
                        "Report opened",
                        "devcraft_reveal",
                        json!({ "path": row.path }),
                    );
                }
            }
        }
    }

    fn view_crafts(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Craft apps",
            "Checkouts beside DevCraft. Install and update stay in CraftDeck.",
            f,
        );
        let (craft_count, built) = {
            let crafts = self.pit.crafts();
            (crafts.len(), crafts.iter().filter(|c| c.has_binary).count())
        };
        let total = craft_count.max(1) as f32;
        widgets::kpi_row(
            ui,
            &[
                Kpi::new("Apps", craft_count.to_string()),
                Kpi::new("Release binary", built.to_string()),
                Kpi::new("Source only", (craft_count - built).to_string()),
            ],
        );
        widgets::card(ui, |ui| {
            ui.label("Share of apps with a release binary");
            widgets::meter(ui, built as f32 / total, ui.available_width(), Tone::Ok);
            ui.label("Freshness, debug and release builds, and SOP requests live on Workflows.");
            if ui.button("Open workflows").clicked() {
                f.goto(V_WORKFLOWS);
            }
        });
        ui.add_space(6.0);
        let smoke_name = self
            .selected
            .clone()
            .filter(|name| self.pit.crafts().iter().any(|craft| &craft.name == name));
        if let Some(name) = smoke_name {
            ui.horizontal(|ui| {
                if ui.button("Smoke selected").clicked() {
                    f.run(
                        "Smoke",
                        "devcraft_craft_smoke",
                        json!({ "repo": name.clone() }),
                    );
                }
                if let Some(note) = self.smoke_notes.get(&name) {
                    ui.label(note);
                }
            });
        }
        let cols = [
            Col::new("App", 1.0),
            Col::new("Version", 0.5),
            Col::new("Binary", 0.5),
            Col::new("Handover", 0.6),
            Col::new("Smoke", 0.8),
            Col::new("Tagline", 2.0),
        ];
        let notes = self.smoke_notes.clone();
        let rows: Vec<(String, Vec<Cell>)> = self
            .pit
            .crafts()
            .iter()
            .map(|c| {
                (
                    c.name.clone(),
                    vec![
                        Cell::Strong(c.name.clone()),
                        Cell::Mono(c.version.clone()),
                        Cell::Pill(
                            if c.has_binary { "yes" } else { "no" }.into(),
                            if c.has_binary {
                                Tone::Ok
                            } else {
                                Tone::Neutral
                            },
                        ),
                        Cell::Text(if c.has_handover { "yes" } else { "no" }.into()),
                        Cell::Weak(notes.get(&c.name).cloned().unwrap_or_else(|| "-".into())),
                        Cell::Weak(c.tagline.clone()),
                    ],
                )
            })
            .collect();
        self.show_table(ui, "crafts", &cols, &rows);
    }

    fn view_upstream(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Upstream",
            "Storytold clones in _upstream. The last 24 hours already in the local checkout. This page does not fetch.",
            f,
        );
        self.pit.load_upstream();
        let clones = self.pit.upstream().len();
        let moved = self
            .pit
            .upstream()
            .iter()
            .filter(|row| row.commits > 0)
            .count();
        let commits: u32 = self.pit.upstream().iter().map(|row| row.commits).sum();
        widgets::kpi_row(
            ui,
            &[
                Kpi::new("Clones", clones.to_string()),
                Kpi::new("Moved", moved.to_string()),
                Kpi::new("Commits", commits.to_string()),
            ],
        );
        if clones == 0 {
            widgets::empty_state(
                ui,
                "No clones",
                "_upstream has no Storytold craft checkout.",
            );
            return;
        }
        let cols = [
            Col::new("App", 0.9),
            Col::new("Tag", 0.6),
            Col::new("24h", 0.4),
            Col::new("Delta", 1.1),
            Col::new("Latest", 1.6),
            Col::new("Changelog", 1.6),
        ];
        let rows: Vec<(String, Vec<Cell>)> = self
            .pit
            .upstream()
            .iter()
            .map(|row| {
                let latest = row
                    .subjects
                    .first()
                    .cloned()
                    .unwrap_or_else(|| row.subject.clone());
                (
                    row.name.clone(),
                    vec![
                        Cell::Strong(row.name.clone()),
                        Cell::Mono(row.tag.clone()),
                        Cell::Pill(
                            row.commits.to_string(),
                            if row.commits > 0 {
                                Tone::Warn
                            } else {
                                Tone::Neutral
                            },
                        ),
                        Cell::Text(row.delta.clone()),
                        Cell::Text(latest),
                        Cell::Weak(changelog_line(&row.changelog)),
                    ],
                )
            })
            .collect();
        self.show_table(ui, "upstream", &cols, &rows);
    }

    fn view_claims(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Claims",
            "Who holds a checkout: cursor, opencode, agy, claude. Claims older than 72h are marked stale.",
            f,
        );
        let details = self.pit.claim_details();
        let stale_count = details.iter().filter(|c| c.stale).count();
        let cols = [
            Col::new("Repo", 1.1),
            Col::new("Holder", 0.6),
            Col::new("Age", 0.5),
            Col::new("Status", 0.6),
        ];
        let rows: Vec<(String, Vec<Cell>)> = details
            .iter()
            .map(|c| {
                let age_text = match c.age_hours {
                    Some(h) if h >= 24 => format!("{}d {}h", h / 24, h % 24),
                    Some(h) => format!("{}h", h),
                    None => "n/a".into(),
                };
                let (pill, tone) = if c.stale {
                    ("Stale (>72h)".into(), Tone::Warn)
                } else {
                    ("Active".into(), Tone::Ok)
                };
                (
                    c.repo.clone(),
                    vec![
                        Cell::Strong(c.repo.clone()),
                        Cell::Text(c.holder.clone()),
                        Cell::Text(age_text),
                        Cell::Pill(pill, tone),
                    ],
                )
            })
            .collect();
        ui.horizontal(|ui| {
            if widgets::primary(ui, "Release every claim").clicked() {
                f.run("Claims released", "devcraft_release_all", json!({}));
            }
            if stale_count > 0
                && ui
                    .button(format!("Release stale ({})", stale_count))
                    .clicked()
            {
                f.run(
                    "Stale claims released",
                    "devcraft_release_stale_claims",
                    json!({ "max_age_hours": devcraft_core::cockpit::STALE_CLAIM_HOURS }),
                );
            }
        });
        ui.add_space(6.0);
        if rows.is_empty() {
            widgets::empty_state(ui, "No claims", "Claim a card from the detail drawer.");
        }
        self.show_table(ui, "claims", &cols, &rows);
    }

    fn view_time(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Time machine",
            "Snapshots of cockpit state, or of one checkout's handover and changelog.",
            f,
        );
        if widgets::primary(ui, "Snapshot cockpit state").clicked() {
            f.run("Snapshot written", "devcraft_checkpoint", json!({}));
        }
        ui.add_space(6.0);
        ui.label("Repository");
        ui.text_edit_singleline(&mut self.repo_snap);
        if ui.button("Snapshot this repo").clicked() {
            f.run(
                "Repo snapshot",
                "devcraft_checkpoint_repo",
                json!({ "repo": self.repo_snap }),
            );
        }
        let snaps = match self.pit.call("devcraft_snapshots", &json!({})) {
            Ok(value) => value,
            Err(err) => {
                ui.label(err);
                return;
            }
        };
        let rows = snaps.as_array().cloned().unwrap_or_default();
        if rows.is_empty() {
            widgets::empty_state(ui, "No snapshots", "The button writes the first one.");
            return;
        }
        let cols = [
            Col::new("Id", 1.4),
            Col::new("Tag", 0.6),
            Col::new("Files", 0.4),
        ];
        let table: Vec<(String, Vec<Cell>)> = rows
            .iter()
            .filter_map(|row| {
                let id = row.get("id")?.as_str()?.to_string();
                let tag = row
                    .get("tag")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let files = row
                    .get("backed_up_files")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0);
                Some((
                    id.clone(),
                    vec![
                        Cell::Mono(id),
                        Cell::Text(tag),
                        Cell::Text(files.to_string()),
                    ],
                ))
            })
            .collect();
        self.show_table(ui, "snapshots", &cols, &table);
    }

    fn view_sandbox(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Sandbox",
            "A confirmed command runs in Docker when the daemon answers, otherwise on the host.",
            f,
        );
        if widgets::primary(ui, "Check Docker").clicked() {
            self.docker_up = self
                .pit
                .call("devcraft_sandbox", &json!({}))
                .ok()
                .and_then(|value| value.get("docker").and_then(Value::as_bool));
            let _ = f;
        }
        let (label, tone) = match self.docker_up {
            Some(true) => ("up", Tone::Ok),
            Some(false) => ("down", Tone::Warn),
            None => ("not checked", Tone::Neutral),
        };
        widgets::kpi_row(
            ui,
            &[Kpi::new("Daemon", label).note("checked only when the button is pressed", tone)],
        );
        ui.add_space(6.0);
        ui.label("Repository");
        ui.text_edit_singleline(&mut self.sandbox_repo);
        ui.label("Command");
        ui.text_edit_singleline(&mut self.sandbox_cmd);
        ui.checkbox(&mut self.sandbox_arm, "I mean to run this");
        if widgets::primary(ui, "Run").clicked() {
            f.run(
                "Sandbox",
                "devcraft_sandbox_run",
                json!({
                    "repo": self.sandbox_repo,
                    "command": self.sandbox_cmd,
                    "confirm": self.sandbox_arm,
                }),
            );
        }
        if self.sandbox_rx.is_some() {
            ui.label("Running");
        }
        if !self.sandbox_note.is_empty() {
            ui.label(&self.sandbox_note);
        }
    }

    fn view_service(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Services",
            "sc.exe query and restart. Nothing restarts until the button is pressed.",
            f,
        );
        ui.label("Service name");
        ui.text_edit_singleline(&mut self.service_name);
        ui.horizontal(|ui| {
            if widgets::primary(ui, "Query").clicked() {
                match self
                    .pit
                    .call("devcraft_service", &json!({ "name": self.service_name }))
                {
                    Ok(value) => self.service_note = value.to_string(),
                    Err(err) => self.service_note = err,
                }
                let _ = f;
            }
            if ui.button("Restart").clicked() {
                match self
                    .pit
                    .call("devcraft_restart", &json!({ "name": self.service_name }))
                {
                    Ok(value) => self.service_note = value.to_string(),
                    Err(err) => self.service_note = err,
                }
            }
        });
        if !self.service_note.is_empty() {
            ui.label(&self.service_note);
        }
        ui.add_space(12.0);
        let palette = self.skin.palette();
        widgets::section(ui, "DevCraft Processes & Stale MCP Workers", None);
        skins::skin_card(ui, &palette, |ui| {
            ui.horizontal(|ui| {
                if widgets::primary(ui, "Inspect processes").clicked() {
                    let report = devcraft_core::processes::inspect_devcraft_processes(
                        devcraft_core::processes::DEFAULT_STALE_HOURS,
                    );
                    self.proc_note = format!(
                        "{} running, {} stale (current PID {})",
                        report.total, report.stale_count, report.current_pid
                    );
                    self.proc_report = Some(report);
                }
                if ui.button("Kill stale MCP workers").clicked() {
                    match self.pit.call("devcraft_kill_stale_processes", &json!({})) {
                        Ok(val) => {
                            let killed =
                                val.get("killed_count").and_then(Value::as_u64).unwrap_or(0);
                            self.proc_note = format!("Stopped {} stale process(es).", killed);
                            self.proc_report =
                                Some(devcraft_core::processes::inspect_devcraft_processes(
                                    devcraft_core::processes::DEFAULT_STALE_HOURS,
                                ));
                        }
                        Err(err) => self.proc_note = err,
                    }
                }
            });
            if !self.proc_note.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new(&self.proc_note).color(palette.text_secondary));
            }
            if let Some(report) = &self.proc_report {
                ui.add_space(6.0);
                for entry in &report.entries {
                    ui.horizontal(|ui| {
                        let led_color = if entry.stale {
                            palette.accent_b
                        } else {
                            palette.accent_ok
                        };
                        skins::status_led(ui, led_color);
                        ui.monospace(format!("PID {}", entry.pid));
                        ui.label(RichText::new(&entry.name).strong());
                        if let Some(h) = entry.age_hours {
                            ui.label(format!("{}h", h));
                        }
                        if !entry.reason.is_empty() {
                            ui.label(RichText::new(&entry.reason).color(palette.text_secondary));
                        }
                    });
                }
            }
        });
    }

    fn view_ports(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Ports",
            "Registered ports in WEBAPP_PORTS.md. Listening means the bind failed.",
            f,
        );
        if ui.button("Audit listeners").clicked() {
            f.run("Ports", "devcraft_ports", json!({}));
        }
        let findings = self.pit.lock_findings().to_vec();
        if !findings.is_empty() {
            widgets::section(ui, "Lock findings", Some(findings.len()));
            let cols = [
                Col::new("Code", 0.4),
                Col::new("Severity", 0.5),
                Col::new("Finding", 2.4),
            ];
            let rows: Vec<(String, Vec<Cell>)> = findings
                .iter()
                .map(|item| {
                    let tone = if item.severity == "error" {
                        Tone::Danger
                    } else {
                        Tone::Warn
                    };
                    (
                        item.code.clone(),
                        vec![
                            Cell::Mono(item.code.clone()),
                            Cell::Pill(item.severity.clone(), tone),
                            Cell::Text(item.message.clone()),
                        ],
                    )
                })
                .collect();
            self.show_table(ui, "lock", &cols, &rows);
            ui.add_space(6.0);
        }
        let Some(report) = self.pit.ports().cloned() else {
            widgets::empty_state(ui, "No registry", "WEBAPP_PORTS.md was not read.");
            return;
        };
        let listening_note = if report.checked {
            format!("{} listening", report.active_listeners)
        } else {
            "not probed yet".into()
        };
        widgets::kpi_row(
            ui,
            &[
                Kpi::new("Registered", report.total_registered.to_string())
                    .note(listening_note, Tone::Info),
                Kpi::new(
                    "Listening",
                    if report.checked {
                        report.active_listeners.to_string()
                    } else {
                        "-".into()
                    },
                ),
            ],
        );
        if report.checked && report.total_registered > 0 {
            widgets::card(ui, |ui| {
                ui.label("Listeners among registered ports");
                widgets::meter(
                    ui,
                    report.active_listeners as f32 / report.total_registered as f32,
                    ui.available_width(),
                    Tone::Ok,
                );
            });
        }
        let cols = [
            Col::new("Port", 0.4),
            Col::new("Repo", 1.0),
            Col::new("Listening", 0.6),
            Col::new("What it is", 1.6),
        ];
        let rows: Vec<(String, Vec<Cell>)> = report
            .entries
            .iter()
            .filter(|e| app::search_hit(&f.search, &[&e.repo, &e.description, &e.port.to_string()]))
            .map(|e| {
                let listen = if !report.checked {
                    Cell::Pill("not checked".into(), Tone::Neutral)
                } else if e.is_listening {
                    Cell::Pill("yes".into(), Tone::Ok)
                } else {
                    Cell::Pill("no".into(), Tone::Neutral)
                };
                (
                    e.port.to_string(),
                    vec![
                        Cell::Mono(e.port.to_string()),
                        Cell::Strong(e.repo.clone()),
                        listen,
                        Cell::Weak(e.description.clone()),
                    ],
                )
            })
            .collect();
        self.show_table(ui, "ports", &cols, &rows);
    }

    fn gh_section(
        &mut self,
        ui: &mut egui::Ui,
        f: &Frame,
        title: &str,
        prefix: &str,
        rows: &[GhLine],
    ) {
        let shown: Vec<(usize, &GhLine)> = rows
            .iter()
            .enumerate()
            .filter(|(_, g)| app::search_hit(&f.search, &[&g.repo, &g.title, &g.kind, &g.number]))
            .collect();
        widgets::section(ui, title, Some(shown.len()));
        if shown.is_empty() {
            widgets::empty_state(
                ui,
                "None in this list",
                "The morning digest has no rows here.",
            );
            return;
        }
        let cols = [
            Col::new("Repo", 1.2),
            Col::new("Kind", 0.5),
            Col::new("#", 0.4),
            Col::new("Title", 2.2),
        ];
        let table: Vec<(String, Vec<Cell>)> = shown
            .into_iter()
            .map(|(i, g)| {
                (
                    format!("{prefix}:{i}"),
                    vec![
                        Cell::Strong(g.repo.clone()),
                        Cell::Pill(g.kind.clone(), Tone::Warn),
                        Cell::Mono(format!("#{}", g.number)),
                        Cell::Text(g.title.clone()),
                    ],
                )
            })
            .collect();
        self.show_table(ui, prefix, &cols, &table);
    }

    fn dirty_section(&mut self, ui: &mut egui::Ui, f: &Frame, d: &devcraft_core::digest::Digest) {
        let rows: Vec<&DirtyLine> = d
            .dirty
            .iter()
            .filter(|row| app::search_hit(&f.search, &[&row.repo, &row.detail]))
            .collect();
        let extra = if d.dirty_more > 0 {
            format!("+{} more in the digest", d.dirty_more)
        } else {
            String::new()
        };
        widgets::section(ui, "Dirty worktrees", Some(rows.len()));
        if !extra.is_empty() {
            ui.label(RichText::new(extra).size(13.0));
        }
        if rows.is_empty() {
            return;
        }
        let max = rows.iter().map(|r| r.files).max().unwrap_or(1).max(1) as f32;
        widgets::card(ui, |ui| {
            for row in rows.iter().take(12) {
                ui.label(format!("{}   {}", row.repo, row.files));
                widgets::meter(ui, row.files as f32 / max, ui.available_width(), Tone::Warn);
            }
        });
        let cols = [
            Col::new("Repo", 1.0),
            Col::new("Files", 0.4),
            Col::new("What changed", 2.2),
        ];
        let table: Vec<(String, Vec<Cell>)> = rows
            .into_iter()
            .map(|row| {
                (
                    format!("dirty:{}", row.repo),
                    vec![
                        Cell::Strong(row.repo.clone()),
                        Cell::Text(row.files.to_string()),
                        Cell::Weak(row.detail.clone()),
                    ],
                )
            })
            .collect();
        self.show_table(ui, "dirty", &cols, &table);
    }

    fn drift_section(&mut self, ui: &mut egui::Ui, f: &Frame, rows: &[DriftLine]) {
        let shown: Vec<&DriftLine> = rows
            .iter()
            .filter(|row| app::search_hit(&f.search, &[&row.repo]))
            .collect();
        widgets::section(ui, "Sync drift", Some(shown.len()));
        if shown.is_empty() {
            return;
        }
        let cols = [
            Col::new("Repo", 1.4),
            Col::new("Ahead", 0.5),
            Col::new("Behind", 0.5),
        ];
        let table: Vec<(String, Vec<Cell>)> = shown
            .into_iter()
            .map(|row| {
                (
                    format!("drift:{}", row.repo),
                    vec![
                        Cell::Strong(row.repo.clone()),
                        Cell::Text(row.ahead.to_string()),
                        Cell::Text(row.behind.to_string()),
                    ],
                )
            })
            .collect();
        self.show_table(ui, "drift", &cols, &table);
    }

    fn show_table(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        cols: &[Col],
        rows: &[(String, Vec<Cell>)],
    ) {
        if rows.is_empty() {
            return;
        }
        for event in Table::new(id, cols, rows)
            .selected(self.selected.as_deref())
            .show(ui)
        {
            match event {
                TableEvent::Select(id) | TableEvent::Open(id) => self.selected = Some(id),
            }
        }
    }

    fn view_graphs(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Graphs",
            "Commit weather, code tide, languages, and releases. Hidden folders stay out.",
            f,
        );
        if !self.graph_cache_loaded {
            self.graph_cache_loaded = true;
            self.graph = devcraft_core::graphs::load_cache(&self.pit.root);
        }
        let label = if self.graph.is_some() {
            "Measure again"
        } else {
            "Measure the fleet"
        };
        if ui.button(label).clicked() {
            f.run("Graphs", "devcraft_graphs", json!({}));
        }
        if self.graph_rx.is_some() {
            ui.add_space(6.0);
            ui.label("Reading git history and line counts.");
        }
        ui.add_space(10.0);
        if let Some(snap) = &self.graph {
            graphs_ui::show(ui, snap);
        } else if self.graph_rx.is_none() {
            graphs_ui::empty(ui);
        }
    }

    fn view_workflows(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Workflows",
            "Craft-app freshness, builds, and SOP requests. A request does not edit the checkout.",
            f,
        );
        widgets::section(ui, "Craft apps", None);
        if ui.button("Scan freshness").clicked() {
            f.run("Freshness", "devcraft_freshness", json!({}));
        }
        if self.fresh_rx.is_some() {
            ui.label("Scanning craft checkouts");
        }
        if !self.fresh_rows.is_empty() {
            let release = self.fresh_rows.iter().filter(|row| row.release_exe).count();
            let debug = self.fresh_rows.iter().filter(|row| row.debug_exe).count();
            let help = self.fresh_rows.iter().filter(|row| row.help).count();
            let menu = self.fresh_rows.iter().filter(|row| row.menu_bar).count();
            widgets::kpi_row(
                ui,
                &[
                    Kpi::new("Release exe", release.to_string()),
                    Kpi::new("Debug exe", debug.to_string()),
                    Kpi::new("Help docs", help.to_string()),
                    Kpi::new("Menu strings", menu.to_string()),
                ],
            );
            let cols = [
                Col::new("App", 1.0),
                Col::new("Release", 0.5),
                Col::new("Debug", 0.5),
                Col::new("Help", 0.5),
                Col::new("Menu", 0.5),
            ];
            let rows: Vec<(String, Vec<Cell>)> = self
                .fresh_rows
                .iter()
                .map(|row| {
                    (
                        row.name.clone(),
                        vec![
                            Cell::Strong(row.name.clone()),
                            Cell::Pill(
                                yes_no(row.release_exe),
                                if row.release_exe {
                                    Tone::Ok
                                } else {
                                    Tone::Warn
                                },
                            ),
                            Cell::Pill(
                                yes_no(row.debug_exe),
                                if row.debug_exe {
                                    Tone::Ok
                                } else {
                                    Tone::Neutral
                                },
                            ),
                            Cell::Pill(
                                yes_no(row.help),
                                if row.help { Tone::Ok } else { Tone::Warn },
                            ),
                            Cell::Pill(
                                yes_no(row.menu_bar),
                                if row.menu_bar { Tone::Ok } else { Tone::Warn },
                            ),
                        ],
                    )
                })
                .collect();
            self.show_table(ui, "fresh", &cols, &rows);
        }
        ui.add_space(8.0);
        widgets::section(ui, "Build the selected craft app", None);
        ui.checkbox(&mut self.build_arm, "I mean to build");
        ui.horizontal(|ui| {
            if ui.button("Debug").clicked() {
                self.queue_build(f, "debug");
            }
            if ui.button("Release").clicked() {
                self.queue_build(f, "release");
            }
        });
        if self.build_rx.is_some() {
            ui.label("Building");
        }
        if !self.build_note.is_empty() {
            ui.label(&self.build_note);
        }
        ui.add_space(8.0);
        widgets::section(ui, "Queue an SOP", None);
        ui.label("Writes a request under .devcraft/workflows for the selected checkout.");
        ui.horizontal_wrapped(|ui| {
            for kind in [
                "craft-assess",
                "assfix",
                "gapfind",
                "drift-scan",
                "quality-check",
                "doc-sync",
            ] {
                if ui.button(kind).clicked() {
                    let Some(repo) = self.selected.clone() else {
                        f.toast("Select a checkout first", Tone::Warn);
                        continue;
                    };
                    f.run(
                        "Workflow queued",
                        "devcraft_workflow",
                        json!({ "kind": kind, "repo": repo, "confirm": true }),
                    );
                }
            }
        });
    }

    fn view_memory(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Memory",
            "advanced-memory-mcp over 127.0.0.1:10732. Read-only probe, never the DB file.",
            f,
        );
        widgets::section(ui, "Dev focus", Some(self.pit.repos().len()));
        ui.horizontal_wrapped(|ui| {
            if ui.button("Doctor").clicked() {
                f.run("Doctor", "devcraft_doctor", json!({}));
            }
            if ui.button("Recent 24h").clicked() {
                f.run("Recent", "devcraft_recent", json!({"hours": 24}));
            }
            if ui.button("Reveal selected folder").clicked() {
                if let Some(repo) = self.selected.clone() {
                    f.run("Revealed", "devcraft_reveal_repo", json!({"repo": repo}));
                } else {
                    f.toast("Select a checkout first", Tone::Warn);
                }
            }
            if ui.button("Focus selected here").clicked() {
                if let Some(repo) = self.selected.clone() {
                    f.run(
                        "Focused",
                        "devcraft_focus",
                        json!({"repo": repo, "holder": "opencode", "ide": "opencode"}),
                    );
                } else {
                    f.toast("Select a checkout first", Tone::Warn);
                }
            }
        });
        ui.add_space(8.0);
        widgets::section(ui, "Daemon", None);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Probe daemon").clicked() {
                match self.pit.call("devcraft_adn_status", &json!({})) {
                    Ok(v) => {
                        self.adn_out = serde_json::to_string_pretty(&v).unwrap_or_default();
                    }
                    Err(e) => {
                        self.adn_out = e;
                    }
                }
            }
            if ui.button("Copy endpoint").clicked() {
                ui.ctx().copy_text("http://127.0.0.1:10732/mcp".to_string());
            }
        });
        ui.label("Service: advanced-memory-mcp-daemon. Start it with NSSM when down; DevCraft never opens the DB directly.");
        ui.add_space(8.0);
        widgets::section(ui, "Search notes", None);
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.adn_query);
            if ui.button("Search").clicked() {
                let q = self.adn_query.clone();
                match self
                    .pit
                    .call("devcraft_adn_search", &json!({"query": q, "limit": 8}))
                {
                    Ok(v) => {
                        self.adn_out = serde_json::to_string_pretty(&v).unwrap_or_default();
                    }
                    Err(e) => {
                        self.adn_out = e;
                    }
                }
            }
        });
        if !self.adn_out.is_empty() {
            ui.add_space(4.0);
            ui.code(&self.adn_out);
        }
    }

    fn queue_build(&mut self, f: &mut Frame, profile: &str) {
        let Some(repo) = self.selected.clone() else {
            f.toast("Select a craft app first", Tone::Warn);
            return;
        };
        f.run(
            "Build",
            "devcraft_craft_build",
            json!({ "repo": repo, "profile": profile, "confirm": self.build_arm }),
        );
    }

    fn view_llm(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "LLM & Settings",
            "Cockpit skin engine, card frames, and local Ollama settings.",
            f,
        );
        skins::skin_settings_card(ui, &mut self.skin);
        ui.add_space(10.0);
        if self.chat.discover_rx.is_none() && self.chat.discovered.is_empty() {
            self.chat.kick_discover();
        }
        widgets::section(ui, "Installed Ollama models", None);
        if ui.button("Scan again").clicked() {
            self.chat.kick_discover();
        }
        if !self.chat.discover_note.is_empty() {
            ui.label(&self.chat.discover_note);
        }
        let mut picked = None;
        ui.horizontal_wrapped(|ui| {
            for model in self.chat.discovered.clone() {
                if ui
                    .selectable_label(self.chat.model == model, &model)
                    .clicked()
                {
                    picked = Some(model);
                }
            }
        });
        if let Some(model) = picked {
            self.chat.model = model;
            self.chat.save();
        }
        ui.label("Model");
        ui.text_edit_singleline(&mut self.chat.model);
        ui.label("Extra instructions");
        ui.text_edit_multiline(&mut self.chat.user_extra);
        ui.checkbox(&mut self.chat.tools_on, "Tools on");
        ui.checkbox(&mut self.chat.save_transcripts, "Save transcripts");
        if ui.button("Save").clicked() {
            self.chat.save();
        }
        if ui.button("Test connection").clicked() {
            self.chat.input = "Reply with the single word pong.".into();
            self.chat_send();
            self.chat.open = true;
        }
        ui.add_space(8.0);
        widgets::section(ui, "Agentic DE coop", None);
        ui.label("Cursor, Claude, and Antigravity call the same tools over stdio.");
        ui.label("devcraft-mcp --mcp");
        ui.label(".\\start.ps1 -Mcp");
        ui.label(".\\start.ps1 -Smoke");
    }

    fn handle_chat_turn(&mut self) {
        let Some(result) = self.chat.poll() else {
            return;
        };
        match result {
            Err(err) => self.chat.error = Some(err),
            Ok(turn) => {
                if !turn.content.is_empty() {
                    self.chat.messages.push(chat_dock::ChatMessage {
                        role: "assistant".into(),
                        content: turn.content,
                    });
                }
                if let Some(call) = turn.calls.into_iter().next() {
                    if self.chat.rounds >= 6 {
                        self.chat.error = Some("Stopped after 6 tool rounds.".into());
                        return;
                    }
                    if chat_dock::is_read_only(&call.name) {
                        self.run_chat_tool(call);
                    } else {
                        self.chat.pending = Some(call);
                    }
                }
            }
        }
    }

    fn chat_send(&mut self) {
        let text = self.chat.input.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.chat.input.clear();
        self.chat.rounds = 0;
        self.chat.pending = None;
        self.chat.messages.push(chat_dock::ChatMessage {
            role: "user".into(),
            content: text,
        });
        let status = self
            .pit
            .call("devcraft_status", &json!({}))
            .map(|value| value.to_string())
            .unwrap_or_default();
        let selection = self
            .selected
            .clone()
            .unwrap_or_else(|| "nothing selected".into());
        self.chat
            .refresh_prompt(self.version(), &status, &selection);
        self.chat.kick_turn();
    }

    fn chat_allow(&mut self) {
        let Some(call) = self.chat.pending.take() else {
            return;
        };
        self.run_chat_tool(call);
    }

    fn run_chat_tool(&mut self, call: chat_dock::PendingCall) {
        self.chat.rounds += 1;
        let result = match self.call(&call.name, &call.arguments) {
            Ok(value) => value.to_string(),
            Err(err) => err,
        };
        self.chat.messages.push(chat_dock::ChatMessage {
            role: "tool".into(),
            content: format!("{} {}", call.name, result),
        });
        self.chat.kick_turn();
    }

    fn card_visible(&self, card: &CardView, search: &str) -> bool {
        let holder = card.holder.clone().unwrap_or_default();
        if !app::search_hit(search, &[&card.title, &card.detail, &card.key, &holder]) {
            return false;
        }
        let Some(want) = &self.holder_filter else {
            return true;
        };
        if card.holder.as_deref() == Some(want.as_str()) {
            return true;
        }
        if let Some((parent, _)) = card.id.split_once('#') {
            return self.pit.repos().iter().any(|repo| {
                repo.id == parent && repo.claimed_by.as_deref() == Some(want.as_str())
            });
        }
        false
    }

    fn ensure_ops_view(&mut self) {
        if !self.pit.ops().ready && self.ops_rx.is_none() {
            let _ = self.kick_ops();
        }
    }

    fn view_tasks(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Tasks",
            "Scheduled fleet jobs and the report each one writes.",
            f,
        );
        self.ensure_ops_view();
        if ui.button("Read again").clicked() {
            f.run("Tasks", "devcraft_tasks", json!({}));
        }
        let snap = self.pit.ops().clone();
        if !snap.ready {
            widgets::empty_state(ui, "Reading tasks", "The scheduler query is running.");
            return;
        }
        if !snap.note.is_empty() {
            ui.label(&snap.note);
        }
        let cols = [
            Col::new("Task", 1.3),
            Col::new("State", 0.6),
            Col::new("Last", 0.9),
            Col::new("Result", 0.5),
            Col::new("Report", 0.5),
        ];
        let rows: Vec<(String, Vec<Cell>)> = snap
            .tasks
            .iter()
            .filter(|row| app::search_hit(&f.search, &[&row.name, &row.summary, &row.schedule]))
            .map(|row| {
                let tone = match row.result.as_str() {
                    "ok" => Tone::Ok,
                    "failed" => Tone::Danger,
                    _ => Tone::Neutral,
                };
                (
                    row.id.clone(),
                    vec![
                        Cell::Strong(row.name.clone()),
                        Cell::Text(row.state.clone()),
                        Cell::Weak(row.last_run.clone()),
                        Cell::Pill(row.result.clone(), tone),
                        Cell::Text(if row.report_present { "yes" } else { "no" }.into()),
                    ],
                )
            })
            .collect();
        self.show_table(ui, "tasks", &cols, &rows);
    }

    fn view_logs(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Logs",
            "Fleet gate, sync, NSSM, and MCP logs from Cursor, Claude, Codex, and OpenCode.",
            f,
        );
        self.ensure_ops_view();
        if ui.button("Read again").clicked() {
            f.run("Logs", "devcraft_logs", json!({}));
        }
        let snap = self.pit.ops().clone();
        if !snap.ready {
            widgets::empty_state(ui, "Reading logs", "The file list is still being built.");
            return;
        }
        let cols = [
            Col::new("Source", 0.7),
            Col::new("Bytes", 0.4),
            Col::new("Path", 2.2),
        ];
        let rows: Vec<(String, Vec<Cell>)> = snap
            .logs
            .iter()
            .filter(|file| app::search_hit(&f.search, &[&file.source, &file.path]))
            .map(|file| {
                (
                    file.id.clone(),
                    vec![
                        Cell::Text(file.source.clone()),
                        Cell::Mono(file.bytes.to_string()),
                        Cell::Weak(file.path.clone()),
                    ],
                )
            })
            .collect();
        if rows.is_empty() {
            widgets::empty_state(ui, "No logs", "None of the known log folders had a file.");
            return;
        }
        self.show_table(ui, "logs", &cols, &rows);
    }

    fn view_telemetry(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Telemetry",
            "The unified monitoring stack on ports 12000-12008.",
            f,
        );
        self.ensure_ops_view();
        if ui.button("Probe again").clicked() {
            f.run("Telemetry", "devcraft_telemetry", json!({}));
        }
        let snap = self.pit.ops().clone();
        if !snap.ready {
            widgets::empty_state(ui, "Probing", "Checking which ports answer.");
            return;
        }
        let up = snap.telemetry.iter().filter(|row| row.listening).count();
        widgets::kpi_row(
            ui,
            &[
                Kpi::new("Up", up.to_string()),
                Kpi::new("Down", (snap.telemetry.len() - up).to_string()),
            ],
        );
        let cols = [
            Col::new("Service", 1.0),
            Col::new("Port", 0.4),
            Col::new("Role", 1.0),
            Col::new("State", 0.5),
        ];
        let rows: Vec<(String, Vec<Cell>)> = snap
            .telemetry
            .iter()
            .map(|row| {
                (
                    row.id.clone(),
                    vec![
                        Cell::Strong(row.name.clone()),
                        Cell::Mono(row.port.to_string()),
                        Cell::Text(row.role.clone()),
                        Cell::Pill(
                            if row.listening { "up" } else { "down" }.into(),
                            if row.listening { Tone::Ok } else { Tone::Warn },
                        ),
                    ],
                )
            })
            .collect();
        self.show_table(ui, "telemetry", &cols, &rows);
    }

    fn detail_ops(&mut self, ui: &mut egui::Ui, f: &mut Frame, id: &str) -> bool {
        let snap = self.pit.ops().clone();
        if let Some(name) = id.strip_prefix("task:") {
            let Some(row) = snap.tasks.iter().find(|row| row.name == name).cloned() else {
                return false;
            };
            ui.heading(&row.name);
            ui.label(&row.summary);
            ui.label(format!(
                "{}  last {}  next {}",
                row.schedule, row.last_run, row.next_run
            ));
            if row.report_present {
                let shown = devcraft_core::ops::show_file(std::path::Path::new(&row.report_path));
                ui.label(&row.report_path);
                if ui.button("Open report").clicked() {
                    f.run(
                        "Opened",
                        "devcraft_reveal",
                        json!({ "path": row.report_path }),
                    );
                }
                if !shown.findings.is_empty() {
                    widgets::section(ui, "Findings", Some(shown.findings.len()));
                }
                ui.label(shown.excerpt);
            } else {
                ui.label("This task does not write a report file.");
            }
            return true;
        }
        if let Some(rest) = id.strip_prefix("log:") {
            let index = rest.parse::<usize>().unwrap_or(usize::MAX);
            let Some(file) = snap.logs.get(index).cloned() else {
                return false;
            };
            ui.heading(&file.source);
            ui.label(&file.path);
            let shown = devcraft_core::ops::show_file(std::path::Path::new(&file.path));
            if ui.button("Scan").clicked() {
                f.run("Scan", "devcraft_analyze", json!({ "id": id }));
            }
            if !shown.findings.is_empty() {
                widgets::section(ui, "Findings", Some(shown.findings.len()));
            }
            ui.label(shown.excerpt);
            return true;
        }
        if let Some(port) = id.strip_prefix("tele:") {
            let Some(row) = snap
                .telemetry
                .iter()
                .find(|row| row.port.to_string() == port)
                .cloned()
            else {
                return false;
            };
            ui.heading(&row.name);
            ui.label(format!("{}  {}", row.role, row.url));
            ui.label(if row.listening {
                "listening"
            } else {
                "not answering"
            });
            if ui.button("Open").clicked() {
                f.run("Opened", "devcraft_reveal", json!({ "path": row.url }));
            }
            return true;
        }
        false
    }

    fn detail_body(&mut self, ui: &mut egui::Ui, f: &mut Frame) -> bool {
        let Some(id) = self.selected.clone() else {
            return false;
        };
        if id.starts_with("task:") || id.starts_with("log:") || id.starts_with("tele:") {
            return self.detail_ops(ui, f, &id);
        }
        if let Some(line) = self.gh_line(&id) {
            ui.heading(if line.title.is_empty() {
                line.repo.clone()
            } else {
                line.title.clone()
            });
            ui.label(format!("{}  {}  #{}", line.repo, line.kind, line.number));
            if !line.url.is_empty() && ui.button("Open on GitHub").clicked() {
                f.run("Opened", "devcraft_reveal", json!({ "path": line.url }));
            }
            if id.starts_with("gh:") && ui.button("Snooze 24 hours").clicked() {
                f.run(
                    "Snoozed",
                    "devcraft_move",
                    json!({ "card": id, "to": "inbox" }),
                );
            }
            return true;
        }
        if let Some(repo) = id
            .strip_prefix("dirty:")
            .or_else(|| id.strip_prefix("drift:"))
        {
            ui.heading(repo);
            ui.label("From the morning digest. Refresh reads the checkout itself.");
            self.ide_row(ui, repo, f);
            return true;
        }
        if let Some((parent, name)) = id.split_once('#') {
            ui.heading(name);
            let branch = self
                .pit
                .repos()
                .iter()
                .find(|repo| repo.id == parent)
                .and_then(|repo| repo.worktrees.iter().find(|tree| tree.name == name))
                .map(|tree| tree.branch.clone())
                .unwrap_or_default();
            ui.label(format!("Worktree of {parent} on {branch}"));
            ui.label("It follows the checkout. A drag is refused.");
            return true;
        }
        let Some(facts) = self.pit.repos().iter().find(|r| r.id == id).cloned() else {
            if let Some(report) = self.pit.reports().iter().find(|r| r.id == id).cloned() {
                ui.heading(&report.title);
                ui.label(&report.path);
                return true;
            }
            if let Some(craft) = self.pit.crafts().iter().find(|c| c.name == id).cloned() {
                ui.heading(&craft.name);
                ui.label(&craft.tagline);
                ui.label(&craft.path);
                self.ide_row(ui, &craft.name, f);
                return true;
            }
            return false;
        };
        ui.heading(&facts.name);
        ui.label(format!("{}  |  {}", facts.branch, board::column(&facts)));
        if facts.handover_next.is_empty() {
            ui.label("No next step in HANDOVER.md.");
        } else {
            ui.label(&facts.handover_next);
        }
        if let Some(reason) = board::block_reason(&facts) {
            ui.label(reason);
        }
        self.ide_row(ui, &facts.id, f);
        if self.task_repo.as_deref() != Some(facts.id.as_str()) {
            self.task_edit = facts.task_key.clone().unwrap_or_default();
            self.task_repo = Some(facts.id.clone());
        }
        ui.label("TaskCraft key");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.task_edit);
            if ui.button("Set key").clicked() {
                f.run(
                    "Key set",
                    "devcraft_task",
                    json!({ "repo": facts.id, "key": self.task_edit }),
                );
            }
        });
        ui.label("A key such as BC-12. This does not create an issue.");
        if ui.button("Release").clicked() {
            f.run("Released", "devcraft_release", json!({ "repo": facts.id }));
        }
        if ui.button("Pin").clicked() {
            f.run("Pin", "devcraft_pin", json!({ "repo": facts.id }));
        }
        true
    }

    fn detail_upstream(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(id) = self.selected.clone() else {
            return false;
        };
        let Some(row) = self
            .pit
            .upstream()
            .iter()
            .find(|row| row.name == id)
            .cloned()
        else {
            return false;
        };
        ui.heading(&row.name);
        ui.label(format!("{}   {}   {}", row.head, row.date, row.subject));
        if !row.tag.is_empty() {
            ui.label(format!("Latest tag   {}", row.tag));
        }
        if row.delta.is_empty() {
            ui.label("No tree change against the checkout from 24 hours ago.");
        } else {
            ui.label(format!("Since 24 hours ago   {}", row.delta));
        }
        ui.add_space(6.0);
        ui.label("Commits in the last 24 hours");
        if row.subjects.is_empty() {
            ui.label("None.");
        }
        for subject in &row.subjects {
            ui.label(format!("- {subject}"));
        }
        ui.add_space(6.0);
        ui.label("Changelog");
        if row.changelog.is_empty() {
            ui.label("No CHANGELOG.md in this clone.");
        }
        for line in row.changelog.lines() {
            ui.label(line);
        }
        true
    }

    fn gh_line(&self, id: &str) -> Option<GhLine> {
        let (kind, rest) = id.split_once(':')?;
        let idx: usize = rest.parse().ok()?;
        let digest = self.pit.digest();
        let list = match kind {
            "gh" | "reply" => &digest.needs_reply,
            "stalepr" => &digest.stale_prs,
            "staleissue" => &digest.stale_issues,
            "qa" => &digest.unanswered,
            _ => return None,
        };
        list.get(idx).cloned()
    }

    fn ide_row(&mut self, ui: &mut egui::Ui, repo: &str, f: &mut Frame) {
        ui.horizontal(|ui| {
            for ide in ["cursor", "opencode", "agy", "claude"] {
                if ui.button(format!("Open {ide}")).clicked() {
                    f.run(
                        "Opened",
                        "devcraft_open",
                        json!({ "repo": repo, "ide": ide }),
                    );
                    f.run(
                        "Claimed",
                        "devcraft_claim",
                        json!({ "repo": repo, "holder": ide }),
                    );
                }
            }
        });
    }
}

fn changelog_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| line.starts_with('-') || line.starts_with('*'))
        .map(|line| line.trim_start_matches(['-', '*']).trim().to_string())
        .filter(|line| !line.is_empty())
        .unwrap_or_else(|| {
            text.lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or("")
                .to_string()
        })
}

fn opt_f(n: Option<u32>) -> f32 {
    n.unwrap_or(0) as f32
}

fn bars(ui: &mut egui::Ui, rows: &[(&str, f32, Tone)]) {
    let max = rows.iter().fold(1.0_f32, |hi, row| hi.max(row.1)).max(1.0);
    for (label, value, tone) in rows {
        ui.horizontal(|ui| {
            ui.set_min_width(168.0);
            ui.label(*label);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{}", *value as u32));
            });
        });
        widgets::meter(ui, *value / max, ui.available_width(), *tone);
        ui.add_space(3.0);
    }
}

fn age_bars(ui: &mut egui::Ui, repos: &[Facts]) {
    let bands = board::age_bands(repos);
    let rows = [
        ("Under a day", bands[0] as f32, Tone::Ok),
        ("1 to 7 days", bands[1] as f32, Tone::Info),
        ("8 to 30 days", bands[2] as f32, Tone::Warn),
        ("31 to 60 days", bands[3] as f32, Tone::Danger),
        ("Older", bands[4] as f32, Tone::Neutral),
    ];
    bars(ui, &rows);
}

fn holder_bars(ui: &mut egui::Ui, repos: &[Facts]) {
    let counts = board::holder_counts(repos);
    let rows = [
        ("Cursor", counts[0] as f32, Tone::Accent),
        ("OpenCode", counts[1] as f32, Tone::Info),
        ("Antigravity", counts[2] as f32, Tone::Warn),
        ("Claude", counts[3] as f32, Tone::Ok),
    ];
    bars(ui, &rows);
}

fn activity_bars(ui: &mut egui::Ui, repos: &[Facts]) {
    let days = board::activity(repos);
    let mut labels = Vec::new();
    for index in 0..14 {
        let label = if index == 13 {
            "today".to_string()
        } else {
            format!("{}d", 13 - index)
        };
        labels.push(label);
    }
    let rows: Vec<(&str, f32, Tone)> = labels
        .iter()
        .enumerate()
        .map(|(index, label)| (label.as_str(), days[index] as f32, Tone::Accent))
        .collect();
    bars(ui, &rows);
}

fn column_chart(
    ui: &mut egui::Ui,
    repos: &[Facts],
    selected: Option<&str>,
) -> Option<&'static str> {
    let names = [
        (board::READY, "Ready", Tone::Info),
        (board::ACTIVE, "Active", Tone::Accent),
        (board::REVIEW, "Review", Tone::Warn),
        (board::BLOCKED, "Blocked", Tone::Danger),
        (board::DONE, "Done today", Tone::Ok),
        (board::QUIET, "Quiet", Tone::Neutral),
    ];
    let counts: Vec<u32> = names
        .iter()
        .map(|(id, _, _)| {
            repos
                .iter()
                .filter(|repo| board::column(repo) == *id)
                .count() as u32
        })
        .collect();
    let max = counts.iter().copied().max().unwrap_or(1).max(1) as f32;
    let tokens = craft_biz_ui::theme::current(ui.ctx());
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 96.0), egui::Sense::hover());
    let gap = 8.0;
    let slot = ((rect.width() - gap * (names.len() as f32 - 1.0)) / names.len() as f32).max(48.0);
    let mut chosen = None;
    for (index, (id, label, tone)) in names.iter().enumerate() {
        let x = rect.left() + index as f32 * (slot + gap);
        let hit =
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(slot, rect.height()));
        let response = ui.interact(
            hit,
            ui.scope_id().with(("board-column", *id)),
            egui::Sense::click(),
        );
        let count = counts[index];
        let bar_h = (count as f32 / max) * 52.0;
        let bar = egui::Rect::from_min_size(
            egui::pos2(x, rect.bottom() - 22.0 - bar_h.max(2.0)),
            egui::vec2(slot, bar_h.max(2.0)),
        );
        let color = craft_biz_ui::theme::tone_solid(*tone, tokens.dark);
        ui.painter().rect_filled(
            bar,
            egui::CornerRadius::same(6),
            if selected == Some(*id) {
                color
            } else {
                color.gamma_multiply(0.72)
            },
        );
        if selected == Some(*id) || response.hovered() {
            ui.painter().rect_stroke(
                hit,
                egui::CornerRadius::same(8),
                egui::Stroke::new(1.5, color),
                egui::StrokeKind::Inside,
            );
        }
        ui.painter().text(
            egui::pos2(x + 4.0, rect.bottom() - 2.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{label}  {count}"),
            egui::FontId::proportional(12.0),
            tokens.text,
        );
        if response.clicked() {
            chosen = Some(*id);
        }
        let _ = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    chosen
}

fn cards_for_column(repos: &[Facts], column: &str) -> Vec<CardView> {
    let mut cards = Vec::new();
    for facts in repos {
        if board::column(facts) != column {
            continue;
        }
        let detail = if facts.handover_next.is_empty() {
            facts.branch.clone()
        } else {
            let mut text = facts.handover_next.clone();
            if text.len() > 90 {
                text.truncate(90);
            }
            text
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
    cards
}

fn column_bars(ui: &mut egui::Ui, repos: &[Facts]) {
    let names = [
        (board::READY, "Ready", Tone::Info),
        (board::ACTIVE, "Active", Tone::Accent),
        (board::REVIEW, "Review", Tone::Warn),
        (board::BLOCKED, "Blocked", Tone::Danger),
        (board::DONE, "Done today", Tone::Ok),
        (board::QUIET, "Quiet", Tone::Neutral),
    ];
    let rows: Vec<(&str, f32, Tone)> = names
        .iter()
        .map(|(id, label, tone)| {
            let n = repos
                .iter()
                .filter(|repo| board::column(repo) == *id)
                .count();
            (*label, n as f32, *tone)
        })
        .collect();
    bars(ui, &rows);
}

fn scan_thread(jobs: Vec<ScanJob>, tx: mpsc::Sender<Live>) {
    let now = SystemTime::now();
    for job in jobs {
        let facts = Cockpit::inspect(&job, now);
        if tx.send(Live::Repo(Box::new(facts))).is_err() {
            return;
        }
    }
    let _ = tx.send(Live::Done);
}

const GUIDE: &str = "\
DevCraft is the fleet cockpit.

The menu bar is File, Edit, View, Fleet, Window, Help.
View and Fleet both open the twenty pages. Help holds the command palette, shortcuts, these docs, licenses, and About.
Graphs draws commit weather, the code tide, languages, the heaviest checkouts, and a release spine. Measure reads git. It does not edit a checkout.
Window > Chat (Ctrl+J) opens the right-hand chat. Its prompt is the bundled skills plus the live status.
Workflows scans craft-app freshness and queues SOP requests. Memory probes the advanced-memory daemon (read-only) and hosts the dev-focus tools. LLM & Settings picks an Ollama model.
Tasks lists the scheduled fleet jobs. Selecting a row shows the report in the drawer.
Logs lists fleet, NSSM, and IDE MCP logs. The drawer shows the tail and the failure lines.
Telemetry probes Grafana and the rest of the monitoring stack on 12000-12008.
Upstream reads the Storytold clones under _upstream: commits and the diff from the last 24 hours, the newest tag, and the top of CHANGELOG.md. It does not fetch.
Refresh in the top bar reads every sibling checkout. The morning digest tables are already filled.
Drag a board card only where the column allows it. Blocked, Review, and Quiet are computed.
Open in the detail drawer starts Cursor, OpenCode, Antigravity, or Claude and claims the checkout.
Claims, pins, snoozes, and TaskCraft keys are stored in .devcraft at the workspace root.
A card key is a TaskCraft key such as BC-12. It does not create an issue.
A git worktree is a child card. It stays with its checkout, and a drag on it is refused.
";

fn to_card(view: &CardView) -> kanban::Card {
    let accent = if view.blocked.is_some() {
        Tone::Danger
    } else if view.craft {
        Tone::Accent
    } else if view.github {
        Tone::Warn
    } else {
        Tone::Neutral
    };
    kanban::Card {
        id: view.id.clone(),
        column: view.column.clone(),
        lane: String::new(),
        key: view.key.clone(),
        title: view.title.clone(),
        tags: if view.child {
            vec![("worktree".into(), Tone::Info)]
        } else {
            Vec::new()
        },
        accent,
        badge: view.holder.as_ref().map(|h| (h.clone(), Tone::Info)),
        assignee: view.holder.clone(),
        meta: vec![view.detail.clone()],
        blocked: view.blocked.clone(),
        done: view.column == board::DONE,
    }
}

fn git_row(facts: &Facts) -> (String, Vec<Cell>) {
    (
        facts.id.clone(),
        vec![
            Cell::Strong(facts.name.clone()),
            Cell::Mono(facts.branch.clone()),
            Cell::Text(facts.changes().to_string()),
            Cell::Text(facts.ahead.to_string()),
            Cell::Pill(board::column(facts).into(), Tone::Neutral),
            Cell::Text(facts.claimed_by.clone().unwrap_or_default()),
        ],
    )
}

fn tools_json() -> Vec<Value> {
    tool_specs()
}

#[cfg_attr(not(test), allow(dead_code))]
const COVERAGE: &[(&str, &str)] = &[
    ("devcraft_status", "Today page and smoke"),
    ("devcraft_refresh", "Page header and Fleet menu"),
    ("devcraft_board", "Board page"),
    ("devcraft_git", "Git page"),
    ("devcraft_digest", "GitHub page and Today"),
    ("devcraft_reports", "Reports page"),
    ("devcraft_crafts", "Craft apps page"),
    ("devcraft_upstream", "Upstream page"),
    ("devcraft_brief", "Detail drawer"),
    ("devcraft_claim", "Detail drawer, Open in IDE"),
    ("devcraft_release", "Detail drawer"),
    ("devcraft_release_all", "Claims page"),
    (
        "devcraft_release_stale_claims",
        "Claims page and Today console",
    ),
    ("devcraft_processes", "Services page process inspector"),
    (
        "devcraft_kill_stale_processes",
        "Services page stale MCP cleanup",
    ),
    ("devcraft_task", "Detail drawer, TaskCraft key"),
    ("devcraft_snapshots", "Time machine page"),
    ("devcraft_checkpoint", "Time machine page"),
    ("devcraft_sandbox", "Sandbox page"),
    ("devcraft_service", "Services page"),
    ("devcraft_restart", "Services page"),
    ("devcraft_pin", "Detail drawer"),
    ("devcraft_move", "Board drag and snooze"),
    ("devcraft_docker", "Docker page and Fleet menu"),
    ("devcraft_ports", "Ports page and Fleet menu"),
    ("devcraft_open", "Detail drawer"),
    ("devcraft_reveal", "Reports page"),
    ("devcraft_sandbox_run", "Sandbox page"),
    ("devcraft_checkpoint_repo", "Time machine page"),
    ("devcraft_craft_smoke", "Craft apps page"),
    ("devcraft_tasks", "Tasks page and Fleet menu"),
    ("devcraft_logs", "Logs page"),
    ("devcraft_telemetry", "Telemetry page"),
    ("devcraft_analyze", "Log and task detail"),
    ("devcraft_lock", "Ports page"),
    ("devcraft_skill_read", "Chat and Help docs"),
    ("devcraft_freshness", "Workflows page"),
    ("devcraft_graphs", "Graphs page"),
    ("devcraft_workflow", "Workflows page"),
    ("devcraft_craft_build", "Workflows page"),
    ("devcraft_todo_add", "Today You section"),
    ("devcraft_todo_list", "Today You section"),
    ("devcraft_todo_done", "Today You section"),
    ("devcraft_todo_clear", "Today You section"),
    ("devcraft_focus", "Memory page and detail drawer"),
    ("devcraft_find", "Memory page and command palette"),
    ("devcraft_recent", "Memory page"),
    ("devcraft_reveal_repo", "Memory page and detail drawer"),
    ("devcraft_doctor", "Memory page and Fleet menu"),
    ("devcraft_adn_status", "Memory page"),
    ("devcraft_adn_search", "Memory page"),
];

impl SuiteApp for DevApp {
    fn name(&self) -> &'static str {
        APP_NAME
    }
    fn id(&self) -> &'static str {
        "devcraft"
    }
    fn tone(&self) -> Tone {
        Tone::Accent
    }
    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
    fn tools(&self) -> Vec<Value> {
        tools_json()
    }
    fn call(&mut self, tool: &str, args: &Value) -> Result<Value, String> {
        match tool {
            "devcraft_refresh" => self.kick_scan(),
            "devcraft_docker" => self.kick_docker(),
            "devcraft_tasks" | "devcraft_logs" | "devcraft_telemetry" => self.kick_ops(),
            "devcraft_craft_smoke" => self.kick_smoke(args),
            "devcraft_sandbox_run" => self.kick_sandbox(args),
            "devcraft_freshness" => self.kick_freshness(),
            "devcraft_graphs" => self.kick_graphs(),
            "devcraft_craft_build" => self.kick_build(args),
            _ => self.pit.call(tool, args),
        }
    }
    fn reload(&mut self) -> Result<(), String> {
        self.pit.reload_light()
    }
    fn nav(&self, _f: &Frame) -> Vec<NavEntry> {
        let entries = self.nav_entries();
        debug_assert_eq!(entries.len(), VIEWS);
        debug_assert_eq!(V_TODAY, 0);
        entries
    }
    fn view(&mut self, ui: &mut egui::Ui, view: usize, f: &mut Frame) {
        self.skin.apply_to_ctx(ui.ctx());
        let palette = self.skin.palette();
        // The backdrop paints only inside this Ui (the central panel), behind
        // the page content. It must never reach the shell chrome, so the rect
        // comes from the panel, not the full window.
        let animate = self.skin.animate_backdrop && self.skin.backdrop.animated();
        let time = if animate {
            ui.ctx().input(|input| input.time) as f32
        } else {
            skins::FROZEN_BACKDROP_TIME
        };
        skins::paint_backdrop(
            ui,
            &palette,
            self.skin.backdrop,
            time,
            0.5,
            self.skin.backdrop_intensity,
        );
        if animate {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(120));
        }
        self.pump(ui.ctx());
        if self.chat.want_settings {
            self.chat.want_settings = false;
            f.goto(V_LLM);
        }
        if view == V_BOARD {
            self.view_board(ui, f);
        } else {
            ScrollArea::vertical().show(ui, |ui| match view {
                V_GRAPHS => self.view_graphs(ui, f),
                V_GIT => self.view_git(ui, f),
                V_GITHUB => self.view_github(ui, f),
                V_DOCKER => self.view_docker(ui, f),
                V_REPORTS => self.view_reports(ui, f),
                V_CRAFTS => self.view_crafts(ui, f),
                V_UPSTREAM => self.view_upstream(ui, f),
                V_CLAIMS => self.view_claims(ui, f),
                V_PORTS => self.view_ports(ui, f),
                V_TIME => self.view_time(ui, f),
                V_SANDBOX => self.view_sandbox(ui, f),
                V_SERVICE => self.view_service(ui, f),
                V_TASKS => self.view_tasks(ui, f),
                V_LOGS => self.view_logs(ui, f),
                V_TELEMETRY => self.view_telemetry(ui, f),
                V_WORKFLOWS => self.view_workflows(ui, f),
                V_MEMORY => self.view_memory(ui, f),
                V_LLM => self.view_llm(ui, f),
                _ => self.view_today(ui, f),
            });
        }
        let skills_count = devcraft_core::skills::skills().len();
        match self.welcome.draw(ui.ctx(), skills_count) {
            welcome::WelcomeAction::RefreshDigest => {
                f.goto(V_TODAY);
                f.run("Refreshing fleet digest", "devcraft_refresh", json!({}));
            }
            welcome::WelcomeAction::OpenBoard => {
                f.goto(V_BOARD);
            }
            welcome::WelcomeAction::OpenHelpCenter => {
                f.open_help(HelpTab::GetStarted);
            }
            welcome::WelcomeAction::EnterWorkspace | welcome::WelcomeAction::None => {}
        }
    }
    fn detail(&mut self, ui: &mut egui::Ui, view: usize, f: &mut Frame) -> bool {
        if view == V_UPSTREAM {
            return self.detail_upstream(ui);
        }
        self.detail_body(ui, f)
    }
    fn has_detail(&self, _view: usize) -> bool {
        self.selected.is_some()
    }
    fn bus(&self) -> &Bus {
        &self.bus
    }
    fn wiring(
        &self,
    ) -> (
        Vec<(&'static str, &'static str)>,
        Vec<(&'static str, &'static str)>,
    ) {
        (Vec::new(), Vec::new())
    }
    fn status_info(&self) -> String {
        if self.scan_rx.is_some() {
            return format!(
                "reading git {}/{} | {}",
                self.scan_done,
                self.scan_total,
                self.pit.root.display()
            );
        }
        let scan = if self.pit.scanned() {
            "git ready"
        } else {
            "git idle"
        };
        format!(
            "{scan} | {} claims | {} | {}",
            self.pit
                .repos()
                .iter()
                .filter(|r| r.claimed_by.is_some())
                .count(),
            self.pit.digest().generated,
            self.pit.root.display()
        )
    }
    fn smoke_tool(&self) -> &'static str {
        "devcraft_status"
    }
    fn menus(&self, _f: &Frame) -> AppMenus {
        AppMenus {
            file_new: vec![Item::run("Refresh fleet", "devcraft_refresh", json!({}))],
            export: vec![Item::run("Show board JSON", "devcraft_board", json!({}))],
            middle: vec![Menu::new(
                "Fleet",
                vec![
                    Item::view("Today", V_TODAY),
                    Item::view("Graphs", V_GRAPHS),
                    Item::view("Board", V_BOARD),
                    Item::view("Git", V_GIT),
                    Item::view("GitHub", V_GITHUB),
                    Item::view("Docker", V_DOCKER),
                    Item::view("Reports", V_REPORTS),
                    Item::view("Craft apps", V_CRAFTS),
                    Item::view("Upstream", V_UPSTREAM),
                    Item::view("Claims", V_CLAIMS),
                    Item::view("Ports", V_PORTS),
                    Item::view("Time machine", V_TIME),
                    Item::view("Sandbox", V_SANDBOX),
                    Item::view("Services", V_SERVICE),
                    Item::view("Tasks", V_TASKS),
                    Item::view("Logs", V_LOGS),
                    Item::view("Telemetry", V_TELEMETRY),
                    Item::view("Workflows", V_WORKFLOWS),
                    Item::view("Memory", V_MEMORY),
                    Item::view("LLM & Settings", V_LLM),
                    Item::sep(),
                    Item::run("Refresh fleet", "devcraft_refresh", json!({})),
                    Item::run("Probe Docker", "devcraft_docker", json!({})),
                    Item::run("Audit ports", "devcraft_ports", json!({})),
                    Item::run("Morning digest", "devcraft_digest", json!({})),
                ],
            )],
            window: vec![Item::action("Chat", "devcraft.chat")
                .key(egui::Modifiers::COMMAND, egui::Key::J)
                .checked(self.chat.open)],
            help: vec![Item::action("Welcome Screen...", "devcraft.welcome")],
            ..AppMenus::default()
        }
    }
    fn menu_action(&mut self, id: &str, _f: &mut Frame) {
        match id {
            "devcraft.chat" => self.chat.open = !self.chat.open,
            "devcraft.welcome" => self.welcome.open = true,
            _ => {}
        }
    }
    fn preferences_view(&self) -> Option<usize> {
        Some(V_LLM)
    }
    fn chat_panel(&mut self, ui: &mut egui::Ui) {
        self.handle_chat_turn();
        if !self.chat.open {
            return;
        }
        let ctx = ui.ctx().clone();
        let status = self
            .pit
            .call("devcraft_status", &json!({}))
            .map(|v| v.to_string())
            .unwrap_or_default();
        let selection = self
            .selected
            .clone()
            .unwrap_or_else(|| "nothing selected".into());
        self.chat
            .refresh_prompt(self.version(), &status, &selection);
        let mut header_action = chat_dock::HeaderAction::None;
        let mut transcript_action = chat_dock::TranscriptAction::None;
        let mut send = false;
        let half = ui.ctx().content_rect().width() * 0.5;
        let max = if self.chat.maximized {
            half.clamp(480.0, 960.0)
        } else {
            480.0
        };
        let width = if self.chat.maximized { max } else { 360.0 };
        egui::Panel::right("chat_dock")
            .resizable(true)
            .default_size(width)
            .min_size(280.0)
            .max_size(max)
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                header_action = self.chat.header(ui);
                transcript_action = self.chat.transcript(ui);
                send = self.chat.input_bar(ui);
            });
        if send {
            self.chat_send();
        }
        match header_action {
            chat_dock::HeaderAction::Settings => self.chat.want_settings = true,
            chat_dock::HeaderAction::Clear => {
                self.chat.messages.clear();
                self.chat.pending = None;
                self.chat.error = None;
            }
            chat_dock::HeaderAction::None => {}
        }
        match transcript_action {
            chat_dock::TranscriptAction::Allow => self.chat_allow(),
            chat_dock::TranscriptAction::Deny => {
                self.chat.pending = None;
                self.chat.messages.push(chat_dock::ChatMessage {
                    role: "assistant".into(),
                    content: "Denied. Nothing changed.".into(),
                });
            }
            chat_dock::TranscriptAction::Settings => self.chat.want_settings = true,
            chat_dock::TranscriptAction::Cancel => {
                self.chat
                    .cancel
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            chat_dock::TranscriptAction::None => {}
        }
        if self.chat.busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
    }
    fn info(&self) -> AppInfo {
        AppInfo {
            name: APP_NAME.into(),
            version: self.version().into(),
            description: "Fleet cockpit for one developer and four IDEs.".into(),
            license: "MIT OR Apache-2.0".into(),
            repository: "https://github.com/sandraschi/devcraft".into(),
            commit: option_env!("GIT_COMMIT").unwrap_or("unknown").into(),
            built: option_env!("BUILD_DATE").unwrap_or("").into(),
            safety: "Pure Rust. The window calls the core in-process. MCP is stdio.".into(),
            third_party: Vec::new(),
        }
    }
    fn top_actions(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        skins::skin_toolbar_controls(ui, &mut self.skin);
        if ui
            .add(egui::Button::new("Chat").selected(self.chat.open))
            .on_hover_text("Open the chat dock (also Window > Chat, Ctrl+J)")
            .clicked()
        {
            self.chat.open = !self.chat.open;
        }
        if ui.button("Refresh").clicked() {
            f.run("Scanning the fleet", "devcraft_refresh", json!({}));
        }
    }
    fn help_md(&self) -> Option<&'static str> {
        Some(include_str!("../../../HELP.md"))
    }
    fn help_skills(&self) -> Vec<Doc> {
        devcraft_core::skills::skills()
            .iter()
            .map(|skill| {
                Doc::embedded(
                    skill.name,
                    PathBuf::from(format!("skills/{}/SKILL.md", skill.name)),
                    skill.body,
                )
            })
            .collect()
    }
    fn docs(&self) -> Vec<Doc> {
        vec![
            Doc::embedded(
                "HELP.md",
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HELP.md"),
                include_str!("../../../HELP.md"),
            ),
            Doc::embedded("Using DevCraft", PathBuf::from("using-devcraft.md"), GUIDE),
            Doc::embedded(
                "README",
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../README.md"),
                include_str!("../../../README.md"),
            ),
            Doc::embedded(
                "PRD",
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/PRD.md"),
                include_str!("../../../docs/PRD.md"),
            ),
            Doc::embedded(
                "Tier Plan",
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/TIERED_PLAN.md"),
                include_str!("../../../docs/TIERED_PLAN.md"),
            ),
            Doc::embedded(
                "Skill: workflow",
                PathBuf::from("skills/devcraft-workflow/SKILL.md"),
                include_str!("../../../skills/devcraft-workflow/SKILL.md"),
            ),
            Doc::embedded(
                "Skill: sops",
                PathBuf::from("skills/devcraft-sops/SKILL.md"),
                include_str!("../../../skills/devcraft-sops/SKILL.md"),
            ),
            Doc::embedded(
                "Changelog",
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md"),
                include_str!("../../../CHANGELOG.md"),
            ),
        ]
    }
    fn copy_text(&self, _view: usize) -> Option<String> {
        self.selected.clone()
    }
    fn save(&mut self) -> Result<String, String> {
        Ok("claims, pins, and snoozes are written when they change".into())
    }
}

fn main() -> std::process::ExitCode {
    use std::process::ExitCode;
    let args = match app::Args::parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return if e == app::HELP {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
    };
    let db = args.db.clone().unwrap_or_else(settings_db);
    if let Some(parent) = db.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let settings_file = shell::settings_path(&db);
    if !settings_file.exists() {
        let prefs = UiSettings {
            lang: "en".into(),
            ..UiSettings::default()
        };
        let _ = prefs.save(&settings_file);
    }
    let application = match DevApp::open() {
        Ok(app) => app,
        Err(e) => {
            eprintln!("cannot open cockpit: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut sh = app::Shell::new(application, Some(settings_file)).with_args(&args);
    if args.smoke {
        return match sh.smoke() {
            Ok(v) => {
                println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("smoke: {e}");
                ExitCode::FAILURE
            }
        };
    }
    match sh.run(args.size) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn yes_no(flag: bool) -> String {
    if flag {
        "yes".into()
    } else {
        "no".into()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_a_place_in_the_window() {
        craft_biz_ui::coverage::check(&tools_json(), COVERAGE).unwrap();
    }

    #[test]
    fn chat_menu_toggles_the_dock() {
        let mut app = today_app();
        let mut frame = Frame::idle();
        let menus = app.menus(&frame);
        assert!(menus.window.iter().any(|item| {
            item.label == "Chat"
                && matches!(
                    &item.kind,
                    craft_biz_ui::menu::Kind::Action(id) if id == "devcraft.chat"
                )
        }));
        assert!(!app.chat.open);
        app.menu_action("devcraft.chat", &mut frame);
        assert!(app.chat.open);
        let open = app.menus(&frame);
        assert_eq!(open.window[0].checked, Some(true));
        app.menu_action("devcraft.chat", &mut frame);
        assert!(!app.chat.open);
    }

    #[test]
    fn help_menu_opens_welcome_screen() {
        let mut app = today_app();
        let mut frame = Frame::idle();
        let menus = app.menus(&frame);
        assert!(menus.help.iter().any(|item| {
            item.label == "Welcome Screen..."
                && matches!(
                    &item.kind,
                    craft_biz_ui::menu::Kind::Action(id) if id == "devcraft.welcome"
                )
        }));
        app.welcome.open = false;
        app.menu_action("devcraft.welcome", &mut frame);
        assert!(app.welcome.open);
    }

    #[test]
    fn help_center_has_sections_skills_and_docs() {
        let app = today_app();
        let md = app.help_md().expect("HELP.md must be embedded");
        let parsed = craft_biz_ui::help::parse_help_md(md);
        assert!(!parsed.get_started.is_empty());
        assert!(!parsed.functions.is_empty());
        assert!(!parsed.usage.is_empty());
        assert!(!parsed.tips_tricks.is_empty());
        assert!(!parsed.pitfalls_safety.is_empty());
        assert!(!parsed.planned_extensions.is_empty());
        assert!(!parsed.other_crafts.is_empty());

        let skills = app.help_skills();
        assert_eq!(skills.len(), 2);
        assert!(app.docs().iter().any(|d| d.title == "HELP.md"));
    }

    #[test]
    fn nineteen_pages() {
        assert_eq!(VIEWS, 20);
        assert_eq!(V_TODAY, 0);
        assert_eq!(V_GRAPHS, 1);
        assert_eq!(V_UPSTREAM, 8);
        assert_eq!(V_PORTS, 10);
        assert_eq!(V_TIME, 11);
        assert_eq!(V_SANDBOX, 12);
        assert_eq!(V_SERVICE, 13);
        assert_eq!(V_TASKS, 14);
        assert_eq!(V_LOGS, 15);
        assert_eq!(V_TELEMETRY, 16);
        assert_eq!(V_WORKFLOWS, 17);
        assert_eq!(V_LLM, 18);
        assert_eq!(V_MEMORY, 19);
    }

    fn today_app() -> DevApp {
        let root =
            std::env::temp_dir().join(format!("devcraft-today-golden-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let docs = root.join("mcp-central-docs").join("operations");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(
            docs.join("fleet-status.md"),
            "\
Generated: 2026-11-14T05:08:49+00:00
Repos scanned: 2
- Open PRs: **3**
- Open issues: **1**
- Stale PRs (14d): **0**
- Stale issues: **0**
- Open discussions: **0**
- Unanswered: **0**
- Dirty worktrees (uncommitted/untracked): **0**
- Sync drift: **0**
",
        )
        .unwrap();
        std::env::set_var("DEVCRAFT_ROOT", &root);
        let mut app = DevApp::open().unwrap();
        std::env::remove_var("DEVCRAFT_ROOT");
        // Animated backdrops use wall-clock time, so screenshots pin Solid.
        app.skin.backdrop = skins::BackdropStyle::Solid;
        app
    }

    fn snap_today(dark: bool, name: &str) -> egui_kittest::Harness<'static, DevApp> {
        let app = today_app();
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1100.0, 1680.0))
            .with_pixels_per_point(1.0)
            .with_theme(if dark {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            })
            .with_accessibility_check(false)
            .build_ui_state(
                move |ui, app: &mut DevApp| {
                    craft_biz_ui::theme::install_fonts(ui.ctx());
                    ui.ctx().set_visuals(craft_biz_ui::theme::visuals(dark));
                    let mut frame = Frame::idle();
                    app.view_today(ui, &mut frame);
                },
                app,
            );
        harness.snapshot(name);
        harness
    }

    #[test]
    fn today_golden_light_and_dark() {
        let mut light = snap_today(false, "today-light");
        let mut dark = snap_today(true, "today-dark");
        let mut results = light.take_snapshot_results();
        results.extend_harness(&mut dark);
    }
}
