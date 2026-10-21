//! Draws the app's icon, `apps/windows/lumenna.ico`, from its shapes: a near-black rounded
//! square, an amber disc, and a check cut into it, on a 256 grid. GTK's SVG
//! (`apps/gtk/data/icons/hicolor/scalable/apps/io.github.emassey0135.Lumenna.svg`) is the same
//! shapes, so change both together.
//!
//! `cargo run -p lumenna-windows --example icon`. Each size is drawn at that size, eight samples
//! a side to a pixel, rather than scaled down from the largest, and stored as a PNG.

use std::io::Write;

/// The sizes Windows asks an icon for: the title bar, taskbar and notification area at each
/// scale (16 to 40), Alt+Tab and Explorer (48, 64), and the largest views (256).
const SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

const DARK: [f64; 3] = [0x14 as f64, 0x18 as f64, 0x2E as f64];
const AMBER: [f64; 3] = [0xFF as f64, 0xC8 as f64, 0x3D as f64];

/// The square: x and y from 8 to 248, its corners rounded by 48.
const SQUARE: (f64, f64, f64) = (8.0, 248.0, 48.0);
/// The disc: its centre and radius.
const DISC: (f64, f64, f64) = (128.0, 128.0, 88.0);
/// The check: a line through these points, 26 wide, rounded at its ends and its corner.
const CHECK: [(f64, f64); 3] = [(84.0, 132.0), (114.0, 162.0), (172.0, 100.0)];
const CHECK_WIDTH: f64 = 26.0;

const SAMPLES: u32 = 8;

fn main() {
    let images: Vec<(u32, Vec<u8>)> = SIZES.iter().map(|&size| (size, png(size, &draw(size)))).collect();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("lumenna.ico");
    std::fs::write(&path, ico(&images)).expect("the icon is written");
    println!("{}", path.display());
}

/// What colour, if any, a point on the 256 grid is.
fn colour_at(x: f64, y: f64) -> Option<[f64; 3]> {
    let (low, high, radius) = SQUARE;
    let centre = f64::midpoint(low, high);
    let half = (high - low) / 2.0 - radius;
    let (qx, qy) = ((x - centre).abs() - half, (y - centre).abs() - half);
    let outside = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0);
    if outside > radius {
        return None;
    }
    let (cx, cy, r) = DISC;
    let on_check = CHECK.windows(2).any(|line| distance_to_segment((x, y), line[0], line[1]) <= CHECK_WIDTH / 2.0);
    Some(if (x - cx).hypot(y - cy) <= r && !on_check { AMBER } else { DARK })
}

fn distance_to_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    (p.0 - (a.0 + t * dx)).hypot(p.1 - (a.1 + t * dy))
}

/// The icon at `size` pixels a side, as RGBA rows.
fn draw(size: u32) -> Vec<u8> {
    let scale = 256.0 / f64::from(size);
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (mut sum, mut covered) = ([0.0; 3], 0u32);
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let px = (f64::from(x) + (f64::from(sx) + 0.5) / f64::from(SAMPLES)) * scale;
                    let py = (f64::from(y) + (f64::from(sy) + 0.5) / f64::from(SAMPLES)) * scale;
                    if let Some(colour) = colour_at(px, py) {
                        covered += 1;
                        (0..3).for_each(|i| sum[i] += colour[i]);
                    }
                }
            }
            if covered == 0 {
                pixels.extend([0, 0, 0, 0]);
            } else {
                pixels.extend(sum.map(|c| (c / f64::from(covered)).round() as u8));
                pixels.push((255.0 * f64::from(covered) / f64::from(SAMPLES * SAMPLES)).round() as u8);
            }
        }
    }
    pixels
}

fn png(size: u32, pixels: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(pixels.len() + size as usize);
    for row in pixels.chunks(size as usize * 4) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    zlib.write_all(&raw).expect("compressed");
    let data = zlib.finish().expect("compressed");

    let mut header = Vec::new();
    header.extend(size.to_be_bytes());
    header.extend(size.to_be_bytes());
    // Eight bits a channel, RGBA, deflate, no filter method of its own, not interlaced.
    header.extend([8, 6, 0, 0, 0]);

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, body) in [(b"IHDR", header), (b"IDAT", data), (b"IEND", Vec::new())] {
        out.extend((body.len() as u32).to_be_bytes());
        let mut crc = flate2::Crc::new();
        crc.update(kind);
        crc.update(&body);
        out.extend(kind);
        out.extend(&body);
        out.extend(crc.sum().to_be_bytes());
    }
    out
}

/// An icon file holding each image as a PNG, which Windows reads at any size.
fn ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(0u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend((images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, data) in images {
        // 256 is written as 0.
        let side = if *size >= 256 { 0 } else { *size as u8 };
        out.extend([side, side, 0, 0]);
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for (_, data) in images {
        out.extend(data);
    }
    out
}
