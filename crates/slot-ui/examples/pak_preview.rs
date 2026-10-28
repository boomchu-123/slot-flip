//! Renders the NES and SNES cartridges for a designer's eye: with no art (a generated label)
//! and with whatever PNGs are in the folders given, one Pak per file.
//!
//!     cargo run -p slot-ui --example pak_preview -- out.png [art_dir ...]
//!
//! Nothing copyrighted is needed: with no folder it shows generated labels only.

use slot_store::scan;
use slot_ui::{cart_face, CartFace};

const HOUSING: [u8; 3] = [0x24, 0x24, 0x29];

fn face_of(root: &std::path::Path, name: &str) -> CartFace {
    let (dir, ext) = match name.starts_with("snes-") {
        true => ("SNES", "sfc"),
        false => ("NES", "nes"),
    };
    let games = root.join("Games").join(dir);
    std::fs::create_dir_all(&games).unwrap();
    std::fs::write(games.join(format!("{name}.{ext}")), "").unwrap();
    let carts = scan(root).unwrap();
    cart_face(carts.iter().find(|c| c.stem == name).unwrap())
}

fn write_png(path: &str, w: u32, h: u32, rgb: &[u8]) {
    let f = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgb).unwrap();
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args
        .next()
        .expect("usage: pak_preview <out.png> [art_dir ...]");
    let dirs: Vec<String> = args.collect();
    let root = tempfile::tempdir().unwrap();
    let mut faces = vec![
        face_of(root.path(), "Sky Racer"),
        face_of(root.path(), "Midnight Garden Adventures Part Two"),
        face_of(root.path(), "snes-Sky Racer"),
        face_of(root.path(), "snes-Midnight Garden Adventures Part Two"),
    ];
    for (n, dir) in dirs.iter().enumerate() {
        let sub = "Labels/NES".to_string();
        std::fs::create_dir_all(root.path().join(&sub)).unwrap();
        let mut pngs: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "png"))
            .collect();
        pngs.sort();
        for png in pngs {
            let stem = format!("{}-{}", n, png.file_stem().unwrap().to_string_lossy());
            std::fs::copy(&png, root.path().join(&sub).join(format!("{stem}.png"))).unwrap();
            faces.push(face_of(root.path(), &stem));
        }
    }
    let (pad, cell_w, cell_h) = (14u32, 340u32, 332u32);
    let w = faces.len() as u32 * (cell_w + pad) + pad;
    let h = cell_h + 2 * pad;
    let mut sheet = HOUSING.repeat((w * h) as usize);
    for (i, face) in faces.iter().enumerate() {
        let ox = pad + i as u32 * (cell_w + pad);
        for y in 0..face.h {
            for x in 0..face.w {
                let s = ((y * face.w + x) * 4) as usize;
                let a = face.rgba[s + 3] as f32 / 255.0;
                let d = (((pad + y) * w + ox + x) * 3) as usize;
                for c in 0..3 {
                    sheet[d + c] =
                        (face.rgba[s + c] as f32 * a + sheet[d + c] as f32 * (1.0 - a)) as u8;
                }
            }
        }
    }
    write_png(&out, w, h, &sheet);
    println!("wrote {out} ({w}x{h}, {} paks)", faces.len());
}
