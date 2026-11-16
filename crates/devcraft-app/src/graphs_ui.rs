//! Poster charts for the fleet. Dark cards, so the colors read in light and dark.

use devcraft_core::graphs::{GraphSnap, LangShare, ReleaseMark, WeekFlow};
use eframe::egui::epaint::{Mesh, Vertex};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Margin, Pos2, Rect, RichText, Sense, Shape,
    Stroke, Vec2,
};

const INK: Color32 = Color32::from_rgb(10, 14, 28);
const PAPER: Color32 = Color32::from_rgb(236, 240, 252);
const MUTED: Color32 = Color32::from_rgb(154, 166, 198);
const TEAL: Color32 = Color32::from_rgb(45, 212, 191);
const PINK: Color32 = Color32::from_rgb(244, 114, 182);
const AMBER: Color32 = Color32::from_rgb(251, 191, 36);
const SKY: Color32 = Color32::from_rgb(56, 189, 248);
const VIOLET: Color32 = Color32::from_rgb(167, 139, 250);
const EMPTY: Color32 = Color32::from_rgb(22, 30, 52);

const PAL: [Color32; 9] = [
    Color32::from_rgb(45, 212, 191),
    Color32::from_rgb(129, 140, 248),
    Color32::from_rgb(251, 191, 36),
    Color32::from_rgb(244, 114, 182),
    Color32::from_rgb(56, 189, 248),
    Color32::from_rgb(167, 139, 250),
    Color32::from_rgb(52, 211, 153),
    Color32::from_rgb(251, 146, 60),
    Color32::from_rgb(148, 163, 184),
];

pub fn empty(ui: &mut egui::Ui) {
    card(
        ui,
        "The fleet, in color",
        "Measure once. The picture is kept.",
        |ui| {
            ui.label(
            RichText::new(
                "A night sky of checkouts, commit weather, who is shipping, which weekdays the fleet works, a code tide, languages, and releases.",
            )
            .color(PAPER),
        );
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                swatch(ui, AMBER, "stars");
                swatch(ui, TEAL, "commits");
                swatch(ui, VIOLET, "authors");
                swatch(ui, SKY, "weekdays");
            });
        },
    );
}

pub fn show(ui: &mut egui::Ui, snap: &GraphSnap) {
    hero(ui, snap);
    ui.add_space(12.0);
    let sky_note = format!(
        "{} lit in 26 weeks. {} touched this week. {} quiet. Size is commits.",
        snap.stars.len(),
        snap.hot,
        snap.quiet
    );
    card(ui, "Night sky", &sky_note, |ui| sky(ui, snap));
    ui.add_space(12.0);
    let width = ui.available_width();
    if width > 920.0 {
        ui.columns(2, |cols| {
            card(
                &mut cols[0],
                "Who is shipping",
                "Authors on commits in the last 26 weeks.",
                |ui| authors(ui, snap),
            );
            card(
                &mut cols[1],
                "Weekday rhythm",
                "Monday through Sunday, across the fleet.",
                |ui| weekdays(ui, snap),
            );
        });
    } else {
        card(
            ui,
            "Who is shipping",
            "Authors on commits in the last 26 weeks.",
            |ui| authors(ui, snap),
        );
        ui.add_space(12.0);
        card(
            ui,
            "Weekday rhythm",
            "Monday through Sunday, across the fleet.",
            |ui| weekdays(ui, snap),
        );
    }
    ui.add_space(12.0);
    card(
        ui,
        "Commit weather",
        "One cell is a day. Brighter means more commits across the fleet.",
        |ui| heatmap(ui, snap),
    );
    ui.add_space(12.0);
    let width = ui.available_width();
    if width > 920.0 {
        ui.columns(2, |cols| {
            card(
                &mut cols[0],
                "Code tide",
                "Lines added rise. Lines deleted fall. Twelve weeks.",
                |ui| tide(ui, snap),
            );
            card(
                &mut cols[1],
                "Languages",
                "Tracked source lines. Generated trees stay out.",
                |ui| donut(ui, snap),
            );
        });
    } else {
        card(
            ui,
            "Code tide",
            "Lines added rise. Lines deleted fall. Twelve weeks.",
            |ui| tide(ui, snap),
        );
        ui.add_space(12.0);
        card(
            ui,
            "Languages",
            "Tracked source lines. Generated trees stay out.",
            |ui| donut(ui, snap),
        );
    }
    ui.add_space(12.0);
    card(
        ui,
        "Heavy checkouts",
        "The twelve largest, colored by their main language.",
        |ui| bars(ui, snap),
    );
    ui.add_space(12.0);
    card(
        ui,
        "Release spine",
        "Newest tags. Each lane is a checkout.",
        |ui| spine(ui, snap),
    );
    ui.add_space(6.0);
    if !snap.generated.is_empty() {
        ui.label(
            RichText::new(format!("Measured {}", snap.generated))
                .size(12.0)
                .color(MUTED),
        );
    }
}

