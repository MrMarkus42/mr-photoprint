//! Tests for the print job around the printer: job checks, the JPEG export, crash safety and
//! the DEVMODE checks (printer.rs).

use super::*;
use crate::layout::{self, PageInfo};
use crate::model::{QUALITY_DRAFT, ScaleMode};
use image::{Rgb, RgbImage};
use std::sync::Mutex;

fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join("mrphotoprint-tests").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn photo(dir: &std::path::Path, name: &str, w: u32, h: u32) -> Item {
    let p = dir.join(name);
    RgbImage::from_pixel(w, h, Rgb([210, 30, 30])).save(&p).unwrap();
    let mut it = Item::new(p);
    it.px = (w, h);
    it
}

fn plan(items: &[Item], job: &JobSettings) -> Vec<Sheet> {
    let order: Vec<usize> = (0..items.len()).collect();
    layout::plan_sheets(items, &order, job, &mut |_, l| {
        let (w, h) = orient(105.0, 148.0, l);
        PageInfo::plain(w, h, 150.0)
    })
}

fn control() -> JobControl {
    JobControl { pause: AtomicBool::new(false), stop: AtomicBool::new(false) }
}

#[test]
fn orient_turns_the_paper() {
    assert_eq!(orient(210.0, 297.0, Some(true)), (297.0, 210.0));
    assert_eq!(orient(297.0, 210.0, Some(false)), (210.0, 297.0));
    assert_eq!(orient(297.0, 210.0, None), (297.0, 210.0));
}

#[test]
fn export_papers_are_sane() {
    let papers = export_papers();
    for p in &papers {
        assert!(p.w_mm > 50.0 && p.w_mm < p.h_mm && p.h_mm < 500.0, "{}", p.name);
        assert_eq!(papers.iter().filter(|q| q.id == p.id).count(), 1, "id {} unique", p.id);
    }
    let ten_by_fifteen = papers.iter().find(|p| p.name.contains("10 × 15")).unwrap();
    assert_eq!((ten_by_fifteen.w_mm, ten_by_fifteen.h_mm), (102.0, 152.0));
    assert!(papers.iter().any(|p| p.id == 9 && p.w_mm == 210.0), "A4 has the Windows id DMPAPER_A4");
}

#[test]
fn photo_counts_as_done_after_its_last_sheet() {
    let d = tmp_dir("finished");
    let mut a = photo(&d, "a.png", 60, 40);
    a.own = Some(crate::model::PhotoSettings { copies: 3, ..Default::default() });
    let b = photo(&d, "b.png", 60, 40);
    let items = vec![a, b];
    let sheets = plan(&items, &JobSettings::default());
    assert_eq!(sheets.len(), 4);
    assert!(finished_items(&sheets, 0).is_empty());
    assert!(finished_items(&sheets, 1).is_empty());
    assert_eq!(finished_items(&sheets, 2), vec![0]);
    assert_eq!(finished_items(&sheets, 3), vec![1]);
}

#[test]
fn job_with_wrong_indices_is_refused() {
    let d = tmp_dir("check");
    let items = vec![photo(&d, "a.png", 60, 40)];
    let sheets = plan(&items, &JobSettings::default());
    let job = |sheets: Vec<Sheet>| PrintJob { printer: EXPORT_PRINTER.into(), devmode: None, sheets, items: items.clone(), job: JobSettings::default() };
    assert!(check_job(&job(sheets.clone())).is_ok());
    assert!(check_job(&job(vec![])).is_ok(), "nothing to do is fine");

    let mut far = sheets.clone();
    far[0].cells[0].item = 7;
    assert!(check_job(&job(far)).is_err());
    let mut owner = sheets.clone();
    owner[0].settings_of = 1;
    assert!(check_job(&job(owner)).is_err());
    let mut empty = sheets;
    empty[0].cells.clear();
    assert!(check_job(&job(empty)).is_err());
}

#[test]
fn stop_and_pause() {
    let c = control();
    assert!(wait_if_paused(&c));
    c.pause.store(true, Ordering::Relaxed);
    c.stop.store(true, Ordering::Relaxed);
    assert!(!wait_if_paused(&c), "stop wins over pause");
}

