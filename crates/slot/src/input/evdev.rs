use std::path::{Path, PathBuf};

use slot_input::{Btn, RawEvent};

/// `struct input_event` is a `timeval` followed by type, code and value. The
/// timeval is two longs, so a 32-bit kernel (the Mini Plus and the A30) hands
/// out 16-byte events and a 64-bit one 24. Decoding 32-bit reads as 24-byte
/// events drops a lone press and garbles the rest.
#[cfg(target_pointer_width = "32")]
pub const EVENT_BYTES: usize = 16;
#[cfg(not(target_pointer_width = "32"))]
pub const EVENT_BYTES: usize = 24;

const HEAD: usize = EVENT_BYTES - 8;

pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_SW: u16 = 0x05;
pub const SW_LID: u16 = 0x00;

pub const EV_ABS: u16 = 0x03;
pub const ABS_HAT0X: u16 = 0x10;
pub const ABS_HAT0Y: u16 = 0x11;

// Right analogue stick, used on the Flip to drive fast-forward since the
// physical R2 shoulder is repurposed to stand in for a dead R1.
#[cfg(feature = "flip")]
pub const ABS_RX: u16 = 0x03;
#[cfg(feature = "flip")]
const STICK_ON: i32 = 20_000;
#[cfg(feature = "flip")]
const STICK_OFF: i32 = 12_000;

#[derive(Default)]
pub struct Hat {
    x: i32,
    y: i32,
}

impl Hat {
    pub fn feed(&mut self, ev: Ev) -> Vec<RawEvent> {
        let (held, ends) = match ev.code {
            ABS_HAT0X => (&mut self.x, [Btn::Left, Btn::Right]),
            ABS_HAT0Y => (&mut self.y, [Btn::Up, Btn::Down]),
            _ => return Vec::new(),
        };
        let (was, now) = (*held, ev.value.signum());
        if now == was {
            return Vec::new();
        }
        *held = now;
        let btn = |v: i32| ends[usize::from(v > 0)];
        let mut out = Vec::new();
        if was != 0 {
            out.push(RawEvent::Up(btn(was)));
        }
        if now != 0 {
            out.push(RawEvent::Down(btn(now)));
        }
        out
    }
}

// Turns the right stick's horizontal axis into R2 presses (fast-forward) with a
// little hysteresis so a resting stick doesn't chatter. Pushing right past
// STICK_ON presses R2; releasing below STICK_OFF lets it go. Left is ignored.
#[cfg(feature = "flip")]
#[derive(Default)]
pub struct Stick {
    held: bool,
}

