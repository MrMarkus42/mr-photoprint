//! Tests for the updater: versions, the release list, and above all the signature checks
//! (update.rs). Test keys are made here with the same format minisign uses.

use super::*;
use blake2::{Blake2b512, Digest};
use ed25519_dalek::{Signer, SigningKey};

fn b64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

struct Key {
    sk: SigningKey,
    id: [u8; 8],
}

impl Key {
    fn new(seed: u8) -> Key {
        Key { sk: SigningKey::from_bytes(&[seed; 32]), id: [seed, 1, 2, 3, 4, 5, 6, 7] }
    }
    fn public(&self) -> String {
        let mut b = b"Ed".to_vec();
        b.extend_from_slice(&self.id);
        b.extend_from_slice(self.sk.verifying_key().as_bytes());
        format!("untrusted comment: minisign public key\n{}\n", b64(&b))
    }
    /// A pre-hashed minisign signature with the given trusted comment.
    fn sign(&self, data: &[u8], trusted: &str) -> String {
        let hash = Blake2b512::digest(data);
        let sig = self.sk.sign(&hash).to_bytes();
        let mut line = b"ED".to_vec();
        line.extend_from_slice(&self.id);
        line.extend_from_slice(&sig);
        let mut global = sig.to_vec();
        global.extend_from_slice(trusted.as_bytes());
        let gsig = self.sk.sign(&global).to_bytes();
        format!("untrusted comment: signature\n{}\ntrusted comment: {trusted}\n{}\n", b64(&line), b64(&gsig))
    }
}

const V: Version = Version(1, 2, 0);

#[test]
fn good_signature_is_accepted() {
    let k = Key::new(7);
    let data = b"MZ pretend installer";
    let sig = k.sign(data, "mr-photoprint-setup 1.2.0");
    assert_eq!(verify_with(&k.public(), data, &sig, SETUP_KIND, V), Ok(()));
    // Windows line endings in the .minisig don't matter.
    assert_eq!(verify_with(&k.public(), data, &sig.replace('\n', "\r\n"), SETUP_KIND, V), Ok(()));
}

#[test]
fn changed_file_is_refused() {
    let k = Key::new(7);
    let sig = k.sign(b"MZ pretend installer", "mr-photoprint-setup 1.2.0");
    assert!(verify_with(&k.public(), b"MZ pretend installer!", &sig, SETUP_KIND, V).is_err());
    assert!(verify_with(&k.public(), b"", &sig, SETUP_KIND, V).is_err());
}

#[test]
fn signature_from_another_key_is_refused() {
    let mine = Key::new(7);
    let attacker = Key::new(9);
    let data = b"MZ evil installer";
    let sig = attacker.sign(data, "mr-photoprint-setup 1.2.0");
    assert!(verify_with(&mine.public(), data, &sig, SETUP_KIND, V).is_err());
    // Same key id, different key: still refused.
    let forged = Key { sk: SigningKey::from_bytes(&[9; 32]), id: mine.id };
    let sig = forged.sign(data, "mr-photoprint-setup 1.2.0");
    assert!(verify_with(&mine.public(), data, &sig, SETUP_KIND, V).is_err());
}

#[test]
fn old_or_other_signed_file_cannot_pose_as_the_update() {
    let k = Key::new(7);
    let data = b"MZ genuine but old installer";
    // A genuinely signed 1.1.0 installer offered as 1.2.0:
    let sig = k.sign(data, "mr-photoprint-setup 1.1.0");
    assert!(verify_with(&k.public(), data, &sig, SETUP_KIND, V).is_err());
    // A different kind of file:
    let sig = k.sign(data, "mr-photoprint-portable 1.2.0");
    assert!(verify_with(&k.public(), data, &sig, SETUP_KIND, V).is_err());
    // Trusted comment changed after signing:
    let sig = k.sign(data, "mr-photoprint-setup 1.1.0").replace("trusted comment: mr-photoprint-setup 1.1.0", "trusted comment: mr-photoprint-setup 1.2.0");
    assert!(verify_with(&k.public(), data, &sig, SETUP_KIND, V).is_err());
}

#[test]
fn legacy_unhashed_signatures_are_refused() {
    let k = Key::new(7);
    let data = b"MZ installer";
    let sig = k.sk.sign(data).to_bytes();
    let mut line = b"Ed".to_vec();
    line.extend_from_slice(&k.id);
    line.extend_from_slice(&sig);
    let trusted = "mr-photoprint-setup 1.2.0";
    let mut global = sig.to_vec();
    global.extend_from_slice(trusted.as_bytes());
    let text = format!("untrusted comment: x\n{}\ntrusted comment: {trusted}\n{}\n", b64(&line), b64(&k.sk.sign(&global).to_bytes()));
    assert!(verify_with(&k.public(), data, &text, SETUP_KIND, V).is_err());
}

