//! A small local log file for problem reports. It stays on this PC – nothing is ever sent.
//! Lives in %LOCALAPPDATA%\Mr. PhotoPrint\mrphotoprint.log; at 1 MB the old log is kept as .old.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());
const MAX_BYTES: u64 = 1024 * 1024;

pub fn dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("Mr. PhotoPrint"))
}

pub fn path() -> Option<PathBuf> {
    dir().map(|d| d.join("mrphotoprint.log"))
}

fn write(level: &str, msg: &str) {
    if cfg!(test) {
        return;
    }
    let Some(p) = path() else { return };
    let _g = LOCK.lock();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if std::fs::metadata(&p).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&p, p.with_extension("log.old"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = writeln!(f, "{} {level} {}", stamp(), one_line(msg));
    }
}

/// Keep each entry on one line, so a crafted file name can't fake log entries.
fn one_line(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).take(2000).collect()
}

fn stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{} UTC", crate::app::format_iso(secs))
}

pub fn info(msg: &str) {
    write("INFO ", msg);
}

pub fn warn(msg: &str) {
    write("WARN ", msg);
}

pub fn error(msg: &str) {
    write("ERROR", msg);
}

/// Record crashes in the log before the program ends.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        error(&format!("panic in thread '{thread}': {info}"));
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    #[test]
    fn entries_stay_on_one_line() {
        assert_eq!(super::one_line("a\nb\r\tc"), "a b  c");
        assert_eq!(super::one_line(&"x".repeat(5000)).len(), 2000);
    }
}
