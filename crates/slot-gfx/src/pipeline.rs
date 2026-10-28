use crate::lcd3x::ScreenEffect;
use crate::power::{screen_brightness, screen_rect_above};
use crate::quad::Quad;
use crate::shaders::{GAME_FRAG, RECT_VERT};
use crate::surface::{GfxError, OUT_H, OUT_W};

pub const SCALE: u32 = 3;
pub const SRC_W: u32 = OUT_W / SCALE;
pub const SRC_H: u32 = OUT_H / SCALE;

pub const WHOLE_TEXTURE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// The game texture at `w` x `h`. The Flip's driver needs RGBA (see `upload`),
/// the H700's takes BGRA.
fn game_texture(w: u32, h: u32) -> gl::types::GLuint {
    #[cfg(feature = "flip")]
    let format = gl::RGBA;
    #[cfg(not(feature = "flip"))]
    let format = gl::BGRA;
    crate::gl::texture(w, h, gl::NEAREST, gl::CLAMP_TO_EDGE, format, None)
}

pub struct GamePass {
    prog: gl::types::GLuint,
    game: gl::types::GLuint,
    /// What `game` currently holds. `SRC_W` x `SRC_H` until a core hands over a
    /// picture of another size.
    tex_w: u32,
    tex_h: u32,
    u_src: gl::types::GLint,
    u_rect: gl::types::GLint,
    u_bright: gl::types::GLint,
    u_uv: gl::types::GLint,
    u_mode: gl::types::GLint,
    u_paper_size: gl::types::GLint,
    u_fbo: gl::types::GLint,
    fbo_scale: [f32; 2],
    u_origin: gl::types::GLint,
    origin: [f32; 2],
    u_pic: gl::types::GLint,
    pic: [f32; 4],
    paper: gl::types::GLuint,
    paper_size: f32,
    effect: ScreenEffect,
    power: f32,
    fills: bool,
    /// Pixels kept for the carousel footer. The picture scales in the space above them.
    bottom: f32,
    src: [f32; 4],
    #[cfg(feature = "flip")]
    rgba: Vec<u8>,
}

impl GamePass {
    pub fn new() -> Result<Self, GfxError> {
        let prog = crate::shaders::program(RECT_VERT, GAME_FRAG)?;
        // The H700's Mali driver takes a BGRA texture and updates it in place
        // every frame. The Flip's GLES stack accepts the BGRA upload but does
        // not reflect later TexSubImage2D updates, so the picture looks frozen.
        // There we use a plain RGBA texture and swizzle on upload instead.
        let game = game_texture(SRC_W, SRC_H);
        let paper = crate::gl::texture(
            1,
            1,
            gl::NEAREST,
            gl::REPEAT,
            crate::gl::gray_format(),
            Some(&[255]),
        );
        let (u_rect, u_bright, u_uv, u_mode, u_paper_size, u_pic, u_fbo, u_origin, u_src);
        unsafe {
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_game"), 0);
            u_src = crate::gl::uniform_location(prog, "u_src");
            gl::Uniform2f(u_src, SRC_W as f32, SRC_H as f32);
            gl::Uniform2f(
                crate::gl::uniform_location(prog, "u_target"),
                OUT_W as f32,
                OUT_H as f32,
            );
            u_rect = crate::gl::uniform_location(prog, "u_rect");
            u_bright = crate::gl::uniform_location(prog, "u_bright");
            u_uv = crate::gl::uniform_location(prog, "u_uv");
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_paper"), 1);
            u_mode = crate::gl::uniform_location(prog, "u_mode");
            u_paper_size = crate::gl::uniform_location(prog, "u_paper_size");
            u_fbo = crate::gl::uniform_location(prog, "u_fbo");
            u_pic = crate::gl::uniform_location(prog, "u_pic");
            u_origin = crate::gl::uniform_location(prog, "u_origin");
        }
        Ok(GamePass {
            prog,
            game,
            tex_w: SRC_W,
            tex_h: SRC_H,
            u_src,
            u_rect,
            u_bright,
            u_uv,
            u_mode,
            u_paper_size,
            u_fbo,
            fbo_scale: [1.0, 1.0],
            u_origin,
            origin: [0.0, 0.0],
            u_pic,
            pic: WHOLE_TEXTURE,
            paper,
            paper_size: 1.0,
            effect: ScreenEffect::Lcd3x,
            power: 1.0,
            fills: false,
            bottom: 0.0,
            src: WHOLE_TEXTURE,
            #[cfg(feature = "flip")]
            rgba: Vec::new(),
        })
    }

