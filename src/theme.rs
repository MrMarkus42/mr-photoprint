//! A modern look: dark by default, with a light variant; one accent gradient (indigo → cyan),
//! rounded cards, Segoe UI where available.

use egui::{Color32, Context, FontFamily, FontId, Painter, Pos2, Rect, Shape, Stroke, epaint::Vertex, pos2, vec2};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

static DARK: AtomicBool = AtomicBool::new(true);

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface2: Color32,
    pub border: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub accent: Color32,
    pub accent2: Color32,
    pub ok: Color32,
    pub bad: Color32,
    pub warn: Color32,
    pub stage: Color32,
}

pub const DARK_P: Palette = Palette {
    bg: Color32::from_rgb(13, 16, 22),
    surface: Color32::from_rgb(22, 27, 36),
    surface2: Color32::from_rgb(31, 37, 49),
    border: Color32::from_rgb(43, 51, 67),
    text: Color32::from_rgb(232, 236, 243),
    dim: Color32::from_rgb(139, 149, 167),
    accent: Color32::from_rgb(99, 102, 241),
    accent2: Color32::from_rgb(34, 211, 238),
    ok: Color32::from_rgb(52, 211, 153),
    bad: Color32::from_rgb(248, 113, 113),
    warn: Color32::from_rgb(251, 191, 36),
    stage: Color32::from_rgb(10, 12, 17),
};

pub const LIGHT_P: Palette = Palette {
    bg: Color32::from_rgb(243, 245, 250),
    surface: Color32::from_rgb(255, 255, 255),
    surface2: Color32::from_rgb(236, 240, 247),
    border: Color32::from_rgb(218, 224, 235),
    text: Color32::from_rgb(17, 24, 39),
    dim: Color32::from_rgb(100, 110, 128),
    accent: Color32::from_rgb(79, 70, 229),
    accent2: Color32::from_rgb(6, 182, 212),
    ok: Color32::from_rgb(5, 150, 105),
    bad: Color32::from_rgb(220, 38, 38),
    warn: Color32::from_rgb(217, 119, 6),
    stage: Color32::from_rgb(226, 231, 240),
};

pub fn p() -> Palette {
    if DARK.load(Ordering::Relaxed) { DARK_P } else { LIGHT_P }
}

pub fn is_dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

pub fn set_dark(ctx: &Context, dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
    apply(ctx);
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn install_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut bold_family = Vec::new();
    #[cfg(windows)]
    {
        if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
            fonts.font_data.insert("segoe".into(), Arc::new(egui::FontData::from_owned(bytes)));
            fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "segoe".into());
        }
        if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\seguisb.ttf").or_else(|_| std::fs::read(r"C:\Windows\Fonts\segoeuib.ttf")) {
            fonts.font_data.insert("segoe_bold".into(), Arc::new(egui::FontData::from_owned(bytes)));
            bold_family.push("segoe_bold".to_string());
        }
    }
    let mut rest = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    bold_family.append(&mut rest);
    fonts.families.insert(FontFamily::Name("bold".into()), bold_family);
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &Context) {
    let c = p();
    let dark = is_dark();
    ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
    ctx.all_styles_mut(|s| {
        use egui::TextStyle::*;
        s.text_styles.insert(Body, FontId::proportional(14.5));
        s.text_styles.insert(Button, FontId::proportional(14.5));
        s.text_styles.insert(Heading, bold(20.0));
        s.text_styles.insert(Small, FontId::proportional(12.5));
        s.spacing.item_spacing = vec2(8.0, 7.0);
        s.spacing.button_padding = vec2(12.0, 5.0);
        s.spacing.interact_size.y = 30.0;
        s.spacing.combo_height = 420.0;
        let v = &mut s.visuals;
        v.panel_fill = c.bg;
        v.window_fill = c.surface;
        v.extreme_bg_color = c.surface2;
        v.faint_bg_color = c.surface2;
        v.window_stroke = Stroke::new(1.0, c.border);
        v.window_corner_radius = egui::CornerRadius::same(14);
        v.menu_corner_radius = egui::CornerRadius::same(10);
        v.override_text_color = Some(c.text);
        v.hyperlink_color = c.accent2;
        v.selection.bg_fill = c.accent;
        v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        let r = egui::CornerRadius::same(8);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, c.border);
        v.widgets.noninteractive.corner_radius = r;
        for (w, fill) in [
            (&mut v.widgets.inactive, c.surface2),
            (&mut v.widgets.hovered, c.surface2.lerp_to_gamma(c.accent, 0.18)),
            (&mut v.widgets.active, c.surface2.lerp_to_gamma(c.accent, 0.32)),
            (&mut v.widgets.open, c.surface2),
        ] {
            w.bg_fill = fill;
            w.weak_bg_fill = fill;
            w.corner_radius = r;
            w.bg_stroke = Stroke::new(1.0, c.border);
            w.fg_stroke = Stroke::new(1.0, c.text);
        }
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, c.accent);
    });
}

/// Rounded rectangle filled with a left-to-right gradient.
pub fn gradient_rect(painter: &Painter, rect: Rect, radius: f32, c0: Color32, c1: Color32) {
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let mut pts: Vec<Pos2> = Vec::new();
    let corners = [
        (pos2(rect.right() - r, rect.top() + r), -90.0f32),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), 90.0),
        (pos2(rect.left() + r, rect.top() + r), 180.0),
    ];
    for (c, start) in corners {
        for k in 0..=6 {
            let a = (start + k as f32 * 15.0).to_radians();
            pts.push(c + vec2(a.cos(), a.sin()) * r);
        }
    }
    let col = |x: f32| {
        let t = ((x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        c0.lerp_to_gamma(c1, t)
    };
    let mut mesh = egui::Mesh::default();
    let center = rect.center();
    mesh.vertices.push(Vertex { pos: center, uv: egui::epaint::WHITE_UV, color: col(center.x) });
    for q in &pts {
        mesh.vertices.push(Vertex { pos: *q, uv: egui::epaint::WHITE_UV, color: col(q.x) });
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.indices.extend_from_slice(&[0, 1 + i, 1 + (i + 1) % n]);
    }
    painter.add(Shape::mesh(mesh));
}
