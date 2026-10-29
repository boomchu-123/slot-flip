//! The slot a game goes into, drawn as the console it is for.
//!
//! GBA keeps the strips traced from upstream's artwork. Every other system gets its
//! own console here, drawn rather than traced so it needs no binary asset and is
//! covered by tests: a PlayStation with its round lid, a NES with its black
//! stripe and red light, a Super Nintendo with its ridges and purple buttons, a
//! Game Boy, and a Game Boy Color in purple plastic.
//!
//! Same contract as the pre-drawn strips in `assets/`: a back layer that sits
//! behind the game, a front layer that hides its lower part so it seems to go in
//! (with an opening cut where it passes), and a "plugged" layer with the opening
//! closed. The opening keeps the width, depth and position of the cart slot's so
//! the insertion clips the same way; only its shape changes.
//!
//! Each console is its plastic: a lit top edge and a darker lower lip, a fine
//! grain, a bevel where the opening is cut away, and the buttons and light it wears.

use slot_store::Platform;

use crate::bitmaps::{SLOT_H, SLOT_W};
use crate::text;

/// The width the console art is drawn at: the 720 px design canvas, the same
/// one the pre-drawn GBA strips are authored on. A narrower panel shows the
/// middle of it (see `fit`), exactly as `bitmaps::load` crops a 720 px template
/// to a 640 px panel. Drawing straight at the panel width instead left every
/// console's opening about 40 px off centre on the software canvas, so the
/// centred cart looked shifted left inside it.
const W: u32 = 720;
const H: u32 = SLOT_H;

/// The opening, matched to the cart slot's: 254 wide about the centre, 44 deep.
const OPEN_HALF: f32 = 127.0;
const OPEN_DEPTH: f32 = 44.0;
const CX: f32 = 359.5;

type Rgb = [u8; 3];

/// Which console a slot is drawn as.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Console {
    Ps1,
    Nes,
    Snes,
    GameBoy,
    GameBoyColor,
    MegaDrive,
    MasterSystem,
    GameGear,
}

