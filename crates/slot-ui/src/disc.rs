//! The round PlayStation disc.
//!
//! Carts are traced from bitmaps; a disc is a circle, so it is drawn. That keeps
//! it crisp at any size it is shelved or seated at, and lets the hole be a real
//! hole: the slot shows through it as the disc goes in.
//!
//! From the middle out: the spindle hole (transparent), the clear clamp ring
//! (frosted plastic with the cover showing through it), a fine stacking groove,
//! then the printed surface that carries the cover art edge to edge, as the label
//! side of a real disc does.
//! The rim is a touch darker where the plastic is thick.
//!
//! The glare across the print is a separate layer ([`glare`]), not part of the
//! face. It is a reflection of the room's light, so it stays where it is while
//! the disc turns underneath it as the slot takes it; baked into the face it
//! would spin with the disc and look like the light was turning too.

use std::f32::consts::TAU;
use std::sync::OnceLock;

use crate::cart::{ink, label_colour, CartFace};
use crate::text;

/// Side of the square a disc is drawn in. The slot is 254 wide, so a disc this
/// size is scaled down to `SEATED_W` as it goes in, like a cart is.
pub const DISC_D: u32 = 300;

const R: f32 = DISC_D as f32 / 2.0;
// Smaller than a real disc's clamp ring (about 0.275 of the radius): at true size
// the hub hides the middle of a cover, which is where its title is.
const HOLE: f32 = 0.11 * R;
const HUB: f32 = 0.24 * R;
/// How much of the cover shows through the clear hub, as frosted plastic.
const FROST: f32 = 0.55;
const GROOVE_HALF: f32 = 1.2;
const RIM: f32 = 2.5;

const PAD: u32 = 6;
const MAX_LINES: usize = 3;
const MIN_PX: f32 = 9.0;

/// Where the glare crosses the print, as an angle. Two opposite wedges.
const GLARE_AT: f32 = 0.62;
const GLARE: f32 = 0.17;

fn centre_dist(x: u32, y: u32) -> (f32, f32, f32) {
    let dx = x as f32 + 0.5 - R;
    let dy = y as f32 + 0.5 - R;
    (dx, dy, (dx * dx + dy * dy).sqrt())
}

/// How much of the pixel the disc covers: one inside the rim, zero outside, a
/// smooth ramp across the edge, and the hole cut out of the middle.
fn shape(d: f32) -> f32 {
    let outer = (R - d + 0.5).clamp(0.0, 1.0);
    let hole = (d - HOLE + 0.5).clamp(0.0, 1.0);
    outer * hole
}

/// The disc's own coverage, for its shadow and for clipping.
pub fn mask() -> &'static [u8] {
    static MASK: OnceLock<Vec<u8>> = OnceLock::new();
    MASK.get_or_init(|| {
        let mut out = Vec::with_capacity((DISC_D * DISC_D) as usize);
        for y in 0..DISC_D {
            for x in 0..DISC_D {
                let (_, _, d) = centre_dist(x, y);
                out.push((shape(d) * 255.0).round() as u8);
            }
        }
        out
    })
}

/// Where a disc turns as it goes into the slot: one whole turn, easing in and
/// out, so it settles upright at both ends (on the shelf and fully seated).
pub fn spin(seat: f32) -> f32 {
    let s = seat.clamp(0.0, 1.0);
    TAU * (s * s * (3.0 - 2.0 * s))
}

/// The label rectangle, as fractions of the disc, that a badge (the favourite
/// star) is placed against. Its top right corner sits inside the circle.
pub const LABEL: (f32, f32, f32, f32) = (0.2, 0.2, 0.6, 0.6);

/// The glare, drawn over a disc without turning with it: soft white wedges on
/// the printed surface only. The same for every disc, so it is built once.
pub fn glare() -> CartFace {
    let mut rgba = vec![0u8; (DISC_D * DISC_D * 4) as usize];
    for y in 0..DISC_D {
        for x in 0..DISC_D {
            let (dx, dy, d) = centre_dist(x, y);
            if d < HUB {
                continue;
            }
            let cover = shape(d);
            let theta = dy.atan2(dx);
            let wedge = (2.0 * (theta - GLARE_AT)).cos().max(0.0).powi(6);
            let strength = wedge * GLARE * ((d - HUB) / (R - HUB)).clamp(0.0, 1.0).sqrt();
            let i = ((y * DISC_D + x) * 4) as usize;
            rgba[i..i + 3].fill(255);
            rgba[i + 3] = (cover * strength * 255.0).round() as u8;
        }
    }
    CartFace {
        rgba,
        w: DISC_D,
        h: DISC_D,
    }
}

