//! CPU rasteriser for slot's five-variant display list.
//!
//! Everything is RGBA8, top-down, premultiplied by nothing: the blend is plain
//! src-over with the source alpha, matching `SPRITE_FRAG` times
//! `glBlendFunc(SRC_ALPHA, ONE_MINUS_SRC_ALPHA)`.
//!
//! Coordinates match the GL path exactly. `RECT_VERT`/`SPRITE_VERT` map
//! `u_rect = (x, y, w, h)` through `gl_Position.y = 1 - 2y`, so a `Draw` rect is
//! in **top-left-origin pixels of the canvas**, which is what this uses
//! natively. The GL canvas texture is stored bottom-up as a consequence of
//! being an FBO; ours is top-down, so `read_frame` is a straight copy rather
//! than a flip.

/// A top-down 8888 image, stored in **the panel's byte order** (B, G, R, A on
/// every fbdev panel we have seen, this one included), not RGBA.
///
/// Keeping the canvas in the order the framebuffer wants makes the output stage
/// a row memcpy instead of a per-pixel swizzle over the whole frame, which is
/// the single biggest saving in the backend. Textures are converted once when
/// they are created rather than on every blit, and `Compositor::read_frame`
/// converts back, since callers expect RGBA as the GL path returns.
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Image {
    pub fn new(w: u32, h: u32) -> Self {
        Image {
            w,
            h,
            px: vec![0; w as usize * h as usize * 4],
        }
    }

    /// Takes RGBA, as slot hands it over, and stores panel order.
    pub fn from_rgba(w: u32, h: u32, rgba: &[u8]) -> Self {
        let want = w as usize * h as usize * 4;
        let mut px = rgba.to_vec();
        px.resize(want, 0);
        for p in px.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        Image { w, h, px }
    }

    pub fn fill(&mut self, rgba: [u8; 4]) {
        // A uniform colour is a memset, which is worth detecting: the canvas is
        // 1.4 MB and the loop below is 345,600 four-byte copies on this device,
        // every frame.
        if rgba.iter().all(|&b| b == rgba[0]) {
            self.px.fill(rgba[0]);
            return;
        }
        for p in self.px.chunks_exact_mut(4) {
            p.copy_from_slice(&rgba);
        }
    }

    #[inline]
    fn idx(&self, x: u32, y: u32) -> usize {
        (y as usize * self.w as usize + x as usize) * 4
    }

    /// Nearest-neighbour fetch, clamped like `CLAMP_TO_EDGE`.
    #[inline]
    pub fn sample(&self, u: f32, v: f32) -> [u8; 4] {
        if self.w == 0 || self.h == 0 {
            return [0; 4];
        }
        let x = ((u * self.w as f32) as i32).clamp(0, self.w as i32 - 1) as u32;
        let y = ((v * self.h as f32) as i32).clamp(0, self.h as i32 - 1) as u32;
        let i = self.idx(x, y);
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }
}

/// Reorder an RGBA colour to the stored byte order, once per draw call rather
/// than once per pixel.
pub fn to_storage(colour: [f32; 4]) -> [f32; 4] {
    [colour[2], colour[1], colour[0], colour[3]]
}

/// Alpha at or above this counts as opaque, so the blit copies the texel
/// instead of reading the destination back to blend with it.
///
/// Measured on the handheld: two cart faces of identical geometry and an
/// identical transparency *pattern* -- same count of fully transparent pixels
/// -- where one had 98,810 texels at 255 and the other had none, every covered
/// pixel being a hair under. The first drew in 2.5 ms and the second in 16.0,
/// one draw of a 16.77 ms frame, because `blend` is a read-modify-write per
/// pixel and `copy_from_slice` is a row of memcpy.
///
/// A face's alpha is multiplied by its silhouette's coverage
/// (`cart_face`'s `clip_to_silhouette`), and that silhouette is rasterised
/// from an SVG at run time: on the host and under qemu its interior comes out
/// 255, so whatever rounds it down happens on the device's own VFP. Rather
/// than chase a rasteriser across architectures, treat the last step of alpha
/// as the nothing it is: 254/255 is a 0.4% error in a blend nobody can see,
/// against 6x the cost of every pixel in the biggest draws on the shelf.
const OPAQUE_ENOUGH: u8 = 254;

