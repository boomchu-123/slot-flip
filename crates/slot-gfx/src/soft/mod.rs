//! Software rendering backend, for devices with a framebuffer and no GPU.
//!
//! `lib.rs` exports this module's `Compositor` and `FbdevSurface` under the
//! `software` feature in place of the GL ones, so nothing above `slot-gfx`
//! changes: `slot-ui` already emits a `Vec<Draw>` and never touches GL, and
//! `device_app.rs` names only these two types.
//!
//! What is faithful to the GL path:
//!   - `Draw` coordinates, blending and texture modulation (see `raster.rs`)
//!   - the game pass's nearest sampling, `u_bright` and the Lcd3x mask, which is
//!     the same `lcd3x_factors` the shader's `u_mode > 0.5` branch computes
//!   - the blue-light gain, applied on output as `BLIT_FRAG` does
//!
//! What is deliberately not, and why:
//!   - **Grid, Dot and Simpletex screen effects** fall back to no effect. They
//!     are multi-tap per-pixel filters; Lcd3x is the default and the only one
//!     that reduces to a table at integer scale. Revisit once there are frame
//!     times from the device.
//!   - **The overlay slide** is recorded and ignored: upstream slides the SP
//!     shell up from the bottom of a taller panel, and here the shell covers
//!     the margin around the picture, which does not move.
//!   - **Panel rotation** (`set_panel_turn`) is accepted and ignored: the Mini
//!     Flip reports `rotate 0` and a landscape panel.

mod fb;
mod raster;

pub use fb::FbdevSurface;

use std::f32::consts::PI;

use crate::draw::{Draw, TexId};
use crate::grade::blue_light_gain;
use crate::lcd3x::ScreenEffect;
use crate::pipeline::{SCALE, SRC_H, SRC_W, WHOLE_TEXTURE};
use crate::power::screen_brightness;
use crate::surface::{GfxError, OUT_H, OUT_W, Surface, blit_rect};
use raster::Image;

pub const BACKDROP: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// The picture's natural size: the game at the GL path's `SCALE`. On a 640x480
/// panel it is wider than the canvas, so `picture_base` fits it first.
const GAME_W: u32 = 720;
const GAME_H: u32 = 480;
const PICTURE: [f32; 2] = [GAME_W as f32, GAME_H as f32];

/// The picture's size in canvas units: `base` shrunk, never stretched, to fit
/// the canvas and keep the base's aspect.
fn picture_base() -> [f32; 2] {
    fit_base(SRC_W, SRC_H)
}

/// The same for a frame of the core's own size, at the GL path's `SCALE`.
fn fit_base(w: u32, h: u32) -> [f32; 2] {
    let (nw, nh) = ((w * SCALE) as f32, (h * SCALE) as f32);
    let k = (OUT_W as f32 / nw).min(OUT_H as f32 / nh);
    if k >= 1.0 { [nw, nh] } else { [nw * k, nh * k] }
}

/// Where the picture sits on the canvas, centred, shrinking to a line for the
/// screen-off animation. The GL path calls this `power::screen_rect`; the
/// software backend keeps its own copy so the two can size the picture
/// differently.
fn screen_rect(t: f32, base: [f32; 2]) -> (f32, f32, f32, f32) {
    screen_rect_in((OUT_W, OUT_H), t, base)
}

/// The same placement on any canvas, so a test can use a panel whose size is
/// not this build's.
fn screen_rect_in(canvas: (u32, u32), t: f32, base: [f32; 2]) -> (f32, f32, f32, f32) {
    let w = base[0] * crate::power::screen_width(t);
    let h = base[1] * crate::power::screen_scale(t);
    (
        (canvas.0 as f32 - w) / 2.0,
        (canvas.1 as f32 - h) / 2.0,
        w,
        h,
    )
}

/// The most buffers a panel is likely to give us. Clearing a few frames more
/// than there are pages is far cheaper than tracking which page is which.
const PAGES: u32 = 4;

// The constants behind lcd3x_factors, which this backend evaluates separably
// rather than calling per pixel. Kept in step with crate::lcd3x by the test at
// the bottom of this file, which checks the split against the original.
const BRIGHTEN_SCANLINES: f32 = 16.0;
const BRIGHTEN_LCD: f32 = 4.0;
const LCD_OFFSETS: [f32; 3] = [PI / 2.0, PI * (0.5 - 2.0 / 3.0), PI * (0.5 - 4.0 / 3.0)];

/// Same rule as the GL compositor: an exact integer multiple of the output size
/// renders at `OUT_W x OUT_H` and is scaled on the way out, anything else
/// renders at the blit size so nothing is resampled twice.
pub fn canvas_size(window: (u32, u32)) -> (u32, u32) {
    let (_, _, w, h) = blit_rect(window, 0.0);
    let (w, h) = (w.max(1) as u32, h.max(1) as u32);
    if w % OUT_W == 0 && h % OUT_H == 0 && w / OUT_W == h / OUT_H {
        (OUT_W, OUT_H)
    } else {
        (w, h)
    }
}

struct Tex {
    img: Image,
    packed: Option<Vec<u8>>,
    used: bool,
    compressible: bool,
}

impl Tex {
    fn new(img: Image) -> Self {
        let mut tex = Self {
            img,
            packed: None,
            used: false,
            compressible: true,
        };
        tex.pack();
        tex
    }

    // Hidden menus and shelf faces otherwise occupy tens of MB on a GPU-less
    // device. Keep them losslessly packed, but leave this frame's draws hot so
    // a stationary shelf does not decompress the same pictures every frame.
    fn pack(&mut self) -> bool {
        if !self.compressible || self.packed.is_some() || self.img.px.is_empty() {
            return false;
        }
        let packed = lz4_flex::compress_prepend_size(&self.img.px);
        if packed.len() >= self.img.px.len() {
            self.compressible = false;
            return false;
        }
        self.packed = Some(packed);
        self.img.px = Vec::new();
        true
    }

    fn unpack(&mut self) {
        if let Some(packed) = self.packed.take() {
            self.img.px = lz4_flex::decompress_size_prepended(&packed)
                .expect("texture compressed by this process must round-trip");
        }
        self.used = true;
    }

    fn bytes(&self) -> usize {
        self.img.px.len() + self.packed.as_ref().map_or(0, Vec::len)
    }
}

/// Where the game pass writes: the canvas, or the panel directly.
enum Dest<'a> {
    Canvas(&'a mut Image),
    Fb(FbDest),
}

/// Enough of the framebuffer to write the game straight at it.
#[derive(Copy, Clone)]
struct FbDest {
    target: fb::Target,
    /// Where the canvas lands on the panel, in panel pixels.
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    /// The panel is mounted upside down, so rows and columns both reverse.
    half: bool,
}

impl Dest<'_> {
    fn width(&self) -> usize {
        match self {
            Dest::Canvas(c) => c.w as usize,
            Dest::Fb(f) => f.w as usize,
        }
    }

    fn height(&self) -> usize {
        match self {
            Dest::Canvas(c) => c.h as usize,
            Dest::Fb(f) => f.h as usize,
        }
    }

    /// The destination bytes for one row of the scaled image, `width * 4` long.
    ///
    /// # Safety
    /// For the framebuffer case the caller must not hold two rows at once; each
    /// call hands out a fresh borrow of a distinct row inside the mapping.
    unsafe fn row(&mut self, i: usize, x0: usize, width: usize) -> Option<&mut [u8]> {
        match self {
            Dest::Canvas(c) => {
                let start = (i * c.w as usize + x0) * 4;
                c.px.get_mut(start..start + width * 4)
            }
            Dest::Fb(f) => {
                // Upside down: the last row of the image goes to the first
                // scanline, and the columns were mirrored when the table was
                // built, so the row is written forwards from the far edge.
                let py = if f.half {
                    f.target.h as i32 - 1 - (f.y + i as i32)
                } else {
                    f.y + i as i32
                };
                if py < 0 || py >= f.target.h as i32 {
                    return None;
                }
                // The picture's own left edge, not the canvas rect's: they were
                // the same thing while the game filled the canvas, and are not
                // now that it is centred in it.
                let x = if f.half {
                    f.target.w as i32 - (f.x + x0 as i32 + width as i32)
                } else {
                    f.x + x0 as i32
                };
                if x < 0 || x + width as i32 > f.target.w as i32 {
                    return None;
                }
                let row = std::slice::from_raw_parts_mut(
                    f.target.ptr.add(py as usize * f.target.stride),
                    f.target.stride,
                );
                let o = x as usize * 4;
                row.get_mut(o..o + width * 4)
            }
        }
    }
}

/// One destination column: which source texel it reads, and the three x factors
/// in 8.8 fixed point (256 where the picture rect does not cover it, so the hot
/// loop needs no branch).
#[derive(Clone, Copy)]
struct Col {
    off: u32,
    k: [u16; 3],
}

/// What the column table depends on. Rebuilding it costs 2160 sin() calls, and
/// none of this changes while a game is running, so it is rebuilt only when one
/// of these does rather than every frame.
#[derive(Clone, Copy, PartialEq)]
struct ColKey {
    x0: i32,
    width: usize,
    rx: u32,
    rw: u32,
    src: [u32; 4],
    pic: [u32; 4],
    lcd: bool,
    img_w: u32,
    /// Built right-to-left, for writing straight at an upside-down panel.
    mirror: bool,
}

