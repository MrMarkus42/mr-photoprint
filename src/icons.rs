//! Our own line icons, drawn with simple shapes (photo pages and plus badges, filled slider knobs).
#![allow(dead_code)]

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2};

fn p(r: Rect, x: f32, y: f32) -> Pos2 {
    pos2(r.left() + x * r.width(), r.top() + y * r.height())
}

fn plus_badge(pt: &Painter, c: Pos2, rad: f32, fg: Color32, bg: Color32) {
    pt.circle_filled(c, rad, fg);
    let s = Stroke::new(rad * 0.28, bg);
    pt.line_segment([c - vec2(rad * 0.55, 0.0), c + vec2(rad * 0.55, 0.0)], s);
    pt.line_segment([c - vec2(0.0, rad * 0.55), c + vec2(0.0, rad * 0.55)], s);
}

/// A small photo symbol (mountain and sun) inside `r`.
fn photo_mark(pt: &Painter, r: Rect, col: Color32, w: f32) {
    let m = |x, y| p(r, x, y);
    pt.add(Shape::line(vec![m(0.1, 0.85), m(0.4, 0.45), m(0.6, 0.7), m(0.72, 0.55), m(0.9, 0.85)], Stroke::new(w, col)));
    pt.circle_stroke(m(0.7, 0.3), r.width() * 0.09, Stroke::new(w, col));
}

/// Two stacked photo cards with a plus badge.
pub fn add_photos(pt: &Painter, r: Rect, fg: Color32, bg: Color32) {
    let w = r.width() * 0.045;
    let s = Stroke::new(w, fg);
    let back = Rect::from_min_max(p(r, 0.12, 0.14), p(r, 0.62, 0.74));
    let front = Rect::from_min_max(p(r, 0.24, 0.26), p(r, 0.74, 0.86));
    pt.rect_stroke(back, CornerRadius::same((r.width() * 0.05) as u8), s, StrokeKind::Middle);
    pt.rect_filled(front, CornerRadius::same((r.width() * 0.05) as u8), bg);
    pt.rect_stroke(front, CornerRadius::same((r.width() * 0.05) as u8), s, StrokeKind::Middle);
    photo_mark(pt, front.shrink(front.width() * 0.12), fg, w * 0.8);
    plus_badge(pt, p(r, 0.78, 0.78), r.width() * 0.16, fg, bg);
}

/// A folder with a plus badge.
pub fn add_folder(pt: &Painter, r: Rect, fg: Color32, bg: Color32) {
    let w = r.width() * 0.045;
    let s = Stroke::new(w, fg);
    let pts = vec![
        p(r, 0.1, 0.22),
        p(r, 0.36, 0.22),
        p(r, 0.44, 0.31),
        p(r, 0.82, 0.31),
        p(r, 0.82, 0.78),
        p(r, 0.1, 0.78),
    ];
    pt.add(Shape::closed_line(pts, s));
    pt.line_segment([p(r, 0.1, 0.4), p(r, 0.82, 0.4)], s);
    photo_mark(pt, Rect::from_min_max(p(r, 0.24, 0.44), p(r, 0.6, 0.74)), fg, w * 0.8);
    plus_badge(pt, p(r, 0.8, 0.78), r.width() * 0.16, fg, bg);
}

/// Three sliders with filled round knobs.
pub fn sliders(pt: &Painter, r: Rect, fg: Color32) {
    let w = r.width() * 0.05;
    for (i, k) in [(0.3, 0.68), (0.5, 0.36), (0.7, 0.56)] {
        let y = i;
        pt.line_segment([p(r, 0.16, y), p(r, 0.84, y)], Stroke::new(w, fg));
        pt.circle_filled(p(r, k, y), r.width() * 0.085, fg);
    }
}

/// A printer with a photo coming out.
pub fn printer(pt: &Painter, r: Rect, fg: Color32, bg: Color32) {
    let body = Rect::from_min_max(p(r, 0.12, 0.36), p(r, 0.88, 0.7));
    pt.rect_filled(body, CornerRadius::same((r.width() * 0.07) as u8), fg);
    let top = Rect::from_min_max(p(r, 0.28, 0.14), p(r, 0.72, 0.36));
    pt.rect_stroke(top, CornerRadius::same(2), Stroke::new(r.width() * 0.045, fg), StrokeKind::Inside);
    let sheet = Rect::from_min_max(p(r, 0.28, 0.58), p(r, 0.72, 0.88));
    pt.rect_filled(sheet, CornerRadius::same(2), bg);
    pt.rect_stroke(sheet, CornerRadius::same(2), Stroke::new(r.width() * 0.04, fg), StrokeKind::Inside);
    photo_mark(pt, sheet.shrink(sheet.width() * 0.12), fg, r.width() * 0.03);
    pt.circle_filled(p(r, 0.78, 0.45), r.width() * 0.03, bg);
}

pub fn gear(pt: &Painter, c: Pos2, rad: f32, fg: Color32, bg: Color32) {
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        let d = Vec2::angled(a);
        pt.line_segment([c + d * rad * 0.6, c + d * rad], Stroke::new(rad * 0.38, fg));
    }
    pt.circle_filled(c, rad * 0.72, fg);
    pt.circle_filled(c, rad * 0.32, bg);
}

/// Header icons: a list with a badge.
pub enum ListBadge {
    RemoveSelected,
    Clear,
    RemovePrinted,
    Reset,
    Preview,
    Menu,
}

