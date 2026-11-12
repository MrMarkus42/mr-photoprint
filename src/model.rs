//! Settings, list items and their defaults (see docs/LAYOUT.md).

use crate::i18n::t;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ScaleMode {
    /// "Verkleinern" – only shrink photos larger than the area.
    Shrink,
    /// "Anpassen" – fit the whole photo, keep proportions.
    Fit,
    /// "Original" – print at the photo's own size.
    AsIs,
    /// "Füllen" – fill the area completely, cut off what sticks out.
    Fill,
    /// "Zoomstufe" – a fixed percentage.
    Zoom,
}

impl ScaleMode {
    pub const ALL: [ScaleMode; 5] = [Self::Fill, Self::Fit, Self::Shrink, Self::AsIs, Self::Zoom];
    pub fn label(self) -> &'static str {
        match self {
            Self::Shrink => t("Verkleinern (nur zu große Fotos)", "Shrink (only photos that are too large)"),
            Self::Fit => t("Anpassen (ganzes Foto)", "Fit (whole photo)"),
            Self::AsIs => t("Original-Größe", "Actual size"),
            Self::Fill => t("Füllen (randlos zuschneiden)", "Fill (crop to the edge)"),
            Self::Zoom => t("Zoomstufe", "Zoom level"),
        }
    }
    pub fn short(self) -> &'static str {
        match self {
            Self::Shrink => t("Verkleinern", "Shrink"),
            Self::Fit => t("Anpassen", "Fit"),
            Self::AsIs => t("Original", "Actual"),
            Self::Fill => t("Füllen", "Fill"),
            Self::Zoom => "Zoom",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum HAlign {
    Left,
    Center,
    Right,
}
impl HAlign {
    pub fn fraction(self) -> f32 {
        match self {
            Self::Left => 0.0,
            Self::Center => 0.5,
            Self::Right => 1.0,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => t("Links", "Left"),
            Self::Center => t("Mitte", "Centre"),
            Self::Right => t("Rechts", "Right"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum VAlign {
    Top,
    Center,
    Bottom,
}
impl VAlign {
    pub fn fraction(self) -> f32 {
        match self {
            Self::Top => 0.0,
            Self::Center => 0.5,
            Self::Bottom => 1.0,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Top => t("Oben", "Top"),
            Self::Center => t("Mitte", "Centre"),
            Self::Bottom => t("Unten", "Bottom"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum AutoRotate {
    None,
    Clockwise,
    CounterClockwise,
}
impl AutoRotate {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => t("Nicht drehen", "Don't turn"),
            Self::Clockwise => t("Im Uhrzeigersinn", "Clockwise"),
            Self::CounterClockwise => t("Gegen den Uhrzeigersinn", "Counter-clockwise"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Orientation {
    AsPrinter,
    Portrait,
    Landscape,
    /// "Basierend auf Seitengröße" – follow each photo.
    ByPhoto,
}
impl Orientation {
    pub const ALL: [Orientation; 4] = [Self::ByPhoto, Self::Portrait, Self::Landscape, Self::AsPrinter];
    pub fn label(self) -> &'static str {
        match self {
            Self::AsPrinter => t("Wie beim Drucker", "As set in the printer"),
            Self::Portrait => t("Hochformat", "Portrait"),
            Self::Landscape => t("Querformat", "Landscape"),
            Self::ByPhoto => t("Passend zum Foto", "Follow the photo"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ColorMode {
    AsPrinter,
    Color,
    Gray,
}
impl ColorMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::AsPrinter => t("Wie beim Drucker", "As set in the printer"),
            Self::Color => t("Farbe", "Colour"),
            Self::Gray => t("Graustufen", "Greyscale"),
        }
    }
}

/// Everything that can be set per photo ("Einstellungen für ausgewählte Elemente").
/// Missing fields take their defaults, so one unknown or older settings file doesn't throw away
/// all settings.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PhotoSettings {
    /// Windows paper id (DMPAPER_…); None = as printer.
    pub paper: Option<i16>,
    /// Paper source (tray) id; None = as printer.
    pub tray: Option<i16>,
    /// Paper type (the driver's media type, e.g. glossy photo paper); None = as printer.
    pub media: Option<u32>,
    pub orientation: Orientation,
    pub scale: ScaleMode,
    pub zoom: u32,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub offset_x_mm: f32,
    pub offset_y_mm: f32,
    pub auto_rotate: AutoRotate,
    /// Manual turn in degrees (0, 90, 180, 270); only used when auto-rotation is off.
    pub rotation: i32,
    pub copies: u32,
    pub color: ColorMode,
}

impl Default for PhotoSettings {
    fn default() -> Self {
        Self {
            paper: None,
            tray: None,
            media: None,
            orientation: Orientation::ByPhoto,
            scale: ScaleMode::Shrink,
            zoom: 100,
            h_align: HAlign::Center,
            v_align: VAlign::Center,
            offset_x_mm: 0.0,
            offset_y_mm: 0.0,
            auto_rotate: AutoRotate::Clockwise,
            rotation: 0,
            copies: 1,
            color: ColorMode::AsPrinter,
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct JobSettings {
    pub printer: String,
    pub photo: PhotoSettings,
    pub ignore_hard_margins: bool,
    pub extra_margin_mm: f32,
    pub cols: u32,
    pub rows: u32,
    pub gap_pct: f32,
    pub frame: bool,
    pub cut_marks: bool,
    pub cut_marks_offset: f32,
    pub next_photo_new_sheet: bool,
    pub single_job: bool,
    pub remove_printed: bool,
    pub export_dir: String,
    pub export_dpi: u32,
    /// Print quality: -999 as printer, -1 draft, -2 low, -3 medium, -4 high; a positive number
    /// is a resolution in dpi.
    pub quality: i32,
}

impl Default for JobSettings {
    fn default() -> Self {
        Self {
            printer: String::new(),
            photo: PhotoSettings::default(),
            ignore_hard_margins: false,
            extra_margin_mm: 0.0,
            cols: 1,
            rows: 1,
            gap_pct: 0.5,
            frame: true,
            cut_marks: false,
            cut_marks_offset: 0.0,
            next_photo_new_sheet: false,
            single_job: false,
            remove_printed: true,
            export_dir: String::new(),
            export_dpi: 300,
            quality: QUALITY_AS_PRINTER,
        }
    }
}

impl JobSettings {
    pub fn collage(&self) -> bool {
        self.cols.max(1) * self.rows.max(1) > 1
    }

    /// Keep values from a settings file (which anyone could have edited) inside sane bounds.
    pub fn sanitize(mut self) -> Self {
        self.cols = self.cols.clamp(1, 10);
        self.rows = self.rows.clamp(1, 10);
        self.gap_pct = finite(self.gap_pct, 0.5).clamp(0.0, 10.0);
        self.extra_margin_mm = finite(self.extra_margin_mm, 0.0).clamp(0.0, 50.0);
        self.cut_marks_offset = finite(self.cut_marks_offset, 0.0).clamp(-100.0, 400.0);
        self.export_dpi = self.export_dpi.clamp(72, 1200);
        if !(self.quality == QUALITY_AS_PRINTER || (-4..=-1).contains(&self.quality) || (50..=4800).contains(&self.quality)) {
            self.quality = QUALITY_AS_PRINTER;
        }
        truncate_at_char(&mut self.printer, 512);
        truncate_at_char(&mut self.export_dir, 4096);
        self.photo = self.photo.sanitize();
        self
    }

    /// Settings from a file someone else may have made ("Importieren …"): size-limited, every
    /// value clamped, and an export folder on another computer dropped – saving there would make
    /// Windows log in to that computer.
    pub fn from_untrusted_json(text: &str) -> Option<JobSettings> {
        if text.len() > MAX_SETTINGS_BYTES {
            return None;
        }
        let mut j = serde_json::from_str::<JobSettings>(text).ok()?.sanitize();
        if is_network_path(std::path::Path::new(&j.export_dir)) {
            j.export_dir.clear();
        }
        Some(j)
    }
}

/// Largest settings file that is read.
pub const MAX_SETTINGS_BYTES: usize = 1_000_000;

/// Replace a file in one step: a crash or full disk while writing leaves the old file intact
/// instead of a half-written one.
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

impl PhotoSettings {
    pub fn sanitize(mut self) -> Self {
        self.zoom = self.zoom.clamp(5, 1000);
        self.copies = self.copies.clamp(1, 999);
        self.rotation = self.rotation.rem_euclid(360) / 90 * 90;
        self.offset_x_mm = finite(self.offset_x_mm, 0.0).clamp(-500.0, 500.0);
        self.offset_y_mm = finite(self.offset_y_mm, 0.0).clamp(-500.0, 500.0);
        self
    }
}

/// Shorten to at most `max` bytes without cutting a character in half (`String::truncate`
/// panics there – a settings file with "€" or "ä" in the right place would stop the start).
fn truncate_at_char(s: &mut String, max: usize) {
    if s.len() > max {
        let mut i = max;
        while !s.is_char_boundary(i) {
            i -= 1;
        }
        s.truncate(i);
    }
}

fn finite(v: f32, default: f32) -> f32 {
    if v.is_finite() { v } else { default }
}

/// A path on another computer (`\\server\share\…`) – or one that could lead there. Opening one
/// makes Windows log in to that computer, so paths from imported lists and settings are never
/// used when they point to the network. Of the `\\?\` / `\\.\` forms only a plain drive path
/// (`\\?\C:\…`) counts as local; `UNC`, `GLOBALROOT\Device\Mup`, devices and volume names don't.
pub fn is_network_path(p: &std::path::Path) -> bool {
    let s = p.to_string_lossy().replace('/', "\\");
    if let Some(rest) = s.strip_prefix("\\\\?\\").or_else(|| s.strip_prefix("\\\\.\\")).or_else(|| s.strip_prefix("\\??\\")) {
        let b = rest.as_bytes();
        return !(b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\');
    }
    s.starts_with("\\\\")
}

/// Most photos taken from one folder scan, and how deep it goes.
pub const MAX_SCAN_FILES: usize = 50_000;
pub const MAX_SCAN_DEPTH: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemStatus {
    Waiting,
    Printing,
    Printed,
    Failed,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub path: PathBuf,
    pub name: String,
    pub modified: Option<std::time::SystemTime>,
    pub bytes: u64,
    /// Pixel size after the camera's orientation tag has been applied (0 until known).
    pub px: (u32, u32),
    /// Pixels per inch the file declares (used for "Original-Größe").
    pub dpi: Option<f32>,
    pub status: ItemStatus,
    pub message: String,
    pub checked: bool,
    pub selected: bool,
    pub own: Option<PhotoSettings>,
    /// Fine position of the photo inside its area, 0..1 per axis (drag in the preview).
    pub fine: Option<(f32, f32)>,
}

impl Item {
    pub fn new(path: PathBuf) -> Self {
        let meta = std::fs::metadata(&path).ok();
        Self {
            name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            modified: meta.as_ref().and_then(|m| m.modified().ok()),
            bytes: meta.map(|m| m.len()).unwrap_or(0),
            path,
            px: (0, 0),
            dpi: None,
            status: ItemStatus::Waiting,
            message: String::new(),
            checked: true,
            selected: false,
            own: None,
            fine: None,
        }
    }

    pub fn settings<'a>(&'a self, job: &'a JobSettings) -> &'a PhotoSettings {
        self.own.as_ref().unwrap_or(&job.photo)
    }
}

pub const PHOTO_EXTENSIONS: &[&str] = &["jpg", "jpeg", "jpe", "jfif", "png", "tif", "tiff", "bmp", "webp", "gif", "heic", "heif", "hif", "avif", "jxl", "ico", "tga", "pnm", "ppm", "pgm", "pbm", "qoi", "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "orf", "rw2", "raf", "pef", "srw", "erf", "kdc", "mrw", "3fr", "x3f"];

pub fn is_photo(path: &std::path::Path) -> bool {
    path.extension()
        .map(|e| PHOTO_EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Print quality values (see `JobSettings::quality`).
pub const QUALITY_AS_PRINTER: i32 = -999;
pub const QUALITY_DRAFT: i32 = -1;
pub const QUALITY_LOW: i32 = -2;
pub const QUALITY_MEDIUM: i32 = -3;
pub const QUALITY_HIGH: i32 = -4;

pub fn quality_label(q: i32) -> String {
    match q {
        QUALITY_AS_PRINTER => t("Wie beim Drucker", "As set in the printer").into(),
        QUALITY_DRAFT => t("Entwurf", "Draft").into(),
        QUALITY_LOW => t("Niedrig", "Low").into(),
        QUALITY_MEDIUM => t("Mittel", "Medium").into(),
        QUALITY_HIGH => t("Hoch", "High").into(),
        d => format!("{d} dpi"),
    }
}

/// Resolution used when saving sheets as image files. Positive values are dpi; the named levels
/// map to 150 / 200 / 300 / 600 dpi.
pub fn export_dpi_for(job: &JobSettings) -> u32 {
    match job.quality {
        QUALITY_DRAFT => 150,
        QUALITY_LOW => 200,
        QUALITY_MEDIUM => 300,
        QUALITY_HIGH => 600,
        d if d > 0 => (d as u32).clamp(72, 1200),
        _ => job.export_dpi,
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