#[allow(clippy::too_many_arguments)]
fn build_col(
    out: &mut Vec<Col>,
    width: usize,
    x0: i32,
    rx: f32,
    rw: f32,
    sw: f32,
    src: [f32; 4],
    pic: [f32; 4],
    lcd: bool,
    img_w: u32,
    mirror: bool,
) {
    for i in 0..width {
        // Mirroring here, once per source row, is what lets the rotated panel
        // take the direct path: the NEON kernel only writes forwards, and
        // reversing afterwards would cost the copy the path exists to avoid.
        let i = if mirror { width - 1 - i } else { i };
        let px = x0 + i as i32;
        let u = src[0] + (px as f32 + 0.5 - rx) / rw * src[2];
        let texel = ((u * sw) as i32).clamp(0, img_w as i32 - 1) as u32;
        let inside = u >= pic[0] && u < pic[2];
        let mut k = [256u16; 3];
        if lcd && inside {
            let fx = (u * sw).fract();
            for (c, k) in k.iter_mut().enumerate() {
                // The canvas is in panel order, so byte 0 is blue.
                let o = LCD_OFFSETS[2 - c];
                let v = (BRIGHTEN_LCD + (2.0 * PI * fx + o).sin()) / (BRIGHTEN_LCD + 1.0);
                *k = (v * 256.0).round().clamp(0.0, 65535.0) as u16;
            }
        }
        out.push(Col { off: texel * 4, k });
    }
}

/// The game plane: a 240x160 frame plus everything `GAME_FRAG` needs.
struct Game {
    /// Source frame as RGBA, converted once on upload from the core's XRGB8888.
    frame: Image,
    effect: ScreenEffect,
    power: f32,
    pic: [f32; 4],
    src: [f32; 4],
    /// The picture's size in canvas units. Set from the video mode; see
    /// `power::screen_rect` and `video_mode::base_size`.
    base: [f32; 2],
    /// A strip along the bottom of the canvas the picture keeps clear of: the
    /// carousel footer, which the pause and quick menus share.
    bottom: f32,
    fbo_scale: [f32; 2],
    /// Scratch that would otherwise be heap-allocated twice a frame. Two
    /// allocations per frame is a poor way to spend a 16 ms budget, and the
    /// jitter shows up as stutter rather than as a worse average.
    col: Vec<Col>,
    col_key: Option<ColKey>,
    line: Vec<u16>,
}

impl Game {
    fn new() -> Self {
        Game {
            frame: Image::new(SRC_W, SRC_H),
            effect: ScreenEffect::Lcd3x,
            power: 1.0,
            pic: WHOLE_TEXTURE,
            src: WHOLE_TEXTURE,
            base: picture_base(),
            bottom: 0.0,
            fbo_scale: [1.0, 1.0],
            col: Vec::new(),
            col_key: None,
            line: Vec::new(),
        }
    }

    /// The core hands us XRGB8888, which is B,G,R,X in memory on little-endian.
    fn upload(&mut self, xrgb8888: &[u8]) {
        self.store(xrgb8888, self.frame.w, self.frame.h);
    }

    /// A frame of the core's own size, which is not always 240x160: a GB or GBC
    /// core hands over 160x144.
    fn upload_sized(&mut self, xrgb8888: &[u8], w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        if (w, h) != (self.frame.w, self.frame.h) {
            self.frame = Image::new(w, h);
            self.base = fit_base(w, h);
            self.col_key = None;
        }
        self.store(xrgb8888, w, h);
    }

    fn store(&mut self, xrgb8888: &[u8], w: u32, h: u32) {
        let want = (w * h * 4) as usize;
        if xrgb8888.len() < want || self.frame.px.len() < want {
            return;
        }
        // XRGB8888 is B, G, R, X in memory on little-endian, which is exactly
        // the order the canvas and the panel use. Nothing reads this image's
        // alpha -- the game plane writes opaque pixels and the output sets 255
        // itself -- so the whole frame is one copy rather than 38,400 three-byte
        // ones. That was several ms a frame and did not show up in the profile
        // because it happens before the parts that were being timed.
        self.frame.px[..want].copy_from_slice(&xrgb8888[..want]);
    }

    /// Returns 2 when it drew, 0 when there was nothing to draw.
    fn draw(&mut self, dst: &mut Image, tex: Option<&Image>) -> u8 {
        self.draw_inner(Dest::Canvas(dst), tex, false)
    }

    /// Scale straight into the framebuffer, skipping the canvas entirely. Worth
    /// a whole pass over 1.4 MB when the display list is nothing but the game.
    fn draw_direct(&mut self, fb: FbDest, tex: Option<&Image>) -> u8 {
        let mirror = fb.half;
        self.draw_inner(Dest::Fb(fb), tex, mirror)
    }

    /// Where the picture lands on the canvas.
    ///
    /// Upstream crops the source by the footer's share of the canvas as well
    /// as moving the picture up, because there the picture *is* the canvas and
    /// the footer covers its bottom rows. Here the picture is smaller than the
    /// canvas -- 720x480 in 752x560 -- so there is nothing of it under the
    /// footer to lose: it only moves up, and keeps all of itself.
    fn rect(&self, canvas: (u32, u32)) -> (f32, f32, f32, f32) {
        let (x, _, w, h) = screen_rect_in(canvas, self.power, self.base);
        let view = canvas.1 as f32 - self.bottom.min(canvas.1 as f32);
        (x, (view - h) / 2.0, w, h)
    }

    fn draw_inner(&mut self, mut dest: Dest<'_>, tex: Option<&Image>, mirror: bool) -> u8 {
        let img = tex.unwrap_or(&self.frame);
        let canvas = (dest.width() as u32, dest.height() as u32);
        let (rx, ry, rw, rh) = self.rect(canvas);
        if !(rw > 0.0 && rh > 0.0) || img.w == 0 || img.h == 0 {
            return 0;
        }
        let src = if tex.is_some() {
            WHOLE_TEXTURE
        } else {
            self.src
        };
        let (sw, sh) = (img.w as f32, img.h as f32);

        let x0 = (rx.round() as i32).max(0);
        let y0 = (ry.round() as i32).max(0);
        let x1 = ((rx + rw).round() as i32).min(dest.width() as i32);
        let y1 = ((ry + rh).round() as i32).min(dest.height() as i32);
        if x0 >= x1 || y0 >= y1 {
            return 0;
        }

        // The whole inner loop is integer. Everything that varies only with the
        // column, or only with the row, is computed once into these tables:
        // without that this does four sin() per pixel through lcd3x_factors,
        // which is roughly 83 million sin() a second at 60 fps over the game
        // area, and the Cortex-A7 cannot come close to that.
        //
        // The mask is separable, so the split is exact rather than an
        // approximation: lcd3x_factors is yfactor(y) * xfactor(x) per channel.
        let lcd = self.effect == ScreenEffect::Lcd3x;
        let bright = screen_brightness(self.power);
        let width = (x1 - x0) as usize;

        // Per column, flattened into one table: the byte offset of the source
        // texel within its row, and the three x factors in 8.8 fixed point
        // (256 where the picture rect does not cover this column, so the inner
        // loop has no branch and no second lookup).
        let bits = |v: [f32; 4]| v.map(f32::to_bits);
        let key = ColKey {
            x0,
            width,
            mirror,
            rx: rx.to_bits(),
            rw: rw.to_bits(),
            src: bits(src),
            pic: bits(self.pic),
            lcd,
            img_w: img.w,
        };
        if self.col_key != Some(key) {
            self.col.clear();
            self.col.reserve(width);
            build_col(
                &mut self.col,
                width,
                x0,
                rx,
                rw,
                sw,
                src,
                self.pic,
                lcd,
                img.w,
                mirror,
            );
            self.col_key = Some(key);
        }
        let col = &self.col;
        if self.line.len() != width * 4 {
            self.line.resize(width * 4, 0);
        }
        let line = &mut self.line;

        let row_bytes = img.w as usize * 4;

        // The vertical scale reuses each source row several times over -- 480
        // output rows from 144 of source for GB -- so the gather is hoisted out
        // of the inner loop. Each *source* row is expanded once into `line`,
        // destination-width, with the per-column mask already folded in as 8.8.
        // Every output row is then a linear multiply by the row's own factor,
        // with no gather left in it, which is both ~3x less scatter-reading and
        // a shape the compiler can actually do something with.
        let mut built: Option<(usize, bool)> = None;

        for py in y0..y1 {
            let v = src[1] + (py as f32 + 0.5 - ry) / rh * src[3];
            let sy = ((v * sh) as i32).clamp(0, img.h as i32 - 1) as usize;
            let row_inside = v >= self.pic[1] && v < self.pic[3];

            // The y half of the mask, with brightness folded in.
            let yk = if lcd && row_inside {
                let fy = (v * sh).fract();
                (BRIGHTEN_SCANLINES + (2.0 * PI * fy).sin()) / (BRIGHTEN_SCANLINES + 1.0) * bright
            } else {
                bright
            };
            let yk = (yk * 256.0).round().clamp(0.0, 65535.0) as u32;

            if built != Some((sy, row_inside)) {
                let srow = &img.px[sy * row_bytes..(sy + 1) * row_bytes];
                for (e, c) in line.chunks_exact_mut(4).zip(col.iter()) {
                    // SAFETY: `off` is a texel index clamped to img.w - 1 times
                    // four, and `srow` is a whole source row, so off + 4 is in
                    // bounds. Reading a fixed-size array is what drops the
                    // bounds checks on the channels below.
                    let s = unsafe { &*(srow.as_ptr().add(c.off as usize) as *const [u8; 4]) };
                    if row_inside {
                        for ch in 0..3 {
                            e[ch] = s[ch] as u16 * c.k[ch];
                        }
                    } else {
                        // Outside the picture the x mask is 1, so 8.8 of itself.
                        for ch in 0..3 {
                            e[ch] = (s[ch] as u16) << 8;
                        }
                    }
                    e[3] = u16::MAX;
                }
                built = Some((sy, row_inside));
            }

            // SAFETY: one row at a time, never two at once.
            let Some(drow) = (unsafe { dest.row(py as usize, x0 as usize, width) }) else {
                continue;
            };
            // Alpha rides through the same multiply rather than being stored
            // separately, which keeps the kernel to one uniform pass. The line's
            // alpha lane is 0xffff, so it lands at 255 whenever the row factor
            // is 1.0 or more and a little under when the screen is dimming.
            // Nothing reads it: the output writes its own alpha, and sprite
            // blending only uses the canvas alpha to update itself.
            // SAFETY: both slices are width * 4 long.
            unsafe { scale_row(drow, line, yk as u16) };
        }

        2
    }
}

