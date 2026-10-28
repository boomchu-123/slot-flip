//! Renders each console's slot as the app draws it: the back strip, a game part way
//! in, then the front plate over it, dark and light, and the plugged state.
//!
//!     cargo run -p slot-ui --example console_slot_preview -- out_prefix
//!
//! Writes `out_prefix-<console>.png` for every console. The games are generated
//! (a colour and a title), so nothing copyrighted is involved.

use slot_store::{scan, Platform};
use slot_ui::{cart_face, console_slot, seated_box, Console, SLOT_H, SLOT_W};

fn layer(canvas: &mut [f32], top: i32, strip: &[u8]) {
    for y in 0..SLOT_H as i32 {
        for x in 0..SLOT_W as i32 {
            let s = ((y as u32 * SLOT_W + x as u32) * 4) as usize;
            let a = strip[s + 3] as f32 / 255.0;
            if a <= 0.0 {
                continue;
            }
            let d = (((top + y) as u32 * SLOT_W + x as u32) * 3) as usize;
            for c in 0..3 {
                canvas[d + c] = strip[s + c] as f32 * a + canvas[d + c] * (1.0 - a);
            }
        }
    }
}

/// The game, scaled to its seated width, with its foot `foot` below the top of the strip.
fn game(canvas: &mut [f32], platform: Platform, top: i32, foot: i32) {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("Games").join(platform.dir_name());
    std::fs::create_dir_all(&dir).unwrap();
    let ext = platform.extensions()[0];
    std::fs::write(dir.join(format!("Sky Racer.{ext}")), vec![0u8; 0x400]).unwrap();
    let face = cart_face(&scan(root.path()).unwrap()[0]);
    let (sw, sh) = seated_box(platform);
    let (left, y0) = ((SLOT_W as i32 - sw as i32) / 2, top + foot - sh as i32);
    let rows = canvas.len() as i32 / (SLOT_W as i32 * 3);
    for y in 0..sh as i32 {
        for x in 0..sw as i32 {
            let (cy, cx) = (y0 + y, left + x);
            if cy < 0 || cy >= rows || cx < 0 || cx >= SLOT_W as i32 {
                continue;
            }
            let (fx, fy) = (x as u32 * face.w / sw, y as u32 * face.h / sh);
            let i = ((fy * face.w + fx) * 4) as usize;
            let a = face.rgba[i + 3] as f32 / 255.0;
            let d = ((cy as u32 * SLOT_W + cx as u32) * 3) as usize;
            for c in 0..3 {
                canvas[d + c] = canvas[d + c] * (1.0 - a) + face.rgba[i + c] as f32 * a;
            }
        }
    }
}

fn main() {
    let prefix = std::env::args()
        .nth(1)
        .expect("usage: console_slot_preview <out_prefix>");
    let (w, h, gap) = (SLOT_W as usize, 150usize, 8usize);
    let top = 90i32;
    // (light theme, how far the game's foot is below the top of the strip, plugged)
    let frames: [(bool, i32, bool); 4] = [
        (false, 8, false),
        (false, 34, false),
        (true, 34, false),
        (false, 0, true),
    ];
    for (console, platform, name) in [
        (Console::Ps1, Platform::Ps1, "ps1"),
        (Console::Nes, Platform::Nes, "nes"),
        (Console::Snes, Platform::Snes, "snes"),
        (Console::GameBoy, Platform::Gb, "gb"),
        (Console::GameBoyColor, Platform::Gbc, "gbc"),
        (Console::MegaDrive, Platform::Md, "md"),
        (Console::MasterSystem, Platform::Sms, "sms"),
        (Console::GameGear, Platform::Gg, "gg"),
    ] {
        let sheet_h = frames.len() * (h + gap) + gap;
        let mut sheet = vec![0x40u8; w * sheet_h * 3];
        for (r, (light, foot, plugged)) in frames.iter().enumerate() {
            let strips = console_slot(console, *light);
            let bg = if *light {
                [0xc9u8, 0xd1, 0xde]
            } else {
                [0x20, 0x24, 0x30]
            };
            let mut canvas: Vec<f32> = (0..w * h).flat_map(|_| bg.map(|c| c as f32)).collect();
            if *plugged {
                layer(&mut canvas, top, &strips.plugged);
            } else {
                layer(&mut canvas, top, &strips.back);
                game(&mut canvas, platform, top, *foot);
                layer(&mut canvas, top, &strips.front);
            }
            let oy = gap + r * (h + gap);
            for y in 0..h {
                for x in 0..w {
                    let s = (y * w + x) * 3;
                    let d = ((oy + y) * w + x) * 3;
                    for c in 0..3 {
                        sheet[d + c] = canvas[s + c].clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
        let path = format!("{prefix}-{name}.png");
        let f = std::fs::File::create(&path).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w as u32, sheet_h as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .unwrap()
            .write_image_data(&sheet)
            .unwrap();
        println!("wrote {path}");
    }
}
