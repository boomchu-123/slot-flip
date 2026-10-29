use std::cell::Cell;
use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::marker::PhantomData;
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

use libloading::Library;

use crate::core::{AvInfo, ButtonMask, CoreError, RetroCore, GBA_H, GBA_W};
use crate::ffi::*;
use crate::link::Link;
use crate::rumble::Rumble;

const VIDEO_BYTES: usize = (GBA_W * GBA_H * 4) as usize;

static LIVE: AtomicBool = AtomicBool::new(false);

/// How many of a core's own on-screen messages go to the log. They are rare (PCSX
/// says "ANALOG ON" when the pad is switched), but a core that chatters every frame
/// must not fill the card's log.
static MESSAGES: AtomicU32 = AtomicU32::new(0);
const MESSAGE_LOG_LIMIT: u32 = 64;

/// How many times the core asked for a stick axis, and the furthest value it was
/// given on each, since `analog_stats` was last called. A core only asks while its
/// pad is a DualShock in analog mode, so a count of zero means the sticks are not
/// reaching the game however they are moved, and the peaks say what it was handed.
static ANALOG_READS: AtomicU32 = AtomicU32::new(0);
static ANALOG_PEAKS: [AtomicI32; 4] = [
    AtomicI32::new(0),
    AtomicI32::new(0),
    AtomicI32::new(0),
    AtomicI32::new(0),
];

/// The core's stick reads since the last call: how many, and the furthest value on
/// left x, left y, right x and right y. Starts the next count from zero.
pub fn analog_stats() -> (u32, [i16; 4]) {
    let reads = ANALOG_READS.swap(0, Ordering::Relaxed);
    let peaks = std::array::from_fn(|i| ANALOG_PEAKS[i].swap(0, Ordering::Relaxed) as i16);
    (reads, peaks)
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum PixelFormat {
    Xrgb8888,
    Rgb565,
}

struct Host {
    video: Vec<u8>,
    /// What `video` holds. Always the GBA canvas unless `native` is on.
    video_size: (u32, u32),
    native: bool,
    /// Environment commands the core has asked for that we do not handle, so each
    /// is reported once and not on every frame.
    unhandled: std::collections::HashSet<c_uint>,
    /// Core log lines already shown, so a core that repeats one is not repeated.
    logged: std::collections::HashSet<String>,
    format: PixelFormat,
    audio: Vec<i16>,
    inputs: [u16; 2],
    /// Left x, left y, right x, right y, for port 1.
    analog: [i16; 4],
    system_dir: CString,
    save_dir: CString,
    rumble: Rumble,
    asked_for_rumble: bool,
    netpacket: Option<NetpacketCallback>,
    net: Link,
    net_peer: Option<u16>,
    audio_status: Option<AudioBufferStatusFn>,
    options: std::collections::HashMap<String, std::ffi::CString>,
    options_dirty: bool,
    declared: std::collections::HashMap<String, Vec<String>>,
    /// What the core gave us to change discs with, if it is a disc core.
    disk: Option<DiskControl>,
}

thread_local! {
    static ACTIVE: Cell<*mut Host> = const { Cell::new(ptr::null_mut()) };
}

struct Active<'a>(PhantomData<&'a mut Host>);

impl Active<'_> {
    fn bind(host: &mut Host) -> Active<'_> {
        ACTIVE.with(|a| a.set(host as *mut Host));
        Active(PhantomData)
    }
}

impl Drop for Active<'_> {
    fn drop(&mut self) {
        ACTIVE.with(|a| a.set(ptr::null_mut()));
    }
}

unsafe fn with_host<R>(f: impl FnOnce(&mut Host) -> R) -> Option<R> {
    let p = ACTIVE.with(|a| a.get());
    if p.is_null() {
        return None;
    }
    Some(f(&mut *p))
}

/// Whether to show a core's routine chatter (debug and info), not only its
/// warnings and errors. Off unless `SLOT_CORE_LOG` is set.
fn core_log_all() -> bool {
    static ALL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ALL.get_or_init(|| std::env::var_os("SLOT_CORE_LOG").is_some())
}

/// What a core says about itself. libretro's callback is variadic (`printf`
/// style), which stable Rust cannot define, so this takes the two fixed
/// arguments and prints the format string as written: `%s` and all, which is
/// enough to name what went wrong. Without this a core that refuses a file does so
/// silently and nothing can be diagnosed. Each distinct message is shown once,
/// because the log is a file on the SD card.
unsafe extern "C" fn log_forward(level: c_uint, fmt: *const c_char) {
    // RETRO_LOG_DEBUG 0, INFO 1, WARN 2, ERROR 3.
    if fmt.is_null() || (level < 2 && !core_log_all()) {
        return;
    }
    let text = std::ffi::CStr::from_ptr(fmt).to_string_lossy();
    let text = text.trim_end();
    let first =
        with_host(|h| h.logged.len() < 64 && h.logged.insert(text.to_string())).unwrap_or(true);
    if first {
        eprintln!("slot: core[{level}]: {text}");
    }
}

unsafe extern "C" fn set_rumble_state(port: c_uint, effect: c_uint, strength: u16) -> bool {
    with_host(|h| h.rumble.set(port, effect, strength)).unwrap_or(false)
}

unsafe extern "C" fn environment(cmd: c_uint, data: *mut c_void) -> bool {
    match cmd {
        GET_CAN_DUPE => {
            if data.is_null() {
                return false;
            }
            *(data as *mut bool) = true;
            true
        }
        SET_PIXEL_FORMAT => {
            if data.is_null() {
                return false;
            }
            let raw = *(data as *const c_uint);
            let want = match raw {
                PIXEL_FORMAT_XRGB8888 => PixelFormat::Xrgb8888,
                PIXEL_FORMAT_RGB565 => PixelFormat::Rgb565,
                _ => {
                    eprintln!("slot: env SET_PIXEL_FORMAT unsupported {raw}");
                    return false;
                }
            };
            let name = match want {
                PixelFormat::Xrgb8888 => "xrgb8888",
                PixelFormat::Rgb565 => "rgb565",
            };
            eprintln!("slot: env SET_PIXEL_FORMAT {raw} -> {name}");
            with_host(|h| h.format = want).is_some()
        }
        GET_SYSTEM_DIRECTORY | GET_SAVE_DIRECTORY => {
            if data.is_null() {
                return false;
            }
            with_host(|h| {
                let dir = if cmd == GET_SYSTEM_DIRECTORY {
                    &h.system_dir
                } else {
                    &h.save_dir
                };
                *(data as *mut *const c_char) = dir.as_ptr();
                true
            })
            .unwrap_or(false)
        }
        GET_VARIABLE => {
            if data.is_null() {
                return false;
            }
            let var = &mut *(data as *mut Variable);
            if var.key.is_null() {
                return false;
            }
            let Ok(key) = std::ffi::CStr::from_ptr(var.key).to_str() else {
                return false;
            };
            with_host(|h| match h.options.get(key) {
                Some(value) => {
                    var.value = value.as_ptr();
                    true
                }
                None => {
                    var.value = ptr::null();
                    false
                }
            })
            .unwrap_or(false)
        }
        GET_VARIABLE_UPDATE => {
            if data.is_null() {
                return false;
            }
            with_host(|h| {
                *(data as *mut bool) = h.options_dirty;
                h.options_dirty = false;
                true
            })
            .unwrap_or(false)
        }
        SET_VARIABLES => {
            if data.is_null() {
                return with_host(|h| h.declared.clear()).is_some();
            }
            let mut list = std::collections::HashMap::new();
            let mut p = data as *const Variable;
            for _ in 0..4096 {
                let var = &*p;
                if var.key.is_null() {
                    break;
                }
                let Ok(key) = std::ffi::CStr::from_ptr(var.key).to_str() else {
                    break;
                };
                let values = if var.value.is_null() {
                    Vec::new()
                } else {
                    std::ffi::CStr::from_ptr(var.value)
                        .to_str()
                        .ok()
                        .and_then(|v| v.split_once(';'))
                        .map(|(_, values)| {
                            values
                                .trim()
                                .split('|')
                                .map(|s| s.trim().to_string())
                                .collect()
                        })
                        .unwrap_or_default()
                };
                list.insert(key.to_string(), values);
                p = p.add(1);
            }
            with_host(|h| h.declared = list).is_some()
        }
        GET_RUMBLE_INTERFACE => {
            if data.is_null() {
                return false;
            }
            with_host(|h| {
                h.asked_for_rumble = true;
                (*(data as *mut RumbleInterface)).set_rumble_state = set_rumble_state;
            })
            .is_some()
        }
        GET_LOG_INTERFACE => {
            if data.is_null() {
                return false;
            }
            (*(data as *mut LogCallback)).log = log_forward as *const c_void;
            true
        }
        SET_AUDIO_BUFFER_STATUS_CALLBACK => {
            if data.is_null() {
                return with_host(|h| h.audio_status = None).is_some();
            }
            let cb = std::ptr::read(data as *const AudioBufferStatusCallback);
            with_host(|h| h.audio_status = cb.callback).is_some()
        }
        GET_DISK_CONTROL_INTERFACE_VERSION => {
            if data.is_null() {
                return false;
            }
            // Version 1 is the one with labels and paths, which is what we read.
            *(data as *mut c_uint) = 1;
            true
        }
        SET_DISK_CONTROL_INTERFACE | SET_DISK_CONTROL_EXT_INTERFACE => {
            if data.is_null() {
                return with_host(|h| h.disk = None).is_some();
            }
            // The older struct is only the first seven members; reading past it
            // would be reading whatever the core had next to it.
            let bytes = if cmd == SET_DISK_CONTROL_INTERFACE {
                DISK_CONTROL_BASE_BYTES
            } else {
                std::mem::size_of::<DiskControl>()
            };
            let mut control: DiskControl = std::mem::zeroed();
            ptr::copy_nonoverlapping(data as *const u8, &mut control as *mut _ as *mut u8, bytes);
            with_host(|h| h.disk = Some(control)).is_some()
        }
        SET_NETPACKET_INTERFACE => {
            if data.is_null() {
                return with_host(|h| h.netpacket = None).is_some();
            }
            with_host(|h| {
                h.netpacket = Some(std::ptr::read(data as *const NetpacketCallback));
            })
            .is_some()
        }
        SET_MESSAGE => {
            // `struct retro_message { const char *msg; unsigned frames; }`. Slot shows
            // no core messages, but it used to drop them without a word, which hid
            // whether a DualShock had been switched between analog and digital.
            if !data.is_null() {
                let msg = *(data as *const *const c_char);
                if !msg.is_null() && MESSAGES.fetch_add(1, Ordering::Relaxed) < MESSAGE_LOG_LIMIT {
                    eprintln!("slot: core says: {}", CStr::from_ptr(msg).to_string_lossy());
                }
            }
            true
        }
        other => {
            // A core can ask the same thing every frame (PCSX asks for a software
            // framebuffer 60 times a second), and the log is a file on the SD card.
            // Say it once.
            let first = with_host(|h| h.unhandled.insert(other)).unwrap_or(true);
            if first {
                eprintln!("slot: env unhandled cmd {other}");
            }
            false
        }
    }
}