/// How the panel is mounted relative to the framebuffer's scan order.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Mount {
    Normal,
    /// The LCD is fitted upside down, so both axes reverse on the way out.
    /// The Mini Flip is like this: a frame written top-down appears inverted.
    Half,
}

impl Mount {
    /// `SLOT_FB_ROTATE=180` selects the upside-down panel. Set per device in the
    /// card's launch script rather than guessed at here.
    fn from_env() -> Self {
        match std::env::var("SLOT_FB_ROTATE").as_deref() {
            Ok("180") => Mount::Half,
            _ => Mount::Normal,
        }
    }
}

/// Where the backend's frame time goes, printed every `REPORT_EVERY` frames
/// when `SLOT_TRACE_LATENCY` is set. The frame breakdown slot already prints
/// only says this backend took N ms; this says which part, and which path the
/// game pass picked, which is the thing worth knowing before optimising it
/// again.
#[derive(Default)]
struct Profile {
    frames: u32,
    clear_us: u64,
    upload_us: u64,
    game_us: u64,
    sprite_us: u64,
    out_us: u64,
    /// The worst single frame in the window. The median says whether we are in
    /// budget on average; this says whether the stutters are the renderer.
    worst_us: u64,
    frame_us: u64,
    /// Rects, which turned out to be almost all of the shelf's draws: the
    /// reuse counters below only cover the textured ones.
    rects: u64,
    /// Draws that took the row-reuse path in the sprite blit, and draws that
    /// gathered per destination row instead. The reuse only pays scaling up, so
    /// this says whether the shelf is even reaching it.
    reused: u64,
    gathered: u64,
    /// Rects that blended per pixel, and the pixels written by each rect path.
    blend_rects: u64,
    opaque_px: u64,
    blend_px: u64,
    /// The sprite stage split by draw kind. 186 of 193 draws are rects and
    /// they write 0.14 screens between them, which cannot be the 11 ms the
    /// stage costs -- so the seven textured draws are timed rather than
    /// inferred.
    kind_us: [u64; 3],
    kind_n: [u64; 3],
    /// The last frame's textured draws: destination size, source size and the
    /// microseconds each took. Eight draws carry 20-30 ms of a 16.77 ms
    /// period, so the question is now what they are -- how big, and scaled up
    /// or down, since the row-reuse path only pays scaling up.
    tex_shape: [(u32, u32, u32, u32, u64); 8],
    /// Per textured draw: texture id, alpha in thousandths, and the pixel
    /// classes. Thousandths because rounding it to 0-255 printed an alpha of
    /// 0.998 as "255" and hid the reason a draw was slow -- `plain` wants
    /// exact equality with 1.0, and 0.998 is not it.
    /// The id says whether a fast draw was simply the same texture again, warm
    /// in cache, which is the other half of the 0.46-vs-3.3 ms question.
    tex_who: [(u32, u32, u32, u32, u32); 8],
    tex_seen: usize,
    /// Sprites drawn in the window, so the sprite cost can be read per draw
    /// rather than per frame. Without it two captures of "the shelf" are not
    /// comparable: its cost scales with how many carts and labels are on
    /// screen, which is exactly how an earlier sprite change went unjudgeable.
    draws: u64,
    /// Whether the game went straight to the panel, skipping the canvas. When
    /// it did, `out` includes `game` rather than being separate work.
    direct: bool,
    /// 0 nothing drawn, 2 the separable path. (1 was a 3x lookup, removed: it
    /// only ever fitted GBA, since GB and GBC are 160x144 inside a 240x160
    /// buffer at 4.5x, and on the one run where it did ship it measured slower
    /// than the multiply it replaced.)
    path: u8,
}

/// What a texture's alpha looks like at upload, since that is the thing the
/// blit branches on: it copies a texel whose alpha is 255 and
/// read-modify-writes anything less. On the handheld two draws of identical
/// geometry measured 2.5 ms and 16.0 ms, one copying 98,810 pixels and the
/// other blending 99,520, and no amount of reading the builders explained
/// which texture was which. This says so at the source.
fn report_alpha(id: usize, w: u32, h: u32, rgba: &[u8]) {
    let (mut full, mut clear, mut partial) = (0u32, 0u32, 0u32);
    let mut common = std::collections::BTreeMap::new();
    for px in rgba.chunks_exact(4) {
        match px[3] {
            255 => full += 1,
            0 => clear += 1,
            a => {
                partial += 1;
                *common.entry(a).or_insert(0u32) += 1;
            }
        }
    }
    let worst = common
        .iter()
        .max_by_key(|(_, n)| **n)
        .map(|(a, n)| (*a, *n));
    eprintln!(
        "slot: soft: texture {id} {w}x{h}: opaque {full}, clear {clear}, partial {partial}{}",
        match worst {
            Some((a, n)) if partial > full => format!(" -- MOSTLY PARTIAL, commonest {a} x{n}"),
            Some((a, n)) => format!(", commonest partial {a} x{n}"),
            None => String::new(),
        }
    );
}

const REPORT_EVERY: u32 = 60;

impl Profile {
    fn on() -> bool {
        std::env::var_os("SLOT_TRACE_LATENCY").is_some_and(|v| v != "0")
    }

    fn tick(&mut self, frame_us: u64) {
        self.frames += 1;
        self.worst_us = self.worst_us.max(frame_us);
        if self.frames < REPORT_EVERY {
            return;
        }
        let n = self.frames as u64;
        let ms = |v: u64| v as f64 / 1000.0 / n as f64;
        eprintln!(
            "slot: soft: clear {:.1} upload {:.1} game {:.1}{} sprites {:.1} over {} draws, out {:.1} ms/frame, worst {:.1}, game path {}",
            ms(self.clear_us),
            ms(self.upload_us),
            ms(self.game_us),
            if self.direct { " (inside out)" } else { "" },
            ms(self.sprite_us),
            self.draws / n,
            ms(self.out_us),
            self.worst_us as f64 / 1000.0,
            match self.path {
                2 => "separable",
                _ => "none",
            }
        );
        eprintln!(
            "slot: soft: {} rects, textured rows reused on {} draws, gathered on {} (reuse only pays scaling up)",
            self.rects / n.max(1),
            self.reused / n.max(1),
            self.gathered / n.max(1),
        );
        // A canvas is 752x560 = 421,120 pixels, so "screens" says how much of
        // the per-frame cost is overdraw rather than area.
        let screens = |px: u64| px as f64 / n as f64 / (OUT_W as f64 * OUT_H as f64);
        for (i, (dw, dh, sw, sh, us)) in self.tex_shape.iter().copied().enumerate() {
            if dw == 0 && dh == 0 {
                continue;
            }
            let (dpx, spx) = (dw as f64 * dh as f64, sw as f64 * sh as f64);
            let (id, a, skip, copy, blend) = self.tex_who[i];
            eprintln!(
                "slot: soft: tex {i}: id {id} alpha {:.3} {dw}x{dh} from {sw}x{sh} ({:.2}x {}) {:.2} ms {:.0} ns/px -- skip {skip} copy {copy} blend {blend}",
                a as f64 / 1000.0,
                (dpx / spx).sqrt(),
                if dpx >= spx { "up" } else { "DOWN" },
                us as f64 / 1000.0,
                us as f64 * 1000.0 / dpx.max(1.0),
            );
        }
        eprintln!(
            "slot: soft: sprite stage by kind: rect {:.1} ms over {}, tex {:.1} ms over {}, turned {:.1} ms over {}",
            ms(self.kind_us[0]),
            self.kind_n[0] / n.max(1),
            ms(self.kind_us[1]),
            self.kind_n[1] / n.max(1),
            ms(self.kind_us[2]),
            self.kind_n[2] / n.max(1),
        );
        eprintln!(
            "slot: soft: rect pixels: opaque {:.2} screens, blended {:.2} screens on {} of {} rects",
            screens(self.opaque_px),
            screens(self.blend_px),
            self.blend_rects / n.max(1),
            self.rects / n.max(1),
        );
        *self = Profile {
            path: self.path,
            direct: self.direct,
            ..Default::default()
        };
    }
}

