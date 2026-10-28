//! Renders what a PlayStation disc looks like, for a designer's eye: with cover
//! art, without, and at the angles it passes through as the slot takes it.
//!
//!     cargo run -p slot-ui --example disc_preview -- out.png [covers_dir]
//!
//! By default the cover is generated here, so nothing copyrighted is involved. Give a
//! folder of PNGs to see how your own cover art sits on a disc instead.

use slot_store::scan;
use slot_ui::{cart_face, disc_glare, disc_spin, CartFace, DISC_D};

const HOUSING: [u8; 3] = [0x24, 0x24, 0x29];
const WALL: [u8; 3] = [0xb9, 0xa7, 0x82];

/// A made-up cover: sky, a sun, hills and a diagonal title band.
fn cover(size: u32) -> Vec<u8> {
    let mut rgb = Vec::with_capacity((size * size * 3) as usize);
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 / size as f32, y as f32 / size as f32);
            let mut c = [
                (40.0 + 90.0 * fy) as u8,
                (90.0 + 110.0 * fy) as u8,
                (200.0 - 60.0 * fy) as u8,
            ];
            let (sx, sy) = (fx - 0.68, fy - 0.3);
            if (sx * sx + sy * sy).sqrt() < 0.13 {
                c = [255, 214, 90];
            }
            if fy > 0.62 + 0.06 * (fx * 9.0).sin() {
                c = [38, 120, 70];
            }
            if ((fx + fy) - 1.0).abs() < 0.085 {
                c = [200, 30, 50];
            }
            rgb.extend_from_slice(&c);
        }
    }
    rgb
}

fn write_png(path: &std::path::Path, w: u32, h: u32, rgb: &[u8]) {
    let f = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .expect("header")
        .write_image_data(rgb)
        .expect("data");
}

fn face_of(root: &std::path::Path, with_art: bool, name: &str) -> CartFace {
    let games = root.join("Games/PS1");
    std::fs::create_dir_all(&games).unwrap();
    std::fs::write(games.join(format!("{name}.cue")), "").unwrap();
    if with_art {
        std::fs::create_dir_all(root.join("Labels")).unwrap();
        write_png(
            &root.join("Labels").join(format!("{name}.png")),
            500,
            500,
            &cover(500),
        );
    }
    let carts = scan(root).unwrap();
    let cart = carts.iter().find(|c| c.stem == name).expect("the cart");
    cart_face(cart)
}

