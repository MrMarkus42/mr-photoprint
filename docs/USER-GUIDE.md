# Mr. PhotoPrint – User guide

[Deutsche Version](ANLEITUNG.md)

- [Installing](#installing)
- [Four steps to your prints](#four-steps-to-your-prints)
- [Presets](#presets)
- [Fitting photos](#fitting-photos)
- [Several photos per sheet](#several-photos-per-sheet)
- [Paper type and print quality](#paper-type-and-print-quality)
- [Own settings for single photos](#own-settings-for-single-photos)
- [Saving as JPEG files](#saving-as-jpeg-files)
- [Keyboard and mouse](#keyboard-and-mouse)
- [Updates](#updates)
- [Language and appearance](#language-and-appearance)
- [FAQ](#faq)

## Installing

1. Download `MrPhotoPrint-Setup-….exe` from the
   [release page](https://github.com/MrMarkus42/mr-photoprint/releases/latest) and run it.
2. **"Install for me only"** needs no administrator rights – recommended.
   **"Install for all users"** installs into `C:\Program Files` and asks for administrator rights.
3. Done – Mr. PhotoPrint is in the Start menu.

> **Windows SmartScreen:** on first start Windows may say "Windows protected your PC" because the
> program has no paid publisher certificate (yet). Click **"More info" → "Run anyway"**. To be
> sure, check the file first as described under [Security](../SECURITY.md#verifying-downloads).

**Without installing:** unzip `MrPhotoPrint-…-portable.zip` and start `MrPhotoPrint.exe`. The
portable version announces new versions but does not install them itself.

**Uninstalling:** Windows Settings → Apps → Mr. PhotoPrint → Uninstall. Your settings stay in
`%APPDATA%\Mr. PhotoPrint` and can be deleted there.

## Four steps to your prints

1. **Add photos** – drag photos or whole folders into the window, or use "Add photos" /
   "Add folder" at the top. Subfolders are included.
2. **Choose printer and paper** – the printer at the top, then a **preset** on the right
   (e.g. "10×15 borderless") or the paper and how the photo is fitted.
3. **Check the preview** – on the right you see each sheet exactly as it will be printed; browse
   with `‹ ›`. **Move the crop** by dragging the photo in the preview; double-click resets it.
4. **Print** – the button shows how many sheets it will be. You can pause or stop while printing.
   Printed photos are removed from the list (can be switched off).

> **Tip:** print a single photo first and check it, then the whole batch.

## Presets

| Preset | Paper | Fit | Per sheet | Borderless |
|---|---|---|---|---|
| 10×15 borderless | 10 × 15 cm (4 × 6 in) | Fill | 1 | yes |
| 13×18 borderless | 13 × 18 cm (5 × 7 in) | Fill | 1 | yes |
| A4 whole photo | A4 | Fit | 1 | no |
| 4 on A4 | A4 | Fill | 2 × 2 | no |
| Contact sheet | A4 | Fit | 3 × 4 | no |

The paper is looked up by its size in your printer's own list. If the printer doesn't know the
size, the paper stays as it is and you get a note.

**Borderless** is set up the way your printer wants it: Epson printers have their own paper
source for it (e.g. "Rear Paper Feed – Borderless"), Canon and HP their own borderless paper
sizes. The photo then reaches 1 mm past the edge so even slightly skewed paper gets no white line.
If the printer still keeps an edge, the program says so – choosing your photo paper under
**Paper type** usually helps.

## Paper type and print quality

The **paper type** (e.g. "Epson Premium Glossy") tells the printer what it prints on. On photo
printers it has the biggest effect on ink amount, speed and quality – always choose your photo
paper. The list comes straight from your printer driver.

The **print quality** (Draft … High) is passed to the driver as well, e.g. as a finer resolution.
What the driver makes of it is up to the driver.

Everything else the driver offers is set via the **gear icon** at the top (printer properties).
Mr. PhotoPrint remembers these settings for each printer.

## Fitting photos

| Mode | Result |
|---|---|
| **Fill** | The photo covers the whole sheet; whatever sticks out is cropped. The crop can be moved in the preview. |
| **Fit** | The whole photo is visible; there may be a white border on two sides. |
| **Shrink** | Like Fit, but small photos are not enlarged. |
| **Actual** | The photo is printed at its stored size. |
| **Zoom** | A fixed percentage ("All settings" → Page layout). |

**Turn automatically:** a landscape photo on portrait paper is turned by 90°.
**Paper "Auto":** the sheet is turned to portrait or landscape for each photo.

**"blurry?"** appears when a photo gets fewer than 150 pixels per inch on paper. Smaller paper,
"Fit" instead of "Fill", or several photos per sheet help.

## Several photos per sheet

Choose 1, 2, 4, 6, 9 or 12 under "Photos per sheet" – or any number of columns and rows, the gap,
frames and cut marks under **All settings → Several photos per sheet**. With "Start each photo on
a new sheet", copies of the same photo end up together on one sheet.

## Own settings for single photos

Select photos (click, Ctrl+click, Shift+click) and choose **"Own settings"** – or simply
double-click a photo. These photos get their own paper, copies, rotation, fitting and so on, and
are marked "own settings". "Reset" removes them again.

## Saving as JPEG files

Choose the printer **"Save as JPEG files"** at the top. Each sheet is saved as a picture file –
ideal for trying things out without paper, or for a photo service. Folder and resolution are set
under **All settings → Margins and output**.

## Keyboard and mouse

| Key / action | Effect |
|---|---|
| `F1` | Help |
| `Ctrl` + `A` | Select all photos |
| `Del` | Remove the selected photos from the list |
| `Esc` | Clear the selection |
| `Ctrl`/`Shift` + click | Select several photos |
| Double-click a photo | Own settings |
| Right-click a photo | Turn, move, open folder … |
| Click the circle | Include or exclude a photo |
| Drag in the preview | Move the crop (double-click: reset) |

## Updates

On first start Mr. PhotoPrint asks whether it may look for new versions. Under
**Settings → Program → Updates** you choose:

- **Check for updates:** every start, daily, weekly or never.
- **When a new version is out:** just let me know · download and offer to install · install
  automatically when closing.
- **Check now** checks right away.

Every update is **digitally signed** and checked before it is installed – a changed or foreign
file is refused. For an installation "for all users" Windows asks for permission (User Account
Control). Afterwards the program restarts by itself.

Only the list of versions is fetched from GitHub – no data about you, your PC or your photos is
sent.

## Language and appearance

**Settings → Program:** Deutsch, English or "Windows" (follows the language of Windows); dark or
light theme (also via the sun/moon icon at the top right).

## FAQ

**HEIC photos from an iPhone don't open.**
Install the free "HEIF Image Extensions" and "HEVC Video Extensions" from the Microsoft Store. For
AVIF: "AV1 Video Extension", for camera RAW: "Raw Image Extension".

**There is still a white strip at the edge despite "Borderless".**
Look for the note below the preview. Choose your photo paper under "Paper type"; some printers
can't print certain sizes (often A4) borderless. "10 × 15" photo paper usually measures
10.2 × 15.2 cm (4 × 6 in) – that's expected.

**A photo is upside down.**
Mr. PhotoPrint follows the orientation the camera stored in the photo. Right-click → "Turn by 90°"
fixes single photos.

**Where are my settings?**
In `%APPDATA%\Mr. PhotoPrint`. Under "All settings" you can export them and import them on another
PC.

**Something doesn't work.**
Settings → Program → "Report a problem …" opens a prepared form on GitHub. The log
(`mrphotoprint.log`, button "Open log folder") helps to find the cause – please look through it
before attaching it.
