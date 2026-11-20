//! First-time user Welcome Screen ("3-3-1" studio pattern):
//! 1-line identity, 3 visual 1-click starter cards, 3 readiness pills,
//! persisted startup preference, and vector fleet radar / node graph emblem.

use egui::{Color32, Pos2, Stroke, Vec2};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WelcomeAction {
    None,
    RefreshDigest,
    OpenBoard,
    OpenHelpCenter,
    EnterWorkspace,
}

pub struct WelcomeState {
    pub open: bool,
    pub show_on_startup: bool,
}

impl Default for WelcomeState {
    fn default() -> Self {
        let show = should_show_on_startup();
        Self {
            open: show,
            show_on_startup: show,
        }
    }
}

impl WelcomeState {
    pub fn set_show_on_startup(&mut self, show: bool) {
        self.show_on_startup = show;
        save_startup_pref(show);
    }

    pub fn draw(&mut self, ctx: &egui::Context, skills_count: usize) -> WelcomeAction {
        if !self.open {
            return WelcomeAction::None;
        }
        let mut action = WelcomeAction::None;

        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            action = WelcomeAction::EnterWorkspace;
        }

        // Softly dimmed backdrop
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::unique("devcraft_welcome_backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
                ui.painter()
                    .rect_filled(rect, 0.0, Color32::from_black_alpha(170));
            });

        egui::Window::new("Welcome to DevCraft")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(760.0, 470.0))
            .frame(
                egui::Frame::new()
                    .corner_radius(10.0)
                    .fill(Color32::from_rgb(14, 20, 33))
                    .stroke(Stroke::new(1.2, Color32::from_rgb(54, 80, 122)))
                    .inner_margin(egui::Margin::same(28)),
            )
            .show(ctx, |ui| {
                // 1. Hero Header with Vector Emblem
                ui.horizontal(|ui| {
                    draw_emblem(ui, Vec2::new(64.0, 64.0));
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(format!("DevCraft  v{}", env!("CARGO_PKG_VERSION")))
                                .size(24.0)
                                .strong()
                                .color(Color32::from_rgb(242, 246, 252)),
                        );
                        ui.label(
                            egui::RichText::new(
                                "Fleet orchestration, port & worktree board, and Time Machine checkpoint studio.",
                            )
                            .size(14.5)
                            .color(Color32::from_rgb(176, 198, 224)),
                        );
                    });
                });

                ui.add_space(18.0);
                ui.separator();
                ui.add_space(14.0);

                // 2. Three 1-click Visual Starter Cards
                ui.columns(3, |cols| {
                    action = starter_card(
                        &mut cols[0],
                        "1. Refresh Fleet Digest",
                        "Scans git status, open PRs, and sync drift across all fleet repositories and updates the Today digest.",
                        "Refresh Digest",
                        Color32::from_rgb(110, 240, 205),
                        WelcomeAction::RefreshDigest,
                    )
                    .or_else_action(action);

                    action = starter_card(
                        &mut cols[1],
                        "2. Open Fleet Board & Worktrees",
                        "Inspects repository lanes, active agent claims, dirty worktrees, and live port bindings across the fleet.",
                        "Open Fleet Board",
                        Color32::from_rgb(255, 205, 105),
                        WelcomeAction::OpenBoard,
                    )
                    .or_else_action(action);

                    action = starter_card(
                        &mut cols[2],
                        "3. Guided Quickstart / Help Center",
                        "Step-by-step fleet onboarding, Fleet Gate protocols, Time Machine checkpoints, and bundled agent skills.",
                        "Open Help Center",
                        Color32::from_rgb(148, 195, 255),
                        WelcomeAction::OpenHelpCenter,
                    )
                    .or_else_action(action);
                });

                ui.add_space(18.0);

                // 3. Three Compact Readiness Pills + Keycap Hints
                ui.horizontal_wrapped(|ui| {
                    pill(ui, "Fleet Gate: Ready", Color32::from_rgb(110, 240, 205));
                    let skills_label = format!("{skills_count} Bundled Skills");
                    pill(ui, &skills_label, Color32::from_rgb(255, 205, 105));
                    pill(ui, "AI Chat: Ctrl+J", Color32::from_rgb(148, 195, 255));
                    ui.separator();
                    ui.small("Ctrl+K Palette  |  F1 Shortcuts  |  Ctrl+J AI Chat");
                });

                ui.add_space(18.0);
                ui.separator();
                ui.add_space(10.0);

                // 4. Footer: Startup Preference + Enter Workspace
                ui.horizontal(|ui| {
                    let mut show = self.show_on_startup;
                    if ui.checkbox(&mut show, "Show welcome screen on startup").changed() {
                        self.set_show_on_startup(show);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(
                                egui::RichText::new("Enter Workspace ->")
                                    .size(14.5)
                                    .strong(),
                            )
                            .clicked()
                        {
                            action = WelcomeAction::EnterWorkspace;
                        }
                    });
                });
            });

        if action != WelcomeAction::None {
            self.open = false;
        }
        action
    }
}