#[test]
fn export_writes_one_jpeg_per_sheet() {
    let d = tmp_dir("export");
    let out = d.join("out");
    let items = vec![photo(&d, "quer.png", 300, 200), photo(&d, "hoch.png", 200, 300)];
    let mut job = JobSettings { export_dir: out.to_string_lossy().into_owned(), quality: QUALITY_DRAFT, ..Default::default() };
    job.photo.scale = ScaleMode::Fill;
    let sheets = plan(&items, &job);
    let pj = PrintJob { printer: EXPORT_PRINTER.into(), devmode: None, sheets, items, job };
    let events = Mutex::new(Vec::new());
    let n = run_export(&pj, &control(), &|e| events.lock().unwrap().push(e)).unwrap();
    assert_eq!(n, 2);

    let a = image::open(out.join("Blatt_0001_quer.jpg")).unwrap();
    // A6 at 150 dpi (draft), paper turned to the landscape photo.
    assert_eq!((a.width(), a.height()), (874, 620));
    let b = image::open(out.join("Blatt_0002_hoch.jpg")).unwrap();
    assert_eq!((b.width(), b.height()), (620, 874));
    let px = a.to_rgb8().get_pixel(10, 10).0;
    assert!(px[0] > 170 && px[1] < 80, "filled to the edge: {px:?}");

    let ev = events.lock().unwrap();
    let done = ev.iter().filter(|e| matches!(e, JobEvent::ItemDone { ok: true, .. })).count();
    assert_eq!(done, 2);
}

#[test]
fn export_reports_broken_photo_and_carries_on() {
    let d = tmp_dir("export-broken");
    let out = d.join("out");
    let good = photo(&d, "gut.png", 300, 200);
    let mut bad = photo(&d, "kaputt.png", 300, 200);
    std::fs::write(&bad.path, b"\x89PNG\r\n\x1a\n but then nothing useful follows at all").unwrap();
    bad.px = (300, 200);
    let job = JobSettings { export_dir: out.to_string_lossy().into_owned(), quality: QUALITY_DRAFT, ..Default::default() };
    let items = vec![bad, good];
    let sheets = plan(&items, &job);
    let pj = PrintJob { printer: EXPORT_PRINTER.into(), devmode: None, sheets, items, job };
    let events = Mutex::new(Vec::new());
    run_export(&pj, &control(), &|e| events.lock().unwrap().push(e)).unwrap();
    let ev = events.lock().unwrap();
    assert!(ev.iter().any(|e| matches!(e, JobEvent::ItemDone { item: 0, ok: false, .. })));
    assert!(ev.iter().any(|e| matches!(e, JobEvent::ItemDone { item: 1, ok: true, .. })));
    assert!(out.join("Blatt_0002_gut.jpg").exists());
}

#[test]
fn stopped_export_writes_nothing() {
    let d = tmp_dir("export-stop");
    let out = d.join("out");
    let job = JobSettings { export_dir: out.to_string_lossy().into_owned(), ..Default::default() };
    let items = vec![photo(&d, "a.png", 60, 40)];
    let sheets = plan(&items, &job);
    let pj = PrintJob { printer: EXPORT_PRINTER.into(), devmode: None, sheets, items, job };
    let c = control();
    c.stop.store(true, Ordering::Relaxed);
    assert!(run_export(&pj, &c, &|_| {}).is_err());
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 0);
}

/// A broken plan must end the job with a message – never a window stuck on "printing".
#[test]
fn broken_job_finishes_with_an_error() {
    let d = tmp_dir("start-broken");
    let items = vec![photo(&d, "a.png", 60, 40)];
    let job = JobSettings { export_dir: d.join("out").to_string_lossy().into_owned(), ..Default::default() };
    let mut sheets = plan(&items, &job);
    sheets[0].cells[0].item = 42;
    let pj = PrintJob { printer: EXPORT_PRINTER.into(), devmode: None, sheets, items, job };
    let (rx, _ctl) = start(pj, egui::Context::default());
    let last = rx.recv_timeout(std::time::Duration::from_secs(20)).expect("job ends");
    assert!(matches!(last, JobEvent::Finished { ok: false, .. }));
}

#[cfg(windows)]
mod devmode {
    use super::super::sys::checked_devmode;
    use std::mem::{offset_of, size_of};
    use windows::Win32::Graphics::Gdi::DEVMODEW;

    const FIXED: usize = size_of::<DEVMODEW>();

    fn dm(size: usize, extra: usize, len: usize) -> Vec<u8> {
        let mut b = vec![0xAB; len];
        let s = offset_of!(DEVMODEW, dmSize);
        let e = offset_of!(DEVMODEW, dmDriverExtra);
        b[s..s + 2].copy_from_slice(&(size as u16).to_le_bytes());
        b[e..e + 2].copy_from_slice(&(extra as u16).to_le_bytes());
        b
    }

    #[test]
    fn well_formed_devmode_passes() {
        let b = dm(FIXED, 100, FIXED + 100);
        assert_eq!(checked_devmode(&b).unwrap(), b);
        // Trailing bytes beyond the stated size are dropped.
        let long = dm(FIXED, 0, FIXED + 50);
        assert_eq!(checked_devmode(&long).unwrap().len(), FIXED);
    }