impl Console {
    pub const ALL: [Console; 8] = [
        Console::Ps1,
        Console::Nes,
        Console::Snes,
        Console::GameBoy,
        Console::GameBoyColor,
        Console::MegaDrive,
        Console::MasterSystem,
        Console::GameGear,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// The console a system's games go into. GBA has none of its own here: it keeps
    /// the slot traced from upstream.
    pub fn of(platform: Platform) -> Option<Console> {
        match platform {
            Platform::Gba => None,
            Platform::Gb => Some(Console::GameBoy),
            Platform::Gbc => Some(Console::GameBoyColor),
            Platform::Ps1 => Some(Console::Ps1),
            Platform::Nes => Some(Console::Nes),
            Platform::Snes => Some(Console::Snes),
            Platform::Md => Some(Console::MegaDrive),
            Platform::Sms => Some(Console::MasterSystem),
            Platform::Gg => Some(Console::GameGear),
        }
    }
}

pub struct ConsoleStrips {
    pub back: Vec<u8>,
    pub front: Vec<u8>,
    pub plugged: Vec<u8>,
}

struct Palette {
    plastic: Rgb,
    lit: Rgb,
    lip: Rgb,
    button: Rgb,
    ink: Rgb,
    /// The colour a console picks out some of its controls in.
    accent: Rgb,
}

/// The shape of the opening the game passes through.
#[derive(Copy, Clone)]
enum Opening {
    /// A half ellipse: the walls stand straight at the edges and curve to a floor.
    Ellipse,
    /// A flat floor, with the lower corners rounded to this radius.
    Flat(f32),
}

/// What a console is made of, apart from its plastic colours.
struct Style {
    palette: fn(bool) -> Palette,
    opening: Opening,
    /// The wall of the opening falling into shadow.
    rim: Rgb,
    dress: fn(&mut Img, &Palette),
    /// Closed, the slot shows a round lid (PlayStation) or a groove where the
    /// opening was.
    lid: bool,
    /// The dark inside, at the top of the opening and at the bottom.
    inside: (Rgb, Rgb),
}

fn style(console: Console) -> Style {
    match console {
        Console::Ps1 => Style {
            palette: ps1_palette,
            opening: Opening::Ellipse,
            rim: [0x2b, 0x2a, 0x29],
            dress: dress_ps1,
            lid: true,
            inside: ([0x2a, 0x2a, 0x2c], [0x0c, 0x0c, 0x0e]),
        },
        Console::Nes => Style {
            palette: nes_palette,
            opening: Opening::Flat(3.0),
            rim: [0x1c, 0x1b, 0x1a],
            dress: dress_nes,
            lid: false,
            inside: ([0x24, 0x23, 0x22], [0x08, 0x08, 0x08]),
        },
        Console::Snes => Style {
            palette: snes_palette,
            opening: Opening::Flat(12.0),
            rim: [0x2a, 0x29, 0x2e],
            dress: dress_snes,
            lid: false,
            inside: ([0x2a, 0x2a, 0x30], [0x0a, 0x0a, 0x0e]),
        },
        Console::GameBoy => Style {
            palette: gb_palette,
            opening: Opening::Flat(2.0),
            rim: [0x2a, 0x2a, 0x26],
            dress: dress_gb,
            lid: false,
            inside: ([0x26, 0x27, 0x22], [0x0a, 0x0a, 0x08]),
        },
        Console::GameBoyColor => Style {
            palette: gbc_palette,
            opening: Opening::Flat(2.0),
            rim: [0x22, 0x1a, 0x38],
            dress: dress_gbc,
            lid: false,
            inside: ([0x24, 0x1e, 0x3a], [0x08, 0x06, 0x12]),
        },
        Console::MegaDrive => Style {
            palette: md_palette,
            opening: Opening::Flat(4.0),
            rim: [0x0c, 0x0c, 0x0e],
            dress: dress_md,
            lid: false,
            inside: ([0x1c, 0x1c, 0x20], [0x04, 0x04, 0x06]),
        },
        Console::MasterSystem => Style {
            palette: sms_palette,
            opening: Opening::Flat(3.0),
            rim: [0x0e, 0x0d, 0x0e],
            dress: dress_sms,
            lid: false,
            inside: ([0x1e, 0x1d, 0x20], [0x05, 0x05, 0x06]),
        },
        Console::GameGear => Style {
            palette: gg_palette,
            opening: Opening::Flat(2.0),
            rim: [0x0a, 0x0a, 0x0c],
            dress: dress_gg,
            lid: false,
            inside: ([0x1a, 0x1a, 0x1e], [0x04, 0x04, 0x06]),
        },
    }
}

fn ps1_palette(light: bool) -> Palette {
    if light {
        Palette {
            plastic: [0xb9, 0xb8, 0xb4],
            lit: [0xe2, 0xe1, 0xdd],
            lip: [0x8f, 0x8e, 0x8b],
            button: [0xc6, 0xc5, 0xc1],
            ink: [0x58, 0x57, 0x55],
            accent: [0xc6, 0xc5, 0xc1],
        }
    } else {
        Palette {
            plastic: [0xa2, 0xa1, 0x9d],
            lit: [0xcd, 0xcc, 0xc8],
            lip: [0x76, 0x75, 0x72],
            button: [0xb1, 0xb0, 0xac],
            ink: [0x4a, 0x49, 0x47],
            accent: [0xb1, 0xb0, 0xac],
        }
    }
}

fn nes_palette(light: bool) -> Palette {
    if light {
        Palette {
            plastic: [0xc2, 0xc1, 0xbc],
            lit: [0xe8, 0xe7, 0xe2],
            lip: [0x95, 0x94, 0x90],
            button: [0xa8, 0xa7, 0xa3],
            ink: [0x3c, 0x3b, 0x39],
            accent: [0xb8, 0x2c, 0x2c],
        }
    } else {
        Palette {
            plastic: [0xa8, 0xa7, 0xa2],
            lit: [0xd2, 0xd1, 0xcc],
            lip: [0x7c, 0x7b, 0x77],
            button: [0x92, 0x91, 0x8d],
            ink: [0x2c, 0x2b, 0x2a],
            accent: [0xb0, 0x28, 0x28],
        }
    }
}

fn snes_palette(light: bool) -> Palette {
    if light {
        Palette {
            plastic: [0xc6, 0xc5, 0xc9],
            lit: [0xea, 0xe9, 0xed],
            lip: [0x98, 0x97, 0x9c],
            button: [0xb0, 0xaf, 0xb4],
            ink: [0x3e, 0x3d, 0x42],
            accent: [0x7c, 0x6c, 0xb4],
        }
    } else {
        Palette {
            plastic: [0xae, 0xad, 0xb2],
            lit: [0xd6, 0xd5, 0xda],
            lip: [0x80, 0x7f, 0x84],
            button: [0x9a, 0x99, 0x9e],
            ink: [0x32, 0x31, 0x36],
            accent: [0x6a, 0x5a, 0xa0],
        }
    }
}

fn gb_palette(light: bool) -> Palette {
    if light {
        Palette {
            plastic: [0xc6, 0xc5, 0xbc],
            lit: [0xe8, 0xe7, 0xde],
            lip: [0x98, 0x97, 0x90],
            button: [0xb0, 0xaf, 0xa6],
            ink: [0x44, 0x44, 0x3f],
            accent: [0x4a, 0x5a, 0xb0],
        }
    } else {
        Palette {
            plastic: [0xac, 0xab, 0xa2],
            lit: [0xd4, 0xd3, 0xca],
            lip: [0x7e, 0x7d, 0x76],
            button: [0x98, 0x97, 0x8f],
            ink: [0x38, 0x38, 0x34],
            accent: [0x3c, 0x4c, 0xa0],
        }
    }
}

fn gbc_palette(light: bool) -> Palette {
    if light {
        Palette {
            plastic: [0x86, 0x73, 0xc0],
            lit: [0xb2, 0xa4, 0xde],
            lip: [0x5c, 0x4b, 0x94],
            button: [0x96, 0x83, 0xcc],
            ink: [0xf0, 0xec, 0xff],
            accent: [0xd8, 0x50, 0x80],
        }
    } else {
        Palette {
            plastic: [0x6a, 0x56, 0xa6],
            lit: [0x96, 0x85, 0xcd],
            lip: [0x46, 0x37, 0x78],
            button: [0x7c, 0x69, 0xb8],
            ink: [0xe6, 0xe0, 0xfa],
            accent: [0xd0, 0x46, 0x78],
        }
    }
}

fn md_palette(light: bool) -> Palette {
    if light {
        Palette {
            plastic: [0x46, 0x46, 0x4a],
            lit: [0x72, 0x72, 0x78],
            lip: [0x26, 0x26, 0x2a],
            button: [0x56, 0x56, 0x5c],
            ink: [0xe6, 0xe6, 0xea],
            accent: [0xb8, 0xb8, 0xc0],
        }
    } else {
        Palette {
            plastic: [0x2a, 0x2a, 0x2d],
            lit: [0x4c, 0x4c, 0x52],
            lip: [0x15, 0x15, 0x18],
            button: [0x3a, 0x3a, 0x3f],
            ink: [0xd2, 0xd2, 0xd8],
            accent: [0x9c, 0x9c, 0xa4],
        }
    }
}

fn sms_palette(light: bool) -> Palette {
    if light {
        Palette {
            plastic: [0x4a, 0x48, 0x4c],
            lit: [0x76, 0x74, 0x7a],
            lip: [0x28, 0x27, 0x2a],
            button: [0xc0, 0x30, 0x30],
            ink: [0xf6, 0xf0, 0xf0],
            accent: [0xcc, 0x34, 0x34],
        }
    } else {
        Palette {
            plastic: [0x2e, 0x2d, 0x30],
            lit: [0x50, 0x4e, 0x54],
            lip: [0x18, 0x18, 0x1a],
            button: [0xb0, 0x26, 0x26],
            ink: [0xf2, 0xea, 0xea],
            accent: [0xc0, 0x2c, 0x2c],
        }
    }
}

fn gg_palette(light: bool) -> Palette {
    if light {
        Palette {
            plastic: [0x42, 0x42, 0x47],
            lit: [0x6c, 0x6c, 0x73],
            lip: [0x22, 0x22, 0x26],
            button: [0x54, 0x54, 0x5a],
            ink: [0xe2, 0xe2, 0xe8],
            accent: [0x8e, 0x8e, 0x98],
        }
    } else {
        Palette {
            plastic: [0x26, 0x26, 0x2a],
            lit: [0x46, 0x46, 0x4c],
            lip: [0x12, 0x12, 0x15],
            button: [0x34, 0x34, 0x39],
            ink: [0xcc, 0xcc, 0xd2],
            accent: [0x7c, 0x7c, 0x86],
        }
    }
}

/// How deep the opening is at column `x`: zero outside it.
fn open_depth(o: Opening, x: u32) -> f32 {
    let dx = (x as f32 + 0.5 - CX).abs();
    if dx >= OPEN_HALF {
        return 0.0;
    }
    match o {
        Opening::Ellipse => {
            let u = dx / OPEN_HALF;
            OPEN_DEPTH * (1.0 - u * u).sqrt()
        }
        Opening::Flat(r) => {
            let into = OPEN_HALF - dx;
            if into >= r {
                OPEN_DEPTH
            } else {
                OPEN_DEPTH - (r - (r * r - (r - into) * (r - into)).max(0.0).sqrt())
            }
        }
    }
}

fn in_opening(o: Opening, x: u32, y: u32) -> bool {
    (y as f32 + 0.5) < open_depth(o, x)
}

/// Fine deterministic grain, so the plastic is not a flat fill.
fn grain(x: u32, y: u32) -> i32 {
    let h = x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663);
    let h = (h ^ (h >> 13)).wrapping_mul(0x5bd1_e995);
    ((h >> 16) & 7) as i32 - 3
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8)
}