#[cfg(feature = "flip")]
impl Stick {
    pub fn feed(&mut self, ev: Ev) -> Vec<RawEvent> {
        if ev.code != ABS_RX {
            return Vec::new();
        }
        if !self.held && ev.value >= STICK_ON {
            self.held = true;
            return vec![RawEvent::Down(Btn::R2)];
        }
        if self.held && ev.value < STICK_OFF {
            self.held = false;
            return vec![RawEvent::Up(Btn::R2)];
        }
        Vec::new()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Ev {
    pub kind: u16,
    pub code: u16,
    pub value: i32,
}

pub fn decode(bytes: &[u8]) -> Option<Ev> {
    let packet: &[u8; EVENT_BYTES] = bytes.get(..EVENT_BYTES)?.try_into().ok()?;
    Some(Ev {
        kind: u16::from_le_bytes([packet[HEAD], packet[HEAD + 1]]),
        code: u16::from_le_bytes([packet[HEAD + 2], packet[HEAD + 3]]),
        value: i32::from_le_bytes([
            packet[HEAD + 4],
            packet[HEAD + 5],
            packet[HEAD + 6],
            packet[HEAD + 7],
        ]),
    })
}

pub fn to_raw(ev: Ev) -> Option<RawEvent> {
    let btn = match ev.kind {
        // A user override in Config/buttons.ini wins over the built-in map, so
        // any device can rebind its keys without a rebuild.
        EV_KEY => super::keymap::override_for(ev.code).or_else(|| code_to_btn(ev.code))?,
        EV_SW if ev.code == SW_LID => Btn::Lid,
        _ => return None,
    };
    match ev.value {
        0 => Some(RawEvent::Up(btn)),
        1 => Some(RawEvent::Down(btn)),
        _ => None,
    }
}

// The Anbernic RG SP (H700) pad speaks BTN_* gamepad codes.
#[cfg(not(any(feature = "flip", feature = "miyoomini", feature = "my282")))]
pub fn code_to_btn(code: u16) -> Option<Btn> {
    Some(match code {
        0x130 => Btn::A,
        0x131 => Btn::B,
        0x132 => Btn::Y,
        0x133 => Btn::X,
        0x134 => Btn::L1,
        0x135 => Btn::R1,
        0x136 => Btn::Select,
        0x137 => Btn::Start,
        0x138 => Btn::Menu,
        0x13a => Btn::L2,
        0x13b => Btn::R2,
        115 => Btn::VolUp,
        114 => Btn::VolDown,
        116 => Btn::Power,
        _ => return None,
    })
}

#[cfg(not(any(feature = "flip", feature = "miyoomini", feature = "my282")))]
const WANTED: [u16; 15] = [
    0x130, 0x131, 0x132, 0x133, 0x134, 0x135, 0x136, 0x137, 0x138, 0x13a, 0x13b, 0x162, 115, 114,
    116,
];

// The Miyoo A30 (my282) routes its buttons through the gpio keyboard node as
// standard Linux key codes. MinUI's platform.h for the A30 uses the SDL/joystick
// path rather than raw codes, so these are a best-effort default following the
// common Allwinner/Miyoo layout (shared with the Mini Plus) — UNVERIFIED on
// hardware. A tester's DEBUG trace confirms them; until then they can be
// corrected without a rebuild via Config/buttons.ini. EXPERIMENTAL — see
// TESTERS.md.
#[cfg(feature = "my282")]
pub fn code_to_btn(code: u16) -> Option<Btn> {
    Some(match code {
        103 => Btn::Up,
        108 => Btn::Down,
        105 => Btn::Left,
        106 => Btn::Right,
        57 => Btn::A,      // KEY_SPACE
        29 => Btn::B,      // KEY_LEFTCTRL
        42 => Btn::X,      // KEY_LEFTSHIFT
        56 => Btn::Y,      // KEY_LEFTALT
        18 => Btn::L1,     // KEY_E
        20 => Btn::R1,     // KEY_T
        15 => Btn::L2,     // KEY_TAB
        14 => Btn::R2,     // KEY_BACKSPACE
        97 => Btn::Select, // KEY_RIGHTCTRL
        28 => Btn::Start,  // KEY_ENTER
        1 => Btn::Menu,    // KEY_ESC
        115 => Btn::VolUp,
        114 => Btn::VolDown,
        116 => Btn::Power,
        _ => return None,
    })
}

#[cfg(feature = "my282")]
const WANTED: [u16; 18] = [
    103, 108, 105, 106, 57, 29, 42, 56, 18, 20, 15, 14, 97, 28, 1, 115, 114, 116,
];

// The Miyoo Mini Plus (miyoomini) routes its buttons through the gpio keyboard
// node as standard Linux key codes. Codes taken from MinUI's own
// workspace/miyoomini/platform/platform.h. Unlike the Flip, the Mini Plus has
// working L1/R1/L2/R2 and no analogue stick or lid, so fast-forward stays on R2
// and there is no stick/hall handling. The face-button codes happen to match
// the Flip's remapped values, so A/B/X/Y already line up with the printed
// labels. EXPERIMENTAL: unverified on hardware — see TESTERS.md.
#[cfg(feature = "miyoomini")]
pub fn code_to_btn(code: u16) -> Option<Btn> {
    Some(match code {
        103 => Btn::Up,
        108 => Btn::Down,
        105 => Btn::Left,
        106 => Btn::Right,
        57 => Btn::A,      // KEY_SPACE
        29 => Btn::B,      // KEY_LEFTCTRL
        42 => Btn::X,      // KEY_LEFTSHIFT
        56 => Btn::Y,      // KEY_LEFTALT
        18 => Btn::L1,     // KEY_E
        20 => Btn::R1,     // KEY_T
        15 => Btn::L2,     // KEY_TAB
        14 => Btn::R2,     // KEY_BACKSPACE
        97 => Btn::Select, // KEY_RIGHTCTRL
        28 => Btn::Start,  // KEY_ENTER
        1 => Btn::Menu,    // KEY_ESC
        115 => Btn::VolUp,
        114 => Btn::VolDown,
        116 => Btn::Power,
        _ => return None,
    })
}

#[cfg(feature = "miyoomini")]
const WANTED: [u16; 18] = [
    103, 108, 105, 106, 57, 29, 42, 56, 18, 20, 15, 14, 97, 28, 1, 115, 114, 116,
];

// The Miyoo Flip (my355) routes every working button through the gpio
// "keyboard" node (event0) as standard Linux key codes. The gamepad node
// (event5) is missing R1 entirely and reports L2/R2 only as analogue axes, so
// event0 is the reliable source. R1 is physically dead on this unit, so it is
// deliberately left unmapped; its actions are recovered from R2 higher up.
#[cfg(feature = "flip")]
pub fn code_to_btn(code: u16) -> Option<Btn> {
    Some(match code {
        103 => Btn::Up,
        108 => Btn::Down,
        105 => Btn::Left,
        106 => Btn::Right,
        // The pad's kernel A/B/X/Y labels are rotated versus the printed
        // letters on this unit, so they are mapped to match what is printed.
        57 => Btn::A,          // KEY_SPACE      (bottom-printed A behaves as confirm)
        29 => Btn::B,          // KEY_LEFTCTRL
        42 => Btn::X,          // KEY_LEFTSHIFT
        56 => Btn::Y,          // KEY_LEFTALT
        15 => Btn::L1,         // KEY_TAB
        104 => Btn::L2,        // KEY_PAGEUP  (rewind)
        // R1 is physically dead on this unit; the working R2 shoulder takes its
        // place so platform-switch, aspect-reset and Select+R save state all
        // keep working. Fast-forward (normally R2) moves to the right stick.
        109 => Btn::R1,        // KEY_PAGEDOWN (acts as R1)
        97 => Btn::Select,     // KEY_RIGHTCTRL
        28 => Btn::Start,      // KEY_ENTER
        1 => Btn::Menu,        // KEY_ESC
        115 => Btn::VolUp,
        114 => Btn::VolDown,
        116 => Btn::Power,
        _ => return None,
    })
}

#[cfg(feature = "flip")]
const WANTED: [u16; 16] = [
    103, 108, 105, 106, 29, 57, 56, 42, 15, 104, 109, 97, 28, 1, 115, 114,
];

pub use slot_power::has_bit;

/// First key code of the range the kernel uses for gamepad and joystick buttons.
#[cfg(feature = "my282")]
const BTN_MISC: u16 = 0x100;
#[cfg(feature = "my282")]
const BTN_JOYSTICK: u16 = 0x120;
#[cfg(feature = "my282")]
const KEY_MAX: u16 = 0x2ff;

#[cfg(feature = "my282")]
fn has_gamepad_keys(keys: &str) -> bool {
    (BTN_MISC..=KEY_MAX).any(|code| has_bit(keys, code))
}

/// The key codes of a gamepad node in the order SDL numbers its buttons: from
/// BTN_JOYSTICK up, then BTN_MISC up to it. MinUI reads the A30's pad through
/// SDL, so its button table is in these numbers, not in key codes.
#[cfg(feature = "my282")]
pub fn joystick_order(keys: &str) -> Vec<u16> {
    joystick_order_in(keys, usize::BITS as u16)
}

/// As `joystick_order`, for a mask printed in words of `width` bits.
#[cfg(feature = "my282")]
pub fn joystick_order_in(keys: &str, width: u16) -> Vec<u16> {
    (BTN_JOYSTICK..=KEY_MAX)
        .chain(BTN_MISC..BTN_JOYSTICK)
        .filter(|code| slot_power::has_bit_in(keys, *code, width))
        .collect()
}

/// MinUI's SDL button numbers for the A30 (its `platform.h`, `JOY_*`).
#[cfg(feature = "my282")]
pub fn joystick_btn(number: usize) -> Option<Btn> {
    Some(match number {
        0 => Btn::A,
        1 => Btn::B,
        2 => Btn::Y,
        3 => Btn::X,
        4 => Btn::L1,
        5 => Btn::R1,
        6 => Btn::Select,
        7 => Btn::Start,
        8 => Btn::Menu,
        9 => Btn::L2,
        10 => Btn::R2,
        13 => Btn::Up,
        14 => Btn::Left,
        15 => Btn::Right,
        16 => Btn::Down,
        17 => Btn::VolDown,
        18 => Btn::VolUp,
        _ => return None,
    })
}

/// The pad's buttons as raw events, for a node whose codes are in `order`.
#[cfg(feature = "my282")]
pub fn joystick_raw(order: &[u16], ev: Ev) -> Option<RawEvent> {
    if ev.kind != EV_KEY {
        return None;
    }
    let btn = joystick_btn(order.iter().position(|code| *code == ev.code)?)?;
    match ev.value {
        0 => Some(RawEvent::Up(btn)),
        1 => Some(RawEvent::Down(btn)),
        _ => None,
    }
}

/// The key codes this node reports that SDL would number, or `None` if it is
/// not a gamepad.
#[cfg(feature = "my282")]
pub fn joystick_node(device: &Path) -> Option<Vec<u16>> {
    let keys = std::fs::read_to_string(device.join("capabilities/key")).ok()?;
    has_gamepad_keys(&keys).then(|| joystick_order(&keys))
}

pub fn pick_devices(dev: &Path, sys: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dev) else {
        return Vec::new();
    };
    let mut nodes: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("event"))
        })
        .collect();
    nodes.sort();
    nodes.retain(|node| {
        let Some(name) = node.file_name() else {
            return false;
        };
        wanted_node(&sys.join(name).join("device"))
    });
    nodes
}