fn hero(ui: &mut egui::Ui, snap: &GraphSnap) {
    let tiles = [
        ("Checkouts", commas(u64::from(snap.repos)), TEAL),
        ("Commits", commas(snap.commits), SKY),
        ("Source lines", commas(snap.lines), AMBER),
        (
            "Lines moved",
            commas(snap.added.saturating_add(snap.deleted)),
            PINK,
        ),
    ];
    ui.columns(4, |cols| {
        for (col, (label, value, color)) in cols.iter_mut().zip(tiles) {
            egui::Frame::new()
                .fill(INK)
                .inner_margin(Margin::same(14))
                .corner_radius(CornerRadius::same(14))
                .show(col, |ui| {
                    let (bar, _) = ui.allocate_exact_size(Vec2::new(36.0, 4.0), Sense::hover());
                    ui.painter().rect_filled(bar, CornerRadius::same(2), color);
                    ui.add_space(8.0);
                    ui.label(RichText::new(value).size(26.0).color(PAPER).strong());
                    ui.label(RichText::new(label).size(12.5).color(MUTED));
                });
        }
    });
}

fn heatmap(ui: &mut egui::Ui, snap: &GraphSnap) {
    let max = snap.days.iter().map(|day| day.commits).max().unwrap_or(0);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 168.0), Sense::hover());
    let gap = 3.0;
    let cols = 26.0;
    let rows = 7.0;
    let cell_w = ((rect.width() - gap * (cols - 1.0)) / cols).max(4.0);
    let cell_h = ((rect.height() - gap * (rows - 1.0)) / rows).max(4.0);
    let mut tip = None;
    for (index, day) in snap.days.iter().enumerate() {
        let col = (index / 7) as f32;
        let row = (index % 7) as f32;
        let cell = Rect::from_min_size(
            Pos2::new(
                rect.left() + col * (cell_w + gap),
                rect.top() + row * (cell_h + gap),
            ),
            Vec2::new(cell_w, cell_h),
        );
        let color = if day.future {
            Color32::from_rgb(14, 18, 32)
        } else {
            heat(day.commits, max)
        };
        ui.painter().rect_filled(cell, CornerRadius::same(3), color);
        if let Some(pos) = response.hover_pos() {
            if cell.contains(pos) && !day.future {
                tip = Some(format!("{}  ·  {} commits", day.day, day.commits));
            }
        }
    }
    if let Some(tip) = tip {
        response.clone().on_hover_text(tip);
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("quiet").size(11.0).color(MUTED));
        for step in 0..5 {
            let color = heat(step * max / 4, max.max(1));
            let (sw, _) = ui.allocate_exact_size(Vec2::new(14.0, 10.0), Sense::hover());
            ui.painter().rect_filled(sw, CornerRadius::same(2), color);
        }
        ui.label(RichText::new("busy").size(11.0).color(MUTED));
    });
}

