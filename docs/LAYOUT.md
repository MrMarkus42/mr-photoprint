# How Mr. PhotoPrint places photos

This is the design of the layout engine (`src/layout.rs`): the rules that decide where each photo
lands on the paper. Everything is computed in millimetres; the printer's resolution only matters
when the sheet is finally drawn.

## Units

* Layout works in **millimetres**. Device pixels come from the printer's DPI at print time.
* Offsets and cut-mark distances are stored in **1/100 inch** (0.254 mm), a common printing unit.
* A photo's natural size is *pixels ÷ the resolution stored in the file*. Files without a
  resolution are assumed to be **96 dpi** (the Windows default).

## Printable area

```
printable width  = paper width  − (left margin + right margin)
printable height = paper height − (top margin + bottom margin)
```

The margins are the printer's *hard* margins (the part it physically can't print) plus an optional
extra white margin on every side.

### Borderless

Switching margins off in the layout alone is not enough: a driver clips everything that falls into
its own non-printable edge. So **Borderless** also switches the driver into its borderless mode,
the way that driver offers it:

* a borderless **paper source** (Epson: "Einzug hinten-Randlos" / "Rear Paper Feed – Borderless"), or
* a borderless variant of the **paper size** (Canon, HP: "10 x 15 cm (borderless)").

Both are recognised by name in several languages. Drivers check each setting against the others
*as they were before*, so the settings are handed to the driver up to three times until it keeps
paper, source and paper type. When the driver then reports no edge at all, the photo reaches
**1 mm** past each paper edge (bleed). If the driver still keeps an edge, the program shows its
width and suggests choosing photo paper as the paper type.

## Paper orientation

| Setting | Effect |
|---|---|
| **Auto** (default) | The sheet is turned to match each photo (landscape photo → landscape paper). |
| Portrait / Landscape | Fixed. |
| As set in the printer | The driver's own setting is used. |

## Automatic rotation

When a photo's orientation differs from the printable area's orientation (and neither is
square), the photo is turned by 90° – clockwise or anti-clockwise, as chosen. With automatic
rotation off, a fixed rotation of 0°, 90°, 180° or 270° can be set per photo.

## Scale modes

With `(w, h)` the photo size and `(W, H)` the available area (after rotation):

| Mode | Factor |
|---|---|
| **Fill** | `max(W/w, H/h)` – covers the area; what sticks out is cropped. |
| **Fit** | `min(W/w, H/h)` – the whole photo is visible. |
| **Shrink** (default) | Like Fit, but only when the photo is larger than the area – never enlarges. |
| **Actual size** | `1` |
| **Zoom** | `zoom / 100` |

## Position

Horizontal: left / centre / right; vertical: top / centre / bottom (default centre/centre).

```
x = offset_x + (W − w) · fx      fx = 0 (left), 0.5 (centre), 1 (right)
y = offset_y + (H − h) · fy      fy = 0 (top),  0.5 (centre), 1 (bottom)
```

Dragging a photo in the preview stores a continuous `(fx, fy)` per photo, so the crop of a filled
photo can be moved freely. Double-click resets it.

For Fit and Shrink, an offset that pushes the photo out of the area makes it fit again into the
space that is left.

## Several photos per sheet

* The printable area is split into `columns × rows` equal cells, filled row by row.
* The gap between photos is a percentage of the sheet's longer side (default 0.5 %); each cell is
  inset by it on every side.
* Each photo is placed in its cell with the normal scale, position and rotation rules, so its
  proportions are always kept.
* Optional: a thin black frame around each photo, and "start each photo on a new sheet" (useful
  with several copies).

## Cut marks

At each corner two short lines (21/100 inch long, starting 9/100 inch outside the photo), moved
inwards by an adjustable offset.

## Print quality

`As printer` leaves the driver's setting alone. `Draft / Low / Medium / High` set the driver's
quality level; a number is a resolution in dpi taken from the driver's own list. When saving as
JPEG files the levels mean 150 / 200 / 300 / 600 dpi.

## Sharpness warning

Effective resolution of a print = `min(pixels wide ÷ printed width in inches, pixels high ÷
printed height in inches)`. Below **150 dpi** a photo is marked "blurry?" – a common rule of thumb
for photo prints.

## Presets

| Preset | Paper | Scale | Per sheet | Borderless |
|---|---|---|---|---|
| 10×15 borderless | 102 × 152 mm (4 × 6 in) | Fill | 1 | yes |
| 13×18 borderless | 127 × 178 mm (5 × 7 in) | Fill | 1 | yes |
| A4 whole photo | A4 | Fit | 1 | no |
| 4 on A4 | A4 | Fill | 2 × 2 | no |
| Contact sheet | A4 | Fit | 3 × 4 | no |

Drivers number photo papers differently, so a preset looks up the paper by **size** (within
3 mm, either orientation) in the printer's own list.

## Protection against hostile files

Photos come from anywhere, so every file is treated as untrusted:

* The format is decided by the **content** (magic bytes), not the file name.
* Files over 400 MB, pictures over 250 megapixels or with a side over 40 000 px are refused
  *before* decoding; decoder memory is capped at 2 GB.
* Decoders run behind `catch_unwind` – a broken file becomes an error message, not a crash.
* Folder scans don't follow links or junctions (max. depth 32, max. 50 000 files).
* Photo lists and imported settings never open network paths (`\\server\…`, `\\?\UNC\…`,
  `\\?\GLOBALROOT\…`, device paths); settings files are size-limited and every value is clamped.
* Printer driver settings (DEVMODE) are size-checked before a driver sees them.
* Settings are saved atomically (temp file + rename).
* Exported sheets obey the same pixel limits as photos.
* HEIC/HEIF, AVIF, JPEG XL and camera RAW are decoded by Windows' own codecs (WIC); width × height
  are checked before pixel memory is allocated.
