// The "Mr. PhotoPrint" logo, drawn from simple shapes so it is sharp at every size.
// Used by build.rs (the icon inside the .exe) and by the app (window and title bar).
// Plain comments only: build.rs includes this file with include!().

// A gradient squircle (indigo -> cyan), a tilted white photo print with mountains and a sun,
// and a gentleman's top hat on its corner.

#[allow(dead_code)]
pub fn logo_rgba(size: u32) -> Vec<u8> {
    let n = size as usize;
    let mut out = vec![0u8; n * n * 4];
    let ss = if size <= 64 { 6 } else { 4 }; // supersampling per axis
    for py in 0..n {
        for px in 0..n {
            let mut acc = [0f32; 4]; // premultiplied
            for sy in 0..ss {
                for sx in 0..ss {
                    let x = (px as f32 + (sx as f32 + 0.5) / ss as f32) / size as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / ss as f32) / size as f32;
                    let c = logo_sample(x, y, size);
                    for k in 0..4 {
                        acc[k] += c[k];
                    }
                }
            }
            let cnt = (ss * ss) as f32;
            let a = acc[3] / cnt;
            let i = (py * n + px) * 4;
            if a > 0.0 {
                for k in 0..3 {
                    out[i + k] = ((acc[k] / cnt / a).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                }
            }
            out[i + 3] = (a.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }
    out
}

fn lg_rot(x: f32, y: f32, cx: f32, cy: f32, deg: f32) -> (f32, f32) {
    let (s, c) = (deg.to_radians()).sin_cos();
    let (dx, dy) = (x - cx, y - cy);
    (dx * c + dy * s, -dx * s + dy * c)
}

// Signed distance to a rounded box centred at the origin.
fn lg_rbox(x: f32, y: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = x.abs() - hw + r;
    let qy = y.abs() - hh + r;
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

fn lg_in_tri(x: f32, y: f32, a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let s = |p: (f32, f32), q: (f32, f32)| (x - q.0) * (p.1 - q.1) - (p.0 - q.0) * (y - q.1);
    let d1 = s(a, b);
    let d2 = s(b, c);
    let d3 = s(c, a);
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

fn lg_over(dst: &mut [f32; 4], rgb: [f32; 3], a: f32) {
    let a = a.clamp(0.0, 1.0);
    for k in 0..3 {
        dst[k] = rgb[k] * a + dst[k] * (1.0 - a);
    }
    dst[3] = a + dst[3] * (1.0 - a);
}

fn lg_rgb(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

fn lg_mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn logo_sample(x: f32, y: f32, size: u32) -> [f32; 4] {
    let mut c = [0f32; 4];
    let small = size <= 24;
    // Background squircle with a diagonal gradient.
    let bg = lg_rbox(x - 0.5, y - 0.5, 0.47, 0.47, 0.21);
    if bg > 0.0 {
        return c;
    }
    let t = ((x + y) / 2.0).clamp(0.0, 1.0);
    let base = if t < 0.5 {
        lg_mix(lg_rgb(99, 102, 241), lg_rgb(139, 92, 246), t * 2.0)
    } else {
        lg_mix(lg_rgb(139, 92, 246), lg_rgb(34, 211, 238), (t - 0.5) * 2.0)
    };
    lg_over(&mut c, base, 1.0);
    // soft highlight at the top
    let hl = (1.0 - y * 2.2).max(0.0) * 0.18;
    lg_over(&mut c, [1.0, 1.0, 1.0], hl);

    // Photo print, tilted.
    let (cx, cy, ang) = (0.53, 0.58, -9.0);
    let (hw, hh) = (0.30, 0.245);
    let (lx, ly) = lg_rot(x, y, cx, cy, ang);
    // shadow
    let sh = lg_rbox(lx - 0.012, ly - 0.022, hw, hh, 0.035);
    if sh < 0.05 {
        lg_over(&mut c, lg_rgb(30, 27, 75), 0.32 * (1.0 - (sh.max(0.0) / 0.05)));
    }
    let card = lg_rbox(lx, ly, hw, hh, 0.035);
    if card <= 0.0 {
        lg_over(&mut c, [1.0, 1.0, 1.0], 1.0);
        // picture inside the white border
        let (iw, ih) = (hw - 0.04, hh - 0.04);
        if lx.abs() <= iw && ly.abs() <= ih {
            let v = ((ly + ih) / (2.0 * ih)).clamp(0.0, 1.0);
            lg_over(&mut c, lg_mix(lg_rgb(186, 230, 253), lg_rgb(224, 242, 254), v), 1.0);
            // sun
            let (sx, sy) = (iw * 0.45, -ih * 0.35);
            if ((lx - sx).powi(2) + (ly - sy).powi(2)).sqrt() < 0.055 {
                lg_over(&mut c, lg_rgb(251, 191, 36), 1.0);
            }
            // mountains
            if lg_in_tri(lx, ly, (-iw - 0.02, ih), (-iw * 0.25, -ih * 0.35), (iw * 0.45, ih)) {
                lg_over(&mut c, lg_rgb(79, 70, 229), 1.0);
            }
            if lg_in_tri(lx, ly, (-iw * 0.05, ih), (iw * 0.45, -ih * 0.05), (iw + 0.02, ih)) {
                lg_over(&mut c, lg_rgb(14, 165, 233), 1.0);
            }
        }
    }

    // Top hat on the photo's upper left corner (left out at tiny sizes for clarity).
    if !small {
        let (hx, hy, hang) = (0.33, 0.27, -14.0);
        let (ax, ay) = lg_rot(x, y, hx, hy, hang);
        let navy = lg_rgb(17, 24, 39);
        let brim = lg_rbox(ax, ay - 0.105, 0.175, 0.026, 0.022);
        let crown = lg_rbox(ax, ay - 0.0, 0.112, 0.105, 0.03);
        let band = ay > 0.035 && ay < 0.07 && ax.abs() < 0.112;
        let hat_shadow = lg_rbox(ax - 0.01, ay - 0.09, 0.18, 0.04, 0.03);
        if hat_shadow < 0.03 && brim > 0.0 && crown > 0.0 {
            lg_over(&mut c, lg_rgb(30, 27, 75), 0.18 * (1.0 - hat_shadow.max(0.0) / 0.03));
        }
        if brim <= 0.0 || crown <= 0.0 {
            lg_over(&mut c, navy, 1.0);
            if band && crown <= 0.0 {
                lg_over(&mut c, lg_rgb(244, 63, 94), 1.0);
            }
            // a little shine on the crown
            if crown <= 0.0 && ax > -0.085 && ax < -0.06 && ay > -0.08 && ay < 0.02 {
                lg_over(&mut c, [1.0, 1.0, 1.0], 0.22);
            }
        }
    }
    c
}
