//! Where each photo lands on the paper. The rules are described in docs/LAYOUT.md.

use crate::model::{AutoRotate, Item, JobSettings, Orientation, PhotoSettings, ScaleMode};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RectMm {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl RectMm {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn intersect(&self, o: &RectMm) -> Option<RectMm> {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        (x1 > x0 && y1 > y0).then(|| RectMm::new(x0, y0, x1 - x0, y1 - y0))
    }
    pub fn inset(&self, dx: f32, dy: f32) -> RectMm {
        RectMm::new(self.x + dx, self.y + dy, (self.w - 2.0 * dx).max(1.0), (self.h - 2.0 * dy).max(1.0))
    }
}

/// One side of the paper as the printer reports it, already turned to the chosen orientation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageInfo {
    pub width_mm: f32,
    pub height_mm: f32,
    /// Hard margins the printer can't print on: left, top, right, bottom.
    pub hard: [f32; 4],
    pub dpi: (f32, f32),
    /// The printer itself prints to the paper edge (borderless is really on in the driver), so a
    /// borderless photo may reach a little past it.
    pub bleed: bool,
}

impl PageInfo {
    pub fn plain(width_mm: f32, height_mm: f32, dpi: f32) -> Self {
        Self { width_mm, height_mm, hard: [0.0; 4], dpi: (dpi, dpi), bleed: false }
    }
    pub fn landscape(&self) -> bool {
        self.width_mm > self.height_mm
    }
}

/// How far a borderless photo reaches past each paper edge, so a sheet that sits a little
/// askew still gets no thin white line.
pub const BLEED_MM: f32 = 1.0;

/// Printable width/height = paper − margins (hard margins + the extra margin on every side).
pub fn printable_area(page: &PageInfo, job: &JobSettings) -> RectMm {
    if job.ignore_hard_margins && page.bleed && job.extra_margin_mm <= 0.0 {
        let b = BLEED_MM;
        return RectMm::new(-b, -b, page.width_mm + 2.0 * b, page.height_mm + 2.0 * b);
    }
    let hard = if job.ignore_hard_margins { [0.0; 4] } else { page.hard };
    let e = job.extra_margin_mm.max(0.0);
    let (l, t, r, b) = (hard[0] + e, hard[1] + e, hard[2] + e, hard[3] + e);
    RectMm::new(l, t, (page.width_mm - l - r).max(1.0), (page.height_mm - t - b).max(1.0))
}

