//! DevCraft Desktop Studio: Native egui 0.37 interface for fleet hygiene and developer cockpit.

use clap::Parser;
use devcraft_core::{
    backup::{BackupEngine, SnapshotRecord},
    fleet::{FleetAuditReport, FleetScanner, RepoInfo},
    ports::{PortAuditReport, PortSentinel},
    resolve_workspace_root,
    sandbox::{ContainerSandbox, SandboxExecutionResult},
    services::{RestartResult, ServiceSentinel, ServiceStatus},
    APP_VERSION, CONTROL_PORT, FRONTEND_PORT,
};
use eframe::egui;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "devcraft", about = "DevCraft Desktop Studio", version = APP_VERSION)]
struct Args {
    /// Headless smoke test verification
    #[arg(long)]
    smoke: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Overview,
    Snapshots,
    Sandbox,
    PortSentinel,
    Services,
}

struct DevCraftApp {
    workspace_root: PathBuf,
    fleet_report: FleetAuditReport,
    selected_repo: Option<RepoInfo>,
    repo_filter: String,
    active_tab: ActiveTab,
    snapshots: Vec<SnapshotRecord>,
    port_report: PortAuditReport,
    sandbox_cmd: String,
    sandbox_image: String,
    sandbox_result: Option<SandboxExecutionResult>,
    service_query_name: String,
    service_status: Option<ServiceStatus>,
    restart_result: Option<RestartResult>,
    status_message: String,
}

impl DevCraftApp {
    fn new(workspace_root: PathBuf) -> Self {
        let fleet_report = FleetScanner::scan_workspace(&workspace_root);
        let port_report = PortSentinel::audit_ports(&workspace_root);
        let snapshots = BackupEngine::list_snapshots(&workspace_root);
        let selected_repo = fleet_report.repos.first().cloned();

        Self {
            workspace_root,
            fleet_report,
            selected_repo,
            repo_filter: String::new(),
            active_tab: ActiveTab::Overview,
            snapshots,
            port_report,
            sandbox_cmd: "cargo check".to_string(),
            sandbox_image: "alpine:latest".to_string(),
            sandbox_result: None,
            service_query_name: "meta_mcp".to_string(),
            service_status: None,
            restart_result: None,
            status_message: "DevCraft initialized. All fleet systems ready.".to_string(),
        }
    }

    fn refresh_fleet(&mut self) {
        self.fleet_report = FleetScanner::scan_workspace(&self.workspace_root);
        self.port_report = PortSentinel::audit_ports(&self.workspace_root);
        self.snapshots = BackupEngine::list_snapshots(&self.workspace_root);
        self.status_message = format!(
            "Fleet refreshed: {} repos scanned ({} dirty).",
            self.fleet_report.total_scanned, self.fleet_report.dirty_repos
        );
    }
}

