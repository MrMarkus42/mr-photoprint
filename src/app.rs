//! The main window, modern style: a top bar (logo, printer, theme), the photo gallery in the
//! middle and the print setup with a live sheet preview on the right.

use crate::i18n::{t, ts};
use crate::icons;
use crate::layout::{self, PageInfo, Sheet};
use crate::model::*;
use crate::printer::{self, Caps, EXPORT_PRINTER, JobControl, JobEvent};
use crate::render::{Loaded, Loader};
use crate::theme::{self, bold, p};
use crate::widgets;
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

#[derive(Default)]
pub struct Printers {
    pub list: Vec<String>,
    pub default: Option<String>,
    pub caps: HashMap<String, Caps>,
    pub devmodes: HashMap<String, Vec<u8>>,
    pub pages: HashMap<(String, Option<i16>, Option<i16>, Option<u32>, Option<bool>, u32), PageInfo>,
    pub generation: u32,
}

impl Printers {
    pub fn caps(&mut self, name: &str) -> &Caps {
        if !self.caps.contains_key(name) {
            let c = if name == EXPORT_PRINTER || name.is_empty() {
                Caps { papers: printer::export_papers(), ..Default::default() }
            } else {
                printer::caps(name, self.devmodes.get(name).map(|v| v.as_slice()))
            };
            self.caps.insert(name.to_string(), c);
        }
        &self.caps[name]
    }

    pub fn page(&mut self, name: &str, s: &PhotoSettings, landscape: Option<bool>, export_dpi: u32) -> PageInfo {
        let paper = s.paper;
        let key = (name.to_string(), paper, s.tray, s.media, landscape, self.generation);
        if let Some(p) = self.pages.get(&key) {
            return *p;
        }
        let info = if name == EXPORT_PRINTER || name.is_empty() {
            let papers = printer::export_papers();
            let p = papers.iter().find(|p| Some(p.id) == paper).unwrap_or(&papers[7]);
            let (w, h) = printer::orient(p.w_mm, p.h_mm, landscape);
            PageInfo::plain(w, h, export_dpi as f32)
        } else {
            printer::page_info(name, self.devmodes.get(name).map(|v| v.as_slice()), paper, s.tray, s.media, landscape)
        };
        self.pages.insert(key, info);
        info
    }

    pub fn paper_name(&mut self, name: &str, id: Option<i16>) -> String {
        match id {
            None => t("Wie beim Drucker", "As set in the printer").into(),
            Some(id) => self
                .caps(name)
                .papers
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| format!("{} {id}", t("Format", "Size"))),
        }
    }
}

#[derive(Clone)]
pub enum Target {
    Global,
    Items(Vec<usize>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    General,
    Layout,
    Collage,
    Output,
    Program,
    Help,
    About,
}

pub struct Draft {
    pub target: Target,
    pub job: JobSettings,
    pub photo: PhotoSettings,
    pub sheet: usize,
    pub tab: Tab,
}

pub struct Running {
    pub rx: mpsc::Receiver<JobEvent>,
    pub ctl: Arc<JobControl>,
    pub done: usize,
    pub total: usize,
    pub map: Vec<usize>,
}

pub struct Shot {
    pub path: PathBuf,
    pub requested: bool,
    pub done: Arc<AtomicBool>,
}

pub struct FotoApp {
    pub job: JobSettings,
    pub items: Vec<Item>,
    pub printers: Printers,
    pub thumbs: HashMap<PathBuf, egui::TextureHandle>,
    pub loader: Loader,
    pub draft: Option<Draft>,
    pub running: Option<Running>,
    pub message: String,
    pub anchor: Option<usize>,
    pub shot: Option<Shot>,
    pub frames: u64,
    pub hwnd: isize,
    pub grid: bool,
    pub sheet_idx: usize,
    pub focus_item: Option<usize>,
    /// Lowest pixels-per-inch each photo gets on paper (item index → dpi), from the last plan.
    pub sharpness: HashMap<usize, f32>,
    pub dark: bool,
    pub logo: egui::TextureHandle,
    /// Language chosen in the settings; None = as Windows.
    pub lang: Option<crate::i18n::Lang>,
    pub updates: crate::update::UpdatePrefs,
    pub updater: crate::update::Updater,
    /// The update window is open.
    pub update_dialog: bool,
    /// Start the downloaded installer when the window closes ("install on exit").
    pub install_on_exit: bool,
    pub screenshot_mode: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
struct Stored {
    version: u32,
    job: JobSettings,
    dark: Option<bool>,
    grid: Option<bool>,
    lang: Option<crate::i18n::Lang>,
    updates: crate::update::UpdatePrefs,
    /// Printer settings made in the driver's own dialog (gear icon), per printer, as hex.
    drivers: Vec<(String, String)>,
}

/// Screenshot mode only: light theme, and a grid of photos per sheet (columns * 10 + rows).
pub static DEMO_LIGHT: AtomicBool = AtomicBool::new(false);
pub static DEMO_GRID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("Mr. PhotoPrint").join("einstellungen.json"))
}

