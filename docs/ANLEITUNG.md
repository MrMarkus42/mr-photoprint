# Mr. PhotoPrint – Anleitung

[English version](USER-GUIDE.md)

- [Installieren](#installieren)
- [In vier Schritten zum Foto](#in-vier-schritten-zum-foto)
- [Vorlagen](#vorlagen)
- [Fotos einpassen](#fotos-einpassen)
- [Mehrere Fotos pro Blatt](#mehrere-fotos-pro-blatt)
- [Papierart und Druckqualität](#papierart-und-druckqualität)
- [Eigene Einstellungen für einzelne Fotos](#eigene-einstellungen-für-einzelne-fotos)
- [Als JPEG-Dateien speichern](#als-jpeg-dateien-speichern)
- [Tastatur und Maus](#tastatur-und-maus)
- [Updates](#updates)
- [Sprache und Darstellung](#sprache-und-darstellung)
- [Häufige Fragen](#häufige-fragen)

## Installieren

1. Auf der [Release-Seite](https://github.com/MrMarkus42/mr-photoprint/releases/latest)
   `MrPhotoPrint-Setup-….exe` herunterladen und starten.
2. **„Nur für mich installieren“** braucht keine Administratorrechte – empfohlen.
   **„Für alle Benutzer“** installiert nach `C:\Programme` und fragt nach Administratorrechten.
3. Fertig – Mr. PhotoPrint steht im Startmenü.

> **Windows-SmartScreen:** Beim ersten Start kann „Der Computer wurde durch Windows geschützt“
> erscheinen, weil das Programm (noch) kein kostenpflichtiges Herausgeber-Zertifikat hat. Auf
> **„Weitere Informationen“ → „Trotzdem ausführen“** klicken. Wer sichergehen will, prüft die
> Datei vorher wie unter [Sicherheit](../SECURITY.md#downloads-prüfen) beschrieben.

**Ohne Installation:** `MrPhotoPrint-…-portable.zip` entpacken und `MrPhotoPrint.exe` starten.
Die portable Version meldet neue Versionen, installiert sie aber nicht selbst.

**Deinstallieren:** Windows-Einstellungen → Apps → Mr. PhotoPrint → Deinstallieren. Ihre
Einstellungen bleiben unter `%APPDATA%\Mr. PhotoPrint` erhalten und können dort gelöscht werden.

## In vier Schritten zum Foto

1. **Fotos hinzufügen** – Fotos oder ganze Ordner ins Fenster ziehen, oder oben
   „Fotos hinzufügen“ bzw. „Ordner hinzufügen“ wählen. Unterordner werden mitgenommen.
2. **Drucker und Papier wählen** – oben den Drucker, rechts eine **Vorlage** (z. B.
   „10×15 randlos“) oder Papier und „Foto einpassen“.
3. **Vorschau prüfen** – rechts sehen Sie jedes Blatt so, wie es gedruckt wird. Mit `‹ ›`
   blättern. Den **Bildausschnitt verschieben** Sie, indem Sie das Foto in der Vorschau mit der
   Maus ziehen; Doppelklick setzt ihn zurück.
4. **Drucken** – der Knopf zeigt, wie viele Blätter es werden. Während des Drucks können Sie
   pausieren oder stoppen. Gedruckte Fotos werden (abschaltbar) aus der Liste entfernt.

> **Tipp:** Erst ein einzelnes Foto drucken und prüfen, dann den ganzen Stapel.

## Vorlagen

| Vorlage | Papier | Einpassen | Pro Blatt | Randlos |
|---|---|---|---|---|
| 10×15 randlos | 10 × 15 cm (4 × 6 Zoll) | Füllen | 1 | ja |
| 13×18 randlos | 13 × 18 cm (5 × 7 Zoll) | Füllen | 1 | ja |
| A4 ganzes Foto | A4 | Anpassen | 1 | nein |
| 4 auf A4 | A4 | Füllen | 2 × 2 | nein |
| Kontaktbogen | A4 | Anpassen | 3 × 4 | nein |

Das Papier wird anhand seiner Größe in der Liste Ihres Druckers gesucht. Kennt der Drucker das
Format nicht, bleibt das Papier unverändert und Sie bekommen einen Hinweis.

**Randlos** stellt Mr. PhotoPrint automatisch so ein, wie es Ihr Drucker verlangt: Epson-Drucker
haben dafür eine eigene Papierquelle (z. B. „Einzug hinten-Randlos“), Canon und HP eine eigene
randlose Papiergröße. Das Foto ragt dann 1 mm über die Kante, damit auch bei leicht schräg
eingezogenem Papier kein weißer Strich bleibt. Lässt der Drucker trotzdem einen Rand, zeigt das
Programm einen Hinweis – meist hilft es, bei **Papierart** das Fotopapier zu wählen.

## Papierart und Druckqualität

Die **Papierart** (z. B. „Epson Premium Glossy“) sagt dem Drucker, worauf er druckt. Bei
Fotodruckern bestimmt sie Tintenmenge, Geschwindigkeit und Qualität am stärksten – wählen Sie
immer Ihr Fotopapier. Die Liste kommt direkt aus Ihrem Druckertreiber.

Die **Druckqualität** (Entwurf … Hoch) wird zusätzlich an den Treiber übergeben, z. B. als feinere
Auflösung. Was der Treiber daraus macht, entscheidet er selbst.

Alles, was der Treiber sonst noch kann, stellen Sie über das **Zahnrad** oben ein
(Druckereigenschaften). Diese Einstellungen merkt sich Mr. PhotoPrint für jeden Drucker.

## Fotos einpassen

| Modus | Ergebnis |
|---|---|
| **Füllen** | Das Foto bedeckt das ganze Blatt; was übersteht, wird abgeschnitten. Ausschnitt in der Vorschau verschiebbar. |
| **Anpassen** | Das ganze Foto ist zu sehen; an zwei Seiten bleibt eventuell ein weißer Rand. |
| **Verkleinern** | Wie Anpassen, aber kleine Fotos werden nicht vergrößert. |
| **Original** | Das Foto wird in seiner gespeicherten Größe gedruckt. |
| **Zoom** | Fester Prozentsatz (unter „Alle Einstellungen“ → Seitenlayout). |

**Automatisch drehen:** Liegt ein Querformat-Foto auf Hochformat-Papier, wird es um 90° gedreht.
**Papier „Auto“:** Das Blatt wird für jedes Foto passend hoch oder quer gelegt.

**„unscharf?“** erscheint, wenn ein Foto auf dem Papier weniger als 150 Pixel pro Zoll hat.
Kleineres Papier, „Anpassen“ statt „Füllen“ oder mehrere Fotos pro Blatt helfen.

## Mehrere Fotos pro Blatt

Unter „Fotos pro Blatt“ 1, 2, 4, 6, 9 oder 12 wählen – oder unter **Alle Einstellungen →
Mehrere Fotos pro Blatt** beliebig viele Spalten und Zeilen, den Abstand, Rahmen und
Schnittmarken einstellen. Mit „Jedes Foto auf einem neuen Blatt beginnen“ landen Kopien
desselben Fotos gemeinsam auf einem Blatt.

## Eigene Einstellungen für einzelne Fotos

Fotos markieren (Klick, Strg+Klick, Umschalt+Klick) und **„Eigene Einstellungen“** wählen –
oder einfach doppelt auf ein Foto klicken. Diese Fotos bekommen ein eigenes Papier, eigene
Kopienzahl, Drehung, Einpassung usw. und sind mit „eigene Einstellung“ markiert.
„Zurücksetzen“ entfernt das wieder.

## Als JPEG-Dateien speichern

Oben den Drucker **„In JPEG-Dateien speichern“** wählen. Jedes Blatt wird als Bilddatei
gespeichert – ideal zum Ausprobieren ohne Papier oder für einen Fotodienst. Ordner und Auflösung
stellen Sie unter **Alle Einstellungen → Ränder und Ausgabe** ein.

## Tastatur und Maus

| Taste / Aktion | Wirkung |
|---|---|
| `F1` | Hilfe |
| `Strg` + `A` | Alle Fotos auswählen |
| `Entf` | Ausgewählte Fotos aus der Liste entfernen |
| `Esc` | Auswahl aufheben |
| `Strg`/`Umschalt` + Klick | Mehrere Fotos auswählen |
| Doppelklick auf ein Foto | Eigene Einstellungen |
| Rechtsklick auf ein Foto | Drehen, verschieben, Ordner öffnen … |
| Klick auf den Kreis | Foto zum Drucken an- oder abwählen |
| In der Vorschau ziehen | Bildausschnitt verschieben (Doppelklick: zurücksetzen) |

## Updates

Beim ersten Start fragt Mr. PhotoPrint, ob es nach neuen Versionen suchen darf. Unter
**Einstellungen → Programm → Updates** stellen Sie ein:

- **Nach Updates suchen:** bei jedem Start, täglich, wöchentlich oder nie.
- **Wenn eine neue Version da ist:** nur Bescheid sagen · herunterladen und Installieren anbieten ·
  automatisch beim Beenden installieren.
- **Jetzt suchen** prüft sofort.

Jedes Update ist **digital signiert** und wird vor der Installation geprüft – eine veränderte oder
fremde Datei wird abgelehnt. Bei einer Installation „für alle Benutzer“ fragt Windows nach der
Erlaubnis (Benutzerkontensteuerung). Danach startet das Programm von selbst neu.

Dabei wird nur die Versionsliste von GitHub abgerufen – es werden keine Daten über Sie, Ihren PC
oder Ihre Fotos gesendet.

## Sprache und Darstellung

**Einstellungen → Programm:** Deutsch, English oder „Windows“ (folgt der Sprache von Windows);
dunkles oder helles Design (auch über das Sonne/Mond-Symbol oben rechts).

## Häufige Fragen

**HEIC-Fotos vom iPhone öffnen sich nicht.**
Installieren Sie aus dem Microsoft Store die kostenlosen „HEIF-Bilderweiterungen“ und
„HEVC-Videoerweiterungen“. Für AVIF: „AV1 Video Extension“, für Kamera-RAW: „Raw Image Extension“.

**Am Rand bleibt trotz „Randlos“ ein weißer Streifen.**
Achten Sie auf den Hinweis unter der Vorschau. Wählen Sie bei „Papierart“ Ihr Fotopapier; manche
Drucker können bestimmte Formate (oft A4) nicht randlos. Fotopapier „10 × 15“ misst übrigens
meist 10,2 × 15,2 cm (4 × 6 Zoll) – das ist richtig so.

**Das Foto ist falsch herum.**
Mr. PhotoPrint beachtet die Ausrichtung, die die Kamera im Foto gespeichert hat. Mit Rechtsklick →
„Um 90° drehen“ korrigieren Sie einzelne Fotos.

**Wo sind meine Einstellungen?**
In `%APPDATA%\Mr. PhotoPrint`. Unter „Alle Einstellungen“ können Sie sie exportieren und auf einem
anderen PC importieren.

**Etwas funktioniert nicht.**
Einstellungen → Programm → „Problem melden …“ öffnet ein vorbereitetes Formular auf GitHub. Das
Protokoll (`mrphotoprint.log`, Knopf „Protokoll-Ordner öffnen“) hilft bei der Fehlersuche – bitte
vor dem Anhängen durchsehen.
