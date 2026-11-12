#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Mr. PhotoPrint – many photos onto photo paper, each fitted to the paper size.

mod app;
mod i18n;
mod icons;
mod layout;
mod log;
mod logo;
mod model;
mod printer;
mod render;
mod settings_ui;
mod theme;
mod update;
mod update_ui;
mod widgets;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    log::install_panic_hook();
    log::info(&format!("start, version {}", env!("CARGO_PKG_VERSION")));
    let mut screenshot: Option<PathBuf> = None;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut size = [1180.0f32, 780.0];
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--screenshot" {
            screenshot = args.next().map(PathBuf::from);
        } else if a == "--lang" {
            match args.next().as_deref() {
                Some("de") => i18n::force(i18n::Lang::De),
                Some("en") => i18n::force(i18n::Lang::En),
                _ => {}
            }
        } else if a == "--theme" {
            app::DEMO_LIGHT.store(args.next().as_deref() == Some("light"), std::sync::atomic::Ordering::Relaxed);
        } else if a == "--grid" {
            // e.g. --grid 2x2 (screenshot mode)
            if let Some((c, r)) = args.next().as_deref().and_then(|s| s.split_once('x')) {
                if let (Ok(c), Ok(r)) = (c.parse::<u32>(), r.parse::<u32>()) {
                    app::DEMO_GRID.store(c.clamp(1, 9) * 10 + r.clamp(1, 9), std::sync::atomic::Ordering::Relaxed);
                }
            }
        } else if a == "--size" {
            // e.g. --size 900x600 (window size in points)
            if let Some((w, h)) = args.next().as_deref().and_then(|s| s.split_once('x')) {
                if let (Ok(w), Ok(h)) = (w.parse::<f32>(), h.parse::<f32>()) {
                    size = [w.clamp(800.0, 4000.0), h.clamp(560.0, 3000.0)];
                }
            }
        } else {
            files.push(PathBuf::from(a));
        }
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(format!("Mr. PhotoPrint {}", env!("CARGO_PKG_VERSION")))
        .with_inner_size(size)
        .with_min_inner_size([800.0, 560.0])
        .with_drag_and_drop(true)
        .with_icon(std::sync::Arc::new(icons::window_icon()));
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|b| {
            use eframe::egui_winit::winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    eframe::run_native(
        "Mr. PhotoPrint",
        options,
        Box::new(move |cc| {
            let mut app = app::FotoApp::new(cc, screenshot);
            if !files.is_empty() {
                app.add_paths(files);
            }
            Ok(Box::new(app))
        }),
    )
}
