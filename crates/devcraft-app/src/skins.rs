//! DJCraft-inspired Skin Engine and Luminous Cockpit Widgets for DevCraft.
//!
//! Ships 7 curated studio palettes (Battle Console, Cockpit Dark, Club Midnight,
//! Synthwave Outrun, Star Trek LCARS, Baroque Velvet & Gold, Daylight Studio)
//! with configurable card decorations (DropShadow, NeonGlow, LcarsBracket,
//! OrnateDoubleBorder, Clean), vector-painted status LEDs, and a Sun/Moon
//! light-mode toggle that preserves >= 7:1 contrast in Daylight Studio.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use eframe::egui::{
    self, Color32, CornerRadius, Frame, Margin, RichText, Shadow, Stroke, StrokeKind, Visuals,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CardDecorStyle {
    Clean,
    #[default]
    DropShadow,
    NeonGlow,
    OrnateDoubleBorder,
    LcarsBracket,
}

impl CardDecorStyle {
    pub const ALL: [Self; 5] = [
        Self::Clean,
        Self::DropShadow,
        Self::NeonGlow,
        Self::OrnateDoubleBorder,
        Self::LcarsBracket,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Clean => "Clean Flat",
            Self::DropShadow => "Bold Drop Shadow",
            Self::NeonGlow => "Neon Outer Glow",
            Self::OrnateDoubleBorder => "Baroque Double Frame",
            Self::LcarsBracket => "LCARS Starfleet Bracket",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SkinId {
    #[default]
    BattleConsole,
    CockpitDark,
    ClubMidnight,
    SynthwaveOutrun,
    LcarsStarfleet,
    BaroqueGold,
    DaylightStudio,
}

impl SkinId {
    pub const ALL: [Self; 7] = [
        Self::BattleConsole,
        Self::CockpitDark,
        Self::ClubMidnight,
        Self::SynthwaveOutrun,
        Self::LcarsStarfleet,
        Self::BaroqueGold,
        Self::DaylightStudio,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::BattleConsole => "Battle Console",
            Self::CockpitDark => "Cockpit Dark",
            Self::ClubMidnight => "Club Midnight",
            Self::SynthwaveOutrun => "Synthwave Outrun",
            Self::LcarsStarfleet => "Star Trek LCARS",
            Self::BaroqueGold => "Baroque Velvet & Gold",
            Self::DaylightStudio => "Daylight Studio (Light)",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::BattleConsole => "Battle",
            Self::CockpitDark => "Cockpit",
            Self::ClubMidnight => "Midnight",
            Self::SynthwaveOutrun => "Synthwave",
            Self::LcarsStarfleet => "LCARS",
            Self::BaroqueGold => "Baroque",
            Self::DaylightStudio => "Daylight",
        }
    }

    pub fn recommended_decor(self) -> CardDecorStyle {
        match self {
            Self::LcarsStarfleet => CardDecorStyle::LcarsBracket,
            Self::BaroqueGold => CardDecorStyle::OrnateDoubleBorder,
            Self::SynthwaveOutrun => CardDecorStyle::NeonGlow,
            Self::DaylightStudio => CardDecorStyle::Clean,
            _ => CardDecorStyle::DropShadow,
        }
    }

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|&s| s == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn palette(self, decor: CardDecorStyle) -> SkinPalette {
        match self {
            Self::BattleConsole => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(9, 10, 14),
                chassis: Color32::from_rgb(16, 19, 26),
                card_fill: Color32::from_rgb(23, 27, 36),
                card_border: Color32::from_rgb(54, 62, 82),
                text_primary: Color32::from_rgb(244, 246, 250),
                text_secondary: Color32::from_rgb(176, 184, 200),
                accent_a: Color32::from_rgb(56, 189, 248),  // Electric Cyan
                accent_b: Color32::from_rgb(239, 68, 68),   // Battle Crimson
                accent_ok: Color32::from_rgb(52, 211, 153), // Emerald
                accent_warn: Color32::from_rgb(250, 204, 21), // Gold
                decor,
            },
            Self::CockpitDark => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(11, 16, 23),
                chassis: Color32::from_rgb(18, 25, 36),
                card_fill: Color32::from_rgb(23, 31, 44),
                card_border: Color32::from_rgb(51, 65, 85),
                text_primary: Color32::from_rgb(243, 246, 250),
                text_secondary: Color32::from_rgb(186, 198, 214),
                accent_a: Color32::from_rgb(96, 165, 250),
                accent_b: Color32::from_rgb(248, 113, 113),
                accent_ok: Color32::from_rgb(52, 211, 153),
                accent_warn: Color32::from_rgb(251, 191, 36),
                decor,
            },
            Self::ClubMidnight => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(8, 12, 20),
                chassis: Color32::from_rgb(15, 23, 38),
                card_fill: Color32::from_rgb(20, 31, 52),
                card_border: Color32::from_rgb(48, 70, 108),
                text_primary: Color32::from_rgb(240, 246, 255),
                text_secondary: Color32::from_rgb(170, 188, 216),
                accent_a: Color32::from_rgb(56, 189, 248),
                accent_b: Color32::from_rgb(167, 139, 250),
                accent_ok: Color32::from_rgb(45, 212, 191),
                accent_warn: Color32::from_rgb(251, 191, 36),
                decor,
            },
            Self::SynthwaveOutrun => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(16, 8, 30),
                chassis: Color32::from_rgb(25, 13, 46),
                card_fill: Color32::from_rgb(33, 17, 58),
                card_border: Color32::from_rgb(126, 58, 178),
                text_primary: Color32::from_rgb(252, 244, 255),
                text_secondary: Color32::from_rgb(212, 188, 235),
                accent_a: Color32::from_rgb(34, 211, 238),  // Neon Cyan
                accent_b: Color32::from_rgb(236, 72, 153),  // Hot Magenta
                accent_ok: Color32::from_rgb(74, 222, 128), // Acid Mint
                accent_warn: Color32::from_rgb(251, 146, 60), // Sunset Orange
                decor,
            },
            Self::LcarsStarfleet => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(6, 6, 10),
                chassis: Color32::from_rgb(14, 14, 20),
                card_fill: Color32::from_rgb(20, 20, 28),
                card_border: Color32::from_rgb(255, 153, 51),
                text_primary: Color32::from_rgb(255, 244, 224),
                text_secondary: Color32::from_rgb(214, 194, 170),
                accent_a: Color32::from_rgb(255, 153, 51),  // LCARS Amber
                accent_b: Color32::from_rgb(204, 153, 204), // LCARS Lavender
                accent_ok: Color32::from_rgb(102, 204, 153),
                accent_warn: Color32::from_rgb(255, 204, 102),
                decor,
            },
            Self::BaroqueGold => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(18, 9, 14),
                chassis: Color32::from_rgb(30, 14, 22),
                card_fill: Color32::from_rgb(38, 18, 28),
                card_border: Color32::from_rgb(212, 175, 55), // Antique Gold
                text_primary: Color32::from_rgb(252, 246, 232),
                text_secondary: Color32::from_rgb(220, 202, 170),
                accent_a: Color32::from_rgb(212, 175, 55),
                accent_b: Color32::from_rgb(225, 92, 122),
                accent_ok: Color32::from_rgb(82, 204, 150),
                accent_warn: Color32::from_rgb(245, 185, 66),
                decor,
            },
            Self::DaylightStudio => SkinPalette {
                is_light: true,
                canvas: Color32::from_rgb(238, 242, 247),
                chassis: Color32::from_rgb(246, 248, 252),
                card_fill: Color32::from_rgb(255, 255, 255),
                card_border: Color32::from_rgb(196, 206, 220),
                text_primary: Color32::from_rgb(11, 18, 32),
                text_secondary: Color32::from_rgb(51, 65, 85),
                accent_a: Color32::from_rgb(29, 78, 216),   // Royal Blue
                accent_b: Color32::from_rgb(220, 38, 38),   // Crisp Crimson
                accent_ok: Color32::from_rgb(5, 150, 105),  // Deep Emerald
                accent_warn: Color32::from_rgb(180, 83, 9), // High-contrast Amber
                decor,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SkinPalette {
    pub is_light: bool,
    pub canvas: Color32,
    pub chassis: Color32,
    pub card_fill: Color32,
    pub card_border: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub accent_a: Color32,
    pub accent_b: Color32,
    pub accent_ok: Color32,
    pub accent_warn: Color32,
    pub decor: CardDecorStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinState {
    pub skin: SkinId,
    pub decor: CardDecorStyle,
    pub last_dark: SkinId,
}

impl Default for SkinState {
    fn default() -> Self {
        Self {
            skin: SkinId::BattleConsole,
            decor: CardDecorStyle::DropShadow,
            last_dark: SkinId::BattleConsole,
        }
    }
}

impl SkinState {
    fn config_path() -> PathBuf {
        let base = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        base.join("devcraft").join("skin.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<SkinState>(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }

    pub fn select(&mut self, skin: SkinId) {
        self.skin = skin;
        self.decor = skin.recommended_decor();
        if skin != SkinId::DaylightStudio {
            self.last_dark = skin;
        }
        self.save();
    }

    pub fn toggle_daylight(&mut self) {
        if self.skin == SkinId::DaylightStudio {
            let target = if self.last_dark == SkinId::DaylightStudio {
                SkinId::BattleConsole
            } else {
                self.last_dark
            };
            self.select(target);
        } else {
            self.last_dark = self.skin;
            self.select(SkinId::DaylightStudio);
        }
    }

    pub fn palette(&self) -> SkinPalette {
        self.skin.palette(self.decor)
    }

    pub fn apply_to_ctx(&self, ctx: &egui::Context) {
        let p = self.palette();
        let mut visuals = if p.is_light {
            Visuals::light()
        } else {
            Visuals::dark()
        };
        visuals.panel_fill = p.canvas;
        visuals.window_fill = p.chassis;
        visuals.faint_bg_color = p.chassis;
        visuals.extreme_bg_color = if p.is_light {
            Color32::from_rgb(255, 255, 255)
        } else {
            p.canvas
        };
        visuals.override_text_color = Some(p.text_primary);
        visuals.widgets.noninteractive.bg_fill = p.card_fill;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text_primary);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.card_border);
        visuals.widgets.inactive.bg_fill = p.chassis;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text_primary);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, p.card_border);
        visuals.widgets.hovered.bg_fill = if p.is_light {
            Color32::from_rgb(226, 234, 246)
        } else {
            p.card_fill
        };
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.4, p.accent_a);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.2, p.text_primary);
        visuals.widgets.active.bg_fill = p.accent_a;
        visuals.widgets.active.fg_stroke = Stroke::new(
            1.4,
            if p.is_light {
                Color32::WHITE
            } else {
                Color32::from_rgb(10, 14, 22)
            },
        );
        visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(
            p.accent_a.r(),
            p.accent_a.g(),
            p.accent_a.b(),
            if p.is_light { 50 } else { 72 },
        );
        visuals.selection.stroke = Stroke::new(1.4, p.accent_a);
        ctx.set_visuals(visuals);
    }
}

