macro_rules! eprintln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

mod bezel;
mod draw;
#[cfg(not(feature = "software"))]
mod fbdev;
#[cfg(not(feature = "software"))]
mod fbo;
#[cfg(not(feature = "software"))]
mod gl;
mod grade;
#[cfg(target_os = "macos")]
mod headless;
#[cfg(feature = "host")]
mod host;
mod layout;
mod lcd3x;
mod pipeline;
mod power;
#[cfg(not(feature = "software"))]
mod quad;
#[cfg(feature = "flip")]
mod sdl;
#[cfg(not(feature = "software"))]
mod shaders;
// The software backend, for a framebuffer with no GPU behind it. It exports the
// same Compositor and FbdevSurface the GL path does, so nothing above this crate
// is aware of which one it is talking to.
#[cfg(feature = "software")]
mod soft;
mod surface;

pub use bezel::{framed, game_rect, lifted_rect, screen_rect};
pub use draw::{Draw, TexId};
#[cfg(not(feature = "software"))]
pub use fbdev::{FbdevSurface, egl_error, panel_mode, panel_size};
#[cfg(not(feature = "software"))]
pub use fbo::{BACKDROP, Compositor, canvas_size};
#[cfg(not(feature = "software"))]
pub use gl::load as load_gl;
pub use grade::{BLUE_LIGHT_MAX, blue_light_gain};
#[cfg(target_os = "macos")]
pub use headless::HeadlessSurface;
#[cfg(feature = "host")]
pub use host::HostSurface;
pub use layout::Canvas;
pub use lcd3x::{ScreenEffect, lcd3x_factors, lcd3x_mask};
pub use pipeline::{SCALE, SRC_H, SRC_W, WHOLE_TEXTURE};
pub use power::{screen_brightness, screen_rect_above, screen_scale, screen_width};
#[cfg(feature = "flip")]
pub use sdl::SdlSurface;
#[cfg(feature = "software")]
pub use soft::{BACKDROP, Compositor, FbdevSurface, canvas_size};
pub use surface::{GfxError, OUT_H, OUT_W, Surface, blit_rect, blit_rect_fit, fit_rect, fit_scale};