    pub fn set_power(&mut self, t: f32) {
        self.power = t.clamp(0.0, 1.0);
    }

    pub fn set_bottom(&mut self, px: f32) {
        self.bottom = px.max(0.0);
    }

    pub fn frame(&self) -> (f32, f32, f32, f32) {
        self.placed()
    }

    fn placed(&self) -> (f32, f32, f32, f32) {
        let foot = self.bottom;
        match self.fills {
            true => (0.0, 0.0, OUT_W as f32, OUT_H as f32 - foot),
            false => screen_rect_above(self.power, foot),
        }
    }

    pub fn set_fills_frame(&mut self, on: bool) {
        self.fills = on;
    }

    pub fn set_picture(&mut self, rect: [f32; 4]) {
        self.pic = rect;
    }

    pub fn set_fbo_scale(&mut self, x: f32, y: f32) {
        self.fbo_scale = [x, y];
    }

    pub fn set_origin(&mut self, x: f32, y: f32) {
        self.origin = [x, y];
    }

    pub fn set_effect(&mut self, effect: ScreenEffect) {
        self.effect = effect;
    }

    pub fn set_paper(&mut self, size: u32, gray: &[u8]) {
        let tex = crate::gl::texture(
            size,
            size,
            gl::NEAREST,
            gl::REPEAT,
            crate::gl::gray_format(),
            Some(gray),
        );
        unsafe { gl::DeleteTextures(1, &self.paper) };
        self.paper = tex;
        self.paper_size = size as f32;
    }

    pub fn set_source_rect(&mut self, rect: [f32; 4]) {
        self.src = rect;
    }

    pub fn upload(&mut self, xrgb8888: &[u8]) {
        self.upload_sized(xrgb8888, SRC_W, SRC_H);
    }

    /// Upload a picture of `w` x `h`. The texture is re-created when the size
    /// changes (a core switching resolution, or a different system), and the
    /// shader is told the new texel grid; the common case is no change.
    pub fn upload_sized(&mut self, xrgb8888: &[u8], w: u32, h: u32) {
        if w == 0 || h == 0 || xrgb8888.len() < (w * h * 4) as usize {
            return;
        }
        if (w, h) != (self.tex_w, self.tex_h) {
            self.resize(w, h);
        }
        #[cfg(feature = "flip")]
        unsafe {
            // This Mali GLES driver does not reliably show TexSubImage2D updates
            // (the game image froze / went white), so the whole texture is
            // re-specified with TexImage2D each frame, which it does honour.
            // The core outputs BGRX; swizzle to RGBA once into a reused buffer.
            let px = (w * h) as usize;
            let need = px * 4;
            if self.rgba.len() != need {
                self.rgba = vec![0xffu8; need];
            }
            let avail = (xrgb8888.len() / 4).min(px);
            let dst = self.rgba.as_mut_ptr();
            let s0 = xrgb8888.as_ptr();
            for i in 0..avail {
                let s = i * 4;
                *dst.add(s) = *s0.add(s + 2); // R
                *dst.add(s + 1) = *s0.add(s + 1); // G
                *dst.add(s + 2) = *s0.add(s); // B
                *dst.add(s + 3) = 0xff; // A
            }
            gl::BindTexture(gl::TEXTURE_2D, self.game);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            // Update in place rather than reallocating the texture every frame.
            // Reallocating raced the GPU still sampling the previous frame,
            // which produced the corrupted pixels, and it was slow enough to
            // drop the emulator below full speed (audible as sped-up audio).
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                w as i32,
                h as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                self.rgba.as_ptr() as *const std::ffi::c_void,
            );
        }
        #[cfg(not(feature = "flip"))]
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.game);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                w as i32,
                h as i32,
                gl::BGRA,
                gl::UNSIGNED_BYTE,
                xrgb8888.as_ptr() as *const std::ffi::c_void,
            );
        }
    }

    fn resize(&mut self, w: u32, h: u32) {
        unsafe {
            gl::DeleteTextures(1, &self.game);
            self.game = game_texture(w, h);
            gl::UseProgram(self.prog);
            gl::Uniform2f(self.u_src, w as f32, h as f32);
        }
        self.tex_w = w;
        self.tex_h = h;
    }

    pub fn draw(&self, quad: &Quad) {
        let src = match self.effect {
            ScreenEffect::Grid => {
                whole_scale_source(self.src, self.fbo_scale, (self.tex_w, self.tex_h))
            }
            _ => self.src,
        };
        self.draw_source(self.game, quad, src);
    }

    pub fn draw_still(&self, tex: gl::types::GLuint, quad: &Quad) {
        self.draw_source(tex, quad, WHOLE_TEXTURE);
    }

    fn draw_source(&self, tex: gl::types::GLuint, quad: &Quad, src: [f32; 4]) {
        let (x, y, w, h) = self.placed();
        let bright = if self.fills {
            1.0
        } else {
            screen_brightness(self.power)
        };
        // The footer covers the bottom of a full-frame picture. Keep the part that sat above
        // it, so opening the scale does not squash the picture into the shorter rect.
        let mut src = src;
        if self.bottom > 0.0 {
            src[3] *= (OUT_H as f32 - self.bottom) / OUT_H as f32;
        }
        unsafe {
            gl::UseProgram(self.prog);
            gl::Uniform4f(self.u_rect, x, y, w, h);
            gl::Uniform4f(self.u_uv, src[0], src[1], src[2], src[3]);
            gl::Uniform1f(self.u_bright, bright);
            gl::Uniform1f(self.u_mode, self.effect.mode());
            gl::Uniform1f(self.u_paper_size, self.paper_size);
            gl::Uniform2f(self.u_fbo, self.fbo_scale[0], self.fbo_scale[1]);
            gl::Uniform2f(self.u_origin, self.origin[0], self.origin[1]);
            gl::Uniform4f(
                self.u_pic,
                self.pic[0],
                self.pic[1],
                self.pic[2],
                self.pic[3],
            );
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, self.paper);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, tex);
        }
        quad.draw();
    }
}

