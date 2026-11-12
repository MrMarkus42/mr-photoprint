//! Loading photos (with the camera's orientation tag), thumbnails, and turning a placement into
//! the pixels a printer or an exported sheet needs.

use crate::i18n::{t, ts};
use crate::layout::{Placement, RectMm, Sheet, cut_mark_lines};
use crate::model::{ColorMode, Item, JobSettings};
use image::{DynamicImage, ImageDecoder, ImageReader, Rgb, RgbImage, imageops::FilterType};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, mpsc};

// ---- Protection against hostile files -------------------------------------------------------
// Photos come from anywhere (downloads, messengers, memory cards), so every file is treated as
// untrusted: the content (not the name) decides the format, sizes are checked before anything is
// decoded, memory is capped, and a decoder that gives up on a broken file can't take the app down.

/// Largest file we open.
pub const MAX_FILE_BYTES: u64 = 400 * 1024 * 1024;
/// Largest picture (width × height) – stops "decompression bombs" (tiny file, gigantic picture).
pub const MAX_PIXELS: u64 = 250_000_000;
/// Longest side in pixels.
pub const MAX_SIDE: u32 = 40_000;
/// Memory one decoder may ask for.
pub const MAX_ALLOC: u64 = 2 * 1024 * 1024 * 1024;

fn limits() -> image::Limits {
    let mut l = image::Limits::default();
    l.max_image_width = Some(MAX_SIDE);
    l.max_image_height = Some(MAX_SIDE);
    l.max_alloc = Some(MAX_ALLOC);
    l
}

/// Run a decoder so that a crash inside it becomes an ordinary error.
fn guarded<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .unwrap_or_else(|_| Err(t("Die Datei ist beschädigt oder manipuliert und wurde nicht geöffnet.", "The file is damaged or tampered with and was not opened.").into()))
}

fn check_file(path: &Path) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| format!("{} {e}", t("Datei nicht lesbar:", "File cannot be read:")))?;
    if !meta.is_file() {
        return Err(t("Keine normale Datei (Verknüpfung oder Gerät) – übersprungen.", "Not an ordinary file (link or device) – skipped.").into());
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!("{} ({} MB).", t("Datei zu groß", "File too large"), meta.len() / 1_048_576));
    }
    if meta.len() < 16 {
        return Err(t("Datei ist leer oder abgeschnitten.", "File is empty or cut off.").into());
    }
    Ok(())
}

/// Formats the built-in (memory-safe Rust) decoders read. The file's content decides; only TGA,
/// which has no signature, is taken by its extension.
fn open_checked(path: &Path) -> Result<ImageReader<std::io::BufReader<std::fs::File>>, String> {
    check_file(path)?;
    let mut reader = ImageReader::open(path).map_err(|e| e.to_string())?.with_guessed_format().map_err(|e| e.to_string())?;
    use image::ImageFormat::*;
    match reader.format() {
        Some(Jpeg | Png | Tiff | Bmp | WebP | Gif | Ico | Tga | Pnm | Qoi) => {}
        _ => return Err(t("Inhalt ist kein unterstütztes Bild (Endung und Inhalt passen nicht zusammen).", "The content is not a supported picture (name and content do not match).").into()),
    }
    reader.limits(limits());
    Ok(reader)
}

fn check_size(w: u32, h: u32) -> Result<(), String> {
    if w == 0 || h == 0 {
        return Err(t("Bild hat keine Größe.", "The picture has no size.").into());
    }
    if w > MAX_SIDE || h > MAX_SIDE || w as u64 * h as u64 > MAX_PIXELS {
        return Err(ts(format!("Bild ist unplausibel groß ({w} × {h} Pixel) – aus Sicherheitsgründen nicht geöffnet."), format!("Picture is implausibly large ({w} × {h} pixels) – not opened for safety.")));
    }
    Ok(())
}

/// Which decoder a file goes to, decided from its first bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Codec {
    /// Built-in Rust decoders (JPEG, PNG, TIFF, BMP, WebP, GIF, ICO, TGA, PNM, QOI).
    Builtin,
    /// HEIC/HEIF (iPhone, many Android phones) via Windows.
    Heif,
    /// AVIF via Windows (needs the AV1 extension).
    Avif,
    /// JPEG XL via Windows (needs the JPEG XL extension).
    Jxl,
    /// Camera RAW (DNG, CR2, CR3, NEF, ARW, ORF, RW2, RAF, PEF, SRW…) via Windows' RAW extension.
    Raw,
}