/// Scale a prepared row by one 8.8 factor, writing 8.8 fixed values down to
/// bytes: `dst[i] = min(255, (line[i] * yk) >> 16)`.
///
/// This is the backend's hot loop. `line` holds the source row already expanded
/// to destination width with the column mask folded in, so there is no gather
/// left and the work is a straight widening multiply — which is exactly what
/// NEON is for, and what LLVM declines to emit here (4-byte stride, three
/// channels of four, u16 to u32 to u8).
///
/// Rust's armv7 NEON intrinsics are unstable and upstream pins stable, so this
/// is inline asm. `tests` drives it against `scale_row_scalar` under qemu.
#[cfg(target_arch = "arm")]
unsafe fn scale_row(dst: &mut [u8], line: &[u16], yk: u16) {
    let n = dst.len().min(line.len());
    let chunks = n / 8;
    if chunks > 0 {
        let mut s = line.as_ptr();
        let mut d = dst.as_mut_ptr();
        for _ in 0..chunks {
            // SAFETY: `chunks` counts whole groups of eight inside both slices.
            core::arch::asm!(
                // The target feature cannot be named on stable for armv7, so
                // tell the assembler directly; cortex-a7 always has NEON.
                ".fpu neon",
                // A 16-bit scalar operand must live in d0-d7, so k goes in d4
                // (the low half of q2) rather than anywhere convenient.
                "vdup.16 d4, {k}",
                "vld1.16 {{d0, d1}}, [{s}]",
                "vmull.u16 q8, d0, d4[0]",
                "vmull.u16 q9, d1, d4[0]",
                "vshrn.i32 d6, q8, #16",
                "vshrn.i32 d7, q9, #16",
                // Saturating narrow: this is where the min(255) happens.
                "vqmovn.u16 d0, q3",
                "vst1.8 {{d0}}, [{d}]",
                k = in(reg) yk as u32,
                s = in(reg) s,
                d = in(reg) d,
                out("q0") _,
                out("q2") _,
                out("q3") _,
                out("q8") _,
                out("q9") _,
                options(nostack),
            );
            s = s.add(8);
            d = d.add(8);
        }
    }
    // The tail, when the row is not a multiple of eight.
    for i in chunks * 8..n {
        dst[i] = (((line[i] as u32 * yk as u32) >> 16).min(255)) as u8;
    }
}

#[cfg(not(target_arch = "arm"))]
unsafe fn scale_row(dst: &mut [u8], line: &[u16], yk: u16) {
    scale_row_scalar(dst, line, yk)
}

/// The reference the NEON kernel is tested against, and the fallback off-device.
/// On arm it is reachable only from the tests, which is the point of it.
#[allow(dead_code)]
fn scale_row_scalar(dst: &mut [u8], line: &[u16], yk: u16) {
    let n = dst.len().min(line.len());
    for i in 0..n {
        dst[i] = (((line[i] as u32 * yk as u32) >> 16).min(255)) as u8;
    }
}

pub struct Compositor {
    canvas: Image,
    /// Column table and row line for the sprite blits, kept across calls and
    /// across frames: ~190 draws a frame each allocated their own otherwise.
    scratch: raster::Scratch,
    profile: Profile,
    profiling: bool,
    mount: Mount,
    /// The last rect written to the panel, and how many more frames must
    /// repaint the surround. The shake offset moves the rect, and skipping the
    /// surround then leaves the pixels it vacated on screen -- visible as
    /// flicker at the edges. One page is cleared per frame, so a change has to
    /// be followed through every page before the fast path is safe again.
    last_out: Option<(i32, i32, i32, i32)>,
    clear_left: u32,
    /// Set by `begin_frame`, discharged by `draw_list` or `end_frame`.
    pending_clear: bool,
    /// Whether the previous frame went straight to the panel, so the switch
    /// back to compositing knows the panel and the canvas have diverged.
    was_direct: bool,
    /// The display list was nothing but a full-canvas game, so the game pass is
    /// deferred to `end_frame` and scaled straight onto the panel. That skips
    /// the canvas entirely: a 1.4 MB write here and a 1.4 MB read-and-write
    /// there, every frame, purely to stage a picture nothing else draws on.
    direct_game: bool,
    textures: Vec<Option<Tex>>,
    white: TexId,
    game: Game,
    /// A still taken because the display list asked for one (`Draw::Capture`).
    captured: Option<(u32, u32, Vec<u8>)>,
    gain: [f32; 3],
    shake: f32,
    lift: f32,
    screen: f32,
    bezel: Option<Image>,
    /// The bezel that belongs to the console being played, drawn in the band
    /// of canvas the picture does not reach: `App::shown_game_bezel`.
    game_bezel: Option<TexId>,
    /// Whether the bezel belongs under what is playing. The art is a Game Boy
    /// Advance SP shell and says so in large letters, which is wrong under a
    /// Game Boy game; `App::bezel_suits_platform` decides and says so here.
    bezel_shown: bool,
    turn: Option<((u32, u32), bool)>,
}

impl Compositor {
    pub fn new(_surface: &dyn Surface) -> Result<Self, GfxError> {
        // No gl::load: there is no GL to load. The surface is only here so the
        // signature matches the GL compositor.
        let mut me = Compositor {
            canvas: Image::new(OUT_W, OUT_H),
            scratch: raster::Scratch::default(),
            profile: Profile::default(),
            profiling: Profile::on(),
            mount: Mount::from_env(),
            last_out: None,
            clear_left: 0,
            pending_clear: false,
            was_direct: false,
            direct_game: false,
            textures: Vec::new(),
            white: TexId::from_raw(0),
            game: Game::new(),
            captured: None,
            gain: blue_light_gain(0),
            shake: 0.0,
            lift: 1.0,
            screen: 0.0,
            bezel: None,
            game_bezel: None,
            bezel_shown: true,
            turn: None,
        };
        me.white = me.create_texture(1, 1, &[255, 255, 255, 255]);
        Ok(me)
    }

    pub fn set_panel_turn(&mut self, panel: (u32, u32), ccw: bool) -> Result<(), GfxError> {
        self.turn = Some((panel, ccw));
        Ok(())
    }

    pub fn upload_game_sized(&mut self, xrgb8888: &[u8], w: u32, h: u32) {
        let t = std::time::Instant::now();
        self.game.upload_sized(xrgb8888, w, h);
        if self.profiling {
            let us = t.elapsed().as_micros() as u64;
            self.profile.upload_us += us;
            self.profile.frame_us += us;
        }
    }

    pub fn upload_game(&mut self, xrgb8888: &[u8]) {
        let t = std::time::Instant::now();
        self.game.upload(xrgb8888);
        if self.profiling {
            {
                let us = t.elapsed().as_micros() as u64;
                self.profile.upload_us += us;
                self.profile.frame_us += us;
            }
        }
    }

    pub fn draw_game(&mut self) {
        let t = std::time::Instant::now();
        let path = self.game.draw(&mut self.canvas, None);
        if self.profiling {
            {
                let us = t.elapsed().as_micros() as u64;
                self.profile.game_us += us;
                self.profile.frame_us += us;
            }
            self.profile.path = path;
        }
    }

    /// Does the deferred clear, unless the list begins with a game that covers
    /// the whole canvas and will therefore overwrite every pixel anyway.
    fn discharge_clear(&mut self, items: &[Draw]) {
        if !self.pending_clear {
            return;
        }
        self.pending_clear = false;
        let covered = matches!(items.first(), Some(Draw::Game)) && {
            let (x, y, w, h) = screen_rect(self.game.power, self.game.base);
            x <= 0.0 && y <= 0.0 && w >= self.canvas.w as f32 && h >= self.canvas.h as f32
        };
        if covered {
            return;
        }
        let t = std::time::Instant::now();
        // Alpha 0 rather than 255, which makes the backdrop uniform and the
        // clear a memset. Nothing downstream reads the canvas alpha: the output
        // writes its own, and sprite blending only uses it to update itself.
        let b = |v: f32| (v * 255.0).clamp(0.0, 255.0) as u8;
        self.canvas
            .fill([b(BACKDROP[0]), b(BACKDROP[1]), b(BACKDROP[2]), 0]);
        if self.profiling {
            let us = t.elapsed().as_micros() as u64;
            self.profile.clear_us += us;
            self.profile.frame_us += us;
        }
    }

    /// Same splitting as the GL compositor: sprites batch up to a `Game` or
    /// `Shot`, which draws the game plane, then the batch continues.
    pub fn draw_list(&mut self, items: &[Draw]) {
        self.discharge_clear(items);
        // Exactly one item, and it covers everything: nothing can be drawn over
        // it and nothing shows through, so it can go straight to the panel.
        // end_frame checks the rest -- scale, byte order, blue light -- since
        // only it knows the panel.
        // Nothing but the game, so nothing can be drawn over it and nothing
        // shows through. It no longer has to cover the whole canvas: since the
        // canvas became the panel the picture is centred in it, and the
        // surround around the picture is backdrop that end_frame clears when the
        // rect moves or when this path is entered.
        self.direct_game = matches!(items, [Draw::Game]);
        if self.direct_game {
            self.was_direct = true;
            return;
        }
        // Coming back from the direct path, whatever it left on the panel is
        // not what the canvas holds, so the canvas cannot be trusted as a
        // starting point.
        if self.was_direct {
            self.was_direct = false;
            self.clear_left = PAGES;
        }
        if self.direct_game {
            return;
        }
        let mut from = 0;
        for (i, item) in items.iter().enumerate() {
            match *item {
                Draw::Game => {
                    self.draw_sprites(&items[from..i]);
                    let t = std::time::Instant::now();
                    let path = self.game.draw(&mut self.canvas, None);
                    if self.profiling {
                        {
                            let us = t.elapsed().as_micros() as u64;
                            self.profile.game_us += us;
                            self.profile.frame_us += us;
                        }
                        self.profile.path = path;
                    }
                }
                Draw::Capture => {
                    self.draw_sprites(&items[from..i]);
                    self.captured = Some(self.capture_rgba());
                }
                Draw::Shot { tex } => {
                    self.draw_sprites(&items[from..i]);
                    // Borrow the still out of the store so `game` can read it
                    // while `canvas` is mutably borrowed.
                    if let Some(img) = self.take_image(tex) {
                        let _ = self.game.draw(&mut self.canvas, Some(&img));
                        self.put_image(tex, img);
                    }
                }
                _ => continue,
            }
            from = i + 1;
        }
        self.draw_sprites(&items[from..]);
    }