unsafe extern "C" fn netpacket_send(
    _flags: c_int,
    buf: *const c_void,
    len: usize,
    _client_id: u16,
) {
    if buf.is_null() || len == 0 {
        return;
    }
    let bytes = std::slice::from_raw_parts(buf as *const u8, len).to_vec();
    with_host(|h| h.net.push_outbound(bytes));
}

unsafe extern "C" fn netpacket_poll_receive() {
    let Some((receive, net, client_id)) = with_host(|h| {
        let receive = h.netpacket.as_ref().and_then(|cb| cb.receive)?;
        let client_id = h.net_peer?;
        Some((receive, h.net.clone(), client_id))
    })
    .flatten() else {
        return;
    };
    if !net.is_active() {
        return;
    }
    while let Some(packet) = net.take_inbound() {
        receive(packet.as_ptr() as *const c_void, packet.len(), client_id);
    }
}

unsafe fn begin_link(client_id: u16) {
    let Some(start) = with_host(|h| h.netpacket.as_ref().and_then(|cb| cb.start)).flatten() else {
        return;
    };
    start(client_id, netpacket_send, netpacket_poll_receive);

    let peer = 1u16.wrapping_sub(client_id);
    let connected = with_host(|h| {
        h.net.set_active(true);
        h.net_peer = Some(peer);
        h.netpacket.as_ref().and_then(|cb| cb.connected)
    })
    .flatten();
    if let Some(connected) = connected {
        connected(peer);
    }
}

unsafe fn halt_link() {
    let Some((stop, peer, disconnected)) = with_host(|h| {
        let stop = h.netpacket.as_ref().and_then(|cb| cb.stop);
        let peer = h.net_peer.take();
        let disconnected = h.netpacket.as_ref().and_then(|cb| cb.disconnected);
        (stop, peer, disconnected)
    }) else {
        return;
    };
    if let (Some(peer), Some(disconnected)) = (peer, disconnected) {
        disconnected(peer);
    }
    if let Some(stop) = stop {
        stop();
    }
}

unsafe fn drain_link() {
    let Some((receive, poll, net, client_id)) = with_host(|h| {
        let cb = h.netpacket.as_ref()?;
        let client_id = h.net_peer?;
        Some((cb.receive, cb.poll, h.net.clone(), client_id))
    })
    .flatten() else {
        return;
    };
    if !net.is_active() {
        return;
    }
    if let Some(receive) = receive {
        while let Some(packet) = net.take_inbound() {
            receive(packet.as_ptr() as *const c_void, packet.len(), client_id);
        }
    }
    if let Some(poll) = poll {
        poll();
    }
}

unsafe fn report_audio_status(skip: bool) {
    let Some(status) = with_host(|h| h.audio_status).flatten() else {
        return;
    };
    status(true, if skip { 0 } else { 100 }, skip);
}

unsafe extern "C" fn video_refresh(
    data: *const c_void,
    width: c_uint,
    height: c_uint,
    pitch: usize,
) {
    if data.is_null() {
        return;
    }
    with_host(|h| {
        if h.native {
            copy_native(h, data as *const u8, width, height, pitch);
            return;
        }
        let cols = width.min(GBA_W) as usize;
        let rows = height.min(GBA_H) as usize;
        let ox = (GBA_W as usize - cols) / 2;
        let oy = (GBA_H as usize - rows) / 2;
        if cols != GBA_W as usize || rows != GBA_H as usize {
            h.video.fill(0);
        }
        for y in 0..rows {
            let src = (data as *const u8).add(y * pitch);
            let row = ((y + oy) * GBA_W as usize + ox) * 4;
            copy_row(&mut h.video, h.format, src, row, cols);
        }
    });
}

/// The largest picture a core may report in native-video mode. PS1 tops out
/// at 640x480 (a little more in PAL), so this only guards a core that reports
/// nonsense from making the host allocate without limit.
const MAX_NATIVE: u32 = 1024;

/// Keep the frame at the size the core drew it: nothing is cropped or
/// centred, and the buffer follows the core when it changes resolution.
unsafe fn copy_native(h: &mut Host, src: *const u8, width: c_uint, height: c_uint, pitch: usize) {
    let cols = width.min(MAX_NATIVE) as usize;
    let rows = height.min(MAX_NATIVE) as usize;
    if cols == 0 || rows == 0 {
        return;
    }
    let need = cols * rows * 4;
    if h.video.len() != need {
        h.video.clear();
        h.video.resize(need, 0);
    }
    h.video_size = (cols as u32, rows as u32);
    for y in 0..rows {
        copy_row(
            &mut h.video,
            h.format,
            src.add(y * pitch),
            y * cols * 4,
            cols,
        );
    }
}

/// One scanline of `cols` pixels from the core into `video` at byte `row`,
/// as XRGB8888 whichever format the core uses.
unsafe fn copy_row(video: &mut [u8], format: PixelFormat, src: *const u8, row: usize, cols: usize) {
    match format {
        PixelFormat::Xrgb8888 => {
            ptr::copy_nonoverlapping(src, video.as_mut_ptr().add(row), cols * 4);
        }
        PixelFormat::Rgb565 => {
            for x in 0..cols {
                let p = ptr::read_unaligned((src as *const u16).add(x));
                let r = ((p >> 11) & 0x1f) as u8;
                let g = ((p >> 5) & 0x3f) as u8;
                let b = (p & 0x1f) as u8;
                let o = row + x * 4;
                video[o] = (b << 3) | (b >> 2);
                video[o + 1] = (g << 2) | (g >> 4);
                video[o + 2] = (r << 3) | (r >> 2);
                video[o + 3] = 0;
            }
        }
    }
}

unsafe extern "C" fn audio_sample(left: i16, right: i16) {
    with_host(|h| h.audio.extend_from_slice(&[left, right]));
}

unsafe extern "C" fn audio_batch(data: *const i16, frames: usize) -> usize {
    if !data.is_null() {
        with_host(|h| {
            h.audio
                .extend_from_slice(std::slice::from_raw_parts(data, frames * 2))
        });
    }
    frames
}

unsafe extern "C" fn input_poll() {}

unsafe extern "C" fn input_state(port: c_uint, device: c_uint, index: c_uint, id: c_uint) -> i16 {
    if device == DEVICE_ANALOG {
        // Only port 1 has sticks; the axes are the two sticks' x and y.
        if port != 0 {
            return 0;
        }
        let at = match (index, id) {
            (ANALOG_LEFT, ANALOG_X) => 0,
            (ANALOG_LEFT, ANALOG_Y) => 1,
            (ANALOG_RIGHT, ANALOG_X) => 2,
            (ANALOG_RIGHT, ANALOG_Y) => 3,
            _ => return 0,
        };
        let value = with_host(|h| h.analog[at]).unwrap_or(0);
        ANALOG_READS.fetch_add(1, Ordering::Relaxed);
        if i32::from(value).abs() > ANALOG_PEAKS[at].load(Ordering::Relaxed).abs() {
            ANALOG_PEAKS[at].store(i32::from(value), Ordering::Relaxed);
        }
        return value;
    }
    if device != DEVICE_JOYPAD {
        return 0;
    }
    with_host(|h| {
        let Some(&input) = h.inputs.get(port as usize) else {
            return 0;
        };
        match id {
            JOYPAD_MASK => input as i16,
            _ if id < 16 => ((input >> id) & 1) as i16,
            _ => 0,
        }
    })
    .unwrap_or(0)
}

pub struct LibretroCore {
    api: Api,
    host: Box<Host>,
    rom: Vec<u8>,
    rom_path: Option<CString>,
    av: AvInfo,
    loaded: bool,
    state_size: Option<(usize, usize)>,
    swap: Option<Swap>,
    dualshock: bool,
    _lib: Library,
}

/// A disc change in progress. The lid is opened, held open while the game
/// notices, the disc is switched while it is open, and only then closed: a game
/// polls the drive, so an instant swap is often not seen at all.
struct Swap {
    target: usize,
    frames: u32,
}

/// Frames the lid stays open before the disc is switched, and before it is closed.
/// At 60 fps that is three quarters of a second and then one and a half.
pub const SWAP_SWITCH_FRAME: u32 = 45;
pub const SWAP_CLOSE_FRAME: u32 = 90;

/// What a core says about itself before any content is loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemInformation {
    pub name: String,
    pub version: String,
    /// The file extensions the core opens, lower case and without the dot.
    pub extensions: Vec<String>,
    /// The core opens its content itself, by path, rather than from memory.
    pub need_fullpath: bool,
}

unsafe fn text(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
}