/// The face turned by `angle` about its centre, over `bg`, as the sprite shader
/// would draw it.
fn drawn(face: &CartFace, glare: &CartFace, angle: f32, bg: [u8; 3], size: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((size * size * 3) as usize);
    let (s, c) = angle.sin_cos();
    let mid = size as f32 / 2.0;
    let k = face.w as f32 / size as f32;
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 + 0.5 - mid, y as f32 + 0.5 - mid);
            // Inverse turn, back into the face.
            let (u, v) = (c * dx + s * dy, -s * dx + c * dy);
            let (fx, fy) = (
                (u * k + face.w as f32 / 2.0) as i32,
                (v * k + face.h as f32 / 2.0) as i32,
            );
            let mut px = bg;
            if (0..face.w as i32).contains(&fx) && (0..face.h as i32).contains(&fy) {
                let i = ((fy as u32 * face.w + fx as u32) * 4) as usize;
                let a = face.rgba[i + 3] as f32 / 255.0;
                for ch in 0..3 {
                    px[ch] = (face.rgba[i + ch] as f32 * a + bg[ch] as f32 * (1.0 - a)) as u8;
                }
            }
            // The glare does not turn with the disc.
            let (gx, gy) = (
                ((dx * k) + glare.w as f32 / 2.0) as i32,
                ((dy * k) + glare.h as f32 / 2.0) as i32,
            );
            if (0..glare.w as i32).contains(&gx) && (0..glare.h as i32).contains(&gy) {
                let i = ((gy as u32 * glare.w + gx as u32) * 4) as usize;
                let a = glare.rgba[i + 3] as f32 / 255.0;
                for v in px.iter_mut().take(3) {
                    *v = (255.0 * a + *v as f32 * (1.0 - a)) as u8;
                }
            }
            out.extend_from_slice(&px);
        }
    }
    out
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: disc_preview <out.png> [covers_dir]");
    if let Some(dir) = std::env::args().nth(2) {
        return covers_sheet(&out, std::path::Path::new(&dir));
    }
    let root = tempfile::tempdir().unwrap();
    let art = face_of(root.path(), true, "Sky Racer");
    let bare = face_of(root.path(), false, "Midnight Garden Adventures");
    assert_eq!((art.w, art.h), (DISC_D, DISC_D));

    let glare = disc_glare();
    let cell = 240u32;
    let pad = 12u32;
    let rows: [(&CartFace, Vec<f32>, [u8; 3]); 3] = [
        (&art, vec![0.0, 0.0], HOUSING),
        (&bare, vec![0.0, 0.0], HOUSING),
        (
            &art,
            vec![
                disc_spin(0.0),
                disc_spin(0.25),
                disc_spin(0.5),
                disc_spin(0.75),
            ],
            WALL,
        ),
    ];
    let cols = 4u32;
    let (w, h) = (
        cols * (cell + pad) + pad,
        rows.len() as u32 * (cell + pad) + pad,
    );
    let mut sheet = vec![0x50u8; (w * h * 3) as usize];
    for (r, (face, angles, bg)) in rows.iter().enumerate() {
        for (c, angle) in angles.iter().enumerate() {
            let tile = drawn(face, &glare, *angle, *bg, cell);
            let (ox, oy) = (pad + c as u32 * (cell + pad), pad + r as u32 * (cell + pad));
            for y in 0..cell {
                let src = (y * cell * 3) as usize;
                let dst = (((oy + y) * w + ox) * 3) as usize;
                sheet[dst..dst + (cell * 3) as usize]
                    .copy_from_slice(&tile[src..src + (cell * 3) as usize]);
            }
        }
    }
    write_png(std::path::Path::new(&out), w, h, &sheet);
    println!("wrote {out} ({w}x{h})");
}

/// One disc per PNG in `dir`, each named after its game, on the housing colour, so
/// real cover art can be judged on the circle before it goes on a card.
fn covers_sheet(out: &str, dir: &std::path::Path) {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("Labels/PS1")).unwrap();
    let mut names = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let path = e.path();
        if path.extension().is_some_and(|x| x == "png") {
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            std::fs::copy(
                &path,
                root.path().join("Labels/PS1").join(format!("{stem}.png")),
            )
            .unwrap();
            names.push(stem);
        }
    }
    names.sort();
    let glare = disc_glare();
    let (cell, pad) = (300u32, 12u32);
    let (w, h) = (names.len() as u32 * (cell + pad) + pad, cell + 2 * pad);
    let mut sheet = vec![0x24u8; (w * h * 3) as usize];
    for (i, name) in names.iter().enumerate() {
        let games = root.path().join("Games/PS1");
        std::fs::create_dir_all(&games).unwrap();
        std::fs::write(games.join(format!("{name}.cue")), "").unwrap();
        let carts = scan(root.path()).unwrap();
        let cart = carts.iter().find(|c| &c.stem == name).unwrap();
        println!("{name}: label paired = {}", cart.label.is_some());
        let face = cart_face(cart);
        let tile = drawn(&face, &glare, 0.0, HOUSING, cell);
        let ox = pad + i as u32 * (cell + pad);
        for y in 0..cell {
            let src = (y * cell * 3) as usize;
            let dst = (((pad + y) * w + ox) * 3) as usize;
            sheet[dst..dst + (cell * 3) as usize]
                .copy_from_slice(&tile[src..src + (cell * 3) as usize]);
        }
    }
    write_png(std::path::Path::new(out), w, h, &sheet);
    println!("wrote {out} ({w}x{h})");
}