fn shade(c: Rgb, by: i32) -> Rgb {
    [0, 1, 2].map(|i| (c[i] as i32 + by).clamp(0, 255) as u8)
}

struct Img {
    rgba: Vec<u8>,
}

impl Img {
    fn new() -> Img {
        Img {
            rgba: vec![0; (W * H * 4) as usize],
        }
    }

    fn put(&mut self, x: u32, y: u32, c: Rgb, a: u8) {
        if x < W && y < H {
            let i = ((y * W + x) * 4) as usize;
            self.rgba[i..i + 3].copy_from_slice(&c);
            self.rgba[i + 3] = a;
        }
    }

    fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * W + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    /// `c` laid over what is there, `a` of the way.
    fn blend(&mut self, x: u32, y: u32, c: Rgb, a: f32) {
        if x >= W || y >= H || a <= 0.0 {
            return;
        }
        let p = self.at(x, y);
        let m = mix([p[0], p[1], p[2]], c, a);
        self.put(x, y, m, p[3].max((a * 255.0) as u8));
    }
}

/// Coverage of a rounded rectangle at a pixel, with a soft one pixel edge.
fn rounded(x: f32, y: f32, rect: (f32, f32, f32, f32), r: f32) -> f32 {
    let (rx, ry, rw, rh) = rect;
    let (cx, cy) = (rx + rw / 2.0, ry + rh / 2.0);
    let (qx, qy) = (
        (x - cx).abs() - (rw / 2.0 - r),
        (y - cy).abs() - (rh / 2.0 - r),
    );
    let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
    (0.5 - d).clamp(0.0, 1.0)
}