/// The bytes to hand a core for `path`. A core that sets `need_fullpath`
/// (disc images, in practice) opens the files itself and may ignore `data`, so
/// reading a multi-hundred-megabyte image into RAM for it is waste and can
/// exhaust memory on a handheld. It still gets a missing file reported here.
/// `need_fullpath` here also stands for a core a launcher asked to load by path.
fn content_bytes(path: &Path, need_fullpath: bool) -> std::io::Result<Vec<u8>> {
    if need_fullpath {
        std::fs::metadata(path)?;
        return Ok(Vec::new());
    }
    std::fs::read(path)
}

/// Whether `name` (a core's library name) is listed in `SLOT_LOAD_BY_PATH`, a
/// comma-separated list set by a launcher for cores that are better given a
/// path than the file's bytes. A core that accepts data copies it, so on a
/// console with little memory a large ROM costs twice over: slot's own copy for
/// as long as the game runs, and the core's. mGBA's loader makes that copy with
/// an anonymous map and writes to whatever the map returns, so when the map
/// fails it crashes at once (a 16 MB ROM on the Mini Plus). Given a path it maps
/// the file instead, which the kernel can drop and re-read.
///
/// Only for a ROM of `BY_PATH_FROM` bytes or more: a small one costs little to
/// copy, and keeps loading the way it is known to.
fn loads_by_path(name: &str, list: Option<&str>, rom_len: u64) -> bool {
    rom_len >= BY_PATH_FROM
        && list.is_some_and(|l| l.split(',').any(|n| n.trim().eq_ignore_ascii_case(name)))
}

const BY_PATH_FROM: u64 = 8 * 1024 * 1024;

/// A short form of the same, for a line printed every couple of seconds: what
/// the system could still hand out, and how much of this process is its own
/// (anonymous) memory and how much is files it has mapped, which the kernel can
/// take back.
pub fn memory_brief() -> String {
    let field = |text: &str, key: &str| -> Option<u64> {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse().ok())
    };
    let info = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let me = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let mb =
        |kb: Option<u64>| kb.map_or_else(|| "?".into(), |k| format!("{:.1}", k as f64 / 1024.0));
    format!(
        "mem available {} MB, free {} MB, own anon {} MB, own file {} MB",
        mb(field(&info, "MemAvailable:")),
        mb(field(&info, "MemFree:")),
        mb(field(&me, "RssAnon:")),
        mb(field(&me, "RssFile:")),
    )
}

/// The memory figures worth having in a report, on one line, so a failure to
/// allocate can be told apart from a bug: what is free, what the kernel will
/// promise, and what this process holds.
pub fn memory_note() -> String {
    let read = |path: &str| std::fs::read_to_string(path).unwrap_or_default();
    let pick = |text: &str, keys: &[&str]| -> Vec<String> {
        text.lines()
            .filter(|l| keys.iter().any(|k| l.starts_with(k)))
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect()
    };
    let mut parts = pick(
        &read("/proc/meminfo"),
        &[
            "MemTotal:",
            "MemFree:",
            "MemAvailable:",
            "CommitLimit:",
            "Committed_AS:",
            "Cached:",
            "Shmem:",
            "Slab:",
        ],
    );
    parts.extend(pick(
        &read("/proc/self/status"),
        &[
            "VmPeak:",
            "VmSize:",
            "VmRSS:",
            "RssAnon:",
            "RssFile:",
            "RssShmem:",
        ],
    ));
    let mode = read("/proc/sys/vm/overcommit_memory");
    if !mode.trim().is_empty() {
        parts.push(format!("overcommit_memory {}", mode.trim()));
    }
    parts.join(", ")
}

fn cdir(path: &Path) -> Result<CString, CoreError> {
    CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| CoreError::Load(format!("{} contains a nul", path.display())))
}

impl LibretroCore {
    pub fn open(dylib: &Path) -> Result<Self, CoreError> {
        let dir = dylib.parent().unwrap_or(Path::new(".")).to_path_buf();
        Self::open_with(dylib, &dir, &dir)
    }

    pub fn open_with(dylib: &Path, system_dir: &Path, save_dir: &Path) -> Result<Self, CoreError> {
        if LIVE.swap(true, Ordering::SeqCst) {
            return Err(CoreError::Unsupported("a core is already open".into()));
        }
        Self::open_inner(dylib, system_dir, save_dir)
            .inspect_err(|_| LIVE.store(false, Ordering::SeqCst))
    }

    pub fn system_info(&self) -> SystemInformation {
        let mut sys = SystemInfo::default();
        unsafe { (self.api.get_system_info)(&mut sys) };
        let (name, version, extensions) = unsafe {
            (
                text(sys.library_name),
                text(sys.library_version),
                text(sys.valid_extensions),
            )
        };
        SystemInformation {
            name,
            version,
            extensions: extensions
                .split('|')
                .filter(|e| !e.is_empty())
                .map(|e| e.to_ascii_lowercase())
                .collect(),
            need_fullpath: sys.need_fullpath,
        }
    }

    pub fn reported_system_dir(&self) -> String {
        self.host.system_dir.to_string_lossy().into_owned()
    }

    pub fn reported_save_dir(&self) -> String {
        self.host.save_dir.to_string_lossy().into_owned()
    }

    pub fn asked_for_rumble(&self) -> bool {
        self.host.asked_for_rumble
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        if !self.host.declared.is_empty() {
            match self.host.declared.get(key) {
                None => eprintln!("slot-retro: core declares no option {key:?}, setting it anyway"),
                Some(values) if !values.is_empty() && !values.iter().any(|v| v == value) => {
                    eprintln!("slot-retro: core declares {key:?} as {values:?}, not {value:?}")
                }
                Some(_) => {}
            }
        }
        let Ok(value) = CString::new(value) else {
            return;
        };
        self.host.options.insert(key.to_string(), value);
        self.host.options_dirty = true;
    }

    pub fn options(&self) -> Vec<(String, String)> {
        self.host
            .options
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), v.to_str().ok()?.to_string())))
            .collect()
    }

    pub fn declared_options(&self) -> &std::collections::HashMap<String, Vec<String>> {
        &self.host.declared
    }

    pub fn option(&self, key: &str) -> Option<String> {
        self.host
            .options
            .get(key)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    }

    fn open_inner(dylib: &Path, system_dir: &Path, save_dir: &Path) -> Result<Self, CoreError> {
        let lib = unsafe { Library::new(dylib) }.map_err(|e| CoreError::Load(e.to_string()))?;
        let api = unsafe { Api::load(&lib) }?;
        let version = unsafe { (api.api_version)() };
        if version != API_VERSION {
            return Err(CoreError::Unsupported(format!("libretro api {version}")));
        }
        let mut host = Box::new(Host {
            video: vec![0; VIDEO_BYTES],
            video_size: (GBA_W, GBA_H),
            native: false,
            unhandled: std::collections::HashSet::new(),
            logged: std::collections::HashSet::new(),
            format: PixelFormat::Xrgb8888,
            audio: Vec::new(),
            inputs: [0; 2],
            analog: [0; 4],
            system_dir: cdir(system_dir)?,
            save_dir: cdir(save_dir)?,
            rumble: Rumble::default(),
            asked_for_rumble: false,
            netpacket: None,
            net: Link::default(),
            net_peer: None,
            audio_status: None,
            options: std::collections::HashMap::new(),
            options_dirty: false,
            declared: std::collections::HashMap::new(),
            disk: None,
        });
        unsafe {
            let _a = Active::bind(&mut host);
            (api.set_environment)(environment);
            (api.set_video_refresh)(video_refresh);
            (api.set_audio_sample)(audio_sample);
            (api.set_audio_sample_batch)(audio_batch);
            (api.set_input_poll)(input_poll);
            (api.set_input_state)(input_state);
            (api.init)();
        }
        Ok(LibretroCore {
            api,
            host,
            rom: Vec::new(),
            rom_path: None,
            av: AvInfo {
                fps: 0.0,
                sample_rate: 0.0,
            },
            loaded: false,
            state_size: None,
            swap: None,
            dualshock: false,
            _lib: lib,
        })
    }

    /// How many discs the loaded game has. One (or none) means there is nothing
    /// to change, so the caller need not offer it.
    pub fn disc_count(&self) -> usize {
        let Some(count) = self.host.disk.and_then(|d| d.get_num_images) else {
            return 0;
        };
        unsafe { count() as usize }
    }

    /// The disc in the drive, or the one it is being changed to.
    pub fn disc_index(&self) -> usize {
        if let Some(swap) = &self.swap {
            return swap.target;
        }
        let Some(index) = self.host.disk.and_then(|d| d.get_image_index) else {
            return 0;
        };
        unsafe { index() as usize }
    }

    /// Whether a disc change is still under way.
    pub fn disc_changing(&self) -> bool {
        self.swap.is_some()
    }

    /// Start changing to disc `to`. The change finishes over the next frames, so
    /// the game sees the lid open and close. False if there is no such disc or
    /// the game has only the one; asking again mid-change just retargets it.
    pub fn change_disc(&mut self, to: usize) -> bool {
        if !self.loaded || self.disc_count() < 2 || to >= self.disc_count() {
            return false;
        }
        if let Some(swap) = &mut self.swap {
            swap.target = to;
            return true;
        }
        if to == self.disc_index() {
            return true;
        }
        let Some(eject) = self.host.disk.and_then(|d| d.set_eject_state) else {
            return false;
        };
        let _a = Active::bind(&mut self.host);
        if !unsafe { eject(true) } {
            return false;
        }
        self.swap = Some(Swap {
            target: to,
            frames: 0,
        });
        true
    }

    fn advance_swap(&mut self) {
        let Some(swap) = &mut self.swap else { return };
        swap.frames += 1;
        let (frames, target) = (swap.frames, swap.target);
        let Some(control) = self.host.disk else {
            self.swap = None;
            return;
        };
        let _a = Active::bind(&mut self.host);
        if frames == SWAP_SWITCH_FRAME {
            if let Some(set) = control.set_image_index {
                if !unsafe { set(target as c_uint) } {
                    eprintln!("slot-retro: the core would not switch to disc {target}");
                }
            }
        }
        if frames >= SWAP_CLOSE_FRAME {
            if let Some(eject) = control.set_eject_state {
                unsafe { eject(false) };
            }
            self.swap = None;
        }
    }

    /// The pad plugged into port 1: a plain one, or a DualShock when sticks are wanted.
    /// A core forgets it when a game loads, so this is said again after every load.
    fn plug_in_pad(&mut self) {
        let device = if self.dualshock {
            DEVICE_PSE_DUALSHOCK
        } else {
            DEVICE_JOYPAD
        };
        let _a = Active::bind(&mut self.host);
        unsafe { (self.api.set_controller_port_device)(0, device) };
    }

    fn unload(&mut self) {
        self.swap = None;
        if !self.loaded {
            return;
        }
        let _a = Active::bind(&mut self.host);
        unsafe { (self.api.unload_game)() };
        self.loaded = false;
    }

    fn serialize_at(&mut self, size: usize) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; size];
        let ok = {
            let _a = Active::bind(&mut self.host);
            unsafe { (self.api.serialize)(buf.as_mut_ptr() as *mut c_void, size) }
        };
        ok.then_some(buf)
    }

    fn memory(&self, id: c_uint) -> Option<&[u8]> {
        let data = unsafe { (self.api.get_memory_data)(id) };
        let len = unsafe { (self.api.get_memory_size)(id) };
        if data.is_null() || len == 0 {
            return None;
        }
        Some(unsafe { std::slice::from_raw_parts(data as *const u8, len) })
    }

    fn memory_mut(&mut self, id: c_uint) -> Option<&mut [u8]> {
        let data = unsafe { (self.api.get_memory_data)(id) };
        let len = unsafe { (self.api.get_memory_size)(id) };
        if data.is_null() || len == 0 {
            return None;
        }
        Some(unsafe { std::slice::from_raw_parts_mut(data as *mut u8, len) })
    }
}

