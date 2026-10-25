//! A `Surface` over `/dev/fb0` for devices with no GPU.
//!
//! The Miyoo Mini Flip's SigmaStar SSD202D has no 3D core and no libEGL, so the
//! GL path in `fbdev.rs` cannot start. This maps the framebuffer directly and
//! flips pages instead.
//!
//! The panel there reports `virtual_size 752,1680` — three 752x560 buffers
//! stacked — so `swap` writes the back buffer and hands it to the scanout with
//! `FBIOPAN_DISPLAY`, which is both tear-free and cheaper than a shadow blit.
//!
//! Where GL keeps an implicitly "current" context, this keeps an explicit
//! current target in a thread local, so `Compositor::end_frame` has somewhere to
//! write without the `Surface` trait growing a pixel accessor.

use std::cell::Cell;
use std::ffi::c_void;
use std::fs::OpenOptions;
use std::os::unix::io::{AsRawFd, RawFd};

use crate::surface::{GfxError, Surface};

const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
const FBIOPUT_VSCREENINFO: libc::c_ulong = 0x4601;
const FBIOGET_FSCREENINFO: libc::c_ulong = 0x4602;
const FBIOPAN_DISPLAY: libc::c_ulong = 0x4606;
const FBIO_WAITFORVSYNC: libc::c_ulong = 0x4620;

/// Pages given to a file-backed stand-in, matching the Mini Flip's three.
const FILE_PAGES: u32 = 3;