fn tide(ui: &mut egui::Ui, snap: &GraphSnap) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 220.0), Sense::hover());
    let mid = rect.center().y;
    ui.painter().line_segment(
        [Pos2::new(rect.left(), mid), Pos2::new(rect.right(), mid)],
        Stroke::new(1.0, Color32::from_rgb(40, 52, 82)),
    );
    let max = snap
        .weeks
        .iter()
        .map(|week| week.added.max(week.deleted))
        .max()
        .unwrap_or(1)
        .max(1);
    let slot = rect.width() / snap.weeks.len().max(1) as f32;
    let bar_w = (slot * 0.46).max(4.0);
    let room = rect.height() * 0.42;
    let mut tip = None;
    for (index, week) in snap.weeks.iter().enumerate() {
        let cx = rect.left() + slot * index as f32 + slot * 0.5;
        paint_tide_bar(
            ui,
            tide_rect(cx, mid, bar_w, room, week.added, max, true),
            TEAL,
        );
        paint_tide_bar(
            ui,
            tide_rect(cx, mid, bar_w, room, week.deleted, max, false),
            PINK,
        );
        let label = rect_for_week(rect, slot, index);
        ui.painter().text(
            Pos2::new(cx, rect.bottom() - 2.0),
            Align2::CENTER_BOTTOM,
            &week.label,
            FontId::proportional(11.0),
            MUTED,
        );
        if let Some(pos) = response.hover_pos() {
            if label.contains(pos) {
                tip = Some(tide_tip(week));
            }
        }
    }
    if let Some(tip) = tip {
        response.on_hover_text(tip);
    }
}

fn donut(ui: &mut egui::Ui, snap: &GraphSnap) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 230.0), Sense::hover());
    if snap.langs.is_empty() || snap.lines == 0 {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "No source lines yet",
            FontId::proportional(14.0),
            MUTED,
        );
        return;
    }
    let radius = (rect.height() * 0.42).min(96.0);
    let center = Pos2::new(rect.left() + radius + 8.0, rect.center().y);
    let total = snap.langs.iter().map(|lang| lang.lines).sum::<u64>().max(1);
    let mut angle = -std::f32::consts::FRAC_PI_2;
    for (index, lang) in snap.langs.iter().enumerate() {
        let sweep = (lang.lines as f32 / total as f32) * std::f32::consts::TAU;
        let gap = if sweep > 0.08 { 0.03 } else { 0.0 };
        ring(
            ui,
            center,
            radius * 0.58,
            radius,
            angle + gap,
            angle + sweep - gap,
            PAL[index % PAL.len()],
        );
        angle += sweep;
    }
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        commas(snap.lines),
        FontId::proportional(16.0),
        PAPER,
    );
    let mut y = rect.top() + 8.0;
    for (index, lang) in snap.langs.iter().enumerate() {
        let row = Pos2::new(center.x + radius + 28.0, y);
        let sw = Rect::from_min_size(row, Vec2::new(10.0, 10.0));
        ui.painter()
            .rect_filled(sw, CornerRadius::same(2), PAL[index % PAL.len()]);
        ui.painter().text(
            Pos2::new(sw.right() + 8.0, row.y - 2.0),
            Align2::LEFT_TOP,
            lang_label(lang, total),
            FontId::proportional(13.0),
            PAPER,
        );
        y += 22.0;
        if y > rect.bottom() - 8.0 {
            break;
        }
    }
}

