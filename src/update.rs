//! Looking for, fetching and installing new versions from GitHub – safely.
//!
//! Trust comes only from a signature, never from where a file was downloaded:
//! * Each release carries the installer and a minisign signature (`.minisig`) made with a private
//!   key that exists only on the maintainer's own computer (password-protected). The matching
//!   *public* key is compiled into the program (`keys/update-key.pub`); it is not read from any
//!   file or registry value at run time, so nothing on this PC can swap it.
//! * The signature's trusted comment names the file kind and the version
//!   (`mr-photoprint-setup 1.2.0`). It must match the release and be newer than the running
//!   version, so an old (genuinely signed) installer can't be passed off as an update.
//! * The installer is opened so that nobody can change it, checked, and started while it is still
//!   held open – nothing can swap it between the check and the start (or the UAC prompt).
//! * Only HTTPS (TLS 1.2+) via Windows' own WinHTTP; sizes are capped; nothing but the release
//!   list and its files is requested, and nothing about this PC is sent.

use crate::i18n::{t, ts};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The GitHub project releases come from.
pub const REPO: &str = "MrMarkus42/mr-photoprint";
pub const REPO_URL: &str = "https://github.com/MrMarkus42/mr-photoprint";

/// The public half of the release signing key (minisign format). See docs/RELEASING.md.
const UPDATE_KEY: &str = include_str!("../keys/update-key.pub");

/// Largest release list, installer and signature that are accepted.
const MAX_LIST: usize = 2 * 1024 * 1024;
const MAX_INSTALLER: usize = 150 * 1024 * 1024;
const MAX_SIG: usize = 4096;

pub const SETUP_KIND: &str = "mr-photoprint-setup";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum CheckFreq {
    Never,
    Startup,
    Daily,
    Weekly,
}