/// Divide by 255, rounding, without dividing.
///
/// The blend did this three times per pixel and a divide is the most expensive
/// thing in it; the sprite pass costs 12 ms a frame on a busy menu. Exact for
/// every value a blend can produce, which the tests check exhaustively rather
/// than by sampling.
#[inline(always)]
fn div255(x: u32) -> u32 {
    let x = x + 128;
    (x + (x >> 8)) >> 8
}

/// src-over, both sides straight (non-premultiplied) alpha.
#[inline(always)]
fn blend(dst: &mut [u8], src: [u8; 4]) {
    let a = src[3] as u32;
    if a == 0 {
        return;
    }
    if a == 255 {
        dst[..4].copy_from_slice(&src);
        return;
    }
    let ia = 255 - a;
    for i in 0..3 {
        dst[i] = div255(src[i] as u32 * a + dst[i] as u32 * ia) as u8;
    }
    dst[3] = (a + div255(dst[3] as u32 * ia)).min(255) as u8;
}

#[inline]
/// Whether a tint only scales alpha, which is every tint slot actually uses:
/// `Draw::Tex` and `Draw::Turned` pass `[1, 1, 1, alpha]`, so three of
/// `modulate`'s four float multiplies are by exactly 1.0.
fn white(colour: [f32; 4]) -> bool {
    colour[0] == 1.0 && colour[1] == 1.0 && colour[2] == 1.0
}

/// Whether a tint can be ignored entirely, so the texel goes down untouched.
///
/// Not exact equality with white-and-opaque, which is what this was: the
/// shelf's lift eases *towards* 1.0 and arrives at 0.9996, so a cart sitting
/// still at what looks like full opacity missed the fast path and blended
/// every one of its 99,520 pixels -- 10.0 ms in one draw, measured. The
/// tolerance is `OPAQUE_ENOUGH`, the same one level of alpha the texel test
/// allows, for the same reason: it cannot be seen and it costs 3x.
fn plain_enough(colour: [f32; 4]) -> bool {
    white(colour) && colour[3] * 255.0 >= OPAQUE_ENOUGH as f32
}

/// `modulate`'s alpha, for every possible texel alpha, built once per draw.
///
/// This is the shelf's whole cost. Scrolling fades each cart face -- measured
/// alphas of 140, 159 and 236 mid-animation -- and any alpha below 1.0 drops
/// the draw off the `plain` path onto `modulate` per pixel: four float
/// multiply-clamp-casts, 90,000 pixels, three faces a frame, 30-36 ms of a
/// 16.77 ms period. A tint that only scales alpha needs one byte of table
/// instead, and `v as f32 * 1.0` is exactly `v` for every u8, so the colour
/// channels can be carried through untouched.
fn alpha_table(colour: [f32; 4]) -> [u8; 256] {
    let mut t = [0u8; 256];
    for (i, v) in t.iter_mut().enumerate() {
        *v = (i as f32 * colour[3]).clamp(0.0, 255.0) as u8;
    }
    t
}

fn modulate(c: [u8; 4], colour: [f32; 4]) -> [u8; 4] {
    let f = |v: u8, m: f32| ((v as f32 * m).clamp(0.0, 255.0)) as u8;
    [
        f(c[0], colour[0]),
        f(c[1], colour[1]),
        f(c[2], colour[2]),
        f(c[3], colour[3]),
    ]
}

/// The clipped integer span of a float rect, as half-open pixel bounds.
fn clip(dst: &Image, x: f32, y: f32, w: f32, h: f32) -> Option<(i32, i32, i32, i32)> {
    if !(w > 0.0 && h > 0.0) {
        return None;
    }
    let x0 = x.round() as i32;
    let y0 = y.round() as i32;
    let x1 = (x + w).round() as i32;
    let y1 = (y + h).round() as i32;
    let cx0 = x0.max(0);
    let cy0 = y0.max(0);
    let cx1 = x1.min(dst.w as i32);
    let cy1 = y1.min(dst.h as i32);
    if cx0 >= cx1 || cy0 >= cy1 {
        return None;
    }
    Some((cx0, cy0, cx1, cy1))
}