/// The plate: plastic across the whole strip, optionally with the opening cut.
fn plate(p: &Palette, o: Opening, rim: Rgb, opening: bool) -> Img {
    let mut img = Img::new();
    for y in 0..H {
        for x in 0..W {
            if opening && in_opening(o, x, y) {
                continue;
            }
            // Lit along the top, darker down the face, a lip along the bottom.
            let t = y as f32 / (H - 1) as f32;
            let mut c = mix(p.lit, p.plastic, (y as f32 / 3.0).min(1.0));
            c = mix(c, p.lip, ((t - 0.62) / 0.38).max(0.0).powi(2));
            c = shade(c, grain(x, y));
            img.put(x, y, c, 255);
        }
    }
    if opening {
        bevel_opening(&mut img, p, o, rim);
    }
    img
}

/// The edge where the lid is cut away: a bright lip, then the wall falling into
/// shadow.
fn bevel_opening(img: &mut Img, p: &Palette, o: Opening, rim: Rgb) {
    const REACH: i32 = 5;
    let transparent = |x: i32, y: i32| {
        x >= 0 && y >= 0 && (x as u32) < W && (y as u32) < H && in_opening(o, x as u32, y as u32)
    };
    for y in 0..H {
        for x in 0..W {
            if in_opening(o, x, y) {
                continue;
            }
            let mut nearest = f32::MAX;
            for dy in -REACH..=REACH {
                for dx in -REACH..=REACH {
                    if transparent(x as i32 + dx, y as i32 + dy) {
                        nearest = nearest.min(((dx * dx + dy * dy) as f32).sqrt());
                    }
                }
            }
            if nearest > REACH as f32 {
                continue;
            }
            if nearest < 1.6 {
                img.blend(x, y, p.lit, 0.55);
            } else {
                img.blend(x, y, rim, 0.34 * (1.0 - (nearest - 1.6) / 3.4));
            }
        }
    }
}