impl FotoApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        // Screenshots never show this PC's own settings (printer names etc.): neutral defaults.
        let stored: Stored = if screenshot.is_some() {
            Stored { dark: Some(!DEMO_LIGHT.load(Ordering::Relaxed)), ..Default::default() }
        } else {
            settings_path()
            .and_then(|p| std::fs::metadata(&p).ok().filter(|m| m.len() < 1_000_000).map(|_| p))
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
        };
        let dark = stored.dark.unwrap_or(true);
        crate::i18n::set(crate::i18n::resolve(stored.lang));
        theme::install_fonts(&cc.egui_ctx);
        theme::set_dark(&cc.egui_ctx, dark);
        // The scale follows the window size (auto_zoom); keyboard zoom would fight it.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let mut job = stored.job.sanitize();
        let (list, default) = printer::list_printers();
        let mut printers = Printers { list, default, ..Default::default() };
        // Driver settings from the last session – checked like any file from outside.
        for (name, hex) in stored.drivers.iter().take(MAX_STORED_DRIVERS) {
            if printers.list.contains(name) {
                if let Some(dm) = from_hex(hex).and_then(|b| printer::valid_devmode(&b)) {
                    printers.devmodes.insert(name.clone(), dm);
                }
            }
        }
        printers.list.push(EXPORT_PRINTER.to_string());
        if job.printer.is_empty() || !printers.list.contains(&job.printer) {
            job.printer = printers.default.clone().unwrap_or_else(|| printers.list[0].clone());
        }
        if screenshot.is_some() {
            job.printer = EXPORT_PRINTER.to_string();
            let g = DEMO_GRID.load(Ordering::Relaxed);
            if g > 0 {
                job.cols = g / 10;
                job.rows = g % 10;
                job.photo.scale = ScaleMode::Fill;
            }
        }
        #[allow(unused_mut)]
        let mut hwnd = 0isize;
        #[cfg(windows)]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(h) = cc.window_handle() {
                if let RawWindowHandle::Win32(w) = h.as_raw() {
                    hwnd = w.hwnd.get();
                }
            }
        }
        let screenshot_mode = screenshot.is_some();
        let updater = crate::update::Updater::new(cc.egui_ctx.clone());
        let mut updates = stored.updates;
        // Look for a new version in the background when it is due (never in screenshot mode and
        // never before the person has agreed).
        if !screenshot_mode && updates.asked && crate::update::key_configured() && updates.due(crate::update::now_secs()) {
            updates.last_check = crate::update::now_secs();
            updater.check(updates.mode != crate::update::UpdateMode::Notify);
        }
        let app = Self {
            job,
            items: Vec::new(),
            printers,
            thumbs: HashMap::new(),
            loader: Loader::new(cc.egui_ctx.clone()),
            draft: None,
            running: None,
            message: String::new(),
            anchor: None,
            shot: screenshot.map(|path| Shot { path, requested: false, done: Arc::new(AtomicBool::new(false)) }),
            frames: 0,
            hwnd,
            grid: stored.grid.unwrap_or(true),
            sheet_idx: 0,
            focus_item: None,
            sharpness: HashMap::new(),
            dark,
            logo: cc.egui_ctx.load_texture(
                "logo",
                egui::ColorImage::from_rgba_unmultiplied([128, 128], &crate::logo::logo_rgba(128)),
                egui::TextureOptions::LINEAR,
            ),
            lang: stored.lang,
            updates,
            updater,
            update_dialog: false,
            install_on_exit: false,
            screenshot_mode,
        };
        app.save_settings();
        app
    }

    pub fn save_settings(&self) {
        if self.screenshot_mode {
            return;
        }
        if let Some(p) = settings_path() {
            let _ = std::fs::create_dir_all(p.parent().unwrap());
            let s = Stored {
                version: 3,
                job: self.job.clone(),
                dark: Some(self.dark),
                grid: Some(self.grid),
                lang: self.lang,
                updates: self.updates.clone(),
                drivers: self.printers.devmodes.iter().take(MAX_STORED_DRIVERS).map(|(k, v)| (k.clone(), to_hex(v))).collect(),
            };
            if let Ok(s) = serde_json::to_string_pretty(&s) {
                let _ = crate::model::write_atomic(&p, s.as_bytes());
            }
        }
    }

    /// Add files and folders the person chose (local or network – their choice).
    pub fn add_paths(&mut self, paths: Vec<PathBuf>) {
        let mut found = Vec::new();
        let mut skipped = 0;
        for p in paths {
            match std::fs::symlink_metadata(&p) {
                Ok(m) if m.is_dir() => collect_dir(&p, &mut found, 0),
                Ok(m) if m.is_file() && is_photo(&p) => found.push(p),
                _ => skipped += 1,
            }
            if found.len() >= MAX_SCAN_FILES {
                break;
            }
        }
        let mut added = 0;
        for p in found {
            if self.items.iter().any(|i| i.path == p) {
                continue;
            }
            self.loader.request(p.clone());
            self.items.push(Item::new(p));
            added += 1;
        }
        self.message = match (added, skipped) {
            (0, 0) => t("Keine neuen Fotos gefunden.", "No new photos found.").into(),
            (a, 0) => ts(format!("{a} Fotos hinzugefügt."), format!("{a} photos added.")),
            (a, s) => ts(format!("{a} Fotos hinzugefügt, {s} übersprungen."), format!("{a} photos added, {s} skipped.")),
        };
    }

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.loader.rx.try_recv() {
            match msg {
                Loaded::Info { path, px, dpi } => {
                    for it in self.items.iter_mut().filter(|i| i.path == path) {
                        it.px = px;
                        it.dpi = dpi;
                    }
                }
                Loaded::Thumb { path, image } => {
                    let tex = ctx.load_texture(path.to_string_lossy(), image, egui::TextureOptions::LINEAR);
                    self.thumbs.insert(path, tex);
                }
                Loaded::Failed { path, message } => {
                    for it in self.items.iter_mut().filter(|i| i.path == path) {
                        it.status = ItemStatus::Failed;
                        it.checked = false;
                        it.message = message.clone();
                    }
                }
            }
        }
        let mut finished = None;
        if let Some(run) = &mut self.running {
            while let Ok(ev) = run.rx.try_recv() {
                match ev {
                    JobEvent::Sheet { done, total } => {
                        run.done = done;
                        run.total = total;
                    }
                    JobEvent::ItemDone { item, ok, message } => {
                        if !ok {
                            crate::log::warn(&format!("job: {message}"));
                        }
                        if let Some(&idx) = run.map.get(item) {
                            if let Some(it) = self.items.get_mut(idx) {
                                it.status = if ok { ItemStatus::Printed } else { ItemStatus::Failed };
                                it.message = message;
                            }
                        }
                    }
                    JobEvent::Finished { ok, message } => finished = Some((ok, message)),
                }
            }
        }
        if let Some((ok, message)) = finished {
            crate::log::info(&format!("job finished (ok={ok}): {message}"));
            self.running = None;
            for it in &mut self.items {
                if it.status == ItemStatus::Printing {
                    it.status = ItemStatus::Waiting;
                }
            }
            let printed = self.items.iter().filter(|i| i.status == ItemStatus::Printed).count();
            if ok && self.job.remove_printed {
                self.items.retain(|i| i.status != ItemStatus::Printed);
            }
            self.message = if ok {
                ts(format!("Fertig: {printed} Fotos. {message}"), format!("Done: {printed} photos. {message}"))
            } else {
                ts(format!("Abgebrochen: {message}"), format!("Cancelled: {message}"))
            };
        }
    }

    /// Indices of the photos that will be printed, in list order.
    pub fn order(&self) -> Vec<usize> {
        let ok = |i: &Item| i.checked && i.status != ItemStatus::Failed;
        let todo: Vec<usize> = (0..self.items.len()).filter(|&i| ok(&self.items[i]) && self.items[i].status != ItemStatus::Printed).collect();
        if todo.is_empty() { (0..self.items.len()).filter(|&i| ok(&self.items[i])).collect() } else { todo }
    }

    pub fn plan_with(&mut self, job: &JobSettings, items: &[Item], order: &[usize]) -> Vec<Sheet> {
        let printer = job.printer.clone();
        let dpi = export_dpi_for(job);
        // Borderless: use the driver's own way (a borderless paper source or paper size).
        if job.ignore_hard_margins {
            let caps = self.printers.caps(&printer).clone();
            let fix = |s: &PhotoSettings| {
                let (paper, tray) = printer::borderless_variant(&caps, s.paper, s.tray);
                PhotoSettings { paper, tray, ..s.clone() }
            };
            let mut job = job.clone();
            job.photo = fix(&job.photo);
            let items: Vec<Item> = items
                .iter()
                .map(|it| {
                    let mut it = it.clone();
                    it.own = it.own.as_ref().map(&fix);
                    it
                })
                .collect();
            let printers = &mut self.printers;
            return layout::plan_sheets(&items, order, &job, &mut |s, land| printers.page(&printer, s, land, dpi));
        }
        let printers = &mut self.printers;
        layout::plan_sheets(items, order, job, &mut |s, land| printers.page(&printer, s, land, dpi))
    }

    fn start_print(&mut self, ctx: &egui::Context) {
        if self.job.printer == EXPORT_PRINTER && self.job.export_dir.is_empty() {
            match rfd::FileDialog::new().set_title(t("Ordner für die Bilddateien wählen", "Choose a folder for the picture files")).pick_folder() {
                Some(d) => self.job.export_dir = d.to_string_lossy().into_owned(),
                None => return,
            }
        }
        let order = self.order();
        if order.is_empty() {
            return;
        }
        let items: Vec<Item> = order.iter().map(|&i| self.items[i].clone()).collect();
        let local: Vec<usize> = (0..items.len()).collect();
        let job = self.job.clone();
        let sheets = self.plan_with(&job, &items, &local);
        for &i in &order {
            self.items[i].status = ItemStatus::Printing;
            self.items[i].message.clear();
        }
        let total = sheets.len();
        let pj = printer::PrintJob { printer: job.printer.clone(), devmode: self.printers.devmodes.get(&job.printer).cloned(), sheets, items, job };
        let (rx, ctl) = printer::start(pj, ctx.clone());
        self.running = Some(Running { rx, ctl, done: 0, total, map: order });
        self.message.clear();
        self.save_settings();
    }

    pub fn selected(&self) -> Vec<usize> {
        (0..self.items.len()).filter(|&i| self.items[i].selected).collect()
    }

    pub fn open_settings(&mut self, target: Target) {
        let photo = match &target {
            Target::Items(ix) => ix.first().and_then(|&i| self.items[i].own.clone()).unwrap_or_else(|| self.job.photo.clone()),
            Target::Global => self.job.photo.clone(),
        };
        self.draft = Some(Draft { target, job: self.job.clone(), photo, sheet: 0, tab: Tab::General });
    }

    pub fn open_settings_at(&mut self, target: Target, tab: Tab) {
        self.open_settings(target);
        if let Some(d) = &mut self.draft {
            d.tab = tab;
        }
    }

    fn set_own(&mut self, f: impl Fn(&mut PhotoSettings)) {
        let base = self.job.photo.clone();
        for it in self.items.iter_mut().filter(|i| i.selected) {
            let mut s = it.own.clone().unwrap_or_else(|| base.clone());
            f(&mut s);
            it.own = if s == base { None } else { Some(s) };
        }
    }
}

/// The photos a list file names: only local, existing, ordinary photo files (no links, no
/// network paths, no URLs), at most `MAX_SCAN_FILES`. Also returns how many lines were ignored.
fn list_entries(text: &str) -> (Vec<PathBuf>, usize) {
    let mut blocked = 0;
    let mut paths = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()).take(MAX_SCAN_FILES) {
        let pth = PathBuf::from(line);
        let ok = !is_network_path(&pth)
            && !line.contains("://")
            && std::fs::symlink_metadata(&pth).map(|m| m.is_file()).unwrap_or(false)
            && is_photo(&pth);
        if ok {
            paths.push(pth);
        } else {
            blocked += 1;
        }
    }
    (paths, blocked)
}

