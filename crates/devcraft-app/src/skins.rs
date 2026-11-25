//! DJCraft-inspired Skin Engine and Luminous Cockpit Widgets for DevCraft.
//!
//! Ships 9 curated studio palettes (Battle Console, Cockpit Dark, Club Midnight,
//! Carbon Deck, Shocking Pink Club, Synthwave Outrun, Star Trek LCARS,
//! Baroque Velvet & Gold, Daylight Studio) with configurable card decorations
//! (DropShadow, NeonGlow, LcarsBracket, OrnateDoubleBorder, Clean), six animated
//! background backdrops (Solid, Brushed Metal, Carbon Weave, Sparkle Club,
//! Starfield Warp, Baroque Damask), vector-painted status LEDs, and a Sun/Moon
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
pub enum BackdropStyle {
    #[default]
    SparkleClub,
    Solid,
    BrushedMetal,
    CarbonWeave,
    StarfieldWarp,
    BaroqueDamask,
}

impl BackdropStyle {
    pub const ALL: [Self; 6] = [
        Self::SparkleClub,
        Self::Solid,
        Self::BrushedMetal,
        Self::CarbonWeave,
        Self::StarfieldWarp,
        Self::BaroqueDamask,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::SparkleClub => "Sparkle Club",
            Self::Solid => "Solid",
            Self::BrushedMetal => "Brushed Metal",
            Self::CarbonWeave => "Carbon Weave",
            Self::StarfieldWarp => "Starfield Warp (LCARS)",
            Self::BaroqueDamask => "Baroque Gold Damask",
        }
    }

    /// Animated backdrops need a repaint loop; static ones paint once.
    pub fn animated(self) -> bool {
        matches!(self, Self::SparkleClub | Self::StarfieldWarp)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SkinId {
    #[default]
    BattleConsole,
    CockpitDark,
    ClubMidnight,
    CarbonDeck,
    ShockingPink,
    SynthwaveOutrun,
    LcarsStarfleet,
    BaroqueGold,
    DaylightStudio,
}

impl SkinId {
    pub const ALL: [Self; 9] = [
        Self::BattleConsole,
        Self::CockpitDark,
        Self::ClubMidnight,
        Self::CarbonDeck,
        Self::ShockingPink,
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
            Self::CarbonDeck => "Carbon Deck",
            Self::ShockingPink => "Shocking Pink Club",
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
            Self::CarbonDeck => "Carbon",
            Self::ShockingPink => "Pink",
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
            Self::ShockingPink | Self::SynthwaveOutrun => CardDecorStyle::NeonGlow,
            Self::DaylightStudio => CardDecorStyle::Clean,
            _ => CardDecorStyle::DropShadow,
        }
    }

    pub fn recommended_backdrop(self) -> BackdropStyle {
        match self {
            Self::LcarsStarfleet => BackdropStyle::StarfieldWarp,
            Self::BaroqueGold => BackdropStyle::BaroqueDamask,
            Self::CarbonDeck => BackdropStyle::CarbonWeave,
            Self::DaylightStudio => BackdropStyle::Solid,
            _ => BackdropStyle::SparkleClub,
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
                accent_a: Color32::from_rgb(56, 189, 248), // Electric Cyan
                accent_b: Color32::from_rgb(239, 68, 68),  // Battle Crimson
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
            Self::CarbonDeck => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(14, 16, 20),
                chassis: Color32::from_rgb(24, 28, 34),
                card_fill: Color32::from_rgb(30, 35, 43),
                card_border: Color32::from_rgb(64, 72, 86),
                text_primary: Color32::from_rgb(245, 247, 250),
                text_secondary: Color32::from_rgb(176, 184, 196),
                accent_a: Color32::from_rgb(20, 184, 166), // Machined Teal
                accent_b: Color32::from_rgb(249, 115, 22), // Safety Orange
                accent_ok: Color32::from_rgb(96, 165, 250),
                accent_warn: Color32::from_rgb(244, 63, 94),
                decor,
            },
            Self::ShockingPink => SkinPalette {
                is_light: false,
                canvas: Color32::from_rgb(20, 4, 18),
                chassis: Color32::from_rgb(34, 8, 30),
                card_fill: Color32::from_rgb(44, 11, 38),
                card_border: Color32::from_rgb(255, 46, 151),
                text_primary: Color32::from_rgb(255, 245, 252),
                text_secondary: Color32::from_rgb(255, 182, 224),
                accent_a: Color32::from_rgb(255, 20, 147), // Shocking Pink
                accent_b: Color32::from_rgb(0, 245, 255),  // Electric Cyan
                accent_ok: Color32::from_rgb(163, 255, 18), // Acid Lime
                accent_warn: Color32::from_rgb(255, 215, 0), // Hot Gold
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
                accent_a: Color32::from_rgb(34, 211, 238), // Neon Cyan
                accent_b: Color32::from_rgb(236, 72, 153), // Hot Magenta
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
                accent_a: Color32::from_rgb(255, 153, 51), // LCARS Amber
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
                accent_a: Color32::from_rgb(29, 78, 216), // Royal Blue
                accent_b: Color32::from_rgb(220, 38, 38), // Crisp Crimson
                accent_ok: Color32::from_rgb(5, 150, 105), // Deep Emerald
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
    #[serde(default = "default_backdrop")]
    pub backdrop: BackdropStyle,
    #[serde(default = "default_intensity")]
    pub backdrop_intensity: f32,
}

fn default_backdrop() -> BackdropStyle {
    BackdropStyle::SparkleClub
}

fn default_intensity() -> f32 {
    1.0
}

impl Default for SkinState {
    fn default() -> Self {
        Self {
            skin: SkinId::BattleConsole,
            decor: CardDecorStyle::DropShadow,
            last_dark: SkinId::BattleConsole,
            backdrop: BackdropStyle::SparkleClub,
            backdrop_intensity: 1.0,
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
        self.backdrop = skin.recommended_backdrop();
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

fn scale_alpha(base: u8, intensity: f32) -> u8 {
    ((base as f32) * intensity.clamp(0.15, 2.5)).clamp(0.0, 255.0) as u8
}

/// Paint a bold textured or dynamic underlay behind all UI panels.
/// Ported from DJCraft's skin engine; accent fields stand in for deck colors.
pub fn paint_backdrop(
    ctx: &egui::Context,
    palette: &SkinPalette,
    style: BackdropStyle,
    time_sec: f32,
    energy: f32,
    intensity: f32,
) {
    let rect = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(rect, 0.0, palette.canvas);
    let intensity = intensity.clamp(0.2, 2.0);

    match style {
        BackdropStyle::Solid => {}
        BackdropStyle::BrushedMetal => {
            let step = 4.0_f32;
            let mut y = rect.top();
            let mut row = 0u32;
            while y < rect.bottom() {
                let base_a = if row.is_multiple_of(3) {
                    115
                } else if row.is_multiple_of(2) {
                    68
                } else {
                    36
                };
                let alpha = scale_alpha(base_a, intensity);
                let col = if palette.is_light {
                    Color32::from_rgba_unmultiplied(15, 23, 42, alpha)
                } else {
                    Color32::from_rgba_unmultiplied(226, 232, 240, alpha)
                };
                painter.line_segment(
                    [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                    Stroke::new(1.6, col),
                );
                y += step;
                row += 1;
            }
            for i in 0..4 {
                let x0 = rect.left() + rect.width() * (0.08 + i as f32 * 0.27);
                painter.line_segment(
                    [
                        egui::pos2(x0, rect.top()),
                        egui::pos2(x0 + rect.height() * 0.35, rect.bottom()),
                    ],
                    Stroke::new(
                        56.0,
                        Color32::from_rgba_unmultiplied(
                            palette.accent_a.r(),
                            palette.accent_a.g(),
                            palette.accent_a.b(),
                            scale_alpha(if palette.is_light { 42 } else { 32 }, intensity),
                        ),
                    ),
                );
            }
        }
        BackdropStyle::CarbonWeave => {
            let step = 14.0_f32;
            let col_a = Color32::from_rgba_unmultiplied(
                palette.accent_a.r(),
                palette.accent_a.g(),
                palette.accent_a.b(),
                scale_alpha(if palette.is_light { 110 } else { 120 }, intensity),
            );
            let col_b = Color32::from_rgba_unmultiplied(
                palette.accent_b.r(),
                palette.accent_b.g(),
                palette.accent_b.b(),
                scale_alpha(if palette.is_light { 78 } else { 88 }, intensity),
            );
            let h = rect.height();
            let mut x = rect.left() - h;
            let mut idx = 0u32;
            while x < rect.right() + h {
                let col = if idx.is_multiple_of(2) { col_a } else { col_b };
                painter.line_segment(
                    [egui::pos2(x, rect.top()), egui::pos2(x + h, rect.bottom())],
                    Stroke::new(2.8, col),
                );
                painter.line_segment(
                    [egui::pos2(x + h, rect.top()), egui::pos2(x, rect.bottom())],
                    Stroke::new(2.4, col_b),
                );
                x += step;
                idx += 1;
            }
        }
        BackdropStyle::SparkleClub => {
            // Bold ambient club glow orbs
            for i in 0..8u32 {
                let fi = i as f32;
                let px = rect.left()
                    + rect.width()
                        * (0.08 + 0.84 * ((fi * 0.41 + time_sec * 0.04).sin() * 0.5 + 0.5));
                let py = rect.top()
                    + rect.height()
                        * (0.10 + 0.80 * ((fi * 0.59 + time_sec * 0.03).cos() * 0.5 + 0.5));
                let base = match i % 3 {
                    0 => palette.accent_a,
                    1 => palette.accent_b,
                    _ => palette.accent_ok,
                };
                let r = (120.0 + 55.0 * energy) * (0.8 + 0.25 * intensity);
                painter.circle_filled(
                    egui::pos2(px, py),
                    r,
                    Color32::from_rgba_unmultiplied(
                        base.r(),
                        base.g(),
                        base.b(),
                        scale_alpha(if palette.is_light { 62 } else { 78 }, intensity),
                    ),
                );
            }
            // Vivid animated diamond sparkles
            let accents = [
                palette.accent_a,
                palette.accent_b,
                palette.accent_ok,
                palette.accent_warn,
            ];
            for i in 0..96u32 {
                let seed_x = ((i.wrapping_mul(73856093) ^ 0x5A17) % 1000) as f32 / 1000.0;
                let seed_y = ((i.wrapping_mul(19349663) ^ 0x2F91) % 1000) as f32 / 1000.0;
                let phase = (i as f32) * 1.13 + time_sec * (2.0 + (i % 5) as f32 * 0.45);
                let twinkle = (phase.sin() * 0.5 + 0.5).powf(1.8);
                let x = rect.left() + seed_x * rect.width();
                let y = rect.top() + seed_y * rect.height();
                let col = accents[(i as usize) % accents.len()];
                let alpha = scale_alpha(
                    (70.0 + 185.0 * twinkle * (0.65 + 0.35 * energy)) as u8,
                    intensity,
                );
                let c = Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), alpha);
                let sz = (2.2 + 4.4 * twinkle) * (0.75 + 0.3 * intensity);
                painter.circle_filled(egui::pos2(x, y), sz * 0.55, c);
                if twinkle > 0.40 {
                    painter.line_segment(
                        [egui::pos2(x - sz * 2.0, y), egui::pos2(x + sz * 2.0, y)],
                        Stroke::new(1.5, c),
                    );
                    painter.line_segment(
                        [egui::pos2(x, y - sz * 2.0), egui::pos2(x, y + sz * 2.0)],
                        Stroke::new(1.5, c),
                    );
                }
            }
        }
        BackdropStyle::StarfieldWarp => {
            // LCARS tactical subspace grid + streaking warp stars
            let grid_step = 48.0_f32;
            let grid_col = Color32::from_rgba_unmultiplied(
                palette.accent_a.r(),
                palette.accent_a.g(),
                palette.accent_a.b(),
                scale_alpha(48, intensity),
            );
            let mut gx = rect.left();
            while gx < rect.right() {
                painter.line_segment(
                    [egui::pos2(gx, rect.top()), egui::pos2(gx, rect.bottom())],
                    Stroke::new(1.0, grid_col),
                );
                gx += grid_step;
            }
            let mut gy = rect.top();
            while gy < rect.bottom() {
                painter.line_segment(
                    [egui::pos2(rect.left(), gy), egui::pos2(rect.right(), gy)],
                    Stroke::new(1.0, grid_col),
                );
                gy += grid_step;
            }
            // Radial warp star streaks from viewport center
            let center = rect.center();
            let max_r = rect.width().hypot(rect.height()) * 0.55;
            for i in 0..110u32 {
                let angle =
                    ((i.wrapping_mul(2654435761) % 3600) as f32 / 3600.0) * std::f32::consts::TAU;
                let speed = 0.14 + (i % 7) as f32 * 0.04 + energy * 0.12;
                let depth = ((i as f32) * 0.173 + time_sec * speed).fract();
                let r0 = depth * depth * max_r;
                let streak = (10.0 + 48.0 * depth) * (0.7 + 0.35 * intensity);
                let dir = egui::vec2(angle.cos(), angle.sin());
                let p0 = center + dir * r0;
                let p1 = center + dir * (r0 + streak);
                let col = match i % 4 {
                    0 => palette.accent_a,
                    1 => palette.accent_b,
                    2 => palette.accent_ok,
                    _ => Color32::WHITE,
                };
                let alpha = scale_alpha((60.0 + 190.0 * depth) as u8, intensity);
                painter.line_segment(
                    [p0, p1],
                    Stroke::new(
                        1.0 + 1.8 * depth,
                        Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), alpha),
                    ),
                );
            }
        }
        BackdropStyle::BaroqueDamask => {
            // Bold gilded diamond lattice & ornate rosette medallion filigree
            let tile = 54.0_f32;
            let gold = palette.card_border;
            let line_col = Color32::from_rgba_unmultiplied(
                gold.r(),
                gold.g(),
                gold.b(),
                scale_alpha(115, intensity),
            );
            let accent_col = Color32::from_rgba_unmultiplied(
                palette.accent_a.r(),
                palette.accent_a.g(),
                palette.accent_a.b(),
                scale_alpha(72, intensity),
            );
            let mut y = rect.top() - tile;
            let mut row = 0u32;
            while y < rect.bottom() + tile {
                let mut x = rect.left() - tile
                    + if row.is_multiple_of(2) {
                        0.0
                    } else {
                        tile * 0.5
                    };
                while x < rect.right() + tile {
                    let cx = x + tile * 0.5;
                    let cy = y + tile * 0.5;
                    let r = tile * 0.44;
                    let top = egui::pos2(cx, cy - r);
                    let right = egui::pos2(cx + r, cy);
                    let bot = egui::pos2(cx, cy + r);
                    let left = egui::pos2(cx - r, cy);
                    // Outer diamond trellis
                    painter.line_segment([top, right], Stroke::new(1.6, line_col));
                    painter.line_segment([right, bot], Stroke::new(1.6, line_col));
                    painter.line_segment([bot, left], Stroke::new(1.6, line_col));
                    painter.line_segment([left, top], Stroke::new(1.6, line_col));
                    // Inner gilded rosette medallion
                    painter.circle_stroke(egui::pos2(cx, cy), r * 0.42, Stroke::new(1.5, line_col));
                    painter.circle_filled(egui::pos2(cx, cy), r * 0.16, accent_col);
                    x += tile;
                }
                y += tile * 0.85;
                row += 1;
            }
        }
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
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("Backdrop (DJCraft underlay):")
                    .size(12.5)
                    .color(palette.text_secondary),
            );
            for backdrop in BackdropStyle::ALL {
                let selected = state.backdrop == backdrop;
                if ui.selectable_label(selected, backdrop.label()).clicked() {
                    state.backdrop = backdrop;
                    state.save();
                }
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Backdrop intensity:")
                    .size(12.5)
                    .color(palette.text_secondary),
            );
            let mut intensity = state.backdrop_intensity;
            if ui
                .add(egui::Slider::new(&mut intensity, 0.2..=2.0).show_value(true))
                .changed()
            {
                state.backdrop_intensity = intensity;
                state.save();
            }
        });
        ui.label(
            RichText::new("Picking a palette also applies its recommended frame and backdrop.")
                .size(12.0)
                .color(palette.text_secondary),
        );
    });
}
