//! Sound on the Miyoo Mini Plus, through the SigmaStar vendor interface.
//!
//! The Mini Plus has an ALSA card (`MI Alsa`), but nothing can open it: a
//! tester's log showed `hw:0,0` busy with no process holding `/dev/snd`, so the
//! device is owned a level below ALSA. Everything that plays sound on this
//! console does it through `libmi_ao`, the vendor library: set the device's
//! attributes, enable it and its channel, and send frames.
//!
//! The firmware's own `audioserver` owns that device from boot, and while it is
//! running the library answers `SetPubAttr` with `0xa0052009`, "operation not
//! allowed" — it will not configure a device somebody else is playing through.
//! A frontend that needs the output itself stops the server first, which is what
//! Onion does for the same reason, and then has to put the volume back: the
//! device comes back at the codec's default and the player hears silence from a
//! working output. The server is started again when slot exits, or the console's
//! own sounds would stay off until the next reboot.
//!
//! The structures below are this project's own description of the layout the
//! library expects (SigmaStar's headers are confidential and are not copied
//! here). They were checked against the SDK's public documentation and against
//! the community SDL ports that ship working drivers. Output buffers get slack
//! beyond what is read, so a library with a longer structure than this one
//! describes cannot write past them.

use std::ffi::{c_int, c_uint, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use libloading::Library;

use super::pacer::Pacer;
use super::ring::Ring;
use super::silence::Silence;
use super::sink::{AudioError, AudioSink};

const DEVICE: c_int = 0;
const CHANNEL: c_int = 0;

/// The one rate this backend plays at, which is also what `audioserver` runs
/// the device at, so adopting its configuration never changes the rate.
const RATE: u32 = 48_000;
const CHANNELS: usize = 2;
const BYTES_PER_FRAME: usize = CHANNELS * 2;

/// Frames per chunk when this backend sets the device up itself, and the buffer
/// depth it asks for. A configured device keeps its own chunk size.
const OWN_CHUNK: u32 = 512;
const OWN_BUFFERS: u32 = 6;
const MIN_CHUNK: u32 = 128;
const MAX_CHUNK: u32 = 4096;

/// How long a send may wait for room before it is treated as having failed.
const SEND_TIMEOUT_MS: c_int = 100;

/// Audio the thread lets get ahead of real time before it sleeps instead of
/// sending more. The device holds about 64 ms; this keeps the thread well inside
/// that, with room for it to be scheduled late, and still cuts the delay a game
/// would otherwise sit behind.
const QUEUE_LIMIT: Duration = Duration::from_millis(48);

/// If the thread falls this far behind real time, the clock starts again from
/// now rather than trying to catch up.
const RESYNC: Duration = Duration::from_millis(100);

/// A nicer scheduling priority for the audio thread: the emulator and the
/// renderer share two cores with it, and a late wake-up is a click.
const AUDIO_NICE: c_int = -10;

const RELEASE_AFTER: Duration = Duration::from_secs(3);
const RETRY_AFTER: Duration = Duration::from_secs(1);

/// How many start-up attempts log every step. A device that keeps refusing
/// would otherwise print several lines a second for as long as a game runs.
const LOUD_ATTEMPTS: u32 = 3;

const LIBRARIES: [&str; 3] = [
    "/config/lib/libmi_ao.so",
    "/customer/lib/libmi_ao.so",
    "libmi_ao.so",
];

/// The firmware's audio server, which owns AO from boot. MinUI ships a modified
/// copy under the second name. Nothing on the console mixes two programs on this
/// device, so taking it means stopping whichever is running.
const SERVERS: [&str; 2] = ["audioserver", "audioserver.mod"];

/// The AO debug node. Reading it says what the device is doing, including the
/// gain it is at; writing to it sets the gain and the mute, which is how Onion
/// puts the volume back after stopping the server.
const PROCFS: &str = "/proc/mi_modules/mi_ao/mi_ao0";

/// How much of the debug node to log. The part that says whether the device is
/// enabled and who holds it comes after the common block, which the first
/// sixteen lines stopped short of.
const DUMP_LINES: usize = 120;

/// The gain to take the device at when the debug node cannot be read: MinUI's own
/// loudest setting, so a guess is never louder than the console already goes.
const FALLBACK_DB: i32 = -30;

/// The gain range the AO channel accepts.
const MIN_DB: i32 = -60;
const MAX_DB: i32 = 30;

/// How long to give a killed server to let go of the device before each retry.
const TAKE_WAITS_MS: [u64; 5] = [0, 50, 100, 200, 400];

const SAMPLE_RATE_48000: c_int = 48_000;
const BIT_WIDTH_16: c_int = 0;
const MODE_I2S_MASTER: c_int = 0;
const SOUND_STEREO: c_int = 1;
const MODULE_AO: c_int = 5;

const MAX_CHANNELS: usize = 16;
const SLACK: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct I2sConfig {
    format: c_int,
    master_clock: c_int,
    sync_clock: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Attr {
    sample_rate: c_int,
    bit_width: c_int,
    work_mode: c_int,
    sound_mode: c_int,
    frame_count: c_uint,
    points_per_frame: c_uint,
    codec_channels: c_uint,
    channels: c_uint,
    i2s: I2sConfig,
}

/// An `Attr` the library may fill in, with room to spare.
#[repr(C)]
struct AttrOut {
    attr: Attr,
    _slack: [c_uint; SLACK],
}

impl AttrOut {
    fn new() -> Self {
        AttrOut {
            attr: Attr::default(),
            _slack: [0; SLACK],
        }
    }
}

#[repr(C)]
struct Frame {
    bit_width: c_int,
    sound_mode: c_int,
    addr: [*mut c_void; MAX_CHANNELS],
    time_stamp: u64,
    sequence: c_uint,
    len: c_uint,
    pool: [c_uint; 2],
    source_addr: [*mut c_void; MAX_CHANNELS],
    source_len: c_uint,
}

#[repr(C)]
#[derive(Default)]
struct ChnState {
    total: c_uint,
    free: c_uint,
    busy: c_uint,
}

#[repr(C)]
struct ChnStateOut {
    state: ChnState,
    _slack: [c_uint; SLACK],
}

impl ChnStateOut {
    fn new() -> Self {
        ChnStateOut {
            state: ChnState::default(),
            _slack: [0; SLACK],
        }
    }
}

#[repr(C)]
struct ChnPort {
    module: c_int,
    device: c_uint,
    channel: c_uint,
    port: c_uint,
}

type AttrFn = unsafe extern "C" fn(c_int, *mut Attr) -> c_int;
type DeviceFn = unsafe extern "C" fn(c_int) -> c_int;
type ChannelFn = unsafe extern "C" fn(c_int, c_int) -> c_int;
type SendFn = unsafe extern "C" fn(c_int, c_int, *mut Frame, c_int) -> c_int;
type QueryFn = unsafe extern "C" fn(c_int, c_int, *mut ChnState) -> c_int;
type DepthFn = unsafe extern "C" fn(*mut ChnPort, c_uint, c_uint) -> c_int;

/// The calls this backend makes, as a table so a test can stand in for the
/// library and drive the whole start-up and send logic without a console.
struct Api {
    set_attr: AttrFn,
    get_attr: AttrFn,
    enable: DeviceFn,
    disable: DeviceFn,
    enable_chn: ChannelFn,
    disable_chn: ChannelFn,
    send: SendFn,
    query: QueryFn,
    /// The output port depth both known-working drivers set after enabling the
    /// channel. It lives in another library and is skipped if that is absent.
    depth: Option<DepthFn>,
    _libs: Vec<Library>,
}

impl Api {
    fn load() -> Result<Self, AudioError> {
        // SLOT_MI_AO_LIB names the library to use first: for a unit that keeps
        // it somewhere unexpected, and for running this against a stand-in.
        let mut paths: Vec<String> = std::env::var("SLOT_MI_AO_LIB").into_iter().collect();
        paths.extend(LIBRARIES.iter().map(|path| path.to_string()));
        let mut failures = Vec::new();
        for path in &paths {
            match Self::open(path) {
                Ok(api) => {
                    eprintln!("slot: audio: mi_ao loaded from {path}");
                    return Ok(api);
                }
                // The loader's own message already names the file it tried.
                Err(e) => failures.push(e),
            }
        }
        Err(AudioError::Device(format!(
            "libmi_ao: {}",
            failures.join("; ")
        )))
    }

    fn open(path: &str) -> Result<Self, String> {
        let dir = path.rsplit_once('/').map(|(dir, _)| dir);
        let mut libs = global_support(dir);
        let ao = open_checked(path)?;
        macro_rules! get {
            ($name:literal) => {
                *unsafe { ao.get(concat!($name, "\0").as_bytes()) }
                    .map_err(|e| format!("{}: {e}", $name))?
            };
        }
        let api = Api {
            set_attr: get!("MI_AO_SetPubAttr"),
            get_attr: get!("MI_AO_GetPubAttr"),
            enable: get!("MI_AO_Enable"),
            disable: get!("MI_AO_Disable"),
            enable_chn: get!("MI_AO_EnableChn"),
            disable_chn: get!("MI_AO_DisableChn"),
            send: get!("MI_AO_SendFrame"),
            query: get!("MI_AO_QueryChnStat"),
            depth: None,
            _libs: Vec::new(),
        };
        let sys = path
            .rsplit_once('/')
            .map(|(dir, _)| format!("{dir}/libmi_sys.so"))
            .unwrap_or_else(|| "libmi_sys.so".to_string());
        let sys = unsafe { Library::new(&sys) }.ok();
        let depth = sys.as_ref().and_then(|lib| {
            unsafe { lib.get::<DepthFn>(b"MI_SYS_SetChnOutputPortDepth\0") }
                .ok()
                .map(|f| *f)
        });
        libs.push(ao);
        libs.extend(sys);
        Ok(Api {
            depth,
            _libs: libs,
            ..api
        })
    }
}

/// The libraries `libmi_ao` leans on without listing them. It calls into the
/// system's OS wrapper (`CamOsRwsemInit` and friends) and leaves the loader to
/// find that from whatever the program already has: MinUI's preloaded libpadsp
/// supplied it, so a process without the preload died inside the first AO call
/// with an undefined symbol. Loading them first and globally makes the program
/// carry its own copy.
const SUPPORT: [&str; 2] = ["libcam_os_wrapper.so", "libmi_common.so"];

fn global_support(dir: Option<&str>) -> Vec<Library> {
    use libloading::os::unix::{Library as Unix, RTLD_GLOBAL, RTLD_NOW};
    let mut loaded = Vec::new();
    for name in SUPPORT {
        let candidates = dir
            .map(|dir| format!("{dir}/{name}"))
            .into_iter()
            .chain(["/config/lib", "/customer/lib"].map(|dir| format!("{dir}/{name}")))
            .chain([name.to_string()]);
        let mut found = false;
        for candidate in candidates {
            if let Ok(lib) = unsafe { Unix::open(Some(&candidate), RTLD_NOW | RTLD_GLOBAL) } {
                eprintln!("slot: audio: support library {candidate}");
                loaded.push(Library::from(lib));
                found = true;
                break;
            }
        }
        if !found {
            eprintln!("slot: audio: support library {name} not found");
        }
    }
    loaded
}

/// Opens the vendor library resolving every symbol at once when it can, so a
/// missing one is a line in the log and a fall back to ALSA instead of the
/// loader killing the process in the middle of a call. If it cannot be opened
/// that way, the lazy open is what was known to work with the preload present.
fn open_checked(path: &str) -> Result<Library, String> {
    use libloading::os::unix::{Library as Unix, RTLD_GLOBAL, RTLD_NOW};
    match unsafe { Unix::open(Some(path), RTLD_NOW | RTLD_GLOBAL) } {
        Ok(lib) => Ok(Library::from(lib)),
        Err(e) => {
            eprintln!("slot: audio: {path} does not resolve completely ({e}); opening lazily");
            unsafe { Library::new(path) }.map_err(|e| e.to_string())
        }
    }
}

fn describe(attr: &Attr) -> String {
    format!(
        "{} Hz, {}-bit mode {}, sound mode {}, {} frames of {} points, {} channels",
        attr.sample_rate,
        16 + 8 * attr.bit_width.clamp(0, 1),
        attr.work_mode,
        attr.sound_mode,
        attr.frame_count,
        attr.points_per_frame,
        attr.channels,
    )
}

/// A device already set up for this backend's format can simply be used.
fn fits(attr: &Attr) -> bool {
    attr.sample_rate == SAMPLE_RATE_48000
        && attr.bit_width == BIT_WIDTH_16
        && attr.sound_mode == SOUND_STEREO
        && (MIN_CHUNK..=MAX_CHUNK).contains(&attr.points_per_frame)
}

fn own_attr() -> Attr {
    Attr {
        sample_rate: SAMPLE_RATE_48000,
        bit_width: BIT_WIDTH_16,
        work_mode: MODE_I2S_MASTER,
        sound_mode: SOUND_STEREO,
        frame_count: OWN_BUFFERS,
        points_per_frame: OWN_CHUNK,
        codec_channels: 0,
        channels: CHANNELS as c_uint,
        i2s: I2sConfig::default(),
    }
}

/// How long to wait before sending more, given what the device still holds.
///
/// The count is in bytes; the limit is the vendor tweak's own measure, which
/// keeps latency down without ever starving the device.
fn throttle(busy_bytes: u32) -> Duration {
    let limit = (RATE as u64 * BYTES_PER_FRAME as u64 * QUEUE_LIMIT.as_millis() as u64) / 1000;
    let busy = u64::from(busy_bytes);
    match busy.checked_sub(limit) {
        Some(over) if over > 0 => {
            Duration::from_micros(over * 1_000_000 / (RATE as u64 * BYTES_PER_FRAME as u64))
        }
        _ => Duration::ZERO,
    }
}

/// A software clock for the audio thread: how much audio it has sent against
/// how much real time has passed. The library normally blocks the thread when its
/// buffer is full, and `throttle` reads the buffer, but neither is something to
/// rely on alone: a library that accepted frames instantly would otherwise be fed
/// as fast as the thread could run.
struct Clock {
    start: Instant,
    frames: u64,
}

/// Counts a sent chunk and says how long to sleep so the thread stays no more
/// than `QUEUE_LIMIT` ahead of real time.
fn ahead(clock: &mut Option<Clock>, now: Instant, frames: usize) -> Duration {
    let c = clock.get_or_insert(Clock {
        start: now,
        frames: 0,
    });
    c.frames += frames as u64;
    let sent = chunk_span_u64(c.frames);
    let elapsed = now.saturating_duration_since(c.start);
    if elapsed > sent + RESYNC {
        *clock = Some(Clock {
            start: now,
            frames: 0,
        });
        return Duration::ZERO;
    }
    sent.saturating_sub(elapsed).saturating_sub(QUEUE_LIMIT)
}

fn chunk_span_u64(frames: u64) -> Duration {
    Duration::from_secs_f64(frames as f64 / RATE as f64)
}

/// The firmware's audio server, stopped so this process could take the device,
/// with what it takes to start it again.
struct Stopped {
    argv: Vec<String>,
}

impl Stopped {
    fn start(&self) -> Result<u32, String> {
        let Some(exe) = self.argv.first() else {
            return Err("its command line was empty".into());
        };
        let child = std::process::Command::new(exe)
            .args(&self.argv[1..])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(child.id())
    }
}

struct Output {
    api: Api,
    /// Frames in each chunk once the device is set up, and zero before then.
    frames: usize,
    failing: bool,
    retry_at: Option<Instant>,
    sent: u64,
    clock: Option<Clock>,
    attempts: u32,
    /// The servers stopped to take the device, started again on drop.
    stopped: Vec<Stopped>,
    /// Whether the device has been taken from the server once already, so a
    /// device that keeps refusing is not chased round the same loop.
    took: bool,
    /// The gain the device was at before it was taken, to put back afterwards.
    volume: Option<i32>,
}

impl Output {
    fn new(api: Api) -> Self {
        Output {
            api,
            frames: 0,
            failing: false,
            retry_at: None,
            sent: 0,
            clock: None,
            attempts: 0,
            stopped: Vec::new(),
            took: false,
            volume: None,
        }
    }

    /// Takes the device from the firmware's audio server: reads the gain to put
    /// back, then stops the server and waits for the library to accept the
    /// device. Called once; a device that refuses after this is a different
    /// problem and is reported rather than chased.
    fn take_device(&mut self) {
        self.volume = procfs_volume();
        self.say(format!(
            "taking the device: volume {}",
            match self.volume {
                Some(db) => format!("{db} dB from {PROCFS}"),
                None => "unreadable, using the fallback".to_string(),
            }
        ));
        dump_procfs("before stopping the server");
        self.stopped = stop_servers();
        if self.stopped.is_empty() {
            self.say("no audio server was running to stop".into());
        }
        dump_procfs("after stopping the server");
    }

    /// Puts the gain back on a device this process took over. A device that
    /// comes up at the codec's default is silent with every call answering
    /// success, which is the failure that costs a whole tester round trip.
    fn restore_volume(&mut self) {
        if self.stopped.is_empty() {
            // Nothing was disturbed, so the device keeps the gain it had.
            return;
        }
        let db = self.volume.unwrap_or(FALLBACK_DB).clamp(MIN_DB, MAX_DB);
        match procfs_write(&format!("set_ao_volume {CHANNEL} {db}dB")) {
            Ok(()) => eprintln!("slot: audio: mi_ao gain set to {db} dB"),
            Err(e) => eprintln!("slot: audio: mi_ao gain not set: {e}"),
        }
        match procfs_write(&format!("set_ao_mute {}", if db > MIN_DB { 0 } else { 1 })) {
            Ok(()) => eprintln!("slot: audio: mi_ao mute {}", db <= MIN_DB),
            Err(e) => eprintln!("slot: audio: mi_ao mute not set: {e}"),
        }
    }

    fn say(&self, line: String) {
        if self.attempts <= LOUD_ATTEMPTS {
            eprintln!("slot: audio: mi_ao {line}");
        }
    }

    fn step(&self, what: &str, code: c_int) -> Result<(), String> {
        self.say(format!("{what} 0x{:x}", code as u32));
        match code {
            0 => Ok(()),
            code => Err(format!("{what} failed with 0x{:x}", code as u32)),
        }
    }

    /// Brings the device up and returns the frames per chunk to send.
    ///
    /// If the firmware's audio server owns the device, the library refuses to
    /// configure it (`0xa0052009`, operation not allowed) for as long as the
    /// server runs. That is the ordinary case on this console, not a fault: the
    /// server is stopped once, the device is asked for again, and the server is
    /// started again when slot exits.
    fn start(&mut self) -> Result<usize, String> {
        self.attempts += 1;
        let refused = match self.set_up() {
            Ok(frames) => {
                // A device this process took over is put back at the gain the
                // player had, every time it comes up.
                self.restore_volume();
                return Ok(frames);
            }
            Err(e) => e,
        };
        if self.took {
            return Err(refused);
        }
        self.took = true;
        self.say(format!(
            "{refused}; the console's audio server holds the device, stopping it"
        ));
        self.take_device();
        let mut last = refused;
        for wait in TAKE_WAITS_MS {
            // Only a server that was actually stopped needs time to let go; with
            // nothing to stop there is nothing to wait for.
            if wait > 0 && !self.stopped.is_empty() {
                std::thread::sleep(Duration::from_millis(wait));
            }
            match self.set_up() {
                Ok(frames) => {
                    self.restore_volume();
                    dump_procfs("set up by slot");
                    return Ok(frames);
                }
                Err(e) => last = e,
            }
        }
        log_holders();
        Err(last)
    }

    /// One attempt at bringing the device up, without touching the server.
    ///
    /// A device someone else already configured for this format is left as it
    /// is. Otherwise this sets it up; if the library refuses because the device
    /// is enabled with something else, it is turned off, set, and turned on.
    fn set_up(&mut self) -> Result<usize, String> {
        let mut found = AttrOut::new();
        let got = unsafe { (self.api.get_attr)(DEVICE, &mut found.attr) };
        // What the device holds now is another process's setup (the vendor audio
        // server's), and `Enable` refuses a device this process has not configured
        // itself (0xa0052007, "not configured": exactly what the first on-device
        // report showed). So attributes are always set. If the device already
        // holds this backend's format its chunk size is kept, so the audio server's
        // setup is repeated, not changed; if that still does not enable, this
        // backend's own attributes are the second try.
        let mut tries: Vec<Attr> = Vec::new();
        if got == 0 && fits(&found.attr) {
            self.say(format!(
                "device already holds this format: {}",
                describe(&found.attr)
            ));
            tries.push(found.attr);
        } else if got == 0 {
            self.say(format!(
                "device is configured differently: {}",
                describe(&found.attr)
            ));
        } else {
            self.say(format!("get attr 0x{:x}", got as u32));
        }
        tries.push(own_attr());

        let mut last = String::new();
        let mut frames = 0;
        let rungs = tries.len();
        for (n, mut attr) in tries.into_iter().enumerate() {
            match self.configure_and_enable(&mut attr) {
                Ok(()) => {
                    frames = attr.points_per_frame as usize;
                    last.clear();
                    break;
                }
                Err(e) => {
                    if n + 1 < rungs {
                        self.say(format!("{e}; trying this backend's own setup"));
                    }
                    last = e;
                }
            }
        }
        if !last.is_empty() {
            return Err(last);
        }
        self.step("enable channel", unsafe {
            (self.api.enable_chn)(DEVICE, CHANNEL)
        })?;
        if let Some(depth) = self.api.depth {
            let mut port = ChnPort {
                module: MODULE_AO,
                device: DEVICE as c_uint,
                channel: CHANNEL as c_uint,
                port: 0,
            };
            // Neither known driver checks this, and nor does this: it is
            // logged so a report shows whether the library accepted it.
            let code = unsafe { depth(&mut port, 12, 13) };
            self.say(format!("output depth 0x{:x}", code as u32));
        }
        self.frames = frames;
        self.attempts = 0;
        Ok(frames)
    }

    /// Sets the attributes and enables the device. A device that refuses new
    /// attributes is turned off and asked again; one that then refuses to enable
    /// is turned off too, so the next setup starts from a stopped device.
    fn configure_and_enable(&self, attr: &mut Attr) -> Result<(), String> {
        let set = unsafe { (self.api.set_attr)(DEVICE, attr) };
        if set != 0 {
            self.say(format!(
                "set attr 0x{:x}, turning the device off first",
                set as u32
            ));
            self.turn_off();
            self.step("set attr", unsafe { (self.api.set_attr)(DEVICE, attr) })?;
        } else {
            self.say(format!("set attr 0x0: {}", describe(attr)));
        }
        let enabled = self.step("enable", unsafe { (self.api.enable)(DEVICE) });
        if enabled.is_err() {
            self.turn_off();
        }
        enabled
    }

    fn turn_off(&self) {
        unsafe {
            let _ = (self.api.disable_chn)(DEVICE, CHANNEL);
            let _ = (self.api.disable)(DEVICE);
        }
    }

    /// Sends one chunk and says how long to wait before the next.
    fn send(&mut self, samples: &[i16]) -> Result<Duration, String> {
        let mut frame = Frame {
            bit_width: BIT_WIDTH_16,
            sound_mode: SOUND_STEREO,
            addr: [std::ptr::null_mut(); MAX_CHANNELS],
            time_stamp: 0,
            sequence: 0,
            len: (samples.len() * 2) as c_uint,
            pool: [0; 2],
            source_addr: [std::ptr::null_mut(); MAX_CHANNELS],
            source_len: 0,
        };
        frame.addr[0] = samples.as_ptr() as *mut c_void;
        let code = unsafe { (self.api.send)(DEVICE, CHANNEL, &mut frame, SEND_TIMEOUT_MS) };
        if code != 0 {
            return Err(format!("send failed with 0x{:x}", code as u32));
        }
        self.sent += 1;
        let mut state = ChnStateOut::new();
        let queried = unsafe { (self.api.query)(DEVICE, CHANNEL, &mut state.state) };
        let held = match queried {
            0 => throttle(state.state.busy),
            _ => Duration::ZERO,
        };
        let early = ahead(&mut self.clock, Instant::now(), samples.len() / CHANNELS);
        Ok(held.max(early))
    }

    fn run(&mut self, ring: &Ring, stop: &AtomicBool) {
        let mut buf = vec![0i16; OWN_CHUNK as usize * CHANNELS];
        let mut silence = Silence::quiet(RELEASE_AFTER);
        let mut pace = Pacer::new(chunk_span(OWN_CHUNK as usize));
        let mut ready = false;
        while !stop.load(Ordering::Relaxed) {
            ring.fill(&mut buf);
            let hold = silence.hear(&buf, chunk_span(buf.len() / CHANNELS));
            let mut resized = false;
            if hold && !ready && self.retry_at.is_none_or(|at| Instant::now() >= at) {
                match self.start() {
                    Ok(frames) => {
                        eprintln!(
                            "slot: audio: mi_ao ready with {frames} frames per chunk at {:.1} ms",
                            crate::boot_time::ms()
                        );
                        ready = true;
                        self.failing = false;
                        self.retry_at = None;
                        if buf.len() != frames * CHANNELS {
                            // The chunk just drawn is the wrong size for this
                            // device, so it is skipped rather than sent short.
                            buf.resize(frames * CHANNELS, 0);
                            resized = true;
                        }
                        pace = Pacer::new(chunk_span(frames));
                    }
                    Err(e) => {
                        if !self.failing {
                            eprintln!("slot: audio: mi_ao: {e}, retrying");
                            self.failing = true;
                        }
                        self.retry_at = Some(Instant::now() + RETRY_AFTER);
                    }
                }
            }
            if ready && hold && !resized {
                pace.reset();
                match self.send(&buf) {
                    Ok(wait) if wait.is_zero() => {}
                    Ok(wait) => std::thread::sleep(wait),
                    Err(e) => {
                        eprintln!("slot: audio: mi_ao: {e}");
                        // Start the device up again rather than keep sending
                        // into one that has stopped answering.
                        ready = false;
                        self.clock = None;
                        self.retry_at = Some(Instant::now() + RETRY_AFTER);
                    }
                }
            } else {
                // Not sending, so the clock has nothing to measure until it is.
                self.clock = None;
                std::thread::sleep(pace.wait(Instant::now()));
            }
        }
    }
}

fn chunk_span(frames: usize) -> Duration {
    Duration::from_secs_f64(frames as f64 / RATE as f64)
}

impl Drop for Output {
    fn drop(&mut self) {
        // The output has to be off before the server is started again, or it
        // finds a device this process still holds and stays silent itself.
        self.turn_off();
        for server in std::mem::take(&mut self.stopped) {
            match server.start() {
                Ok(pid) => eprintln!("slot: audio: the console's audio server is back as pid {pid}"),
                Err(e) => eprintln!("slot: audio: the console's audio server was not started again ({e}); the console's own sounds stay off until a reboot"),
            }
        }
    }
}

/// The pids of the firmware's audio server, matched the way Onion's own stop
/// script matches it: by the program's name, or its path, so a copy under
/// another file name still counts.
fn server_pids() -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let me = std::process::id();
    let mut pids = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == me {
            continue;
        }
        let comm = std::fs::read_to_string(entry.path().join("comm")).unwrap_or_default();
        let argv = argv_of(pid);
        let named = SERVERS.contains(&comm.trim());
        let pathed = argv
            .first()
            .and_then(|a| a.rsplit('/').next())
            .is_some_and(|a| SERVERS.iter().any(|s| a.contains(s)));
        if named || pathed {
            pids.push(pid);
        }
    }
    pids
}