/// A solid rect. `Draw::Rect` in GL is the 1x1 white texture times `u_colour`,
/// so this is the same thing without the multiply.
///
/// This is where the shelf's time actually goes. It reports ~193 draws a frame
/// and only seven of them are textured, so the other ~186 are rects -- and this
/// was computing `idx` and taking a bounds-checked four-byte slice per pixel,
/// with no shortcut for the two cases a UI is mostly made of. Now: a
/// transparent rect returns, an opaque one builds one row and memcpy's it, and
/// the general case walks row slices so the per-pixel bounds checks fall away.
pub fn fill_rect(
    dst: &mut Image,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    colour: [f32; 4],
    scratch: &mut Scratch,
) {
    let Some((x0, y0, x1, y1)) = clip(dst, x, y, w, h) else {
        return;
    };
    // `colour` arrives already in storage order from the caller.
    let src = modulate([255, 255, 255, 255], colour);
    if src[3] == 0 {
        return;
    }
    let dw = dst.w as usize;
    let width = (x1 - x0) as usize;

    if src[3] == 255 {
        scratch.opaque_px += width as u64 * (y1 - y0) as u64;
        // One row of the colour, then a memcpy per scanline.
        let want = width * 4;
        if scratch.line.len() != want {
            scratch.line.resize(want, 0);
        }
        for p in scratch.line.chunks_exact_mut(4) {
            p.copy_from_slice(&src);
        }
        for py in y0..y1 {
            let start = (py as usize * dw + x0 as usize) * 4;
            if let Some(drow) = dst.px.get_mut(start..start + want) {
                drow.copy_from_slice(&scratch.line);
            }
        }
        return;
    }

    scratch.blend_rects += 1;
    scratch.blend_px += width as u64 * (y1 - y0) as u64;
    for py in y0..y1 {
        let start = (py as usize * dw + x0 as usize) * 4;
        let Some(drow) = dst.px.get_mut(start..start + width * 4) else {
            continue;
        };
        for d in drow.chunks_exact_mut(4) {
            blend(d, src);
        }
    }
}

/// Reusable per-call scratch for `blit`, owned by the compositor.
///
/// The column table used to be a `vec!` inside `blit`, which is an allocation
/// and a float divide per column on every one of the shelf's ~190 draws a
/// frame. `line` is what makes the row reuse below possible.
#[derive(Default)]
pub struct Scratch {
    col: Vec<u32>,
    line: Vec<u8>,
    /// Draws that took the row-reuse path, and draws that did not, since the
    /// last read. The reuse only pays where a sprite is scaled *up*, and the
    /// shelf's gain from it measured as nothing -- which it would, if the shelf
    /// scales its box art down. Worth knowing before optimising it again.
    pub reused: u32,
    pub direct: u32,
    /// Rects that went down the per-pixel blend path rather than the opaque
    /// row memcpy, and the pixels each path wrote, since the last read.
    ///
    /// 186 of the shelf's 193 draws a frame are rects and they cost 11 ms of a
    /// 16.77 ms period, so which path they take and how much they overdraw is
    /// the whole question. A count of rects cannot answer it: one full-canvas
    /// blend is 421k pixels of read-modify-write and a small opaque one is a
    /// few hundred bytes of memcpy.
    pub blend_rects: u32,
    pub opaque_px: u64,
    pub blend_px: u64,
    /// Destination pixels in the last textured draw by class: skipped because
    /// the texel was fully transparent, copied because it was fully opaque,
    /// blended because it was partial.
    ///
    /// Two draws of identical geometry measured 0.46 ms and 3.3 ms, so the
    /// cost is not the pixel count or the scale. Either those pixels are
    /// mostly partial-alpha, which this counts, or the difference is the
    /// 305 KB texture missing cache, which this rules in by elimination.
    pub tex_skip: u32,
    pub tex_copy: u32,
    pub tex_blend: u32,
}

