// Builds the program icon (several sizes in one .ico) from src/logo.rs and puts it, with the
// program's name and version, into the Windows .exe so Explorer shows it.

include!("src/logo.rs");

fn ico_bytes(sizes: &[u32]) -> Vec<u8> {
    let mut images: Vec<Vec<u8>> = Vec::new();
    for &s in sizes {
        let rgba = logo_rgba(s);
        let n = s as usize;
        let mask_row = (n.div_ceil(32)) * 4;
        let mut b = Vec::with_capacity(40 + n * n * 4 + mask_row * n);
        // BITMAPINFOHEADER (height counts the colour and mask halves)
        b.extend_from_slice(&40u32.to_le_bytes());
        b.extend_from_slice(&(s as i32).to_le_bytes());
        b.extend_from_slice(&((s * 2) as i32).to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&32u16.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&((n * n * 4 + mask_row * n) as u32).to_le_bytes());
        b.extend_from_slice(&[0u8; 16]);
        for y in (0..n).rev() {
            for x in 0..n {
                let i = (y * n + x) * 4;
                b.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
            }
        }
        b.extend(std::iter::repeat_n(0u8, mask_row * n));
        images.push(b);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len();
    for (k, &s) in sizes.iter().enumerate() {
        let d = if s >= 256 { 0u8 } else { s as u8 };
        out.extend_from_slice(&[d, d, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(images[k].len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += images[k].len();
    }
    for img in images {
        out.extend_from_slice(&img);
    }
    out
}

fn main() {
    println!("cargo:rerun-if-changed=src/logo.rs");
    println!("cargo:rerun-if-changed=build.rs");
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = out_dir.join("mr-photoprint.ico");
    std::fs::write(&ico, ico_bytes(&[16, 20, 24, 32, 40, 48, 64, 128, 256])).unwrap();

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico.to_str().unwrap());
        res.set("ProductName", "Mr. PhotoPrint");
        res.set("FileDescription", "Mr. PhotoPrint – print many photos, each fitted to the paper");
        res.set("InternalName", "MrPhotoPrint");
        res.set("OriginalFilename", "MrPhotoPrint.exe");
        res.set("CompanyName", "");
        res.set("LegalCopyright", "MIT OR Apache-2.0");
        if let Err(e) = res.compile() {
            // The program still builds; only the Explorer icon would be missing.
            println!("cargo:warning=Icon could not be embedded: {e}");
        }
    }
}