/// Collect photos below a folder: no shortcuts/links followed (no loops, no surprises),
/// limited depth and count.
fn collect_dir(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > MAX_SCAN_DEPTH || out.len() >= MAX_SCAN_FILES {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<(PathBuf, std::fs::FileType)> = rd.flatten().filter_map(|e| e.file_type().ok().map(|t| (e.path(), t))).collect();
    entries.sort_by_key(|(p, _)| p.to_string_lossy().to_lowercase());
    for (p, t) in entries {
        if out.len() >= MAX_SCAN_FILES {
            return;
        }
        if t.is_symlink() {
            continue;
        }
        if t.is_dir() {
            collect_dir(&p, out, depth + 1);
        } else if t.is_file() && is_photo(&p) {
            out.push(p);
        }
    }
}

impl eframe::App for FotoApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frames += 1;
        self.auto_zoom(&ctx);
        self.poll(&ctx);
        self.screenshot(&ctx);
        let c = p();

        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        if !dropped.is_empty() && self.running.is_none() {
            self.add_paths(dropped);
        }

        egui::Panel::top("topbar")
            .exact_size(66.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(c.surface).inner_margin(egui::Margin::symmetric(22, 0)))
            .show(ui, |ui| self.top_bar(ui));
        // Responsive: the setup column takes ~30 % of the window, between 320 and 430 px.
        let win_w = ui.max_rect().width();
        let setup_w = (win_w * 0.3).clamp(320.0, 430.0);
        egui::Panel::right("setup")
            .exact_size(setup_w)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(c.bg).inner_margin(egui::Margin { left: 0, right: 20, top: 20, bottom: 20 }))
            .show(ui, |ui| self.setup_panel(ui, &ctx));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(c.bg).inner_margin(egui::Margin::same(20)))
            .show(ui, |ui| self.gallery(ui));

        self.settings_window(&ctx);
        self.update_ui(&ctx);
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) && self.draft.is_none() && self.running.is_none() {
            self.open_settings_at(Target::Global, Tab::Help);
        }
        if self.shot.is_some() {
            ctx.request_repaint();
        }
    }
}

/// The layout is designed for this size (in points at 100 %).
const DESIGN_W: f32 = 1180.0;
const DESIGN_H: f32 = 760.0;
/// Smallest scale – enough for a 1024 × 768 screen.
const MIN_ZOOM: f32 = 0.72;

impl FotoApp {
    /// Scale the whole interface down a little when the window is smaller than the design size,
    /// so everything still fits (e.g. on a 1024 × 768 screen). Computed from the window's real
    /// pixel size, so it doesn't feed back on itself.
    fn auto_zoom(&mut self, ctx: &egui::Context) {
        let native = ctx.native_pixels_per_point().unwrap_or(1.0);
        let ppp = ctx.pixels_per_point();
        let size = ctx.content_rect().size() * ppp / native; // points at zoom 1.0
        if size.x < 10.0 || size.y < 10.0 {
            return;
        }
        let want = (size.x / DESIGN_W).min(size.y / DESIGN_H).clamp(MIN_ZOOM, 1.0);
        let want = (want * 50.0).round() / 50.0; // steps of 2 % so it doesn't flicker
        if (ctx.zoom_factor() - want).abs() > 0.001 {
            ctx.set_zoom_factor(want);
        }
    }

