use std::path::PathBuf;
#[cfg(not(feature = "miyoomini"))]
use std::time::{Duration, Instant};

use slot::input::DeviceInput;
#[cfg(not(feature = "miyoomini"))]
use slot::frontend::Frontend;
#[cfg(not(any(feature = "flip", feature = "miyoomini")))]
use slot_gfx::FbdevSurface;
#[cfg(feature = "flip")]
use slot_gfx::SdlSurface;
#[cfg(not(feature = "miyoomini"))]
use slot_gfx::{Compositor, Surface};
#[cfg(not(feature = "miyoomini"))]
use slot_power::{trace_first_frame, DevicePlatform};

const CARD: &str = "/mnt/sdcard";

#[cfg(not(feature = "miyoomini"))]
const MIN_FRAME: Duration = Duration::from_millis(12);

#[cfg(not(feature = "miyoomini"))]
const STEP_TIMEOUT: Duration = Duration::from_millis(12);

#[cfg(not(feature = "miyoomini"))]
#[cfg(not(feature = "miyoomini"))]
const MARGIN: Duration = Duration::from_micros(1500);

#[cfg(not(feature = "miyoomini"))]
struct Pacer {
    last_return: Option<Instant>,
    period: Duration,
    work: Duration,
    works: [Duration; WORK_WINDOW],
    next: usize,
}

#[cfg(not(feature = "miyoomini"))]
const WORK_WINDOW: usize = 32;

#[cfg(not(feature = "miyoomini"))]
impl Pacer {
    fn new() -> Self {
        Pacer {
            last_return: None,
            period: Duration::from_micros(16_760),
            work: Duration::from_millis(4),
            works: [Duration::from_millis(4); WORK_WINDOW],
            next: 0,
        }
    }

    fn wait(&self) {
        let Some(last) = self.last_return else {
            return;
        };
        let delay = self.period.saturating_sub(self.work + MARGIN);
        if let Some(left) = (last + delay).checked_duration_since(Instant::now()) {
            std::thread::sleep(left);
        }
    }

    fn swapped(&mut self, work: Duration, blocked: bool) -> bool {
        let now = Instant::now();
        let mut missed = false;
        if let Some(last) = self.last_return {
            let seen = now - last;
            missed = seen > self.period.mul_f64(1.5);
            if blocked && seen > Duration::from_millis(12) && seen < Duration::from_millis(22) {
                self.period = self.period.mul_f64(0.95) + seen.mul_f64(0.05);
            }
        }
        self.works[self.next] = work;
        self.next = (self.next + 1) % WORK_WINDOW;
        self.work = self.works.iter().copied().max().unwrap_or(work);
        self.last_return = Some(now);
        missed
    }
}

