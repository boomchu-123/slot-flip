use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use slot_input::{Btn, InputSource, Millis, RawEvent};

#[cfg(feature = "miyoomini")]
use super::evdev::code_to_btn;
use super::evdev::{
    decode, device_name, pick_devices, to_raw, Ev, Hat, EVENT_BYTES, EV_ABS, EV_KEY, EV_SYN,
};
#[cfg(feature = "flip")]
use super::evdev::{LeftStick, Stick};
use super::trace;

const DEV: &str = "/dev/input";
const SYS: &str = "/sys/class/input";
const PSY: &str = "/sys/class/power_supply";

const LID_POLL_MS: Millis = 150;

pub struct DeviceInput {
    pending: Arc<Mutex<Vec<RawEvent>>>,
    stick: Arc<AtomicBool>,
    hall: Option<PathBuf>,
    lid_shut: Option<bool>,
    next_lid_poll: Millis,
}

impl DeviceInput {
    pub fn open(root: &Path) -> Self {
        DeviceInput::open_in(Path::new(DEV), Path::new(SYS), root, trace::enabled())
    }

    pub fn open_in(dev: &Path, sys: &Path, root: &Path, trace: bool) -> Self {
        let trace = trace.then(|| Trace::start(root, dev, sys)).flatten();
        DeviceInput::open_traced(dev, sys, trace)
    }

    fn open_traced(dev: &Path, sys: &Path, trace: Option<Arc<Trace>>) -> Self {
        let pending: Arc<Mutex<Vec<RawEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let stick = Arc::new(AtomicBool::new(false));
        // The Mini Plus reports every button on one node, but its power key and
        // battery may live on another. One line per node, always in the log,
        // says what is there and what was picked; on that handheld the answer
        // was not visible anywhere else.
        #[cfg(feature = "miyoomini")]
        for line in trace::survey(dev, sys) {
            eprintln!("slot: input {line}");
        }
        for node in pick_devices(dev, sys) {
            eprintln!(
                "slot: input {} ({})",
                node.display(),
                device_name(sys, &node)
            );
            quicken_poll(sys, &node);
            let queue = pending.clone();
            let steers = stick.clone();
            let name = node.display().to_string();
            let trace = trace.clone();
            #[cfg(feature = "flip")]
            let stick_only =
                super::evdev::is_gamepad_node(&sys.join(node.file_name().unwrap()).join("device"));
            #[cfg(not(feature = "flip"))]
            let stick_only = false;
            let spawned = std::thread::Builder::new()
                .name(format!("slot-input-{name}"))
                .spawn(move || read_node(&node, &queue, &steers, trace.as_deref(), stick_only));
            if let Err(e) = spawned {
                eprintln!("slot: input {name}: {e}");
            }
        }
        DeviceInput {
            pending,
            stick,
            hall: find_hall(Path::new(PSY)),
            lid_shut: None,
            next_lid_poll: 0,
        }
    }
}

/// The two stick clicks, which only the gamepad node reports.
///
/// The gamepad node also repeats START (0x13b), MENU (0x13c, BTN_MODE) and SELECT
/// (0x13a). Those must stay unmapped here: the keyboard node already reports them,
/// and a second button arriving with MENU held reads as a chord and cancels the tap
/// and the hold that ejects.
#[cfg_attr(not(feature = "flip"), allow(dead_code))]
fn click(code: u16) -> Option<Btn> {
    Some(match code {
        0x13d => Btn::L3, // BTN_THUMBL
        0x13e => Btn::R3, // BTN_THUMBR
        _ => return None,
    })
}

const POLL_MS: u32 = 10;

fn quicken_poll(sys: &Path, node: &Path) {
    let Some(name) = node.file_name() else {
        return;
    };
    let attr = sys.join(name).join("device/poll");
    let Some(now) = std::fs::read_to_string(&attr)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
    else {
        return;
    };
    if now > POLL_MS {
        match std::fs::write(&attr, POLL_MS.to_string()) {
            Ok(()) => eprintln!(
                "slot: input {}: poll {now} ms -> {POLL_MS} ms",
                node.display()
            ),
            Err(e) => eprintln!("slot: input {}: poll: {e}", node.display()),
        }
    }
}

