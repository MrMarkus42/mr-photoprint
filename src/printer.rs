//! Printers: what's installed, which papers and trays they offer, how big the printable part
//! is, and the print job itself (Windows GDI). The "save as JPEG files" printer works everywhere.

use crate::i18n::{t, ts};
use crate::layout::{PageInfo, Sheet};
use crate::model::{ColorMode, Item, JobSettings};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

/// The built-in "printer" that saves each sheet as a JPEG file. The value is only an id (it is
/// stored in the settings); on screen it is shown with `printer_label`.
pub const EXPORT_PRINTER: &str = "In JPEG-Dateien speichern";

/// A printer name as shown on screen.
pub fn printer_label(name: &str) -> &str {
    if name == EXPORT_PRINTER { t("In JPEG-Dateien speichern", "Save as JPEG files") } else { name }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaperDef {
    pub id: i16,
    pub name: String,
    pub w_mm: f32,
    pub h_mm: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrayDef {
    pub id: i16,
    pub name: String,
}

/// A paper type ("media type") as the driver names it.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaDef {
    pub id: u32,
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct Caps {
    pub papers: Vec<PaperDef>,
    pub trays: Vec<TrayDef>,
    /// Resolutions the driver offers (x, y dpi).
    pub resolutions: Vec<(i32, i32)>,
    /// Paper types (media) the driver offers: glossy photo paper, plain paper …
    pub media: Vec<MediaDef>,
}

/// Paper sizes offered by the JPEG export (photo papers first).
pub fn export_papers() -> Vec<PaperDef> {
    let p = |id, name: &str, w, h| PaperDef { id, name: name.to_string(), w_mm: w, h_mm: h };
    vec![
        p(1001, t("Foto 9 × 13 cm", "Photo 9 × 13 cm"), 89.0, 127.0),
        p(1002, t("Foto 10 × 15 cm", "Photo 10 × 15 cm"), 102.0, 152.0),
        p(1003, t("Foto 13 × 18 cm", "Photo 13 × 18 cm"), 127.0, 178.0),
        p(1004, t("Foto 15 × 20 cm", "Photo 15 × 20 cm"), 152.0, 203.0),
        p(1005, t("Foto 20 × 30 cm", "Photo 20 × 30 cm"), 203.0, 305.0),
        p(70, "A6 (105 × 148 mm)", 105.0, 148.0),
        p(11, "A5 (148 × 210 mm)", 148.0, 210.0),
        p(9, "A4 (210 × 297 mm)", 210.0, 297.0),
        p(8, "A3 (297 × 420 mm)", 297.0, 420.0),
        p(1, t("Letter (8,5 × 11 in)", "Letter (8.5 × 11 in)"), 215.9, 279.4),
    ]
}

/// Does a paper or paper-source name say "borderless" (in the languages drivers use)?
pub fn is_borderless_name(name: &str) -> bool {
    let n = name.to_lowercase();
    ["randlos", "borderless", "sans marge", "sans bord", "senza margini", "senza bordi", "sin márgenes", "sin bordes", "zonder rand", "randloos", "bez okraj", "bez margines", "kenarsız", "no margin"]
        .iter()
        .any(|k| n.contains(k))
}

/// How this driver prints borderless: printers either have a borderless paper *source* (Epson:
/// "Einzug hinten-Randlos") or a borderless variant of each paper *size* (Canon, HP:
/// "10 x 15 cm (randlos)"). Returns the paper and source to use; a source the person picked
/// themselves is kept.
pub fn borderless_variant(caps: &Caps, paper: Option<i16>, tray: Option<i16>) -> (Option<i16>, Option<i16>) {
    let tray = tray.or_else(|| caps.trays.iter().find(|t| is_borderless_name(&t.name)).map(|t| t.id));
    let paper = paper.map(|id| {
        let Some(cur) = caps.papers.iter().find(|p| p.id == id) else { return id };
        if is_borderless_name(&cur.name) {
            return id;
        }
        let same = |p: &PaperDef| {
            let close = |a: f32, b: f32| (a - b).abs() <= 1.5;
            (close(p.w_mm, cur.w_mm) && close(p.h_mm, cur.h_mm)) || (close(p.w_mm, cur.h_mm) && close(p.h_mm, cur.w_mm))
        };
        caps.papers.iter().find(|p| is_borderless_name(&p.name) && same(p)).map(|p| p.id).unwrap_or(id)
    });
    (paper, tray)
}

/// A printer's own settings (DEVMODE) read from a file: only kept when they pass the size checks.
pub fn valid_devmode(buf: &[u8]) -> Option<Vec<u8>> {
    #[cfg(windows)]
    {
        sys::checked_devmode(buf)
    }
    #[cfg(not(windows))]
    {
        Some(buf.to_vec())
    }
}

pub fn orient(w: f32, h: f32, landscape: Option<bool>) -> (f32, f32) {
    match landscape {
        Some(true) => (w.max(h), w.min(h)),
        Some(false) => (w.min(h), w.max(h)),
        None => (w, h),
    }
}

/// Messages from the print job to the window.
pub enum JobEvent {
    Sheet { done: usize, total: usize },
    ItemDone { item: usize, ok: bool, message: String },
    Finished { ok: bool, message: String },
}

pub struct JobControl {
    pub pause: AtomicBool,
    pub stop: AtomicBool,
}

pub struct PrintJob {
    pub printer: String,
    pub devmode: Option<Vec<u8>>,
    pub sheets: Vec<Sheet>,
    pub items: Vec<Item>,
    pub job: JobSettings,
}

/// Run the job on its own thread.
pub fn start(job: PrintJob, ctx: egui::Context) -> (mpsc::Receiver<JobEvent>, Arc<JobControl>) {
    let (tx, rx) = mpsc::channel();
    let ctl = Arc::new(JobControl { pause: AtomicBool::new(false), stop: AtomicBool::new(false) });
    let c = ctl.clone();
    std::thread::spawn(move || {
        let send = |e: JobEvent| {
            let _ = tx.send(e);
            ctx.request_repaint();
        };
        // A crash on this thread must still end the job in the window (otherwise it would show
        // "printing" forever), so it becomes an ordinary error.
        let result = check_job(&job).and_then(|_| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if job.printer == EXPORT_PRINTER { run_export(&job, &c, &send) } else { run_print(&job, &c, &send) }
            }))
            .unwrap_or_else(|_| Err(t("Interner Fehler – der Auftrag wurde abgebrochen.", "Internal error – the job was cancelled.").into()))
        });
        match result {
            Ok(n) => send(JobEvent::Finished { ok: true, message: ts(format!("{n} Blätter verarbeitet."), format!("{n} sheets done.")) }),
            Err(e) => send(JobEvent::Finished { ok: false, message: e }),
        }
    });
    (rx, ctl)
}