/// A raised rounded button with its label.
fn button(img: &mut Img, p: &Palette, rect: (f32, f32, f32, f32), label: &str) {
    button_in(img, p, p.button, rect, label);
}

/// A button in a chosen colour, with its label.
fn button_in(img: &mut Img, p: &Palette, fill: Rgb, rect: (f32, f32, f32, f32), label: &str) {
    let (bx, by, bw, bh) = rect;
    for y in (by as u32 - 2)..(by as u32 + bh as u32 + 3) {
        for x in (bx as u32 - 2)..(bx as u32 + bw as u32 + 3) {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // The shadow the button casts down and to the right.
            let drop = rounded(fx - 1.0, fy - 1.5, rect, 4.0);
            img.blend(x, y, [0x30, 0x2f, 0x2e], 0.32 * drop);
            let body = rounded(fx, fy, rect, 4.0);
            if body > 0.0 {
                let t = ((fy - by) / bh).clamp(0.0, 1.0);
                let c = mix(shade(fill, 14), shade(fill, -16), t);
                img.blend(x, y, c, body);
            }
        }
    }
    if let Some(font) = text::label_font() {
        let (tw, th) = ((bw - 6.0) as u32, (bh - 4.0) as u32);
        let layout = text::fit_box(font, label, tw as f32, th as f32, 1, th as f32, 6.0);
        let cover = text::coverage(tw, th, &layout);
        let (ox, oy) = (bx as u32 + 3, by as u32 + 2);
        for ty in 0..th {
            for tx in 0..tw {
                let a = cover[(ty * tw + tx) as usize];
                if a > 0 {
                    img.blend(ox + tx, oy + ty, p.ink, a as f32 / 255.0 * 0.9);
                }
            }
        }
    }
}

/// The colours of a power light.
struct Lamp {
    dark: Rgb,
    bright: Rgb,
    halo: Rgb,
}

const GREEN: Lamp = Lamp {
    dark: [0x2a, 0xa8, 0x3a],
    bright: [0xa6, 0xff, 0xb0],
    halo: [0x58, 0xd0, 0x6a],
};

const RED: Lamp = Lamp {
    dark: [0xa0, 0x18, 0x18],
    bright: [0xff, 0x9c, 0x8c],
    halo: [0xe0, 0x50, 0x46],
};

/// The power light: a lamp in a small bezel.
fn led(img: &mut Img, cx: f32, cy: f32, lamp: &Lamp) {
    for y in (cy as u32 - 6)..(cy as u32 + 7) {
        for x in (cx as u32 - 6)..(cx as u32 + 7) {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            img.blend(
                x,
                y,
                [0x35, 0x34, 0x33],
                (1.0 - (d - 4.5).max(0.0)).clamp(0.0, 1.0) * 0.8,
            );
            if d < 3.2 {
                let glow = 1.0 - d / 3.2;
                img.blend(x, y, mix(lamp.dark, lamp.bright, glow * 0.7), 1.0);
            } else if d < 6.0 {
                img.blend(x, y, lamp.halo, 0.22 * (1.0 - (d - 3.2) / 2.8));
            }
        }
    }
}

/// A row of fine vertical grooves, `count` of them `pitch` apart from `x0`.
fn grooves(img: &mut Img, p: &Palette, x0: u32, count: u32, pitch: u32, y0: u32, y1: u32) {
    for n in 0..count {
        let x = x0 + n * pitch;
        for y in y0..y1 {
            img.blend(x, y, p.lip, 0.55);
            img.blend(x + 1, y, p.lit, 0.45);
        }
    }
}

/// A dark band across the plate, below the opening.
fn stripe(img: &mut Img, y0: u32, y1: u32, c: Rgb) {
    for y in y0..y1 {
        for x in 0..W {
            img.blend(x, y, c, 0.78);
        }
    }
}

