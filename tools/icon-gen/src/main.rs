//! Regenerates the app icons from the SVG sources in `assets/icon`.
//!
//! ```sh
//! cargo run --release --manifest-path tools/icon-gen/Cargo.toml -- assets/icon
//! ```
//!
//! Writes `nswpedia-search.ico` (Windows), `nswpedia-search.icns` (macOS) and
//! `icon-256.png` (README/preview). Output is deterministic, so regenerated
//! files only change when an SVG changes.

use resvg::{tiny_skia, usvg};
use std::{env, fs, path::Path};

/// Sizes at or below this use `icon-small.svg`, which drops fine detail.
const SMALL_MAX: u32 = 32;

fn render(svg: &Path, size: u32) -> Vec<u8> {
    let data = fs::read(svg).unwrap_or_else(|e| panic!("{}: {e}", svg.display()));
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).expect("valid SVG");
    let mut pixmap = tiny_skia::Pixmap::new(size, size).unwrap();
    let scale = size as f32 / tree.size().width();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().expect("PNG encoding")
}

/// ICO with PNG-compressed entries (supported by Windows Vista and later).
fn ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend([0u8, 0, 1, 0]);
    out.extend((images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, png) in images {
        let dim = if *size >= 256 { 0 } else { *size as u8 };
        out.extend([dim, dim, 0, 0]);
        out.extend(1u16.to_le_bytes()); // planes
        out.extend(32u16.to_le_bytes()); // bits per pixel
        out.extend((png.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in images {
        out.extend(png);
    }
    out
}

/// ICNS with PNG payloads for every slot macOS 11+ uses.
fn icns(images: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let body: usize = images.iter().map(|(_, png)| 8 + png.len()).sum();
    let mut out = Vec::with_capacity(8 + body);
    out.extend(b"icns");
    out.extend(((8 + body) as u32).to_be_bytes());
    for (kind, png) in images {
        out.extend(*kind);
        out.extend(((8 + png.len()) as u32).to_be_bytes());
        out.extend(png);
    }
    out
}

fn main() {
    let dir = env::args().nth(1).unwrap_or_else(|| "assets/icon".into());
    let dir = Path::new(&dir);
    let (full, small, mac) = (
        dir.join("icon.svg"),
        dir.join("icon-small.svg"),
        dir.join("icon-macos.svg"),
    );

    let windows: Vec<_> = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]
        .into_iter()
        .map(|size| {
            (
                size,
                render(if size <= SMALL_MAX { &small } else { &full }, size),
            )
        })
        .collect();
    fs::write(dir.join("nswpedia-search.ico"), ico(&windows)).unwrap();

    let mac_png = |size| render(if size <= SMALL_MAX { &small } else { &mac }, size);
    let slots: [(&[u8; 4], u32); 10] = [
        (b"icp4", 16),
        (b"icp5", 32),
        (b"ic11", 32),
        (b"icp6", 64),
        (b"ic12", 64),
        (b"ic07", 128),
        (b"ic08", 256),
        (b"ic13", 256),
        (b"ic09", 512),
        (b"ic14", 512),
    ];
    let mut images: Vec<_> = slots
        .iter()
        .map(|(kind, size)| (*kind, mac_png(*size)))
        .collect();
    images.push((b"ic10", mac_png(1024)));
    fs::write(dir.join("nswpedia-search.icns"), icns(&images)).unwrap();

    fs::write(dir.join("icon-256.png"), render(&full, 256)).unwrap();
    println!("Wrote icons to {}", dir.display());
}