/// Every sheet has photos and points only at photos the job carries, so the print loops can
/// index without checks.
fn check_job(job: &PrintJob) -> Result<(), String> {
    let n = job.items.len();
    let bad = job.sheets.iter().any(|s| s.cells.is_empty() || s.settings_of >= n || s.cells.iter().any(|c| c.item >= n));
    if bad { Err(t("Interner Fehler: Der Druckplan passt nicht zur Fotoliste.", "Internal error: the sheet plan does not match the photo list.").into()) } else { Ok(()) }
}

fn wait_if_paused(c: &JobControl) -> bool {
    while c.pause.load(Ordering::Relaxed) && !c.stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    !c.stop.load(Ordering::Relaxed)
}

/// After sheet `i`, every item whose last sheet that was is finished.
fn finished_items(sheets: &[Sheet], i: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for cell in &sheets[i].cells {
        let later = sheets[i + 1..].iter().any(|s| s.cells.iter().any(|c| c.item == cell.item));
        if !later && !out.contains(&cell.item) {
            out.push(cell.item);
        }
    }
    out
}

fn run_export(job: &PrintJob, c: &JobControl, send: &dyn Fn(JobEvent)) -> Result<usize, String> {
    let dir = PathBuf::from(&job.job.export_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{} {}: {e}", t("Ordner", "Folder"), dir.display()))?;
    let dpi = crate::model::export_dpi_for(&job.job) as f32;
    let mut cache = None;
    let total = job.sheets.len();
    for (i, sheet) in job.sheets.iter().enumerate() {
        if !wait_if_paused(c) {
            return Err(t("Vom Benutzer gestoppt.", "Stopped by the user.").into());
        }
        let first = &job.items[sheet.cells[0].item];
        let stem = first.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let out = dir.join(format!("{}_{:04}_{stem}.jpg", t("Blatt", "Sheet"), i + 1));
        let res = crate::render::render_sheet(sheet, &job.items, &job.job, dpi, &mut cache).and_then(|img| {
            let f = std::fs::File::create(&out).map_err(|e| e.to_string())?;
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(f), 95);
            enc.encode_image(&img).map_err(|e| e.to_string())
        });
        for it in finished_items(&job.sheets, i) {
            send(JobEvent::ItemDone {
                item: it,
                ok: res.is_ok(),
                message: res.as_ref().err().cloned().unwrap_or_else(|| t("Gespeichert", "Saved").into()),
            });
        }
        send(JobEvent::Sheet { done: i + 1, total });
    }
    Ok(total)
}

