//! The update window, the first-start question and the "Updates" part of the settings.

use crate::app::FotoApp;
use crate::i18n::{t, ts};
use crate::theme::{bold, p};
use crate::update::{self, CheckFreq, Install, Phase, UpdateMode, Version};
use crate::widgets;
use egui::{Align2, RichText, Ui, vec2};

fn seen_id() -> egui::Id {
    egui::Id::NULL.with("update_seen")
}

/// A short key for a phase, so the window opens on its own only once per new state.
fn phase_key(ph: &Phase) -> Option<String> {
    match ph {
        Phase::Available(r) => Some(format!("available {}", r.version)),
        Phase::Ready { release, .. } => Some(format!("ready {}", release.version)),
        _ => None,
    }
}

impl FotoApp {
    pub fn update_ui(&mut self, ctx: &egui::Context) {
        if self.screenshot_mode {
            return;
        }
        if !self.updates.asked && update::key_configured() {
            self.first_start_question(ctx);
            return;
        }
        let phase = self.updater.phase();
        let skipped = |v: Version| self.updates.skipped == v.to_string();
        // Open the window by itself when something new happened (not for skipped versions).
        if let Some(key) = phase_key(&phase) {
            let seen: Option<String> = ctx.data(|d| d.get_temp(seen_id()));
            if seen.as_deref() != Some(key.as_str()) {
                ctx.data_mut(|d| d.insert_temp(seen_id(), key));
                let v = match &phase {
                    Phase::Available(r) => r.version,
                    Phase::Ready { release, .. } => release.version,
                    _ => unreachable!(),
                };
                if !skipped(v) {
                    if matches!(phase, Phase::Ready { .. }) && self.updates.mode == UpdateMode::Auto {
                        self.install_on_exit = true;
                        self.message = ts(
                            format!("Version {v} ist bereit und wird beim Beenden installiert."),
                            format!("Version {v} is ready and will be installed when you close the program."),
                        );
                    } else {
                        self.update_dialog = true;
                    }
                }
            }
        }
        // "Install when closing": start the installer as the window goes away.
        if ctx.input(|i| i.viewport().close_requested()) && self.install_on_exit && self.running.is_none() {
            if let Phase::Ready { release, file, signature } = &phase {
                if let Err(e) = update::install(file, signature, release.version, false) {
                    crate::log::warn(&format!("install on exit: {e}"));
                }
                self.install_on_exit = false;
            }
        }
        if self.update_dialog {
            self.update_window(ctx, phase);
        }
    }