pub const RAW_EXTENSIONS: &[&str] = &["dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "orf", "rw2", "raf", "pef", "srw", "erf", "kdc", "mrw", "3fr", "x3f"];

pub fn codec_of(path: &Path) -> Codec {
    use std::io::Read;
    let mut b = [0u8; 16];
    let n = std::fs::File::open(path).and_then(|mut f| f.read(&mut b)).unwrap_or(0);
    let b = &b[..n];
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if b.len() >= 12 && &b[4..8] == b"ftyp" {
        match &b[8..12] {
            b"heic" | b"heix" | b"heim" | b"heis" | b"hevc" | b"hevx" | b"mif1" | b"msf1" => return Codec::Heif,
            b"avif" | b"avis" => return Codec::Avif,
            b"crx " => return Codec::Raw,
            _ => {}
        }
    }
    if b.starts_with(&[0xFF, 0x0A]) || b.starts_with(&[0, 0, 0, 0x0C, b'J', b'X', b'L', b' ']) {
        return Codec::Jxl;
    }
    let raw_sig = b.starts_with(b"II*\0") || b.starts_with(b"MM\0*") || b.starts_with(b"IIRO") || b.starts_with(b"IIRS") || b.starts_with(b"IIU\0") || b.starts_with(b"FUJIFILMCCD-RAW");
    if raw_sig && RAW_EXTENSIONS.contains(&ext.as_str()) {
        return Codec::Raw;
    }
    Codec::Builtin
}

pub fn is_heif(path: &Path) -> bool {
    codec_of(path) == Codec::Heif
}

fn missing_codec(c: Codec) -> &'static str {
    match c {
        Codec::Heif => t("HEIC-Fotos brauchen unter Windows die kostenlosen „HEIF-Bilderweiterungen“ und „HEVC-Videoerweiterungen“ aus dem Microsoft Store.", "HEIC photos need the free 'HEIF Image Extensions' and 'HEVC Video Extensions' from the Microsoft Store."),
        Codec::Avif => t("AVIF-Bilder brauchen unter Windows die kostenlose „AV1 Video Extension“ aus dem Microsoft Store.", "AVIF pictures need the free 'AV1 Video Extension' from the Microsoft Store."),
        Codec::Jxl => t("JPEG-XL-Bilder brauchen unter Windows die „JPEG XL Image Extension“ aus dem Microsoft Store.", "JPEG XL pictures need the 'JPEG XL Image Extension' from the Microsoft Store."),
        Codec::Raw => t("Kamera-RAW-Dateien brauchen unter Windows die kostenlose „Raw Image Extension“ aus dem Microsoft Store.", "Camera RAW files need the free 'Raw Image Extension' from the Microsoft Store."),
        Codec::Builtin => t("Format wird nicht unterstützt.", "This format is not supported."),
    }
}

/// Decode with Windows' own image codecs (WIC), which Windows Update keeps patched.
#[cfg(windows)]
mod wic {
    use super::*;
    use windows::Win32::Foundation::GENERIC_READ;
    use windows::Win32::Graphics::Imaging::*;
    use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
    use windows::Win32::System::Com::*;
    use windows::Win32::System::Variant::VT_UI2;
    use windows::core::HSTRING;

    fn frame(path: &Path, c: Codec) -> Result<(IWICImagingFactory, IWICBitmapFrameDecode), String> {
        unsafe {
            // Each worker thread joins COM once; repeated calls are harmless.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let factory: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).map_err(|e| e.message())?;
            let dec = factory
                .CreateDecoderFromFilename(&HSTRING::from(path), None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)
                .map_err(|_| missing_codec(c).to_string())?;
            let fr = dec.GetFrame(0).map_err(|_| missing_codec(c).to_string())?;
            Ok((factory, fr))
        }
    }

    /// The camera's orientation tag (1–8), as Windows reports it for any format.
    fn orientation(fr: &IWICBitmapFrameDecode) -> u16 {
        unsafe {
            let Ok(q) = fr.GetMetadataQueryReader() else { return 1 };
            let mut v = PROPVARIANT::default();
            let mut o = 1;
            if q.GetMetadataByName(&HSTRING::from("System.Photo.Orientation"), &mut v).is_ok() {
                if v.Anonymous.Anonymous.vt == VT_UI2 {
                    o = v.Anonymous.Anonymous.Anonymous.uiVal;
                }
            }
            let _ = PropVariantClear(&mut v);
            if (1..=8).contains(&o) { o } else { 1 }
        }
    }

    pub fn info(path: &Path, c: Codec) -> Result<((u32, u32), Option<f32>), String> {
        let (_, fr) = frame(path, c)?;
        unsafe {
            let (mut w, mut h) = (0u32, 0u32);
            fr.GetSize(&mut w, &mut h).map_err(|e| e.message())?;
            check_size(w, h)?;
            let (mut dx, mut dy) = (0f64, 0f64);
            let dpi = fr.GetResolution(&mut dx, &mut dy).ok().map(|_| dx as f32).filter(|d| *d >= 10.0 && *d <= 10_000.0);
            let px = if (5..=8).contains(&orientation(&fr)) { (h, w) } else { (w, h) };
            Ok((px, dpi))
        }
    }