#[cfg(not(windows))]
mod sys {
    use super::*;
    pub fn list_printers() -> (Vec<String>, Option<String>) {
        (vec![], None)
    }
    pub fn caps(_printer: &str, _dm: Option<&[u8]>) -> Caps {
        Caps { papers: export_papers(), ..Default::default() }
    }
    pub fn page_info(_p: &str, _dm: Option<&[u8]>, paper: Option<i16>, _tray: Option<i16>, _media: Option<u32>, landscape: Option<bool>) -> PageInfo {
        let p = export_papers().into_iter().find(|p| Some(p.id) == paper).unwrap_or_else(|| export_papers()[7].clone());
        let (w, h) = orient(p.w_mm, p.h_mm, landscape);
        PageInfo::plain(w, h, 300.0)
    }
    pub fn properties_dialog(_p: &str, _hwnd: isize, dm: Option<Vec<u8>>) -> Option<Vec<u8>> {
        dm
    }
    pub fn run_print(_job: &PrintJob, _c: &JobControl, _s: &dyn Fn(JobEvent)) -> Result<usize, String> {
        Err(t("Drucken ist nur unter Windows möglich.", "Printing only works on Windows.").into())
    }
}

#[cfg(windows)]
mod sys {
    use super::*;
    use crate::render::{load_oriented, prepare, wants_gray, decoration_lines};
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::Graphics::Printing::*;
    use windows::Win32::Storage::Xps::*;
    use windows::core::{HSTRING, PCWSTR, PWSTR};

    fn wide_to_string(p: PWSTR) -> String {
        if p.is_null() { String::new() } else { unsafe { p.to_string().unwrap_or_default() } }
    }

    pub fn list_printers() -> (Vec<String>, Option<String>) {
        let mut names = Vec::new();
        unsafe {
            let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
            let (mut need, mut count) = (0u32, 0u32);
            let _ = EnumPrintersW(flags, PCWSTR::null(), 4, None, &mut need, &mut count);
            if need > 0 {
                let mut buf = vec![0u8; need as usize];
                if EnumPrintersW(flags, PCWSTR::null(), 4, Some(&mut buf), &mut need, &mut count).is_ok() {
                    let infos = std::slice::from_raw_parts(buf.as_ptr() as *const PRINTER_INFO_4W, count as usize);
                    for i in infos {
                        names.push(wide_to_string(i.pPrinterName));
                    }
                }
            }
            let mut len = 0u32;
            let _ = GetDefaultPrinterW(None, &mut len);
            let default = if len > 0 {
                let mut b = vec![0u16; len as usize];
                if GetDefaultPrinterW(Some(PWSTR(b.as_mut_ptr())), &mut len).as_bool() {
                    Some(String::from_utf16_lossy(&b[..(len as usize).saturating_sub(1)]))
                } else {
                    None
                }
            } else {
                None
            };
            names.sort_by_key(|n| n.to_lowercase());
            (names, default)
        }
    }

    /// Size of the fixed part of a DEVMODE as this program knows it.
    const DM_FIXED: usize = std::mem::size_of::<DEVMODEW>();
    /// Largest DEVMODE (fixed part + driver data) we accept; real drivers stay far below.
    const DM_MAX: usize = 1 << 20;