    fn screenshot(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.shot else { return };
        if shot.done.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.frames >= 120 && !shot.requested {
            shot.requested = true;
            let path = shot.path.clone();
            let done = shot.done.clone();
            ctx.request_screenshot(move |img| {
                let [w, h] = img.size;
                let mut buf = image::RgbaImage::new(w as u32, h as u32);
                for (i, px) in img.pixels.iter().enumerate() {
                    buf.put_pixel((i % w) as u32, (i / w) as u32, image::Rgba(px.to_srgba_unmultiplied()));
                }
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = buf.save(&path) {
                    eprintln!("Screenshot could not be saved: {e}");
                }
                done.store(true, Ordering::Relaxed);
            });
        }
        if shot.requested && self.frames > 900 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn top_bar(&mut self, ui: &mut Ui) {
        let c = p();
        let full = ui.max_rect();
        ui.painter().line_segment([pos2(full.left() - 30.0, full.bottom()), pos2(full.right() + 30.0, full.bottom())], Stroke::new(1.0, c.border));
        let busy = self.running.is_some();
        let wide = full.width() > 1100.0;
        // Room left for the printer list after logo, title, label and the four icon buttons.
        let title_w = if wide { 300.0 } else { 170.0 };
        let combo_w = (full.width() - title_w - 60.0 - 4.0 * 48.0 - 40.0).clamp(150.0, 320.0);
        ui.horizontal_centered(|ui| {
            let (logo, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
            ui.painter().image(self.logo.id(), logo.expand(3.0), Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), Color32::WHITE);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.add_space(if wide { 12.0 } else { 20.0 });
                ui.label(RichText::new("Mr. PhotoPrint").font(bold(19.0)).color(c.text));
                if wide {
                    ui.add_space(-6.0);
                    ui.label(RichText::new(t("Viele Fotos – perfekt aufs Papier", "Many photos – perfectly on paper")).size(12.5).color(c.dim));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, 40.0, t("Hilfe (F1)", "Help (F1)"), |pt, r, col| icons::help(pt, r, col)).clicked() && !busy {
                    self.open_settings_at(Target::Global, Tab::Help);
                }
                // theme
                let dark = self.dark;
                if widgets::icon_button(ui, 40.0, if dark { t("Helles Design", "Light theme") } else { t("Dunkles Design", "Dark theme") }, |pt, r, col| {
                    if dark { icons::sun(pt, r, col) } else { icons::moon(pt, r, col) }
                })
                .clicked()
                {
                    self.dark = !self.dark;
                    theme::set_dark(ui.ctx(), self.dark);
                    self.save_settings();
                }
                if widgets::icon_button(ui, 40.0, t("Alle Einstellungen", "All settings"), |pt, r, col| icons::sliders(pt, r, col)).clicked() && !busy {
                    self.open_settings(Target::Global);
                }
                let props_tip = if self.job.printer == EXPORT_PRINTER {
                    t("Zielordner für die Bilddateien wählen", "Choose the folder for the picture files")
                } else {
                    t("Druckereigenschaften (Treiber)", "Printer properties (driver)")
                };
                if widgets::icon_button(ui, 40.0, props_tip, |pt, r, col| {
                    let bg = p().surface;
                    icons::gear(pt, r.center(), r.width() * 0.5, col, bg)
                })
                .clicked()
                    && !busy
                {
                    self.printer_properties();
                }
                ui.add_space(4.0);
                let mut chosen = None;
                ui.add_enabled_ui(!busy, |ui| {
                    ui.spacing_mut().interact_size.y = 38.0;
                    egui::ComboBox::from_id_salt("printer")
                        .width(combo_w)
                        .truncate()
                        .selected_text(RichText::new(printer::printer_label(&self.job.printer)).size(14.5))
                        .show_ui(ui, |ui| {
                            for pr in &self.printers.list {
                                let shown = printer::printer_label(pr);
                                let label = if Some(pr) == self.printers.default.as_ref() { format!("{shown}  ({})", t("Standard", "default")) } else { shown.to_string() };
                                if ui.selectable_label(*pr == self.job.printer, label).clicked() {
                                    chosen = Some(pr.clone());
                                }
                            }
                        });
                });
                if wide {
                    widgets::label(ui, t("Drucker", "Printer"));
                }
                if let Some(pr) = chosen {
                    if pr != self.job.printer {
                        self.job.printer = pr;
                        self.job.photo.paper = None;
                        self.job.photo.tray = None;
                        self.job.photo.media = None;
                        self.save_settings();
                    }
                }
            });
        });
    }

    fn printer_properties(&mut self) {
        if self.job.printer == EXPORT_PRINTER {
            if let Some(d) = rfd::FileDialog::new().set_title(t("Ordner für die Bilddateien wählen", "Choose a folder for the picture files")).pick_folder() {
                self.job.export_dir = d.to_string_lossy().into_owned();
                self.save_settings();
            }
        } else {
            let name = self.job.printer.clone();
            let base = self.printers.devmodes.get(&name).cloned();
            if let Some(dm) = printer::properties_dialog(&name, self.hwnd, base) {
                self.printers.devmodes.insert(name.clone(), dm);
                self.save_settings();
                self.printers.caps.remove(&name);
                self.printers.generation += 1;
            }
        }
    }

    // ---------------------------------------------------------------- gallery (middle)

    fn gallery(&mut self, ui: &mut Ui) {
        let c = p();
        let busy = self.running.is_some();
        // Toolbar: three sizes so nothing overlaps when the window is made narrow.
        let tw = ui.available_width();
        let (add_label, add_w, dir_label, dir_w, seg_w, show_count) = if tw >= 760.0 {
            (t("+   Fotos hinzufügen", "+   Add photos"), 196.0, t("Ordner hinzufügen", "Add folder"), 186.0, 170.0, true)
        } else if tw >= 560.0 {
            (t("+  Fotos", "+  Photos"), 120.0, t("Ordner", "Folder"), 100.0, 150.0, true)
        } else {
            (t("+  Fotos", "+  Photos"), 104.0, t("Ordner", "Folder"), 90.0, 128.0, false)
        };
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!busy, |ui| {
                if widgets::primary(ui, add_label, vec2(add_w, 42.0), !busy).on_hover_text(t("Fotos hinzufügen", "Add photos")).clicked() {
                    if let Some(files) = rfd::FileDialog::new().set_title(t("Fotos hinzufügen", "Add photos")).add_filter(t("Fotos", "Photos"), PHOTO_EXTENSIONS).pick_files() {
                        self.add_paths(files);
                    }
                }
                if widgets::secondary(ui, dir_label, vec2(dir_w, 42.0)).on_hover_text(t("Ordner mit Unterordnern hinzufügen", "Add a folder with its subfolders")).clicked() && !busy {
                    if let Some(dir) = rfd::FileDialog::new().set_title(t("Ordner hinzufügen", "Add folder")).pick_folder() {
                        self.add_paths(vec![dir]);
                    }
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(!busy, |ui| {
                    ui.menu_button(RichText::new(" ⋯ ").size(20.0), |ui| self.more_menu(ui));
                });
                ui.add_space(4.0);
                ui.allocate_ui(vec2(seg_w, 36.0), |ui| {
                    let mut g = self.grid;
                    if widgets::segmented(ui, &mut g, &[(true, t("Kacheln", "Tiles")), (false, t("Liste", "List"))], 36.0) {
                        self.grid = g;
                        self.save_settings();
                    }
                });
                if show_count {
                    ui.add_space(8.0);
                    let sel = self.items.iter().filter(|i| i.selected).count();
                    let txt = if self.items.is_empty() {
                        String::new()
                    } else if sel > 0 && tw >= 760.0 {
                        ts(format!("{} Fotos · {sel} ausgewählt", self.items.len()), format!("{} photos · {sel} selected", self.items.len()))
                    } else {
                        format!("{} {}", self.items.len(), t("Fotos", "photos"))
                    };
                    let room = (ui.available_width() - 6.0).max(0.0);
                    ui.add_sized(vec2(room.min(220.0), 36.0), egui::Label::new(RichText::new(txt).color(c.dim)).truncate());
                }
            });
        });
        if !self.message.is_empty() {
            ui.add_space(2.0);
            ui.label(RichText::new(&self.message).size(13.0).color(c.accent2));
        }
        ui.add_space(10.0);

        let has_sel = self.items.iter().any(|i| i.selected) && !busy;
        let mut area = ui.available_rect_before_wrap();
        if has_sel {
            area.max.y -= 64.0;
        }
        ui.painter().rect_filled(area, CornerRadius::same(16), c.surface);
        ui.painter().rect_stroke(area, CornerRadius::same(16), Stroke::new(1.0, c.border), StrokeKind::Inside);
        if self.items.is_empty() {
            self.drop_zone(ui, area);
        } else {
            let inner = area.shrink(14.0);
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                if self.grid {
                    self.grid_view(ui);
                } else {
                    self.list_view(ui);
                }
            });
        }
        if ui.ctx().input(|i| !i.raw.hovered_files.is_empty()) {
            ui.painter().rect_filled(area, CornerRadius::same(16), c.accent.gamma_multiply(0.18));
            ui.painter().rect_stroke(area, CornerRadius::same(16), Stroke::new(2.0, c.accent2), StrokeKind::Inside);
            ui.painter().text(area.center(), Align2::CENTER_CENTER, t("Loslassen zum Hinzufügen", "Drop to add"), bold(22.0), c.text);
        }
        ui.allocate_rect(area, Sense::hover());
        if has_sel {
            ui.add_space(10.0);
            self.selection_bar(ui);
        }
        if !busy && self.draft.is_none() {
            let (del, all, esc) = ui.ctx().input(|i| {
                (i.key_pressed(egui::Key::Delete), i.modifiers.command && i.key_pressed(egui::Key::A), i.key_pressed(egui::Key::Escape))
            });
            if del {
                self.items.retain(|i| !i.selected);
            }
            if all {
                self.items.iter_mut().for_each(|i| i.selected = true);
            }
            if esc {
                self.items.iter_mut().for_each(|i| i.selected = false);
            }
        }
    }

    fn drop_zone(&mut self, ui: &mut Ui, area: Rect) {
        let c = p();
        let zone = area.shrink(26.0);
        let pts = [zone.left_top(), zone.right_top(), zone.right_bottom(), zone.left_bottom(), zone.left_top()];
        ui.painter().extend(Shape::dashed_line(&pts, Stroke::new(1.5, c.border.lerp_to_gamma(c.dim, 0.4)), 8.0, 6.0));
        let center = zone.center() - vec2(0.0, 40.0);
        let badge = Rect::from_center_size(center, Vec2::splat(92.0));
        ui.painter().circle_filled(badge.center(), 52.0, c.accent.gamma_multiply(0.12));
        theme::gradient_rect(ui.painter(), badge.shrink(10.0), 20.0, c.accent, c.accent2);
        icons::add_photos(ui.painter(), badge.shrink(24.0), Color32::WHITE, c.accent);
        ui.painter().text(center + vec2(0.0, 80.0), Align2::CENTER_CENTER, t("Fotos hierher ziehen", "Drag photos here"), bold(24.0), c.text);
        ui.painter().text(
            center + vec2(0.0, 112.0),
            Align2::CENTER_CENTER,
            t("oder oben über „Fotos hinzufügen“ bzw. „Ordner hinzufügen“ auswählen", "or choose them with \"Add photos\" or \"Add folder\" above"),
            FontId::proportional(14.5),
            c.dim,
        );
        ui.painter().text(
            center + vec2(0.0, 138.0),
            Align2::CENTER_CENTER,
            t("JPG · PNG · TIFF · WebP · BMP · GIF · HEIC  —  Dateien werden geprüft, bevor sie geöffnet werden", "JPG · PNG · TIFF · WebP · BMP · GIF · HEIC  —  files are checked before they are opened"),
            FontId::proportional(12.5),
            c.dim.gamma_multiply(0.85),
        );
    }

    fn handle_click(&mut self, i: usize, resp: &egui::Response, check_rect: Rect, ui: &Ui) {
        let busy = self.running.is_some();
        if resp.clicked() {
            let pos = resp.interact_pointer_pos().unwrap_or_default();
            if check_rect.contains(pos) {
                self.items[i].checked = !self.items[i].checked;
            } else {
                let m = ui.input(|inp| inp.modifiers);
                if m.shift {
                    let a = self.anchor.unwrap_or(i).min(self.items.len() - 1);
                    let (lo, hi) = (a.min(i), a.max(i));
                    for (k, it) in self.items.iter_mut().enumerate() {
                        it.selected = k >= lo && k <= hi;
                    }
                } else if m.command {
                    self.items[i].selected = !self.items[i].selected;
                    self.anchor = Some(i);
                } else {
                    let was_only = self.items[i].selected && self.items.iter().filter(|x| x.selected).count() == 1;
                    self.items.iter_mut().for_each(|x| x.selected = false);
                    self.items[i].selected = !was_only;
                    self.anchor = Some(i);
                }
                self.focus_item = Some(i);
            }
        }
        if resp.double_clicked() && !busy {
            self.items.iter_mut().for_each(|x| x.selected = false);
            self.items[i].selected = true;
            self.open_settings(Target::Items(vec![i]));
        }
        if resp.secondary_clicked() && !self.items[i].selected {
            self.items.iter_mut().for_each(|x| x.selected = false);
            self.items[i].selected = true;
            self.focus_item = Some(i);
        }
    }

    fn context_menu(&mut self, resp: &egui::Response) {
        if self.running.is_some() {
            return;
        }
        let mut what: Option<&str> = None;
        resp.context_menu(|ui| {
            let mut pick = |ui: &mut Ui, label: &str, w: &'static str| {
                if ui.button(label).clicked() {
                    what = Some(w);
                    ui.close();
                }
            };
            pick(ui, t("Eigene Einstellungen…", "Own settings…"), "settings");
            pick(ui, t("Um 90° drehen", "Turn by 90°"), "rotate");
            pick(ui, t("Eigene Einstellungen entfernen", "Remove own settings"), "reset");
            ui.separator();
            pick(ui, t("Nach oben", "Move up"), "up");
            pick(ui, t("Nach unten", "Move down"), "down");
            pick(ui, t("Enthaltenden Ordner öffnen", "Open containing folder"), "folder");
            ui.separator();
            pick(ui, t("Entfernen", "Remove"), "remove");
        });
        if let Some(w) = what {
            self.selection_action(w);
        }
    }

    fn selection_action(&mut self, what: &str) {
        let sel = self.selected();
        match what {
            "settings" if !sel.is_empty() => self.open_settings(Target::Items(sel)),
            "rotate" => self.set_own(|s| {
                s.auto_rotate = AutoRotate::None;
                s.rotation = (s.rotation + 90) % 360;
            }),
            "reset" => {
                for it in self.items.iter_mut().filter(|i| i.selected) {
                    it.own = None;
                    it.fine = None;
                }
            }
            "up" => {
                for i in sel {
                    if i > 0 && !self.items[i - 1].selected {
                        self.items.swap(i, i - 1);
                    }
                }
            }
            "down" => {
                for &i in sel.iter().rev() {
                    if i + 1 < self.items.len() && !self.items[i + 1].selected {
                        self.items.swap(i, i + 1);
                    }
                }
            }
            "folder" => {
                if let Some(dir) = sel.first().and_then(|&i| self.items[i].path.parent().map(|d| d.to_path_buf())) {
                    open_path(&dir);
                }
            }
            "remove" => self.items.retain(|i| !i.selected),
            "none" => self.items.iter_mut().for_each(|i| i.selected = false),
            _ => {}
        }
    }

    fn grid_view(&mut self, ui: &mut Ui) {
        let c = p();
        let gap = 14.0;
        let w = ui.available_width();
        let cols = ((w + gap) / (190.0 + gap)).floor().max(1.0) as usize;
        let card_w = (w - gap * (cols as f32 - 1.0)) / cols as f32 - 0.5;
        let thumb_h = card_w * 0.72;
        let card_h = thumb_h + 58.0;
        let n = self.items.len();
        let rows = n.div_ceil(cols);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, card_h + gap, rows, |ui, range| {
            for row in range {
                let (row_rect, _) = ui.allocate_exact_size(vec2(w, card_h + gap), Sense::hover());
                for col in 0..cols {
                    let i = row * cols + col;
                    if i >= n {
                        break;
                    }
                    let rect = Rect::from_min_size(row_rect.min + vec2(col as f32 * (card_w + gap), 0.0), vec2(card_w, card_h));
                    let resp = ui.interact(rect, ui.scope_id().with(("card", i)), Sense::click());
                    let it = &self.items[i];
                    let pt = ui.painter();
                    let hovered = resp.hovered();
                    let fill = if it.selected { c.surface2.lerp_to_gamma(c.accent, 0.16) } else if hovered { c.surface2 } else { c.surface2.lerp_to_gamma(c.surface, 0.5) };
                    pt.rect_filled(rect, CornerRadius::same(14), fill);
                    let stroke = if it.selected { Stroke::new(2.0, c.accent2) } else if hovered { Stroke::new(1.0, c.accent) } else { Stroke::new(1.0, c.border) };
                    pt.rect_stroke(rect, CornerRadius::same(14), stroke, StrokeKind::Inside);
                    // thumbnail
                    let tr = Rect::from_min_size(rect.min + vec2(8.0, 8.0), vec2(card_w - 16.0, thumb_h - 8.0));
                    pt.rect_filled(tr, CornerRadius::same(10), c.stage);
                    if let Some(tex) = self.thumbs.get(&it.path) {
                        let ts = tex.size_vec2();
                        let k = (tr.width() / ts.x).min(tr.height() / ts.y);
                        let ir = Rect::from_center_size(tr.center(), ts * k);
                        pt.image(tex.id(), ir, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), if it.checked { Color32::WHITE } else { Color32::from_gray(110) });
                    } else if it.status != ItemStatus::Failed {
                        pt.text(tr.center(), Align2::CENTER_CENTER, t("Lädt …", "Loading …"), FontId::proportional(13.0), c.dim);
                    } else {
                        pt.text(tr.center(), Align2::CENTER_CENTER, "⚠", FontId::proportional(30.0), c.bad);
                    }
                    // check circle
                    let cc = tr.left_top() + vec2(16.0, 16.0);
                    let check_rect = Rect::from_center_size(cc, Vec2::splat(30.0));
                    if it.checked {
                        pt.circle_filled(cc, 11.0, c.accent);
                        pt.add(Shape::line(vec![cc + vec2(-5.0, 0.0), cc + vec2(-1.5, 4.0), cc + vec2(5.0, -4.0)], Stroke::new(2.2, Color32::WHITE)));
                    } else {
                        pt.circle_filled(cc, 11.0, Color32::from_black_alpha(90));
                        pt.circle_stroke(cc, 10.0, Stroke::new(1.6, Color32::WHITE));
                    }
                    // status chip
                    let (st, fg) = match it.status {
                        ItemStatus::Waiting => ("", c.dim),
                        ItemStatus::Printing => (t("Druckt …", "Printing …"), c.accent2),
                        ItemStatus::Printed => (t("Erledigt", "Done"), c.ok),
                        ItemStatus::Failed => (t("Fehler", "Error"), c.bad),
                    };
                    if !st.is_empty() {
                        widgets::chip(pt, tr.right_top() + vec2(-8.0, 8.0), Align2::RIGHT_TOP, st, Color32::WHITE, fg.gamma_multiply(0.9));
                    }
                    if it.own.is_some() || it.fine.is_some() {
                        widgets::chip(pt, tr.left_bottom() + vec2(8.0, -8.0), Align2::LEFT_BOTTOM, t("eigene Einstellung", "own settings"), Color32::WHITE, c.accent.gamma_multiply(0.92));
                    }
                    // text
                    let name_pos = pos2(rect.left() + 12.0, tr.bottom() + 10.0);
                    let g = pt.layout_no_wrap(it.name.clone(), bold(13.5), c.text);
                    pt.with_clip_rect(Rect::from_min_size(name_pos, vec2(card_w - 24.0, 22.0))).galley(name_pos, g, c.text);
                    let s = it.settings(&self.job);
                    if let Some(d) = self.sharpness.get(&i).filter(|d| **d < layout::LOW_DPI) {
                        widgets::chip(pt, tr.right_bottom() + vec2(-8.0, -8.0), Align2::RIGHT_BOTTOM, &format!("{} {d:.0} dpi", t("unscharf?", "blurry?")), Color32::BLACK, c.warn);
                    }
                    let meta = if it.status == ItemStatus::Failed {
                        it.message.clone()
                    } else if it.px.0 > 0 {
                        format!("{} × {} · {}", it.px.0, it.px.1, s.scale.short())
                    } else {
                        s.scale.short().to_string()
                    };
                    let mcol = if it.status == ItemStatus::Failed { c.bad } else { c.dim };
                    let mg = pt.layout_no_wrap(meta, FontId::proportional(12.5), mcol);
                    pt.with_clip_rect(Rect::from_min_size(name_pos + vec2(0.0, 21.0), vec2(card_w - 24.0, 20.0))).galley(name_pos + vec2(0.0, 21.0), mg, mcol);
                    let tip = if it.message.is_empty() { it.path.display().to_string() } else { format!("{}\n{}", it.path.display(), it.message) };
                    let resp = resp.on_hover_text(tip);
                    self.handle_click(i, &resp, check_rect, ui);
                    self.context_menu(&resp);
                }
            }
        });
    }

    fn list_view(&mut self, ui: &mut Ui) {
        let c = p();
        let row_h = 48.0;
        let n = self.items.len();
        let w = ui.available_width();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, row_h, n, |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for i in range {
                let (rr, resp) = ui.allocate_exact_size(vec2(w, row_h), Sense::click());
                let it = &self.items[i];
                let pt = ui.painter();
                if it.selected {
                    pt.rect_filled(rr.shrink2(vec2(0.0, 2.0)), CornerRadius::same(10), c.surface2.lerp_to_gamma(c.accent, 0.18));
                } else if resp.hovered() {
                    pt.rect_filled(rr.shrink2(vec2(0.0, 2.0)), CornerRadius::same(10), c.surface2);
                }
                let cc = pos2(rr.left() + 20.0, rr.center().y);
                let check_rect = Rect::from_center_size(cc, Vec2::splat(30.0));
                if it.checked {
                    pt.circle_filled(cc, 10.0, c.accent);
                    pt.add(Shape::line(vec![cc + vec2(-4.5, 0.0), cc + vec2(-1.5, 3.5), cc + vec2(4.5, -3.5)], Stroke::new(2.0, Color32::WHITE)));
                } else {
                    pt.circle_stroke(cc, 9.0, Stroke::new(1.5, c.dim));
                }
                pt.text(pos2(rr.left() + 40.0, rr.center().y), Align2::LEFT_CENTER, format!("{}", i + 1), FontId::proportional(12.5), c.dim);
                let tr = Rect::from_min_size(pos2(rr.left() + 70.0, rr.top() + 5.0), vec2(52.0, row_h - 10.0));
                pt.rect_filled(tr, CornerRadius::same(6), c.stage);
                if let Some(tex) = self.thumbs.get(&it.path) {
                    let ts = tex.size_vec2();
                    let k = (tr.width() / ts.x).min(tr.height() / ts.y);
                    pt.image(tex.id(), Rect::from_center_size(tr.center(), ts * k), Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), Color32::WHITE);
                }
                let x = tr.right() + 14.0;
                let right_w = 330.0_f32.min(w * 0.45);
                let name_clip = Rect::from_min_max(pos2(x, rr.top()), pos2(rr.right() - right_w, rr.bottom()));
                let mut name = it.name.clone();
                if it.own.is_some() || it.fine.is_some() {
                    name.push_str(t("  (eigene Einstellung)", "  (own settings)"));
                }
                let g = pt.layout_no_wrap(name, bold(14.0), c.text);
                pt.with_clip_rect(name_clip).galley(pos2(x, rr.center().y - g.size().y / 2.0), g, c.text);
                let s = it.settings(&self.job);
                let info = format!(
                    "{}   {}   {}",
                    if it.px.0 > 0 { format!("{} × {}", it.px.0, it.px.1) } else { "…".into() },
                    s.scale.short(),
                    it.modified.map(format_time).unwrap_or_default()
                );
                pt.text(pos2(rr.right() - right_w, rr.center().y), Align2::LEFT_CENTER, info, FontId::proportional(13.0), c.dim);
                let (st, fg) = match it.status {
                    ItemStatus::Waiting => (t("Bereit", "Ready"), c.dim),
                    ItemStatus::Printing => (t("Druckt …", "Printing …"), c.accent2),
                    ItemStatus::Printed => (t("Erledigt", "Done"), c.ok),
                    ItemStatus::Failed => (t("Fehler", "Error"), c.bad),
                };
                pt.text(pos2(rr.right() - 12.0, rr.center().y), Align2::RIGHT_CENTER, st, bold(12.5), fg);
                let tip = if it.message.is_empty() { it.path.display().to_string() } else { format!("{}\n{}", it.path.display(), it.message) };
                let resp = resp.on_hover_text(tip);
                self.handle_click(i, &resp, check_rect, ui);
                self.context_menu(&resp);
            }
        });
    }

    fn selection_bar(&mut self, ui: &mut Ui) {
        let c = p();
        let n = self.items.iter().filter(|i| i.selected).count();
        let rect = ui.available_rect_before_wrap();
        let rect = Rect::from_min_size(rect.min, vec2(rect.width(), 54.0));
        ui.painter().rect_filled(rect, CornerRadius::same(14), c.surface);
        ui.painter().rect_stroke(rect, CornerRadius::same(14), Stroke::new(1.0, c.accent.gamma_multiply(0.7)), StrokeKind::Inside);
        let mut what = None;
        let wide = rect.width() >= 720.0;
        let buttons: Vec<(&str, &str, &str)> = if wide {
            vec![
                (t("Eigene Einstellungen", "Own settings"), "settings", ""),
                (t("Um 90° drehen", "Turn by 90°"), "rotate", ""),
                (t("Zurücksetzen", "Reset"), "reset", ""),
                (t("Entfernen", "Remove"), "remove", ""),
            ]
        } else {
            vec![
                (t("Einstellen", "Settings"), "settings", t("Eigene Einstellungen", "Own settings")),
                (t("Drehen", "Turn"), "rotate", t("Um 90° drehen", "Turn by 90°")),
                (t("Zurück", "Reset"), "reset", t("Eigene Einstellungen entfernen", "Remove own settings")),
                (t("Entfernen", "Remove"), "remove", ""),
            ]
        };
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(16.0, 8.0))), |ui| {
            ui.set_clip_rect(rect);
            ui.horizontal_centered(|ui| {
                ui.label(RichText::new(if wide { format!("{n} {}", t("ausgewählt", "selected")) } else { n.to_string() }).font(bold(15.0)).color(c.text));
                ui.add_space(if wide { 10.0 } else { 4.0 });
                for (label, w, tip) in &buttons {
                    let r = ui.button(*label);
                    let r = if tip.is_empty() { r } else { r.on_hover_text(*tip) };
                    if r.clicked() {
                        what = Some(*w);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(if wide { t("Auswahl aufheben", "Clear selection") } else { "×" }).on_hover_text(t("Auswahl aufheben", "Clear selection")).clicked() {
                        what = Some("none");
                    }
                });
            });
        });
        ui.allocate_rect(rect, Sense::hover());
        if let Some(w) = what {
            self.selection_action(w);
        }
    }

    fn more_menu(&mut self, ui: &mut Ui) {
        ui.set_min_width(250.0);
        if ui.button(t("Liste speichern …", "Save list …")).clicked() {
            ui.close();
            if let Some(p) = rfd::FileDialog::new().add_filter(t("Fotoliste", "Photo list"), &["txt"]).set_file_name(t("Fotoliste.txt", "Photo list.txt")).save_file() {
                let text: String = self.items.iter().map(|i| format!("{}\n", i.path.display())).collect();
                self.message = match std::fs::write(&p, text) {
                    Ok(_) => t("Liste gespeichert.", "List saved.").into(),
                    Err(e) => format!("{} {e}", t("Speichern fehlgeschlagen:", "Saving failed:")),
                };
            }
        }
        if ui.button(t("Liste laden …", "Load list …")).clicked() {
            ui.close();
            if let Some(p) = rfd::FileDialog::new().add_filter(t("Fotoliste", "Photo list"), &["txt", "flist"]).pick_file() {
                self.load_list(&p);
            }
        }
        ui.separator();
        if ui.button(t("Nach Name sortieren", "Sort by name")).clicked() {
            ui.close();
            self.items.sort_by_key(|i| i.name.to_lowercase());
        }
        if ui.button(t("Nach Aufnahmedatum (Datei) sortieren", "Sort by date (file)")).clicked() {
            ui.close();
            self.items.sort_by_key(|i| i.modified);
        }
        ui.separator();
        if ui.button(t("Alle auswählen", "Select all")).clicked() {
            ui.close();
            self.items.iter_mut().for_each(|i| i.selected = true);
        }
        if ui.button(t("Alle / keine zum Drucken markieren", "Mark all / none for printing")).clicked() {
            ui.close();
            let all = self.items.iter().all(|i| i.checked);
            self.items.iter_mut().for_each(|i| i.checked = !all);
        }
        if ui.button(t("Status zurücksetzen", "Reset status")).clicked() {
            ui.close();
            for it in &mut self.items {
                if it.status != ItemStatus::Failed {
                    it.status = ItemStatus::Waiting;
                    it.message.clear();
                }
            }
        }
        if ui.button(t("Erledigte entfernen", "Remove finished")).clicked() {
            ui.close();
            self.items.retain(|i| i.status != ItemStatus::Printed);
        }
        if ui.button(t("Liste leeren", "Clear list")).clicked() {
            ui.close();
            self.items.clear();
            self.message.clear();
        }
    }

    /// A list file is untrusted text: only local, existing photo files are taken from it.
    fn load_list(&mut self, path: &Path) {
        let Ok(meta) = std::fs::metadata(path) else { return };
        if meta.len() > 20 * 1024 * 1024 {
            self.message = t("Die Liste ist zu groß.", "The list is too large.").into();
            return;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            self.message = t("Die Liste konnte nicht gelesen werden.", "The list could not be read.").into();
            return;
        };
        let (paths, blocked) = list_entries(&text);
        self.add_paths(paths);
        if blocked > 0 {
            self.message.push_str(&ts(
                format!(" {blocked} Einträge ignoriert (Netzwerkpfade, fehlende oder keine Fotodateien)."),
                format!(" {blocked} entries ignored (network paths, missing files or not photos)."),
            ));
        }
    }

    // ---------------------------------------------------------------- setup panel (right)

    fn setup_panel(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let c = p();
        let busy = self.running.is_some();
        widgets::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(ui.available_height());
            // The summary and the print button always stay visible at the bottom; on short windows
            // the controls scroll and the sheet preview steps aside.
            let total_h = ui.available_height();
            let bottom_h = 150.0;
            let controls_max = if total_h < 760.0 { (total_h - 124.0).max(120.0) } else { total_h };
            egui::ScrollArea::vertical().id_salt("setup_controls").max_height(controls_max).auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(t("Druck-Setup", "Print setup")).font(bold(18.0)).color(c.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(RichText::new(t("Alle Einstellungen ›", "All settings ›")).size(13.0).color(c.accent2)).frame(false)).clicked() && !busy {
                        self.open_settings(Target::Global);
                    }
                });
            });
            ui.add_space(4.0);
            ui.add_enabled_ui(!busy, |ui| {
                let printer = self.job.printer.clone();
                let caps = self.printers.caps(&printer).clone();
                let paper_name = self.printers.paper_name(&printer, self.job.photo.paper);
                let mut changed = false;
                // Presets for the usual photo jobs. The paper is looked up by its size in the
                // printer's own list, because every driver numbers photo papers differently.
                let find = |size: (f32, f32)| caps.papers.iter().find(|pp| paper_matches(pp.w_mm, pp.h_mm, size.0, size.1)).cloned();
                let active = PRESETS.iter().position(|(_, size, scale, grid, borderless)| {
                    find(*size).map(|pp| Some(pp.id) == self.job.photo.paper).unwrap_or(false)
                        && self.job.photo.scale == *scale
                        && (self.job.cols, self.job.rows) == *grid
                        && self.job.ignore_hard_margins == *borderless
                });
                let label_w = 64.0;
                let mut pick = None;
                ui.horizontal(|ui| {
                    ui.add_sized(vec2(label_w, 30.0), egui::Label::new(RichText::new(t("Vorlage", "Preset")).color(c.dim)));
                    let current = active.map(|k| preset_name(k)).unwrap_or(t("Eigene Einstellung", "Custom"));
                    egui::ComboBox::from_id_salt("preset").width(ui.available_width() - 8.0).selected_text(current).show_ui(ui, |ui| {
                        for (k, (_, size, scale, grid, _)) in PRESETS.iter().enumerate() {
                            let name = preset_name(k);
                            let tip = match find(*size) {
                                Some(pp) => format!("{} · {} · {} {}", pp.name, scale.label(), grid.0 * grid.1, t("Foto(s) pro Blatt", "photo(s) per sheet")),
                                None => ts(
                                    format!("Der Drucker meldet kein Papier mit {:.0} × {:.0} mm", size.0, size.1),
                                    format!("The printer reports no paper of {:.0} × {:.0} mm", size.0, size.1),
                                ),
                            };
                            if ui.selectable_label(active == Some(k), name).on_hover_text(tip).clicked() {
                                pick = Some(k);
                            }
                        }
                    });
                });
                if let Some(k) = pick {
                    let (_, size, scale, grid, borderless) = PRESETS[k];
                    let name = preset_name(k);
                    match find(size) {
                        Some(pp) => {
                            self.job.photo.paper = Some(pp.id);
                            self.message = ts(format!("Vorlage „{name}“: {}", pp.name), format!("Preset \"{name}\": {}", pp.name));
                        }
                        None => {
                            self.message = ts(
                                format!("„{name}“: Der Drucker kennt dieses Papierformat nicht – Papier bleibt unverändert."),
                                format!("\"{name}\": the printer does not know this paper size – the paper stays as it is."),
                            )
                        }
                    }
                    self.job.photo.scale = scale;
                    self.job.photo.orientation = Orientation::ByPhoto;
                    self.job.cols = grid.0;
                    self.job.rows = grid.1;
                    self.job.next_photo_new_sheet = false;
                    self.job.ignore_hard_margins = borderless;
                    changed = true;
                }
                let ph = &mut self.job.photo;
                ui.horizontal(|ui| {
                    ui.add_sized(vec2(label_w, 30.0), egui::Label::new(RichText::new(t("Papier", "Paper")).color(c.dim)));
                    egui::ComboBox::from_id_salt("quick_paper").width(ui.available_width() - 8.0).selected_text(paper_name).show_ui(ui, |ui| {
                        changed |= ui.selectable_value(&mut ph.paper, None, t("Wie beim Drucker", "As set in the printer")).changed();
                        for pp in &caps.papers {
                            changed |= ui.selectable_value(&mut ph.paper, Some(pp.id), format!("{}  ·  {:.0} × {:.0} mm", pp.name, pp.w_mm, pp.h_mm)).changed();
                        }
                    });
                });
                if !caps.media.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_sized(vec2(label_w, 30.0), egui::Label::new(RichText::new(t("Papierart", "Paper type")).color(c.dim)))
                            .on_hover_text(media_tip());
                        let current = media_name(&caps, ph.media);
                        egui::ComboBox::from_id_salt("quick_media").width(ui.available_width() - 8.0).selected_text(current).show_ui(ui, |ui| {
                            changed |= ui.selectable_value(&mut ph.media, None, t("Wie beim Drucker", "As set in the printer")).changed();
                            for m in &caps.media {
                                changed |= ui.selectable_value(&mut ph.media, Some(m.id), &m.name).changed();
                            }
                        });
                    });
                }
                ui.add_space(2.0);
                widgets::label(ui, t("Foto einpassen", "Fit the photo"));
                changed |= widgets::segmented(
                    ui,
                    &mut ph.scale,
                    &[(ScaleMode::Fill, ScaleMode::Fill.short()), (ScaleMode::Fit, ScaleMode::Fit.short()), (ScaleMode::Shrink, ScaleMode::Shrink.short()), (ScaleMode::AsIs, ScaleMode::AsIs.short())],
                    32.0,
                );
                ui.add_space(2.0);
                widgets::label(ui, t("Fotos pro Blatt", "Photos per sheet"));
                let mut layout = (self.job.cols, self.job.rows);
                if widgets::segmented(ui, &mut layout, &[((1, 1), "1"), ((1, 2), "2"), ((2, 2), "4"), ((2, 3), "6"), ((3, 3), "9"), ((3, 4), "12")], 32.0) {
                    self.job.cols = layout.0;
                    self.job.rows = layout.1;
                    self.job.next_photo_new_sheet = false;
                    changed = true;
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("Kopien", "Copies")).color(c.text));
                    let ph = &mut self.job.photo;
                    if ui.add_enabled(ph.copies > 1, egui::Button::new("–").min_size(vec2(30.0, 30.0))).clicked() {
                        ph.copies -= 1;
                        changed = true;
                    }
                    ui.label(RichText::new(ph.copies.to_string()).font(bold(16.0)));
                    if ui.add(egui::Button::new("+").min_size(vec2(30.0, 30.0))).clicked() {
                        ph.copies = (ph.copies + 1).min(999);
                        changed = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::switch(ui, &mut self.job.ignore_hard_margins, t("Randlos", "Borderless"))
                            .on_hover_text(t("Druckerränder ignorieren – für randlosen Fotodruck", "Ignore the printer margins – for borderless photo prints")).clicked() {
                            changed = true;
                        }
                    });
                });
                // Less frequent options fold away so the sheet preview keeps its room.
                egui::CollapsingHeader::new(RichText::new(t("Weitere Optionen: Ausrichtung, Druckqualität", "More options: orientation, print quality")).size(13.0).color(c.dim))
                    .id_salt("more_setup")
                    .default_open(false)
                    .show(ui, |ui| {
                        let ph = &mut self.job.photo;
                        widgets::label(ui, t("Ausrichtung des Papiers", "Paper orientation"));
                        changed |= widgets::segmented(
                            ui,
                            &mut ph.orientation,
                            &[
                                (Orientation::ByPhoto, "Auto"),
                                (Orientation::Portrait, t("Hoch", "Portrait")),
                                (Orientation::Landscape, t("Quer", "Landscape")),
                                (Orientation::AsPrinter, t("Drucker", "Printer")),
                            ],
                            32.0,
                        );
                        widgets::label(ui, if self.job.printer == EXPORT_PRINTER { t("Qualität der Bilddateien", "Quality of the picture files") } else { t("Druckqualität", "Print quality") });
                        let mut q = if [QUALITY_AS_PRINTER, QUALITY_DRAFT, QUALITY_LOW, QUALITY_MEDIUM, QUALITY_HIGH].contains(&self.job.quality) { self.job.quality } else { QUALITY_AS_PRINTER };
                        let first = if self.job.printer == EXPORT_PRINTER { t("Standard", "Default") } else { t("Drucker", "Printer") };
                        if widgets::segmented(ui, &mut q, &[
                                (QUALITY_AS_PRINTER, first),
                                (QUALITY_DRAFT, t("Entwurf", "Draft")),
                                (QUALITY_LOW, t("Niedrig", "Low")),
                                (QUALITY_MEDIUM, t("Mittel", "Medium")),
                                (QUALITY_HIGH, t("Hoch", "High")),
                            ], 32.0) {
                            self.job.quality = q;
                            changed = true;
                        }
                        if self.job.quality > 0 {
                            ui.label(RichText::new(ts(
                                format!("Eingestellt: {} dpi (unter „Alle Einstellungen“)", self.job.quality),
                                format!("Set to {} dpi (under \"All settings\")", self.job.quality),
                            )).size(12.0).color(c.dim));
                        }
                    });
                if changed {
                    self.save_settings();
                }
            });
            });
            ui.add_space(8.0);

            // preview (only when there is room for it)
            let avail = ui.available_rect_before_wrap();
            let show_preview = avail.height() - bottom_h >= 130.0;
            let prev = Rect::from_min_size(avail.min, vec2(avail.width(), if show_preview { avail.height() - bottom_h } else { 0.0 }));
            if show_preview {
                ui.painter().rect_filled(prev, CornerRadius::same(12), c.stage);
            }
            let order = self.order();
            let job = self.job.clone();
            let items = std::mem::take(&mut self.items);
            let sheets = if order.is_empty() { Vec::new() } else { self.plan_with(&job, &items, &order) };
            self.items = items;
            self.sharpness.clear();
            for sh in &sheets {
                for cl in &sh.cells {
                    if let Some(d) = layout::effective_dpi(&self.items[cl.item], &cl.placement) {
                        let e = self.sharpness.entry(cl.item).or_insert(d);
                        *e = e.min(d);
                    }
                }
            }
            let soft = self.sharpness.values().filter(|d| **d < layout::LOW_DPI).count();
            if let Some(f) = self.focus_item.take() {
                if let Some(k) = sheets.iter().position(|s| s.cells.iter().any(|cl| cl.item == f)) {
                    self.sheet_idx = k;
                }
            }
            if !show_preview {
                if !sheets.is_empty() {
                    self.sheet_idx = self.sheet_idx.min(sheets.len() - 1);
                }
            } else if sheets.is_empty() {
                let land = layout::sheet_landscape(job.photo.orientation, (100.0, 150.0));
                let page = self.printers.page(&job.printer, &job.photo, land, job.export_dpi);
                let empty = Sheet { paper: job.photo.paper, tray: None, media: None, landscape: land, settings_of: 0, page, cells: vec![] };
                self.sheet_view(ui, prev.shrink2(vec2(10.0, 22.0)), &empty, "quick_preview");
                ui.painter().text(prev.center_bottom() - vec2(0.0, 14.0), Align2::CENTER_CENTER, t("Vorschau erscheint, sobald Fotos da sind", "The preview appears as soon as there are photos"), FontId::proportional(12.5), c.dim);
            } else {
                self.sheet_idx = self.sheet_idx.min(sheets.len() - 1);
                let sh = sheets[self.sheet_idx].clone();
                self.sheet_view(ui, Rect::from_min_max(prev.min + vec2(10.0, 10.0), prev.max - vec2(10.0, 36.0)), &sh, "quick_preview");
                // navigation
                let nav = Rect::from_center_size(prev.center_bottom() - vec2(0.0, 18.0), vec2(200.0, 28.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(nav), |ui| {
                    ui.horizontal_centered(|ui| {
                        if ui.add_enabled(self.sheet_idx > 0, egui::Button::new("‹").min_size(vec2(28.0, 26.0))).clicked() {
                            self.sheet_idx -= 1;
                        }
                        ui.label(RichText::new(format!("{} {} / {}", t("Blatt", "Sheet"), self.sheet_idx + 1, sheets.len())).size(13.0).color(c.dim));
                        if ui.add_enabled(self.sheet_idx + 1 < sheets.len(), egui::Button::new("›").min_size(vec2(28.0, 26.0))).clicked() {
                            self.sheet_idx += 1;
                        }
                    });
                });
            }
            if show_preview {
                ui.allocate_rect(prev, Sense::hover());
                ui.add_space(10.0);
            }

            // summary + print button
            let page = sheets.get(self.sheet_idx).map(|s| s.page);
            let summary = match (order.len(), page) {
                (0, _) => t("Noch keine Fotos ausgewählt", "No photos chosen yet").to_string(),
                (n, Some(pg)) => ts(
                    format!("{n} Fotos  →  {} Blätter  ·  {:.0} × {:.0} mm", sheets.len(), pg.width_mm, pg.height_mm),
                    format!("{n} photos  →  {} sheets  ·  {:.0} × {:.0} mm", sheets.len(), pg.width_mm, pg.height_mm),
                ),
                (n, None) => format!("{n} {}", t("Fotos", "photos")),
            };
            ui.label(RichText::new(summary).size(13.0).color(c.dim));
            if let Some(edge) = page.filter(|_| self.job.printer != EXPORT_PRINTER).and_then(|pg| layout::borderless_refused(&pg, &self.job)) {
                ui.label(
                    RichText::new(ts(
                        format!("Nicht randlos: Der Drucker lässt hier {edge:.1} mm Rand."),
                        format!("Not borderless: the printer keeps a {edge:.1} mm edge here."),
                    ))
                    .size(12.5)
                    .color(c.warn),
                )
                .on_hover_text(t(
                    "Viele Drucker drucken nur auf Fotopapier randlos: Wählen Sie bei „Papierart“ Ihr Fotopapier. Hilft das nicht, kann der Drucker dieses Papierformat nicht randlos drucken – oder es muss in den Druckereigenschaften (Zahnrad) eingeschaltet werden.",
                    "Many printers only print borderless on photo paper: choose your photo paper under \"Paper type\". If that doesn't help, the printer can't print this paper size borderless – or it has to be switched on in the printer properties (gear icon).",
                ));
            }
            if soft > 0 {
                ui.label(
                    RichText::new(ts(
                        format!("{soft} Foto(s) könnten unscharf werden (unter {:.0} dpi)", layout::LOW_DPI),
                        format!("{soft} photo(s) may look blurry (below {:.0} dpi)", layout::LOW_DPI),
                    ))
                    .size(12.5)
                    .color(c.warn),
                )
                .on_hover_text(t(
                    "Tipp: kleineres Papier, „Anpassen“ statt „Füllen“ oder mehrere Fotos pro Blatt wählen.",
                    "Tip: choose smaller paper, \"Fit\" instead of \"Fill\", or several photos per sheet.",
                ));
            }
            ui.add_space(6.0);
            let w = ui.available_width();
            if let Some(run) = &self.running {
                let frac = if run.total > 0 { run.done as f32 / run.total as f32 } else { 0.0 };
                let (bar, _) = ui.allocate_exact_size(vec2(w, 12.0), Sense::hover());
                ui.painter().rect_filled(bar, CornerRadius::same(6), c.surface2);
                if frac > 0.0 {
                    theme::gradient_rect(ui.painter(), Rect::from_min_size(bar.min, vec2(bar.width() * frac, bar.height())), 6.0, c.accent, c.accent2);
                }
                let ctl = run.ctl.clone();
                let now = (run.done + 1).min(run.total);
                let label = match (self.job.printer == EXPORT_PRINTER, crate::i18n::get()) {
                    (true, crate::i18n::Lang::De) => format!("Speichere Blatt {now} von {}", run.total),
                    (false, crate::i18n::Lang::De) => format!("Drucke Blatt {now} von {}", run.total),
                    (true, crate::i18n::Lang::En) => format!("Saving sheet {now} of {}", run.total),
                    (false, crate::i18n::Lang::En) => format!("Printing sheet {now} of {}", run.total),
                };
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).font(bold(14.0)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(t("Stopp", "Stop")).clicked() {
                            ctl.stop.store(true, Ordering::Relaxed);
                        }
                        let paused = ctl.pause.load(Ordering::Relaxed);
                        if ui.button(if paused { t("Weiter", "Resume") } else { "Pause" }).clicked() {
                            ctl.pause.store(!paused, Ordering::Relaxed);
                        }
                    });
                });
            } else {
                let text = if self.job.printer == EXPORT_PRINTER { t("Als Bilder speichern", "Save as pictures") } else { t("Drucken", "Print") };
                let label = if order.is_empty() { text.to_string() } else { format!("{text}  ·  {} {}", sheets.len(), t("Blätter", "sheets")) };
                if widgets::primary(ui, &label, vec2(w, 54.0), !order.is_empty()).clicked() {
                    self.start_print(ctx);
                }
            }
        });
    }
}