trait ActionOr {
    fn or_else_action(self, prev: WelcomeAction) -> WelcomeAction;
}

impl ActionOr for WelcomeAction {
    fn or_else_action(self, prev: WelcomeAction) -> WelcomeAction {
        if self != WelcomeAction::None {
            self
        } else {
            prev
        }
    }
}

fn starter_card(
    ui: &mut egui::Ui,
    title: &str,
    desc: &str,
    btn: &str,
    accent: Color32,
    on_click: WelcomeAction,
) -> WelcomeAction {
    let mut act = WelcomeAction::None;
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(18, 26, 42))
        .stroke(Stroke::new(1.0, Color32::from_rgb(44, 64, 96)))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_min_height(175.0);
            ui.label(egui::RichText::new(title).size(15.5).strong().color(accent));
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(desc)
                    .size(13.0)
                    .color(Color32::from_rgb(190, 206, 228)),
            );
            ui.add_space(12.0);
            if ui.button(btn).clicked() {
                act = on_click;
            }
        });
    act
}

fn pill(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(Color32::from_rgb(20, 30, 48))
        .stroke(Stroke::new(1.0, color))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(12.0).color(color));
        });
}

fn draw_emblem(ui: &mut egui::Ui, size: Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    let teal = Color32::from_rgb(110, 240, 205);
    let gold = Color32::from_rgb(255, 205, 105);
    let blue = Color32::from_rgb(148, 195, 255);
    let grid = Color32::from_rgb(44, 68, 104);

    p.rect_filled(rect, 10.0, Color32::from_rgb(18, 28, 46));
    p.rect_stroke(rect, 10.0, Stroke::new(1.5, teal), egui::StrokeKind::Inside);

    let c = rect.center();
    let r_outer = rect.width().min(rect.height()) * 0.36;
    let r_inner = r_outer * 0.52;

    // Concentric radar rings + subtle crosshair
    p.circle_stroke(c, r_outer, Stroke::new(1.1, grid));
    p.circle_stroke(c, r_inner, Stroke::new(1.0, grid));
    p.line_segment(
        [Pos2::new(c.x - r_outer, c.y), Pos2::new(c.x + r_outer, c.y)],
        Stroke::new(0.8, grid),
    );
    p.line_segment(
        [Pos2::new(c.x, c.y - r_outer), Pos2::new(c.x, c.y + r_outer)],
        Stroke::new(0.8, grid),
    );

    // Fleet node graph positions around radar
    let n1 = Pos2::new(c.x - r_outer * 0.65, c.y - r_outer * 0.55);
    let n2 = Pos2::new(c.x + r_outer * 0.72, c.y - r_outer * 0.42);
    let n3 = Pos2::new(c.x + r_outer * 0.52, c.y + r_outer * 0.68);
    let n4 = Pos2::new(c.x - r_outer * 0.62, c.y + r_outer * 0.52);

    // Graph edges + radar sweep ray
    p.line_segment([c, n1], Stroke::new(1.5, teal));
    p.line_segment([c, n2], Stroke::new(1.8, gold));
    p.line_segment([c, n3], Stroke::new(1.5, blue));
    p.line_segment([c, n4], Stroke::new(1.5, teal));
    p.line_segment([n1, n2], Stroke::new(1.0, grid));
    p.line_segment([n2, n3], Stroke::new(1.0, grid));

    // Nodes + central hub
    p.circle_filled(n1, 3.6, teal);
    p.circle_filled(n2, 4.0, gold);
    p.circle_filled(n3, 3.6, blue);
    p.circle_filled(n4, 3.4, teal);
    p.circle_filled(c, 4.8, gold);
    p.circle_stroke(c, 4.8, Stroke::new(1.2, Color32::from_rgb(14, 20, 33)));
}

fn pref_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("HOME"))
        .map(|root| {
            PathBuf::from(root)
                .join("devcraft")
                .join("welcome_hide.flag")
        })
}

fn should_show_on_startup() -> bool {
    let Some(path) = pref_path() else {
        return true;
    };
    !path.exists()
}

fn save_startup_pref(show: bool) {
    let Some(path) = pref_path() else {
        return;
    };
    if show {
        let _ = std::fs::remove_file(path);
    } else {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, b"hide");
    }
}