/// A process's command line, as it would be started again.
fn argv_of(pid: u32) -> Vec<String> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    let argv: Vec<String> = raw
        .split(|b| *b == 0)
        .filter(|a| !a.is_empty())
        .map(|a| String::from_utf8_lossy(a).into_owned())
        .collect();
    if !argv.is_empty() {
        return argv;
    }
    // A kernel or a busybox that does not fill cmdline still has its binary.
    std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .map(|p| vec![p.to_string_lossy().into_owned()])
        .unwrap_or_default()
}

/// Stops every audio server found, returning what it takes to start them again.
fn stop_servers() -> Vec<Stopped> {
    let mut stopped = Vec::new();
    for pid in server_pids() {
        let argv = argv_of(pid);
        let code = unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
        if code == 0 {
            eprintln!(
                "slot: audio: mi_ao stopped pid {pid} ({})",
                argv.first().map(String::as_str).unwrap_or("unknown")
            );
        } else {
            eprintln!(
                "slot: audio: mi_ao could not stop pid {pid}: {}",
                std::io::Error::last_os_error()
            );
        }
        stopped.push(Stopped { argv });
    }
    stopped
}

/// Names the processes holding the AO device or the library that fronts it, for
/// a report where the library still refuses after the server was stopped. The
/// console's own audio server is not the only thing that can hold it: anything
/// started with MinUI's `libpadsp` preloaded and playing a sound holds it open.
fn log_holders() {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return;
    };
    let me = std::process::id();
    let mut found = 0;
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == me {
            continue;
        }
        let fds = std::fs::read_dir(entry.path().join("fd"))
            .map(|fds| {
                fds.flatten()
                    .filter_map(|fd| std::fs::read_link(fd.path()).ok())
                    .any(|target| target.to_string_lossy().contains("mi_ao"))
            })
            .unwrap_or(false);
        let padsp = std::fs::read_to_string(entry.path().join("maps"))
            .map(|maps| maps.contains("libpadsp"))
            .unwrap_or(false);
        if !fds && !padsp {
            continue;
        }
        found += 1;
        if found > 5 {
            continue;
        }
        let comm = std::fs::read_to_string(entry.path().join("comm")).unwrap_or_default();
        eprintln!(
            "slot: audio: mi_ao holder pid {pid} ({}) {}",
            comm.trim(),
            match (fds, padsp) {
                (true, true) => "has the device open and libpadsp loaded",
                (true, false) => "has the device open",
                _ => "has libpadsp loaded",
            }
        );
    }
    if found == 0 {
        eprintln!("slot: audio: mi_ao no process holds the device");
    }
}