/// Seconds since 1970 as "YYYY-MM-DD hh:mm:ss" (UTC).
pub fn format_iso(secs: u64) -> String {
    let secs = secs as i64;
    let (y, m, d) = civil(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    format!("{y}-{m:02}-{d:02} {:02}:{:02}:{:02}", rem / 3600, rem / 60 % 60, rem % 60)
}

pub fn format_time(t: std::time::SystemTime) -> String {
    let secs = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil(days);
    match crate::i18n::get() {
        crate::i18n::Lang::De => format!("{d:02}.{m:02}.{y} {:02}:{:02}", rem / 3600, rem / 60 % 60),
        crate::i18n::Lang::En => format!("{y}-{m:02}-{d:02} {:02}:{:02}", rem / 3600, rem / 60 % 60),
    }
}

fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Open a folder in Explorer, or a file with its usual program.
pub fn open_path(dir: &Path) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(dir).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(dir).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
}

/// Name of a preset in the current language.
fn preset_name(k: usize) -> &'static str {
    match k {
        0 => t("10×15 randlos", "10×15 borderless"),
        1 => t("13×18 randlos", "13×18 borderless"),
        2 => t("A4 ganzes Foto", "A4 whole photo"),
        3 => t("4 auf A4", "4 on A4"),
        _ => t("Kontaktbogen", "Contact sheet"),
    }
}