    fn first_start_question(&mut self, ctx: &egui::Context) {
        let c = p();
        egui::Window::new(t("Updates", "Updates"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label(RichText::new(t("Nach neuen Versionen suchen?", "Look for new versions?")).font(bold(17.0)));
                ui.add_space(6.0);
                ui.label(t(
                    "Mr. PhotoPrint kann bei GitHub nachsehen, ob es eine neue Version gibt, und sie auf Wunsch installieren. Dabei wird nur die Versionsliste des Projekts abgerufen – es werden keine Daten über Sie, Ihren PC oder Ihre Fotos gesendet. Jedes Update ist digital signiert und wird vor der Installation geprüft.",
                    "Mr. PhotoPrint can check GitHub for a new version and install it if you like. Only the project's list of versions is fetched – no data about you, your PC or your photos is sent. Every update is digitally signed and checked before it is installed.",
                ));
                ui.add_space(4.0);
                ui.label(RichText::new(t("Sie können das jederzeit unter Einstellungen → Programm ändern.", "You can change this at any time under Settings → Program.")).size(12.5).color(c.dim));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if widgets::primary(ui, t("Ja, wöchentlich prüfen", "Yes, check weekly"), vec2(210.0, 40.0), true).clicked() {
                        self.updates.asked = true;
                        self.updates.check = CheckFreq::Weekly;
                        self.updates.last_check = update::now_secs();
                        self.updater.check(self.updates.mode != UpdateMode::Notify);
                        self.save_settings();
                    }
                    if widgets::secondary(ui, t("Nein, danke", "No, thanks"), vec2(140.0, 40.0)).clicked() {
                        self.updates.asked = true;
                        self.updates.check = CheckFreq::Never;
                        self.save_settings();
                    }
                });
            });
    }

    fn update_window(&mut self, ctx: &egui::Context, phase: Phase) {
        let c = p();
        let mut open = true;
        let current = Version::current();
        let kind = update::install_kind();
        egui::Window::new(t("Update", "Update"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.set_width(500.0);
                match &phase {
                    Phase::Idle | Phase::Checking => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(t("Suche nach neuen Versionen …", "Looking for new versions …"));
                        });
                    }
                    Phase::UpToDate => {
                        ui.label(RichText::new(ts(format!("Sie haben die neueste Version ({current})."), format!("You have the latest version ({current})."))).font(bold(16.0)));
                        ui.add_space(8.0);
                        if ui.button("OK").clicked() {
                            self.update_dialog = false;
                        }
                    }
                    Phase::Failed(e) => {
                        ui.label(RichText::new(t("Die Suche nach Updates hat nicht geklappt.", "Looking for updates did not work.")).font(bold(16.0)));
                        ui.label(RichText::new(e).color(c.bad));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button(t("Nochmal versuchen", "Try again")).clicked() {
                                self.updater.check(false);
                            }
                            if ui.button(t("Schließen", "Close")).clicked() {
                                self.update_dialog = false;
                            }
                        });
                    }
                    Phase::Available(r) => {
                        ui.label(RichText::new(ts(format!("Version {} ist verfügbar", r.version), format!("Version {} is available", r.version))).font(bold(18.0)));
                        ui.label(RichText::new(ts(format!("Sie haben {current}."), format!("You have {current}."))).color(c.dim));
                        notes(ui, &r.notes);
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if kind == Install::Portable {
                                if widgets::primary(ui, t("Download-Seite öffnen", "Open download page"), vec2(220.0, 40.0), true).clicked() {
                                    ui.ctx().open_url(egui::OpenUrl::new_tab(&r.page));
                                    self.update_dialog = false;
                                }
                            } else if widgets::primary(ui, t("Herunterladen", "Download"), vec2(170.0, 40.0), true).clicked() {
                                self.updater.download(r.clone());
                            }
                            if widgets::secondary(ui, t("Später", "Later"), vec2(100.0, 40.0)).clicked() {
                                self.update_dialog = false;
                            }
                            if widgets::secondary(ui, t("Überspringen", "Skip"), vec2(120.0, 40.0)).on_hover_text(t("Diese Version nicht mehr anbieten", "Don't offer this version again")).clicked() {
                                self.updates.skipped = r.version.to_string();
                                self.save_settings();
                                self.update_dialog = false;
                            }
                        });
                        if kind == Install::Portable {
                            ui.label(RichText::new(t("Dies ist die portable Version – sie wird nicht automatisch ersetzt.", "This is the portable version – it is not replaced automatically.")).size(12.5).color(c.dim));
                        }
                    }
                    Phase::Downloading { release, done, total } => {
                        ui.label(RichText::new(ts(format!("Version {} wird geladen …", release.version), format!("Downloading version {} …", release.version))).font(bold(16.0)));
                        let frac = if *total > 0 { (*done as f32 / *total as f32).clamp(0.0, 1.0) } else { 0.0 };
                        ui.add(egui::ProgressBar::new(frac).show_percentage());
                        ui.label(RichText::new(format!("{:.1} / {:.1} MB", *done as f64 / 1e6, *total as f64 / 1e6)).size(12.5).color(c.dim));
                    }
                    Phase::Ready { release, file, signature } => {
                        ui.label(RichText::new(ts(format!("Version {} ist bereit", release.version), format!("Version {} is ready", release.version))).font(bold(18.0)));
                        ui.label(RichText::new(t("Heruntergeladen und Signatur geprüft ✔", "Downloaded and signature checked ✔")).color(c.ok));
                        notes(ui, &release.notes);
                        if kind == Install::AllUsers {
                            ui.label(RichText::new(t("Windows fragt gleich nach der Erlaubnis (Benutzerkontensteuerung), weil das Programm für alle Benutzer installiert ist.", "Windows will ask for permission (User Account Control) because the program is installed for all users.")).size(12.5).color(c.dim));
                        }
                        ui.add_space(8.0);
                        let busy = self.running.is_some();
                        ui.horizontal(|ui| {
                            if widgets::primary(ui, t("Jetzt installieren und neu starten", "Install now and restart"), vec2(280.0, 40.0), !busy).clicked() {
                                match update::install(file, signature, release.version, true) {
                                    Ok(()) => {
                                        self.save_settings();
                                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                                    }
                                    Err(e) => {
                                        crate::log::warn(&format!("install: {e}"));
                                        self.message = e;
                                    }
                                }
                                self.update_dialog = false;
                            }
                            if widgets::secondary(ui, t("Beim Beenden", "When closing"), vec2(140.0, 40.0)).on_hover_text(t("Installiert das Update, wenn Sie das Programm schließen", "Installs the update when you close the program")).clicked() {
                                self.install_on_exit = true;
                                self.update_dialog = false;
                            }
                        });
                        if busy {
                            ui.label(RichText::new(t("Bitte warten, bis der Druckauftrag fertig ist.", "Please wait until the print job has finished.")).size(12.5).color(c.warn));
                        }
                    }
                }
            });
        if !open {
            self.update_dialog = false;
        }
    }

    /// The "Updates" part of Settings → Program.
    pub fn update_settings(&mut self, ui: &mut Ui) {
        let c = p();
        let current = Version::current();
        ui.label(RichText::new(ts(format!("Installierte Version: {current}"), format!("Installed version: {current}"))).color(c.text));
        let kind_text = match update::install_kind() {
            Install::Portable => t("Portable Version (ohne Installation)", "Portable version (not installed)"),
            Install::CurrentUser => t("Installiert für diesen Benutzer", "Installed for this user"),
            Install::AllUsers => t("Installiert für alle Benutzer", "Installed for all users"),
        };
        ui.label(RichText::new(kind_text).size(12.5).color(c.dim));
        ui.add_space(6.0);
        let enabled = update::key_configured();
        if !enabled {
            ui.label(RichText::new(t(
                "Diese Version wurde ohne Update-Schlüssel gebaut – automatische Updates sind abgeschaltet. Versionen von der offiziellen Download-Seite haben ihn.",
                "This build has no update key – automatic updates are switched off. Versions from the official download page have it.",
            ))
            .color(c.warn));
        }
        ui.add_enabled_ui(enabled, |ui| self.update_controls(ui));
        ui.label(
            RichText::new(t(
                "Updates kommen nur von der offiziellen GitHub-Seite und werden vor der Installation anhand ihrer digitalen Signatur geprüft. Es werden keine Daten über Sie gesendet.",
                "Updates come only from the official GitHub page and are checked by their digital signature before they are installed. No data about you is sent.",
            ))
            .size(12.5)
            .color(c.dim),
        );
    }

    fn update_controls(&mut self, ui: &mut Ui) {
        let c = p();
        let mut changed = false;
        egui::Grid::new("updates").num_columns(2).spacing([14.0, 10.0]).show(ui, |ui| {
            ui.label(t("Nach Updates suchen:", "Check for updates:"));
            egui::ComboBox::from_id_salt("upd_freq").width(260.0).selected_text(self.updates.check.label()).show_ui(ui, |ui| {
                for f in CheckFreq::ALL {
                    changed |= ui.selectable_value(&mut self.updates.check, f, f.label()).changed();
                }
            });
            ui.end_row();
            ui.label(t("Wenn eine neue Version da ist:", "When a new version is out:"));
            egui::ComboBox::from_id_salt("upd_mode").width(260.0).selected_text(self.updates.mode.label()).show_ui(ui, |ui| {
                for m in UpdateMode::ALL {
                    changed |= ui.selectable_value(&mut self.updates.mode, m, m.label()).changed();
                }
            });
            ui.end_row();
        });
        if changed {
            self.updates.asked = true;
            self.install_on_exit = self.install_on_exit && self.updates.mode == UpdateMode::Auto;
            self.save_settings();
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let busy = self.updater.busy();
            if ui.add_enabled(!busy, egui::Button::new(t("Jetzt suchen", "Check now"))).clicked() {
                self.updates.last_check = update::now_secs();
                self.updates.skipped.clear();
                self.save_settings();
                ui.ctx().data_mut(|d| d.remove::<String>(seen_id()));
                self.updater.check(false);
                self.update_dialog = true;
            }
            let status = match self.updater.phase() {
                Phase::Idle => String::new(),
                Phase::Checking => t("Suche …", "Checking …").into(),
                Phase::UpToDate => t("Aktuell.", "Up to date.").into(),
                Phase::Available(r) => ts(format!("Version {} verfügbar.", r.version), format!("Version {} available.", r.version)),
                Phase::Downloading { .. } => t("Lädt …", "Downloading …").into(),
                Phase::Ready { release, .. } => ts(format!("Version {} bereit.", release.version), format!("Version {} ready.", release.version)),
                Phase::Failed(e) => e,
            };
            ui.label(RichText::new(status).color(c.dim));
        });
        if self.updates.last_check > 0 {
            ui.label(
                RichText::new(format!("{} {} UTC", t("Zuletzt gesucht:", "Last checked:"), crate::app::format_iso(self.updates.last_check)))
                    .size(12.0)
                    .color(c.dim),
            );
        }
    }
}

fn notes(ui: &mut Ui, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    ui.add_space(6.0);
    ui.label(RichText::new(t("Was ist neu:", "What's new:")).font(bold(14.0)));
    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
        // Shown as plain text: nothing in the notes is clickable or executed.
        ui.label(RichText::new(text).size(13.0));
    });
}