    /// Check a DEVMODE before Windows or a printer driver reads it. The sizes it states for
    /// itself (dmSize + dmDriverExtra) must fit inside the buffer – otherwise the driver would
    /// read past its end. An older, shorter fixed part is widened to today's size (keeping the
    /// driver's private data behind it), so the fields we set never land in that private data.
    pub(crate) fn checked_devmode(buf: &[u8]) -> Option<Vec<u8>> {
        use std::mem::offset_of;
        let at = |o: usize| buf.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
        let size = at(offset_of!(DEVMODEW, dmSize))?;
        let extra = at(offset_of!(DEVMODEW, dmDriverExtra))?;
        let min = offset_of!(DEVMODEW, dmFields) + 4;
        if size < min || size + extra > buf.len() || size + extra > DM_MAX {
            return None;
        }
        if size >= DM_FIXED {
            return Some(buf[..size + extra].to_vec());
        }
        let mut out = vec![0u8; DM_FIXED + extra];
        out[..size].copy_from_slice(&buf[..size]);
        out[DM_FIXED..].copy_from_slice(&buf[size..size + extra]);
        let o = offset_of!(DEVMODEW, dmSize);
        out[o..o + 2].copy_from_slice(&(DM_FIXED as u16).to_le_bytes());
        Some(out)
    }

    /// A buffer big enough for whatever DEVMODE this printer's driver writes.
    unsafe fn out_buffer(h: PRINTER_HANDLE, name: &HSTRING, at_least: usize) -> Option<Vec<u8>> {
        let need = unsafe { DocumentPropertiesW(None, h, name, None, None, 0) };
        if need <= 0 || need as usize > DM_MAX {
            return None;
        }
        Some(vec![0u8; (need as usize).max(at_least)])
    }

    /// The printer's own settings (DEVMODE) as bytes.
    pub fn default_devmode(printer: &str) -> Option<Vec<u8>> {
        unsafe {
            let name = HSTRING::from(printer);
            let mut h = PRINTER_HANDLE::default();
            OpenPrinterW(&name, &mut h, None).ok()?;
            let out = out_buffer(h, &name, 0).and_then(|mut buf| {
                let r = DocumentPropertiesW(None, h, &name, Some(buf.as_mut_ptr() as *mut DEVMODEW), None, DM_OUT_BUFFER.0);
                (r >= 0).then_some(buf)
            });
            let _ = ClosePrinter(h);
            checked_devmode(&out?)
        }
    }

    /// A copy of the printer's settings with our paper, tray, orientation and colour.
    pub(crate) fn devmode_for(printer: &str, base: Option<&[u8]>, paper: Option<i16>, tray: Option<i16>, media: Option<u32>, landscape: Option<bool>, color: ColorMode, quality: i32) -> Option<Vec<u8>> {
        let mut buf = match base {
            Some(b) => checked_devmode(b)?,
            None => default_devmode(printer)?,
        };
        // Drivers check each setting against the others as they are *before* the change: an
        // Epson driver, for example, refuses its borderless paper source while the paper type is
        // still plain paper. So the settings are applied and handed to the driver again (at most
        // three rounds) until it keeps what was asked for.
        for _ in 0..3 {
            let mut dm: DEVMODEW = read_dm(&buf);
            set_fields(&mut dm, paper, tray, media, landscape, color, quality);
            write_dm(&mut buf, dm);
            match driver_merge(printer, &buf) {
                Some(out) => buf = out,
                None => break,
            }
            if keeps(&read_dm(&buf), paper, tray, media) {
                break;
            }
        }
        Some(buf)
    }

    /// A byte buffer has no alignment guarantee, so the fixed part is copied out and back
    /// instead of being referenced in place.
    fn read_dm(buf: &[u8]) -> DEVMODEW {
        unsafe { std::ptr::read_unaligned(buf.as_ptr() as *const DEVMODEW) }
    }

    fn write_dm(buf: &mut [u8], dm: DEVMODEW) {
        unsafe { std::ptr::write_unaligned(buf.as_mut_ptr() as *mut DEVMODEW, dm) }
    }

    /// Did the driver keep the paper, source and type we asked for?
    fn keeps(dm: &DEVMODEW, paper: Option<i16>, tray: Option<i16>, media: Option<u32>) -> bool {
        let (p, s) = unsafe { (dm.Anonymous1.Anonymous1.dmPaperSize, dm.Anonymous1.Anonymous1.dmDefaultSource) };
        paper.is_none_or(|x| x == p) && tray.is_none_or(|x| x == s) && media.is_none_or(|x| x == dm.dmMediaType)
    }