/// Scale factor for one mode.
pub fn scale_factor(mode: ScaleMode, w: f32, h: f32, aw: f32, ah: f32, zoom: u32) -> f32 {
    if w <= 0.0 || h <= 0.0 {
        return 1.0;
    }
    let fit = (aw / w).min(ah / h);
    match mode {
        ScaleMode::Shrink => {
            if w > aw || h > ah {
                fit
            } else {
                1.0
            }
        }
        ScaleMode::Fit => fit,
        ScaleMode::Fill => (aw / w).max(ah / h),
        ScaleMode::AsIs => 1.0,
        ScaleMode::Zoom => zoom as f32 / 100.0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Clockwise turn of the photo, in degrees: 0, 90, 180 or 270.
    pub rot: i32,
    /// Where the (turned) photo is drawn, in page millimetres. May reach beyond `clip`.
    pub rect: RectMm,
    /// The part of the page the photo may cover.
    pub clip: RectMm,
}

/// The photo's own size in mm when printed at 100 %.
pub fn native_size_mm(item: &Item) -> (f32, f32) {
    let dpi = item.dpi.filter(|d| *d >= 10.0).unwrap_or(DEFAULT_DPI);
    let (w, h) = if item.px.0 > 0 { item.px } else { (1500, 1000) };
    (w as f32 / dpi * 25.4, h as f32 / dpi * 25.4)
}

/// Pixels per inch assumed when a file doesn't state its resolution (the usual Windows default).
pub const DEFAULT_DPI: f32 = 96.0;

/// Place one photo of native size `size` (mm) inside `area`.
pub fn place_photo(size: (f32, f32), area: RectMm, s: &PhotoSettings, fine: Option<(f32, f32)>) -> Placement {
    let (mut w, mut h) = size;
    let mut rot = 0;
    if s.auto_rotate != AutoRotate::None {
        // Turn when the orientations differ and neither is square.
        if w != h && area.w != area.h && (w > h) != (area.w > area.h) {
            rot = if s.auto_rotate == AutoRotate::Clockwise { 90 } else { 270 };
            std::mem::swap(&mut w, &mut h);
        }
    } else {
        rot = s.rotation.rem_euclid(360) / 90 * 90;
        if rot == 90 || rot == 270 {
            std::mem::swap(&mut w, &mut h);
        }
    }
    let (fx, fy) = fine.unwrap_or((s.h_align.fraction(), s.v_align.fraction()));
    let k = scale_factor(s.scale, w, h, area.w, area.h, s.zoom);
    w *= k;
    h *= k;
    let mut x = s.offset_x_mm + (area.w - w) * fx;
    let mut y = s.offset_y_mm + (area.h - h) * fy;
    // Offset re-fit: when the offset pushes the photo out of the area, fit it into what is left
    // (only for Shrink and Fit).
    if matches!(s.scale, ScaleMode::Shrink | ScaleMode::Fit) && (y + h > area.h || x + w > area.w) {
        let rem_h = if area.h - y > 0.0 { area.h - y } else { area.h };
        let rem_w = if area.w - x > 0.0 { area.w - x } else { area.w };
        let k2 = scale_factor(s.scale, w, h, rem_w, rem_h, s.zoom);
        w *= k2;
        h *= k2;
        x += (rem_w - w) * fx;
        y += (rem_h - h) * fy;
    }
    Placement { rot, rect: RectMm::new(area.x + x, area.y + y, w, h), clip: area }
}

/// Size of the photo after its manual turn (used to decide "Passend zum Foto").
pub fn turned_size(size: (f32, f32), s: &PhotoSettings) -> (f32, f32) {
    let r = s.rotation.rem_euclid(360);
    if s.auto_rotate == AutoRotate::None && (r == 90 || r == 270) {
        (size.1, size.0)
    } else {
        size
    }
}

/// Which way the paper should lie: Some(true) = landscape, None = as the printer is set.
pub fn sheet_landscape(o: Orientation, photo: (f32, f32)) -> Option<bool> {
    match o {
        Orientation::AsPrinter => None,
        Orientation::Portrait => Some(false),
        Orientation::Landscape => Some(true),
        Orientation::ByPhoto => Some(photo.0 > photo.1),
    }
}

#[derive(Clone, Debug)]
pub struct Cell {
    pub item: usize,
    pub placement: Placement,
}

#[derive(Clone, Debug)]
pub struct Sheet {
    pub paper: Option<i16>,
    pub tray: Option<i16>,
    /// Paper type (driver media type); None = as printer.
    pub media: Option<u32>,
    pub landscape: Option<bool>,
    pub settings_of: usize,
    pub page: PageInfo,
    pub cells: Vec<Cell>,
}

/// Grid cells for several photos per sheet: the printable area split into columns × rows, each
/// cell inset by the gap (a percentage of the longer paper side).
pub fn collage_cells(page: &PageInfo, job: &JobSettings) -> Vec<RectMm> {
    let area = printable_area(page, job);
    let cols = job.cols.max(1);
    let rows = job.rows.max(1);
    let longer = page.width_mm.max(page.height_mm);
    let gap = job.gap_pct.max(0.0) * longer / 100.0;
    let cw = area.w / cols as f32;
    let ch = area.h / rows as f32;
    let mut out = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let cell = RectMm::new(area.x + c as f32 * cw, area.y + r as f32 * ch, cw, ch);
            out.push(if cols * rows > 1 { cell.inset(gap.min(cw / 3.0), gap.min(ch / 3.0)) } else { cell });
        }
    }
    out
}