    #[test]
    fn devmode_that_claims_more_than_it_has_is_refused() {
        assert!(checked_devmode(&[]).is_none());
        assert!(checked_devmode(&[0u8; 60]).is_none());
        assert!(checked_devmode(&dm(FIXED, 1, FIXED)).is_none(), "driver data missing");
        assert!(checked_devmode(&dm(FIXED + 10, 0, FIXED)).is_none());
        assert!(checked_devmode(&dm(10, 0, FIXED)).is_none(), "too small to be a DEVMODE");
        assert!(checked_devmode(&dm(FIXED, 65535, FIXED + 100)).is_none());
    }

    #[test]
    fn older_short_devmode_is_widened_and_keeps_driver_data() {
        let short = FIXED - 32;
        let mut b = dm(short, 4, short + 4);
        b[short..].copy_from_slice(&[1, 2, 3, 4]);
        let out = checked_devmode(&b).unwrap();
        assert_eq!(out.len(), FIXED + 4);
        assert_eq!(&out[FIXED..], &[1, 2, 3, 4], "driver data stays behind the fixed part");
        assert!(out[short..FIXED].iter().all(|&x| x == 0), "new fields start empty");
        let s = offset_of!(DEVMODEW, dmSize);
        assert_eq!(u16::from_le_bytes([out[s], out[s + 1]]) as usize, FIXED);
        assert_eq!(checked_devmode(&out).unwrap(), out, "stable once widened");
    }

    /// The whole DEVMODE path against a real driver that every Windows has (no paper needed).
    #[test]
    fn real_driver_round_trip() {
        const PDF: &str = "Microsoft Print to PDF";
        let Some(dm) = super::super::sys::default_devmode(PDF) else {
            eprintln!("{PDF} not installed – skipped");
            return;
        };
        assert_eq!(checked_devmode(&dm).as_ref(), Some(&dm));
        let caps = crate::printer::caps(PDF, Some(&dm));
        let a4 = caps.papers.iter().find(|p| p.id == 9).expect("A4 offered");
        assert!((a4.w_mm - 210.0).abs() < 1.0 && (a4.h_mm - 297.0).abs() < 1.0);
        let portrait = crate::printer::page_info(PDF, Some(&dm), Some(9), None, None, Some(false));
        assert!((portrait.width_mm - 210.0).abs() < 1.0 && (portrait.height_mm - 297.0).abs() < 1.0, "{portrait:?}");
        let landscape = crate::printer::page_info(PDF, Some(&dm), Some(9), None, None, Some(true));
        assert!(landscape.width_mm > landscape.height_mm, "paper turned: {landscape:?}");
        // A broken DEVMODE is never handed to the driver; the printer's own settings are used.
        let broken = vec![0xFFu8; 40];
        assert!(!crate::printer::caps(PDF, Some(&broken)).papers.is_empty());
    }
}

/// Shows what a real printer's driver offers (run by hand):
/// `$env:MRPP_PRINTER="EPSON ET-4700 Series"; cargo test dump_printer -- --ignored --nocapture`
#[cfg(windows)]
#[test]
#[ignore]
fn dump_printer() {
    let (list, default) = list_printers();
    println!("printers: {list:?}\ndefault: {default:?}");
    let name = std::env::var("MRPP_PRINTER").ok().or(default).expect("a printer");
    let caps = caps(&name, None);
    println!("== {name}: {} papers, {} trays, media {:?}, resolutions {:?}", caps.papers.len(), caps.trays.len(), caps.media, caps.resolutions);
    for p in &caps.papers {
        let photo = (p.w_mm - 101.6).abs() < 4.0 || (p.h_mm - 101.6).abs() < 4.0 || p.name.to_lowercase().contains("randlos") || p.name.to_lowercase().contains("borderless");
        if photo {
            let pi = page_info(&name, None, Some(p.id), None, None, Some(true));
            println!("  {:>5} {:<45} {:6.1} x {:6.1} mm  margins {:?}", p.id, p.name, p.w_mm, p.h_mm, pi.hard);
            for tr in &caps.trays {
                let pt = page_info(&name, None, Some(p.id), Some(tr.id), None, Some(true));
                println!("        tray {:<28} {:6.1} x {:6.1} mm  margins {:?}", tr.name, pt.width_mm, pt.height_mm, pt.hard);
                for m in &caps.media {
                    let pm = page_info(&name, None, Some(p.id), Some(tr.id), Some(m.id), Some(true));
                    println!("            media {:<28} margins {:?}", m.name, pm.hard);
                }
            }
        }
    }
    println!("trays: {:?}", caps.trays);
}