    /// Let the driver check and complete a DEVMODE.
    fn driver_merge(printer: &str, buf: &[u8]) -> Option<Vec<u8>> {
        unsafe {
            let name = HSTRING::from(printer);
            let mut h = PRINTER_HANDLE::default();
            OpenPrinterW(&name, &mut h, None).ok()?;
            let r = out_buffer(h, &name, buf.len()).and_then(|mut out| {
                let r = DocumentPropertiesW(
                    None,
                    h,
                    &name,
                    Some(out.as_mut_ptr() as *mut DEVMODEW),
                    Some(buf.as_ptr() as *const DEVMODEW),
                    DM_IN_BUFFER.0 | DM_OUT_BUFFER.0,
                );
                (r >= 0).then_some(out)
            });
            let _ = ClosePrinter(h);
            r.and_then(|o| checked_devmode(&o))
        }
    }

    fn set_fields(dm: &mut DEVMODEW, paper: Option<i16>, tray: Option<i16>, media: Option<u32>, landscape: Option<bool>, color: ColorMode, quality: i32) {
        {
            if let Some(m) = media {
                dm.dmFields |= DM_MEDIATYPE;
                dm.dmMediaType = m;
            }
            if let Some(p) = paper {
                dm.dmFields |= DM_PAPERSIZE;
                dm.Anonymous1.Anonymous1.dmPaperSize = p;
            }
            if let Some(t) = tray {
                dm.dmFields |= DM_DEFAULTSOURCE;
                dm.Anonymous1.Anonymous1.dmDefaultSource = t;
            }
            if let Some(l) = landscape {
                dm.dmFields |= DM_ORIENTATION;
                dm.Anonymous1.Anonymous1.dmOrientation = if l { 2 } else { 1 };
            }
            match color {
                ColorMode::AsPrinter => {}
                ColorMode::Color => {
                    dm.dmFields |= DM_COLOR;
                    dm.dmColor = DMCOLOR_COLOR;
                }
                ColorMode::Gray => {
                    dm.dmFields |= DM_COLOR;
                    dm.dmColor = DMCOLOR_MONOCHROME;
                }
            }
            // Print quality:
            // -999 leaves the driver alone, -1..-4 are draft..high, positive values are dpi.
            if quality != crate::model::QUALITY_AS_PRINTER {
                dm.dmFields |= DM_PRINTQUALITY;
                dm.Anonymous1.Anonymous1.dmPrintQuality = quality as i16;
                if quality > 0 {
                    dm.dmFields |= DM_YRESOLUTION;
                    dm.dmYResolution = quality as i16;
                }
            }
            dm.dmFields |= DM_COPIES;
            dm.Anonymous1.Anonymous1.dmCopies = 1;
        }
    }