/// Turn the list into printed sheets. `page_of` answers the size and margins of the paper a
/// photo's settings ask for (paper, tray, paper type).
pub fn plan_sheets(
    items: &[Item],
    order: &[usize],
    job: &JobSettings,
    page_of: &mut dyn FnMut(&PhotoSettings, Option<bool>) -> PageInfo,
) -> Vec<Sheet> {
    let mut sheets = Vec::new();
    if job.collage() {
        let mut current: Option<(Sheet, Vec<RectMm>)> = None;
        let mut last_item = None;
        for &i in order {
            let item = &items[i];
            let s = item.settings(job);
            for _ in 0..s.copies.max(1) {
                let new_needed = match &current {
                    None => true,
                    Some((sh, cells)) => {
                        sh.cells.len() >= cells.len() || (job.next_photo_new_sheet && last_item != Some(i))
                    }
                };
                if new_needed {
                    if let Some((sh, _)) = current.take() {
                        sheets.push(sh);
                    }
                    let g = &job.photo;
                    let landscape = sheet_landscape(g.orientation, turned_size(native_size_mm(item), s));
                    let page = page_of(g, landscape);
                    let cells = collage_cells(&page, job);
                    current = Some((
                        Sheet { paper: g.paper, tray: g.tray, media: g.media, landscape, settings_of: i, page, cells: Vec::new() },
                        cells,
                    ));
                }
                let (sh, cells) = current.as_mut().unwrap();
                let area = cells[sh.cells.len()];
                let placement = place_photo(native_size_mm(item), area, s, item.fine);
                sh.cells.push(Cell { item: i, placement });
                last_item = Some(i);
            }
        }
        if let Some((sh, _)) = current {
            sheets.push(sh);
        }
    } else {
        for &i in order {
            let item = &items[i];
            let s = item.settings(job);
            let size = native_size_mm(item);
            let landscape = sheet_landscape(s.orientation, turned_size(size, s));
            let page = page_of(s, landscape);
            let area = printable_area(&page, job);
            let placement = place_photo(size, area, s, item.fine);
            for _ in 0..s.copies.max(1) {
                sheets.push(Sheet {
                    paper: s.paper,
                    tray: s.tray,
                    media: s.media,
                    landscape,
                    settings_of: i,
                    page,
                    cells: vec![Cell { item: i, placement }],
                });
            }
        }
    }
    sheets
}

/// 1/100 inch in mm – the unit of the offsets and cut-mark distances.
pub const HUNDREDTH_INCH_MM: f32 = 0.254;