#[test]
fn broken_signatures_and_keys_are_refused() {
    let k = Key::new(7);
    for bad in ["", "garbage", "untrusted comment: x\n!!!\ntrusted comment: y\n!!!\n"] {
        assert!(verify_with(&k.public(), b"x", bad, SETUP_KIND, V).is_err());
    }
    let sig = k.sign(b"x", "mr-photoprint-setup 1.2.0");
    assert!(verify_with("", b"x", &sig, SETUP_KIND, V).is_err());
    assert!(verify_with("not a key", b"x", &sig, SETUP_KIND, V).is_err());
}

#[test]
fn the_built_in_key_is_a_real_key_or_disables_updates() {
    // Either the release key is set up, or no update can ever pass.
    if !key_configured() {
        let k = Key::new(7);
        let sig = k.sign(b"x", "mr-photoprint-setup 1.2.0");
        assert!(verify(b"x", &sig, SETUP_KIND, V).is_err());
    }
}

#[test]
fn versions_compare_by_number() {
    assert_eq!(Version::parse("v1.2.3"), Some(Version(1, 2, 3)));
    assert_eq!(Version::parse(" 10.0.0 "), Some(Version(10, 0, 0)));
    assert!(Version(1, 10, 0) > Version(1, 9, 9));
    assert!(Version(2, 0, 0) > Version(1, 99, 99));
    for bad in ["", "1", "1.2", "1.2.3.4", "1.2.x", "1.2.3-beta", "v", "-1.2.3", "1..3", "1.2.9999999"] {
        assert_eq!(Version::parse(bad), None, "{bad}");
    }
    assert_eq!(Version::current().to_string(), env!("CARGO_PKG_VERSION"));
}