    pub fn load(path: &Path, c: Codec) -> Result<DynamicImage, String> {
        let (factory, fr) = frame(path, c)?;
        unsafe {
            let (mut w, mut h) = (0u32, 0u32);
            fr.GetSize(&mut w, &mut h).map_err(|e| e.message())?;
            check_size(w, h)?; // checked before any pixel memory is taken
            let conv = factory.CreateFormatConverter().map_err(|e| e.message())?;
            conv.Initialize(&fr, &GUID_WICPixelFormat32bppRGBA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)
                .map_err(|_| missing_codec(c).to_string())?;
            let stride = w * 4;
            let mut buf = vec![0u8; stride as usize * h as usize];
            conv.CopyPixels(std::ptr::null(), stride, &mut buf).map_err(|e| format!("{} {}", t("Bild konnte nicht gelesen werden:", "Picture could not be read:"), e.message()))?;
            let mut img = image::RgbaImage::from_raw(w, h, buf).map(DynamicImage::ImageRgba8).ok_or(t("Bild konnte nicht gelesen werden.", "Picture could not be read."))?;
            if let Some(o) = image::metadata::Orientation::from_exif(orientation(&fr) as u8) {
                img.apply_orientation(o);
            }
            Ok(img)
        }
    }
}

#[cfg(not(windows))]
mod wic {
    use super::*;
    pub fn info(_: &Path, c: Codec) -> Result<((u32, u32), Option<f32>), String> {
        Err(missing_codec(c).into())
    }
    pub fn load(_: &Path, c: Codec) -> Result<DynamicImage, String> {
        Err(missing_codec(c).into())
    }
}

pub fn load_oriented(path: &Path) -> Result<DynamicImage, String> {
    guarded(|| {
        check_file(path)?;
        let c = codec_of(path);
        if c != Codec::Builtin {
            return wic::load(path, c);
        }
        let reader = open_checked(path)?;
        let mut dec = reader.into_decoder().map_err(|e| e.to_string())?;
        let (w, h) = dec.dimensions();
        check_size(w, h)?;
        let orient = dec.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
        let mut img = DynamicImage::from_decoder(dec).map_err(|e| e.to_string())?;
        img.apply_orientation(orient);
        Ok(img)
    })
}

/// Pixel size (after orientation) without decoding the whole photo.
pub fn read_info(path: &Path) -> Result<((u32, u32), Option<f32>), String> {
    guarded(|| read_info_inner(path))
}

fn read_info_inner(path: &Path) -> Result<((u32, u32), Option<f32>), String> {
    check_file(path)?;
    let c = codec_of(path);
    if c != Codec::Builtin {
        return wic::info(path, c);
    }
    let reader = open_checked(path)?;
    let mut dec = reader.into_decoder().map_err(|e| e.to_string())?;
    let (w, h) = dec.dimensions();
    check_size(w, h)?;
    let o = dec.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    use image::metadata::Orientation::*;
    let px = match o {
        Rotate90 | Rotate270 | Rotate90FlipH | Rotate270FlipH => (h, w),
        _ => (w, h),
    };
    Ok((px, jfif_dpi(path)))
}

/// Pixels per inch from a JPEG's JFIF header, when it states one.
fn jfif_dpi(path: &Path) -> Option<f32> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).ok()?;
    let mut b = [0u8; 20];
    f.read_exact(&mut b).ok()?;
    if b[0] != 0xFF || b[1] != 0xD8 || b[2] != 0xFF || b[3] != 0xE0 || &b[6..11] != b"JFIF\0" {
        return None;
    }
    let unit = b[13];
    let x = u16::from_be_bytes([b[14], b[15]]) as f32;
    let dpi = match unit {
        1 => x,
        2 => x * 2.54,
        _ => return None,
    };
    // Same plausible range as for the Windows codecs.
    (10.0..=10_000.0).contains(&dpi).then_some(dpi)
}

/// Results of the background loader.
pub enum Loaded {
    Info { path: PathBuf, px: (u32, u32), dpi: Option<f32> },
    Thumb { path: PathBuf, image: egui::ColorImage },
    Failed { path: PathBuf, message: String },
}

#[derive(Default)]
struct Queues {
    info: VecDeque<PathBuf>,
    thumbs: VecDeque<PathBuf>,
}

/// A few worker threads that read photo sizes first and then make thumbnails.
pub struct Loader {
    queues: Arc<(Mutex<Queues>, Condvar)>,
    pub rx: mpsc::Receiver<Loaded>,
}