/// DJCraft-inspired skin-aware card frame with DropShadow, NeonGlow, LCARS, or Baroque accents.
pub fn skin_card<R>(
    ui: &mut egui::Ui,
    palette: &SkinPalette,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let corner = match palette.decor {
        CardDecorStyle::LcarsBracket => CornerRadius::same(14),
        CardDecorStyle::OrnateDoubleBorder => CornerRadius::same(4),
        _ => CornerRadius::same(9),
    };
    let shadow = match palette.decor {
        CardDecorStyle::Clean => Shadow::NONE,
        CardDecorStyle::DropShadow => Shadow {
            offset: [3, 4],
            blur: 14,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(
                0,
                0,
                0,
                if palette.is_light { 36 } else { 130 },
            ),
        },
        CardDecorStyle::NeonGlow => Shadow {
            offset: [0, 0],
            blur: 16,
            spread: 1,
            color: Color32::from_rgba_unmultiplied(
                palette.accent_a.r(),
                palette.accent_a.g(),
                palette.accent_a.b(),
                if palette.is_light { 60 } else { 115 },
            ),
        },
        CardDecorStyle::OrnateDoubleBorder => Shadow {
            offset: [2, 3],
            blur: 10,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(
                palette.card_border.r(),
                palette.card_border.g(),
                palette.card_border.b(),
                68,
            ),
        },
        CardDecorStyle::LcarsBracket => Shadow {
            offset: [0, 0],
            blur: 10,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(
                palette.accent_a.r(),
                palette.accent_a.g(),
                palette.accent_a.b(),
                70,
            ),
        },
    };
    let border_col = match palette.decor {
        CardDecorStyle::NeonGlow => palette.accent_a,
        _ => palette.card_border,
    };

    let inner_resp = Frame::new()
        .fill(palette.card_fill)
        .stroke(Stroke::new(1.3, border_col))
        .shadow(shadow)
        .corner_radius(corner)
        .inner_margin(Margin::same(10))
        .show(ui, add_contents);

    let rect = inner_resp.response.rect;
    let painter = ui.painter();
    match palette.decor {
        CardDecorStyle::OrnateDoubleBorder => {
            painter.rect_stroke(
                rect.shrink(4.0),
                CornerRadius::same(2),
                Stroke::new(
                    1.0,
                    Color32::from_rgba_unmultiplied(
                        palette.card_border.r(),
                        palette.card_border.g(),
                        palette.card_border.b(),
                        180,
                    ),
                ),
                StrokeKind::Inside,
            );
            for pt in [
                rect.left_top(),
                rect.right_top(),
                rect.left_bottom(),
                rect.right_bottom(),
            ] {
                painter.circle_filled(pt, 2.8, palette.accent_a);
            }
        }
        CardDecorStyle::LcarsBracket => {
            let left_bar = egui::Rect::from_min_max(
                egui::pos2(rect.left() + 1.0, rect.top() + 8.0),
                egui::pos2(rect.left() + 5.5, rect.bottom() - 8.0),
            );
            painter.rect_filled(left_bar, CornerRadius::same(3), palette.accent_a);
            let top_pill = egui::Rect::from_min_max(
                egui::pos2(rect.left() + 10.0, rect.top() + 1.0),
                egui::pos2(
                    (rect.left() + 68.0).min(rect.right() - 12.0),
                    rect.top() + 4.5,
                ),
            );
            painter.rect_filled(top_pill, CornerRadius::same(2), palette.accent_b);
        }
        CardDecorStyle::NeonGlow => {
            painter.rect_stroke(
                rect.shrink(2.0),
                CornerRadius::same(7),
                Stroke::new(
                    1.0,
                    Color32::from_rgba_unmultiplied(
                        palette.accent_b.r(),
                        palette.accent_b.g(),
                        palette.accent_b.b(),
                        110,
                    ),
                ),
                StrokeKind::Inside,
            );
        }
        _ => {}
    }

    inner_resp
}