/// An axis-aligned textured rect, nearest-sampled.
///
/// The shelf is several of these a frame, each covering a large part of the
/// canvas, so the per-pixel work matters as much as the game plane's. Three
/// things carry it:
///
/// - the source byte offset per destination column is computed once into
///   `scratch.col`, so the inner loop has no float divide and no clamp;
/// - where the sprite is scaled up, each *source* row is expanded once into
///   `scratch.line` at destination width, with `colour` already folded in, and
///   every destination row sharing that source row is then a sequential pass
///   over it rather than a fresh gather. This is the same shape that took the
///   game pass from 17.1 ms to 7.8, and for the same reason: the gather is what
///   costs, not the arithmetic;
/// - a line that came out fully opaque -- the wallpaper, and the solid middle
///   of a cart -- makes each destination row a straight `copy_from_slice`.
///
/// Scaled *down*, no two destination rows share a source row, so the line would
/// be pure overhead and the direct path is kept.
#[allow(clippy::too_many_arguments)]
pub fn blit(
    dst: &mut Image,
    tex: &Image,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    colour: [f32; 4],
    scratch: &mut Scratch,
) {
    blit_part(dst, tex, x, y, w, h, colour, WHOLE, scratch);
}

/// The whole texture, as `Draw::Tex` samples it.
pub const WHOLE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// `blit`, sampling only the `uv` window of the texture -- `Draw::Part`, which
/// the shelf uses to show the middle of a texture wider than the canvas.
#[allow(clippy::too_many_arguments)]
pub fn blit_part(
    dst: &mut Image,
    tex: &Image,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    colour: [f32; 4],
    uv: [f32; 4],
    scratch: &mut Scratch,
) {
    let Some((x0, y0, x1, y1)) = clip(dst, x, y, w, h) else {
        return;
    };
    if tex.w == 0 || tex.h == 0 {
        return;
    }
    let plain = plain_enough(colour);
    let width = (x1 - x0) as usize;

    // Source byte offset per destination column.
    scratch.col.clear();
    scratch.col.reserve(width);
    for i in 0..width {
        let px = x0 + i as i32;
        let u = uv[0] + (px as f32 + 0.5 - x) / w * uv[2];
        let sx = ((u * tex.w as f32) as i32).clamp(0, tex.w as i32 - 1) as u32;
        scratch.col.push(sx * 4);
    }
    let col = &scratch.col;

    let dw = dst.w as usize;
    let tw = tex.w as usize;
    let row_bytes = tw * 4;

    // Worth the line only when destination rows share source rows.
    let reuse = h >= tex.h as f32 * uv[3];
    if reuse && scratch.line.len() != width * 4 {
        scratch.line.resize(width * 4, 0);
    }
    if reuse {
        scratch.reused += 1;
    } else {
        scratch.direct += 1;
    }
    let mut built: Option<usize> = None;
    let mut opaque = false;
    // One table per draw rather than four float multiplies per pixel.
    let faded = (!plain && white(colour)).then(|| alpha_table(colour));
    // Near-opaque counts as opaque only for an untinted draw. With a colour
    // the row has been modulated, and the gather path still blends such
    // pixels, so taking the shortcut here as well would make one path
    // disagree with the other -- which is what
    // `blit_matches_the_reference_at_every_scale` caught.
    let enough = if plain { OPAQUE_ENOUGH } else { 255 };

    for py in y0..y1 {
        let v = uv[1] + (py as f32 + 0.5 - y) / h * uv[3];
        let sy = ((v * tex.h as f32) as i32).clamp(0, tex.h as i32 - 1) as usize;
        let src_row = sy * row_bytes;
        let dstart = (py as usize * dw + x0 as usize) * 4;

        // Row slices and fixed-size reads, so the bounds checks fall away: this
        // is four of them per pixel otherwise, and the sprite pass is the whole
        // cost of a busy menu.
        let Some(trow) = tex.px.get(src_row..src_row + row_bytes) else {
            continue;
        };

        if !reuse {
            let Some(drow) = dst.px.get_mut(dstart..dstart + width * 4) else {
                continue;
            };
            for (d, &c) in drow.chunks_exact_mut(4).zip(col.iter()) {
                // SAFETY: `c` is a texel index clamped to tex.w - 1 when the
                // column table was built, times four, and `trow` is one whole
                // source row.
                let s = unsafe { &*(trow.as_ptr().add(c as usize) as *const [u8; 4]) };
                if s[3] == 0 {
                    scratch.tex_skip += 1;
                    continue;
                }
                if plain {
                    if s[3] >= OPAQUE_ENOUGH {
                        scratch.tex_copy += 1;
                        d.copy_from_slice(s);
                    } else {
                        scratch.tex_blend += 1;
                        blend(d, *s);
                    }
                    continue;
                }
                scratch.tex_blend += 1;
                if let Some(t) = &faded {
                    blend(d, [s[0], s[1], s[2], t[s[3] as usize]]);
                    continue;
                }
                blend(d, modulate(*s, colour));
            }
            continue;
        }

        if built != Some(sy) {
            for (e, &c) in scratch.line.chunks_exact_mut(4).zip(col.iter()) {
                // SAFETY: as above.
                let s = unsafe { &*(trow.as_ptr().add(c as usize) as *const [u8; 4]) };
                let s = match (plain, &faded) {
                    (true, _) => *s,
                    (false, Some(t)) => [s[0], s[1], s[2], t[s[3] as usize]],
                    (false, None) => modulate(*s, colour),
                };
                e.copy_from_slice(&s);
            }
            // One scan per source row buys the memcpy path below for every
            // destination row that shares it.
            opaque = scratch.line.chunks_exact(4).all(|p| p[3] >= enough);
            built = Some(sy);
        }

        let Some(drow) = dst.px.get_mut(dstart..dstart + width * 4) else {
            continue;
        };
        if opaque {
            drow.copy_from_slice(&scratch.line);
            continue;
        }
        for (d, s) in drow.chunks_exact_mut(4).zip(scratch.line.chunks_exact(4)) {
            // Cart art is mostly rounded corners and padding; skipping the
            // blend here is most of the saving on the shelf.
            if s[3] == 0 {
                continue;
            }
            if s[3] >= enough {
                d.copy_from_slice(s);
                continue;
            }
            blend(d, [s[0], s[1], s[2], s[3]]);
        }
    }
}