/// Cut-mark lines around a rectangle: 21/100 inch long, starting 9/100 inch outside the photo,
/// moved inwards by `offset` (in 1/100 inch).
pub fn cut_mark_lines(r: &RectMm, offset: f32) -> Vec<[(f32, f32); 2]> {
    let u = HUNDREDTH_INCH_MM;
    let gap = 9.0 * u;
    let len = 21.0 * u;
    let o = offset * u;
    let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
    vec![
        [(x0 - gap - len, y0 + o), (x0 - gap, y0 + o)],
        [(x0 - gap - len, y1 - o), (x0 - gap, y1 - o)],
        [(x1 + gap + len, y0 + o), (x1 + gap, y0 + o)],
        [(x1 + gap + len, y1 - o), (x1 + gap, y1 - o)],
        [(x0 + o, y0 - gap - len), (x0 + o, y0 - gap)],
        [(x0 + o, y1 + gap + len), (x0 + o, y1 + gap)],
        [(x1 - o, y0 - gap - len), (x1 - o, y0 - gap)],
        [(x1 - o, y1 + gap + len), (x1 - o, y1 + gap)],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn s(mode: ScaleMode) -> PhotoSettings {
        PhotoSettings { scale: mode, auto_rotate: AutoRotate::None, ..Default::default() }
    }

    #[test]
    fn fit_keeps_whole_photo() {
        let p = place_photo((300.0, 200.0), RectMm::new(0.0, 0.0, 150.0, 150.0), &s(ScaleMode::Fit), None);
        assert!((p.rect.w - 150.0).abs() < 1e-3 && (p.rect.h - 100.0).abs() < 1e-3);
        assert!((p.rect.y - 25.0).abs() < 1e-3);
    }

    #[test]
    fn fill_covers_area() {
        let p = place_photo((300.0, 200.0), RectMm::new(0.0, 0.0, 150.0, 150.0), &s(ScaleMode::Fill), None);
        assert!((p.rect.h - 150.0).abs() < 1e-3 && p.rect.w > 150.0);
    }

    #[test]
    fn shrink_leaves_small_photo() {
        let p = place_photo((50.0, 40.0), RectMm::new(0.0, 0.0, 150.0, 150.0), &s(ScaleMode::Shrink), None);
        assert_eq!((p.rect.w, p.rect.h), (50.0, 40.0));
    }

    #[test]
    fn auto_rotate_turns_landscape_photo_on_portrait_area() {
        let mut st = s(ScaleMode::Fit);
        st.auto_rotate = AutoRotate::Clockwise;
        let p = place_photo((150.0, 100.0), RectMm::new(0.0, 0.0, 100.0, 150.0), &st, None);
        assert_eq!(p.rot, 90);
        assert!((p.rect.w - 100.0).abs() < 1e-3 && (p.rect.h - 150.0).abs() < 1e-3);
    }
}

/// The printer keeps a white edge although borderless was asked for: the widest margin in mm.
pub fn borderless_refused(page: &PageInfo, job: &JobSettings) -> Option<f32> {
    let widest = page.hard.iter().cloned().fold(0.0f32, f32::max);
    (job.ignore_hard_margins && widest > 0.3).then_some(widest)
}

/// Pixels per inch a photo will have on paper (the lower of both directions). Below about
/// 150 dpi prints start to look soft.
pub fn effective_dpi(item: &Item, pl: &Placement) -> Option<f32> {
    if item.px.0 == 0 || pl.rect.w <= 0.0 || pl.rect.h <= 0.0 {
        return None;
    }
    let (pw, ph) = if pl.rot == 90 || pl.rot == 270 { (item.px.1, item.px.0) } else { item.px };
    let dx = pw as f32 / (pl.rect.w / 25.4);
    let dy = ph as f32 / (pl.rect.h / 25.4);
    Some(dx.min(dy))
}

/// Below this many pixels per inch a print is flagged as possibly blurry (common rule of thumb
/// for photo prints).
pub const LOW_DPI: f32 = 150.0;

#[cfg(test)]
mod dpi_tests {
    use super::*;
    #[test]
    fn effective_dpi_of_a_10x15_print() {
        let mut it = Item::new(std::path::PathBuf::from("x.jpg"));
        it.px = (1800, 1200);
        let pl = Placement { rot: 0, rect: RectMm::new(0.0, 0.0, 152.4, 101.6), clip: RectMm::new(0.0, 0.0, 152.4, 101.6) };
        assert!((effective_dpi(&it, &pl).unwrap() - 300.0).abs() < 0.5);
        let turned = Placement { rot: 90, rect: RectMm::new(0.0, 0.0, 101.6, 152.4), clip: pl.clip };
        assert!((effective_dpi(&it, &turned).unwrap() - 300.0).abs() < 0.5);
    }
}

#[cfg(test)]
mod sheet_tests {
    use super::*;
    use crate::model::*;

    #[test]
    fn four_per_sheet_puts_four_photos_on_one_sheet() {
        let items: Vec<Item> = (0..9).map(|i| {
            let mut it = Item::new(std::path::PathBuf::from(format!("{i}.jpg")));
            it.px = (1800, 1200);
            it
        }).collect();
        let order: Vec<usize> = (0..9).collect();
        let job = JobSettings { cols: 2, rows: 2, ..Default::default() };
        let sheets = plan_sheets(&items, &order, &job, &mut |_, _| PageInfo::plain(297.0, 210.0, 300.0));
        assert_eq!(sheets.iter().map(|s| s.cells.len()).collect::<Vec<_>>(), vec![4, 4, 1]);
        let apart = JobSettings { next_photo_new_sheet: true, ..job };
        assert_eq!(plan_sheets(&items, &order, &apart, &mut |_, _| PageInfo::plain(297.0, 210.0, 300.0)).len(), 9);
    }
}