/// Vector-painted luminous status LED dot with subtle outer halo.
pub fn status_led(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    let center = rect.center();
    let painter = ui.painter();
    painter.circle_filled(
        center,
        5.5,
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 58),
    );
    painter.circle_filled(center, 3.6, color);
}

/// Luminous KPI card with colored top accent strip, big value, and LED sub-label.
pub struct HeroKpi<'a> {
    pub label: &'a str,
    pub value: String,
    pub sub: String,
    pub accent: Color32,
}

pub fn luminous_kpi_grid(ui: &mut egui::Ui, palette: &SkinPalette, items: &[HeroKpi<'_>]) {
    let cols = 4;
    for chunk in items.chunks(cols) {
        ui.columns(cols, |row| {
            for (idx, item) in chunk.iter().enumerate() {
                row[idx].vertical(|ui| {
                    let resp = skin_card(ui, palette, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            status_led(ui, item.accent);
                            ui.label(
                                RichText::new(item.label)
                                    .size(12.0)
                                    .color(palette.text_secondary),
                            );
                        });
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(&item.value)
                                .size(24.0)
                                .strong()
                                .monospace()
                                .color(palette.text_primary),
                        );
                        if !item.sub.is_empty() {
                            ui.label(
                                RichText::new(&item.sub)
                                    .size(11.5)
                                    .color(palette.text_secondary),
                            );
                        }
                    });
                    // Colored top accent rail (DJCraft console bar)
                    let r = resp.response.rect;
                    let rail = egui::Rect::from_min_max(
                        egui::pos2(r.left() + 8.0, r.top() + 1.0),
                        egui::pos2(r.right() - 8.0, r.top() + 3.5),
                    );
                    ui.painter()
                        .rect_filled(rail, CornerRadius::same(2), item.accent);
                });
            }
        });
        ui.add_space(6.0);
    }
}

