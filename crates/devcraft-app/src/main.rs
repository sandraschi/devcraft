//! DevCraft window: fleet board on the shared craft shell.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use craft_biz::bus::Bus;
use craft_biz_ui::app::{self, AppMenus, Frame, NavEntry, SuiteApp};
use craft_biz_ui::help::{AppInfo, Doc};
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
const V_BOARD: usize = 1;
const V_GIT: usize = 2;
const V_GITHUB: usize = 3;
const V_DOCKER: usize = 4;
const V_REPORTS: usize = 5;
const V_CRAFTS: usize = 6;
const V_UPSTREAM: usize = 7;
const V_CLAIMS: usize = 8;
const V_PORTS: usize = 9;
const V_TIME: usize = 10;
const V_SANDBOX: usize = 11;
const V_SERVICE: usize = 12;
const VIEWS: usize = 13;

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
        })
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
        if self.scan_rx.is_some() || self.docker_rx.is_some() {
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
            NavEntry::new("\u{23F1}", "Time machine").group("Host"),
            NavEntry::new("\u{25A1}", "Sandbox").group("Host"),
            NavEntry::new("\u{21BB}", "Services").group("Host"),
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
        let d = self.pit.digest().clone();
        let note = if d.present {
            d.generated.clone()
        } else {
            d.note.clone()
        };
        let tone = if d.present { Tone::Ok } else { Tone::Warn };
        widgets::kpi_row(
            ui,
            &[
                Kpi::new("Open PRs", num(d.open_prs)).note(note, tone),
                Kpi::new("Open issues", num(d.open_issues)),
                Kpi::new("Dirty trees", num(d.dirty_worktrees))
                    .note(format!("{} listed", d.dirty.len()), Tone::Warn),
                Kpi::new("Sync drift", num(d.sync_drift)),
                Kpi::new("Needs a reply", d.needs_reply.len().to_string())
                    .note("first reply", Tone::Danger),
                Kpi::new("Stale PRs", num(d.stale_pr_count)),
                Kpi::new("Craft apps", self.pit.crafts().len().to_string()),
                Kpi::new("On the board", self.pit.cards().len().to_string()),
            ],
        );
        ui.add_space(6.0);
        ui.columns(2, |cols| {
            cols[0].vertical(|ui| {
                widgets::section(ui, "This morning", None);
                widgets::card(ui, |ui| {
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
                widgets::card(ui, |ui| {
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
                widgets::card(ui, |ui| age_bars(ui, self.pit.repos()));
            });
            cols[1].vertical(|ui| {
                widgets::section(ui, "Holders", None);
                widgets::card(ui, |ui| holder_bars(ui, self.pit.repos()));
            });
        });
        ui.add_space(8.0);
        widgets::section(ui, "14 days", None);
        widgets::card(ui, |ui| activity_bars(ui, self.pit.repos()));
        ui.add_space(8.0);
        self.gh_section(ui, f, "Needs a first reply", "reply", &d.needs_reply);
        self.dirty_section(ui, f, &d);
        self.drift_section(ui, f, &d.drift);
    }

    fn view_board(&mut self, ui: &mut egui::Ui, f: &mut Frame) {
        self.header(
            ui,
            "Board",
            "Inbox, Ready, Active, Review, Blocked, Done, Quiet. Drag is a command.",
            f,
        );
        self.progress(ui);
        if !self.pit.repos().is_empty() {
            widgets::card(ui, |ui| column_bars(ui, self.pit.repos()));
            ui.add_space(8.0);
        }
        let columns = [
            Column::new(board::INBOX, "Inbox", Tone::Danger),
            Column::new(board::READY, "Ready", Tone::Info),
            Column::new(board::ACTIVE, "Active", Tone::Accent).wip(3),
            Column::new(board::REVIEW, "Review", Tone::Warn),
            Column::new(board::BLOCKED, "Blocked", Tone::Danger),
            Column::new(board::DONE, "Done today", Tone::Ok),
            Column::new(board::QUIET, "Quiet", Tone::Neutral),
        ];
        let views = self.pit.cards();
        let cards: Vec<kanban::Card> = views.iter().map(to_card).collect();
        let rule = |card: &kanban::Card, to: &str| {
            if card.id.contains('#') {
                return Err("A worktree follows its checkout.".into());
            }
            board::allow(to, card.id.starts_with("gh:")).map(|_| ())
        };
        let events = kanban::Board::new("fleet", &columns, &cards)
            .add_buttons(false)
            .rule(&rule)
            .selected(self.selected.as_deref())
            .show(ui);
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
            .filter(|r| r.has_git && app::search_hit(&f.search, &[&r.name, &r.branch]))
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
        });
        ui.add_space(6.0);
        let cols = [
            Col::new("App", 1.0),
            Col::new("Version", 0.5),
            Col::new("Binary", 0.5),
            Col::new("Handover", 0.6),
            Col::new("Tagline", 2.0),
        ];
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
            "Who holds a checkout: cursor, opencode, agy, claude.",
            f,
        );
        let cols = [
            Col::new("Repo", 1.2),
            Col::new("Holder", 0.7),
            Col::new("Column", 0.7),
        ];
        let rows: Vec<(String, Vec<Cell>)> = self
            .pit
            .repos()
            .iter()
            .filter(|r| r.claimed_by.is_some() || r.pinned)
            .map(|r| {
                (
                    r.id.clone(),
                    vec![
                        Cell::Strong(r.name.clone()),
                        Cell::Text(r.claimed_by.clone().unwrap_or_else(|| "pinned".into())),
                        Cell::Pill(board::column(r).into(), Tone::Accent),
                    ],
                )
            })
            .collect();
        if widgets::primary(ui, "Release every claim").clicked() {
            f.run("Claims released", "devcraft_release_all", json!({}));
        }
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
            "Snapshots of claims, pins, and snoozes under .devcraft.",
            f,
        );
        if widgets::primary(ui, "Snapshot cockpit state").clicked() {
            f.run("Snapshot written", "devcraft_checkpoint", json!({}));
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
            "Docker is the engine. The page only asks whether the daemon answers.",
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

    fn detail_body(&mut self, ui: &mut egui::Ui, f: &mut Frame) -> bool {
        let Some(id) = self.selected.clone() else {
            return false;
        };
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
View and Fleet both open the thirteen pages. Help holds the command palette, shortcuts, these docs, licenses, and About.
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
        self.pump(ui.ctx());
        ScrollArea::vertical().show(ui, |ui| match view {
            V_BOARD => self.view_board(ui, f),
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
            _ => self.view_today(ui, f),
        });
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
                    Item::sep(),
                    Item::run("Refresh fleet", "devcraft_refresh", json!({})),
                    Item::run("Probe Docker", "devcraft_docker", json!({})),
                    Item::run("Audit ports", "devcraft_ports", json!({})),
                    Item::run("Morning digest", "devcraft_digest", json!({})),
                ],
            )],
            ..AppMenus::default()
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
        if ui.button("Refresh").clicked() {
            f.run("Scanning the fleet", "devcraft_refresh", json!({}));
        }
    }
    fn docs(&self) -> Vec<Doc> {
        vec![
            Doc::embedded("Using DevCraft", PathBuf::from("using-devcraft.md"), GUIDE),
            Doc::embedded(
                "README",
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../README.md"),
                include_str!("../../../README.md"),
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_a_place_in_the_window() {
        craft_biz_ui::coverage::check(&tools_json(), COVERAGE).unwrap();
    }

    #[test]
    fn thirteen_pages() {
        assert_eq!(VIEWS, 13);
        assert_eq!(V_TODAY, 0);
        assert_eq!(V_UPSTREAM, 7);
        assert_eq!(V_PORTS, 9);
        assert_eq!(V_TIME, 10);
        assert_eq!(V_SANDBOX, 11);
        assert_eq!(V_SERVICE, 12);
    }

    fn today_app() -> DevApp {
        let root = std::env::temp_dir().join("devcraft-today-golden");
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
        let app = DevApp::open().unwrap();
        std::env::remove_var("DEVCRAFT_ROOT");
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