fn bars(ui: &mut egui::Ui, snap: &GraphSnap) {
    if snap.largest.is_empty() {
        ui.label(RichText::new("No checkout had counted source lines.").color(MUTED));
        return;
    }
    let max = snap
        .largest
        .iter()
        .map(|repo| repo.lines)
        .max()
        .unwrap_or(1)
        .max(1);
    for repo in &snap.largest {
        ui.horizontal(|ui| {
            ui.add_sized(
                Vec2::new(150.0, 18.0),
                egui::Label::new(RichText::new(&repo.name).color(PAPER)),
            );
            let (track, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
            ui.painter()
                .rect_filled(track, CornerRadius::same(6), EMPTY);
            let width = (track.width() * repo.lines as f32 / max as f32).max(8.0);
            let fill = Rect::from_min_size(track.left_top(), Vec2::new(width, track.height()));
            ui.painter()
                .rect_filled(fill, CornerRadius::same(6), color_for(&repo.lang));
            ui.painter().text(
                Pos2::new(fill.right() - 8.0, fill.center().y),
                Align2::RIGHT_CENTER,
                commas(repo.lines),
                FontId::proportional(11.0),
                INK,
            );
        });
        ui.add_space(4.0);
    }
}

fn spine(ui: &mut egui::Ui, snap: &GraphSnap) {
    if snap.releases.is_empty() {
        ui.label(RichText::new("No tags in the measured checkouts.").color(MUTED));
        return;
    }
    let mut lanes: Vec<&str> = Vec::new();
    for mark in &snap.releases {
        if lanes.iter().any(|lane| *lane == mark.repo) {
            continue;
        }
        lanes.push(mark.repo.as_str());
        if lanes.len() == 8 {
            break;
        }
    }
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 36.0 + lanes.len() as f32 * 28.0),
        Sense::hover(),
    );
    let left = rect.left() + 132.0;
    let right = rect.right() - 8.0;
    let (min_day, max_day) = span(&snap.releases);
    let mut tip = None;
    for (index, lane) in lanes.iter().enumerate() {
        let y = rect.top() + 18.0 + index as f32 * 28.0;
        ui.painter().text(
            Pos2::new(rect.left(), y),
            Align2::LEFT_CENTER,
            lane,
            FontId::proportional(13.0),
            PAPER,
        );
        ui.painter().line_segment(
            [Pos2::new(left, y), Pos2::new(right, y)],
            Stroke::new(1.0, Color32::from_rgb(36, 48, 74)),
        );
        for mark in snap.releases.iter().filter(|mark| mark.repo == *lane) {
            let Some(t) = day_t(&mark.day, &min_day, &max_day) else {
                continue;
            };
            let x = left + (right - left) * t;
            let dot = Pos2::new(x, y);
            ui.painter().circle_filled(dot, 6.0, PAL[index % PAL.len()]);
            if let Some(pos) = response.hover_pos() {
                if pos.distance(dot) < 10.0 {
                    tip = Some(format!("{}  {}  {}", mark.repo, mark.name, mark.day));
                }
            }
        }
    }
    if let Some(tip) = tip {
        response.on_hover_text(tip);
    }
}

fn sky(ui: &mut egui::Ui, snap: &GraphSnap) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 280.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(12), Color32::from_rgb(6, 8, 18));
    if snap.stars.is_empty() {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "No commits in this window yet",
            FontId::proportional(14.0),
            MUTED,
        );
        return;
    }
    let center = rect.center();
    let max_r = (rect.width().min(rect.height()) * 0.46).max(40.0);
    let spread = max_r / (snap.stars.len() as f32).sqrt();
    let mut tip = None;
    for (index, star) in snap.stars.iter().enumerate() {
        let angle = index as f32 * 2.399963;
        let orbit = 16.0 + (index as f32).sqrt() * spread;
        let at = center + Vec2::new(angle.cos(), angle.sin()) * orbit.min(max_r);
        let radius = (3.2 + (star.commits as f32).sqrt() * 1.15).min(16.0);
        let color = if star.lang.is_empty() {
            AMBER
        } else {
            color_for(&star.lang)
        };
        ui.painter()
            .circle_filled(at, radius * 2.4, color.gamma_multiply(0.28));
        ui.painter().circle_filled(at, radius, color);
        if index < 8 {
            ui.painter().text(
                at + Vec2::new(radius + 4.0, -radius),
                Align2::LEFT_BOTTOM,
                &star.name,
                FontId::proportional(11.0),
                PAPER,
            );
        }
        if let Some(pos) = response.hover_pos() {
            if pos.distance(at) <= radius + 6.0 {
                tip = Some(format!(
                    "{}  ·  {} commits  ·  {} lines  ·  {}",
                    star.name,
                    star.commits,
                    commas(star.lines),
                    if star.lang.is_empty() {
                        "mixed"
                    } else {
                        star.lang.as_str()
                    }
                ));
            }
        }
    }
    if let Some(tip) = tip {
        response.on_hover_text(tip);
    }
}