    pub fn caps(printer: &str, dm: Option<&[u8]>) -> Caps {
        let mut caps = Caps::default();
        let dm = dm.and_then(checked_devmode);
        unsafe {
            let name = HSTRING::from(printer);
            let dmp = dm.as_ref().map(|d| d.as_ptr() as *const DEVMODEW);
            let n = DeviceCapabilitiesW(&name, PCWSTR::null(), DC_PAPERS, None, dmp);
            if n > 0 {
                let n = n as usize;
                let mut ids = vec![0u16; n];
                let mut names = vec![0u16; n * 64];
                let mut sizes = vec![0i32; n * 2];
                DeviceCapabilitiesW(&name, PCWSTR::null(), DC_PAPERS, Some(PWSTR(ids.as_mut_ptr())), dmp);
                DeviceCapabilitiesW(&name, PCWSTR::null(), DC_PAPERNAMES, Some(PWSTR(names.as_mut_ptr())), dmp);
                DeviceCapabilitiesW(&name, PCWSTR::null(), DC_PAPERSIZE, Some(PWSTR(sizes.as_mut_ptr() as *mut u16)), dmp);
                for i in 0..n {
                    let raw = &names[i * 64..i * 64 + 64];
                    let end = raw.iter().position(|&c| c == 0).unwrap_or(64);
                    caps.papers.push(PaperDef {
                        id: ids[i] as i16,
                        name: String::from_utf16_lossy(&raw[..end]),
                        w_mm: sizes[i * 2] as f32 / 10.0,
                        h_mm: sizes[i * 2 + 1] as f32 / 10.0,
                    });
                }
            }
            let n = DeviceCapabilitiesW(&name, PCWSTR::null(), DC_BINS, None, dmp);
            if n > 0 {
                let n = n as usize;
                let mut ids = vec![0u16; n];
                let mut names = vec![0u16; n * 24];
                DeviceCapabilitiesW(&name, PCWSTR::null(), DC_BINS, Some(PWSTR(ids.as_mut_ptr())), dmp);
                DeviceCapabilitiesW(&name, PCWSTR::null(), DC_BINNAMES, Some(PWSTR(names.as_mut_ptr())), dmp);
                for i in 0..n {
                    let raw = &names[i * 24..i * 24 + 24];
                    let end = raw.iter().position(|&c| c == 0).unwrap_or(24);
                    caps.trays.push(TrayDef { id: ids[i] as i16, name: String::from_utf16_lossy(&raw[..end]) });
                }
            }
            let n = DeviceCapabilitiesW(&name, PCWSTR::null(), DC_MEDIATYPES, None, dmp);
            if n > 0 && n < 512 {
                let n = n as usize;
                let mut ids = vec![0u32; n];
                let mut names = vec![0u16; n * 64];
                let got = DeviceCapabilitiesW(&name, PCWSTR::null(), DC_MEDIATYPES, Some(PWSTR(ids.as_mut_ptr() as *mut u16)), dmp);
                let got_names = DeviceCapabilitiesW(&name, PCWSTR::null(), DC_MEDIATYPENAMES, Some(PWSTR(names.as_mut_ptr())), dmp);
                if got as usize == n && got_names as usize == n {
                    for i in 0..n {
                        let raw = &names[i * 64..i * 64 + 64];
                        let end = raw.iter().position(|&c| c == 0).unwrap_or(64);
                        let label = String::from_utf16_lossy(&raw[..end]).trim().to_string();
                        if !label.is_empty() && !caps.media.iter().any(|m| m.id == ids[i]) {
                            caps.media.push(MediaDef { id: ids[i], name: label });
                        }
                    }
                }
            }
            let n = DeviceCapabilitiesW(&name, PCWSTR::null(), DC_ENUMRESOLUTIONS, None, dmp);
            if n > 0 && n < 256 {
                let mut res = vec![0i32; n as usize * 2];
                DeviceCapabilitiesW(&name, PCWSTR::null(), DC_ENUMRESOLUTIONS, Some(PWSTR(res.as_mut_ptr() as *mut u16)), dmp);
                for k in 0..n as usize {
                    let (x, y) = (res[k * 2], res[k * 2 + 1]);
                    if (50..=4800).contains(&x) && !caps.resolutions.contains(&(x, y)) {
                        caps.resolutions.push((x, y));
                    }
                }
                caps.resolutions.sort();
            }
        }
        caps
    }

    struct DcPage {
        dpi: (f32, f32),
        phys: (f32, f32),
        offset: (f32, f32),
        printable: (f32, f32),
    }

    unsafe fn measure(hdc: HDC) -> DcPage {
        unsafe {
            let g = |i| GetDeviceCaps(Some(hdc), i) as f32;
            let dpi = (g(LOGPIXELSX).max(1.0), g(LOGPIXELSY).max(1.0));
            let mm = |v: f32, d: f32| v / d * 25.4;
            DcPage {
                dpi,
                phys: (mm(g(PHYSICALWIDTH), dpi.0), mm(g(PHYSICALHEIGHT), dpi.1)),
                offset: (mm(g(PHYSICALOFFSETX), dpi.0), mm(g(PHYSICALOFFSETY), dpi.1)),
                printable: (mm(g(HORZRES), dpi.0), mm(g(VERTRES), dpi.1)),
            }
        }
    }

