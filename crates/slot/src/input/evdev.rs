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
pub const ABS_LSX: u16 = 0x02;
pub const ABS_LSY: u16 = 0x03;

pub const STICK_DEADZONE: i32 = 2048;

const LEFT_STICK_TARGETS: [&str; 6] = [
    "rg34xxsp",
    "rg35xxh",
    "rg35xxpro",
    "rg40xxh",
    "rg40xxv",
    "rgcubexx",
];

pub fn has_left_stick(target: &str) -> bool {
    LEFT_STICK_TARGETS.contains(&target)
}

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
    hat: [i32; 2],
    stick: [i32; 2],
    steers: bool,
}

impl Hat {
    pub fn steer_with_stick(&mut self, on: bool) {
        self.steers = on;
    }

    pub fn feed(&mut self, ev: Ev) -> Vec<RawEvent> {
        let tilt = |v: i32| match v {
            v if v > STICK_DEADZONE => 1,
            v if v < -STICK_DEADZONE => -1,
            _ => 0,
        };
        let steer = |v: i32| if self.steers { tilt(v) } else { 0 };
        let (axis, from_stick, value) = match ev.code {
            ABS_HAT0X => (0, false, ev.value.signum()),
            ABS_HAT0Y => (1, false, ev.value.signum()),
            ABS_LSX => (0, true, steer(ev.value)),
            ABS_LSY => (1, true, steer(ev.value)),
            _ => return Vec::new(),
        };
        let was = self.held(axis);
        match from_stick {
            true => self.stick[axis] = value,
            false => self.hat[axis] = value,
        }
        let now = self.held(axis);
        if now == was {
            return Vec::new();
        }
        let ends = [[Btn::Left, Btn::Right], [Btn::Up, Btn::Down]][axis];
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

    fn held(&self, axis: usize) -> i32 {
        match self.hat[axis] {
            0 => self.stick[axis],
            hat => hat,
        }
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
        // A game reading the sticks owns this one; it is not also a button.
        if super::sticks::taken() {
            return match std::mem::take(&mut self.held) {
                true => vec![RawEvent::Up(Btn::R2)],
                false => Vec::new(),
            };
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

// The Flip's left stick (ABS_X / ABS_Y on the gamepad node), as a D-pad, for games
// that only read the pad. `Hat` does this for other devices but uses their axis
// codes (2 and 3); on the Flip 3 is the right stick's X and the left stick is 0 and
// 1, and its gamepad node is read by a branch that never reaches `Hat`, so the
// Left Stick setting did nothing here. These sticks also rest well off centre (a
// right stick at rest was measured at about -5100), so a direction needs a firm
// push to start and a real release to end, not `STICK_DEADZONE`.
#[cfg(feature = "flip")]
pub const ABS_LX: u16 = 0x00;
#[cfg(feature = "flip")]
pub const ABS_LY: u16 = 0x01;
#[cfg(feature = "flip")]
const DPAD_ON: i32 = 14_000;
#[cfg(feature = "flip")]
const DPAD_OFF: i32 = 9_000;

#[cfg(feature = "flip")]
#[derive(Default)]
pub struct LeftStick {
    held: [i32; 2],
}

#[cfg(feature = "flip")]
impl LeftStick {
    /// `steering` is false when the setting is off, or when a game is reading the
    /// sticks itself; then a held direction is let go rather than left pressed.
    pub fn feed(&mut self, ev: Ev, steering: bool) -> Vec<RawEvent> {
        let axis = match ev.code {
            ABS_LX => 0,
            ABS_LY => 1,
            _ => return Vec::new(),
        };
        let was = self.held[axis];
        let now = if steering {
            match ev.value {
                v if v >= DPAD_ON => 1,
                v if v <= -DPAD_ON => -1,
                v if was > 0 && v >= DPAD_OFF => 1,
                v if was < 0 && v <= -DPAD_OFF => -1,
                _ => 0,
            }
        } else {
            0
        };
        if now == was {
            return Vec::new();
        }
        self.held[axis] = now;
        let ends = [[Btn::Left, Btn::Right], [Btn::Up, Btn::Down]][axis];
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
        // The Buttons screen can be waiting for the next physical key; if so it
        // takes this one and the app never sees it as a button.
        EV_KEY if super::keymap::capture_offer(ev.code, ev.value) => return None,
        // The live map (the built-in one plus the user's Config/buttons.ini and
        // whatever they remapped in the Buttons screen) decides what a key is.
        EV_KEY => super::keymap::resolve(ev.code)?,
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
        57 => Btn::A,   // KEY_SPACE      (bottom-printed A behaves as confirm)
        29 => Btn::B,   // KEY_LEFTCTRL
        42 => Btn::X,   // KEY_LEFTSHIFT
        56 => Btn::Y,   // KEY_LEFTALT
        15 => Btn::L1,  // KEY_TAB
        104 => Btn::L2, // KEY_PAGEUP  (rewind)
        // R1 is physically dead on this unit; the working R2 shoulder takes its
        // place so platform-switch, aspect-reset and Select+R save state all
        // keep working. Fast-forward (normally R2) moves to the right stick.
        109 => Btn::R1,    // KEY_PAGEDOWN (acts as R1)
        97 => Btn::Select, // KEY_RIGHTCTRL
        28 => Btn::Start,  // KEY_ENTER
        1 => Btn::Menu,    // KEY_ESC
        115 => Btn::VolUp,
        114 => Btn::VolDown,
        116 => Btn::Power,
        _ => return None,
    })
}

// 116 is the power key. On the Flip it is the only key on the pmic's own node
// (`rk805 pwrkey`), so leaving it out meant slot never opened that node and a held
// power button reached nothing: MinUI's keymon used to catch it instead.
#[cfg(feature = "flip")]
const WANTED: [u16; 17] = [
    103, 108, 105, 106, 29, 57, 56, 42, 15, 104, 109, 97, 28, 1, 115, 114, 116,
];

pub use slot_power::has_bit;

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

#[cfg(all(test, feature = "flip"))]
mod flip_left_stick {
    use super::*;

    fn push(s: &mut LeftStick, code: u16, value: i32) -> Vec<RawEvent> {
        s.feed(
            Ev {
                kind: EV_ABS,
                code,
                value,
            },
            true,
        )
    }

    #[test]
    fn a_firm_push_presses_the_direction_and_a_release_lets_it_go() {
        let mut s = LeftStick::default();
        assert_eq!(
            push(&mut s, ABS_LX, 20_000),
            vec![RawEvent::Down(Btn::Right)]
        );
        assert_eq!(push(&mut s, ABS_LX, 0), vec![RawEvent::Up(Btn::Right)]);
        assert_eq!(push(&mut s, ABS_LY, -20_000), vec![RawEvent::Down(Btn::Up)]);
        assert_eq!(
            push(&mut s, ABS_LY, 30_000),
            vec![RawEvent::Up(Btn::Up), RawEvent::Down(Btn::Down)]
        );
    }

    #[test]
    fn a_stick_resting_off_centre_presses_nothing() {
        let mut s = LeftStick::default();
        // What the Flip's sticks were measured doing at rest.
        for v in [-5_100, 1_400, -4_400, 900, 5_000] {
            assert!(push(&mut s, ABS_LX, v).is_empty(), "{v}");
            assert!(push(&mut s, ABS_LY, v).is_empty(), "{v}");
        }
    }

    #[test]
    fn it_does_not_chatter_around_the_edge_of_a_press() {
        let mut s = LeftStick::default();
        assert_eq!(
            push(&mut s, ABS_LX, 15_000),
            vec![RawEvent::Down(Btn::Right)]
        );
        // Easing back, but not past the release point, keeps it held.
        assert!(push(&mut s, ABS_LX, 11_000).is_empty());
        assert!(push(&mut s, ABS_LX, 13_999).is_empty());
        assert_eq!(push(&mut s, ABS_LX, 8_000), vec![RawEvent::Up(Btn::Right)]);
    }

    #[test]
    fn the_right_stick_and_other_axes_are_not_the_dpad() {
        let mut s = LeftStick::default();
        assert!(
            push(&mut s, ABS_RX, 32_000).is_empty(),
            "right stick x is R2, not a direction"
        );
        assert!(push(&mut s, 0x04, 32_000).is_empty());
        assert!(push(&mut s, 0x10, 1).is_empty());
    }

    #[test]
    fn it_lets_go_when_steering_stops() {
        let mut s = LeftStick::default();
        push(&mut s, ABS_LX, 25_000);
        // The setting was switched off, or a game took the sticks.
        let out = s.feed(
            Ev {
                kind: EV_ABS,
                code: ABS_LX,
                value: 25_000,
            },
            false,
        );
        assert_eq!(out, vec![RawEvent::Up(Btn::Right)]);
    }
}