fn release_json(tag: &str, assets: &[(&str, &str)], extra: &str) -> Vec<u8> {
    let a: Vec<String> = assets.iter().map(|(n, u)| format!(r#"{{"name":"{n}","browser_download_url":"{u}","size":1234}}"#)).collect();
    format!(r#"{{"tag_name":"{tag}","body":"Neu / New","html_url":"{REPO_URL}/releases/tag/{tag}","assets":[{}]{extra}}}"#, a.join(",")).into_bytes()
}

fn dl(name: &str) -> String {
    format!("https://github.com/{REPO}/releases/download/v1.2.0/{name}")
}

#[test]
fn newer_release_with_signed_installer_is_found() {
    let (i, s) = (dl("MrPhotoPrint-Setup-1.2.0.exe"), dl("MrPhotoPrint-Setup-1.2.0.exe.minisig"));
    let json = release_json("v1.2.0", &[("MrPhotoPrint-Setup-1.2.0.exe", &i), ("MrPhotoPrint-Setup-1.2.0.exe.minisig", &s)], "");
    let r = parse_latest(&json, Version(1, 0, 0)).unwrap().unwrap();
    assert_eq!(r.version, V);
    assert_eq!((r.installer_url, r.signature_url, r.size), (i, s, 1234));
    assert_eq!(r.notes, "Neu / New");
    assert!(r.page.starts_with(REPO_URL));
}

#[test]
fn releases_that_are_not_offered() {
    let (i, s) = (dl("MrPhotoPrint-Setup-1.2.0.exe"), dl("MrPhotoPrint-Setup-1.2.0.exe.minisig"));
    let both = [("MrPhotoPrint-Setup-1.2.0.exe", i.as_str()), ("MrPhotoPrint-Setup-1.2.0.exe.minisig", s.as_str())];
    let none = |json: Vec<u8>, cur| parse_latest(&json, cur).unwrap().is_none();
    assert!(none(release_json("v1.2.0", &both, ""), Version(1, 2, 0)), "same version");
    assert!(none(release_json("v1.2.0", &both, ""), Version(1, 3, 0)), "older than running");
    assert!(none(release_json("v1.2.0", &both, r#","prerelease":true"#), Version(1, 0, 0)));
    assert!(none(release_json("v1.2.0", &both, r#","draft":true"#), Version(1, 0, 0)));
    assert!(none(release_json("v1.2.0", &both[..1], ""), Version(1, 0, 0)), "no signature");
    assert!(none(release_json("nightly", &both, ""), Version(1, 0, 0)));
    // Files somewhere else than this project's releases are ignored.
    let evil = [("MrPhotoPrint-Setup-1.2.0.exe", "https://evil.example/MrPhotoPrint-Setup-1.2.0.exe"), both[1]];
    assert!(none(release_json("v1.2.0", &evil, ""), Version(1, 0, 0)));
    let other_repo = [("MrPhotoPrint-Setup-1.2.0.exe", "https://github.com/someone/else/releases/download/v1.2.0/MrPhotoPrint-Setup-1.2.0.exe"), both[1]];
    assert!(none(release_json("v1.2.0", &other_repo, ""), Version(1, 0, 0)));
    assert!(parse_latest(b"<html>", Version(1, 0, 0)).is_err());
}

#[test]
fn release_urls_are_checked() {
    assert!(is_release_url(&dl("a.exe")));
    for bad in [
        "http://github.com/MrMarkus42/mr-photoprint/releases/download/v1/a.exe".to_string(),
        format!("https://github.com/{REPO}/releases/download/../../x"),
        format!("https://github.com/{REPO}/releases/download/v1/a.exe?x=1"),
        format!("https://github.com/{REPO}/releases/download/v1/a.exe@evil.example"),
        format!("https://github.com.evil.example/{REPO}/releases/download/v1/a.exe"),
    ] {
        assert!(!is_release_url(&bad), "{bad}");
    }
}

#[test]
fn check_is_due_by_frequency() {
    let mut p = UpdatePrefs { check: CheckFreq::Daily, last_check: 1_000_000, ..Default::default() };
    assert!(!p.due(1_000_000 + 3600));
    assert!(p.due(1_000_000 + 86_400));
    assert!(p.due(10), "clock went backwards");
    p.check = CheckFreq::Weekly;
    assert!(!p.due(1_000_000 + 86_400 * 3));
    p.check = CheckFreq::Startup;
    assert!(p.due(1_000_000));
    p.check = CheckFreq::Never;
    assert!(!p.due(u64::MAX));
    assert_eq!(UpdatePrefs::default().check, CheckFreq::Never, "nothing goes online until the person agrees");
}

#[test]
fn http_refuses_plain_http_and_odd_hosts() {
    let mut nop = |_: u64, _: u64| {};
    assert!(http::get("http://github.com/", 10, &mut nop).is_err());
    assert!(http::get("https://user@github.com/", 10, &mut nop).is_err());
    assert!(http::get("https://github.com:8080/", 10, &mut nop).is_err());
    assert!(http::get("ftp://x/", 10, &mut nop).is_err());
}

#[test]
fn portable_copy_never_installs() {
    // The test program has no uninstaller next to it, so it counts as portable.
    assert_eq!(install_kind(), Install::Portable);
    assert!(install(Path::new("nothing.exe"), "", V, false).is_err());
}

/// Talks to the real GitHub (run by hand: `cargo test live_ -- --ignored`).
#[test]
#[ignore]
fn live_github_answers() {
    let mut nop = |_: u64, _: u64| {};
    let body = http::get("https://api.github.com/repos/rust-lang/rust/releases/latest", MAX_LIST, &mut nop).expect("GitHub reachable");
    let r: GhRelease = serde_json::from_slice(&body).expect("release JSON");
    assert!(Version::parse(&r.tag_name).is_some(), "{}", r.tag_name);
    // This project's own release list (404 until the first release is published).
    match http::get(&format!("https://api.github.com/repos/{REPO}/releases/latest"), MAX_LIST, &mut nop) {
        Ok(b) => println!("own release list: {} bytes", b.len()),
        Err(e) => println!("own release list: {e}"),
    }
}

/// A real download with GitHub's redirect, checked against a real minisign signature: minisign's
/// own release, signed with its author's published key (run by hand, like the test above).
#[test]
#[ignore]
fn live_real_minisign_signature() {
    const MINISIGN_KEY: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    let mut nop = |_: u64, _: u64| {};
    let body = http::get("https://api.github.com/repos/jedisct1/minisign/releases/latest", MAX_LIST, &mut nop).unwrap();
    let r: GhRelease = serde_json::from_slice(&body).unwrap();
    let zip = r.assets.iter().find(|a| a.name.ends_with("win64.zip")).expect("win64 zip");
    let sig = r.assets.iter().find(|a| a.name == format!("{}.minisig", zip.name)).expect("its signature");
    let data = http::get(&zip.browser_download_url, MAX_INSTALLER, &mut nop).expect("download follows the redirect");
    let sig_text = String::from_utf8(http::get(&sig.browser_download_url, MAX_SIG, &mut nop).unwrap()).unwrap();
    let pk = minisign_verify::PublicKey::from_base64(MINISIGN_KEY).unwrap();
    let s = minisign_verify::Signature::decode(&sig_text).unwrap();
    pk.verify(&data, &s, false).expect("genuine file verifies");
    let mut bad = data.clone();
    bad[data.len() / 2] ^= 1;
    assert!(pk.verify(&bad, &s, false).is_err(), "one changed bit is caught");
    println!("{} ({} bytes) verified: {}", zip.name, data.len(), s.trusted_comment());
}