impl Loader {
    pub fn new(ctx: egui::Context) -> Self {
        let queues: Arc<(Mutex<Queues>, Condvar)> = Arc::default();
        let (tx, rx) = mpsc::channel();
        // Leave one core free so the window stays smooth while photos load.
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).saturating_sub(1).clamp(1, 3);
        for _ in 0..n {
            let q = queues.clone();
            let tx = tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                loop {
                    let job = {
                        let (lock, cv) = &*q;
                        let mut g = lock.lock().unwrap();
                        loop {
                            if let Some(p) = g.info.pop_front() {
                                break (true, p);
                            }
                            if let Some(p) = g.thumbs.pop_front() {
                                break (false, p);
                            }
                            g = cv.wait(g).unwrap();
                        }
                    };
                    let msg = match job {
                        (true, path) => match read_info(&path) {
                            Ok((px, dpi)) => Loaded::Info { path, px, dpi },
                            Err(message) => Loaded::Failed { path, message },
                        },
                        (false, path) => match load_oriented(&path) {
                            Ok(img) => {
                                let t = img.thumbnail(220, 220).to_rgba8();
                                let image = egui::ColorImage::from_rgba_unmultiplied(
                                    [t.width() as usize, t.height() as usize],
                                    t.as_raw(),
                                );
                                Loaded::Thumb { path, image }
                            }
                            Err(message) => Loaded::Failed { path, message },
                        },
                    };
                    if tx.send(msg).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            });
        }
        Self { queues, rx }
    }

    pub fn request(&self, path: PathBuf) {
        let (lock, cv) = &*self.queues;
        let mut g = lock.lock().unwrap();
        g.info.push_back(path.clone());
        g.thumbs.push_back(path);
        cv.notify_all();
    }
}

/// Pixels ready to be put on paper at `dest` (device pixels: x, y, w, h).
pub struct Prepared {
    pub img: RgbImage,
    pub dest: (i32, i32, i32, i32),
}

pub fn turn(img: &DynamicImage, rot: i32) -> DynamicImage {
    match rot {
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => img.clone(),
    }
}

/// Cut the visible part of the photo and size it for a device with `dpi`, whose pixel (0,0)
/// sits at `origin_mm` on the page. `exact` resizes up too (exports); printers stretch
/// themselves when the photo has fewer pixels than the paper.
pub fn prepare(
    img: &DynamicImage,
    pl: &Placement,
    dpi: (f32, f32),
    origin_mm: (f32, f32),
    gray: bool,
    exact: bool,
) -> Option<Prepared> {
    let vis = pl.rect.intersect(&pl.clip)?;
    let turned = turn(img, pl.rot);
    let (iw, ih) = (turned.width() as f32, turned.height() as f32);
    let sx = ((vis.x - pl.rect.x) / pl.rect.w * iw).clamp(0.0, iw - 1.0);
    let sy = ((vis.y - pl.rect.y) / pl.rect.h * ih).clamp(0.0, ih - 1.0);
    let sw = (vis.w / pl.rect.w * iw).round().clamp(1.0, iw - sx);
    let sh = (vis.h / pl.rect.h * ih).round().clamp(1.0, ih - sy);
    let crop = turned.crop_imm(sx as u32, sy as u32, sw as u32, sh as u32);
    let to_px = |mm: f32, d: f32| (mm / 25.4 * d).round() as i32;
    let dx = to_px(vis.x - origin_mm.0, dpi.0);
    let dy = to_px(vis.y - origin_mm.1, dpi.1);
    let dw = to_px(vis.w, dpi.0).max(1);
    let dh = to_px(vis.h, dpi.1).max(1);
    let sized = if exact || (dw as u32) < crop.width() {
        crop.resize_exact(dw as u32, dh as u32, FilterType::CatmullRom)
    } else {
        crop
    };
    let img = if gray { DynamicImage::ImageLuma8(sized.to_luma8()).to_rgb8() } else { sized.to_rgb8() };
    Some(Prepared { img, dest: (dx, dy, dw, dh) })
}

pub fn wants_gray(item: &Item, job: &JobSettings) -> bool {
    item.settings(job).color == ColorMode::Gray
}

fn fill(canvas: &mut RgbImage, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb<u8>) {
    let (w, h) = (canvas.width() as i32, canvas.height() as i32);
    for y in y0.max(0)..y1.min(h) {
        for x in x0.max(0)..x1.min(w) {
            canvas.put_pixel(x as u32, y as u32, c);
        }
    }
}

/// Lines (frames and cut marks) in page millimetres, for any renderer.
pub fn decoration_lines(sheet: &Sheet, job: &JobSettings) -> Vec<RectMm> {
    let mut out = Vec::new();
    let t = 0.25; // line width in mm
    for cell in &sheet.cells {
        let Some(vis) = cell.placement.rect.intersect(&cell.placement.clip) else { continue };
        if job.collage() && job.frame {
            out.push(RectMm::new(vis.x, vis.y, vis.w, t));
            out.push(RectMm::new(vis.x, vis.bottom() - t, vis.w, t));
            out.push(RectMm::new(vis.x, vis.y, t, vis.h));
            out.push(RectMm::new(vis.right() - t, vis.y, t, vis.h));
        }
        if job.cut_marks {
            for [a, b] in cut_mark_lines(&vis, job.cut_marks_offset) {
                let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
                let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
                out.push(RectMm::new(x0 - t / 2.0, y0 - t / 2.0, (x1 - x0) + t, (y1 - y0) + t));
            }
        }
    }
    out
}