/// Does the driver keep the paper source and type we set? (run by hand, like dump_printer)
#[cfg(windows)]
#[test]
#[ignore]
fn dump_devmode_fields() {
    use windows::Win32::Graphics::Gdi::DEVMODEW;
    let name = std::env::var("MRPP_PRINTER").expect("MRPP_PRINTER");
    let show = |label: &str, d: &[u8]| {
        let dm: DEVMODEW = unsafe { std::ptr::read_unaligned(d.as_ptr() as *const DEVMODEW) };
        let (paper, src, q) = unsafe { (dm.Anonymous1.Anonymous1.dmPaperSize, dm.Anonymous1.Anonymous1.dmDefaultSource, dm.Anonymous1.Anonymous1.dmPrintQuality) };
        println!("{label:<34} fields {:#010x} paper {paper} source {src} media {} quality {q} extra {} bytes", dm.dmFields.0, dm.dmMediaType, dm.dmDriverExtra);
    };
    let base = super::sys::default_devmode(&name).unwrap();
    show("printer default", &base);
    let d = super::sys::devmode_for(&name, None, Some(285), Some(292), Some(396), Some(true), ColorMode::AsPrinter, crate::model::QUALITY_HIGH).unwrap();
    show("after 10x15 + borderless + glossy", &d);
    std::fs::write(std::env::temp_dir().join("mrpp-devmode.bin"), &d).unwrap();
    let two = super::sys::devmode_for(&name, Some(&d), Some(285), Some(292), Some(396), Some(true), ColorMode::AsPrinter, crate::model::QUALITY_HIGH).unwrap();
    show("second pass", &two);
    let pi = page_info(&name, Some(&two), Some(285), Some(292), Some(396), Some(true));
    println!("margins after second pass {:?}", pi.hard);
    let only_src = super::sys::devmode_for(&name, None, None, Some(292), None, None, ColorMode::AsPrinter, crate::model::QUALITY_AS_PRINTER).unwrap();
    show("only source 292", &only_src);
}

#[test]
fn borderless_names_in_several_languages() {
    for n in ["Einzug hinten-Randlos", "4 x 6 in (Borderless)", "10x15 (sans marges)", "Senza margini 10x15", "A4 randloos"] {
        assert!(is_borderless_name(n), "{n}");
    }
    for n in ["Papiereinzug hinten", "A4", "10 x 15 cm (4 x 6 Zoll)", "Rand oben"] {
        assert!(!is_borderless_name(n), "{n}");
    }
}

#[test]
fn borderless_uses_the_drivers_own_way() {
    let paper = |id, name: &str, w, h| PaperDef { id, name: name.into(), w_mm: w, h_mm: h };
    // Epson style: a borderless paper source.
    let epson = Caps {
        papers: vec![paper(285, "10 x 15 cm (4 x 6 Zoll)", 101.6, 152.4), paper(9, "A4", 210.0, 297.0)],
        trays: vec![TrayDef { id: 261, name: "Papiereinzug hinten".into() }, TrayDef { id: 292, name: "Einzug hinten-Randlos".into() }],
        ..Default::default()
    };
    assert_eq!(borderless_variant(&epson, Some(285), None), (Some(285), Some(292)));
    assert_eq!(borderless_variant(&epson, Some(285), Some(261)), (Some(285), Some(261)), "a chosen source is kept");
    // Canon/HP style: a borderless variant of the paper size.
    let canon = Caps {
        papers: vec![paper(11, "10x15cm 4\"x6\"", 101.6, 152.4), paper(300, "10x15cm 4\"x6\" (Randlos)", 101.6, 152.4), paper(9, "A4", 210.0, 297.0)],
        ..Default::default()
    };
    assert_eq!(borderless_variant(&canon, Some(11), None), (Some(300), None));
    assert_eq!(borderless_variant(&canon, Some(9), None), (Some(9), None), "no borderless A4: unchanged");
    assert_eq!(borderless_variant(&Caps::default(), None, None), (None, None));
}

/// The whole borderless path against a real printer (run by hand, like dump_printer).
#[cfg(windows)]
#[test]
#[ignore]
fn real_borderless() {
    let name = std::env::var("MRPP_PRINTER").expect("MRPP_PRINTER");
    let caps = caps(&name, None);
    let p10x15 = caps.papers.iter().find(|p| (p.w_mm - 101.6).abs() < 1.0 && (p.h_mm - 152.4).abs() < 1.0).expect("10x15").id;
    let job = JobSettings { ignore_hard_margins: true, ..Default::default() };
    let (paper, tray) = borderless_variant(&caps, Some(p10x15), None);
    let photo = caps.media.iter().find(|m| m.name.to_lowercase().contains("gloss")).map(|m| m.id);
    for (label, media) in [("photo paper", photo), ("as printer", None)] {
        let pg = page_info(&name, None, paper, tray, media, Some(true));
        println!("{label:<12} paper {paper:?} tray {tray:?} media {media:?}: margins {:?} bleed {} refused {:?} area {:?}", pg.hard, pg.bleed, crate::layout::borderless_refused(&pg, &job), crate::layout::printable_area(&pg, &job));
    }
}