pub fn run() {
    slot::boot_time::mark("start");
    let _ = std::thread::Builder::new()
        .name("slot-fonts".into())
        .spawn(slot_ui::warm_fonts);
    // Opened on its own thread so it overlaps the display setup. The Mini Plus
    // build exits before it would ever need audio.
    #[cfg(not(feature = "miyoomini"))]
    let audio = std::thread::Builder::new()
        .name("slot-audio-open".into())
        .spawn(slot::audio::opened_sink);
    let root = std::env::var_os("SLOT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(CARD));
    #[cfg(feature = "miyoomini")]
    run_miyoomini(&root);
    #[cfg(not(feature = "miyoomini"))]
    run_gles(root, audio);
}

// The Miyoo Mini Plus has no GPU, so the GLES renderer cannot run. Probe its
// framebuffer, report what it looks like, then exit cleanly — the software
// compositor this target needs is not written yet. This leaves testers a useful
// slot.log instead of a silent crash. See TESTERS.md.
#[cfg(feature = "miyoomini")]
fn run_miyoomini(root: &std::path::Path) {
    eprintln!("slot: EXPERIMENTAL miyoomini build — software renderer not yet implemented");
    // Confirm the input map loads and devices enumerate, so the log shows the
    // buttons were found even though nothing is drawn yet.
    slot::input::keymap::load(root);
    let _ = DeviceInput::open(root);
    match slot_gfx::probe_fb() {
        Ok(info) => eprintln!("slot: {}", info.report()),
        Err(e) => eprintln!("slot: framebuffer probe failed: {e}"),
    }
    eprintln!(
        "slot: input map and hardware probing are wired up, but drawing to the \
         framebuffer is still TODO. Please share this slot.log on the GitHub thread."
    );
}

#[cfg(not(feature = "miyoomini"))]
fn run_gles(
    root: PathBuf,
    audio: std::io::Result<std::thread::JoinHandle<Box<dyn slot::audio::AudioSink>>>,
) {
    #[cfg(not(feature = "flip"))]
    let surface_result = FbdevSurface::new();
    #[cfg(feature = "flip")]
    let surface_result = SdlSurface::new();
    let mut surface = match surface_result {
        Ok(s) => s,
        Err(e) => {
            eprintln!("slot: {e}");
            return;
        }
    };
    slot::boot_time::mark("surface");
    let mut compositor = match Compositor::new(&surface) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("slot: {e}");
            return;
        }
    };
    slot::boot_time::mark("compositor");
    let panel = surface.window_size();
    let turned = panel.0 < panel.1;
    let window = if turned { (panel.1, panel.0) } else { panel };
    let ccw = std::env::var("SLOT_ROTATE")
        .map(|v| v.trim().eq_ignore_ascii_case("ccw"))
        .unwrap_or(false);
    eprintln!(
        "slot: panel {}x{}, canvas {}x{}, turned {turned}, ccw {ccw}",
        panel.0, panel.1, window.0, window.1
    );
    if turned {
        if let Err(e) = compositor.set_panel_turn(panel, ccw) {
            eprintln!("slot: {e}");
            return;
        }
    }
    let platform = DevicePlatform::new(root.clone());
    eprintln!("slot: {}", platform.report());
    platform.trace_boot();
    slot::boot_time::mark("platform");
    let sink = match audio.map(|h| h.join()) {
        Ok(Ok(sink)) => sink,
        _ => slot::audio::opened_sink(),
    };
    slot::boot_time::mark("audio");
    // Load any user button remap before input threads start reading events.
    slot::input::keymap::load(&root);
    let mut frontend = Frontend::boot_with(Box::new(platform), sink);
    slot::boot_time::mark("frontend");
    frontend.upload_boot_faces(&mut compositor);
    slot::boot_time::mark("faces");
    frontend.upload_bezel(&mut compositor, window);
    slot::boot_time::mark("bezel");
    let mut input = DeviceInput::open(&root);
    slot::boot_time::mark("input");
    let card = root.clone();
    let _ = std::thread::Builder::new()
        .name("slot-bootlogo".into())
        .spawn(move || slot::bootlogo::refresh(&card));
    let mut drawn = false;
    frontend.drive_emulator();
    let mut pacer = Pacer::new();
    let (mut frames, mut missed, mut early) = (0u32, 0u32, 0u32);
    #[cfg(feature = "flip")]
    let mut heartbeat = (Instant::now(), 0u64, frontend.frames_published());
    loop {
        pacer.wait();
        let began = Instant::now();
        frontend.advance(&mut input);
        if frontend.restarting() {
            frontend.restart();
        }
        if frontend.powering_off() {
            frontend.poweroff();
            return;
        }
        frontend.step_emulator(pacer.period, STEP_TIMEOUT);
        frontend.render(&mut compositor, window);
        let swap = Instant::now();
        let work = swap - began;
        if let Err(e) = surface.swap() {
            eprintln!("slot: {e}");
            return;
        }
        let swap_took = swap.elapsed();
        slot::latency::swapped(swap_took.as_secs_f64() * 1000.0);
        let dropped = pacer.swapped(work, swap_took > Duration::from_millis(1));
        if slot::latency::tracing() {
            frames += 1;
            missed += u32::from(dropped);
            early += u32::from(swap_took > pacer.period / 2);
            if frames == 600 {
                eprintln!(
                    "slot: pace: 600 frames, {missed} missed a latch, {early} drawn early, work {:.1} ms, period {:.2} ms",
                    pacer.work.as_secs_f64() * 1e3,
                    pacer.period.as_secs_f64() * 1e3
                );
                (frames, missed, early) = (0, 0, 0);
            }
        }
        // Flip heartbeat: every ~2s report how many main-loop iterations ran
        // versus how many emulator frames the core published. Only under trace.
        #[cfg(feature = "flip")]
        if slot::latency::tracing() {
            heartbeat.1 += 1;
            if heartbeat.0.elapsed() >= Duration::from_secs(2) {
                let published = frontend.frames_published();
                eprintln!(
                    "slot: heartbeat: {} loops, {} emu frames in {:?} (settling {}, period {:.2} ms)",
                    heartbeat.1,
                    published.saturating_sub(heartbeat.2),
                    heartbeat.0.elapsed(),
                    frontend.core_settling(),
                    pacer.period.as_secs_f64() * 1e3,
                );
                heartbeat = (Instant::now(), 0, published);
            }
        }
        if !drawn {
            drawn = true;
            slot::boot_time::mark("first frame");
            trace_first_frame();
        }
        if let Some(left) = MIN_FRAME.checked_sub(began.elapsed()) {
            std::thread::sleep(left);
        }
    }
}