fn procfs_text() -> Option<String> {
    std::fs::read_to_string(PROCFS).ok()
}

/// The device's gain in dB, read the way Onion's stop script reads it: the
/// `LineOut` row carries it in its eighth field.
fn procfs_volume() -> Option<i32> {
    parse_volume(&procfs_text()?)
}

fn parse_volume(text: &str) -> Option<i32> {
    let line = text.lines().find(|l| l.contains("LineOut"))?;
    let field = line.split_whitespace().nth(7)?;
    let digits: String = field
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    digits
        .parse::<i32>()
        .ok()
        .filter(|db| (MIN_DB..=MAX_DB).contains(db))
}

fn procfs_write(command: &str) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(PROCFS)
        .map_err(|e| format!("{command}: {e}"))?;
    file.write_all(format!("{command}\n").as_bytes())
        .map_err(|e| format!("{command}: {e}"))
}

/// Writes what the device says about itself to the log, once. A report that
/// names the volume, the mute and whether the output is started answers most of
/// what a silent game can be.
fn dump_procfs(when: &str) {
    let Some(text) = procfs_text() else {
        eprintln!("slot: audio: mi_ao {when}: no {PROCFS} to read");
        return;
    };
    for line in text.lines().take(DUMP_LINES) {
        eprintln!("slot: audio: mi_ao {when} | {}", line.trim_end());
    }
}