/// Draw a whole sheet into a picture (for "In JPEG-Dateien speichern").
pub fn render_sheet(
    sheet: &Sheet,
    items: &[Item],
    job: &JobSettings,
    dpi: f32,
    cache: &mut Option<(PathBuf, DynamicImage)>,
) -> Result<RgbImage, String> {
    let w = (sheet.page.width_mm / 25.4 * dpi).round();
    let h = (sheet.page.height_mm / 25.4 * dpi).round();
    // A sheet picture gets the same limits as a photo, so no setting can ask for gigabytes.
    if !(w.is_finite() && h.is_finite()) || w > MAX_SIDE as f32 || h > MAX_SIDE as f32 || w as u64 * h as u64 > MAX_PIXELS {
        return Err(ts(format!("Das Blatt wäre bei {dpi} dpi zu groß für eine Bilddatei – bitte eine kleinere Auflösung wählen."), format!("At {dpi} dpi the sheet would be too large for a picture file – please choose a lower resolution.")));
    }
    let mut canvas = RgbImage::from_pixel((w as u32).max(1), (h as u32).max(1), Rgb([255, 255, 255]));
    for cell in &sheet.cells {
        let item = items.get(cell.item).ok_or(t("Interner Fehler: Foto fehlt im Druckplan.", "Internal error: photo missing from the sheet plan."))?;
        if cache.as_ref().map(|c| &c.0) != Some(&item.path) {
            *cache = Some((item.path.clone(), load_oriented(&item.path)?));
        }
        let img = &cache.as_ref().unwrap().1;
        let gray = wants_gray(item, job);
        if let Some(p) = prepare(img, &cell.placement, (dpi, dpi), (0.0, 0.0), gray, true) {
            image::imageops::overlay(&mut canvas, &p.img, p.dest.0 as i64, p.dest.1 as i64);
        }
    }
    let px = |mm: f32| (mm / 25.4 * dpi).round() as i32;
    for r in decoration_lines(sheet, job) {
        let (x0, y0) = (px(r.x), px(r.y));
        fill(&mut canvas, x0, y0, x0 + px(r.w).max(1), y0 + px(r.h).max(1), Rgb([0, 0, 0]));
    }
    Ok(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join("mrphotoprint-tests");
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    #[test]
    fn fake_photo_is_refused() {
        let p = tmp("fake.jpg");
        std::fs::write(&p, b"MZ this is not a photo at all, just some program bytes......").unwrap();
        assert!(load_oriented(&p).is_err());
    }

    #[test]
    fn decompression_bomb_is_refused() {
        // A valid PNG header that claims 60000 x 60000 pixels.
        let p = tmp("bomb.png");
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];
        b.extend_from_slice(&60000u32.to_be_bytes());
        b.extend_from_slice(&60000u32.to_be_bytes());
        b.extend_from_slice(&[8, 2, 0, 0, 0, 0, 0, 0, 0]);
        b.extend_from_slice(&[0u8; 64]);
        std::fs::write(&p, b).unwrap();
        let e = load_oriented(&p).unwrap_err();
        assert!(!e.is_empty());
        assert!(read_info(&p).is_err());
    }

    #[test]
    fn truncated_jpeg_does_not_crash() {
        let p = tmp("cut.jpg");
        let img = RgbImage::from_pixel(300, 200, Rgb([200, 30, 30]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Jpeg).unwrap();
        bytes.truncate(bytes.len() / 3);
        std::fs::write(&p, bytes).unwrap();
        let _ = load_oriented(&p); // error or partial picture – never a crash
    }

    #[test]
    fn sheet_renders_filled_photo() {
        let p = tmp("red.png");
        RgbImage::from_pixel(300, 200, Rgb([220, 20, 20])).save(&p).unwrap();
        let mut item = Item::new(p);
        item.px = (300, 200);
        let mut job = JobSettings::default();
        job.photo.scale = ScaleMode::Fill;
        let items = vec![item];
        let sheets = crate::layout::plan_sheets(&items, &[0], &job, &mut |_, l| {
            let (w, h) = crate::printer::orient(102.0, 152.0, l);
            crate::layout::PageInfo::plain(w, h, 100.0)
        });
        assert_eq!(sheets.len(), 1);
        assert!(sheets[0].page.width_mm > sheets[0].page.height_mm, "paper turned to the photo");
        let img = render_sheet(&sheets[0], &items, &job, 100.0, &mut None).unwrap();
        let px = img.get_pixel(img.width() / 2, img.height() / 2);
        assert!(px[0] > 180 && px[1] < 80, "photo fills the sheet");
        let corner = img.get_pixel(2, 2);
        assert!(corner[0] > 180 && corner[1] < 80, "no white border when filled");
    }

    /// Write a real HEIC with Windows' own encoder (when the HEIF extension is installed) and
    /// check that Mr. PhotoPrint recognises and opens it.
    #[cfg(windows)]
    #[test]
    fn heic_opens_through_windows_codecs() {
        use windows::Win32::Foundation::GENERIC_WRITE;
        use windows::Win32::Graphics::Imaging::*;
        use windows::Win32::System::Com::*;
        use windows::core::HSTRING;
        let p = tmp("test.heic");
        let _ = std::fs::remove_file(&p);
        let (w, h) = (320u32, 200u32);
        let written = unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            (|| -> windows::core::Result<()> {
                let f: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
                let stream = f.CreateStream()?;
                stream.InitializeFromFilename(&HSTRING::from(p.as_path()), GENERIC_WRITE.0)?;
                let enc = f.CreateEncoder(&GUID_ContainerFormatHeif, std::ptr::null())?;
                enc.Initialize(&stream, WICBitmapEncoderNoCache)?;
                let mut frame = None;
                let mut bag = None;
                enc.CreateNewFrame(&mut frame, &mut bag)?;
                let frame = frame.unwrap();
                frame.Initialize(bag.as_ref())?;
                frame.SetSize(w, h)?;
                let mut fmt = GUID_WICPixelFormat24bppBGR;
                frame.SetPixelFormat(&mut fmt)?;
                // The encoder may ask for another layout (e.g. 32-bit BGRA).
                let bpp: u32 = if fmt == GUID_WICPixelFormat24bppBGR { 3 } else { 4 };
                let stride = w * bpp;
                let mut px = vec![255u8; (stride * h) as usize];
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let i = y * stride as usize + x * bpp as usize;
                        px[i] = 30; // B
                        px[i + 1] = 60; // G
                        px[i + 2] = 220; // R
                    }
                }
                frame.WritePixels(h, stride, &px)?;
                frame.Commit()?;
                enc.Commit()?;
                Ok(())
            })()
        };
        if written.is_err() {
            eprintln!("No HEIF encoder on this computer – skipped: {written:?}");
            return;
        }
        assert!(is_heif(&p), "recognised by content");
        let ((iw, ih), _) = read_info(&p).expect("size readable");
        assert_eq!((iw, ih), (w, h));
        let img = load_oriented(&p).expect("HEIC opens").to_rgb8();
        let c = img.get_pixel(w / 2, h / 2);
        assert!(c[0] > 150 && c[2] < 100, "colours survive: {c:?}");
    }

    #[test]
    fn common_formats_open() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 30, Rgb([20, 200, 40])));
        for (name, fmt) in [
            ("f.png", image::ImageFormat::Png),
            ("f.jpg", image::ImageFormat::Jpeg),
            ("f.bmp", image::ImageFormat::Bmp),
            ("f.tif", image::ImageFormat::Tiff),
            ("f.gif", image::ImageFormat::Gif),
            ("f.webp", image::ImageFormat::WebP),
            ("f.ico", image::ImageFormat::Ico),
            ("f.tga", image::ImageFormat::Tga),
            ("f.ppm", image::ImageFormat::Pnm),
            ("f.qoi", image::ImageFormat::Qoi),
        ] {
            let p = tmp(name);
            let rgba = matches!(fmt, image::ImageFormat::Gif | image::ImageFormat::Ico | image::ImageFormat::WebP);
            let src = if rgba { DynamicImage::ImageRgba8(img.to_rgba8()) } else { img.clone() };
            src.save_with_format(&p, fmt).unwrap();
            assert!(crate::model::is_photo(&p), "{name} listed");
            assert_eq!(codec_of(&p), Codec::Builtin, "{name}");
            let ((w, h), _) = read_info(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!((w, h), (40, 30), "{name}");
            let px = load_oriented(&p).unwrap().to_rgb8().get_pixel(20, 15).0;
            assert!(px[1] > 150 && px[0] < 90, "{name}: {px:?}");
        }
    }

    #[test]
    fn content_decides_codec() {
        let p = tmp("disguised.dng"); // a PNG that pretends to be a camera RAW
        RgbImage::from_pixel(8, 8, Rgb([1, 2, 3])).save_with_format(&p, image::ImageFormat::Png).unwrap();
        assert_eq!(codec_of(&p), Codec::Builtin);
        let q = tmp("fake.heic"); // garbage with a HEIC name
        std::fs::write(&q, b"this is not an image at all, honestly").unwrap();
        assert_eq!(codec_of(&q), Codec::Builtin);
        assert!(load_oriented(&q).is_err());
    }

    #[test]
    fn quality_maps_to_export_resolution() {
        let mut j = JobSettings::default();
        assert_eq!(j.quality, QUALITY_AS_PRINTER);
        assert_eq!(export_dpi_for(&j), 300);
        j.quality = QUALITY_HIGH;
        assert_eq!(export_dpi_for(&j), 600);
        j.quality = 720;
        assert_eq!(export_dpi_for(&j), 720);
        j.quality = 12345; // a hand-edited settings file
        assert_eq!(j.sanitize().quality, QUALITY_AS_PRINTER);
    }

    fn jfif(unit: u8, density: u16) -> PathBuf {
        let p = tmp(&format!("jfif-{unit}-{density}.jpg"));
        let mut b = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 16, b'J', b'F', b'I', b'F', 0, 1, 1, unit];
        b.extend_from_slice(&density.to_be_bytes());
        b.extend_from_slice(&density.to_be_bytes());
        b.extend_from_slice(&[0, 0, 0xFF, 0xD9]);
        std::fs::write(&p, b).unwrap();
        p
    }

    #[test]
    fn jfif_resolution_is_read_and_checked() {
        assert_eq!(jfif_dpi(&jfif(1, 300)), Some(300.0));
        let cm = jfif_dpi(&jfif(2, 118)).unwrap();
        assert!((cm - 299.7).abs() < 0.1, "dots per cm: {cm}");
        assert_eq!(jfif_dpi(&jfif(0, 300)), None, "aspect ratio only");
        assert_eq!(jfif_dpi(&jfif(1, 1)), None, "implausibly low");
        assert_eq!(jfif_dpi(&jfif(2, 60000)), None, "implausibly high");
        let png = tmp("nojfif.png");
        RgbImage::new(4, 4).save(&png).unwrap();
        assert_eq!(jfif_dpi(&png), None);
    }

    #[test]
    fn size_limits() {
        assert!(check_size(0, 10).is_err());
        assert!(check_size(10, 0).is_err());
        assert!(check_size(MAX_SIDE, 1).is_ok());
        assert!(check_size(MAX_SIDE + 1, 1).is_err());
        assert!(check_size(20_000, 20_000).is_err(), "400 megapixels");
        assert!(check_size(15_000, 15_000).is_ok());
    }

    #[test]
    fn empty_folder_and_tiny_files_are_refused() {
        let empty = tmp("empty.jpg");
        std::fs::write(&empty, b"").unwrap();
        assert!(load_oriented(&empty).is_err());
        assert!(read_info(&empty).is_err());
        let dir = tmp("a-folder.jpg");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(load_oriented(&dir).is_err());
        assert!(read_info(&tmp("missing.jpg")).is_err());
    }

    #[test]
    fn other_formats_behind_a_photo_name_are_refused() {
        for (name, bytes) in [
            ("pdf.jpg", &b"%PDF-1.7\n1 0 obj << /Type /Catalog >> endobj\n"[..]),
            ("svg.png", &b"<svg xmlns='http://www.w3.org/2000/svg'><script>alert(1)</script></svg>"[..]),
            ("zip.webp", &b"PK\x03\x04\x14\x00\x00\x00\x08\x00 some zipped thing"[..]),
        ] {
            let p = tmp(name);
            std::fs::write(&p, bytes).unwrap();
            assert!(load_oriented(&p).is_err(), "{name}");
            assert!(read_info(&p).is_err(), "{name}");
        }
    }

    #[test]
    fn camera_orientation_tag_swaps_the_size() {
        // JPEG with an EXIF APP1 segment saying "turn 90°" (orientation 6).
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 20, Rgb([10, 10, 200])));
        let mut jpg = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut jpg), image::ImageFormat::Jpeg).unwrap();
        let exif: Vec<u8> = [
            &b"Exif\0\0"[..],
            &b"MM\0\x2a\0\0\0\x08"[..],
            &[0, 1, 0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0, 0, 0, 0, 0][..],
        ]
        .concat();
        let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
        out.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(&exif);
        out.extend_from_slice(&jpg[2..]);
        let p = tmp("turned.jpg");
        std::fs::write(&p, out).unwrap();
        assert_eq!(read_info(&p).unwrap().0, (20, 40));
        let loaded = load_oriented(&p).unwrap();
        assert_eq!((loaded.width(), loaded.height()), (20, 40));
    }

    fn sheet_with(page: crate::layout::PageInfo, item: usize, rect: RectMm) -> Sheet {
        let placement = Placement { rot: 0, rect, clip: rect };
        Sheet { paper: None, tray: None, media: None, landscape: None, settings_of: 0, page, cells: vec![crate::layout::Cell { item, placement }] }
    }

    #[test]
    fn giant_sheet_picture_is_refused() {
        let page = crate::layout::PageInfo::plain(2000.0, 2000.0, 300.0);
        let s = sheet_with(page, 0, RectMm::new(0.0, 0.0, 10.0, 10.0));
        let items = vec![Item::new(tmp("red.png"))];
        let e = render_sheet(&s, &items, &JobSettings::default(), 1200.0, &mut None).unwrap_err();
        assert!(e.contains("zu groß"), "{e}");
        let bad = crate::layout::PageInfo::plain(f32::NAN, 100.0, 300.0);
        assert!(render_sheet(&sheet_with(bad, 0, RectMm::new(0.0, 0.0, 1.0, 1.0)), &items, &JobSettings::default(), 100.0, &mut None).is_err());
    }

    #[test]
    fn sheet_pointing_at_a_missing_photo_is_an_error_not_a_crash() {
        let page = crate::layout::PageInfo::plain(100.0, 100.0, 100.0);
        let s = sheet_with(page, 5, RectMm::new(0.0, 0.0, 10.0, 10.0));
        assert!(render_sheet(&s, &[], &JobSettings::default(), 100.0, &mut None).is_err());
    }

    #[test]
    fn prepare_cuts_what_lies_outside_the_clip() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(200, 100, Rgb([255, 0, 0])));
        // Photo 200 × 100 mm, only its middle 100 × 100 mm may be printed.
        let pl = Placement { rot: 0, rect: RectMm::new(-50.0, 0.0, 200.0, 100.0), clip: RectMm::new(0.0, 0.0, 100.0, 100.0) };
        let p = prepare(&img, &pl, (25.4, 25.4), (0.0, 0.0), false, true).unwrap();
        assert_eq!(p.dest, (0, 0, 100, 100));
        assert_eq!((p.img.width(), p.img.height()), (100, 100));
        // Printable area starts 5 mm in: device pixel 0 lies there.
        let p = prepare(&img, &pl, (25.4, 25.4), (5.0, 5.0), true, true).unwrap();
        assert_eq!((p.dest.0, p.dest.1), (-5, -5));
        let g = p.img.get_pixel(50, 50).0;
        assert!(g[0] == g[1] && g[1] == g[2], "grey: {g:?}");
        // Entirely outside: nothing to print.
        let off = Placement { rot: 0, rect: RectMm::new(200.0, 0.0, 10.0, 10.0), clip: RectMm::new(0.0, 0.0, 100.0, 100.0) };
        assert!(prepare(&img, &off, (100.0, 100.0), (0.0, 0.0), false, true).is_none());
        // Turned photo: 200 × 100 pixels become 100 × 200.
        let tall = Placement { rot: 90, rect: RectMm::new(0.0, 0.0, 100.0, 200.0), clip: RectMm::new(0.0, 0.0, 100.0, 200.0) };
        let p = prepare(&img, &tall, (25.4, 25.4), (0.0, 0.0), false, false).unwrap();
        assert_eq!((p.img.width(), p.img.height()), (100, 200));
    }

    #[test]
    fn frames_only_in_collages_and_eight_cut_marks_per_photo() {
        let page = crate::layout::PageInfo::plain(100.0, 100.0, 100.0);
        let s = sheet_with(page, 0, RectMm::new(10.0, 10.0, 50.0, 50.0));
        let mut job = JobSettings::default();
        assert!(decoration_lines(&s, &job).is_empty(), "single photo: no frame");
        job.cols = 2;
        assert_eq!(decoration_lines(&s, &job).len(), 4);
        job.cut_marks = true;
        assert_eq!(decoration_lines(&s, &job).len(), 12);
        job.frame = false;
        assert_eq!(decoration_lines(&s, &job).len(), 8);
    }

    #[test]
    fn loader_delivers_size_and_thumbnail() {
        let p = tmp("loader.png");
        RgbImage::from_pixel(640, 480, Rgb([0, 120, 0])).save(&p).unwrap();
        let bad = tmp("loader-bad.jpg");
        std::fs::write(&bad, b"definitely not a jpeg, only text in here").unwrap();
        let l = Loader::new(egui::Context::default());
        l.request(p.clone());
        l.request(bad.clone());
        let (mut info, mut thumb, mut failed) = (false, false, false);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !(info && thumb && failed) && std::time::Instant::now() < deadline {
            match l.rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(Loaded::Info { path, px, .. }) if path == p => {
                    assert_eq!(px, (640, 480));
                    info = true;
                }
                Ok(Loaded::Thumb { path, image }) if path == p => {
                    assert!(image.size[0] <= 220 && image.size[1] <= 220);
                    thumb = true;
                }
                Ok(Loaded::Failed { path, .. }) if path == bad => failed = true,
                _ => {}
            }
        }
        assert!(info && thumb && failed, "info {info}, thumb {thumb}, failed {failed}");
    }
}