pub fn face(title: &str, art: Option<Vec<u8>>) -> CartFace {
    let n = (DISC_D * DISC_D) as usize;
    let print = match art {
        Some(a) if a.len() == n * 4 => a,
        _ => generated(title),
    };
    let mut rgba = vec![0u8; n * 4];
    for y in 0..DISC_D {
        for x in 0..DISC_D {
            let (_, _, d) = centre_dist(x, y);
            let cover = shape(d);
            if cover <= 0.0 {
                continue;
            }
            let i = ((y * DISC_D + x) * 4) as usize;

            let p = &print[i..i + 4];
            let art = [p[0] as f32, p[1] as f32, p[2] as f32];
            let (mut c, opacity) = if d >= HUB {
                (art, 1.0)
            } else {
                // Frosted clear plastic over the cover: silver, a little lighter
                // towards the rim of the hole, with the artwork showing through.
                let t = (d - HOLE) / (HUB - HOLE);
                let v = 196.0 + 34.0 * t;
                let silver = [v - 6.0, v - 2.0, v];
                let mix = |k: usize| silver[k] * (1.0 - FROST) + art[k] * FROST;
                ([mix(0), mix(1), mix(2)], 0.92)
            };

            // The stacking groove where the clear ring meets the print.
            let groove = (1.0 - (d - HUB).abs() / GROOVE_HALF).clamp(0.0, 1.0) * 0.45;
            // The thick plastic at the edge.
            let rim = ((d - (R - RIM)) / RIM).clamp(0.0, 1.0) * 0.30;
            let dark = 1.0 - groove.max(rim);

            for v in &mut c {
                *v = (*v * dark).clamp(0.0, 255.0);
            }
            rgba[i] = c[0].round() as u8;
            rgba[i + 1] = c[1].round() as u8;
            rgba[i + 2] = c[2].round() as u8;
            rgba[i + 3] = (cover * opacity * 255.0).round() as u8;
        }
    }
    CartFace {
        rgba,
        w: DISC_D,
        h: DISC_D,
    }
}

/// A printed surface for a disc with no cover art: the game's colour, a fine
/// border ring, and its title across the lower half where the circle is wide.
fn generated(title: &str) -> Vec<u8> {
    let bg = label_colour(title);
    let ink = ink(bg);
    let n = (DISC_D * DISC_D) as usize;
    let mut rgba = Vec::with_capacity(n * 4);
    for _ in 0..n {
        rgba.extend_from_slice(&[bg[0], bg[1], bg[2], 255]);
    }

    // A thin printed border just inside the rim.
    for y in 0..DISC_D {
        for x in 0..DISC_D {
            let (_, _, d) = centre_dist(x, y);
            let line = 1.0 - ((d - 0.93 * R).abs() / 1.2).clamp(0.0, 1.0);
            if line > 0.0 {
                let i = ((y * DISC_D + x) * 4) as usize;
                for c in 0..3 {
                    rgba[i + c] = (ink[c] as f32 * line * 0.35
                        + rgba[i + c] as f32 * (1.0 - line * 0.35))
                        .round() as u8;
                }
            }
        }
    }

    if let Some(font) = text::label_font() {
        // The widest box that fits the circle below the hub: at 0.86R down the
        // circle is still about a radius wide.
        let (bw, bh) = ((0.9 * R) as u32, (0.46 * R) as u32);
        let layout = text::fit_box(
            font,
            title,
            (bw - 2 * PAD) as f32,
            (bh - 2 * PAD) as f32,
            MAX_LINES,
            (bh - 2 * PAD) as f32 / MAX_LINES as f32,
            MIN_PX,
        );
        let mut line = vec![0u8; (bw * bh * 4) as usize];
        for px in line.chunks_exact_mut(4) {
            px.copy_from_slice(&[bg[0], bg[1], bg[2], 255]);
        }
        text::draw_centred(&mut line, bw, bh, &layout, ink);
        let (ox, oy) = (DISC_D / 2 - bw / 2, (R + 0.38 * R) as u32);
        for row in 0..bh {
            let src = (row * bw * 4) as usize;
            let dst = (((oy + row) * DISC_D + ox) * 4) as usize;
            rgba[dst..dst + (bw * 4) as usize].copy_from_slice(&line[src..src + (bw * 4) as usize]);
        }
    }
    rgba
}