    fn take_image(&mut self, id: TexId) -> Option<Image> {
        self.textures.get_mut(id.raw())?.take().map(|mut t: Tex| {
            t.unpack();
            t.img
        })
    }

    fn put_image(&mut self, id: TexId, img: Image) {
        if let Some(slot) = self.textures.get_mut(id.raw()) {
            *slot = Some(Tex { img, packed: None, used: true, compressible: true });
        }
    }

    fn draw_sprites(&mut self, items: &[Draw]) {
        let t = std::time::Instant::now();
        self.scratch.reused = 0;
        self.scratch.direct = 0;
        (
            self.scratch.blend_rects,
            self.scratch.opaque_px,
            self.scratch.blend_px,
        ) = (0, 0, 0);
        // Per frame, so the shapes printed below are one frame's draws rather
        // than a mixture: the report fires every 60.
        self.profile.tex_seen = 0;
        self.draw_sprites_inner(items);
        if self.profiling {
            self.profile.draws += items.len() as u64;
            self.profile.reused += self.scratch.reused as u64;
            self.profile.gathered += self.scratch.direct as u64;
            self.profile.blend_rects += self.scratch.blend_rects as u64;
            self.profile.opaque_px += self.scratch.opaque_px;
            self.profile.blend_px += self.scratch.blend_px;
            {
                let us = t.elapsed().as_micros() as u64;
                self.profile.sprite_us += us;
                self.profile.frame_us += us;
            }
        }
    }

    fn draw_sprites_inner(&mut self, items: &[Draw]) {
        for item in items {
            // A texture this backend packed to save memory is unpacked here, once
            // per draw that uses it. `Tex::pack` skips anything that does not
            // shrink, so a frame's worth of work is only ever paid for the
            // pictures on screen.
            let tex = match *item {
                Draw::Tex { tex, .. } | Draw::Part { tex, .. } | Draw::Turned { tex, .. } => Some(tex),
                _ => None,
            };
            if let Some(t) = tex.and_then(|id| self.textures.get_mut(id.raw())) {
                if let Some(t) = t.as_mut() {
                    t.unpack();
                }
            }
            // Per draw, and only while profiling: 193 clock reads a frame is
            // ~20 us here, worth paying to learn which kind owns the stage.
            let at = self.profiling.then(std::time::Instant::now);
            let shape = match *item {
                Draw::Tex { w, h, tex, .. }
                | Draw::Part { w, h, tex, .. }
                | Draw::Turned { w, h, tex, .. } => self
                    .textures
                    .get(tex.raw())
                    .and_then(|t| t.as_ref())
                    .map(|t| (w.round() as u32, h.round() as u32, t.img.w, t.img.h)),
                _ => None,
            };
            let who = match *item {
                Draw::Tex { tex, alpha, .. }
                | Draw::Part { tex, alpha, .. }
                | Draw::Turned { tex, alpha, .. } => {
                    Some((tex.raw() as u32, (alpha * 1000.0).round() as u32))
                }
                _ => None,
            };
            if shape.is_some() {
                (
                    self.scratch.tex_skip,
                    self.scratch.tex_copy,
                    self.scratch.tex_blend,
                ) = (0, 0, 0);
            }
            let kind = match item {
                Draw::Rect { .. } => 0usize,
                Draw::Tex { .. } | Draw::Part { .. } => 1,
                Draw::Turned { .. } => 2,
                _ => 3,
            };
            match *item {
                Draw::Rect { x, y, w, h, colour } => {
                    self.profile.rects += 1;
                    raster::fill_rect(
                        &mut self.canvas,
                        x,
                        y,
                        w,
                        h,
                        raster::to_storage(colour),
                        &mut self.scratch,
                    )
                }
                Draw::Tex {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    alpha,
                } => {
                    if let Some(t) = self.textures.get(tex.raw()).and_then(|t| t.as_ref()) {
                        raster::blit(
                            &mut self.canvas,
                            &t.img,
                            x,
                            y,
                            w,
                            h,
                            raster::to_storage([1.0, 1.0, 1.0, alpha]),
                            &mut self.scratch,
                        );
                    }
                }
                Draw::Part {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    alpha,
                    uv,
                } => {
                    if let Some(t) = self.textures.get(tex.raw()).and_then(|t| t.as_ref()) {
                        raster::blit_part(
                            &mut self.canvas,
                            &t.img,
                            x,
                            y,
                            w,
                            h,
                            raster::to_storage([1.0, 1.0, 1.0, alpha]),
                            uv,
                            &mut self.scratch,
                        );
                    }
                }
                Draw::Turned {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    alpha,
                    turn,
                } => {
                    if let Some(t) = self.textures.get(tex.raw()).and_then(|t| t.as_ref()) {
                        raster::blit_turned(
                            &mut self.canvas,
                            &t.img,
                            x,
                            y,
                            w,
                            h,
                            raster::to_storage([1.0, 1.0, 1.0, alpha]),
                            turn,
                            &mut self.scratch,
                        );
                    }
                }
                Draw::Game | Draw::Shot { .. } | Draw::Capture => {}
            }
            if let (Some(at), true) = (at, kind < 3) {
                let us = at.elapsed().as_micros() as u64;
                self.profile.kind_us[kind] += us;
                self.profile.kind_n[kind] += 1;
                if let (Some((dw, dh, sw, sh)), Some((id, a))) = (shape, who) {
                    let at = self.profile.tex_seen.min(7);
                    self.profile.tex_shape[at] = (dw, dh, sw, sh, us);
                    self.profile.tex_who[at] = (
                        id,
                        a,
                        self.scratch.tex_skip,
                        self.scratch.tex_copy,
                        self.scratch.tex_blend,
                    );
                    self.profile.tex_seen += 1;
                }
            }
        }
    }

    pub fn create_texture(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        let id = self.textures.len();
        if self.profiling {
            report_alpha(id, w, h, rgba);
        }
        self.textures.push(Some(Tex::new(Image::from_rgba(w, h, rgba))));
        TexId::from_raw(id)
    }

    /// Nearest and linear are the same here: everything is nearest-sampled.
    /// The UI draws its faces at 1:1, so this costs nothing visible yet.
    pub fn create_texture_nearest(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.create_texture(w, h, rgba)
    }

    pub fn update_texture(&mut self, id: TexId, w: u32, h: u32, rgba: &[u8]) {
        if self.profiling {
            report_alpha(id.raw(), w, h, rgba);
        }
        if let Some(slot) = self.textures.get_mut(id.raw()) {
            *slot = Some(Tex::new(Image::from_rgba(w, h, rgba)));
        }
    }

    /// How many textures are held and the bytes of pixels behind them. With no
    /// GPU memory to put them in, they are slot's own memory on this backend, and
    /// a report of where a small console's memory went needs the figure.
    pub fn texture_bytes(&self) -> (usize, usize) {
        self.textures
            .iter()
            .flatten()
            .fold((0, 0), |(n, b), t| (n + 1, b + t.bytes()))
    }

    /// Frees a texture's pixels and keeps its id, which draws nothing until
    /// `update_texture` puts an image there again. A game that fills the screen
    /// needs none of the shelf's faces, and on a console with no GPU memory they
    /// are slot's own.
    pub fn release_texture(&mut self, id: TexId) {
        if let Some(slot) = self.textures.get_mut(id.raw()) {
            *slot = None;
        }
    }

    /// The largest groups of same-sized textures: (width, height, how many).
    pub fn texture_sizes(&self, top: usize) -> Vec<(u32, u32, usize)> {
        let mut groups: Vec<(u32, u32, usize)> = Vec::new();
        for t in self.textures.iter().flatten() {
            match groups.iter_mut().find(|g| g.0 == t.img.w && g.1 == t.img.h) {
                Some(g) => g.2 += 1,
                None => groups.push((t.img.w, t.img.h, 1)),
            }
        }
        groups.sort_by_key(|g| std::cmp::Reverse(g.0 as usize * g.1 as usize * g.2));
        groups.truncate(top);
        groups
    }

    pub fn set_blue_light(&mut self, step: u8) {
        self.gain = blue_light_gain(step);
    }

    pub fn set_screen_effect(&mut self, effect: ScreenEffect) {
        self.game.effect = effect;
    }

    pub fn set_picture(&mut self, rect: [f32; 4]) {
        self.game.pic = rect;
    }

    /// The paper texture only feeds the Simpletex effect, which this backend
    /// does not implement; kept so the API matches.
    pub fn set_paper(&mut self, _size: u32, _gray: &[u8]) {}

    pub fn fit(&mut self, window: (u32, u32)) {
        let size = canvas_size(window);
        if (size.0, size.1) != (self.canvas.w, self.canvas.h) {
            self.canvas = Image::new(size.0, size.1);
        }
    }

    pub fn set_overlay_slide(&mut self, t: f32) {
        self.lift = t;
    }

    pub fn set_screen_footer(&mut self, px: f32) {
        self.game.bottom = px.max(0.0);
    }