/// POWER and its light on the left, RESET and OPEN on the right, clear of the
/// opening and inside what a 640 px panel shows of the 720 px strip.
fn dress_ps1(img: &mut Img, p: &Palette) {
    led(img, 138.0, 29.0, &GREEN);
    button(img, p, (152.0, 20.0, 50.0, 17.0), "POWER");
    button(img, p, (520.0, 20.0, 46.0, 17.0), "RESET");
    button(img, p, (574.0, 20.0, 42.0, 17.0), "OPEN");
}

/// A NES: POWER and RESET, a red light, and the dark stripe along its front.
fn dress_nes(img: &mut Img, p: &Palette) {
    stripe(img, 50, 53, [0x24, 0x23, 0x22]);
    led(img, 138.0, 27.0, &RED);
    button(img, p, (152.0, 18.0, 50.0, 17.0), "POWER");
    button(img, p, (520.0, 18.0, 46.0, 17.0), "RESET");
}

/// A Super Nintendo: ridges either side of the opening, a grey POWER, and RESET and
/// EJECT in purple.
fn dress_snes(img: &mut Img, p: &Palette) {
    grooves(img, p, 78, 6, 5, 12, 40);
    grooves(img, p, 618, 6, 5, 12, 40);
    led(img, 138.0, 29.0, &RED);
    button(img, p, (152.0, 20.0, 50.0, 17.0), "POWER");
    button_in(img, p, p.accent, (520.0, 20.0, 46.0, 17.0), "RESET");
    button_in(img, p, p.accent, (574.0, 20.0, 42.0, 17.0), "EJECT");
}

/// A Game Boy: ridges at both ends, an ON/OFF slider, and a small red light.
fn dress_gb(img: &mut Img, p: &Palette) {
    grooves(img, p, 60, 7, 5, 10, 46);
    grooves(img, p, 610, 7, 5, 10, 46);
    led(img, 138.0, 29.0, &RED);
    button(img, p, (152.0, 20.0, 50.0, 17.0), "ON/OFF");
    button_in(img, p, p.accent, (540.0, 20.0, 34.0, 17.0), "A");
    button_in(img, p, p.accent, (582.0, 20.0, 34.0, 17.0), "B");
}

/// A Game Boy Color: the same layout in purple plastic, its buttons pink.
fn dress_gbc(img: &mut Img, p: &Palette) {
    grooves(img, p, 60, 7, 5, 10, 46);
    grooves(img, p, 610, 7, 5, 10, 46);
    led(img, 138.0, 29.0, &RED);
    button(img, p, (152.0, 20.0, 50.0, 17.0), "ON/OFF");
    button_in(img, p, p.accent, (540.0, 20.0, 34.0, 17.0), "A");
    button_in(img, p, p.accent, (582.0, 20.0, 34.0, 17.0), "B");
}

/// A Mega Drive: black, a silver band along the front, a red light, POWER and
/// RESET, and its bit width on a tag.
fn dress_md(img: &mut Img, p: &Palette) {
    stripe(img, 50, 52, p.accent);
    led(img, 138.0, 27.0, &RED);
    button(img, p, (152.0, 18.0, 50.0, 17.0), "POWER");
    button(img, p, (520.0, 18.0, 46.0, 17.0), "RESET");
    button_in(
        img,
        p,
        shade(p.plastic, -8),
        (574.0, 18.0, 42.0, 17.0),
        "16-BIT",
    );
}

/// A Master System: black with a red band along the front, and POWER and RESET in
/// red.
fn dress_sms(img: &mut Img, p: &Palette) {
    stripe(img, 50, 53, p.accent);
    led(img, 138.0, 27.0, &RED);
    button_in(img, p, p.button, (152.0, 18.0, 50.0, 17.0), "POWER");
    button_in(img, p, p.button, (520.0, 18.0, 46.0, 17.0), "RESET");
}

/// A Game Gear: black, ridged at both ends like a handheld, with a volume dial and
/// an ON/OFF switch.
fn dress_gg(img: &mut Img, p: &Palette) {
    grooves(img, p, 60, 7, 5, 10, 46);
    grooves(img, p, 610, 7, 5, 10, 46);
    led(img, 138.0, 29.0, &RED);
    button(img, p, (152.0, 20.0, 50.0, 17.0), "ON/OFF");
    button(img, p, (540.0, 20.0, 34.0, 17.0), "VOL");
    stripe(img, 50, 52, p.accent);
}

