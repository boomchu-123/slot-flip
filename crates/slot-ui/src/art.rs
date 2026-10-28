use std::path::Path;

pub fn cover(path: &Path, w: u32, h: u32) -> Option<Vec<u8>> {
    let (src, sw, sh) = decode(path)?;
    cover_rgba(&src, sw, sh, w, h)
}

/// How elongated a picture may be, as width over height, and still be taken for a scan
/// of a cartridge label. Real labels sit well inside this; box art is taller than it
/// is wide and a wheel or banner is far wider, and those are cropped to fill instead.
pub const LABEL_ASPECT: (f32, f32) = (1.0, 2.4);

/// How near, in pixels, a label may come to the room it has before it is simply given
/// all of it, so a scan that is a hair off the recess's shape is not framed by a seam.
const SNAP: u32 = 2;

/// The size a label scan is printed at: its own proportions, as large as fits in
/// `bw`x`bh`, or all of that room when the picture is not shaped like a label.
pub(crate) fn label_box(sw: u32, sh: u32, bw: u32, bh: u32) -> (u32, u32) {
    if sw == 0 || sh == 0 || bw == 0 || bh == 0 {
        return (bw, bh);
    }
    let aspect = sw as f32 / sh as f32;
    if !(LABEL_ASPECT.0..=LABEL_ASPECT.1).contains(&aspect) {
        return (bw, bh);
    }
    let scale = (bw as f32 / sw as f32).min(bh as f32 / sh as f32);
    let fit = |n: u32, room: u32| {
        let v = ((n as f32 * scale).round() as u32).clamp(1, room);
        if room - v <= SNAP {
            room
        } else {
            v
        }
    };
    (fit(sw, bw), fit(sh, bh))
}

/// A label scan fitted to a cartridge's label: it keeps its own shape, so none of it
/// is cropped, and comes back with the size it was printed at.
pub fn label_cover(path: &Path, bw: u32, bh: u32) -> Option<(Vec<u8>, u32, u32)> {
    let (src, sw, sh) = decode(path)?;
    let (w, h) = label_box(sw, sh, bw, bh);
    Some((cover_rgba(&src, sw, sh, w, h)?, w, h))
}

/// Cover art for a disc: the front of a jewel case, fitted to the circle it is
/// printed on. A PlayStation front insert carries the black "PlayStation" spine
/// strip down its left edge, which belongs to the case and not to the disc, so it
/// is trimmed first.
pub fn disc_cover(path: &Path, d: u32) -> Option<Vec<u8>> {
    let (src, sw, sh) = decode(path)?;
    let (src, sw) = trim_spine(src, sw, sh);
    cover_rgba(&src, sw, sh, d, d)
}

/// Width of the spine as a fraction of the cover. The strip is ~54 px on a 512 px
/// cover and its lettering runs to ~64, so this clears the lettering; the artwork
/// starts straight after.
const SPINE: f32 = 0.128;

/// The leading columns, as fractions of the width, that are averaged to call a cover
/// spined, and how black they must be on average. A scan's black edge is ragged
/// (one real cover's columns run 1.00, 1.00, 1.00, 1.00, 0.84), so every column
/// cannot be demanded; but dark artwork such as a night sky is nowhere near this black
/// over the whole height, so an average this high is not mistaken for a spine.
const SPINE_PROBE: (f32, f32) = (0.002, 0.008);
const SPINE_BLACK: f32 = 0.94;

/// Drops a black spine from the left of a cover, if it has one. Nothing else is
/// ever trimmed, so artwork that merely starts dark is left alone.
pub(crate) fn trim_spine(src: Vec<u8>, w: u32, h: u32) -> (Vec<u8>, u32) {
    if w < 64 || h == 0 {
        return (src, w);
    }
    let from = ((w as f32 * SPINE_PROBE.0).floor() as u32).max(1);
    let to = ((w as f32 * SPINE_PROBE.1).ceil() as u32).max(3);
    let black: usize = (from..=to)
        .map(|x| {
            (0..h)
                .filter(|&y| {
                    let i = ((y * w + x) * 4) as usize;
                    src[i..i + 3].iter().all(|&c| c < 26)
                })
                .count()
        })
        .sum();
    let share = black as f32 / ((to - from + 1) * h) as f32;
    if share < SPINE_BLACK {
        return (src, w);
    }
    let cut = (w as f32 * SPINE).round() as u32;
    let kept = w - cut;
    let mut out = Vec::with_capacity((kept * h * 4) as usize);
    for y in 0..h {
        let row = ((y * w + cut) * 4) as usize;
        out.extend_from_slice(&src[row..row + (kept * 4) as usize]);
    }
    (out, kept)
}

pub fn cover_bytes(bytes: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let (src, sw, sh) = decode_png_bytes(bytes)?;
    cover_rgba(&src, sw, sh, w, h)
}