    fn to_info(m: &DcPage) -> PageInfo {
        PageInfo {
            width_mm: m.phys.0,
            height_mm: m.phys.1,
            hard: [
                m.offset.0,
                m.offset.1,
                (m.phys.0 - m.offset.0 - m.printable.0).max(0.0),
                (m.phys.1 - m.offset.1 - m.printable.1).max(0.0),
            ],
            dpi: m.dpi,
            bleed: m.offset.0 < 0.1 && m.offset.1 < 0.1 && (m.phys.0 - m.printable.0) < 0.2 && (m.phys.1 - m.printable.1) < 0.2,
        }
    }

    pub fn page_info(printer: &str, base: Option<&[u8]>, paper: Option<i16>, tray: Option<i16>, media: Option<u32>, landscape: Option<bool>) -> PageInfo {
        let fallback = || {
            let (w, h) = orient(210.0, 297.0, landscape);
            PageInfo { width_mm: w, height_mm: h, hard: [3.0; 4], dpi: (600.0, 600.0), bleed: false }
        };
        let Some(dm) = devmode_for(printer, base, paper, tray, media, landscape, ColorMode::AsPrinter, crate::model::QUALITY_AS_PRINTER) else { return fallback() };
        unsafe {
            let name = HSTRING::from(printer);
            let hdc = CreateICW(PCWSTR::null(), &name, PCWSTR::null(), Some(dm.as_ptr() as *const DEVMODEW));
            if hdc.is_invalid() {
                return fallback();
            }
            let m = measure(hdc);
            let _ = DeleteDC(hdc);
            if m.phys.0 < 1.0 { fallback() } else { to_info(&m) }
        }
    }

    pub fn properties_dialog(printer: &str, hwnd: isize, dm: Option<Vec<u8>>) -> Option<Vec<u8>> {
        let base = dm.as_deref().and_then(checked_devmode).or_else(|| default_devmode(printer))?;
        unsafe {
            let name = HSTRING::from(printer);
            let mut h = PRINTER_HANDLE::default();
            OpenPrinterW(&name, &mut h, None).ok()?;
            let Some(mut out) = out_buffer(h, &name, base.len()) else {
                let _ = ClosePrinter(h);
                return None;
            };
            let r = DocumentPropertiesW(
                Some(HWND(hwnd as *mut _)),
                h,
                &name,
                Some(out.as_mut_ptr() as *mut DEVMODEW),
                Some(base.as_ptr() as *const DEVMODEW),
                DM_IN_BUFFER.0 | DM_OUT_BUFFER.0 | DM_IN_PROMPT.0,
            );
            let _ = ClosePrinter(h);
            // IDOK = 1
            if r == 1 { checked_devmode(&out) } else { None }
        }
    }

    unsafe fn blit(hdc: HDC, p: &crate::render::Prepared) {
        let (w, h) = (p.img.width() as i32, p.img.height() as i32);
        let stride = ((w * 3 + 3) / 4 * 4) as usize;
        let mut bits = vec![0u8; stride * h as usize];
        for (y, row) in p.img.rows().enumerate() {
            let line = &mut bits[y * stride..];
            for (x, px) in row.enumerate() {
                line[x * 3] = px[2];
                line[x * 3 + 1] = px[1];
                line[x * 3 + 2] = px[0];
            }
        }
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = -h;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 24;
        bmi.bmiHeader.biCompression = BI_RGB.0;
        unsafe {
            SetStretchBltMode(hdc, HALFTONE);
            let _ = SetBrushOrgEx(hdc, 0, 0, None);
            StretchDIBits(hdc, p.dest.0, p.dest.1, p.dest.2, p.dest.3, 0, 0, w, h, Some(bits.as_ptr() as *const _), &bmi, DIB_RGB_COLORS, SRCCOPY);
        }
    }

