//! Tests for adding photos: folder scans and list files (app.rs).

use super::*;

fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join("mrphotoprint-tests").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn touch(p: &Path) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, b"x").unwrap();
}

#[test]
fn folder_scan_finds_photos_in_subfolders_sorted() {
    let d = tmp_dir("scan");
    touch(&d.join("b.JPG"));
    touch(&d.join("a.png"));
    touch(&d.join("notes.txt"));
    touch(&d.join("Urlaub").join("c.heic"));
    touch(&d.join("Urlaub").join("program.exe"));
    let mut out = Vec::new();
    collect_dir(&d, &mut out, 0);
    let names: Vec<String> = out.iter().map(|p| p.strip_prefix(&d).unwrap().to_string_lossy().replace('\\', "/")).collect();
    assert_eq!(names, ["a.png", "b.JPG", "Urlaub/c.heic"]);
}

#[test]
fn folder_scan_stops_at_the_depth_limit() {
    let d = tmp_dir("deep");
    let mut deep = d.clone();
    for _ in 0..=MAX_SCAN_DEPTH + 1 {
        deep = deep.join("d");
    }
    touch(&deep.join("too-deep.jpg"));
    touch(&d.join("d").join("d").join("ok.jpg"));
    let mut out = Vec::new();
    collect_dir(&d, &mut out, 0);
    assert_eq!(out.len(), 1);
    assert!(out[0].ends_with("ok.jpg"));
}

#[test]
fn folder_scan_stops_at_the_file_limit() {
    let d = tmp_dir("many");
    touch(&d.join("a.jpg"));
    let mut out = vec![PathBuf::new(); MAX_SCAN_FILES];
    collect_dir(&d, &mut out, 0);
    assert_eq!(out.len(), MAX_SCAN_FILES);
}

/// A junction pointing back up would make the scan run in circles; it is skipped.
#[cfg(windows)]
#[test]
fn folder_scan_does_not_follow_junctions() {
    let d = tmp_dir("junction");
    touch(&d.join("real").join("photo.jpg"));
    let link = d.join("real").join("loop");
    let ok = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&d)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        eprintln!("mklink /J not available – skipped");
        return;
    }
    let mut out = Vec::new();
    collect_dir(&d, &mut out, 0);
    assert_eq!(out.len(), 1, "{out:?}");
    let _ = std::fs::remove_dir(&link);
}

#[test]
fn list_file_takes_only_local_existing_photos() {
    let d = tmp_dir("list");
    let photo = d.join("a.jpg");
    touch(&photo);
    touch(&d.join("b.txt"));
    std::fs::create_dir_all(d.join("folder.jpg")).unwrap();
    let text = format!(
        "{}\n\n   {}   \n{}\n{}\n{}\n\\\\server\\share\\x.jpg\n//server/share/y.jpg\n\\\\?\\UNC\\server\\share\\z.jpg\nhttps://example.com/p.jpg\nfile://server/share/q.jpg\n",
        photo.display(),
        photo.display(),
        d.join("missing.jpg").display(),
        d.join("b.txt").display(),
        d.join("folder.jpg").display(),
    );
    let (paths, blocked) = list_entries(&text);
    assert_eq!(paths, vec![photo.clone(), photo], "blank lines skipped, spaces trimmed");
    assert_eq!(blocked, 8);
}

#[test]
fn list_file_is_capped() {
    let d = tmp_dir("list-cap");
    let photo = d.join("a.jpg");
    touch(&photo);
    let line = format!("{}\n", photo.display());
    let (paths, blocked) = list_entries(&line.repeat(MAX_SCAN_FILES + 10));
    assert_eq!(paths.len(), MAX_SCAN_FILES);
    assert_eq!(blocked, 0);
}

#[test]
fn driver_settings_survive_hex_and_junk_is_refused() {
    let b: Vec<u8> = (0..=255).collect();
    assert_eq!(from_hex(&to_hex(&b)).unwrap(), b);
    assert!(from_hex("abc").is_none(), "odd length");
    assert!(from_hex("zz").is_none());
    assert!(from_hex("€€").is_none(), "multibyte characters don't panic");
    assert!(from_hex(&"0".repeat(2 * 256 * 1024 + 2)).is_none(), "too long");
}