impl Drop for LibretroCore {
    fn drop(&mut self) {
        self.unload();
        unsafe {
            let _a = Active::bind(&mut self.host);
            (self.api.deinit)();
        }
        LIVE.store(false, Ordering::SeqCst);
    }
}

impl RetroCore for LibretroCore {
    fn set_option(&mut self, key: &str, value: &str) {
        LibretroCore::set_option(self, key, value);
    }

    fn load(&mut self, rom: &Path) -> Result<(), CoreError> {
        self.unload();
        self.state_size = None;
        let mut sys = SystemInfo::default();
        unsafe { (self.api.get_system_info)(&mut sys) };
        let name = unsafe { text(sys.library_name) };
        let by_path = sys.need_fullpath
            || loads_by_path(
                &name,
                std::env::var("SLOT_LOAD_BY_PATH").ok().as_deref(),
                std::fs::metadata(rom).map_or(0, |m| m.len()),
            );
        eprintln!(
            "slot: core {name}: loading {} ({}); {}",
            rom.display(),
            if by_path { "by path" } else { "from memory" },
            memory_note()
        );
        self.rom = content_bytes(rom, by_path)?;
        self.rom_path = Some(
            CString::new(rom.as_os_str().as_encoded_bytes())
                .map_err(|_| CoreError::Load("rom path contains a nul".into()))?,
        );
        let info = GameInfo {
            path: self.rom_path.as_ref().map_or(ptr::null(), |p| p.as_ptr()),
            data: if self.rom.is_empty() {
                ptr::null()
            } else {
                self.rom.as_ptr() as *const c_void
            },
            size: self.rom.len(),
            meta: ptr::null(),
        };
        let ok = {
            let _a = Active::bind(&mut self.host);
            unsafe { (self.api.load_game)(&info) }
        };
        if !ok {
            self.rom = Vec::new();
            self.rom_path = None;
            return Err(CoreError::Load(format!("core refused {}", rom.display())));
        }
        self.loaded = true;
        let mut av = SystemAvInfo::default();
        unsafe { (self.api.get_system_av_info)(&mut av) };
        self.av = AvInfo {
            fps: av.timing.fps,
            sample_rate: av.timing.sample_rate,
        };
        self.plug_in_pad();
        Ok(())
    }

    fn run_frame(&mut self, input: ButtonMask) {
        self.run_frame_linked(input, ButtonMask::default());
    }

    fn run_frame_linked(&mut self, p1: ButtonMask, p2: ButtonMask) {
        if !self.loaded {
            return;
        }
        self.advance_swap();
        self.host.inputs = [p1.0, p2.0];
        let _a = Active::bind(&mut self.host);
        unsafe { (self.api.run)() };
    }

    fn set_analog(&mut self, sticks: [i16; 4]) {
        self.host.analog = sticks;
    }

    fn set_dualshock(&mut self, on: bool) {
        if self.dualshock == on {
            return;
        }
        self.dualshock = on;
        if self.loaded {
            self.plug_in_pad();
        }
        if !on {
            self.host.analog = [0; 4];
        }
    }

    fn disc_count(&self) -> usize {
        LibretroCore::disc_count(self)
    }

    fn disc_index(&self) -> usize {
        LibretroCore::disc_index(self)
    }

    fn disc_changing(&self) -> bool {
        LibretroCore::disc_changing(self)
    }

    fn change_disc(&mut self, to: usize) -> bool {
        LibretroCore::change_disc(self, to)
    }

    fn set_frame_skip(&mut self, skip: bool) {
        let _a = Active::bind(&mut self.host);
        unsafe { report_audio_status(skip) };
    }

    fn video_xrgb8888(&self) -> &[u8] {
        &self.host.video
    }

    fn video_size(&self) -> (u32, u32) {
        self.host.video_size
    }

    fn set_native_video(&mut self, on: bool) {
        if self.host.native == on {
            return;
        }
        self.host.native = on;
        // Back to the fixed canvas, blank, so the next frame starts clean. Turning
        // it on needs nothing: the buffer is resized by the first frame.
        if !on {
            self.host.video = vec![0; VIDEO_BYTES];
            self.host.video_size = (GBA_W, GBA_H);
        }
    }

    fn take_audio(&mut self) -> Vec<i16> {
        std::mem::take(&mut self.host.audio)
    }

    fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
        let chip = unsafe { (self.api.get_memory_size)(MEMORY_SAVE_RAM) };
        if let Some(buf) = self
            .state_size
            .filter(|&(_, at)| at == chip)
            .and_then(|(size, _)| self.serialize_at(size))
        {
            return Ok(buf);
        }
        let size = unsafe { (self.api.serialize_size)() };
        self.state_size = (size > 0).then_some((size, chip));
        if size == 0 {
            return Err(CoreError::State("core reports no state".into()));
        }
        self.serialize_at(size)
            .ok_or_else(|| CoreError::State("serialize refused".into()))
    }

    fn unserialize(&mut self, data: &[u8]) -> Result<(), CoreError> {
        if data.is_empty() {
            return Err(CoreError::State("empty state".into()));
        }
        let ok = {
            let _a = Active::bind(&mut self.host);
            unsafe { (self.api.unserialize)(data.as_ptr() as *const c_void, data.len()) }
        };
        if ok {
            Ok(())
        } else {
            Err(CoreError::State("unserialize refused".into()))
        }
    }

    fn save_ram(&self) -> Option<Vec<u8>> {
        let mut sav = self.memory(MEMORY_SAVE_RAM)?.to_vec();
        if let Some(rtc) = self.memory(MEMORY_RTC) {
            sav.extend_from_slice(rtc);
        }
        Some(sav)
    }

    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), CoreError> {
        let Some(ram) = self.memory_mut(MEMORY_SAVE_RAM) else {
            return Err(CoreError::Unsupported("core exposes no save ram".into()));
        };
        let n = ram.len().min(data.len());
        ram[..n].copy_from_slice(&data[..n]);
        let rest = &data[n..];
        if let Some(rtc) = self.memory_mut(MEMORY_RTC) {
            let m = rtc.len().min(rest.len());
            rtc[..m].copy_from_slice(&rest[..m]);
        }
        Ok(())
    }

    fn av_info(&self) -> AvInfo {
        self.av
    }

    fn rumble(&self) -> Rumble {
        self.host.rumble.clone()
    }

    fn net(&self) -> Link {
        self.host.net.clone()
    }

    fn start_link(&mut self, client_id: u16) {
        let _a = Active::bind(&mut self.host);
        unsafe { begin_link(client_id) };
    }

    fn pump_link(&mut self) {
        let _a = Active::bind(&mut self.host);
        unsafe { drain_link() };
    }

    fn stop_link(&mut self) {
        let _a = Active::bind(&mut self.host);
        unsafe { halt_link() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn a_core_is_loaded_by_path_only_when_the_launcher_lists_it() {
        let big = BY_PATH_FROM;
        assert!(!loads_by_path("mGBA", None, big));
        assert!(!loads_by_path("mGBA", Some(""), big));
        assert!(!loads_by_path("gpSP", Some("mGBA"), big));
        assert!(loads_by_path("mGBA", Some("mGBA"), big));
        assert!(loads_by_path("mGBA", Some("gpsp, MGBA"), big));
        assert!(!loads_by_path("mGBA", Some("mGBA2"), big));
    }

    #[test]
    fn a_small_rom_keeps_loading_from_memory() {
        assert!(!loads_by_path("mGBA", Some("mGBA"), BY_PATH_FROM - 1));
        assert!(!loads_by_path("mGBA", Some("mGBA"), 0));
    }

    #[test]
    fn the_memory_note_is_one_line_and_never_panics() {
        let note = memory_note();
        assert!(!note.contains('\n'), "{note}");
        let brief = memory_brief();
        assert!(
            !brief.contains('\n') && brief.starts_with("mem available"),
            "{brief}"
        );
    }

    use std::collections::HashMap;
    use std::ffi::CStr;

    fn host_with(options: HashMap<String, CString>, options_dirty: bool) -> Box<Host> {
        Box::new(Host {
            video: vec![0; VIDEO_BYTES],
            video_size: (GBA_W, GBA_H),
            native: false,
            unhandled: std::collections::HashSet::new(),
            logged: std::collections::HashSet::new(),
            format: PixelFormat::Xrgb8888,
            audio: Vec::new(),
            inputs: [0; 2],
            analog: [0; 4],
            system_dir: CString::new(".").unwrap(),
            save_dir: CString::new(".").unwrap(),
            rumble: Rumble::default(),
            asked_for_rumble: false,
            netpacket: None,
            net: Link::default(),
            net_peer: None,
            audio_status: None,
            options,
            options_dirty,
            declared: HashMap::new(),
            disk: None,
        })
    }

    fn declaration(key: &str, value: &str) -> (Variable, CString, CString) {
        let key = CString::new(key).unwrap();
        let value = CString::new(value).unwrap();
        let var = Variable {
            key: key.as_ptr(),
            value: value.as_ptr(),
        };
        (var, key, value)
    }

    fn end_of_list() -> Variable {
        Variable {
            key: ptr::null(),
            value: ptr::null(),
        }
    }

    #[test]
    fn set_variables_records_what_the_core_declared() {
        let mut host = host_with(HashMap::new(), false);
        let (a, _ak, _av) = declaration("mgba_sgb_borders", "Use Super Game Boy Borders; ON|OFF");
        let (b, _bk, _bv) =
            declaration("mgba_gb_colors_preset", "Game Boy Palette Preset; 0|1|2|3");
        let list = [a, b, end_of_list()];
        let ok = {
            let _active = Active::bind(&mut host);
            unsafe { environment(SET_VARIABLES, list.as_ptr() as *mut c_void) }
        };

        assert!(ok);
        assert_eq!(
            host.declared.get("mgba_sgb_borders").map(Vec::as_slice),
            Some(["ON".to_string(), "OFF".to_string()].as_slice())
        );
        assert_eq!(
            host.declared
                .get("mgba_gb_colors_preset")
                .map(Vec::as_slice),
            Some(["0", "1", "2", "3"].map(str::to_string).as_slice())
        );
        assert_eq!(
            host.declared.len(),
            2,
            "the walk ran past the terminator and read whatever was after it"
        );
        assert!(
            !host.declared.contains_key("mgba_sgb_border"),
            "a key the core never declared came back declared"
        );
    }

    #[test]
    fn set_variables_with_no_list_declares_nothing() {
        let mut host = host_with(HashMap::new(), false);
        host.declared
            .insert("stale".to_string(), vec!["x".to_string()]);

        let ok = {
            let _active = Active::bind(&mut host);
            unsafe { environment(SET_VARIABLES, ptr::null_mut()) }
        };

        assert!(ok);
        assert!(host.declared.is_empty(), "a stale declaration survived");
    }

    #[test]
    fn get_variable_writes_the_options_pointer_for_the_core_to_read() {
        let mut options = HashMap::new();
        options.insert("gpsp_serial".to_string(), CString::new("rfu").unwrap());
        let mut host = host_with(options, false);
        let _active = Active::bind(&mut host);

        let key = CString::new("gpsp_serial").unwrap();
        let mut var = Variable {
            key: key.as_ptr(),
            value: ptr::null(),
        };
        let ok = unsafe { environment(GET_VARIABLE, &mut var as *mut Variable as *mut c_void) };

        assert!(ok);
        let value = unsafe { CStr::from_ptr(var.value) };
        assert_eq!(value.to_str().unwrap(), "rfu");
    }

    #[test]
    fn get_variable_reports_false_for_an_unknown_key() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);

        let key = CString::new("nope").unwrap();
        let mut var = Variable {
            key: key.as_ptr(),
            value: ptr::null(),
        };
        let ok = unsafe { environment(GET_VARIABLE, &mut var as *mut Variable as *mut c_void) };

        assert!(!ok);
        assert!(var.value.is_null());
    }

    #[test]
    fn get_variable_update_reports_and_clears_the_dirty_flag() {
        let mut host = host_with(HashMap::new(), true);
        let _active = Active::bind(&mut host);

        let mut dirty = false;
        let ok =
            unsafe { environment(GET_VARIABLE_UPDATE, &mut dirty as *mut bool as *mut c_void) };
        assert!(ok);
        assert!(dirty, "first call must report the pending change");

        let mut dirty_again = true;
        let ok = unsafe {
            environment(
                GET_VARIABLE_UPDATE,
                &mut dirty_again as *mut bool as *mut c_void,
            )
        };
        assert!(ok);
        assert!(!dirty_again, "flag must be cleared after being read once");
    }

    #[test]
    fn get_variable_refuses_a_null_key() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);

        let mut var = Variable {
            key: ptr::null(),
            value: ptr::null(),
        };
        let ok = unsafe { environment(GET_VARIABLE, &mut var as *mut Variable as *mut c_void) };
        assert!(!ok);
    }

    thread_local! {
        static TEST_AUDIO_STATUS: RefCell<Vec<(bool, c_uint, bool)>> =
            const { RefCell::new(Vec::new()) };
    }

    unsafe extern "C" fn test_audio_status(active: bool, occupancy: c_uint, underrun: bool) {
        TEST_AUDIO_STATUS.with(|r| r.borrow_mut().push((active, occupancy, underrun)));
    }

    #[test]
    fn set_audio_buffer_status_callback_stores_what_the_core_hands_over() {
        let mut host = host_with(HashMap::new(), false);
        let mut cb = AudioBufferStatusCallback {
            callback: Some(test_audio_status),
        };
        let ok = {
            let _active = Active::bind(&mut host);
            unsafe {
                environment(
                    SET_AUDIO_BUFFER_STATUS_CALLBACK,
                    &mut cb as *mut AudioBufferStatusCallback as *mut c_void,
                )
            }
        };

        assert!(ok, "refusing this turns the cores' frameskip off entirely");
        assert!(host.audio_status.is_some(), "the callback was never stored");
    }

    #[test]
    fn set_audio_buffer_status_callback_null_withdraws_it() {
        let mut host = host_with(HashMap::new(), false);
        host.audio_status = Some(test_audio_status);
        let ok = {
            let _active = Active::bind(&mut host);
            unsafe { environment(SET_AUDIO_BUFFER_STATUS_CALLBACK, ptr::null_mut()) }
        };

        assert!(ok, "withdrawing is a legal call and must be answered true");
        assert!(host.audio_status.is_none());
    }

    #[test]
    fn set_frame_skip_tells_the_core_to_skip_by_reporting_an_underrun() {
        TEST_AUDIO_STATUS.with(|r| r.borrow_mut().clear());
        let mut host = host_with(HashMap::new(), false);
        host.audio_status = Some(test_audio_status);
        {
            let _active = Active::bind(&mut host);
            unsafe {
                report_audio_status(true);
                report_audio_status(false);
            }
        }

        TEST_AUDIO_STATUS.with(|r| {
            assert_eq!(
                r.borrow().as_slice(),
                &[(true, 0, true), (true, 100, false)],
                "the skip and the draw did not read as an underrun and a healthy buffer"
            );
        });
    }

    #[test]
    fn set_frame_skip_is_a_noop_when_the_core_registered_no_callback() {
        TEST_AUDIO_STATUS.with(|r| r.borrow_mut().clear());
        let mut host = host_with(HashMap::new(), false);
        {
            let _active = Active::bind(&mut host);
            unsafe { report_audio_status(true) };
        }

        TEST_AUDIO_STATUS.with(|r| assert!(r.borrow().is_empty()));
    }

    thread_local! {
        static TEST_RECEIVED: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
        static TEST_RECEIVED_CLIENT_IDS: RefCell<Vec<u16>> = const { RefCell::new(Vec::new()) };
        static TEST_POLLS: Cell<u32> = const { Cell::new(0) };
        static TEST_START_CLIENT: Cell<Option<u16>> = const { Cell::new(None) };
        static TEST_STOP_CALLS: Cell<u32> = const { Cell::new(0) };
        static TEST_CONNECTED_CLIENT: Cell<Option<u16>> = const { Cell::new(None) };
        static TEST_DISCONNECTED_CLIENT: Cell<Option<u16>> = const { Cell::new(None) };
    }

    fn reset_test_netpacket_recorders() {
        TEST_RECEIVED.with(|r| r.borrow_mut().clear());
        TEST_RECEIVED_CLIENT_IDS.with(|c| c.borrow_mut().clear());
        TEST_POLLS.with(|p| p.set(0));
        TEST_START_CLIENT.with(|c| c.set(None));
        TEST_STOP_CALLS.with(|c| c.set(0));
        TEST_CONNECTED_CLIENT.with(|c| c.set(None));
        TEST_DISCONNECTED_CLIENT.with(|c| c.set(None));
    }

    unsafe extern "C" fn test_receive(buf: *const c_void, len: usize, client_id: u16) {
        let bytes = std::slice::from_raw_parts(buf as *const u8, len).to_vec();
        TEST_RECEIVED.with(|r| r.borrow_mut().push(bytes));
        TEST_RECEIVED_CLIENT_IDS.with(|c| c.borrow_mut().push(client_id));
    }

    unsafe extern "C" fn test_receive_reentrant(buf: *const c_void, len: usize, client_id: u16) {
        test_receive(buf, len, client_id);
        let reentrant = b"reentrant";
        netpacket_send(0, reentrant.as_ptr() as *const c_void, reentrant.len(), 0);
    }

    unsafe extern "C" fn test_start_reentrant(
        client_id: u16,
        _send: NetpacketSend,
        poll_receive: NetpacketPollReceive,
    ) {
        TEST_START_CLIENT.with(|c| c.set(Some(client_id)));
        poll_receive();
    }

    unsafe extern "C" fn test_poll() {
        TEST_POLLS.with(|p| p.set(p.get() + 1));
    }

    unsafe extern "C" fn test_start(
        client_id: u16,
        _send: NetpacketSend,
        _poll_receive: NetpacketPollReceive,
    ) {
        TEST_START_CLIENT.with(|c| c.set(Some(client_id)));
    }

    unsafe extern "C" fn test_stop() {
        TEST_STOP_CALLS.with(|c| c.set(c.get() + 1));
    }

    unsafe extern "C" fn test_connected(client_id: u16) -> bool {
        TEST_CONNECTED_CLIENT.with(|c| c.set(Some(client_id)));
        true
    }

    unsafe extern "C" fn test_disconnected(client_id: u16) {
        TEST_DISCONNECTED_CLIENT.with(|c| c.set(Some(client_id)));
    }

    fn test_netpacket_callback() -> NetpacketCallback {
        NetpacketCallback {
            start: Some(test_start),
            receive: Some(test_receive),
            stop: None,
            poll: Some(test_poll),
            connected: None,
            disconnected: None,
            protocol_version: ptr::null(),
        }
    }

    #[test]
    fn set_netpacket_interface_stores_the_callback_the_core_hands_over() {
        let mut host = host_with(HashMap::new(), false);
        let mut cb = test_netpacket_callback();
        let ok = {
            let _active = Active::bind(&mut host);
            unsafe {
                environment(
                    SET_NETPACKET_INTERFACE,
                    &mut cb as *mut NetpacketCallback as *mut c_void,
                )
            }
        };

        assert!(ok);
        assert!(host.netpacket.is_some(), "the callback was never stored");
    }

    #[test]
    fn set_netpacket_interface_null_data_withdraws_the_interface() {
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        let ok = {
            let _active = Active::bind(&mut host);
            unsafe { environment(SET_NETPACKET_INTERFACE, ptr::null_mut()) }
        };

        assert!(ok, "withdrawing is a legal call and must be answered true");
        assert!(
            host.netpacket.is_none(),
            "a NULL data pointer must clear a previously registered callback"
        );
    }

    #[test]
    fn netpacket_send_pushes_the_cores_packet_onto_outbound() {
        let mut host = host_with(HashMap::new(), false);
        let packet = b"link cable byte";
        {
            let _active = Active::bind(&mut host);
            unsafe {
                netpacket_send(
                    NETPACKET_RELIABLE,
                    packet.as_ptr() as *const c_void,
                    packet.len(),
                    0,
                )
            };
        }

        assert_eq!(host.net.take_outbound().as_deref(), Some(&packet[..]));
        assert_eq!(host.net.take_outbound(), None, "only one packet was sent");
    }

    #[test]
    fn netpacket_send_ignores_a_null_or_empty_packet() {
        let mut host = host_with(HashMap::new(), false);
        {
            let _active = Active::bind(&mut host);
            unsafe { netpacket_send(0, ptr::null(), 4, 0) };
            unsafe { netpacket_send(0, [1u8].as_ptr() as *const c_void, 0, 0) };
        }

        assert_eq!(
            host.net.take_outbound(),
            None,
            "a null buffer or zero length must not enqueue a phantom packet"
        );
    }

    #[test]
    fn netpacket_poll_receive_drains_inbound_into_the_cores_receive_in_order() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        host.net.set_active(true);
        host.net_peer = Some(1);
        host.net.push_inbound(b"first".to_vec());
        host.net.push_inbound(b"second".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { netpacket_poll_receive() };
        }

        TEST_RECEIVED.with(|r| {
            assert_eq!(
                r.borrow().as_slice(),
                &[b"first".to_vec(), b"second".to_vec()],
                "order was not kept"
            );
        });
        assert_eq!(host.net.take_inbound(), None, "queue must be drained");
    }

    #[test]
    fn netpacket_poll_receive_does_nothing_without_a_receive_callback() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.net.set_active(true);
        host.net.push_inbound(b"stranded".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { netpacket_poll_receive() };
        }

        TEST_RECEIVED.with(|r| assert!(r.borrow().is_empty()));
        assert_eq!(
            host.net.take_inbound().as_deref(),
            Some(&b"stranded"[..]),
            "with nowhere to hand the packet it must be left queued, not dropped"
        );
    }

    #[test]
    fn netpacket_poll_receive_does_nothing_once_the_session_is_no_longer_active() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        host.net.push_inbound(b"stale".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { netpacket_poll_receive() };
        }

        TEST_RECEIVED.with(|r| assert!(r.borrow().is_empty(), "an ended session reached the core"));
        assert_eq!(
            host.net.take_inbound().as_deref(),
            Some(&b"stale"[..]),
            "the packet must be left queued, not delivered to a session that already ended"
        );
    }

    #[test]
    fn netpacket_poll_receive_does_nothing_without_a_recorded_peer() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        host.net.set_active(true);
        host.net.push_inbound(b"orphaned".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { netpacket_poll_receive() };
        }

        TEST_RECEIVED.with(|r| {
            assert!(
                r.borrow().is_empty(),
                "a packet must not be delivered with no peer to tag it"
            )
        });
        assert_eq!(
            host.net.take_inbound().as_deref(),
            Some(&b"orphaned"[..]),
            "the packet must be left queued, not delivered mislabelled as our own id"
        );
    }

    #[test]
    fn netpacket_poll_receive_tags_packets_with_the_peers_client_id() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(0) };
        }
        reset_test_netpacket_recorders();
        host.net.push_inbound(b"from the peer".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { netpacket_poll_receive() };
        }

        TEST_RECEIVED_CLIENT_IDS.with(|c| {
            assert_eq!(
                c.borrow().as_slice(),
                &[1],
                "packets must be tagged with the peer's id, not the host's own"
            );
        });
    }

    #[test]
    fn drain_link_hands_the_core_everything_waiting_then_polls() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        host.net.set_active(true);
        host.net_peer = Some(1);
        host.net.push_inbound(b"queued".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_RECEIVED.with(|r| assert_eq!(r.borrow().as_slice(), &[b"queued".to_vec()]));
        TEST_POLLS.with(|p| assert_eq!(p.get(), 1, "poll must run once a frame"));
    }

    #[test]
    fn drain_link_polls_even_with_nothing_inbound() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        host.net.set_active(true);
        host.net_peer = Some(1);
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_POLLS.with(|p| assert_eq!(p.get(), 1));
    }

    #[test]
    fn drain_link_skips_poll_when_the_core_offered_none() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            poll: None,
            ..test_netpacket_callback()
        });
        host.net.set_active(true);
        host.net_peer = Some(1);
        host.net.push_inbound(b"still delivered".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_RECEIVED.with(|r| {
            assert_eq!(r.borrow().as_slice(), &[b"still delivered".to_vec()]);
        });
        TEST_POLLS.with(|p| assert_eq!(p.get(), 0, "poll is optional and was not offered"));
    }

    #[test]
    fn drain_link_is_a_noop_when_the_core_never_registered_netpacket() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.net.set_active(true);
        host.net.push_inbound(b"nobody asked".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_RECEIVED.with(|r| assert!(r.borrow().is_empty()));
        TEST_POLLS.with(|p| assert_eq!(p.get(), 0));
        assert_eq!(
            host.net.take_inbound().as_deref(),
            Some(&b"nobody asked"[..]),
            "with no registered core the packet must be left queued, not lost"
        );
    }

    #[test]
    fn drain_link_does_nothing_once_the_session_is_no_longer_active() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        host.net.push_inbound(b"stale".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_RECEIVED.with(|r| assert!(r.borrow().is_empty(), "an ended session reached the core"));
        TEST_POLLS.with(|p| assert_eq!(p.get(), 0, "an ended session must not be polled either"));
        assert_eq!(
            host.net.take_inbound().as_deref(),
            Some(&b"stale"[..]),
            "the packet must be left queued, not delivered to a session that already ended"
        );
    }

    #[test]
    fn drain_link_does_nothing_without_a_recorded_peer() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        host.net.set_active(true);
        host.net.push_inbound(b"orphaned".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_RECEIVED.with(|r| {
            assert!(
                r.borrow().is_empty(),
                "a packet must not be delivered with no peer to tag it"
            )
        });
        TEST_POLLS.with(|p| {
            assert_eq!(
                p.get(),
                0,
                "must not even poll with no peer to tag a receive"
            )
        });
        assert_eq!(
            host.net.take_inbound().as_deref(),
            Some(&b"orphaned"[..]),
            "the packet must be left queued, not delivered mislabelled as our own id"
        );
    }

    #[test]
    fn drain_link_tags_packets_with_the_peers_client_id() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(0) };
        }
        reset_test_netpacket_recorders();
        host.net.push_inbound(b"from the peer".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_RECEIVED_CLIENT_IDS.with(|c| {
            assert_eq!(
                c.borrow().as_slice(),
                &[1],
                "packets must be tagged with the peer's id, not our own"
            );
        });
    }

    #[test]
    fn drain_link_survives_the_reentrancy_gpsp_documents() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            receive: Some(test_receive_reentrant),
            ..test_netpacket_callback()
        });
        host.net.set_active(true);
        host.net_peer = Some(1);
        host.net.push_inbound(b"queued".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_RECEIVED.with(|r| assert_eq!(r.borrow().as_slice(), &[b"queued".to_vec()]));
        assert_eq!(
            host.net.take_outbound().as_deref(),
            Some(&b"reentrant"[..]),
            "the reentrant netpacket_send call must still reach outbound"
        );
    }

    #[test]
    fn netpacket_poll_receive_survives_the_reentrancy_gpsp_documents() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            receive: Some(test_receive_reentrant),
            ..test_netpacket_callback()
        });
        host.net.set_active(true);
        host.net_peer = Some(1);
        host.net.push_inbound(b"queued".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { netpacket_poll_receive() };
        }

        assert_eq!(host.net.take_outbound().as_deref(), Some(&b"reentrant"[..]));
    }

    #[test]
    fn begin_link_survives_a_core_that_reenters_from_start() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            start: Some(test_start_reentrant),
            ..test_netpacket_callback()
        });
        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(0) };
        }

        TEST_START_CLIENT.with(|c| assert_eq!(c.get(), Some(0)));
        assert!(host.net.is_active());
    }

    #[test]
    fn begin_link_hands_the_core_its_client_id_and_marks_the_session_active() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        assert!(
            !host.net.is_active(),
            "a fresh link is not backing a session"
        );
        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(1) };
        }

        TEST_START_CLIENT.with(|c| assert_eq!(c.get(), Some(1), "client id was not forwarded"));
        assert!(
            host.net.is_active(),
            "starting a session must mark the link active"
        );
    }

    #[test]
    fn begin_link_is_a_noop_when_the_core_never_registered_netpacket() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(0) };
        }

        TEST_START_CLIENT.with(|c| assert_eq!(c.get(), None, "nothing to start, nothing called"));
        assert!(!host.net.is_active());
    }

    #[test]
    fn begin_link_calls_connected_with_the_peers_client_id() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            connected: Some(test_connected),
            ..test_netpacket_callback()
        });
        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(0) };
        }

        TEST_CONNECTED_CLIENT.with(|c| {
            assert_eq!(
                c.get(),
                Some(1),
                "connected was not called with the peer's id"
            )
        });
    }

    #[test]
    fn begin_link_is_fine_with_no_connected_callback() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());

        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(0) };
        }

        assert!(
            host.net.is_active(),
            "a missing connected callback must not stop the session from starting"
        );
    }

    #[test]
    fn halt_link_calls_the_cores_stop_when_it_registered_one() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            stop: Some(test_stop),
            ..test_netpacket_callback()
        });
        {
            let _active = Active::bind(&mut host);
            unsafe { halt_link() };
        }

        TEST_STOP_CALLS.with(|c| assert_eq!(c.get(), 1, "stop was never called"));
    }

    #[test]
    fn halt_link_is_a_noop_when_the_core_never_offered_a_stop() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());

        {
            let _active = Active::bind(&mut host);
            unsafe { halt_link() };
        }

        TEST_STOP_CALLS.with(|c| assert_eq!(c.get(), 0));
    }

    #[test]
    fn halt_link_calls_disconnected_with_the_peers_client_id() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            disconnected: Some(test_disconnected),
            ..test_netpacket_callback()
        });
        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(1) };
        }
        {
            let _active = Active::bind(&mut host);
            unsafe { halt_link() };
        }

        TEST_DISCONNECTED_CLIENT.with(|c| {
            assert_eq!(
                c.get(),
                Some(0),
                "disconnected was not called with the peer's id"
            )
        });
    }

    #[test]
    fn halt_link_does_not_call_disconnected_when_no_session_ever_started() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            disconnected: Some(test_disconnected),
            ..test_netpacket_callback()
        });

        {
            let _active = Active::bind(&mut host);
            unsafe { halt_link() };
        }

        TEST_DISCONNECTED_CLIENT.with(|c| assert_eq!(c.get(), None, "nothing was ever connected"));
    }

    /// The stick-read counters are process-wide, and tests run in parallel: the ones
    /// that read sticks take turns.
    static STICK_COUNTERS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn sticks_turn() -> std::sync::MutexGuard<'static, ()> {
        STICK_COUNTERS.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn input_state_answers_each_port_from_its_own_mask() {
        let mut host = host_with(HashMap::new(), false);
        host.inputs = [ButtonMask::A, ButtonMask::B | ButtonMask::START];
        let _active = Active::bind(&mut host);

        let mask = |port| unsafe { input_state(port, DEVICE_JOYPAD, 0, JOYPAD_MASK) } as u16;
        assert_eq!(mask(0), ButtonMask::A);
        assert_eq!(mask(1), ButtonMask::B | ButtonMask::START);
    }

    #[test]
    fn input_state_answers_single_buttons_on_port_1() {
        let mut host = host_with(HashMap::new(), false);
        host.inputs = [0, ButtonMask::B];
        let _active = Active::bind(&mut host);

        let button = |id| unsafe { input_state(1, DEVICE_JOYPAD, 0, id) };
        assert_eq!(button(0), 1, "B is bit 0 of the libretro joypad");
        assert_eq!(button(8), 0, "A is not held on port 1");
    }

    #[test]
    fn input_state_answers_each_stick_axis_on_port_1() {
        let _turn = sticks_turn();
        let mut host = host_with(HashMap::new(), false);
        host.analog = [100, -200, 300, -400];
        let _active = Active::bind(&mut host);

        let axis = |port, index, id| unsafe { input_state(port, DEVICE_ANALOG, index, id) };
        assert_eq!(axis(0, ANALOG_LEFT, ANALOG_X), 100);
        assert_eq!(axis(0, ANALOG_LEFT, ANALOG_Y), -200);
        assert_eq!(axis(0, ANALOG_RIGHT, ANALOG_X), 300);
        assert_eq!(axis(0, ANALOG_RIGHT, ANALOG_Y), -400);
    }

    #[test]
    fn the_cores_stick_reads_are_counted_with_their_furthest_values() {
        let _turn = sticks_turn();
        let mut host = host_with(HashMap::new(), false);
        host.analog = [100, -20_000, 300, -400];
        let _active = Active::bind(&mut host);
        let _ = analog_stats();
        for _ in 0..3 {
            unsafe { input_state(0, DEVICE_ANALOG, ANALOG_LEFT, ANALOG_Y) };
        }
        unsafe { input_state(0, DEVICE_ANALOG, ANALOG_LEFT, ANALOG_X) };
        let (reads, peaks) = analog_stats();
        assert_eq!(reads, 4);
        assert_eq!(peaks, [100, -20_000, 0, 0]);
        // Reading resets it, and a button read is not a stick read.
        unsafe { input_state(0, DEVICE_JOYPAD, 0, JOYPAD_MASK) };
        assert_eq!(analog_stats(), (0, [0; 4]));
    }

    #[test]
    fn input_state_has_no_sticks_past_port_1_or_past_the_two_sticks() {
        let _turn = sticks_turn();
        let mut host = host_with(HashMap::new(), false);
        host.analog = [1, 2, 3, 4];
        let _active = Active::bind(&mut host);

        assert_eq!(
            unsafe { input_state(1, DEVICE_ANALOG, ANALOG_LEFT, ANALOG_X) },
            0
        );
        assert_eq!(
            unsafe { input_state(0, DEVICE_ANALOG, 2, ANALOG_X) },
            0,
            "no third stick"
        );
        assert_eq!(
            unsafe { input_state(0, DEVICE_ANALOG, ANALOG_LEFT, 2) },
            0,
            "no z axis"
        );
    }

    #[test]
    fn the_dualshock_is_the_playstation_cores_own_device_number() {
        // PCSX-ReARMed defines it as RETRO_DEVICE_SUBCLASS(RETRO_DEVICE_ANALOG, 1).
        assert_eq!(DEVICE_PSE_DUALSHOCK, 517);
        assert_eq!(DEVICE_PSE_DUALSHOCK & 0xff, DEVICE_ANALOG);
    }

    fn native_host() -> Box<Host> {
        let mut host = host_with(HashMap::new(), false);
        host.native = true;
        host
    }

    /// A frame where every pixel differs from its neighbours, so a crop, a
    /// shift or a wrong stride changes the bytes.
    fn numbered(w: usize, h: usize) -> Vec<u8> {
        (0..(w * h) as u32).flat_map(|i| i.to_le_bytes()).collect()
    }

    #[test]
    fn a_core_warning_is_shown_once_and_its_chatter_is_not_shown_at_all() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        let warn = CString::new("CD image is not usable: %s\n").unwrap();
        let chatter = CString::new("loading %s\n").unwrap();
        for _ in 0..40 {
            unsafe { log_forward(2, warn.as_ptr()) };
            unsafe { log_forward(1, chatter.as_ptr()) };
        }
        unsafe { log_forward(3, ptr::null()) };
        let shown = unsafe { with_host(|h| h.logged.iter().cloned().collect::<Vec<_>>()) }.unwrap();
        assert_eq!(shown, ["CD image is not usable: %s"]);
    }

    #[test]
    fn a_command_the_core_keeps_asking_for_is_reported_once() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        for _ in 0..60 {
            assert!(!unsafe { environment(65576, ptr::null_mut()) });
        }
        assert!(!unsafe { environment(63, ptr::null_mut()) });
        let seen = unsafe { with_host(|h| h.unhandled.len()) }.unwrap();
        assert_eq!(seen, 2, "each distinct command is remembered once");
    }

    #[test]
    fn a_native_picture_keeps_its_size_and_is_not_cropped() {
        let mut host = native_host();
        let _active = Active::bind(&mut host);
        let frame = numbered(320, 240);
        unsafe { video_refresh(frame.as_ptr() as *const c_void, 320, 240, 320 * 4) };

        let (size, video) = unsafe { with_host(|h| (h.video_size, h.video.clone())) }.unwrap();
        assert_eq!(size, (320, 240));
        assert_eq!(video, frame, "every pixel arrives where the core drew it");
    }

    #[test]
    fn the_buffer_follows_the_core_when_it_changes_resolution() {
        let mut host = native_host();
        let _active = Active::bind(&mut host);
        for (w, h) in [(320usize, 240usize), (640, 480), (256, 240)] {
            let frame = numbered(w, h);
            unsafe { video_refresh(frame.as_ptr() as *const c_void, w as _, h as _, w * 4) };
            let (size, video) = unsafe { with_host(|h| (h.video_size, h.video.clone())) }.unwrap();
            assert_eq!(size, (w as u32, h as u32));
            assert_eq!(video, frame, "{w}x{h}");
        }
    }

    #[test]
    fn a_padded_pitch_is_skipped_not_drawn() {
        let mut host = native_host();
        let _active = Active::bind(&mut host);
        let (w, h, pitch) = (64usize, 8usize, 96 * 4);
        let mut padded = vec![0xEEu8; pitch * h];
        let expect = numbered(w, h);
        for y in 0..h {
            padded[y * pitch..y * pitch + w * 4].copy_from_slice(&expect[y * w * 4..][..w * 4]);
        }
        unsafe { video_refresh(padded.as_ptr() as *const c_void, w as _, h as _, pitch) };

        let video = unsafe { with_host(|h| h.video.clone()) }.unwrap();
        assert_eq!(
            video, expect,
            "the padding at the end of a row is not picture"
        );
    }

    #[test]
    fn a_native_565_picture_is_widened_to_xrgb() {
        let mut host = native_host();
        host.format = PixelFormat::Rgb565;
        let _active = Active::bind(&mut host);
        // red, green, blue, white
        let frame: Vec<u8> = [0xF800u16, 0x07E0, 0x001F, 0xFFFF]
            .iter()
            .flat_map(|p| p.to_le_bytes())
            .collect();
        unsafe { video_refresh(frame.as_ptr() as *const c_void, 2, 2, 2 * 2) };

        let video = unsafe { with_host(|h| h.video.clone()) }.unwrap();
        assert_eq!(video.len(), 2 * 2 * 4);
        // Stored B, G, R, unused.
        assert_eq!(&video[0..4], &[0, 0, 255, 0], "red");
        assert_eq!(&video[4..8], &[0, 255, 0, 0], "green");
        assert_eq!(&video[8..12], &[255, 0, 0, 0], "blue");
        assert_eq!(&video[12..16], &[255, 255, 255, 0], "white");
    }

    #[test]
    fn a_nonsense_size_is_capped_rather_than_allocated() {
        let mut host = native_host();
        let _active = Active::bind(&mut host);
        let (w, h) = (MAX_NATIVE as usize + 76, MAX_NATIVE as usize + 76);
        let frame = vec![0x55u8; w * h * 4];
        unsafe { video_refresh(frame.as_ptr() as *const c_void, w as _, h as _, w * 4) };

        let (size, len) = unsafe { with_host(|h| (h.video_size, h.video.len())) }.unwrap();
        assert_eq!(size, (MAX_NATIVE, MAX_NATIVE));
        assert_eq!(len, (MAX_NATIVE * MAX_NATIVE * 4) as usize);
    }

    #[test]
    fn an_empty_native_frame_changes_nothing() {
        let mut host = native_host();
        let _active = Active::bind(&mut host);
        let frame = numbered(32, 32);
        unsafe {
            video_refresh(frame.as_ptr() as *const c_void, 32, 32, 32 * 4);
            video_refresh(frame.as_ptr() as *const c_void, 0, 0, 0);
        }
        let (size, video) = unsafe { with_host(|h| (h.video_size, h.video.clone())) }.unwrap();
        assert_eq!(size, (32, 32));
        assert_eq!(video, frame);
    }

    #[test]
    fn without_native_video_a_big_picture_is_still_cropped_to_the_canvas() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        let frame = numbered(320, 240);
        unsafe { video_refresh(frame.as_ptr() as *const c_void, 320, 240, 320 * 4) };

        let (size, len) = unsafe { with_host(|h| (h.video_size, h.video.len())) }.unwrap();
        assert_eq!(
            size,
            (GBA_W, GBA_H),
            "the Game Boy family path must not change"
        );
        assert_eq!(len, VIDEO_BYTES);
    }

    #[test]
    fn input_state_answers_the_second_shoulders_and_stick_clicks() {
        let mut host = host_with(HashMap::new(), false);
        host.inputs = [
            ButtonMask::L2 | ButtonMask::R3,
            ButtonMask::R2 | ButtonMask::L3,
        ];
        let _active = Active::bind(&mut host);

        let button = |port, id| unsafe { input_state(port, DEVICE_JOYPAD, 0, id) };
        // libretro joypad ids 12..15 are L2, R2, L3, R3.
        assert_eq!((button(0, 12), button(0, 13)), (1, 0), "L2 held, R2 not");
        assert_eq!((button(0, 14), button(0, 15)), (0, 1), "R3 held, L3 not");
        assert_eq!((button(1, 12), button(1, 13)), (0, 1), "R2 on port 1");
        assert_eq!((button(1, 14), button(1, 15)), (1, 0), "L3 on port 1");
    }

    #[test]
    fn a_core_that_needs_a_path_is_not_handed_the_file_in_memory() {
        let dir = std::env::temp_dir().join(format!("slot-content-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = dir.join("disc.bin");
        std::fs::write(&image, vec![7u8; 4096]).unwrap();

        assert!(
            content_bytes(&image, true).unwrap().is_empty(),
            "a path-only core must not have the image read into RAM"
        );
        assert_eq!(content_bytes(&image, false).unwrap().len(), 4096);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_file_is_still_reported_for_a_path_only_core() {
        let missing = std::env::temp_dir().join("slot-content-does-not-exist.cue");
        assert!(content_bytes(&missing, true).is_err());
        assert!(content_bytes(&missing, false).is_err());
    }

    #[test]
    fn input_state_answers_nothing_past_port_1() {
        let mut host = host_with(HashMap::new(), false);
        host.inputs = [u16::MAX, u16::MAX];
        let _active = Active::bind(&mut host);

        assert_eq!(unsafe { input_state(2, DEVICE_JOYPAD, 0, JOYPAD_MASK) }, 0);
    }

    #[test]
    fn a_small_picture_is_centred_in_the_buffer() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        let frame = vec![0xffu8; 160 * 144 * 4];
        unsafe { video_refresh(frame.as_ptr() as *const c_void, 160, 144, 160 * 4) };

        let lit =
            |x: usize, y: usize| unsafe { with_host(|h| h.video[(y * 240 + x) * 4]) }.unwrap() != 0;
        assert!(lit(40, 8), "the top left of the picture is not at (40, 8)");
        assert!(lit(199, 151), "the bottom right is not at (199, 151)");
        assert!(!lit(39, 8), "the picture starts one column too early");
        assert!(!lit(40, 7), "the picture starts one row too early");
    }

    #[test]
    fn the_margin_is_cleared_when_the_picture_shrinks() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        unsafe {
            let full = vec![0xffu8; 240 * 160 * 4];
            video_refresh(full.as_ptr() as *const c_void, 240, 160, 240 * 4);
            let small = vec![0x11u8; 160 * 144 * 4];
            video_refresh(small.as_ptr() as *const c_void, 160, 144, 160 * 4);
        }
        let corner = unsafe { with_host(|h| h.video[0]) }.unwrap();
        assert_eq!(corner, 0, "the previous picture is still in the margin");
    }

    fn gradient(w: usize, h: usize) -> Vec<u8> {
        let mut px = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let o = (y * w + x) * 4;
                px[o] = x as u8;
                px[o + 1] = y as u8;
                px[o + 2] = (x ^ y) as u8;
            }
        }
        px
    }

    #[test]
    fn a_full_size_picture_is_unmoved() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        let frame = gradient(GBA_W as usize, GBA_H as usize);
        unsafe {
            video_refresh(
                frame.as_ptr() as *const c_void,
                GBA_W,
                GBA_H,
                GBA_W as usize * 4,
            )
        };

        assert_eq!(unsafe { with_host(|h| h.video.clone()) }.unwrap(), frame);
    }

    #[test]
    fn get_can_dupe_tells_a_core_the_frontend_keeps_the_last_frame() {
        assert_eq!(GET_CAN_DUPE, 3);

        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);

        let mut can_dupe = false;
        let ok = unsafe { environment(GET_CAN_DUPE, &mut can_dupe as *mut bool as *mut c_void) };

        assert!(ok, "a core that asks has to be answered, not ignored");
        assert!(
            can_dupe,
            "the frontend does keep the last frame, so it must say so"
        );
    }

    #[test]
    fn get_can_dupe_refuses_a_null_pointer() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);

        assert!(!unsafe { environment(GET_CAN_DUPE, ptr::null_mut()) });
    }

    #[test]
    fn a_duplicate_frame_leaves_the_previous_picture_exactly_as_it_was() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        let frame = gradient(GBA_W as usize, GBA_H as usize);
        unsafe {
            video_refresh(
                frame.as_ptr() as *const c_void,
                GBA_W,
                GBA_H,
                GBA_W as usize * 4,
            );
            video_refresh(ptr::null(), GBA_W, GBA_H, GBA_W as usize * 4);
        }

        assert_eq!(
            unsafe { with_host(|h| h.video.clone()) }.unwrap(),
            frame,
            "a duplicate frame changed the picture instead of keeping it"
        );
    }

    #[test]
    fn a_duplicate_frame_keeps_a_centred_game_boy_picture_where_it_is() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        let frame = gradient(160, 144);
        unsafe { video_refresh(frame.as_ptr() as *const c_void, 160, 144, 160 * 4) };
        let drawn = unsafe { with_host(|h| h.video.clone()) }.unwrap();

        unsafe {
            video_refresh(ptr::null(), 160, 144, 160 * 4);
            video_refresh(ptr::null(), 160, 144, 160 * 4);
        }

        let kept = unsafe { with_host(|h| h.video.clone()) }.unwrap();
        assert_eq!(kept, drawn, "the picture did not survive two duplicates");
        let lit = (58 * GBA_W as usize + 140) * 4;
        assert_eq!(
            &kept[lit..lit + 3],
            &[100u8, 50, 100 ^ 50],
            "the picture was blanked or moved where a duplicate should have kept it"
        );
    }
}
