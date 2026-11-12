//! Tests for settings, their limits and the path checks (model.rs).

use super::*;
use std::path::Path;

fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join("mrphotoprint-tests").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn defaults_are_sensible_for_photo_prints() {
    let j = JobSettings::default();
    let p = &j.photo;
    assert_eq!(p.scale, ScaleMode::Shrink, "never enlarge by default");
    assert_eq!(p.auto_rotate, AutoRotate::Clockwise);
    assert_eq!(p.orientation, Orientation::ByPhoto, "paper follows the photo");
    assert_eq!((p.h_align, p.v_align), (HAlign::Center, VAlign::Center));
    assert_eq!((p.zoom, p.copies, p.rotation), (100, 1, 0));
    assert_eq!((p.paper, p.tray, p.color), (None, None, ColorMode::AsPrinter));
    assert_eq!((j.cols, j.rows), (1, 1));
    assert_eq!(j.gap_pct, 0.5);
    assert!(j.frame && j.remove_printed);
    assert!(!j.next_photo_new_sheet, "4 per sheet means 4 different photos per sheet");
    assert!(!j.single_job && !j.cut_marks && !j.ignore_hard_margins);
    assert_eq!(j.cut_marks_offset, 0.0);
    assert_eq!(j.quality, QUALITY_AS_PRINTER, "the driver decides");
    assert!(!j.collage());
}

#[test]
fn defaults_survive_sanitize_unchanged() {
    assert_eq!(JobSettings::default().sanitize(), JobSettings::default());
}

#[test]
fn sanitize_clamps_hostile_values() {
    let mut j = JobSettings {
        cols: 0,
        rows: u32::MAX,
        gap_pct: f32::NAN,
        extra_margin_mm: f32::INFINITY,
        cut_marks_offset: -1e9,
        export_dpi: 1,
        ..Default::default()
    };
    j.photo.zoom = 0;
    j.photo.copies = u32::MAX;
    j.photo.rotation = -450;
    j.photo.offset_x_mm = f32::NEG_INFINITY;
    j.photo.offset_y_mm = 1e30;
    let j = j.sanitize();
    assert_eq!((j.cols, j.rows), (1, 10));
    assert_eq!(j.gap_pct, 0.5);
    assert_eq!(j.extra_margin_mm, 0.0);
    assert_eq!(j.cut_marks_offset, -100.0);
    assert_eq!(j.export_dpi, 72);
    assert_eq!((j.photo.zoom, j.photo.copies), (5, 999));
    assert_eq!(j.photo.rotation, 270);
    assert_eq!((j.photo.offset_x_mm, j.photo.offset_y_mm), (0.0, 500.0));
}

#[test]
fn rotation_snaps_to_quarter_turns() {
    for (given, want) in [(0, 0), (89, 0), (90, 90), (359, 270), (360, 0), (-90, 270), (i32::MIN, 180)] {
        let p = PhotoSettings { rotation: given, ..Default::default() }.sanitize();
        assert_eq!(p.rotation, want, "{given}");
    }
}

#[test]
fn only_known_print_qualities_are_kept() {
    for q in [QUALITY_AS_PRINTER, QUALITY_DRAFT, QUALITY_LOW, QUALITY_MEDIUM, QUALITY_HIGH, 50, 300, 4800] {
        assert_eq!(JobSettings { quality: q, ..Default::default() }.sanitize().quality, q);
    }
    for q in [0, -5, -998, 49, 4801, i32::MAX, i32::MIN] {
        assert_eq!(JobSettings { quality: q, ..Default::default() }.sanitize().quality, QUALITY_AS_PRINTER, "{q}");
    }
}

#[test]
fn long_names_are_cut_at_a_character_boundary() {
    // 3-byte characters: 512 is not a boundary, a plain truncate would panic.
    let j = JobSettings { printer: "€".repeat(400), export_dir: "ä".repeat(3000) + "€", ..Default::default() }.sanitize();
    assert!(j.printer.len() <= 512 && !j.printer.is_empty() && j.printer.chars().all(|c| c == '€'));
    assert!(j.export_dir.len() <= 4096);
    let mut s = String::from("abc");
    truncate_at_char(&mut s, 10);
    assert_eq!(s, "abc");
}

#[test]
fn settings_round_trip_through_json() {
    let mut j = JobSettings { cols: 3, rows: 4, printer: "Büro".into(), quality: QUALITY_HIGH, ..Default::default() };
    j.photo.scale = ScaleMode::Fill;
    j.photo.paper = Some(9);
    let text = serde_json::to_string(&j).unwrap();
    assert_eq!(serde_json::from_str::<JobSettings>(&text).unwrap(), j);
}