    pub fn run_print(job: &PrintJob, c: &JobControl, send: &dyn Fn(JobEvent)) -> Result<usize, String> {
        let total = job.sheets.len();
        let name = HSTRING::from(job.printer.as_str());
        let mut hdc: Option<(HDC, Vec<u8>)> = None;
        let mut cache: Option<(PathBuf, image::DynamicImage)> = None;
        let doc_name = HSTRING::from("Mr. PhotoPrint");
        let one_job = job.job.single_job || job.job.collage();
        let finish = |hdc: &mut Option<(HDC, Vec<u8>)>| unsafe {
            if let Some((h, _)) = hdc.take() {
                EndDoc(h);
                let _ = DeleteDC(h);
            }
        };
        for (i, sheet) in job.sheets.iter().enumerate() {
            if !wait_if_paused(c) {
                if let Some((h, _)) = hdc.take() {
                    unsafe {
                        AbortDoc(h);
                        let _ = DeleteDC(h);
                    }
                }
                return Err(t("Vom Benutzer gestoppt.", "Stopped by the user.").into());
            }
            let first = &job.items[sheet.settings_of];
            let color = first.settings(&job.job).color;
            let dm = devmode_for(&job.printer, job.devmode.as_deref(), sheet.paper, sheet.tray, sheet.media, sheet.landscape, color, job.job.quality)
                .ok_or(t("Die Druckereinstellungen können nicht gelesen werden.", "The printer settings cannot be read."))?;
            let new_doc = !one_job && i > 0 && job.sheets[i - 1].settings_of != sheet.settings_of;
            if new_doc {
                finish(&mut hdc);
            }
            unsafe {
                match &mut hdc {
                    None => {
                        let h = CreateDCW(PCWSTR::null(), &name, PCWSTR::null(), Some(dm.as_ptr() as *const DEVMODEW));
                        if h.is_invalid() {
                            return Err(ts(format!("Drucker „{}“ kann nicht geöffnet werden.", job.printer), format!("Printer \"{}\" cannot be opened.", job.printer)));
                        }
                        let title = if one_job { doc_name.clone() } else { HSTRING::from(first.name.as_str()) };
                        let di = DOCINFOW { cbSize: std::mem::size_of::<DOCINFOW>() as i32, lpszDocName: PCWSTR(title.as_ptr()), ..Default::default() };
                        if StartDocW(h, &di) <= 0 {
                            let _ = DeleteDC(h);
                            return Err(t("Der Druckauftrag konnte nicht gestartet werden.", "The print job could not be started.").into());
                        }
                        hdc = Some((h, dm));
                    }
                    Some((h, cur)) => {
                        if *cur != dm {
                            ResetDCW(*h, dm.as_ptr() as *const DEVMODEW);
                            *cur = dm;
                        }
                    }
                }
                let h = hdc.as_ref().unwrap().0;
                if StartPage(h) <= 0 {
                    finish(&mut hdc);
                    return Err(t("Seite konnte nicht begonnen werden.", "The page could not be started.").into());
                }
                let m = measure(h);
                let mut errors = Vec::new();
                for cell in &sheet.cells {
                    let item = &job.items[cell.item];
                    if cache.as_ref().map(|c| &c.0) != Some(&item.path) {
                        match load_oriented(&item.path) {
                            Ok(img) => cache = Some((item.path.clone(), img)),
                            Err(e) => {
                                errors.push((cell.item, e));
                                continue;
                            }
                        }
                    }
                    let img = &cache.as_ref().unwrap().1;
                    if let Some(p) = prepare(img, &cell.placement, m.dpi, m.offset, wants_gray(item, &job.job), false) {
                        blit(h, &p);
                    }
                }
                let px = |v: f32, d: f32| (v / 25.4 * d).round() as i32;
                let brush = GetStockObject(BLACK_BRUSH);
                for r in decoration_lines(sheet, &job.job) {
                    let x0 = px(r.x - m.offset.0, m.dpi.0);
                    let y0 = px(r.y - m.offset.1, m.dpi.1);
                    let rc = RECT { left: x0, top: y0, right: x0 + px(r.w, m.dpi.0).max(1), bottom: y0 + px(r.h, m.dpi.1).max(1) };
                    FillRect(h, &rc, HBRUSH(brush.0));
                }
                EndPage(h);
                for it in finished_items(&job.sheets, i) {
                    let err = errors.iter().find(|e| e.0 == it).map(|e| e.1.clone());
                    send(JobEvent::ItemDone { item: it, ok: err.is_none(), message: err.unwrap_or_else(|| t("Gedruckt", "Printed").into()) });
                }
            }
            send(JobEvent::Sheet { done: i + 1, total });
        }
        finish(&mut hdc);
        Ok(total)
    }
}

pub use sys::{caps, list_printers, page_info, properties_dialog};
use sys::run_print;

#[cfg(test)]
#[path = "printer_tests.rs"]
mod tests;