impl CheckFreq {
    pub const ALL: [CheckFreq; 4] = [CheckFreq::Startup, CheckFreq::Daily, CheckFreq::Weekly, CheckFreq::Never];
    pub fn label(self) -> &'static str {
        match self {
            CheckFreq::Never => t("Nie", "Never"),
            CheckFreq::Startup => t("Bei jedem Start", "Every start"),
            CheckFreq::Daily => t("Täglich", "Daily"),
            CheckFreq::Weekly => t("Wöchentlich", "Weekly"),
        }
    }
    fn interval_secs(self) -> Option<u64> {
        match self {
            CheckFreq::Never => None,
            CheckFreq::Startup => Some(0),
            CheckFreq::Daily => Some(86_400),
            CheckFreq::Weekly => Some(7 * 86_400),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum UpdateMode {
    /// Only say that there is a new version.
    Notify,
    /// Download it in the background, then offer to install.
    Download,
    /// Download it and install it when the program is closed.
    Auto,
}

impl UpdateMode {
    pub const ALL: [UpdateMode; 3] = [UpdateMode::Notify, UpdateMode::Download, UpdateMode::Auto];
    pub fn label(self) -> &'static str {
        match self {
            UpdateMode::Notify => t("Nur Bescheid sagen", "Just let me know"),
            UpdateMode::Download => t("Herunterladen und Installieren anbieten", "Download and offer to install"),
            UpdateMode::Auto => t("Automatisch beim Beenden installieren", "Install automatically when closing"),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdatePrefs {
    pub check: CheckFreq,
    pub mode: UpdateMode,
    /// The person has answered the first-start question.
    pub asked: bool,
    /// A version the person chose to skip.
    pub skipped: String,
    /// Last check, seconds since 1970.
    pub last_check: u64,
}

impl Default for UpdatePrefs {
    fn default() -> Self {
        Self { check: CheckFreq::Never, mode: UpdateMode::Download, asked: false, skipped: String::new(), last_check: 0 }
    }
}

impl UpdatePrefs {
    pub fn due(&self, now: u64) -> bool {
        match self.check.interval_secs() {
            None => false,
            Some(0) => true,
            Some(s) => now.saturating_sub(self.last_check) >= s || self.last_check > now,
        }
    }
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ------------------------------------------------------------------------------- versions

/// A version like 1.2.3 (a leading "v" is allowed). Pre-release suffixes are not accepted.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    pub fn parse(s: &str) -> Option<Version> {
        let s = s.trim();
        let s = s.strip_prefix('v').unwrap_or(s);
        let mut it = s.split('.');
        let mut part = || -> Option<u32> {
            let p = it.next()?;
            if p.is_empty() || p.len() > 6 || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            p.parse().ok()
        };
        let v = Version(part()?, part()?, part()?);
        if it.next().is_some() {
            return None;
        }
        Some(v)
    }
    pub fn current() -> Version {
        Version::parse(env!("CARGO_PKG_VERSION")).expect("package version is x.y.z")
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

// ------------------------------------------------------------------------------- releases

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// A newer release that has a signed installer.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: Version,
    pub notes: String,
    pub page: String,
    pub installer_url: String,
    pub signature_url: String,
    pub size: u64,
}

pub fn installer_name(v: Version) -> String {
    format!("MrPhotoPrint-Setup-{v}.exe")
}

/// Only files of this project's own releases are fetched.
fn is_release_url(url: &str) -> bool {
    let prefix = format!("https://github.com/{REPO}/releases/download/");
    url.starts_with(&prefix) && !url.contains("..") && !url[prefix.len()..].contains(['?', '#', '\\', '@'])
}

/// Read GitHub's "latest release" answer. `None` when it is not newer than `current`, is a draft
/// or pre-release, or lacks a signed installer.
pub fn parse_latest(json: &[u8], current: Version) -> Result<Option<Release>, String> {
    let r: GhRelease = serde_json::from_slice(json).map_err(|e| format!("{} {e}", t("Antwort von GitHub unlesbar:", "Unreadable answer from GitHub:")))?;
    if r.draft || r.prerelease {
        return Ok(None);
    }
    let Some(version) = Version::parse(&r.tag_name) else { return Ok(None) };
    if version <= current {
        return Ok(None);
    }
    let name = installer_name(version);
    let sig_name = format!("{name}.minisig");
    let find = |n: &str| r.assets.iter().find(|a| a.name == n && is_release_url(&a.browser_download_url));
    let (Some(inst), Some(sig)) = (find(&name), find(&sig_name)) else { return Ok(None) };
    let mut notes = r.body.unwrap_or_default();
    if notes.len() > 20_000 {
        let mut cut = 20_000;
        while !notes.is_char_boundary(cut) {
            cut -= 1;
        }
        notes.truncate(cut);
        notes.push_str(" …");
    }
    let page = if r.html_url.starts_with(&format!("{REPO_URL}/releases/")) { r.html_url } else { format!("{REPO_URL}/releases") };
    Ok(Some(Release {
        version,
        notes,
        page,
        installer_url: inst.browser_download_url.clone(),
        signature_url: sig.browser_download_url.clone(),
        size: inst.size,
    }))
}

// ------------------------------------------------------------------------------- signatures

/// Is the release key set up in this build? (A build without it never installs anything.)
pub fn key_configured() -> bool {
    minisign_verify::PublicKey::decode(UPDATE_KEY).is_ok()
}

/// Check `data` against `signature` (minisign text) with the public key `key` (minisign .pub
/// text). The trusted comment must be exactly "<kind> <version>".
pub fn verify_with(key: &str, data: &[u8], signature: &str, kind: &str, version: Version) -> Result<(), String> {
    let pk = minisign_verify::PublicKey::decode(key).map_err(|_| t("Kein gültiger Update-Schlüssel in diesem Programm.", "No valid update key in this program."))?;
    let sig = minisign_verify::Signature::decode(signature).map_err(|_| t("Die Signatur ist unlesbar.", "The signature is unreadable."))?;
    // Only the modern, pre-hashed form (allow_legacy = false).
    pk.verify(data, &sig, false).map_err(|_| t("Die Signatur passt nicht – die Datei wurde NICHT installiert.", "The signature does not match – the file was NOT installed."))?;
    let expected = format!("{kind} {version}");
    if sig.trusted_comment().trim() != expected {
        return Err(t("Die Signatur gehört zu einer anderen Datei oder Version.", "The signature belongs to a different file or version.").into());
    }
    Ok(())
}

pub fn verify(data: &[u8], signature: &str, kind: &str, version: Version) -> Result<(), String> {
    verify_with(UPDATE_KEY, data, signature, kind, version)
}

// ------------------------------------------------------------------------------- state

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Downloading { release: Release, done: u64, total: u64 },
    /// Downloaded and checked; `file` is the installer, `signature` its minisig text.
    Ready { release: Release, file: PathBuf, signature: String },
    Failed(String),
}

#[derive(Clone)]
pub struct Updater {
    pub phase: Arc<Mutex<Phase>>,
    ctx: egui::Context,
}

impl Updater {
    pub fn new(ctx: egui::Context) -> Self {
        cleanup_old_downloads();
        Self { phase: Arc::new(Mutex::new(Phase::Idle)), ctx }
    }

    pub fn phase(&self) -> Phase {
        self.phase.lock().map(|p| p.clone()).unwrap_or(Phase::Idle)
    }

    fn set(&self, p: Phase) {
        if let Ok(mut g) = self.phase.lock() {
            *g = p;
        }
        self.ctx.request_repaint();
    }

    pub fn busy(&self) -> bool {
        matches!(self.phase(), Phase::Checking | Phase::Downloading { .. })
    }

    /// Ask GitHub in the background. With `then_download` a found release is fetched right away.
    pub fn check(&self, then_download: bool) {
        if self.busy() {
            return;
        }
        self.set(Phase::Checking);
        let me = self.clone();
        std::thread::spawn(move || {
            let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
            let result = http::get(&url, MAX_LIST, &mut |_, _| {}).and_then(|body| parse_latest(&body, Version::current()));
            match result {
                Ok(Some(r)) => {
                    crate::log::info(&format!("update: version {} available", r.version));
                    if then_download {
                        me.run_download(r);
                    } else {
                        me.set(Phase::Available(r));
                    }
                }
                Ok(None) => me.set(Phase::UpToDate),
                Err(e) => {
                    crate::log::warn(&format!("update check failed: {e}"));
                    me.set(Phase::Failed(e));
                }
            }
        });
    }

    pub fn download(&self, r: Release) {
        if self.busy() {
            return;
        }
        let me = self.clone();
        std::thread::spawn(move || me.run_download(r));
    }

    fn run_download(&self, r: Release) {
        self.set(Phase::Downloading { release: r.clone(), done: 0, total: r.size });
        let result = (|| -> Result<(PathBuf, String), String> {
            if !is_release_url(&r.installer_url) || !is_release_url(&r.signature_url) {
                return Err(t("Unerwartete Download-Adresse.", "Unexpected download address.").into());
            }
            let sig = http::get(&r.signature_url, MAX_SIG, &mut |_, _| {})?;
            let sig = String::from_utf8(sig).map_err(|_| t("Die Signatur ist unlesbar.", "The signature is unreadable."))?;
            let me = self.clone();
            let rel = r.clone();
            let data = http::get(&r.installer_url, MAX_INSTALLER, &mut |done, total| {
                if let Ok(mut g) = me.phase.lock() {
                    *g = Phase::Downloading { release: rel.clone(), done, total: if total > 0 { total } else { rel.size } };
                }
                me.ctx.request_repaint();
            })?;
            // Checked here already, so a broken download is reported at once; checked again
            // right before starting it.
            verify(&data, &sig, SETUP_KIND, r.version)?;
            let dir = download_dir().ok_or(t("Kein Ordner für Downloads.", "No folder for downloads."))?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let file = dir.join(installer_name(r.version));
            crate::model::write_atomic(&file, &data).map_err(|e| e.to_string())?;
            Ok((file, sig))
        })();
        match result {
            Ok((file, signature)) => {
                crate::log::info(&format!("update: {} downloaded and verified", r.version));
                self.set(Phase::Ready { release: r, file, signature });
            }
            Err(e) => {
                crate::log::warn(&format!("update download failed: {e}"));
                self.set(Phase::Failed(e));
            }
        }
    }
}

fn download_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("Mr. PhotoPrint").join("updates"))
}

/// Installers of the running or older versions are no longer needed.
fn cleanup_old_downloads() {
    let Some(dir) = download_dir() else { return };
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let cur = Version::current();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let v = name.strip_prefix("MrPhotoPrint-Setup-").and_then(|s| s.strip_suffix(".exe")).and_then(Version::parse);
        if v.map(|v| v <= cur).unwrap_or(true) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

// ------------------------------------------------------------------------------- installation

/// How this copy of the program was installed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Install {
    /// Unpacked from the ZIP: updates are only announced.
    Portable,
    /// Installed for this Windows user only: no UAC prompt needed.
    CurrentUser,
    /// Installed for everyone (Program Files): Windows asks for permission (UAC).
    AllUsers,
}

pub fn install_kind() -> Install {
    let Ok(exe) = std::env::current_exe() else { return Install::Portable };
    let Some(dir) = exe.parent() else { return Install::Portable };
    if !dir.join("unins000.exe").exists() {
        return Install::Portable;
    }
    let lower = dir.to_string_lossy().to_lowercase();
    let in_program_files = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .any(|pf| !pf.is_empty() && lower.starts_with(&pf.to_lowercase()));
    if in_program_files { Install::AllUsers } else { Install::CurrentUser }
}

/// Check the downloaded installer once more – opened so that nobody can change it – and start it
/// while it is still held open. `relaunch` starts the program again afterwards.
pub fn install(file: &Path, signature: &str, version: Version, relaunch: bool) -> Result<(), String> {
    let kind = install_kind();
    if kind == Install::Portable {
        return Err(t("Diese Kopie ist die portable Version – bitte die neue Version von GitHub herunterladen.", "This copy is the portable version – please download the new version from GitHub.").into());
    }
    crate::log::info(&format!("update: starting installer for {version} ({kind:?})"));
    sys::install(file, signature, version, kind, relaunch)
}

#[cfg(windows)]
mod sys {
    use super::*;
    use std::io::Read;
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Foundation::{CloseHandle, HWND};
    use windows::Win32::UI::Shell::{SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{HSTRING, PCWSTR, w};

    const FILE_SHARE_READ: u32 = 1;

    pub fn install(file: &Path, signature: &str, version: Version, kind: Install, relaunch: bool) -> Result<(), String> {
        // Others may read the file but not change, replace or delete it while we hold it.
        let mut f = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(file)
            .map_err(|e| format!("{} {e}", t("Installationsdatei nicht lesbar:", "Installer cannot be read:")))?;
        let mut data = Vec::new();
        f.by_ref().take(MAX_INSTALLER as u64 + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
        if data.len() > MAX_INSTALLER {
            return Err(t("Installationsdatei zu groß.", "Installer too large.").into());
        }
        verify(&data, signature, SETUP_KIND, version)?;

        let mut params = String::from("/SILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS ");
        params.push_str(if kind == Install::AllUsers { "/ALLUSERS" } else { "/CURRENTUSER" });
        params.push_str(match crate::i18n::get() {
            crate::i18n::Lang::De => " /LANG=german",
            crate::i18n::Lang::En => " /LANG=english",
        });
        if relaunch {
            params.push_str(" /relaunch=1");
        }
        let file_w = HSTRING::from(file);
        let params_w = HSTRING::from(params.as_str());
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
            hwnd: HWND::default(),
            // "runas" makes Windows show the UAC prompt; a per-user installation needs none.
            lpVerb: if kind == Install::AllUsers { w!("runas") } else { w!("open") },
            lpFile: PCWSTR(file_w.as_ptr()),
            lpParameters: PCWSTR(params_w.as_ptr()),
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        let started = unsafe { ShellExecuteExW(&mut info) };
        // The new process has mapped the file by now; our lock can go.
        drop(f);
        match started {
            Ok(()) => {
                if !info.hProcess.is_invalid() {
                    unsafe {
                        let _ = CloseHandle(info.hProcess);
                    }
                }
                Ok(())
            }
            Err(e) if e.code().0 as u32 == 0x800704C7 => Err(t("Die Installation wurde abgebrochen.", "The installation was cancelled.").into()),
            Err(e) => Err(format!("{} {}", t("Installation konnte nicht gestartet werden:", "Installation could not be started:"), e.message())),
        }
    }
}

#[cfg(not(windows))]
mod sys {
    use super::*;
    pub fn install(_file: &Path, _signature: &str, _version: Version, _kind: Install, _relaunch: bool) -> Result<(), String> {
        Err(t("Updates werden nur unter Windows installiert.", "Updates are only installed on Windows.").into())
    }
}

// ------------------------------------------------------------------------------- HTTPS

pub mod http {
    use super::*;

    /// GET an https URL; at most `max` bytes. `progress(done, total)` is called while reading.
    #[cfg(windows)]
    pub fn get(url: &str, max: usize, progress: &mut dyn FnMut(u64, u64)) -> Result<Vec<u8>, String> {
        use windows::Win32::Networking::WinHttp::*;
        use windows::core::{HSTRING, PCWSTR, w};

        let rest = url.strip_prefix("https://").ok_or("https only")?;
        let (host, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        if host.is_empty() || host.contains(['@', ':', '\\']) {
            return Err("bad host".into());
        }
        let fail = |what: &str| ts(format!("Verbindung fehlgeschlagen ({what})."), format!("Connection failed ({what})."));

        struct H(*mut core::ffi::c_void);
        impl Drop for H {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    unsafe {
                        let _ = WinHttpCloseHandle(self.0);
                    }
                }
            }
        }
        unsafe {
            let agent = HSTRING::from(format!("Mr-PhotoPrint/{}", env!("CARGO_PKG_VERSION")));
            let session = H(WinHttpOpen(&agent, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0));
            if session.0.is_null() {
                return Err(fail("open"));
            }
            // TLS 1.2 or 1.3 only.
            let protocols = (WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2 | WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_3).to_le_bytes();
            if WinHttpSetOption(Some(session.0), WINHTTP_OPTION_SECURE_PROTOCOLS, Some(&protocols)).is_err() {
                let only12 = WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2.to_le_bytes();
                let _ = WinHttpSetOption(Some(session.0), WINHTTP_OPTION_SECURE_PROTOCOLS, Some(&only12));
            }
            let _ = WinHttpSetTimeouts(session.0, 15_000, 15_000, 30_000, 60_000);
            let host_w = HSTRING::from(host);
            let conn = H(WinHttpConnect(session.0, &host_w, INTERNET_DEFAULT_HTTPS_PORT, 0));
            if conn.0.is_null() {
                return Err(fail("connect"));
            }
            let path_w = HSTRING::from(path);
            let req = H(WinHttpOpenRequest(conn.0, w!("GET"), &path_w, PCWSTR::null(), PCWSTR::null(), std::ptr::null(), WINHTTP_FLAG_SECURE));
            if req.0.is_null() {
                return Err(fail("request"));
            }
            let headers: Vec<u16> = "Accept: application/vnd.github+json, application/octet-stream\r\n".encode_utf16().collect();
            WinHttpSendRequest(req.0, Some(&headers), None, 0, 0, 0).map_err(|e| fail(&e.message()))?;
            WinHttpReceiveResponse(req.0, std::ptr::null_mut()).map_err(|e| fail(&e.message()))?;
            let mut status = 0u32;
            let mut len = 4u32;
            WinHttpQueryHeaders(
                req.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some(&mut status as *mut u32 as *mut _),
                &mut len,
                std::ptr::null_mut(),
            )
            .map_err(|e| fail(&e.message()))?;
            if status == 404 {
                return Err(t("Auf GitHub wurde (noch) keine Version gefunden.", "No release found on GitHub (yet).").into());
            }
            if status != 200 {
                return Err(fail(&format!("HTTP {status}")));
            }
            let mut total = 0u64;
            let mut tlen = 8u32;
            let _ = WinHttpQueryHeaders(
                req.0,
                WINHTTP_QUERY_CONTENT_LENGTH | WINHTTP_QUERY_FLAG_NUMBER64,
                PCWSTR::null(),
                Some(&mut total as *mut u64 as *mut _),
                &mut tlen,
                std::ptr::null_mut(),
            );
            if total > max as u64 {
                return Err(t("Die Datei ist größer als erlaubt.", "The file is larger than allowed.").into());
            }
            let mut out: Vec<u8> = Vec::new();
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                let mut read = 0u32;
                WinHttpReadData(req.0, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut read).map_err(|e| fail(&e.message()))?;
                if read == 0 {
                    break;
                }
                if out.len() + read as usize > max {
                    return Err(t("Die Datei ist größer als erlaubt.", "The file is larger than allowed.").into());
                }
                out.extend_from_slice(&buf[..read as usize]);
                progress(out.len() as u64, total);
            }
            if total > 0 && out.len() as u64 != total {
                return Err(fail("incomplete"));
            }
            Ok(out)
        }
    }

    #[cfg(not(windows))]
    pub fn get(_url: &str, _max: usize, _progress: &mut dyn FnMut(u64, u64)) -> Result<Vec<u8>, String> {
        Err(t("Updates gibt es nur unter Windows.", "Updates are only available on Windows.").into())
    }
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