#[test]
fn partial_settings_keep_the_other_defaults() {
    let j: JobSettings = serde_json::from_str(r#"{"cols":2,"photo":{"zoom":50}}"#).unwrap();
    assert_eq!(j.cols, 2);
    assert_eq!(j.photo.zoom, 50);
    assert_eq!(j.photo.scale, ScaleMode::Shrink);
    assert_eq!(j.rows, 1);
}

#[test]
fn untrusted_settings_are_checked() {
    assert!(JobSettings::from_untrusted_json("not json").is_none());
    assert!(JobSettings::from_untrusted_json(r#"{"cols":-1}"#).is_none(), "wrong type is refused");
    assert!(JobSettings::from_untrusted_json(r#"{"photo":{"scale":"Explode"}}"#).is_none());
    let big = format!(r#"{{"printer":"{}"}}"#, "x".repeat(MAX_SETTINGS_BYTES));
    assert!(JobSettings::from_untrusted_json(&big).is_none());

    let j = JobSettings::from_untrusted_json(r#"{"cols":99,"export_dir":"\\\\evil\\share\\out"}"#).unwrap();
    assert_eq!(j.cols, 10, "clamped");
    assert!(j.export_dir.is_empty(), "network export folder dropped");
    let j = JobSettings::from_untrusted_json(r#"{"export_dir":"//evil/share"}"#).unwrap();
    assert!(j.export_dir.is_empty());
    let j = JobSettings::from_untrusted_json(r#"{"export_dir":"C:\\Fotos\\Export"}"#).unwrap();
    assert_eq!(j.export_dir, "C:\\Fotos\\Export", "local folder kept");
}

#[test]
fn network_paths_are_recognised() {
    for p in [
        r"\\server\share\a.jpg",
        "//server/share/a.jpg",
        r"\\?\UNC\server\share\a.jpg",
        r"\\?\unc\server\share\a.jpg",
        r"\\.\UNC\server\share\a.jpg",
        r"\\?\GLOBALROOT\Device\Mup\server\share\a.jpg",
        r"\\.\pipe\something",
        r"\\?\Volume{12345678-0000-0000-0000-000000000000}\a.jpg",
        r"\??\UNC\server\share\a.jpg",
        r"\\192.168.0.1\c$\a.jpg",
    ] {
        assert!(is_network_path(Path::new(p)), "{p}");
    }
    for p in [r"C:\Fotos\a.jpg", "C:/Fotos/a.jpg", r"\\?\C:\Fotos\a.jpg", r"\\.\D:\a.jpg", r"Fotos\a.jpg", r"\Fotos\a.jpg", ""] {
        assert!(!is_network_path(Path::new(p)), "{p}");
    }
}

#[test]
fn photo_extensions_ignore_case() {
    for p in ["a.jpg", "B.JPG", "c.Jpeg", "d.heic", "e.CR3", "f.webp", "g.tiff"] {
        assert!(is_photo(Path::new(p)), "{p}");
    }
    for p in ["a.txt", "b.exe", "c.jpg.lnk", "noext", ".jpg", "d.svg", "e.pdf"] {
        assert!(!is_photo(Path::new(p)), "{p}");
    }
}

#[test]
fn collage_needs_more_than_one_cell() {
    let j = |cols, rows| JobSettings { cols, rows, ..Default::default() };
    assert!(!j(1, 1).collage());
    assert!(!j(0, 0).collage());
    assert!(j(2, 1).collage());
    assert!(j(1, 3).collage());
}

#[test]
fn export_resolution_per_quality() {
    let q = |quality| export_dpi_for(&JobSettings { quality, export_dpi: 240, ..Default::default() });
    assert_eq!(q(QUALITY_AS_PRINTER), 240);
    assert_eq!(q(QUALITY_DRAFT), 150);
    assert_eq!(q(QUALITY_LOW), 200);
    assert_eq!(q(QUALITY_MEDIUM), 300);
    assert_eq!(q(QUALITY_HIGH), 600);
    assert_eq!(q(50), 72);
    assert_eq!(q(4800), 1200);
}

#[test]
fn quality_labels() {
    assert_eq!(quality_label(QUALITY_AS_PRINTER), "Wie beim Drucker");
    assert_eq!(quality_label(QUALITY_HIGH), "Hoch");
    assert_eq!(quality_label(600), "600 dpi");
}

#[test]
fn scale_mode_list_is_complete() {
    let mut all = ScaleMode::ALL.to_vec();
    all.dedup();
    assert_eq!(all.len(), 5);
    assert!(all.iter().all(|m| !m.label().is_empty() && !m.short().is_empty()));
}

#[test]
fn item_uses_own_settings_before_the_job() {
    let job = JobSettings::default();
    let mut it = Item::new(PathBuf::from("does-not-exist.jpg"));
    assert_eq!(it.bytes, 0);
    assert!(it.checked && it.status == ItemStatus::Waiting);
    assert_eq!(it.name, "does-not-exist.jpg");
    assert_eq!(it.settings(&job), &job.photo);
    let own = PhotoSettings { copies: 3, ..Default::default() };
    it.own = Some(own.clone());
    assert_eq!(it.settings(&job), &own);
}

#[test]
fn atomic_write_replaces_and_leaves_no_temp_file() {
    let d = tmp_dir("atomic");
    let p = d.join("einstellungen.json");
    write_atomic(&p, b"one").unwrap();
    write_atomic(&p, b"two").unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), b"two");
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1, "no .tmp left behind");
    assert!(write_atomic(&d.join("missing").join("x.json"), b"x").is_err());
}