/// Presets: name, paper size in mm (either way round), scale mode, columns × rows, borderless.
const PRESETS: [(&str, (f32, f32), ScaleMode, (u32, u32), bool); 5] = [
    ("10×15 randlos", (102.0, 152.0), ScaleMode::Fill, (1, 1), true),
    ("13×18 randlos", (127.0, 178.0), ScaleMode::Fill, (1, 1), true),
    ("A4 ganzes Foto", (210.0, 297.0), ScaleMode::Fit, (1, 1), false),
    ("4 auf A4", (210.0, 297.0), ScaleMode::Fill, (2, 2), false),
    ("Kontaktbogen", (210.0, 297.0), ScaleMode::Fit, (3, 4), false),
];

/// Same paper size within 3 mm (so 4×6 inch counts as 10×15 cm), in either orientation.
fn paper_matches(w: f32, h: f32, pw: f32, ph: f32) -> bool {
    let close = |a: f32, b: f32| (a - b).abs() <= 3.0;
    (close(w, pw) && close(h, ph)) || (close(w, ph) && close(h, pw))
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;

/// Name of a paper type in this printer's list.
pub fn media_name(caps: &Caps, id: Option<u32>) -> String {
    match id {
        None => t("Wie beim Drucker", "As set in the printer").into(),
        Some(id) => caps.media.iter().find(|m| m.id == id).map(|m| m.name.clone()).unwrap_or_else(|| format!("#{id}")),
    }
}

/// Why the paper type matters.
pub fn media_tip() -> &'static str {
    t(
        "Die Papierart sagt dem Drucker, worauf er druckt. Bei Fotodruckern bestimmt sie Tintenmenge, Druckgeschwindigkeit und Qualität stärker als die Qualitätsstufe – und oft auch, ob randlos gedruckt werden kann.",
        "The paper type tells the printer what it prints on. On photo printers it decides ink amount, print speed and quality more than the quality level does – and often whether borderless printing is possible.",
    )
}

/// At most this many printers' driver settings are remembered.
const MAX_STORED_DRIVERS: usize = 8;

fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || s.len() > 2 * 256 * 1024 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}