fn wanted_node(device: &Path) -> bool {
    let cap = |file: &str| std::fs::read_to_string(device.join("capabilities").join(file));
    let keys = cap("key").unwrap_or_default();
    let lid = cap("sw").is_ok_and(|sw| has_bit(&sw, SW_LID));
    if lid || WANTED.iter().any(|bit| has_bit(&keys, *bit)) {
        return true;
    }
    // On the Flip the buttons arrive on the gpio keyboard node, but the right
    // stick (used for fast-forward) lives on the gamepad node, which advertises
    // analogue axes. Accept any node that reports ABS so the stick is read too.
    #[cfg(feature = "flip")]
    if cap("abs").is_ok_and(|abs| abs.trim().chars().any(|c| c != '0' && c != ' ')) {
        return true;
    }
    // The A30's face buttons are not on its keyboard node: MinUI reads them
    // through SDL, so they are gamepad buttons (and perhaps a hat) on a node of
    // their own, which the keyboard codes above never match.
    #[cfg(feature = "my282")]
    if has_gamepad_keys(&keys)
        || cap("abs").is_ok_and(|abs| abs.trim().chars().any(|c| c != '0' && c != ' '))
    {
        return true;
    }
    false
}

// On the Flip, both the gpio keyboard node (event0) and the gamepad node
// (event5) report every button, so reading both double-counts every press.
// We take buttons + D-pad from event0 and use event5 only for its right
// analogue stick. This flags the gamepad node: it is the one that advertises
// BTN_SOUTH (0x130), which the keyboard node never does.
#[cfg(feature = "flip")]
pub fn is_gamepad_node(device: &Path) -> bool {
    let keys = std::fs::read_to_string(device.join("capabilities/key")).unwrap_or_default();
    has_bit(&keys, 0x130)
}

pub fn device_name(sys: &Path, node: &Path) -> String {
    node.file_name()
        .and_then(|n| std::fs::read_to_string(sys.join(n).join("device/name")).ok())
        .map(|n| n.trim().to_string())
        .unwrap_or_else(|| node.display().to_string())
}
