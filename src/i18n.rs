//! Two languages, German and English. Every text on screen is written as `t("Deutsch", "English")`
//! right where it is used, so both versions are always next to each other and none can go missing.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Lang {
    De,
    En,
}

impl Lang {
    /// The language's own name (shown the same in both languages).
    pub fn name(self) -> &'static str {
        match self {
            Lang::De => "Deutsch",
            Lang::En => "English",
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);
/// Language given on the command line (`--lang de|en`); wins over the settings.
static FORCED: std::sync::OnceLock<Lang> = std::sync::OnceLock::new();

pub fn force(lang: Lang) {
    let _ = FORCED.set(lang);
}

pub fn set(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn get() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == Lang::En as u8 { Lang::En } else { Lang::De }
}

/// Pick the text for the current language.
pub fn t(de: &'static str, en: &'static str) -> &'static str {
    match get() {
        Lang::De => de,
        Lang::En => en,
    }
}

/// Like `t`, for texts built at run time.
pub fn ts(de: String, en: String) -> String {
    match get() {
        Lang::De => de,
        Lang::En => en,
    }
}

/// The language Windows shows its own menus in: German for German, English for everything else.
pub fn system_lang() -> Lang {
    #[cfg(windows)]
    {
        // LANG_GERMAN = 0x07 in the low 10 bits of the language id.
        let id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
        if id & 0x3ff == 0x07 {
            return Lang::De;
        }
        Lang::En
    }
    #[cfg(not(windows))]
    {
        let l = std::env::var("LANG").unwrap_or_default().to_lowercase();
        if l.starts_with("de") { Lang::De } else { Lang::En }
    }
}

/// The language to use: the one chosen in the settings, otherwise the system's.
pub fn resolve(chosen: Option<Lang>) -> Lang {
    if let Some(l) = FORCED.get() {
        return *l;
    }
    chosen.unwrap_or_else(system_lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chosen_language_wins_over_the_system() {
        assert_eq!(resolve(Some(Lang::En)), Lang::En);
        assert_eq!(resolve(Some(Lang::De)), Lang::De);
        let sys = system_lang();
        assert_eq!(resolve(None), sys);
    }

    #[test]
    fn language_names_are_their_own() {
        assert_eq!(Lang::De.name(), "Deutsch");
        assert_eq!(Lang::En.name(), "English");
        assert_eq!(serde_json::to_string(&Lang::En).unwrap(), "\"En\"");
    }
}