/// A textured rect rotated about its own centre, as `Draw::Turned`.
///
/// `SPRITE_VERT` rotates the quad's corners by `(cos, sin)` around the rect
/// centre. Forward-mapping corners would leave seams, so this walks the
/// destination bounding box and inverse-rotates each pixel back into the rect.
#[allow(clippy::too_many_arguments)]
pub fn blit_turned(
    dst: &mut Image,
    tex: &Image,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    colour: [f32; 4],
    turn: f32,
    scratch: &mut Scratch,
) {
    if turn == 0.0 {
        blit(dst, tex, x, y, w, h, colour, scratch);
        return;
    }
    let (sin, cos) = turn.sin_cos();
    let (cx, cy) = (x + w * 0.5, y + h * 0.5);
    // Half-extent of the rotated rect: the bound the inverse map has to cover.
    let ex = (w * cos.abs() + h * sin.abs()) * 0.5;
    let ey = (w * sin.abs() + h * cos.abs()) * 0.5;
    let Some((x0, y0, x1, y1)) = clip(dst, cx - ex, cy - ey, ex * 2.0, ey * 2.0) else {
        return;
    };
    for py in y0..y1 {
        let dy = py as f32 + 0.5 - cy;
        for px in x0..x1 {
            let dx = px as f32 + 0.5 - cx;
            // Inverse of the forward rotation, so transpose of the matrix.
            let lx = cos * dx + sin * dy;
            let ly = -sin * dx + cos * dy;
            let u = lx / w + 0.5;
            let v = ly / h + 0.5;
            if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                continue;
            }
            let s = modulate(tex.sample(u, v), colour);
            let i = dst.idx(px as u32, py as u32);
            blend(&mut dst.px[i..i + 4], s);
        }
    }
}

#[cfg(test)]
mod blit_tests {
    use super::{Image, OPAQUE_ENOUGH, Scratch, blend, modulate, white};