impl eframe::App for DevCraftApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Top Menu / Status Bar
        egui::Panel::top("top_panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(format!("DevCraft v{}", APP_VERSION));
                ui.separator();
                ui.label(format!("Frontend: {}", FRONTEND_PORT));
                ui.label(format!("Control/MCP: {}", CONTROL_PORT));
                ui.separator();
                ui.label(format!("Fleet Repos: {}", self.fleet_report.total_scanned));
                if self.fleet_report.dirty_repos > 0 {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 170, 0),
                        format!("Dirty: {}", self.fleet_report.dirty_repos),
                    );
                } else {
                    ui.colored_label(egui::Color32::from_rgb(100, 220, 100), "Clean");
                }
                ui.separator();
                if ui.button("Refresh Fleet").clicked() {
                    self.refresh_fleet();
                }
            });
        });

        // Bottom Status Bar
        egui::Panel::bottom("bottom_panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
            });
        });

        // Left Panel: Fleet Radar
        egui::Panel::left("left_panel").show(ui, |ui| {
            ui.set_width(280.0);
            ui.heading("Fleet Radar");
            ui.text_edit_singleline(&mut self.repo_filter);
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                let filter = self.repo_filter.to_lowercase();
                for repo in &self.fleet_report.repos {
                    if !filter.is_empty() && !repo.name.to_lowercase().contains(&filter) {
                        continue;
                    }

                    let is_selected = self
                        .selected_repo
                        .as_ref()
                        .map(|r| r.name == repo.name)
                        .unwrap_or(false);

                    let label = if repo.dirty_count > 0 || repo.untracked_count > 0 {
                        format!("* {} (+{})", repo.name, repo.dirty_count + repo.untracked_count)
                    } else {
                        format!("  {}", repo.name)
                    };

                    if ui.selectable_label(is_selected, label).clicked() {
                        self.selected_repo = Some(repo.clone());
                    }
                }
            });
        });

        // Central Panel
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, ActiveTab::Overview, "Selected Repo");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Snapshots, "Time Machine");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Sandbox, "Container Sandbox");
                ui.selectable_value(&mut self.active_tab, ActiveTab::PortSentinel, "Port Sentinel");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Services, "Service Watchdog");
            });
            ui.separator();

            match self.active_tab {
                ActiveTab::Overview => {
                    if let Some(ref repo) = self.selected_repo {
                        ui.heading(&repo.name);
                        ui.label(format!("Path: {}", repo.path.to_string_lossy()));
                        ui.label(format!("Branch: {}", repo.branch));
                        ui.label(format!("Dirty Files: {}", repo.dirty_count));
                        ui.label(format!("Untracked Files: {}", repo.untracked_count));
                        ui.label(format!("Has AGENTS.md: {}", repo.has_agents_md));
                        ui.label(format!("Has CLAUDE.md: {}", repo.has_claude_md));

                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui.button("Create Snapshot Checkpoint").clicked() {
                                let target_file = repo.path.join("Cargo.toml");
                                let files = if target_file.exists() {
                                    vec![target_file]
                                } else {
                                    vec![]
                                };
                                match BackupEngine::create_snapshot(
                                    &self.workspace_root,
                                    &files,
                                    &repo.name,
                                    "Manual UI checkpoint",
                                ) {
                                    Ok(rec) => {
                                        self.status_message = format!("Snapshot created: {}", rec.id);
                                        self.snapshots = BackupEngine::list_snapshots(&self.workspace_root);
                                    }
                                    Err(e) => {
                                        self.status_message = format!("Snapshot failed: {}", e);
                                    }
                                }
                            }
                        });
                    } else {
                        ui.label("Select a repository from the Fleet Radar.");
                    }
                }
                ActiveTab::Snapshots => {
                    ui.heading("Time Machine Snapshots");
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if self.snapshots.is_empty() {
                            ui.label("No snapshots recorded yet.");
                        }
                        for snap in &self.snapshots {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.strong(&snap.id);
                                    ui.label(format!("[{}]", snap.tag));
                                });
                                ui.label(&snap.description);
                                ui.label(format!("Files ({}): {:?}", snap.backed_up_files.len(), snap.backed_up_files));
                            });
                        }
                    });
                }
                ActiveTab::Sandbox => {
                    ui.heading("Container Sandbox Execution");
                    ui.horizontal(|ui| {
                        ui.label("Image:");
                        ui.text_edit_singleline(&mut self.sandbox_image);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Command:");
                        ui.text_edit_singleline(&mut self.sandbox_cmd);
                    });

                    if ui.button("Run in Sandbox").clicked() {
                        let path = self
                            .selected_repo
                            .as_ref()
                            .map(|r| r.path.clone())
                            .unwrap_or_else(|| self.workspace_root.clone());

                        match ContainerSandbox::execute(&path, Some(&self.sandbox_image), &self.sandbox_cmd) {
                            Ok(res) => {
                                self.status_message = format!("Execution finished (code {})", res.exit_code);
                                self.sandbox_result = Some(res);
                            }
                            Err(e) => {
                                self.status_message = format!("Execution failed: {}", e);
                            }
                        }
                    }

                    if let Some(ref res) = self.sandbox_result {
                        ui.separator();
                        ui.label(format!("Engine: {} | Exit Code: {} | Duration: {}ms", res.engine, res.exit_code, res.duration_ms));
                        ui.label("STDOUT:");
                        ui.code(&res.stdout);
                        if !res.stderr.is_empty() {
                            ui.label("STDERR:");
                            ui.code(&res.stderr);
                        }
                    }
                }
                ActiveTab::PortSentinel => {
                    ui.heading("Port Sentinel (WEBAPP_PORTS.md)");
                    ui.label(format!("Registered: {} | Active Listeners: {}", self.port_report.total_registered, self.port_report.active_listeners));
                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("ports_grid").striped(true).show(ui, |ui| {
                            ui.strong("Port");
                            ui.strong("Repo");
                            ui.strong("Status");
                            ui.strong("Description");
                            ui.end_row();

                            for entry in &self.port_report.entries {
                                ui.label(entry.port.to_string());
                                ui.label(&entry.repo);
                                if entry.is_listening {
                                    ui.colored_label(egui::Color32::from_rgb(50, 200, 50), "LISTENING");
                                } else {
                                    ui.label("idle");
                                }
                                ui.label(&entry.description);
                                ui.end_row();
                            }
                        });
                    });
                }
                ActiveTab::Services => {
                    ui.heading("Windows NSSM Service Watchdog");
                    ui.horizontal(|ui| {
                        ui.label("Service Name:");
                        ui.text_edit_singleline(&mut self.service_query_name);
                        if ui.button("Query Status").clicked() {
                            self.service_status = Some(ServiceSentinel::query_service(&self.service_query_name));
                        }
                        if ui.button("Safe Restart").clicked() {
                            self.restart_result = Some(ServiceSentinel::restart_service_safely(&self.service_query_name, None));
                        }
                    });

                    if let Some(ref st) = self.service_status {
                        ui.group(|ui| {
                            ui.label(format!("Service: {}", st.name));
                            ui.label(format!("State: {}", st.state));
                            ui.label(format!("Running: {}", st.is_running));
                        });
                    }

                    if let Some(ref res) = self.restart_result {
                        ui.group(|ui| {
                            ui.label(format!("Service: {}", res.service_name));
                            ui.label(format!("Stop OK: {}", res.stop_success));
                            ui.label(format!("Start OK: {}", res.start_success));
                            ui.label(format!("Final State: {}", res.final_state));
                        });
                    }
                }
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let args = Args::parse();

    if args.smoke {
        println!("DevCraft App smoke test: OK (v{})", APP_VERSION);
        return Ok(());
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title(format!("DevCraft — Sovereign Developer Cockpit (v{})", APP_VERSION)),
        ..Default::default()
    };

    let workspace_root = resolve_workspace_root();
    eframe::run_native(
        "DevCraft Studio",
        native_options,
        Box::new(move |_cc| Ok(Box::new(DevCraftApp::new(workspace_root)))),
    )
}
