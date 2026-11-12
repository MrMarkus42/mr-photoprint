//! Small modern controls: gradient buttons, segmented choices, a switch, icon buttons, cards.

use crate::theme::{self, bold, p};
use egui::{Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, vec2};

pub fn card() -> egui::Frame {
    let c = p();
    egui::Frame::new()
        .fill(c.surface)
        .stroke(Stroke::new(1.0, c.border))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(egui::Margin::same(18))
}

/// A big gradient button. `enabled` false draws it flat and inactive.
pub fn primary(ui: &mut Ui, text: &str, size: Vec2, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let c = p();
    let pt = ui.painter();
    if enabled {
        let (mut a, mut b) = (c.accent, c.accent2);
        if resp.hovered() {
            a = a.lerp_to_gamma(Color32::WHITE, 0.1);
            b = b.lerp_to_gamma(Color32::WHITE, 0.1);
        }
        if resp.is_pointer_button_down_on() {
            a = a.lerp_to_gamma(Color32::BLACK, 0.12);
            b = b.lerp_to_gamma(Color32::BLACK, 0.12);
        }
        // soft glow
        pt.rect_filled(rect.expand(2.0).translate(vec2(0.0, 3.0)), CornerRadius::same(14), c.accent.gamma_multiply(0.18));
        theme::gradient_rect(pt, rect, 12.0, a, b);
        pt.text(rect.center(), Align2::CENTER_CENTER, text, bold((size.y * 0.34).clamp(14.0, 19.0)), Color32::WHITE);
    } else {
        pt.rect_filled(rect, CornerRadius::same(12), c.surface2);
        pt.text(rect.center(), Align2::CENTER_CENTER, text, bold((size.y * 0.34).clamp(14.0, 19.0)), c.dim);
    }
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// A quiet button with a border.
pub fn secondary(ui: &mut Ui, text: &str, size: Vec2) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let c = p();
    let fill = if resp.hovered() { c.surface2.lerp_to_gamma(c.accent, 0.15) } else { c.surface2 };
    ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
    ui.painter().rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, if resp.hovered() { c.accent } else { c.border }), StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, FontId::proportional(14.5), c.text);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Choose one of several options laid out side by side.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)], height: f32) -> bool {
    let c = p();
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(10), c.surface2);
    ui.painter().rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, c.border), StrokeKind::Inside);
    let n = options.len().max(1) as f32;
    let w = rect.width() / n;
    let mut changed = false;
    for (i, (v, label)) in options.iter().enumerate() {
        let seg = Rect::from_min_size(rect.min + vec2(w * i as f32, 0.0), vec2(w, rect.height())).shrink(3.0);
        let resp = ui.interact(seg, ui.scope_id().with(("seg", label, i)), Sense::click());
        let selected = *value == *v;
        if selected {
            theme::gradient_rect(ui.painter(), seg, 8.0, c.accent, c.accent2);
        } else if resp.hovered() {
            ui.painter().rect_filled(seg, CornerRadius::same(8), c.border.gamma_multiply(0.6));
        }
        let col = if selected { Color32::WHITE } else { c.text };
        // Shrink the text (down to 10 px) when the window is narrow, so labels never get cut off.
        let mk = |s: f32| if selected { bold(s) } else { FontId::proportional(s) };
        let room = seg.width() - 8.0;
        let natural = ui.painter().layout_no_wrap(label.to_string(), mk(13.5), col).size().x;
        let size = if natural > room { (13.5 * room / natural).max(10.0) } else { 13.5 };
        ui.painter().with_clip_rect(seg).text(seg.center(), Align2::CENTER_CENTER, *label, mk(size), col);
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !selected {
            *value = *v;
            changed = true;
        }
    }
    changed
}

/// An on/off switch with a label to its left.
pub fn switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let c = p();
    let h: f32 = 22.0;
    let w = 40.0;
    let galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(14.5), c.text);
    let size = vec2(galley.size().x + 10.0 + w, h.max(galley.size().y));
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
    }
    ui.painter().galley(rect.left_center() - vec2(0.0, galley.size().y / 2.0), galley, c.text);
    let track = Rect::from_min_size(rect.right_center() - vec2(w, h / 2.0), vec2(w, h));
    let t = ui.ctx().animate_bool(resp.id, *on);
    if *on {
        theme::gradient_rect(ui.painter(), track, h / 2.0, c.accent, c.accent2);
    } else {
        ui.painter().rect_filled(track, CornerRadius::same(11), c.border);
    }
    let knob = egui::pos2(track.left() + h / 2.0 + t * (w - h), track.center().y);
    ui.painter().circle_filled(knob, h / 2.0 - 3.0, Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A square button that draws its own icon.
pub fn icon_button(ui: &mut Ui, size: f32, tip: &str, draw: impl FnOnce(&egui::Painter, Rect, Color32)) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let c = p();
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(10), c.surface2);
    }
    draw(ui.painter(), rect.shrink(size * 0.22), if resp.hovered() { c.text } else { c.dim.lerp_to_gamma(c.text, 0.4) });
    resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A small coloured label.
pub fn chip(painter: &egui::Painter, at: egui::Pos2, align: Align2, text: &str, fg: Color32, bg: Color32) -> Rect {
    let g = painter.layout_no_wrap(text.to_string(), bold(11.5), fg);
    let size = g.size() + vec2(14.0, 6.0);
    let rect = align.anchor_size(at, size);
    painter.rect_filled(rect, CornerRadius::same(255), bg);
    painter.galley(rect.center() - g.size() / 2.0, g, fg);
    rect
}

/// A field label above a control.
pub fn label(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(12.5).color(p().dim));
}