pub fn list_icon(pt: &Painter, r: Rect, fg: Color32, bg: Color32, badge: ListBadge) {
    let w = r.width() * 0.07;
    let s = Stroke::new(w, fg);
    let lines: &[(f32, f32)] = match badge {
        ListBadge::Preview | ListBadge::Menu => &[],
        _ => &[(0.25, 0.62), (0.45, 0.48), (0.65, 0.42), (0.85, 0.5)],
    };
    for (y, x1) in lines {
        pt.line_segment([p(r, 0.08, *y), p(r, *x1, *y)], s);
    }
    let c = p(r, 0.72, 0.68);
    let rad = r.width() * 0.25;
    match badge {
        ListBadge::RemoveSelected | ListBadge::Clear => {
            if matches!(badge, ListBadge::Clear) {
                pt.circle_filled(c, rad, fg);
            } else {
                pt.circle_stroke(c, rad, Stroke::new(w, fg));
            }
            let col = if matches!(badge, ListBadge::Clear) { bg } else { fg };
            let d = rad * 0.45;
            pt.line_segment([c + vec2(-d, -d), c + vec2(d, d)], Stroke::new(w, col));
            pt.line_segment([c + vec2(d, -d), c + vec2(-d, d)], Stroke::new(w, col));
        }
        ListBadge::RemovePrinted => {
            pt.circle_filled(c, rad, fg);
            pt.add(Shape::line(
                vec![c + vec2(-rad * 0.5, 0.0), c + vec2(-rad * 0.1, rad * 0.4), c + vec2(rad * 0.5, -rad * 0.4)],
                Stroke::new(w, bg),
            ));
        }
        ListBadge::Reset => {
            pt.circle_filled(c, rad, fg);
            let n = 12;
            let pts: Vec<Pos2> = (0..=n)
                .map(|i| {
                    let a = -0.3 + i as f32 / n as f32 * 4.6;
                    c + Vec2::angled(a) * rad * 0.55
                })
                .collect();
            let tip = *pts.last().unwrap();
            pt.add(Shape::line(pts, Stroke::new(w * 0.9, bg)));
            pt.circle_filled(tip, w * 0.9, bg);
        }
        ListBadge::Preview => {
            let c = r.center();
            let pts: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let t = i as f32 / 16.0 * std::f32::consts::PI;
                    c + vec2(-t.cos() * r.width() * 0.42, -t.sin() * r.height() * 0.26)
                })
                .chain((0..=16).map(|i| {
                    let t = i as f32 / 16.0 * std::f32::consts::PI;
                    c + vec2(t.cos() * r.width() * 0.42, t.sin() * r.height() * 0.26)
                }))
                .collect();
            pt.add(Shape::closed_line(pts, Stroke::new(w, fg)));
            pt.circle_filled(c, r.width() * 0.14, fg);
        }
        ListBadge::Menu => {
            for x in [0.25, 0.5, 0.75] {
                pt.circle_filled(p(r, x, 0.5), r.width() * 0.08, fg);
            }
        }
    }
}

/// A simple photo symbol: a rounded frame with a hill and a sun.
pub fn photo_glyph(pt: &Painter, r: Rect, col: Color32) {
    let w = r.width() * 0.09;
    pt.rect_stroke(r, CornerRadius::same((r.width() * 0.16) as u8), Stroke::new(w, col), StrokeKind::Inside);
    photo_mark(pt, r.shrink(r.width() * 0.14), col, w * 0.9);
}

pub fn sun(pt: &Painter, r: Rect, col: Color32) {
    let c = r.center();
    let rad = r.width() * 0.2;
    pt.circle_filled(c, rad, col);
    for i in 0..8 {
        let d = Vec2::angled(i as f32 * std::f32::consts::TAU / 8.0);
        pt.line_segment([c + d * rad * 1.5, c + d * rad * 2.2], Stroke::new(r.width() * 0.07, col));
    }
}

pub fn moon(pt: &Painter, r: Rect, col: Color32) {
    let c = r.center();
    let rad = r.width() * 0.36;
    let n = 24;
    let mut pts = Vec::new();
    for i in 0..=n {
        let a = -2.2 + i as f32 / n as f32 * 4.4;
        pts.push(c + Vec2::angled(a + std::f32::consts::PI) * rad);
    }
    let off = vec2(rad * 0.55, -rad * 0.25);
    for i in (0..=n).rev() {
        let a = -1.75 + i as f32 / n as f32 * 3.5;
        pts.push(c + off + Vec2::angled(a + std::f32::consts::PI) * rad * 0.82);
    }
    pt.add(Shape::closed_line(pts, Stroke::new(r.width() * 0.08, col)));
}

/// A question mark in a ring (help).
pub fn help(pt: &Painter, r: Rect, col: Color32) {
    let c = r.center();
    let rad = r.width() * 0.42;
    pt.circle_stroke(c, rad, Stroke::new(r.width() * 0.08, col));
    pt.text(c + vec2(0.0, r.width() * 0.02), egui::Align2::CENTER_CENTER, "?", crate::theme::bold(r.width() * 0.62), col);
}

/// The window icon: the Mr. PhotoPrint logo (the same drawing as the .exe icon).
pub fn window_icon() -> egui::IconData {
    let n = 128u32;
    egui::IconData { rgba: crate::logo::logo_rgba(n), width: n, height: n }
}