pub(crate) fn cover_rgba(src: &[u8], sw: u32, sh: u32, w: u32, h: u32) -> Option<Vec<u8>> {
    if sw == 0 || sh == 0 {
        return None;
    }

    let scale = (w as f32 / sw as f32).max(h as f32 / sh as f32);
    let ox = (sw as f32 - w as f32 / scale) / 2.0;
    let oy = (sh as f32 - h as f32 / scale) / 2.0;

    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let y0 = oy + y as f32 / scale;
        let y1 = oy + (y + 1) as f32 / scale;
        for x in 0..w {
            let x0 = ox + x as f32 / scale;
            let x1 = ox + (x + 1) as f32 / scale;
            let px = box_average(src, sw, sh, x0, y0, x1, y1);
            out[((y * w + x) * 4) as usize..][..4].copy_from_slice(&px);
        }
    }
    Some(out)
}

fn box_average(src: &[u8], sw: u32, sh: u32, x0: f32, y0: f32, x1: f32, y1: f32) -> [u8; 4] {
    let xa = (x0.floor().max(0.0) as u32).min(sw - 1);
    let ya = (y0.floor().max(0.0) as u32).min(sh - 1);
    let xb = ((x1.ceil() as u32).max(xa + 1)).min(sw);
    let yb = ((y1.ceil() as u32).max(ya + 1)).min(sh);

    let mut acc = [0u32; 4];
    let mut n = 0u32;
    for y in ya..yb {
        for x in xa..xb {
            let i = ((y * sw + x) * 4) as usize;
            for c in 0..4 {
                acc[c] += src[i + c] as u32;
            }
            n += 1;
        }
    }
    let mut out = [0u8; 4];
    for c in 0..4 {
        out[c] = ((acc[c] + n / 2) / n) as u8;
    }
    out
}

pub fn decode(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    let bytes = std::fs::read(path).ok()?;
    decode_png_bytes(&bytes)
}

pub(crate) fn decode_png_bytes(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let orientation = exif_chunk(bytes).and_then(exif_orientation).unwrap_or(1);
    let mut dec = png::Decoder::new(bytes);
    dec.set_transformations(png::Transformations::normalize_to_color8());
    dec.set_limits(png::Limits { bytes: 64 << 20 });

    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let src = &buf[..info.buffer_size()];
    let n = (info.width * info.height) as usize;

    let mut rgba = vec![255u8; n * 4];
    match info.color_type {
        png::ColorType::Rgba => rgba.copy_from_slice(src),
        png::ColorType::Rgb => {
            for i in 0..n {
                rgba[i * 4..i * 4 + 3].copy_from_slice(&src[i * 3..i * 3 + 3]);
            }
        }
        png::ColorType::Grayscale => {
            for i in 0..n {
                rgba[i * 4..i * 4 + 3].fill(src[i]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for i in 0..n {
                rgba[i * 4..i * 4 + 3].fill(src[i * 2]);
                rgba[i * 4 + 3] = src[i * 2 + 1];
            }
        }
        png::ColorType::Indexed => return None,
    }
    Some(orient(rgba, info.width, info.height, orientation))
}

fn exif_chunk(png: &[u8]) -> Option<&[u8]> {
    let mut at = 8;
    loop {
        let len = u32::from_be_bytes(png.get(at..at + 4)?.try_into().ok()?) as usize;
        let kind = png.get(at + 4..at + 8)?;
        match kind {
            b"eXIf" => return png.get(at + 8..at + 8 + len),
            b"IDAT" | b"IEND" => return None,
            _ => at += 12 + len,
        }
    }
}

fn exif_orientation(exif: &[u8]) -> Option<u16> {
    let big = match exif.get(..2)? {
        b"MM" => true,
        b"II" => false,
        _ => return None,
    };
    let u16_at = |i: usize| {
        let b: [u8; 2] = exif.get(i..i + 2)?.try_into().ok()?;
        Some(if big {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    };
    let u32_at = |i: usize| {
        let b: [u8; 4] = exif.get(i..i + 4)?.try_into().ok()?;
        Some(if big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    };
    let ifd = u32_at(4)? as usize;
    let count = u16_at(ifd)? as usize;
    (0..count)
        .map(|n| ifd + 2 + n * 12)
        .find(|&e| u16_at(e) == Some(0x0112))
        .and_then(|e| u16_at(e + 8))
}

fn orient(src: Vec<u8>, w: u32, h: u32, orientation: u16) -> (Vec<u8>, u32, u32) {
    if !(2..=8).contains(&orientation) {
        return (src, w, h);
    }
    let (dw, dh) = if orientation >= 5 { (h, w) } else { (w, h) };
    let mut out = vec![0u8; src.len()];
    for y in 0..dh {
        for x in 0..dw {
            let (sx, sy) = match orientation {
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (y, h - 1 - x),
                7 => (w - 1 - y, h - 1 - x),
                _ => (w - 1 - y, x),
            };
            let s = ((sy * w + sx) * 4) as usize;
            let d = ((y * dw + x) * 4) as usize;
            out[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    (out, dw, dh)
}

pub(crate) fn render_svg(svg: &str, w: u32, h: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    let size = tree.size();
    let scale =
        resvg::tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, scale, &mut pixmap.as_mut());
    let mut rgba = pixmap.data().to_vec();
    unpremultiply(&mut rgba);
    Some(rgba)
}

fn unpremultiply(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3];
        if a == 0 || a == 255 {
            continue;
        }
        for c in &mut px[..3] {
            *c = ((*c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
        }
    }
}