/// Where the opening was, shown closed: a groove let into the plate, shadowed on
/// the upper-left wall and lit on the lower-right.
fn groove(img: &mut Img, p: &Palette, o: Opening) {
    const REACH: i32 = 3;
    let inside = |x: i32, y: i32| {
        x >= 0 && y >= 0 && (x as u32) < W && (y as u32) < H && in_opening(o, x as u32, y as u32)
    };
    for y in 0..H {
        for x in 0..W {
            let here = in_opening(o, x, y);
            let mut nearest = f32::MAX;
            for dy in -REACH..=REACH {
                for dx in -REACH..=REACH {
                    if inside(x as i32 + dx, y as i32 + dy) != here {
                        nearest = nearest.min(((dx * dx + dy * dy) as f32).sqrt());
                    }
                }
            }
            if nearest > REACH as f32 {
                continue;
            }
            let k = (1.0 - nearest / (REACH as f32 + 0.5)) * 0.55;
            // The side inside the outline is the shadowed wall; outside it catches light.
            img.blend(x, y, if here { [0x2b, 0x2a, 0x29] } else { p.lit }, k);
        }
    }
}

/// The lid, seen closed: a round ring let into the plate, shown when the slot is
/// plugged.
fn lid_ring(img: &mut Img, p: &Palette) {
    let (cx, cy, r) = (CX, 27.0, 21.0);
    for y in 0..H {
        for x in 0..W {
            let (fx, fy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let d = (fx * fx + fy * fy).sqrt();
            if (d - r).abs() < 3.0 {
                // A groove: shadowed on its upper-left wall, lit on its lower-right.
                let lit = (fx + fy) / (2.0 * r);
                let k = (1.0 - (d - r).abs() / 3.0) * 0.5;
                img.blend(x, y, if lit > 0.0 { p.lit } else { [0x2f, 0x2e, 0x2d] }, k);
            } else if d < r - 3.0 {
                // The lid's centre sits a shade lower than the plate around it.
                img.blend(x, y, p.lip, 0.10);
            }
        }
    }
}

/// What is behind the opening: the dark inside of the console, deepening
/// downwards.
fn back(p: &Palette, inside: (Rgb, Rgb)) -> Img {
    let mut img = Img::new();
    for y in 0..H {
        for x in 0..W {
            let t = y as f32 / (H - 1) as f32;
            let base = mix(inside.0, inside.1, t.min(1.0).sqrt());
            img.put(x, y, shade(base, grain(x, y) / 2), 255);
        }
    }
    // A hint of the opening's rim catching the light along the top of the interior.
    for x in 0..W {
        img.blend(x, 0, p.lip, 0.5);
        img.blend(x, 1, p.lip, 0.22);
    }
    img
}

pub fn console_slot(console: Console, light: bool) -> ConsoleStrips {
    let st = style(console);
    let p = (st.palette)(light);

    let mut front = plate(&p, st.opening, st.rim, true);
    (st.dress)(&mut front, &p);

    let mut plugged = plate(&p, st.opening, st.rim, false);
    (st.dress)(&mut plugged, &p);
    match st.lid {
        true => lid_ring(&mut plugged, &p),
        false => groove(&mut plugged, &p, st.opening),
    }

    ConsoleStrips {
        back: fit(back(&p, st.inside).rgba),
        front: fit(front.rgba),
        plugged: fit(plugged.rgba),
    }
}

/// The middle of the 720 px design that a panel shows. At 720 this is the whole
/// strip; at 640 it is the centre crop, which keeps the opening on the panel's
/// centre line and the decorations where the design put them relative to it.
fn fit(strip: Vec<u8>) -> Vec<u8> {
    if W == SLOT_W {
        strip
    } else {
        crate::bitmaps::crop_centre(&strip, W, H, SLOT_W)
    }
}

/// How deep the opening is cut at this column of the panel, for tests and
/// callers that line something up with it. `x` is a panel column, so it is
/// shifted onto the design sheet the art is actually drawn on.
pub fn opening_depth(console: Console, x: u32) -> f32 {
    let design = x + W.saturating_sub(SLOT_W) / 2;
    open_depth(style(console).opening, design)
}