    /// The canvas, which is the panel here: there is no offscreen target to be
    /// a different size from it.
    pub fn frame_size(&self) -> (u32, u32) {
        (self.canvas.w, self.canvas.h)
    }

    pub fn set_screen_power(&mut self, t: f32) {
        self.screen = t;
        self.game.power = t.clamp(0.0, 1.0);
    }

    pub fn set_game_base(&mut self, base: [f32; 2]) {
        self.game.base = base;
    }

    pub fn set_game_source_rect(&mut self, rect: [f32; 4]) {
        self.game.src = rect;
    }

    pub fn set_shake(&mut self, dx: f32) {
        self.shake = dx;
    }

    /// Top-down RGBA, as the GL path returns after its flip. The canvas is kept
    /// in panel order, so convert on the way out.
    pub fn take_capture(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        self.captured.take()
    }

    /// The canvas as RGBA, which is what a screenshot and a state thumbnail
    /// want. The canvas is in panel order (blue first), so the channels swap.
    fn capture_rgba(&self) -> (u32, u32, Vec<u8>) {
        let (w, h) = (self.canvas.w, self.canvas.h);
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for px in self.canvas.px.chunks_exact(4) {
            out.extend_from_slice(&[px[2], px[1], px[0], 255]);
        }
        (w, h, out)
    }

    pub fn read_frame(&self) -> Vec<u8> {
        let mut out = self.canvas.px.clone();
        for p in out.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        out
    }

    pub fn begin_frame(&mut self) {
        for t in self.textures.iter_mut().flatten() {
            t.used = false;
        }
        self.game.fbo_scale = [
            self.canvas.w as f32 / OUT_W as f32,
            self.canvas.h as f32 / OUT_H as f32,
        ];
        // The clear is deferred rather than done here. In play the display list
        // is a single Draw::Game covering the whole canvas, so clearing first
        // writes 1.4 MB that the game pass immediately overwrites. draw_list
        // decides; anything that does not fully cover still gets its clear.
        self.pending_clear = true;
    }

    /// Whether the panel bezel -- the Advance SP shell -- suits what is
    /// playing. Upstream's name for it, and upstream's answer: a Game Boy game
    /// gets its own bezel instead, through `set_game_bezel`.
    pub fn set_sp_frame(&mut self, shown: bool) {
        if self.bezel_shown == shown {
            return;
        }
        self.bezel_shown = shown;
        // The surround is painted only when the output rect moves, so without
        // this the answer that was on the panel stays there -- the shell would
        // survive into the next Game Boy game until something else happened to
        // move the picture.
        self.clear_left = PAGES;
    }

    pub fn set_game_bezel(&mut self, tex: Option<TexId>) {
        if self.game_bezel == tex {
            return;
        }
        self.game_bezel = tex;
        // Painted with the surround rather than per frame, so the panel has to
        // be told to repaint: see `output_frame`.
        self.clear_left = PAGES;
    }

    pub fn set_bezel(&mut self, w: u32, h: u32, rgba: &[u8]) {
        // Stored top-down, unlike the GL path which pre-flips for its FBO.
        // Unused until a 4:3 panel is supported; see the module note.
        self.bezel = Some(Image::from_rgba(w, h, rgba));
    }

    /// Scale the canvas onto the panel and hand it to the framebuffer.
    ///
    /// Only the area the canvas lands on is written. The surround is black and
    /// `FbdevSurface::new` clears every page once, so repainting it each frame
    /// would be ~75k pixels of wasted work per frame on this panel.
    pub fn end_frame(&mut self, window: (u32, u32)) {
        let t = std::time::Instant::now();
        self.output_frame(window);
        let mut released = 0;
        for t in self.textures.iter_mut().flatten() {
            if !t.used && t.pack() {
                released += 1;
            }
        }
        // glibc keeps freed pages rather than returning them, so packing alone
        // does not lower the resident size the kernel is judging.
        #[cfg(target_env = "gnu")]
        if released > 0 {
            unsafe {
                libc::malloc_trim(0);
            }
        }
        if self.profiling {
            let us = t.elapsed().as_micros() as u64;
            self.profile.out_us += us;
            // Everything this backend did for this frame.
            let frame = self.profile.frame_us + us;
            self.profile.frame_us = 0;
            self.profile.tick(frame);
        }
    }

    fn output_frame(&mut self, window: (u32, u32)) {
        // A frame that drew nothing at all still needs its clear.
        self.discharge_clear(&[]);
        let Some(target) = fb::current() else {
            return;
        };
        let (x, y_gl, w, h) = blit_rect(window, self.shake);
        // `blit_rect` measures y from the bottom, as a GL viewport does.
        let y = window.1 as i32 - y_gl - h;
        if w <= 0 || h <= 0 {
            return;
        }

        let (cw, ch) = (self.canvas.w as i32, self.canvas.h as i32);
        let pw = target.w as i32;
        let ph = target.h as i32;
        let half = self.mount == Mount::Half;

        // Repaint the surround only when the image has moved. PAGES is the most
        // buffers a panel is likely to give us; clearing a few extra frames is
        // far cheaper than tracking which page is which.
        // The game's own rect when it goes straight out, since that is what
        // actually gets written; otherwise the canvas blit's.
        let rect = if self.direct_game {
            let (gx, gy, gw, gh) = screen_rect(self.game.power, self.game.base);
            (x + gx as i32, y + gy as i32, gw as i32, gh as i32)
        } else {
            (x, y, w, h)
        };
        if self.last_out != Some(rect) {
            self.last_out = Some(rect);
            self.clear_left = PAGES;
        }
        if self.clear_left > 0 {
            self.clear_left -= 1;
            // The bezel is the panel's background: a full-panel image of the
            // Game Boy's body with a hole where the picture goes, and the
            // canvas is copied over that hole below. Painted here rather than
            // per frame, which is the only reason it is affordable -- this
            // runs when the output rect moves and for PAGES frames after, not
            // every frame, and a full-panel blit is 421,120 pixels.
            //
            // Only when it is exactly panel-sized. `bezel_face` scales the art
            // to the panel before it gets here, so a mismatch means something
            // changed underneath us and black is the safe answer.
            let bezel = self
                .bezel
                .as_ref()
                .filter(|_| self.bezel_shown)
                .filter(|b| b.w == target.w && b.h == target.h);
            let bw = bezel.map_or(0, |b| b.w as usize * 4);
            for py in 0..ph {
                // SAFETY: one row inside the mapping.
                let row = unsafe {
                    std::slice::from_raw_parts_mut(
                        target.ptr.add(py as usize * target.stride),
                        target.stride,
                    )
                };
                let Some(b) = bezel else {
                    row.fill(0);
                    continue;
                };
                // Same mirroring the canvas copy does: this panel is mounted
                // upside down, so rows and columns both reverse. A Game Boy
                // body is not symmetric -- getting this wrong puts the
                // lettering under the screen and the power light above it.
                let sy = if half { b.h as i32 - 1 - py } else { py } as usize;
                let src = &b.px[sy * bw..sy * bw + bw];
                let n = bw.min(row.len());
                if half {
                    for (d, s) in row[..n].chunks_exact_mut(4).zip(src.chunks_exact(4).rev()) {
                        d.copy_from_slice(s);
                    }
                } else {
                    row[..n].copy_from_slice(&src[..n]);
                }
                if n < row.len() {
                    row[n..].fill(0);
                }
            }
        }

        // Blue-light gain as a lookup instead of a float multiply per channel
        // per pixel; at step 0 it is the identity and the copy is straight.
        let identity = self.gain == [1.0, 1.0, 1.0];
        let mut lut = [[0u8; 256]; 3];
        if !identity {
            for (c, table) in lut.iter_mut().enumerate() {
                // The canvas is in panel order, so the table for byte 0 is the
                // blue gain, not the red one.
                let gain = [self.gain[2], self.gain[1], self.gain[0]][c];
                for (v, out) in table.iter_mut().enumerate() {
                    *out = (v as f32 * gain).clamp(0.0, 255.0) as u8;
                }
            }
        }

        let x_start = x.max(0);
        let x_end = (x + w).min(pw);
        let y_start = y.max(0);
        let y_end = (y + h).min(ph);
        if x_start >= x_end || y_start >= y_end {
            return;
        }

        // The case that actually runs on this device: the canvas is exactly the
        // size of the rect, the panel wants the order the canvas already holds,
        // and there is no gain. That makes every row a memcpy.
        // With nothing but the game to draw, scale it onto the panel here
        // rather than via the canvas. Same conditions as the one-to-one copy
        // below, since it is that copy being folded into the game pass.
        if self.direct_game && w == cw && h == ch && target.bgr && identity {
            let t = std::time::Instant::now();
            let path = self.game.draw_direct(
                FbDest {
                    target,
                    x,
                    y,
                    w,
                    h,
                    half,
                },
                None,
            );
            if self.profiling {
                // game_us only, not frame_us: this runs inside end_frame, whose
                // own timer already counts it. Adding it here too double-counted
                // the game and inflated the worst-frame figure with it.
                self.profile.game_us += t.elapsed().as_micros() as u64;
                self.profile.path = path;
                self.profile.direct = true;
            }
            return;
        }

        // The case that actually runs on this device: the canvas is exactly the
        // size of the rect, the panel wants the order the canvas already holds,
        // and there is no gain. The upside-down panel still qualifies -- a 180
        // degree rotation is just the rows and the pixels within them reversed,
        // which is a tight copy rather than the general per-pixel path. Leaving
        // `half` out of this test was worth ~50 ms a frame of nothing, since
        // this device is always rotated.
        let one_to_one = w == cw && h == ch && target.bgr && identity;
        let span = (x_end - x_start) as usize;

        if one_to_one {
            for py in y_start..y_end {
                let si = ((py - y) as usize * cw as usize + (x_start - x) as usize) * 4;
                let Some(src) = self.canvas.px.get(si..si + span * 4) else {
                    continue;
                };
                let dy = if half { ph - 1 - py } else { py };
                let o = if half {
                    (pw - x_end) as usize * 4
                } else {
                    x_start as usize * 4
                };
                // SAFETY: one row inside the mapping.
                let row = unsafe {
                    std::slice::from_raw_parts_mut(
                        target.ptr.add(dy as usize * target.stride),
                        target.stride,
                    )
                };
                let Some(dst) = row.get_mut(o..o + span * 4) else {
                    continue;
                };
                if half {
                    // Reverse the row, four bytes at a time. Iterators here so
                    // the bounds checks fall away.
                    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4).rev()) {
                        d.copy_from_slice(s);
                    }
                } else {
                    dst.copy_from_slice(src);
                }
            }
            return;
        }

        // Source column per destination column, once.
        let mut col = vec![0usize; span];
        for (i, slot) in col.iter_mut().enumerate() {
            let px = x_start + i as i32;
            *slot = (((px - x) as i64 * cw as i64) / w as i64).clamp(0, cw as i64 - 1) as usize;
        }

        for py in y_start..y_end {
            let sy = (((py - y) as i64 * ch as i64) / h as i64).clamp(0, ch as i64 - 1) as usize;
            let src_row = sy * self.canvas.w as usize * 4;
            let dy = if half { ph - 1 - py } else { py };
            // SAFETY: one row inside the mapping, `stride` bytes wide.
            let row = unsafe {
                std::slice::from_raw_parts_mut(
                    target.ptr.add(dy as usize * target.stride),
                    target.stride,
                )
            };
            for (i, &sx) in col.iter().enumerate() {
                let px = x_start + i as i32;
                let dx = if half { pw - 1 - px } else { px } as usize;
                let o = dx * 4;
                if o + 4 > row.len() {
                    continue;
                }
                let si = src_row + sx * 4;
                let mut p = [
                    self.canvas.px[si],
                    self.canvas.px[si + 1],
                    self.canvas.px[si + 2],
                ];
                if !identity {
                    for (c, v) in p.iter_mut().enumerate() {
                        *v = lut[c][*v as usize];
                    }
                }
                if target.bgr {
                    row[o..o + 3].copy_from_slice(&p);
                } else {
                    row[o] = p[2];
                    row[o + 1] = p[1];
                    row[o + 2] = p[0];
                }
                row[o + 3] = 255;
            }
        }
        let _ = SCALE;
    }
}