    /// `blit` as it was before the row line and the opaque memcpy: one gather
    /// and one blend per destination pixel, no reuse, no special cases beyond
    /// the fully transparent and near-opaque texel. The optimised version has
    /// to agree with this exactly -- `task shot` cannot settle it, because the
    /// frame it captures varies between two runs of identical code by more
    /// pixels than this change moves.
    fn reference(dst: &mut Image, tex: &Image, x: f32, y: f32, w: f32, h: f32, colour: [f32; 4]) {
        let Some((x0, y0, x1, y1)) = super::clip(dst, x, y, w, h) else {
            return;
        };
        if tex.w == 0 || tex.h == 0 {
            return;
        }
        let plain = white(colour) && colour[3] >= OPAQUE_ENOUGH as f32 / 255.0;
        for py in y0..y1 {
            let v = (py as f32 + 0.5 - y) / h;
            let sy = ((v * tex.h as f32) as i32).clamp(0, tex.h as i32 - 1) as u32;
            for px in x0..x1 {
                let u = (px as f32 + 0.5 - x) / w;
                let sx = ((u * tex.w as f32) as i32).clamp(0, tex.w as i32 - 1) as u32;
                let i = (sy as usize * tex.w as usize + sx as usize) * 4;
                let s = [tex.px[i], tex.px[i + 1], tex.px[i + 2], tex.px[i + 3]];
                if s[3] == 0 {
                    continue;
                }
                let d = dst.idx(px as u32, py as u32);
                if plain {
                    // The same near-opaque rule as the real blit: this is a
                    // second, per-pixel implementation of the *intended*
                    // semantics, not of exact src-over, so it has to agree
                    // about them. `near_opaque_costs_at_most_one_level` is
                    // what pins the rule itself.
                    if s[3] >= OPAQUE_ENOUGH {
                        dst.px[d..d + 4].copy_from_slice(&s);
                    } else {
                        blend(&mut dst.px[d..d + 4], s);
                    }
                } else if white(colour) {
                    // Independently of the table the real blit builds: one
                    // float multiply, here, per pixel.
                    let a = (s[3] as f32 * colour[3]).clamp(0.0, 255.0) as u8;
                    blend(&mut dst.px[d..d + 4], [s[0], s[1], s[2], a]);
                } else {
                    blend(&mut dst.px[d..d + 4], modulate(s, colour));
                }
            }
        }
    }

    /// A texture with every alpha case in it: transparent, fully opaque, and a
    /// spread in between, plus rows that are uniformly one of those so the
    /// opaque-memcpy and skip paths both get exercised.
    fn texture(w: u32, h: u32) -> Image {
        let mut img = Image::new(w, h);
        for ty in 0..h {
            for tx in 0..w {
                let i = (ty as usize * w as usize + tx as usize) * 4;
                let a = match ty % 4 {
                    0 => 255,
                    1 => 0,
                    2 => ((tx * 37 + ty * 11) % 256) as u8,
                    _ => {
                        if tx % 3 == 0 {
                            0
                        } else {
                            255
                        }
                    }
                };
                img.px[i] = (tx * 7 + ty * 3) as u8;
                img.px[i + 1] = (tx * 13 + ty * 5) as u8;
                img.px[i + 2] = (tx * 29 + ty * 17) as u8;
                img.px[i + 3] = a;
            }
        }
        img
    }

    fn canvas(w: u32, h: u32) -> Image {
        let mut img = Image::new(w, h);
        for (i, b) in img.px.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        img
    }

    /// `fill_rect` as it was: `idx` and a bounds-checked slice per pixel, one
    /// blend each, no transparent or opaque shortcut.
    fn fill_reference(dst: &mut Image, x: f32, y: f32, w: f32, h: f32, colour: [f32; 4]) {
        let Some((x0, y0, x1, y1)) = super::clip(dst, x, y, w, h) else {
            return;
        };
        let src = modulate([255, 255, 255, 255], colour);
        for py in y0..y1 {
            for px in x0..x1 {
                let i = dst.idx(px as u32, py as u32);
                blend(&mut dst.px[i..i + 4], src);
            }
        }
    }

    /// Rects are ~186 of the shelf's ~193 draws, so this is the hot one.
    #[test]
    fn fill_rect_matches_the_reference() {
        let rects = [
            (0.0f32, 0.0f32, 48.0f32, 36.0f32),
            (2.5, 1.5, 20.0, 10.0),
            (0.3, 0.7, 47.4, 35.2),
            (-5.0, -3.0, 30.0, 20.0),
            (40.0, 28.0, 30.0, 20.0),
            (10.0, 10.0, 1.0, 1.0),
            (10.0, 10.0, 0.0, 5.0),
            (5.0, 5.0, 13.5, 7.5),
        ];
        // Fully transparent, fully opaque, and a spread between, since each
        // takes a different path now.
        let alphas = [0.0, 0.004, 0.25, 0.5, 0.75, 0.996, 1.0];
        let mut scratch = Scratch::default();
        for (x, y, w, h) in rects {
            for a in alphas {
                for rgb in [[1.0, 1.0, 1.0], [0.0, 0.0, 0.0], [0.2, 0.6, 0.9]] {
                    let colour = [rgb[0], rgb[1], rgb[2], a];
                    let mut want = canvas(48, 36);
                    let mut got = canvas(48, 36);
                    fill_reference(&mut want, x, y, w, h, colour);
                    super::fill_rect(&mut got, x, y, w, h, colour, &mut scratch);
                    assert_eq!(got.px, want.px, "rect {:?} colour {colour:?}", (x, y, w, h));
                }
            }
        }
    }