/// Vector-painted 24x24 Sun / Crescent Moon toggle (Daylight Studio vs active dark skin).
pub fn theme_toggle_button(ui: &mut egui::Ui, is_light: bool) -> bool {
    let size = egui::vec2(24.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&resp);
        ui.painter().rect(
            rect,
            CornerRadius::same(6),
            visuals.bg_fill,
            visuals.bg_stroke,
            StrokeKind::Inside,
        );
        let center = rect.center();
        let painter = ui.painter();
        if !is_light {
            let sun = Color32::from_rgb(251, 191, 36);
            painter.circle_filled(center, 3.8, sun);
            for i in 0..8 {
                let angle = (i as f32) * std::f32::consts::TAU / 8.0;
                let dir = egui::vec2(angle.cos(), angle.sin());
                painter.line_segment(
                    [center + dir * 5.6, center + dir * 8.0],
                    Stroke::new(1.4, sun),
                );
            }
        } else {
            let moon = Color32::from_rgb(30, 41, 59);
            painter.circle_filled(center, 5.5, moon);
            painter.circle_filled(center + egui::vec2(2.6, -1.8), 4.6, visuals.bg_fill);
        }
    }
    resp.on_hover_text("Toggle Daylight Studio / Dark Cockpit Skin")
        .clicked()
}