#[cfg(test)]
mod packed_texture_tests {
    use super::{Image, Tex};

    #[test]
    fn hidden_texture_round_trips_without_losing_colour_or_alpha() {
        let rgba: Vec<u8> = (0..4096).map(|i| (i % 251) as u8).collect();
        let image = Image::from_rgba(32, 32, &rgba);
        let expected = image.px.clone();
        let mut tex = Tex::new(image);
        assert!(tex.bytes() < expected.len());
        assert!(tex.img.px.is_empty());
        for _ in 0..3 {
            tex.unpack();
            assert!(tex.used);
            assert_eq!(tex.img.px, expected);
            assert_eq!((tex.img.w, tex.img.h), (32, 32));
            assert!(tex.pack());
            assert!(!tex.pack(), "already packed texture was compressed twice");
        }
    }

    #[test]
    fn tiny_incompressible_texture_stays_raw() {
        let mut tex = Tex::new(Image::from_rgba(1, 1, &[1, 2, 3, 4]));
        assert!(tex.packed.is_none());
        assert_eq!(tex.bytes(), 4);
        assert!(!tex.pack());
        tex.unpack();
        assert_eq!(tex.img.px, [3, 2, 1, 4]);
    }
}

#[cfg(test)]
mod lcd3x_split_tests {
    use super::{BRIGHTEN_LCD, BRIGHTEN_SCANLINES, LCD_OFFSETS, PI};
    use crate::lcd3x::lcd3x_factors;