    /// A rect and a textured draw share `scratch.line`, so the one must not
    /// leave the other reading its leftovers.
    #[test]
    fn rects_and_blits_share_a_scratch_safely() {
        let tex = texture(13, 7);
        let mut scratch = Scratch::default();
        let mut want = canvas(48, 36);
        let mut got = canvas(48, 36);
        for i in 0..6 {
            let a = if i % 2 == 0 { 1.0 } else { 0.5 };
            let colour = [1.0, 0.5, 0.25, a];
            let (x, y, w, h) = (i as f32, 2.0, 30.0 - i as f32, 20.0);
            fill_reference(&mut want, x, y, w, h, colour);
            super::fill_rect(&mut got, x, y, w, h, colour, &mut scratch);
            assert_eq!(got.px, want.px, "rect {i}");

            reference(&mut want, &tex, 1.0, 1.0, 39.0, 21.0, [1.0, 1.0, 1.0, 1.0]);
            super::blit(
                &mut got,
                &tex,
                1.0,
                1.0,
                39.0,
                21.0,
                [1.0, 1.0, 1.0, 1.0],
                &mut scratch,
            );
            assert_eq!(got.px, want.px, "blit after rect {i}");
        }
    }

    /// What `OPAQUE_ENOUGH` actually costs. Copying a texel instead of
    /// blending it is only sound because the result differs by at most one
    /// level per channel, which is below what the panel can show -- and it
    /// buys a row memcpy instead of a read-modify-write per pixel, measured
    /// at 16.0 ms against 2.5 for one cart face on the handheld.
    #[test]
    fn near_opaque_costs_at_most_one_level() {
        for a in OPAQUE_ENOUGH..=255 {
            for s in [[0u8, 0, 0, a], [255, 255, 255, a], [37, 128, 201, a]] {
                for d0 in [[0u8, 0, 0, 255], [255, 255, 255, 255], [91, 17, 240, 255]] {
                    let mut exact = d0;
                    blend(&mut exact, s);
                    for c in 0..3 {
                        let off = exact[c].abs_diff(s[c]);
                        assert!(
                            off <= 1,
                            "alpha {a}: copying {s:?} over {d0:?} is {off} off \
                             channel {c} ({} exact, {} copied)",
                            exact[c],
                            s[c]
                        );
                    }
                }
            }
        }
    }

    /// The same bound for a near-opaque *draw* alpha, which is the shelf's
    /// case: `plain_enough` drops the tint entirely, so the texel is copied
    /// where an exact blend would have scaled it by 254/255.
    #[test]
    fn a_near_opaque_tint_costs_at_most_one_level() {
        let colour = [1.0, 1.0, 1.0, OPAQUE_ENOUGH as f32 / 255.0];
        assert!(
            super::plain_enough(colour),
            "the tolerance should accept it"
        );
        for s in [[0u8, 0, 0, 255], [255, 255, 255, 255], [37, 128, 201, 255]] {
            for d0 in [[0u8, 0, 0, 255], [255, 255, 255, 255], [91, 17, 240, 255]] {
                let mut exact = d0;
                blend(&mut exact, modulate(s, colour));
                for c in 0..3 {
                    let off = exact[c].abs_diff(s[c]);
                    assert!(off <= 1, "channel {c} is {off} off with {s:?} over {d0:?}");
                }
            }
        }
        let faded = [1.0, 1.0, 1.0, (OPAQUE_ENOUGH - 1) as f32 / 255.0];
        assert!(!super::plain_enough(faded), "a real fade must still blend");
    }