/// Compact toolbar skin cycler pill.
pub fn skin_toolbar_controls(ui: &mut egui::Ui, state: &mut SkinState) {
    let label = format!("Skin: {}", state.skin.short_label());
    if ui
        .button(label)
        .on_hover_text("Cycle DJCraft-inspired cockpit skin (also in LLM & Settings)")
        .clicked()
    {
        let next = state.skin.next();
        state.select(next);
    }
    if theme_toggle_button(ui, state.skin == SkinId::DaylightStudio) {
        state.toggle_daylight();
    }
}

/// Skin & Card Decoration selector card for the Settings view.
pub fn skin_settings_card(ui: &mut egui::Ui, state: &mut SkinState) {
    let palette = state.palette();
    skin_card(ui, &palette, |ui| {
        ui.label(
            RichText::new("Cockpit Skin & Visuals (DJCraft Engine)")
                .strong()
                .size(15.0)
                .color(palette.text_primary),
        );
        ui.label(
            RichText::new("Choose a studio palette and card frame style. Daylight Studio keeps >= 7:1 contrast.")
                .size(12.5)
                .color(palette.text_secondary),
        );
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            for skin in SkinId::ALL {
                let selected = state.skin == skin;
                let btn = egui::Button::new(skin.label()).selected(selected);
                if ui.add(btn).clicked() {
                    state.select(skin);
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("Card frame:")
                    .size(12.5)
                    .color(palette.text_secondary),
            );
            for decor in CardDecorStyle::ALL {
                let selected = state.decor == decor;
                if ui.selectable_label(selected, decor.label()).clicked() {
                    state.decor = decor;
                    state.save();
                }
            }
        });
    });
}