    /// The hot loop multiplies a per-row y factor by a per-column x factor
    /// instead of calling lcd3x_factors per pixel. That is only legitimate if
    /// the product is the same function, so pin it.
    #[test]
    fn the_separated_mask_equals_lcd3x_factors() {
        for yi in 0..17 {
            for xi in 0..17 {
                let (x, y) = (xi as f32 / 17.0, yi as f32 / 17.0);
                let yk = (BRIGHTEN_SCANLINES + (2.0 * PI * y).sin()) / (BRIGHTEN_SCANLINES + 1.0);
                let want = lcd3x_factors(x, y);
                for c in 0..3 {
                    let xk = (BRIGHTEN_LCD + (2.0 * PI * x + LCD_OFFSETS[c]).sin())
                        / (BRIGHTEN_LCD + 1.0);
                    let got = yk * xk;
                    assert!(
                        (got - want[c]).abs() < 1e-6,
                        "channel {c} at ({x}, {y}): {got} vs {}",
                        want[c]
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod whole_source_tests {
    use super::{BRIGHTEN_LCD, BRIGHTEN_SCANLINES, Game, Image, LCD_OFFSETS, PI};
    use crate::pipeline::{SCALE, SRC_H, SRC_W};
    use crate::surface::{OUT_H, OUT_W};

    /// A frame with every channel varying, so a swapped or truncated channel
    /// shows up rather than cancelling out.
    fn frame() -> Vec<u8> {
        let mut v = Vec::with_capacity((SRC_W * SRC_H * 4) as usize);
        for y in 0..SRC_H {
            for x in 0..SRC_W {
                // XRGB8888 is B, G, R, X in memory.
                v.push((x % 256) as u8);
                v.push((y % 256) as u8);
                v.push(((x + y) % 256) as u8);
                v.push(0);
            }
        }
        v
    }

    /// What the separable path computes, in floats, for one output pixel.
    fn expected(px: u32, py: u32, src: &[u8], c: u32) -> u8 {
        let (ox, oy) = (px % SCALE, py % SCALE);
        let (sx, sy) = (px / SCALE, py / SCALE);
        let (fx, fy) = ((ox as f32 + 0.5) / 3.0, (oy as f32 + 0.5) / 3.0);
        let yk = (BRIGHTEN_SCANLINES + (2.0 * PI * fy).sin()) / (BRIGHTEN_SCANLINES + 1.0);
        // Channel c of the stored (panel-order) pixel is colour 2 - c.
        let xk = (BRIGHTEN_LCD + (2.0 * PI * fx + LCD_OFFSETS[(2 - c) as usize]).sin())
            / (BRIGHTEN_LCD + 1.0);
        let v = src[((sy * SRC_W + sx) * 4 + c) as usize] as f32;
        (v * xk * yk).clamp(0.0, 255.0) as u8
    }

    #[test]
    fn a_whole_source_3x_frame_matches_the_mask_in_floats() {
        // The 3x lookup needs the picture to fit at an integer scale. This
        // build's canvas is 640x480 and the picture is 720x480, so it does not;
        // a 752x560 panel does, and the lookup still has to be right for it.
        let (w, h) = (752u32, 560u32);
        let mut game = Game::new();
        game.upload(&frame());
        game.base = super::PICTURE;
        let mut canvas = Image::new(w, h);
        let path = game.draw(&mut canvas, None);
        // The picture is centred in the canvas, so walk it in picture
        // coordinates and offset into the canvas.
        let (ox, oy, _, _) = super::screen_rect_in((w, h), 1.0, super::PICTURE);
        let (ox, oy) = (ox as u32, oy as u32);
        let mut worst = 0i32;
        for py in (0..super::GAME_H).step_by(37) {
            for px in (0..super::GAME_W).step_by(41) {
                for c in 0..3u32 {
                    let at = (((py + oy) * w + px + ox) * 4 + c) as usize;
                    let got = canvas.px[at] as i32;
                    let want = expected(px, py, &game.frame.px, c) as i32;
                    worst = worst.max((got - want).abs());
                }
            }
        }
        // Fixed point truncates where the float reference rounds, so allow a
        // single level, but no more.
        assert!(worst <= 1, "3x output differs from the mask by {worst}");
    }

    #[test]
    fn it_actually_wrote_something() {
        let mut game = Game::new();
        game.upload(&frame());
        let mut canvas = Image::new(OUT_W, OUT_H);
        game.draw(&mut canvas, None);
        assert!(canvas.px.iter().any(|&b| b != 0), "canvas left blank");
    }
}

#[cfg(test)]
mod separable_path_tests {
    use super::{BRIGHTEN_LCD, BRIGHTEN_SCANLINES, Game, Image, LCD_OFFSETS, PI};
    use super::{GAME_H, GAME_W, picture_base, screen_rect};
    use crate::pipeline::{SRC_H, SRC_W};
    use crate::surface::{OUT_H, OUT_W};

    /// A GB/GBC frame: 160x144 of picture sitting inside the 240x160 buffer, so
    /// the source and picture rects are both partial and the scale is 4.5x
    /// across and 3.33x down. This is what the device actually runs, and it is
    /// the path the 3x lookup does *not* cover.
    fn gb_game() -> Game {
        let mut g = Game::new();
        let mut frame = Vec::with_capacity((SRC_W * SRC_H * 4) as usize);
        for y in 0..SRC_H {
            for x in 0..SRC_W {
                frame.push((x % 251) as u8);
                frame.push((y % 241) as u8);
                frame.push(((x * 3 + y) % 239) as u8);
                frame.push(0);
            }
        }
        g.upload(&frame);
        let (pw, ph) = (160.0 / SRC_W as f32, 144.0 / SRC_H as f32);
        g.src = [0.0, 0.0, pw, ph];
        g.pic = [0.0, 0.0, pw, ph];
        g
    }

    #[test]
    fn the_separable_path_matches_the_mask_in_floats() {
        let mut g = gb_game();
        let mut canvas = Image::new(OUT_W, OUT_H);
        let path = g.draw(&mut canvas, None);
        assert_eq!(path, 2, "this shape must not take the 3x lookup");

        // Walk the canvas the way the renderer does: absolute columns and
        // rows, each mapped back into the source by the same expression.
        let (rx, ry, rw, rh) = super::screen_rect(1.0, super::picture_base());
        let (x0, y0) = (rx.round() as i32, ry.round() as i32);
        let x1 = ((rx + rw).round() as i32).min(OUT_W as i32);
        let y1 = ((ry + rh).round() as i32).min(OUT_H as i32);
        let (sw, sh) = (SRC_W as f32, SRC_H as f32);
        let mut worst = 0i32;
        for py in (y0..y1).step_by(29) {
            let v = g.src[1] + (py as f32 + 0.5 - ry) / rh * g.src[3];
            let sy = ((v * sh) as i32).clamp(0, SRC_H as i32 - 1) as usize;
            let row_inside = v >= g.pic[1] && v < g.pic[3];
            let yk = if row_inside {
                (BRIGHTEN_SCANLINES + (2.0 * PI * (v * sh).fract()).sin())
                    / (BRIGHTEN_SCANLINES + 1.0)
            } else {
                1.0
            };
            for px in (x0..x1).step_by(31) {
                let u = g.src[0] + (px as f32 + 0.5 - rx) / rw * g.src[2];
                let sx = ((u * sw) as i32).clamp(0, SRC_W as i32 - 1) as usize;
                let inside = u >= g.pic[0] && u < g.pic[2];
                for c in 0..3usize {
                    let xk = if inside {
                        (BRIGHTEN_LCD + (2.0 * PI * (u * sw).fract() + LCD_OFFSETS[2 - c]).sin())
                            / (BRIGHTEN_LCD + 1.0)
                    } else {
                        1.0
                    };
                    let src = g.frame.px[(sy * SRC_W as usize + sx) * 4 + c] as f32;
                    let want = (src * xk * yk).clamp(0.0, 255.0) as i32;
                    let got =
                        canvas.px[(((py as u32) * OUT_W + px as u32) * 4) as usize + c] as i32;
                    worst = worst.max((got - want).abs());
                }
            }
        }
        assert!(
            worst <= 2,
            "separable path differs from the mask by {worst}"
        );
    }

    /// The hot loop reads the source through a raw pointer on the strength of
    /// the column offsets being clamped. Drive every row and column so an
    /// out-of-bounds offset would fault here rather than on the handheld.
    #[test]
    fn it_stays_inside_the_source_for_every_pixel() {
        let mut g = gb_game();
        let mut canvas = Image::new(OUT_W, OUT_H);
        g.draw(&mut canvas, None);
        assert!(canvas.px.iter().any(|&b| b != 0));
    }
}

#[cfg(test)]
mod neon_kernel_tests {
    use super::{scale_row, scale_row_scalar};

    /// Values that exercise the saturation, the rounding and every lane
    /// position, including a length that is not a multiple of eight so the
    /// scalar tail is covered.
    fn line() -> Vec<u16> {
        (0..723u32).map(|i| ((i * 2731) % 65536) as u16).collect()
    }

    #[test]
    fn it_matches_the_scalar_reference() {
        let line = line();
        // 256 is unity; above it saturates, below it dims. 410 is the brightest
        // the screen-power overshoot reaches.
        for yk in [0u16, 1, 64, 255, 256, 257, 300, 410, 1024, 65535] {
            let mut a = vec![0u8; line.len()];
            let mut b = vec![0u8; line.len()];
            unsafe { scale_row(&mut a, &line, yk) };
            scale_row_scalar(&mut b, &line, yk);
            assert_eq!(a, b, "neon and scalar differ at yk = {yk}");
        }
    }

    #[test]
    fn it_saturates_rather_than_wrapping() {
        let line = vec![u16::MAX; 16];
        let mut out = vec![0u8; 16];
        unsafe { scale_row(&mut out, &line, 65535) };
        assert!(
            out.iter().all(|&v| v == 255),
            "expected saturation, got {out:?}"
        );
    }

    #[test]
    fn it_writes_nothing_past_the_end() {
        let line = line();
        let mut out = vec![0u8; line.len() + 8];
        let n = line.len();
        unsafe { scale_row(&mut out[..n], &line, 300) };
        assert!(out[n..].iter().all(|&v| v == 0), "wrote past the slice");
    }
}

#[cfg(test)]
mod direct_path_tests {
    use super::{Dest, FbDest, Game, Image, fb};
    use crate::pipeline::{SRC_H, SRC_W};
    use crate::surface::{OUT_H, OUT_W};

    fn game() -> Game {
        let mut g = Game::new();
        let mut frame = Vec::with_capacity((SRC_W * SRC_H * 4) as usize);
        for y in 0..SRC_H {
            for x in 0..SRC_W {
                frame.push((x % 251) as u8);
                frame.push((y % 241) as u8);
                frame.push(((x * 3 + y) % 239) as u8);
                frame.push(0);
            }
        }
        g.upload(&frame);
        g
    }

    /// Render the panel the long way -- game into the canvas, canvas copied out
    /// -- so the direct path has something to be equal to.
    fn via_canvas(half: bool, pw: u32, ph: u32, x: i32, y: i32) -> Vec<u8> {
        let mut canvas = Image::new(OUT_W, OUT_H);
        game().draw(&mut canvas, None);
        let mut panel = vec![0u8; pw as usize * ph as usize * 4];
        for cy in 0..OUT_H as i32 {
            for cx in 0..OUT_W as i32 {
                let (px, py) = if half {
                    (pw as i32 - 1 - (x + cx), ph as i32 - 1 - (y + cy))
                } else {
                    (x + cx, y + cy)
                };
                let si = ((cy as usize * OUT_W as usize) + cx as usize) * 4;
                let di = ((py as usize * pw as usize) + px as usize) * 4;
                panel[di..di + 4].copy_from_slice(&canvas.px[si..si + 4]);
            }
        }
        panel
    }

    fn via_direct(half: bool, pw: u32, ph: u32, x: i32, y: i32) -> Vec<u8> {
        let mut panel = vec![0u8; pw as usize * ph as usize * 4];
        let target = fb::Target {
            ptr: panel.as_mut_ptr(),
            w: pw,
            h: ph,
            stride: pw as usize * 4,
            bgr: true,
        };
        game().draw_direct(
            FbDest {
                target,
                x,
                y,
                w: OUT_W as i32,
                h: OUT_H as i32,
                half,
            },
            None,
        );
        panel
    }

    /// The canvas is the panel now, so it lands at the origin; the picture is
    /// centred *within* it by screen_rect rather than by this offset.
    const PW: u32 = OUT_W;
    const PH: u32 = OUT_H;
    const X: i32 = 0;
    const Y: i32 = 0;

    /// Compare two panels and say *where* they differ, rather than printing
    /// 1.6 MB of bytes at whoever runs the tests.
    fn diff(a: &[u8], b: &[u8]) {
        assert_eq!(a.len(), b.len(), "panel sizes differ");
        let mut n = 0;
        let mut first = None;
        for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
            if x != y {
                n += 1;
                first.get_or_insert((i / 4 % PW as usize, i / 4 / PW as usize, *x, *y));
            }
        }
        if let Some((x, y, got, want)) = first {
            panic!("{n} bytes differ; first at ({x}, {y}): {got} vs {want}");
        }
    }

    #[test]
    fn direct_matches_the_canvas_path_upright() {
        diff(
            &via_direct(false, PW, PH, X, Y),
            &via_canvas(false, PW, PH, X, Y),
        );
    }

    #[test]
    fn direct_matches_the_canvas_path_upside_down() {
        // The one that matters here: this panel is always rotated, and the
        // mirroring happens in the column table rather than in a second pass.
        diff(
            &via_direct(true, PW, PH, X, Y),
            &via_canvas(true, PW, PH, X, Y),
        );
    }

    #[test]
    fn it_leaves_the_surround_alone() {
        let panel = via_direct(true, PW, PH, X, Y);
        // The border is the compositor's to clear, not the game pass's; the
        // game must not have written outside its rect.
        let lit = |px: u32, py: u32| {
            let i = ((py as usize * PW as usize) + px as usize) * 4;
            panel[i..i + 3].iter().any(|&b| b != 0)
        };
        assert!(!lit(0, 0), "wrote into the top-left border");
        assert!(!lit(PW - 1, PH - 1), "wrote into the bottom-right border");
        assert!(!lit(PW / 2, 5), "wrote above the image");
    }

    #[test]
    fn a_canvas_destination_still_works() {
        let mut canvas = Image::new(OUT_W, OUT_H);
        let path = game().draw(&mut canvas, None);
        assert_eq!(path, 2, "the separable path is the only one now");
        assert!(canvas.px.iter().any(|&b| b != 0));
        let _ = Dest::Canvas(&mut canvas);
    }
}