    /// And that the rule does not reach any further: a texel one level below
    /// the threshold must still blend, or a genuinely translucent sprite would
    /// turn opaque.
    #[test]
    fn below_the_threshold_still_blends() {
        let a = OPAQUE_ENOUGH - 1;
        let s = [255u8, 255, 255, a];
        let mut got = [0u8, 0, 0, 255];
        blend(&mut got, s);
        assert_ne!(got[0], s[0], "alpha {a} was treated as opaque");
    }

    #[test]
    fn blit_matches_the_reference_at_every_scale() {
        let tex = texture(13, 7);
        // Upscales (where the row line pays), 1:1, downscales (where it is
        // skipped), fractional offsets, and clipping off each edge.
        let rects = [
            (0.0, 0.0, 13.0, 7.0),
            (0.0, 0.0, 39.0, 21.0),
            (2.5, 1.5, 40.0, 30.0),
            (1.0, 1.0, 60.0, 8.0),
            (0.0, 0.0, 6.0, 3.0),
            (3.0, 2.0, 7.5, 3.5),
            (-5.0, -3.0, 30.0, 20.0),
            (40.0, 28.0, 30.0, 20.0),
            (0.3, 0.7, 48.0, 36.0),
        ];
        let colours = [
            [1.0, 1.0, 1.0, 1.0],
            [1.0, 1.0, 1.0, 0.5],
            [0.5, 0.75, 0.25, 1.0],
            [0.5, 0.75, 0.25, 0.4],
        ];
        let mut scratch = Scratch::default();
        for (x, y, w, h) in rects {
            for colour in colours {
                let mut want = canvas(48, 36);
                let mut got = canvas(48, 36);
                reference(&mut want, &tex, x, y, w, h, colour);
                super::blit(&mut got, &tex, x, y, w, h, colour, &mut scratch);
                assert_eq!(got.px, want.px, "rect {:?} colour {colour:?}", (x, y, w, h));
            }
        }
    }

    /// The scratch is shared across calls and across frames, so a stale line or
    /// column table from the previous draw must not leak into the next one.
    /// Same draws, interleaved, through one scratch.
    #[test]
    fn a_shared_scratch_does_not_leak_between_draws() {
        let a = texture(13, 7);
        let b = texture(5, 11);
        let mut scratch = Scratch::default();
        let mut want = canvas(48, 36);
        let mut got = canvas(48, 36);
        let draws = [
            (&a, 0.0f32, 0.0f32, 39.0f32, 21.0f32),
            (&b, 4.0, 2.0, 40.0, 33.0),
            (&a, 1.0, 1.0, 13.0, 7.0),
            (&b, 0.0, 0.0, 8.0, 5.0),
            (&a, 2.0, 3.0, 44.0, 30.0),
        ];
        for (tex, x, y, w, h) in draws {
            reference(&mut want, tex, x, y, w, h, [1.0, 1.0, 1.0, 1.0]);
            super::blit(
                &mut got,
                tex,
                x,
                y,
                w,
                h,
                [1.0, 1.0, 1.0, 1.0],
                &mut scratch,
            );
            assert_eq!(got.px, want.px, "after {}x{} at {x},{y}", tex.w, tex.h);
        }
    }
}

#[cfg(test)]
mod blend_tests {
    use super::{blend, div255};

    /// The old blend divided by 255 after adding 127. Prove the replacement is
    /// the same function over every value a blend can produce -- src * a +
    /// dst * (255 - a) peaks at 255 * 255 -- rather than trusting the identity.
    #[test]
    fn div255_matches_the_division_everywhere() {
        for x in 0..=(255u32 * 255 + 255) {
            assert_eq!(div255(x), (x + 127) / 255, "div255({x})");
        }
    }

    /// And the blend itself, against the arithmetic it replaced, for every
    /// alpha and a spread of colours.
    #[test]
    fn blend_matches_the_old_arithmetic() {
        for a in 0..=255u32 {
            for s in (0..=255u32).step_by(17) {
                for d in (0..=255u32).step_by(17) {
                    let mut got = [d as u8; 4];
                    blend(&mut got, [s as u8, s as u8, s as u8, a as u8]);

                    let want = if a == 0 {
                        d as u8
                    } else if a == 255 {
                        s as u8
                    } else {
                        ((s * a + d * (255 - a) + 127) / 255) as u8
                    };
                    assert_eq!(got[0], want, "a={a} s={s} d={d}");
                }
            }
        }
    }
}
