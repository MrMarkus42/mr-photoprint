//! The settings dialog (its own window) and the sheet preview used everywhere: a navigation
//! column, sections, a live sheet preview with a summary, export / import / reset, OK / Cancel.

use crate::app::*;
use crate::i18n::{Lang, t, ts};
use crate::icons;
use crate::layout::{self, Sheet};
use crate::model::*;
use crate::printer::{EXPORT_PRINTER, printer_label};
use crate::theme::{self, bold, p};
use crate::widgets;
use egui::{Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    let c = p();
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(c.surface2.lerp_to_gamma(c.surface, 0.45))
        .stroke(Stroke::new(1.0, c.border))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).font(bold(15.0)).color(c.text));
            ui.add_space(4.0);
            add(ui);
        });
    ui.add_space(8.0);
}

impl FotoApp {
    pub fn settings_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.draft.take() else { return };
        let mut close = false;
        let title = match &draft.target {
            Target::Global => format!("Mr. PhotoPrint – {}", t("Einstellungen", "Settings")),
            Target::Items(ix) if ix.len() == 1 => {
                let name = self.items.get(ix[0]).map(|i| i.name.as_str()).unwrap_or("");
                ts(format!("Einstellungen für „{name}“"), format!("Settings for \"{name}\""))
            }
            Target::Items(ix) => ts(format!("Einstellungen für {} ausgewählte Fotos", ix.len()), format!("Settings for {} selected photos", ix.len())),
        };
        let builder = egui::ViewportBuilder::default()
            .with_title(title.clone())
            .with_inner_size([1060.0, 720.0])
            .with_min_inner_size([860.0, 580.0])
            .with_icon(std::sync::Arc::new(icons::window_icon()));
        let c = ctx.clone();
        c.show_viewport_immediate(egui::ViewportId::from_hash_of("settings"), builder, |ui, _class| {
            if ui.ctx().input(|i| i.viewport().close_requested()) {
                close = true;
            }
            self.settings_contents(ui, &mut draft, &mut close, &title);
        });
        if !close {
            self.draft = Some(draft);
        }
    }

    fn settings_contents(&mut self, ui: &mut Ui, d: &mut Draft, close: &mut bool, title: &str) {
        let c = p();
        let global = matches!(d.target, Target::Global);
        egui::Panel::left("nav")
            .exact_size(232.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(c.surface).inner_margin(egui::Margin::same(16)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
                    theme::gradient_rect(ui.painter(), r, 10.0, c.accent, c.accent2);
                    icons::sliders(ui.painter(), r.shrink(7.0), Color32::WHITE);
                    ui.label(RichText::new(t("Einstellungen", "Settings")).font(bold(18.0)));
                });
                ui.add_space(14.0);
                let mut tabs = vec![(Tab::General, t("Allgemein", "General")), (Tab::Layout, t("Seitenlayout", "Page layout"))];
                if global {
                    tabs.push((Tab::Collage, t("Mehrere Fotos pro Blatt", "Several photos per sheet")));
                    tabs.push((Tab::Output, t("Ränder und Ausgabe", "Margins and output")));
                    tabs.push((Tab::Program, t("Programm", "Program")));
                }
                tabs.push((Tab::Help, t("Hilfe", "Help")));
                tabs.push((Tab::About, t("Über", "About")));
                for (t, label) in tabs {
                    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
                    if d.tab == t {
                        theme::gradient_rect(ui.painter(), r, 10.0, c.accent, c.accent2);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, CornerRadius::same(10), c.surface2);
                    }
                    let col = if d.tab == t { Color32::WHITE } else { c.text };
                    ui.painter().text(r.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, label, if d.tab == t { bold(14.5) } else { FontId::proportional(14.5) }, col);
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        d.tab = t;
                    }
                    ui.add_space(4.0);
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    let small = |ui: &mut Ui, t: &str| ui.add(egui::Button::new(RichText::new(t).size(13.5)).min_size(vec2(ui.available_width(), 32.0)));
                    if small(ui, t("Auf Standard zurücksetzen", "Reset to defaults")).clicked() {
                        d.photo = PhotoSettings::default();
                        if global {
                            let printer = d.job.printer.clone();
                            d.job = JobSettings { printer, ..Default::default() };
                        }
                    }
                    if small(ui, t("Importieren …", "Import …")).on_hover_text(t("Einstellungen aus einer Datei laden", "Load settings from a file")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter(t("Einstellungen", "Settings"), &["json"]).pick_file() {
                            let ok = std::fs::metadata(&path).map(|m| m.len() < crate::model::MAX_SETTINGS_BYTES as u64).unwrap_or(false);
                            if let Some(j) = ok.then(|| std::fs::read_to_string(&path).ok()).flatten().and_then(|s| JobSettings::from_untrusted_json(&s)) {
                                d.photo = j.photo.clone();
                                if global {
                                    let printer = d.job.printer.clone();
                                    d.job = JobSettings { printer, ..j };
                                }
                            }
                        }
                    }
                    if small(ui, t("Exportieren …", "Export …")).on_hover_text(t("Einstellungen in eine Datei speichern", "Save settings to a file")).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter(t("Einstellungen", "Settings"), &["json"])
                            .set_file_name(t("Mr-PhotoPrint-Einstellungen.json", "Mr-PhotoPrint-settings.json")).save_file() {
                            let mut j = d.job.clone();
                            j.photo = d.photo.clone();
                            if let Ok(s) = serde_json::to_string_pretty(&j) {
                                let _ = std::fs::write(path, s);
                            }
                        }
                    }
                });
            });
        egui::Panel::bottom("buttons")
            .frame(egui::Frame::new().fill(c.bg).inner_margin(egui::Margin::symmetric(20, 14)))
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::primary(ui, "OK", vec2(150.0, 42.0), true).clicked() {
                        self.apply(d);
                        *close = true;
                    }
                    if widgets::secondary(ui, t("Abbrechen", "Cancel"), vec2(130.0, 42.0)).clicked() {
                        *close = true;
                    }
                    if !global {
                        ui.add_space(10.0);
                        if widgets::secondary(ui, t("Eigene Einstellungen entfernen", "Remove own settings"), vec2(240.0, 42.0)).clicked() {
                            if let Target::Items(ix) = &d.target {
                                for &i in ix {
                                    if let Some(it) = self.items.get_mut(i) {
                                        it.own = None;
                                        it.fine = None;
                                    }
                                }
                            }
                            *close = true;
                        }
                    }
                });
            });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(c.bg).inner_margin(egui::Margin { left: 20, right: 20, top: 18, bottom: 0 })).show(ui, |ui| {
            // Program, help and about have nothing to preview: they use the whole width.
            let with_preview = !matches!(d.tab, Tab::Program | Tab::Help | Tab::About);
            let heading = match (global, d.tab) {
                (true, Tab::General) => t("Allgemein", "General").to_string(),
                (true, Tab::Layout) => t("Seitenlayout", "Page layout").to_string(),
                (true, Tab::Collage) => t("Mehrere Fotos pro Blatt", "Several photos per sheet").to_string(),
                (true, Tab::Output) => t("Ränder und Ausgabe", "Margins and output").to_string(),
                (_, Tab::Program) => t("Programm", "Program").to_string(),
                (_, Tab::Help) => t("Hilfe", "Help").to_string(),
                (_, Tab::About) => t("Über Mr. PhotoPrint", "About Mr. PhotoPrint").to_string(),
                _ => title.to_string(),
            };
            ui.label(RichText::new(heading).font(bold(20.0)));
            ui.add_space(6.0);
            let body = ui.available_rect_before_wrap();
            let left_w = if with_preview { (body.width() * 0.54).clamp(380.0, 540.0) } else { body.width().min(760.0) };
            let left = Rect::from_min_size(body.min, vec2(left_w, body.height()));
            let right = Rect::from_min_max(pos2(left.right() + 18.0, body.top()), body.max);
            ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
                // Nothing in the settings column may ever reach into the preview next to it.
                ui.set_clip_rect(left.intersect(ui.clip_rect()));
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.set_width(left_w - 14.0);
                    match d.tab {
                        Tab::General => self.tab_general(ui, d, global),
                        Tab::Layout => self.tab_layout(ui, d),
                        Tab::Collage => self.tab_collage(ui, d),
                        Tab::Output => self.tab_output(ui, d),
                        Tab::Program => self.tab_program(ui),
                        Tab::Help => self.help_text(ui),
                        Tab::About => self.tab_about(ui),
                    }
                });
            });
            ui.allocate_rect(body, Sense::hover());
            if with_preview {
                self.draft_preview(ui, right, d);
            }
        });
    }

    fn help_text(&self, ui: &mut Ui) {
        let c = p();
        let entries = |ui: &mut Ui, list: &[(&str, &str)]| {
            for (head, body) in list {
                ui.add_space(4.0);
                ui.label(RichText::new(*head).font(bold(14.0)).color(c.text));
                ui.label(RichText::new(*body).color(c.dim));
            }
        };
        section(ui, t("In vier Schritten zum Foto", "Four steps to your prints"), |ui| {
            entries(ui, &[
                (t("1. Fotos hinzufügen", "1. Add photos"), t("Fotos oder ganze Ordner ins Fenster ziehen – oder „Fotos hinzufügen“ bzw. „Ordner hinzufügen“ wählen.", "Drag photos or whole folders into the window – or use \"Add photos\" or \"Add folder\".")),
                (t("2. Drucker und Papier wählen", "2. Choose printer and paper"), t("Oben den Drucker wählen, rechts eine Vorlage (z. B. „10×15 randlos“) oder Papier und „Foto einpassen“ einstellen.", "Choose the printer at the top, then a preset on the right (e.g. \"10×15 borderless\") or the paper and how the photo is fitted.")),
                (t("3. Vorschau prüfen", "3. Check the preview"), t("Die Vorschau zeigt jedes Blatt. Den Bildausschnitt verschieben Sie mit der Maus; Doppelklick setzt ihn zurück.", "The preview shows every sheet. Drag a photo to move the crop; double-click resets it.")),
                (t("4. Drucken", "4. Print"), t("„Drucken“ klicken. Zum Ausprobieren ohne Papier den Drucker „In JPEG-Dateien speichern“ wählen.", "Click \"Print\". To try it without paper, choose the printer \"Save as JPEG files\".")),
            ]);
        });
        section(ui, t("So passen Sie Fotos an das Papier an", "How photos are fitted to the paper"), |ui| {
            entries(ui, &[
                (ScaleMode::Fill.short(), t("Das Foto bedeckt das ganze Blatt bzw. die ganze Zelle; was übersteht, wird abgeschnitten.", "The photo covers the whole sheet or cell; whatever sticks out is cut off.")),
                (ScaleMode::Fit.short(), t("Das ganze Foto ist zu sehen; an zwei Seiten bleibt eventuell ein weißer Rand.", "The whole photo is visible; there may be a white border on two sides.")),
                (ScaleMode::Shrink.short(), t("Wie Anpassen, aber kleine Fotos werden nicht vergrößert.", "Like Fit, but small photos are not enlarged.")),
                (t("Original-Größe / Zoomstufe", "Actual size / zoom level"), t("Das Foto wird in seiner gespeicherten Größe bzw. mit festem Prozentsatz gedruckt.", "The photo is printed at its stored size or at a fixed percentage.")),
                (t("Automatisch drehen", "Turn automatically"), t("Liegt ein Querformat-Foto auf Hochformat-Papier, wird es um 90° gedreht.", "A landscape photo on portrait paper is turned by 90°.")),
                (t("Papier „Auto“", "Paper \"Auto\""), t("Das Papier wird für jedes Foto hoch oder quer gelegt.", "The paper is turned to portrait or landscape for each photo.")),
                (t("Mehrere Fotos pro Blatt", "Several photos per sheet"), t("Teilt das Blatt in Spalten und Zeilen, z. B. 2 × 2 für vier Fotos auf A4.", "Splits the sheet into columns and rows, e.g. 2 × 2 for four photos on A4.")),
                (t("Eigene Einstellungen", "Own settings"), t("Fotos markieren → „Eigene Einstellungen“ oder Doppelklick auf ein Foto. Diese Fotos bekommen ein eigenes Papier, eine eigene Drehung usw.", "Select photos → \"Own settings\", or double-click a photo. These photos get their own paper, rotation and so on.")),
                (t("„unscharf?“", "\"blurry?\""), t("Das Foto hat auf dem Papier weniger als 150 Pixel pro Zoll. Kleineres Papier oder „Anpassen“ hilft.", "The photo has fewer than 150 pixels per inch on paper. Smaller paper or \"Fit\" helps.")),
            ]);
        });
        section(ui, t("Tastatur und Maus", "Keyboard and mouse"), |ui| {
            ui.vertical(|ui| {
                for (k, what) in [
                    ("F1", t("Diese Hilfe", "This help")),
                    (t("Strg + A", "Ctrl + A"), t("Alle Fotos auswählen", "Select all photos")),
                    (t("Entf", "Del"), t("Ausgewählte Fotos aus der Liste entfernen", "Remove the selected photos from the list")),
                    ("Esc", t("Auswahl aufheben", "Clear the selection")),
                    (t("Strg / Umschalt + Klick", "Ctrl / Shift + click"), t("Mehrere Fotos auswählen", "Select several photos")),
                    (t("Doppelklick auf ein Foto", "Double-click a photo"), t("Eigene Einstellungen für dieses Foto", "Own settings for this photo")),
                    (t("Rechtsklick auf ein Foto", "Right-click a photo"), t("Drehen, verschieben, Ordner öffnen …", "Turn, move, open folder …")),
                    (t("Klick auf den Kreis", "Click the circle"), t("Foto zum Drucken an- oder abwählen", "Include or exclude a photo")),
                ] {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(k).font(bold(13.5)));
                        ui.label(RichText::new(format!("– {what}")).color(c.dim));
                    });
                }
            });
        });
        section(ui, t("Dateiformate", "File formats"), |ui| {
            ui.label(RichText::new(t(
                "JPEG, PNG, TIFF, WebP, BMP, GIF, ICO, TGA, PNM und QOI öffnet Mr. PhotoPrint selbst. HEIC/HEIF (iPhone), AVIF, JPEG XL und Kamera-RAW werden mit den Erweiterungen von Windows geöffnet – falls nötig, die kostenlosen „HEIF-Bilderweiterungen“, „HEVC-Videoerweiterungen“, „AV1 Video Extension“ bzw. „Raw Image Extension“ aus dem Microsoft Store installieren.",
                "Mr. PhotoPrint opens JPEG, PNG, TIFF, WebP, BMP, GIF, ICO, TGA, PNM and QOI itself. HEIC/HEIF (iPhone), AVIF, JPEG XL and camera RAW are opened with Windows' own extensions – if needed, install the free \"HEIF Image Extensions\", \"HEVC Video Extensions\", \"AV1 Video Extension\" or \"Raw Image Extension\" from the Microsoft Store.",
            )).color(c.dim));
        });
        section(ui, t("Sicherheit und Datenschutz", "Security and privacy"), |ui| {
            ui.label(RichText::new(t(
                "Jede Datei wird vor dem Öffnen geprüft: Das Format wird am Inhalt erkannt (nicht an der Endung), unplausibel große oder beschädigte Bilder werden abgelehnt, und ein fehlerhaftes Bild kann das Programm nicht zum Absturz bringen. Aus geladenen Listen werden keine Netzwerkpfade geöffnet.",
                "Every file is checked before it is opened: the format is recognised by its content (not its name), implausibly large or damaged pictures are refused, and a broken picture cannot crash the program. Network paths from loaded lists are never opened.",
            )).color(c.dim));
            ui.add_space(4.0);
            ui.label(RichText::new(t(
                "Mr. PhotoPrint sendet keine Daten. Nur wenn Sie Updates erlauben, fragt es bei GitHub nach neuen Versionen. Updates sind digital signiert und werden vor der Installation geprüft.",
                "Mr. PhotoPrint sends no data. Only if you allow updates does it ask GitHub for new versions. Updates are digitally signed and checked before they are installed.",
            )).color(c.dim));
        });
        section(ui, t("Hilfe im Internet", "Help online"), |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.hyperlink_to(t("Ausführliche Anleitung", "Full user guide"), user_guide_url());
                ui.label("·");
                ui.hyperlink_to(t("Häufige Fragen", "FAQ"), format!("{}#{}", user_guide_url(), t("häufige-fragen", "faq")));
                ui.label("·");
                ui.hyperlink_to(t("Problem melden", "Report a problem"), report_url());
            });
        });
    }

    fn tab_program(&mut self, ui: &mut Ui) {
        let c = p();
        section(ui, "Sprache · Language", |ui| {
            let mut lang = self.lang;
            if widgets::segmented(ui, &mut lang, &[(None, "Windows"), (Some(Lang::De), Lang::De.name()), (Some(Lang::En), Lang::En.name())], 34.0) {
                self.lang = lang;
                crate::i18n::set(crate::i18n::resolve(lang));
                self.save_settings();
            }
            ui.label(RichText::new(t("„Windows“ folgt der Sprache von Windows.", "\"Windows\" follows the language of Windows.")).size(12.5).color(c.dim));
        });
        section(ui, t("Darstellung", "Appearance"), |ui| {
            let mut dark = self.dark;
            if widgets::segmented(ui, &mut dark, &[(true, t("Dunkel", "Dark")), (false, t("Hell", "Light"))], 34.0) {
                self.dark = dark;
                theme::set_dark(ui.ctx(), dark);
                self.save_settings();
            }
        });
        section(ui, "Updates", |ui| self.update_settings(ui));
        section(ui, t("Probleme", "Problems"), |ui| {
            ui.label(RichText::new(t(
                "Mr. PhotoPrint schreibt Fehler in eine Protokolldatei auf diesem PC. Sie wird nie automatisch verschickt – bei einer Problemmeldung können Sie sie selbst anhängen.",
                "Mr. PhotoPrint writes errors to a log file on this PC. It is never sent automatically – you can attach it yourself when reporting a problem.",
            )).color(c.dim));
            ui.horizontal(|ui| {
                if ui.button(t("Protokoll-Ordner öffnen", "Open log folder")).clicked() {
                    if let Some(d) = crate::log::dir() {
                        let _ = std::fs::create_dir_all(&d);
                        open_path(&d);
                    }
                }
                if ui.button(t("Problem melden …", "Report a problem …")).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(report_url()));
                }
            });
        });
    }

    fn tab_about(&mut self, ui: &mut Ui) {
        let c = p();
        section(ui, "Mr. PhotoPrint", |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
                ui.painter().image(self.logo.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("{} {}", t("Version", "Version"), env!("CARGO_PKG_VERSION"))).font(bold(16.0)));
                    ui.label(RichText::new(t("Viele Fotos – perfekt aufs Papier.", "Many photos – perfectly on paper.")).color(c.dim));
                });
            });
            ui.add_space(6.0);
            ui.label(t(
                "Freie Software: Sie dürfen das Programm nutzen, weitergeben und verändern – nach Wahl unter der MIT- oder der Apache-2.0-Lizenz.",
                "Free software: you may use, share and change it – under the MIT or the Apache 2.0 licence, at your choice.",
            ));
            ui.label(RichText::new(t("Ohne Gewähr. Keine Werbung, keine Telemetrie.", "No warranty. No ads, no telemetry.")).color(c.dim));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.hyperlink_to(t("Projektseite auf GitHub", "Project page on GitHub"), crate::update::REPO_URL);
                ui.label("·");
                ui.hyperlink_to(t("Änderungen", "Changelog"), format!("{}/blob/main/CHANGELOG.md", crate::update::REPO_URL));
                ui.label("·");
                ui.hyperlink_to(t("Lizenz", "Licence"), format!("{}/blob/main/{}", crate::update::REPO_URL, t("README.de.md#lizenz", "README.md#license")));
            });
        });
        section(ui, t("Verwendete Bausteine", "Components used"), |ui| {
            ui.label(RichText::new(t(
                "Mr. PhotoPrint ist in Rust geschrieben und nutzt freie Bibliotheken, u. a. egui/eframe, wgpu, image, serde, rfd, windows-rs und minisign-verify, sowie die mitgelieferten Schriften von egui. Ihre Lizenztexte stehen in THIRD_PARTY_LICENSES.html.",
                "Mr. PhotoPrint is written in Rust and uses free libraries, among them egui/eframe, wgpu, image, serde, rfd, windows-rs and minisign-verify, plus the fonts that come with egui. Their licence texts are in THIRD_PARTY_LICENSES.html.",
            )).color(c.dim));
            if ui.button(t("Lizenzen der Bausteine anzeigen", "Show component licences")).clicked() {
                let local = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("THIRD_PARTY_LICENSES.html"))).filter(|p| p.is_file());
                match local {
                    Some(p) => open_path(&p),
                    None => ui.ctx().open_url(egui::OpenUrl::new_tab(format!("{}/releases/latest", crate::update::REPO_URL))),
                }
            }
        });
    }

    fn tab_general(&mut self, ui: &mut Ui, d: &mut Draft, global: bool) {
        let printer = self.job.printer.clone();
        let caps = self.printers.caps(&printer).clone();
        let paper_name = self.printers.paper_name(&printer, d.photo.paper);
        let ph = &mut d.photo;
        section(ui, t("Papier", "Paper"), |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().combo_width = ui.available_width();
                ui.label(t("Papierformat:", "Paper size:"));
                egui::ComboBox::from_id_salt("paper").width(ui.available_width()).selected_text(paper_name).show_ui(ui, |ui| {
                    ui.selectable_value(&mut ph.paper, None, t("Wie beim Drucker", "As set in the printer"));
                    for pp in &caps.papers {
                        ui.selectable_value(&mut ph.paper, Some(pp.id), format!("{}  ·  {:.0} × {:.0} mm", pp.name, pp.w_mm, pp.h_mm));
                    }
                });
                ui.add_space(6.0);
                if !caps.trays.is_empty() {
                    ui.label(t("Papierquelle:", "Paper source:"));
                    let cur = ph.tray.and_then(|t| caps.trays.iter().find(|x| x.id == t)).map(|t| t.name.clone()).unwrap_or(t("Wie beim Drucker", "As set in the printer").into());
                    egui::ComboBox::from_id_salt("tray").width(ui.available_width()).selected_text(cur).show_ui(ui, |ui| {
                        ui.selectable_value(&mut ph.tray, None, t("Wie beim Drucker", "As set in the printer"));
                        for t in &caps.trays {
                            ui.selectable_value(&mut ph.tray, Some(t.id), &t.name);
                        }
                    });
                    ui.add_space(6.0);
                }
                if !caps.media.is_empty() {
                    ui.label(t("Papierart:", "Paper type:")).on_hover_text(media_tip());
                    egui::ComboBox::from_id_salt("media").width(ui.available_width()).selected_text(media_name(&caps, ph.media)).show_ui(ui, |ui| {
                        ui.selectable_value(&mut ph.media, None, t("Wie beim Drucker", "As set in the printer"));
                        for m in &caps.media {
                            ui.selectable_value(&mut ph.media, Some(m.id), &m.name);
                        }
                    });
                    ui.add_space(6.0);
                }
                ui.label(t("Seitenausrichtung:", "Orientation:"));
                egui::ComboBox::from_id_salt("orient").width(ui.available_width()).selected_text(ph.orientation.label()).show_ui(ui, |ui| {
                    for o in Orientation::ALL {
                        ui.selectable_value(&mut ph.orientation, o, o.label());
                    }
                });
                ui.add_space(6.0);
                ui.label(t("Farbmodus:", "Colour:"));
                egui::ComboBox::from_id_salt("color").width(ui.available_width()).selected_text(ph.color.label()).show_ui(ui, |ui| {
                    for col in [ColorMode::AsPrinter, ColorMode::Color, ColorMode::Gray] {
                        ui.selectable_value(&mut ph.color, col, col.label());
                    }
                });
                ui.add_space(6.0);
            });
        });
        section(ui, t("Kopien", "Copies"), |ui| {
            ui.horizontal(|ui| {
                ui.label(t("Kopien je Foto:", "Copies per photo:"));
                ui.add(egui::DragValue::new(&mut ph.copies).range(1..=999));
            });
        });
        if global {
            let export = printer == EXPORT_PRINTER;
            section(ui, if export { t("Qualität der Bilddateien", "Quality of the picture files") } else { t("Druckqualität", "Print quality") }, |ui| {
                let j = &mut d.job;
                ui.vertical(|ui| {
                    ui.label(t("Qualität:", "Quality:"));
                    egui::ComboBox::from_id_salt("quality").width(ui.available_width()).selected_text(quality_label(j.quality)).show_ui(ui, |ui| {
                        for q in [QUALITY_AS_PRINTER, QUALITY_DRAFT, QUALITY_LOW, QUALITY_MEDIUM, QUALITY_HIGH] {
                            ui.selectable_value(&mut j.quality, q, quality_label(q));
                        }
                        if !caps.resolutions.is_empty() {
                            ui.separator();
                            for &(x, y) in &caps.resolutions {
                                let label = if x == y { format!("{x} dpi") } else { format!("{x} × {y} dpi") };
                                ui.selectable_value(&mut j.quality, x, label);
                            }
                        }
                    });
                });
                let hint = if export {
                    ts(format!("Bilddateien werden mit {} dpi gespeichert.", export_dpi_for(j)), format!("Picture files are saved at {} dpi.", export_dpi_for(j)))
                } else {
                    t(
                        "Die Stufe wird an den Druckertreiber übergeben (z. B. „Hoch“ = feinere Auflösung). Wie langsam und fein ein Fotodrucker wirklich druckt, hängt meist vor allem von der Papierart ab – wählen Sie dort Ihr Fotopapier.",
                        "The level is passed to the printer driver (e.g. \"High\" = finer resolution). How slowly and finely a photo printer really prints mostly depends on the paper type – choose your photo paper there.",
                    )
                    .to_string()
                };
                ui.label(RichText::new(hint).size(12.5).color(p().dim));
            });
        }
        if global {
            section(ui, t("Mehrere Fotos pro Blatt", "Several photos per sheet"), |ui| {
                let j = &mut d.job;
                let mut layout = (j.cols, j.rows);
                if widgets::segmented(ui, &mut layout, &[((1, 1), "1"), ((1, 2), "2"), ((2, 2), "4"), ((2, 3), "6"), ((3, 3), "9"), ((3, 4), "12")], 34.0) {
                    j.cols = layout.0;
                    j.rows = layout.1;
                }
                ui.label(RichText::new(t("Feinere Einstellungen unter „Mehrere Fotos pro Blatt“.", "More settings under \"Several photos per sheet\".")).size(12.5).color(p().dim));
            });
        }
    }

    fn tab_layout(&mut self, ui: &mut Ui, d: &mut Draft) {
        let ph = &mut d.photo;
        section(ui, t("Skalierungsmodus", "Scaling"), |ui| {
            for m in ScaleMode::ALL {
                ui.radio_value(&mut ph.scale, m, m.label());
            }
            if ph.scale == ScaleMode::Zoom {
                ui.horizontal(|ui| {
                    ui.label(t("Zoomstufe:", "Zoom level:"));
                    ui.add(egui::DragValue::new(&mut ph.zoom).range(5..=1000).suffix(" %"));
                });
            }
        });
        section(ui, t("Ausrichtung auf dem Blatt", "Position on the sheet"), |ui| {
            widgets::label(ui, t("Waagerecht", "Horizontal"));
            widgets::segmented(ui, &mut ph.h_align, &[(HAlign::Left, HAlign::Left.label()), (HAlign::Center, HAlign::Center.label()), (HAlign::Right, HAlign::Right.label())], 32.0);
            widgets::label(ui, t("Senkrecht", "Vertical"));
            widgets::segmented(ui, &mut ph.v_align, &[(VAlign::Top, VAlign::Top.label()), (VAlign::Center, VAlign::Center.label()), (VAlign::Bottom, VAlign::Bottom.label())], 32.0);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(t("Versatz  X", "Offset  X"));
                ui.add(egui::DragValue::new(&mut ph.offset_x_mm).speed(0.5).range(-500.0..=500.0).suffix(" mm"));
                ui.label("Y");
                ui.add(egui::DragValue::new(&mut ph.offset_y_mm).speed(0.5).range(-500.0..=500.0).suffix(" mm"));
            });
            ui.label(RichText::new(t("Tipp: In der Vorschau verschieben Sie den Bildausschnitt mit der Maus; Doppelklick setzt ihn zurück.", "Tip: drag the picture in the preview to move the crop; double-click resets it.")).size(12.5).color(p().dim));
        });
        section(ui, t("Drehung", "Rotation"), |ui| {
            widgets::label(ui, t("Automatisch drehen", "Turn automatically"));
            widgets::segmented(
                ui,
                &mut ph.auto_rotate,
                &[(AutoRotate::Clockwise, t("Rechtsherum", "Clockwise")), (AutoRotate::CounterClockwise, t("Linksherum", "Anti-clockwise")), (AutoRotate::None, t("Aus", "Off"))],
                32.0,
            );
            ui.add_enabled_ui(ph.auto_rotate == AutoRotate::None, |ui| {
                widgets::label(ui, t("Feste Drehung", "Fixed rotation"));
                widgets::segmented(ui, &mut ph.rotation, &[(0, "0°"), (90, "90°"), (180, "180°"), (270, "270°")], 32.0);
            });
        });
    }

    fn tab_collage(&mut self, ui: &mut Ui, d: &mut Draft) {
        let j = &mut d.job;
        section(ui, t("Anordnung", "Arrangement"), |ui| {
            let mut layout = (j.cols, j.rows);
            if widgets::segmented(ui, &mut layout, &[((1, 1), "1×1"), ((1, 2), "1×2"), ((2, 2), "2×2"), ((2, 3), "2×3"), ((3, 3), "3×3"), ((3, 4), "3×4"), ((4, 5), "4×5")], 34.0) {
                j.cols = layout.0;
                j.rows = layout.1;
            }
            ui.add_space(4.0);
            egui::Grid::new("collage").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                ui.label(t("Spalten:", "Columns:"));
                ui.add(egui::DragValue::new(&mut j.cols).range(1..=10));
                ui.end_row();
                ui.label(t("Zeilen:", "Rows:"));
                ui.add(egui::DragValue::new(&mut j.rows).range(1..=10));
                ui.end_row();
                ui.label(t("Lücken zwischen den Fotos:", "Gap between photos:"));
                ui.add(egui::DragValue::new(&mut j.gap_pct).speed(0.05).range(0.0..=10.0).suffix(" %"));
                ui.end_row();
            });
            ui.label(RichText::new(t("Der Abstand ist ein Prozentsatz der längeren Seite des Blattes.", "The gap is a percentage of the sheet's longer side.")).size(12.5).color(p().dim));
        });
        section(ui, t("Darstellung", "Appearance"), |ui| {
            ui.add_enabled_ui(j.collage(), |ui| {
                ui.checkbox(&mut j.frame, t("Rahmen um jedes Foto zeichnen", "Draw a frame around each photo"));
                ui.checkbox(&mut j.next_photo_new_sheet, t("Jedes Foto auf einem neuen Blatt beginnen", "Start each photo on a new sheet"));
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut j.cut_marks, t("Schnittmarken drucken", "Print cut marks"));
                ui.add_enabled(j.cut_marks, egui::DragValue::new(&mut j.cut_marks_offset).range(-100.0..=400.0).suffix(t(" /100 Zoll versetzt", " /100 inch offset")));
            });
        });
    }

    fn tab_output(&mut self, ui: &mut Ui, d: &mut Draft) {
        let j = &mut d.job;
        section(ui, t("Ränder", "Margins"), |ui| {
            ui.checkbox(&mut j.ignore_hard_margins, t("Druckerränder ignorieren (randlos, wenn der Drucker es kann)", "Ignore printer margins (borderless, if the printer can)"));
            ui.horizontal(|ui| {
                ui.label(t("Zusätzlicher weißer Rand:", "Extra white margin:"));
                ui.add(egui::DragValue::new(&mut j.extra_margin_mm).speed(0.5).range(0.0..=50.0).suffix(" mm"));
            });
        });
        section(ui, t("Druckauftrag", "Print job"), |ui| {
            ui.checkbox(&mut j.single_job, t("Alle Fotos in einem einzigen Druckauftrag senden", "Send all photos as one print job"));
            ui.checkbox(&mut j.remove_printed, t("Gedruckte Fotos aus der Liste entfernen", "Remove printed photos from the list"));
        });
        section(ui, t("Als Bilddateien speichern", "Save as picture files"), |ui| {
            ui.horizontal(|ui| {
                ui.label(t("Auflösung:", "Resolution:"));
                ui.add(egui::DragValue::new(&mut j.export_dpi).range(72..=1200).suffix(" dpi"));
            });
            ui.horizontal(|ui| {
                if ui.button(t("Zielordner …", "Target folder …")).clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        j.export_dir = dir.to_string_lossy().into_owned();
                    }
                }
                let t = if j.export_dir.is_empty() { t("(wird beim Start gefragt)", "(asked when starting)").to_string() } else { j.export_dir.clone() };
                ui.label(RichText::new(t).color(p().dim));
            });
            ui.label(RichText::new(ts(format!("Dafür oben den Drucker „{}“ wählen.", printer_label(EXPORT_PRINTER)), format!("For this choose the printer \"{}\" at the top.", printer_label(EXPORT_PRINTER)))).size(12.5).color(p().dim));
        });
    }

    pub fn apply(&mut self, draft: &Draft) {
        match &draft.target {
            Target::Global => {
                self.job = draft.job.clone().sanitize();
                self.job.photo = draft.photo.clone().sanitize();
                self.save_settings();
            }
            Target::Items(ix) => {
                for &i in ix {
                    if let Some(it) = self.items.get_mut(i) {
                        it.own = if draft.photo == self.job.photo { None } else { Some(draft.photo.clone().sanitize()) };
                    }
                }
            }
        }
    }

    fn draft_preview(&mut self, ui: &mut Ui, r: Rect, d: &mut Draft) {
        let c = p();
        let mut job = d.job.clone();
        job.printer = self.job.printer.clone();
        let mut items = self.items.clone();
        let focus: Vec<usize> = match &d.target {
            Target::Global => {
                job.photo = d.photo.clone();
                self.selected()
            }
            Target::Items(ix) => {
                for &i in ix {
                    if let Some(it) = items.get_mut(i) {
                        it.own = Some(d.photo.clone());
                    }
                }
                ix.clone()
            }
        };
        let order = self.order();
        let sheets = if order.is_empty() {
            let land = layout::sheet_landscape(d.photo.orientation, (100.0, 150.0));
            let page = self.printers.page(&job.printer, &d.photo, land, job.export_dpi);
            vec![Sheet { paper: d.photo.paper, tray: None, media: None, landscape: land, settings_of: 0, page, cells: vec![] }]
        } else {
            self.plan_with(&job, &items, &order)
        };
        if d.sheet == 0 {
            d.sheet = focus.first().and_then(|f| sheets.iter().position(|s| s.cells.iter().any(|cl| cl.item == *f))).map(|k| k + 1).unwrap_or(1);
        }
        let n = sheets.len().max(1);
        let idx = d.sheet.saturating_sub(1).min(n - 1);
        let preview_h = (r.height() * 0.66).max(220.0);
        let pr = Rect::from_min_size(r.min, vec2(r.width(), preview_h));
        ui.painter().rect_filled(pr, CornerRadius::same(14), c.stage);
        let saved_job = std::mem::replace(&mut self.job, job.clone());
        let saved_items = std::mem::replace(&mut self.items, items);
        self.sheet_view(ui, pr.shrink(14.0), &sheets[idx], "preview_draft");
        let moved: Vec<Option<(f32, f32)>> = self.items.iter().map(|i| i.fine).collect();
        self.job = saved_job;
        self.items = saved_items;
        for (it, f) in self.items.iter_mut().zip(moved) {
            it.fine = f;
        }
        if order.is_empty() {
            ui.painter().text(pr.center_bottom() - vec2(0.0, 18.0), Align2::CENTER_CENTER, t("Fügen Sie Fotos hinzu, um sie hier zu sehen", "Add photos to see them here"), FontId::proportional(13.0), c.dim);
        }
        let below = Rect::from_min_max(pos2(r.left(), pr.bottom() + 10.0), r.max);
        let page = sheets[idx].page;
        let paper = self.printers.paper_name(&job.printer, d.photo.paper);
        ui.scope_builder(egui::UiBuilder::new().max_rect(below), |ui| {
            ui.horizontal(|ui| {
                if ui.add_enabled(idx > 0, egui::Button::new("‹").min_size(vec2(30.0, 28.0))).clicked() {
                    d.sheet = idx;
                }
                ui.label(RichText::new(ts(format!("Blatt {} von {}", idx + 1, n), format!("Sheet {} of {}", idx + 1, n))).color(c.dim));
                if ui.add_enabled(idx + 1 < n, egui::Button::new("›").min_size(vec2(30.0, 28.0))).clicked() {
                    d.sheet = idx + 2;
                }
            });
            ui.add_space(4.0);
            let area = layout::printable_area(&page, &job);
            let row = |ui: &mut Ui, k: &str, v: String| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(k).color(c.dim));
                    ui.label(RichText::new(v).color(c.text));
                });
            };
            row(ui, t("Papier:", "Paper:"), format!("{paper} · {:.0} × {:.0} mm", page.width_mm, page.height_mm));
            row(ui, t("Bedruckbar:", "Printable:"), format!("{:.0} × {:.0} mm", area.w, area.h));
            row(ui, t("Einpassen:", "Fit:"), format!("{} · {} / {}", d.photo.scale.short(), d.photo.h_align.label(), d.photo.v_align.label()));
            row(ui, t("Pro Blatt:", "Per sheet:"), if job.collage() { format!("{} × {} {}", job.cols, job.rows, t("Fotos", "photos")) } else { t("1 Foto", "1 photo").into() });
            row(ui, t("Kopien je Foto:", "Copies per photo:"), d.photo.copies.to_string());
            row(ui, t("Blätter insgesamt:", "Sheets in total:"), if order.is_empty() { "0".into() } else { sheets.len().to_string() });
        });
    }

    /// Draw one sheet with its photos; dragging moves a photo inside its area.
    pub fn sheet_view(&mut self, ui: &mut Ui, r: Rect, sheet: &Sheet, id: &str) {
        let c = p();
        let page = sheet.page;
        let scale = (r.width() / page.width_mm).min(r.height() / page.height_mm) * 0.94;
        let size = vec2(page.width_mm * scale, page.height_mm * scale);
        let paper = Rect::from_center_size(r.center(), size);
        let to = |x: f32, y: f32| pos2(paper.left() + x * scale, paper.top() + y * scale);
        let resp = ui.interact(paper, ui.scope_id().with(id), Sense::click_and_drag());
        let pnt = ui.painter_at(r.expand(6.0));
        for k in 1..=4 {
            pnt.rect_filled(paper.translate(vec2(0.0, k as f32 * 1.5)).expand(k as f32), CornerRadius::same(3), Color32::from_black_alpha(14));
        }
        pnt.rect_filled(paper, CornerRadius::same(2), Color32::WHITE);
        let area = layout::printable_area(&page, &self.job);
        if area.w < page.width_mm - 0.1 || area.h < page.height_mm - 0.1 {
            let a = Rect::from_min_max(to(area.x, area.y), to(area.right(), area.bottom()));
            pnt.extend(Shape::dashed_line(
                &[a.left_top(), a.right_top(), a.right_bottom(), a.left_bottom(), a.left_top()],
                Stroke::new(1.0, Color32::from_rgb(150, 170, 230)),
                4.0,
                3.0,
            ));
        }
        let pointer = resp.interact_pointer_pos().or(resp.hover_pos());
        let mut hovered_cell = None;
        for (ci, cell) in sheet.cells.iter().enumerate() {
            let pl = cell.placement;
            let Some(vis) = pl.rect.intersect(&pl.clip) else { continue };
            let vr = Rect::from_min_max(to(vis.x, vis.y), to(vis.right(), vis.bottom()));
            let cr = Rect::from_min_max(to(pl.clip.x, pl.clip.y), to(pl.clip.right(), pl.clip.bottom()));
            if pointer.map(|h| cr.contains(h)).unwrap_or(false) {
                hovered_cell = Some(ci);
            }
            let item = &self.items[cell.item];
            if let Some(tex) = self.thumbs.get(&item.path) {
                let uvp = |x: f32, y: f32| {
                    let u2 = (x - pl.rect.x) / pl.rect.w;
                    let v2 = (y - pl.rect.y) / pl.rect.h;
                    match pl.rot {
                        90 => pos2(v2, 1.0 - u2),
                        180 => pos2(1.0 - u2, 1.0 - v2),
                        270 => pos2(1.0 - v2, u2),
                        _ => pos2(u2, v2),
                    }
                };
                let mut mesh = egui::Mesh::with_texture(tex.id());
                for (x, y) in [(vis.x, vis.y), (vis.right(), vis.y), (vis.right(), vis.bottom()), (vis.x, vis.bottom())] {
                    mesh.vertices.push(egui::epaint::Vertex { pos: to(x, y), uv: uvp(x, y), color: Color32::WHITE });
                }
                mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                pnt.add(Shape::mesh(mesh));
                if item.settings(&self.job).color == ColorMode::Gray {
                    widgets::chip(&pnt, vr.left_top() + vec2(4.0, 4.0), Align2::LEFT_TOP, t("S/W", "B/W"), Color32::WHITE, Color32::from_black_alpha(160));
                }
            } else {
                pnt.rect_filled(vr, CornerRadius::ZERO, Color32::from_rgb(214, 222, 236));
                let ir = Rect::from_center_size(vr.center(), Vec2::splat(vr.width().min(vr.height()) * 0.45));
                icons::photo_glyph(&pnt, ir, Color32::from_rgb(160, 172, 196));
            }
        }
        for line in crate::render::decoration_lines(sheet, &self.job) {
            let lr = Rect::from_min_max(to(line.x, line.y), to(line.right(), line.bottom()));
            pnt.rect_filled(lr.expand(0.2), CornerRadius::ZERO, Color32::BLACK);
        }
        if let Some(ci) = hovered_cell {
            let cell = &sheet.cells[ci];
            let pl = cell.placement;
            let cr = Rect::from_min_max(to(pl.clip.x, pl.clip.y), to(pl.clip.right(), pl.clip.bottom()));
            pnt.rect_stroke(cr, CornerRadius::ZERO, Stroke::new(2.0, c.accent2), StrokeKind::Inside);
            if resp.dragged() {
                let dl = resp.drag_delta() / scale;
                let job_photo = self.job.photo.clone();
                let item = &mut self.items[cell.item];
                let s = item.own.clone().unwrap_or(job_photo);
                let (mut fx, mut fy) = item.fine.unwrap_or((s.h_align.fraction(), s.v_align.fraction()));
                let room_x = pl.clip.w - pl.rect.w;
                let room_y = pl.clip.h - pl.rect.h;
                if room_x.abs() > 0.5 {
                    fx = (fx + dl.x / room_x).clamp(0.0, 1.0);
                }
                if room_y.abs() > 0.5 {
                    fy = (fy + dl.y / room_y).clamp(0.0, 1.0);
                }
                item.fine = Some((fx, fy));
            }
            if resp.double_clicked() {
                self.items[cell.item].fine = None;
            }
            let name = self.items[cell.item].name.clone();
            resp.on_hover_text(format!("{name}\n{}", t("Ziehen verschiebt den Bildausschnitt · Doppelklick setzt ihn zurück", "Drag to move the crop · double-click resets it")))
                .on_hover_cursor(egui::CursorIcon::Grab);
        }
    }
}

/// The user guide on GitHub, in the current language.
pub fn user_guide_url() -> String {
    format!("{}/blob/main/docs/{}", crate::update::REPO_URL, t("ANLEITUNG.md", "USER-GUIDE.md"))
}

/// A new bug report with the version filled in (nothing else about this PC is sent).
pub fn report_url() -> String {
    format!("{}/issues/new?template=bug_report.yml&version={}", crate::update::REPO_URL, env!("CARGO_PKG_VERSION"))
}