/// Gives the calling thread a higher scheduling priority. A thread has its own
/// nice value on Linux, and 0 means the calling one. It is only a request: a
/// process that may not raise it keeps running at the priority it had.
pub(super) fn raise_priority() {
    let code = unsafe { libc::setpriority(libc::PRIO_PROCESS as _, 0, AUDIO_NICE) };
    match code {
        0 => eprintln!("slot: audio: thread priority raised to {AUDIO_NICE}"),
        _ => eprintln!(
            "slot: audio: thread priority unchanged: {}",
            std::io::Error::last_os_error()
        ),
    }
}

pub struct MiAoSink {
    ring: Arc<Ring>,
    thread: Option<(Arc<AtomicBool>, JoinHandle<()>)>,
}

impl MiAoSink {
    pub fn new() -> Self {
        MiAoSink {
            ring: Arc::new(Ring::new(0)),
            thread: None,
        }
    }

    fn close(&mut self) {
        if let Some((stop, join)) = self.thread.take() {
            stop.store(true, Ordering::Relaxed);
            let _ = join.join();
        }
    }
}

impl Default for MiAoSink {
    fn default() -> Self {
        MiAoSink::new()
    }
}

impl Drop for MiAoSink {
    fn drop(&mut self) {
        self.close();
    }
}