fn whole_scale_source(src: [f32; 4], fbo: [f32; 2], tex: (u32, u32)) -> [f32; 4] {
    let (x, w) = whole_scale(src[0], src[2], tex.0 as f32, OUT_W as f32 * fbo[0]);
    let (y, h) = whole_scale(src[1], src[3], tex.1 as f32, OUT_H as f32 * fbo[1]);
    [x, y, w, h]
}

fn whole_scale(at: f32, len: f32, texels: f32, out: f32) -> (f32, f32) {
    let shown = len * texels;
    let scale = (out / shown).ceil();
    let trim = scale * shown - out;
    if trim >= 0.1 * shown {
        return (at, len);
    }
    let top = (trim / 2.0).floor();
    (at + top / scale / texels, out / scale / texels)
}

impl Drop for GamePass {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.game);
            gl::DeleteTextures(1, &self.paper);
            gl::DeleteProgram(self.prog);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: [f32; 4], b: [f32; 4]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
    }

    #[test]
    fn the_canvas_sized_texture_fits_the_output_exactly() {
        let got = whole_scale_source(WHOLE_TEXTURE, [1.0, 1.0], (SRC_W, SRC_H));
        assert!(near(got, WHOLE_TEXTURE), "{got:?}");
    }

    #[test]
    fn the_texel_grid_comes_from_the_texture_not_the_canvas() {
        // One texel wider than the canvas: the whole-number scale now trims a
        // sliver, so the source must narrow. With the old fixed SRC_W this
        // would have looked identical to the canvas.
        let wider = whole_scale_source(WHOLE_TEXTURE, [1.0, 1.0], (SRC_W + 1, SRC_H));
        assert!(wider[2] < 1.0, "width should be trimmed: {wider:?}");
        assert!(
            near([0.0, wider[1], 0.0, wider[3]], [0.0, 0.0, 0.0, 1.0]),
            "height untouched"
        );
    }

    #[test]
    fn a_picture_that_would_lose_much_is_left_whole() {
        // A 4:3 PS1-sized frame does not divide the 3x output evenly; trimming
        // it to fit would cut a visible slice, so it is left as it is.
        let got = whole_scale_source(WHOLE_TEXTURE, [1.0, 1.0], (320, 240));
        assert!(near(got, WHOLE_TEXTURE), "{got:?}");
    }
}