fn authors(ui: &mut egui::Ui, snap: &GraphSnap) {
    if snap.authors.is_empty() {
        ui.label(RichText::new("No author names in this window.").color(MUTED));
        return;
    }
    let max = snap
        .authors
        .iter()
        .map(|author| author.commits)
        .max()
        .unwrap_or(1)
        .max(1);
    for (index, author) in snap.authors.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.add_sized(
                Vec2::new(150.0, 18.0),
                egui::Label::new(RichText::new(&author.name).color(PAPER)),
            );
            let (track, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
            ui.painter()
                .rect_filled(track, CornerRadius::same(6), EMPTY);
            let width = (track.width() * author.commits as f32 / max as f32).max(8.0);
            let fill = Rect::from_min_size(track.left_top(), Vec2::new(width, track.height()));
            ui.painter()
                .rect_filled(fill, CornerRadius::same(6), PAL[index % PAL.len()]);
            ui.painter().text(
                Pos2::new(fill.right() - 8.0, fill.center().y),
                Align2::RIGHT_CENTER,
                author.commits.to_string(),
                FontId::proportional(11.0),
                INK,
            );
        });
        ui.add_space(4.0);
    }
}

fn weekdays(ui: &mut egui::Ui, snap: &GraphSnap) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 200.0), Sense::hover());
    let labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let max = snap.weekdays.iter().copied().max().unwrap_or(1).max(1);
    let slot = rect.width() / 7.0;
    let bar_w = (slot * 0.55).max(8.0);
    let mut tip = None;
    for (index, (label, count)) in labels.iter().zip(snap.weekdays).enumerate() {
        let cx = rect.left() + slot * index as f32 + slot * 0.5;
        let h = (count as f32 / max as f32) * (rect.height() - 28.0);
        let bar = Rect::from_min_size(
            Pos2::new(cx - bar_w * 0.5, rect.bottom() - 22.0 - h),
            Vec2::new(bar_w, h.max(2.0)),
        );
        let color = if count == max && count > 0 {
            AMBER
        } else {
            TEAL
        };
        ui.painter().rect_filled(bar, CornerRadius::same(6), color);
        ui.painter().text(
            Pos2::new(cx, rect.bottom() - 2.0),
            Align2::CENTER_BOTTOM,
            *label,
            FontId::proportional(12.0),
            MUTED,
        );
        let hit = Rect::from_min_size(
            Pos2::new(rect.left() + slot * index as f32, rect.top()),
            Vec2::new(slot, rect.height()),
        );
        if let Some(pos) = response.hover_pos() {
            if hit.contains(pos) {
                tip = Some(format!("{label}  ·  {count} commits"));
            }
        }
    }
    if let Some(tip) = tip {
        response.on_hover_text(tip);
    }
}

fn card(ui: &mut egui::Ui, title: &str, subtitle: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(INK)
        .inner_margin(Margin::same(16))
        .corner_radius(CornerRadius::same(16))
        .show(ui, |ui| {
            ui.label(RichText::new(title).size(18.0).color(PAPER).strong());
            ui.label(RichText::new(subtitle).size(12.5).color(MUTED));
            ui.add_space(10.0);
            body(ui);
        });
}

fn swatch(ui: &mut egui::Ui, color: Color32, label: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(3), color);
    ui.label(RichText::new(label).color(PAPER));
}

fn heat(commits: u32, max: u32) -> Color32 {
    if commits == 0 || max == 0 {
        return EMPTY;
    }
    let t = (commits as f32 / max as f32).sqrt();
    if t < 0.45 {
        mix(Color32::from_rgb(14, 74, 92), TEAL, t / 0.45)
    } else if t < 0.75 {
        mix(TEAL, AMBER, (t - 0.45) / 0.30)
    } else {
        mix(AMBER, PINK, (t - 0.75) / 0.25)
    }
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |from: u8, to: u8| (from as f32 + (to as f32 - from as f32) * t).round() as u8;
    Color32::from_rgb(ch(a.r(), b.r()), ch(a.g(), b.g()), ch(a.b(), b.b()))
}