impl AudioSink for MiAoSink {
    fn open(&mut self, sample_rate: u32) -> Result<(), AudioError> {
        if sample_rate != RATE {
            return Err(AudioError::Config(format!(
                "the vendor output plays at {RATE} Hz, not {sample_rate}"
            )));
        }
        self.close();
        let ring = self.ring.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        let join = std::thread::Builder::new()
            .name("slot-audio".into())
            .spawn(move || match Api::load() {
                Ok(api) => {
                    raise_priority();
                    ring.reopen(RATE);
                    let _ = ready_tx.send(Ok(()));
                    Output::new(api).run(&ring, &flag);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            })
            .map_err(|e| AudioError::Device(e.to_string()))?;
        match ready_rx.recv() {
            Ok(Ok(())) => {
                self.thread = Some((stop, join));
                Ok(())
            }
            Ok(Err(e)) => {
                let _ = join.join();
                Err(e)
            }
            Err(_) => {
                let _ = join.join();
                Err(AudioError::Device("output thread stopped".into()))
            }
        }
    }

    fn ring(&self) -> Arc<Ring> {
        self.ring.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::mem::{offset_of, size_of};

    use super::*;

    thread_local! {
        static CALLS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
        static CODES: RefCell<Vec<(&'static str, c_int)>> = const { RefCell::new(Vec::new()) };
        static DEVICE_ATTR: RefCell<Option<Attr>> = const { RefCell::new(None) };
        static SENT: RefCell<Vec<(u32, usize, usize, c_int, c_int)>> = const { RefCell::new(Vec::new()) };
        static BUSY: RefCell<u32> = const { RefCell::new(0) };
    }

    fn note(call: &str) {
        CALLS.with(|c| c.borrow_mut().push(call.to_string()));
    }

    /// The code a fake call answers with: the first one queued for it, once.
    fn answer(call: &'static str) -> c_int {
        CODES.with(|codes| {
            let mut codes = codes.borrow_mut();
            match codes.iter().position(|(name, _)| *name == call) {
                Some(at) => codes.remove(at).1,
                None => 0,
            }
        })
    }

    unsafe extern "C" fn fake_set(_: c_int, attr: *mut Attr) -> c_int {
        let attr = unsafe { *attr };
        note(&format!(
            "set {} {} {} {}",
            attr.sample_rate, attr.sound_mode, attr.points_per_frame, attr.frame_count
        ));
        answer("set")
    }
    unsafe extern "C" fn fake_get(_: c_int, attr: *mut Attr) -> c_int {
        note("get");
        match DEVICE_ATTR.with(|a| *a.borrow()) {
            Some(found) => {
                unsafe { *attr = found };
                answer("get")
            }
            None => 0x8000_0001u32 as c_int,
        }
    }
    unsafe extern "C" fn fake_enable(_: c_int) -> c_int {
        note("enable");
        answer("enable")
    }
    unsafe extern "C" fn fake_disable(_: c_int) -> c_int {
        note("disable");
        0
    }
    unsafe extern "C" fn fake_enable_chn(_: c_int, _: c_int) -> c_int {
        note("enable chn");
        answer("enable chn")
    }
    unsafe extern "C" fn fake_disable_chn(_: c_int, _: c_int) -> c_int {
        note("disable chn");
        0
    }
    unsafe extern "C" fn fake_send(_: c_int, _: c_int, frame: *mut Frame, wait: c_int) -> c_int {
        let frame = unsafe { &*frame };
        SENT.with(|s| {
            s.borrow_mut().push((
                frame.len,
                frame.addr[0] as usize,
                frame.addr[1] as usize,
                frame.bit_width,
                frame.sound_mode,
            ))
        });
        note(&format!("send wait {wait}"));
        answer("send")
    }
    unsafe extern "C" fn fake_query(_: c_int, _: c_int, state: *mut ChnState) -> c_int {
        unsafe { (*state).busy = BUSY.with(|b| *b.borrow()) };
        0
    }
    unsafe extern "C" fn fake_depth(port: *mut ChnPort, user: c_uint, queue: c_uint) -> c_int {
        let port = unsafe { &*port };
        note(&format!(
            "depth module {} {} {} {} {user} {queue}",
            port.module, port.device, port.channel, port.port
        ));
        0
    }

    fn fake(with_depth: bool) -> Output {
        CALLS.with(|c| c.borrow_mut().clear());
        CODES.with(|c| c.borrow_mut().clear());
        DEVICE_ATTR.with(|a| *a.borrow_mut() = None);
        SENT.with(|s| s.borrow_mut().clear());
        BUSY.with(|b| *b.borrow_mut() = 0);
        Output::new(Api {
            set_attr: fake_set,
            get_attr: fake_get,
            enable: fake_enable,
            disable: fake_disable,
            enable_chn: fake_enable_chn,
            disable_chn: fake_disable_chn,
            send: fake_send,
            query: fake_query,
            depth: with_depth.then_some(fake_depth as DepthFn),
            _libs: Vec::new(),
        })
    }

    /// Refuses `set` this many times. Each rung sets the attributes, fails, turns
    /// the device off and sets them again, so one attempt is two refusals per
    /// rung: two with a device that reports a setup to match, one without.
    fn refuse_sets(times: usize) {
        for _ in 0..times {
            CODES.with(|c| c.borrow_mut().push(("set", 0xA005_2009u32 as c_int)));
        }
    }

    fn calls() -> Vec<String> {
        CALLS.with(|c| c.borrow().clone())
    }

    fn configured(rate: c_int, points: c_uint) -> Attr {
        Attr {
            sample_rate: rate,
            bit_width: BIT_WIDTH_16,
            work_mode: MODE_I2S_MASTER,
            sound_mode: SOUND_STEREO,
            frame_count: 6,
            points_per_frame: points,
            codec_channels: 0,
            channels: 2,
            i2s: I2sConfig::default(),
        }
    }

    #[test]
    fn a_device_already_holding_this_format_is_set_again_with_its_own_chunk_size() {
        // The device reports a setup, but it is another process's: Enable refuses a
        // device this process never configured, so the attributes are set anyway.
        let mut out = fake(false);
        DEVICE_ATTR.with(|a| *a.borrow_mut() = Some(configured(48_000, 1024)));
        assert_eq!(out.start(), Ok(1024));
        assert_eq!(
            calls(),
            ["get", "set 48000 1 1024 6", "enable", "enable chn"]
        );
    }

    #[test]
    fn a_device_that_will_not_enable_with_its_own_chunk_gets_this_backends_setup() {
        // What the first report from a console showed: GetPubAttr answered with a
        // working setup and Enable then said "not configured" (0xa0052007).
        let mut out = fake(false);
        DEVICE_ATTR.with(|a| *a.borrow_mut() = Some(configured(48_000, 768)));
        CODES.with(|c| c.borrow_mut().push(("enable", 0xA005_2007u32 as c_int)));
        assert_eq!(out.start(), Ok(OWN_CHUNK as usize));
        assert_eq!(
            calls(),
            [
                "get",
                "set 48000 1 768 6",
                "enable",
                "disable chn",
                "disable",
                "set 48000 1 512 6",
                "enable",
                "enable chn"
            ]
        );
    }

    #[test]
    fn a_device_the_audio_server_holds_is_taken_from_it_and_set_up_again() {
        // What the second report from a console showed: with the firmware's
        // audio server running, SetPubAttr answers 0xa0052009, "operation not
        // allowed", because the server is playing through the device. Both rungs
        // are refused once, then the device accepts this backend's own setup.
        let mut out = fake(false);
        DEVICE_ATTR.with(|a| *a.borrow_mut() = Some(configured(48_000, 768)));
        refuse_sets(4);
        // Once the server has let go, the device is free to keep the setup it
        // reports (768 frames), which is what a server set it to.
        assert_eq!(out.start(), Ok(768));
        assert!(out.took, "the device was taken from the server");
        let calls = calls();
        assert_eq!(
            calls.iter().filter(|c| *c == "get").count(),
            2,
            "the device was asked for again after the server was stopped: {calls:?}"
        );
        assert_eq!(
            calls.last().map(String::as_str),
            Some("enable chn"),
            "{calls:?}"
        );
    }

    #[test]
    fn a_device_that_keeps_refusing_after_the_takeover_reports_the_last_refusal() {
        let mut out = fake(false);
        DEVICE_ATTR.with(|a| *a.borrow_mut() = Some(configured(48_000, 768)));
        // Two rungs, the first attempt and the five retries after the takeover.
        refuse_sets(2 * 2 * 6);
        let err = out.start().unwrap_err();
        assert!(err.contains("set") && err.contains("0xa0052009"), "{err}");
        assert!(!calls().contains(&"enable chn".to_string()));
    }

    #[test]
    fn an_unconfigured_device_is_set_up_with_this_backends_own_attributes() {
        let mut out = fake(false);
        assert_eq!(out.start(), Ok(OWN_CHUNK as usize));
        assert_eq!(
            calls(),
            ["get", "set 48000 1 512 6", "enable", "enable chn"]
        );
    }

    #[test]
    fn a_device_at_another_rate_is_set_up_again_not_adopted() {
        let mut out = fake(false);
        DEVICE_ATTR.with(|a| *a.borrow_mut() = Some(configured(44_100, 1024)));
        assert_eq!(out.start(), Ok(OWN_CHUNK as usize));
        assert!(
            calls().contains(&"set 48000 1 512 6".to_string()),
            "{:?}",
            calls()
        );
    }

    #[test]
    fn a_device_that_refuses_new_attributes_is_turned_off_and_set_again() {
        let mut out = fake(false);
        DEVICE_ATTR.with(|a| *a.borrow_mut() = Some(configured(44_100, 1024)));
        CODES.with(|c| c.borrow_mut().push(("set", 0xA005_2005u32 as c_int)));
        assert_eq!(out.start(), Ok(OWN_CHUNK as usize));
        assert_eq!(
            calls(),
            [
                "get",
                "set 48000 1 512 6",
                "disable chn",
                "disable",
                "set 48000 1 512 6",
                "enable",
                "enable chn"
            ]
        );
    }

    #[test]
    fn start_reports_the_code_of_the_step_that_failed() {
        let mut out = fake(false);
        for _ in 0..12 {
            CODES.with(|c| c.borrow_mut().push(("enable", 0xA005_2007u32 as c_int)));
        }
        let err = out.start().unwrap_err();
        assert!(
            err.contains("enable") && err.contains("0xa0052007"),
            "{err}"
        );
        assert!(!calls().contains(&"enable chn".to_string()));
    }

    #[test]
    fn a_device_that_keeps_refusing_is_counted_and_a_success_clears_the_count() {
        let mut out = fake(false);
        // This device reports no setup, so one attempt is one rung and two sets.
        // The first start also makes the five retries that follow the takeover;
        // the device is taken once, so every start after it is a single attempt.
        refuse_sets(2 * 6);
        assert!(out.start().is_err());
        for _ in 0..4 {
            refuse_sets(2);
            assert!(out.start().is_err());
        }
        assert_eq!(
            out.attempts, 5,
            "only the first few attempts log every step"
        );
        assert!(out.start().is_ok());
        assert_eq!(out.attempts, 0);
    }

    #[test]
    fn the_output_depth_is_set_for_the_ao_channel_when_that_library_is_there() {
        let mut out = fake(true);
        out.start().unwrap();
        assert_eq!(calls().last().unwrap(), "depth module 5 0 0 0 12 13");
        let mut out = fake(false);
        out.start().unwrap();
        assert!(!calls().iter().any(|c| c.starts_with("depth")));
    }

    #[test]
    fn a_chunk_goes_out_as_one_interleaved_stereo_frame() {
        let mut out = fake(false);
        let samples = vec![1i16; 512 * 2];
        out.send(&samples).unwrap();
        let (len, first, second, width, mode) = SENT.with(|s| s.borrow()[0]);
        assert_eq!(len as usize, 512 * 4, "length is in bytes");
        assert_eq!(first, samples.as_ptr() as usize);
        assert_eq!(second, 0, "no second channel buffer");
        assert_eq!((width, mode), (BIT_WIDTH_16, SOUND_STEREO));
        assert_eq!(calls(), [format!("send wait {SEND_TIMEOUT_MS}")]);
    }

    #[test]
    fn a_failed_send_carries_its_code() {
        let mut out = fake(false);
        CODES.with(|c| c.borrow_mut().push(("send", 0xA005_200Du32 as c_int)));
        let err = out.send(&[0; 8]).unwrap_err();
        assert!(err.contains("0xa005200d"), "{err}");
    }

    #[test]
    fn the_thread_waits_only_when_the_device_holds_more_than_its_limit() {
        // 48 ms of 48 kHz stereo 16-bit is 9216 bytes.
        assert_eq!(throttle(0), Duration::ZERO);
        assert_eq!(throttle(9216), Duration::ZERO);
        assert_eq!(throttle(9216 + 1920), Duration::from_millis(10));
        let mut out = fake(false);
        BUSY.with(|b| *b.borrow_mut() = 9216 + 1920);
        assert_eq!(out.send(&[0; 8]), Ok(Duration::from_millis(10)));
    }

    #[test]
    fn a_thread_that_sends_faster_than_real_time_is_made_to_wait() {
        let mut clock = None;
        let now = Instant::now();
        let chunk = 512;
        // Four chunks at once is 42.7 ms of audio, inside the 48 ms limit.
        for _ in 0..4 {
            assert_eq!(ahead(&mut clock, now, chunk), Duration::ZERO);
        }
        // The fifth is 53.3 ms ahead of a clock that has not moved.
        let wait = ahead(&mut clock, now, chunk);
        assert!(
            wait > Duration::from_micros(5_000) && wait < Duration::from_micros(5_600),
            "{wait:?}"
        );
        // Keeping sending without waiting makes the wait grow by a chunk each time.
        let more = ahead(&mut clock, now, chunk);
        assert!(
            more > wait + Duration::from_millis(10),
            "{more:?} after {wait:?}"
        );
    }

    #[test]
    fn a_thread_keeping_real_time_never_waits() {
        let mut clock = None;
        let start = Instant::now();
        for n in 1..=200u32 {
            let at = start + Duration::from_secs_f64(f64::from(n) * 512.0 / RATE as f64);
            assert_eq!(ahead(&mut clock, at, 512), Duration::ZERO, "chunk {n}");
        }
    }

    #[test]
    fn a_thread_that_fell_behind_starts_its_clock_again_instead_of_catching_up() {
        let mut clock = None;
        let start = Instant::now();
        assert_eq!(ahead(&mut clock, start, 512), Duration::ZERO);
        // Half a second later, with one chunk sent: far behind real time.
        let late = start + Duration::from_millis(500);
        assert_eq!(ahead(&mut clock, late, 512), Duration::ZERO);
        // The clock is anchored at the late moment, so what follows is measured
        // from there, not from the stale start.
        assert_eq!(ahead(&mut clock, late, 512), Duration::ZERO);
        assert_eq!(clock.as_ref().unwrap().frames, 512);
    }

    #[test]
    fn only_a_device_in_this_format_with_a_sane_chunk_is_adopted() {
        assert!(fits(&configured(48_000, 1024)));
        assert!(!fits(&configured(44_100, 1024)));
        assert!(!fits(&configured(48_000, 0)));
        assert!(!fits(&configured(48_000, 1_000_000)));
        let mut mono = configured(48_000, 1024);
        mono.sound_mode = 0;
        assert!(!fits(&mono));
    }

    #[test]
    fn the_gain_is_read_from_the_lineout_row_the_way_the_onion_script_reads_it() {
        // Field eight of the LineOut row, with the punctuation some SDK builds
        // put in it. The row itself is written to the log, so a build that puts
        // the gain elsewhere is visible in the next report.
        let text = "\
AoDev   SamR BitWidth SondMod PtNumPerFrm VolumeDb bMuteEnable bStartPcmOut
LineOut 48000 16       ster    768        0        0           -12
";
        assert_eq!(parse_volume(text), Some(-12));
        assert_eq!(parse_volume("LineOut a b c d e f -6,\n"), Some(-6));
        let out_of_range = "LineOut a b c d e f 99dB\n";
        assert_eq!(parse_volume(out_of_range), None, "a gain no device accepts");
        assert_eq!(parse_volume("nothing useful here\n"), None);
        assert_eq!(parse_volume("LineOut a b c d e f x\n"), None);
    }

    #[test]
    fn the_structures_have_the_layout_the_library_expects() {
        let ptr = size_of::<usize>();
        // MI_AUDIO_Attr_t: eight 32-bit fields, then the 12-byte I2S block.
        assert_eq!(size_of::<I2sConfig>(), 12);
        assert_eq!(size_of::<Attr>(), 44);
        assert_eq!(offset_of!(Attr, points_per_frame), 20);
        assert_eq!(offset_of!(Attr, channels), 28);
        assert_eq!(offset_of!(Attr, i2s), 32);
        // MI_AUDIO_Frame_t: two enums, sixteen pointers, a 64-bit stamp.
        assert_eq!(offset_of!(Frame, addr), 8);
        let after_pointers = 8 + MAX_CHANNELS * ptr;
        assert_eq!(offset_of!(Frame, time_stamp), after_pointers);
        assert_eq!(offset_of!(Frame, sequence), after_pointers + 8);
        assert_eq!(offset_of!(Frame, len), after_pointers + 12);
        assert_eq!(offset_of!(Frame, pool), after_pointers + 16);
        assert_eq!(offset_of!(Frame, source_addr), after_pointers + 24);
        assert_eq!(size_of::<ChnState>(), 12);
        assert_eq!(size_of::<ChnPort>(), 16);
        #[cfg(target_pointer_width = "32")]
        assert_eq!(size_of::<Frame>(), 168);
    }

    /// Runs the real thread and the real library loading against a stand-in
    /// `libmi_ao.so`. It needs that library built for the machine running the
    /// tests, so it only runs when asked: `SLOT_TEST_FAKE_MI_AO=<path to the .so>`
    /// and `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs the stand-in library from tests/fake_mi_ao"]
    fn the_real_thread_plays_a_tone_through_a_stand_in_library() {
        let lib = std::env::var("SLOT_TEST_FAKE_MI_AO").expect("SLOT_TEST_FAKE_MI_AO");
        let log = std::env::temp_dir().join(format!("fake-mi-ao-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&log);
        std::env::set_var("SLOT_MI_AO_LIB", &lib);
        std::env::set_var("FAKE_MI_AO_LOG", &log);
        let mode = std::env::var("FAKE_MI_AO_MODE").unwrap_or_else(|_| "fresh".into());

        let mut sink = MiAoSink::new();
        sink.open(RATE).expect("the stand-in library should load");
        let ring = sink.ring();
        // Half a second of a loud square tone, fed at about the rate it plays.
        let tone: Vec<i16> = (0..960 * 2)
            .map(|i| if (i / 2) % 48 < 24 { 8000 } else { -8000 })
            .collect();
        for _ in 0..25 {
            ring.push(&tone);
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(Duration::from_millis(200));
        drop(sink);

        let text = std::fs::read_to_string(&log).expect("the stand-in should have logged");
        let lines: Vec<&str> = text.lines().collect();
        eprintln!("{text}");
        let first_send = lines
            .iter()
            .position(|l| l.starts_with("send #1 "))
            .expect("a frame was sent");
        let chunk = match mode.as_str() {
            "configured" => 1024,
            "unowned" => 768,
            _ => 512,
        };
        assert!(
            lines[first_send].contains(&format!("len {}", chunk * 4)),
            "a chunk is its frames times four bytes: {}",
            lines[first_send]
        );
        assert!(
            lines[first_send].contains("bits 0 sound 1"),
            "{}",
            lines[first_send]
        );
        assert!(
            !lines[first_send].ends_with("sum 0"),
            "the tone arrived: {}",
            lines[first_send]
        );
        assert!(lines.iter().all(|l| *l != "send second buffer set"));
        let total: u64 = lines
            .iter()
            .find_map(|l| l.strip_prefix("total sends "))
            .and_then(|n| n.parse().ok())
            .expect("the stand-in reports its total when it is unloaded");
        // About 0.7 s of audio at 93 chunks a second, plus the trailing silence the
        // thread keeps sending for a moment. An unthrottled thread sent 1700 here.
        assert!(
            total < 260,
            "sent {total} chunks in under a second of real time"
        );
        assert!(
            lines[..first_send].iter().any(|l| *l == "enable chn"),
            "the channel was enabled before the first frame"
        );
        match mode.as_str() {
            "configured" | "unowned" => assert!(
                lines
                    .iter()
                    .any(|l| l.starts_with(&format!("set 48000 1 {chunk} 6"))),
                "{lines:?}"
            ),
            _ => assert!(
                lines.iter().any(|l| l.starts_with("set 48000 1 512 6")),
                "{lines:?}"
            ),
        }
    }

    #[test]
    fn a_rate_other_than_the_vendors_is_refused_up_front() {
        let mut sink = MiAoSink::new();
        let err = sink.open(44_100).unwrap_err().to_string();
        assert!(err.contains("48000"), "{err}");
    }
}