fn parse_size(s: String) -> Option<(u32, u32)> {
    let (w, h) = s.split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

/// Where `Compositor::end_frame` writes. Set by `make_current`, as a GL context
/// would be. Raw parts rather than a borrow: the compositor and the surface are
/// separate objects to the caller and neither owns the other.
#[derive(Copy, Clone)]
pub struct Target {
    pub ptr: *mut u8,
    pub w: u32,
    pub h: u32,
    /// Bytes per row, which is not always `w * 4`.
    pub stride: usize,
    pub bgr: bool,
}

thread_local! {
    static CURRENT: Cell<Option<Target>> = const { Cell::new(None) };
}

pub fn current() -> Option<Target> {
    CURRENT.with(|c| c.get())
}

fn set_current(t: Option<Target>) {
    CURRENT.with(|c| c.set(t));
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Bitfield {
    offset: u32,
    length: u32,
    msb_right: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct VarScreenInfo {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: Bitfield,
    green: Bitfield,
    blue: Bitfield,
    transp: Bitfield,
    nonstd: u32,
    activate: u32,
    height: u32,
    width: u32,
    accel_flags: u32,
    pixclock: u32,
    left_margin: u32,
    right_margin: u32,
    upper_margin: u32,
    lower_margin: u32,
    hsync_len: u32,
    vsync_len: u32,
    sync: u32,
    vmode: u32,
    rotate: u32,
    colorspace: u32,
    reserved: [u32; 4],
}

#[repr(C)]
struct FixScreenInfo {
    id: [u8; 16],
    smem_start: libc::c_ulong,
    smem_len: u32,
    kind: u32,
    type_aux: u32,
    visual: u32,
    xpanstep: u16,
    ypanstep: u16,
    ywrapstep: u16,
    line_length: u32,
    mmio_start: libc::c_ulong,
    mmio_len: u32,
    accel: u32,
    capabilities: u16,
    reserved: [u16; 2],
}

pub struct FbdevSurface {
    file: std::fs::File,
    map: *mut u8,
    map_len: usize,
    w: u32,
    h: u32,
    stride: usize,
    bgr: bool,
    /// How many whole screens fit in the mapping, so 3 on the Mini Flip.
    pages: u32,
    /// The page `end_frame` draws into; the other one is on screen.
    back: u32,
    /// Geometry came from SLOT_FB_SIZE, not a driver, so skip every ioctl.
    synthetic: bool,
    /// Block in `swap` until the panel latches. Off by default; see `swap`.
    wait_vsync: bool,
    /// Whether the one-off note about which swap path is in use has been made.
    said_path: bool,
}

fn ioctl<T>(fd: RawFd, req: libc::c_ulong, arg: &mut T) -> std::io::Result<()> {
    // SAFETY: `arg` is a correctly-sized struct for `req` in every call below.
    let rc = unsafe { libc::ioctl(fd, req as _, arg as *mut T as *mut c_void) };
    if rc < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

impl FbdevSurface {
    pub fn new() -> Result<Self, GfxError> {
        let path = std::env::var("SLOT_FB").unwrap_or_else(|_| "/dev/fb0".into());
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| GfxError::Context(format!("{path}: {e}")))?;
        let fd = file.as_raw_fd();

        let mut var = VarScreenInfo::default();
        // A plain file plus SLOT_FB_SIZE stands in for a panel, so the renderer
        // can be driven and its frames inspected off-device. A real framebuffer
        // never takes this path: the geometry still comes from the driver.
        if let Some(size) = std::env::var("SLOT_FB_SIZE").ok().and_then(parse_size) {
            var.xres = size.0;
            var.yres = size.1;
            var.xres_virtual = size.0;
            var.yres_virtual = size.1 * FILE_PAGES;
            var.bits_per_pixel = 32;
            var.red.offset = 16;
            let len = size.0 as u64 * size.1 as u64 * FILE_PAGES as u64 * 4;
            file.set_len(len)
                .map_err(|e| GfxError::Context(format!("{path}: set_len: {e}")))?;
        } else {
            ioctl(fd, FBIOGET_VSCREENINFO, &mut var)
                .map_err(|e| GfxError::Context(format!("FBIOGET_VSCREENINFO: {e}")))?;
        }
        // SAFETY: zeroed is a valid FixScreenInfo; the driver fills it.
        let mut fix: FixScreenInfo = unsafe { std::mem::zeroed() };
        if var.xres == 0 {
            return Err(GfxError::Context(format!("{path}: driver reports 0 width")));
        }
        if std::env::var_os("SLOT_FB_SIZE").is_none() {
            ioctl(fd, FBIOGET_FSCREENINFO, &mut fix)
                .map_err(|e| GfxError::Context(format!("FBIOGET_FSCREENINFO: {e}")))?;
        }

        if var.bits_per_pixel != 32 {
            return Err(GfxError::Context(format!(
                "{path}: {} bpp, this backend only does 32",
                var.bits_per_pixel
            )));
        }

        let stride = match fix.line_length {
            0 => var.xres_virtual as usize * 4,
            n => n as usize,
        };
        // Red at bit 16 means the byte order is BGRA, which is what the Mini
        // Flip reports and what nearly every 32bpp fbdev panel uses.
        let bgr = var.red.offset >= 16;
        // The driver offers three pages, but its pan is not free: on this
        // panel FBIOPAN_DISPLAY with FB_ACTIVATE_NOW costs 6.6 ms a frame,
        // which is the shape of a whole-page copy (1.68 MB at the bus speed we
        // see elsewhere) rather than a latch. That is 40% of the frame budget
        // spent inside an ioctl, and it is why in-game pacing sits at 50 Hz.
        //
        // `SLOT_FB_PAGES=1` gives up the flip and draws straight at the
        // scanout, waiting for the blank instead, which trades the copy for a
        // tearing risk. Per device in the launch script; measure both.
        let offered = (var.yres_virtual / var.yres.max(1)).max(1);
        let pages = match std::env::var("SLOT_FB_PAGES")
            .ok()
            .and_then(|v| v.trim().parse().ok())
        {
            Some(n) if n >= 1 => offered.min(n),
            _ => offered,
        };
        let map_len = stride * var.yres_virtual.max(var.yres) as usize;

        // SAFETY: mapping `map_len` bytes of the framebuffer we just opened.
        let map = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                map_len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if map == libc::MAP_FAILED {
            return Err(GfxError::Context(format!(
                "mmap {path}: {}",
                std::io::Error::last_os_error()
            )));
        }

        let mut me = FbdevSurface {
            file,
            map: map as *mut u8,
            map_len,
            w: var.xres,
            h: var.yres,
            stride,
            bgr,
            pages,
            back: 0,
            synthetic: std::env::var_os("SLOT_FB_SIZE").is_some(),
            wait_vsync: std::env::var("SLOT_FB_VSYNC").as_deref() == Ok("1"),
            said_path: false,
        };
        // Start on a cleared screen rather than whatever the launcher left.
        me.clear();
        me.make_current()?;
        Ok(me)
    }

    fn clear(&mut self) {
        // SAFETY: the whole mapping is ours and writable.
        unsafe { std::ptr::write_bytes(self.map, 0, self.map_len) };
    }

    fn page_offset(&self, page: u32) -> usize {
        self.stride * self.h as usize * page as usize
    }

    /// True when the driver gave us room for more than one page.
    pub fn flips(&self) -> bool {
        self.pages > 1
    }
}

impl Surface for FbdevSurface {
    fn make_current(&mut self) -> Result<(), GfxError> {
        let off = self.page_offset(self.back);
        set_current(Some(Target {
            // SAFETY: `off` is within the mapping by construction of `pages`.
            ptr: unsafe { self.map.add(off) },
            w: self.w,
            h: self.h,
            stride: self.stride,
            bgr: self.bgr,
        }));
        Ok(())
    }

    fn window_size(&self) -> (u32, u32) {
        (self.w, self.h)
    }

    fn swap(&mut self) -> Result<(), GfxError> {
        if self.pages == 1 && !self.synthetic {
            // Nothing to flip to: we drew straight at the scanout, so waiting
            // for the blank is the only way the frame lands whole. With pages to
            // flip between this is skipped, which is the normal case here.
            let mut arg = 0u32;
            let _ = ioctl(self.file.as_raw_fd(), FBIO_WAITFORVSYNC, &mut arg);
            return Ok(());
        }

        if self.synthetic {
            self.back = (self.back + 1) % self.pages;
            return self.make_current();
        }

        let mut var = VarScreenInfo::default();
        let fd = self.file.as_raw_fd();
        ioctl(fd, FBIOGET_VSCREENINFO, &mut var)
            .map_err(|e| GfxError::Context(format!("FBIOGET_VSCREENINFO: {e}")))?;
        var.xoffset = 0;
        var.yoffset = self.h * self.back;
        // FB_ACTIVATE_VBL (0x200) asks the driver to latch at the next vertical
        // blank, and on this device that *blocks* for the rest of the frame --
        // measured at 11 ms median, capped at one 16.7 ms period. slot already
        // paces itself, so waiting here as well puts two pacers in series and
        // costs most of the frame budget for nothing.
        //
        // FB_ACTIVATE_NOW hands the page over and returns; the hardware still
        // latches whole pages at its own blank, and with three buffers there is
        // a page in flight, so tearing is not expected. SLOT_FB_VSYNC=1 restores
        // the blocking behaviour if a panel turns out to need it.
        var.activate = if self.wait_vsync { 0x200 } else { 0x0 };
        if let Err(e) = ioctl(fd, FBIOPAN_DISPLAY, &mut var) {
            // Some SigmaStar kernels only accept the pan through PUT_VSCREENINFO,
            // which is a full mode set and much more expensive. Say so once:
            // which of these runs is worth knowing when the swap is costing most
            // of the frame.
            if !self.said_path {
                self.said_path = true;
                eprintln!("slot: fb: pan rejected ({e}), falling back to PUT_VSCREENINFO");
            }
            ioctl(fd, FBIOPUT_VSCREENINFO, &mut var)
                .map_err(|_| GfxError::Context(format!("FBIOPAN_DISPLAY: {e}")))?;
        } else if !self.said_path {
            self.said_path = true;
            eprintln!(
                "slot: fb: FBIOPAN_DISPLAY ok, {} pages, vsync wait {}",
                self.pages,
                if self.wait_vsync { "on" } else { "off" }
            );
        }

        self.back = (self.back + 1) % self.pages;
        self.make_current()
    }

    fn proc_address(&self, _name: &str) -> *const c_void {
        // There is no GL here. Nothing in the software path asks for one, and
        // `Compositor::new` must not call `gl::load` when this backend is built.
        std::ptr::null()
    }
}

impl Drop for FbdevSurface {
    fn drop(&mut self) {
        set_current(None);
        // SAFETY: unmapping exactly what we mapped.
        unsafe { libc::munmap(self.map as *mut c_void, self.map_len) };
    }
}