fn tide_rect(cx: f32, mid: f32, width: f32, room: f32, value: u64, max: u64, up: bool) -> Rect {
    let h = (value as f32 / max as f32) * room;
    if up {
        Rect::from_min_size(Pos2::new(cx - width * 0.5, mid - h), Vec2::new(width, h))
    } else {
        Rect::from_min_size(Pos2::new(cx - width * 0.5, mid), Vec2::new(width, h))
    }
}

fn paint_tide_bar(ui: &mut egui::Ui, rect: Rect, color: Color32) {
    if rect.height() < 1.0 {
        return;
    }
    ui.painter().rect_filled(rect, CornerRadius::same(4), color);
}

fn rect_for_week(rect: Rect, slot: f32, index: usize) -> Rect {
    Rect::from_min_size(
        Pos2::new(rect.left() + slot * index as f32, rect.top()),
        Vec2::new(slot, rect.height()),
    )
}

fn tide_tip(week: &WeekFlow) -> String {
    format!(
        "week of {}  ·  {} commits  ·  +{} / -{}",
        week.label,
        week.commits,
        commas(week.added),
        commas(week.deleted)
    )
}

fn ring(ui: &mut egui::Ui, center: Pos2, r_in: f32, r_out: f32, a0: f32, a1: f32, color: Color32) {
    if a1 - a0 <= 0.01 {
        return;
    }
    let steps = 28;
    let mut mesh = Mesh::default();
    for step in 0..steps {
        let t0 = step as f32 / steps as f32;
        let t1 = (step + 1) as f32 / steps as f32;
        let b0 = a0 + (a1 - a0) * t0;
        let b1 = a0 + (a1 - a0) * t1;
        let point = |angle: f32, radius: f32| center + Vec2::new(angle.cos(), angle.sin()) * radius;
        let base = mesh.vertices.len() as u32;
        mesh.vertices
            .push(Vertex::untextured(point(b0, r_out), color));
        mesh.vertices
            .push(Vertex::untextured(point(b0, r_in), color));
        mesh.vertices
            .push(Vertex::untextured(point(b1, r_out), color));
        mesh.vertices
            .push(Vertex::untextured(point(b1, r_in), color));
        mesh.add_triangle(base, base + 2, base + 1);
        mesh.add_triangle(base + 2, base + 3, base + 1);
    }
    ui.painter().add(Shape::mesh(mesh));
}

fn lang_label(lang: &LangShare, total: u64) -> String {
    let pct = (lang.lines as f32 / total as f32) * 100.0;
    format!("{}   {}   {pct:.0}%", lang.name, commas(lang.lines))
}

fn color_for(lang: &str) -> Color32 {
    match lang {
        "Rust" => TEAL,
        "Python" => AMBER,
        "TypeScript" | "JavaScript" => SKY,
        "PowerShell" => VIOLET,
        "Go" => Color32::from_rgb(52, 211, 153),
        "C" | "C++" => Color32::from_rgb(251, 146, 60),
        _ => PINK,
    }
}

fn span(marks: &[ReleaseMark]) -> (String, String) {
    let mut min = marks[0].day.clone();
    let mut max = marks[0].day.clone();
    for mark in marks {
        if mark.day < min {
            min = mark.day.clone();
        }
        if mark.day > max {
            max = mark.day.clone();
        }
    }
    (min, max)
}

fn day_t(day: &str, min: &str, max: &str) -> Option<f32> {
    let day = parse_day(day)?;
    let min = parse_day(min)?;
    let max = parse_day(max)?;
    let span = (max - min).num_days();
    if span <= 0 {
        return Some(0.5);
    }
    Some(((day - min).num_days() as f32 / span as f32).clamp(0.0, 1.0))
}

fn parse_day(text: &str) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
}

fn commas(value: u64) -> String {
    let raw = value.to_string();
    let mut out = String::new();
    for (index, ch) in raw.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}