fn read_node(
    node: &Path,
    queue: &Mutex<Vec<RawEvent>>,
    steers: &AtomicBool,
    trace: Option<&Trace>,
    stick_only: bool,
) {
    let _ = stick_only;
    let label = node
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| node.display().to_string());
    let mut hat = Hat::default();
    #[cfg(feature = "flip")]
    let mut stick = Stick::default();
    #[cfg(feature = "flip")]
    let mut left = LeftStick::default();
    let mut file = match File::open(node) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("slot: input {}: {e}", node.display());
            return;
        }
    };
    let mut buf = [0u8; EVENT_BYTES * 16];
    loop {
        let read = match file.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                eprintln!("slot: input {}: {e}", node.display());
                return;
            }
        };
        let mut events = Vec::new();
        for ev in buf[..read].chunks_exact(EVENT_BYTES).filter_map(decode) {
            if let Some(trace) = trace {
                trace.event(&label, ev);
            }
            // Experimental Mini Plus builds have no proven button table yet:
            // every key's raw code and what it mapped to goes in the log, so a
            // tester's report identifies each physical button instead of us
            // guessing from actions that happened to do something.
            #[cfg(feature = "miyoomini")]
            if ev.kind == EV_KEY {
                eprintln!(
                    "slot: key {label} code={} value={} -> {}",
                    ev.code,
                    ev.value,
                    code_to_btn(ev.code)
                        .map_or_else(|| "unmapped".to_string(), |b| format!("{b:?}"))
                );
            }
            if (ev.kind == EV_KEY && ev.value == 1) || (ev.kind == EV_ABS && ev.value != 0) {
                crate::latency::read();
            }
            #[cfg(feature = "flip")]
            if stick_only {
                // Gamepad node: its buttons and hat duplicate the gpio keyboard node,
                // so only the right stick (fast-forward) and the two stick clicks come
                // from here; nothing else reports those.
                if ev.kind == EV_KEY {
                    if let Some(btn) = click(ev.code) {
                        match ev.value {
                            1 => events.push(RawEvent::Down(btn)),
                            0 => events.push(RawEvent::Up(btn)),
                            _ => {}
                        }
                    }
                }
                if ev.kind == EV_ABS {
                    super::sticks::feed(ev.code, ev.value);
                    events.extend(stick.feed(ev));
                    // The left stick as a D-pad, unless a game is reading the
                    // sticks itself.
                    let steering = steers.load(Ordering::Relaxed) && !super::sticks::taken();
                    events.extend(left.feed(ev, steering));
                }
                continue;
            }
            match ev.kind {
                EV_ABS => {
                    hat.steer_with_stick(steers.load(Ordering::Relaxed));
                    events.extend(hat.feed(ev));
                }
                _ => events.extend(to_raw(ev)),
            }
        }
        if events.is_empty() {
            continue;
        }
        queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend(events);
    }
}

struct Trace {
    out: Mutex<File>,
    began: Instant,
}

impl Trace {
    fn start(root: &Path, dev: &Path, sys: &Path) -> Option<Arc<Trace>> {
        let dir = crate::root::logs_dir(root);
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(trace::TRACE_FILE);
        let mut file = match File::create(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("slot: {}: {e}", path.display());
                return None;
            }
        };
        for line in trace::survey(dev, sys) {
            let _ = writeln!(file, "{line}");
        }
        let _ = writeln!(file, "--");
        let _ = file.flush();
        Some(Arc::new(Trace {
            out: Mutex::new(file),
            began: Instant::now(),
        }))
    }

    fn event(&self, node: &str, ev: Ev) {
        if ev.kind == EV_SYN {
            return;
        }
        let line = trace::event_line(node, self.began.elapsed().as_millis(), ev);
        let mut out = self.out.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }
}

impl DeviceInput {
    pub fn steer_with_stick(&self, on: bool) {
        self.stick.store(on, Ordering::Relaxed);
    }

    fn read_lid(&self) -> Option<bool> {
        let raw = std::fs::read_to_string(self.hall.as_ref()?).ok()?;
        match raw.trim() {
            "0" => Some(true),
            "1" => Some(false),
            _ => None,
        }
    }
}

impl InputSource for DeviceInput {
    fn poll(&mut self, now: Millis) -> Vec<RawEvent> {
        let mut out = std::mem::take(&mut *self.pending.lock().unwrap_or_else(|e| e.into_inner()));
        if self.hall.is_some() && now >= self.next_lid_poll {
            self.next_lid_poll = now + LID_POLL_MS;
            if let Some(shut) = self.read_lid() {
                if self.lid_shut != Some(shut) {
                    self.lid_shut = Some(shut);
                    out.push(if shut {
                        RawEvent::Down(Btn::Lid)
                    } else {
                        RawEvent::Up(Btn::Lid)
                    });
                }
            }
        }
        out
    }
}

// The Miyoo Flip exposes its lid magnet here (0 = closed, 1 = open), not as a
// power-supply hallkey. Checked first so the Flip gets lid events.
const FLIP_HALL: &str = "/sys/devices/platform/hall-mh248/hallvalue";

fn find_hall(psy: &Path) -> Option<PathBuf> {
    let flip = PathBuf::from(FLIP_HALL);
    if flip.is_file() {
        return Some(flip);
    }
    let mut supplies: Vec<PathBuf> = std::fs::read_dir(psy)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    supplies.sort();
    supplies
        .into_iter()
        .map(|d| d.join("hallkey"))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_stick_clicks_are_read_from_the_gamepad_node() {
        assert_eq!(click(0x13d), Some(Btn::L3));
        assert_eq!(click(0x13e), Some(Btn::R3));
        // SELECT, START and MENU (BTN_MODE) arrive here as well; mapping any of them
        // would put a second button down while MENU is held.
        for code in [0x13a, 0x13b, 0x13c] {
            assert_eq!(click(code), None, "{code:#x} is not a stick click");
        }
    }
}
