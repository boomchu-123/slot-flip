//! User-overridable button map.
//!
//! The compiled defaults in [`super::evdev::code_to_btn`] cover the devices we
//! ship for, but any MinUI handheld routes its buttons through evdev with its
//! own kernel key codes. `Slot/Config/buttons.ini` lets a user remap without a
//! rebuild, and the **Buttons** screen edits the same file from inside the app.
//! Each line is `<kernel-code> = <button-name>`, e.g.
//!
//! ```ini
//! # Right shoulder sends KEY_PAGEDOWN (109); bind it to R1.
//! 109 = r1
//! 104 = l2
//! 57 = none     # switch a key off
//! ```
//!
//! Codes may be decimal (`109`) or hex (`0x135`). Button names are the ones
//! from [`Btn::from_name`] (a, b, x, y, up, down, left, right, l1, r1, l2, r2,
//! start, select, menu, volup, voldown, power, lid), or `none` to disable a key.
//! Any code not listed keeps its built-in mapping, so a partial file only
//! overrides what it names.
//!
//! The map is live: [`rebind`] takes effect on the very next key event, and
//! [`save`] writes it back to the file.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use slot_input::Btn;

use super::evdev::code_to_btn;

pub const BUTTONS_FILE: &str = "Config/buttons.ini";

/// Linux key codes live below this; everything we scan for is under it.
const SCAN: u16 = 0x300;

const HEADER: &str = "\
# Button remap for Slot.
#
# Edited from inside Slot (Menu > Buttons). Slot rewrites this file when you
# change a binding there, so hand-written comments are not kept.
#
# Each line is: <kernel-key-code> = <button>
# Codes may be decimal (109) or hex (0x135). Any code you do NOT list keeps the
# built-in mapping for your device, so only changes are listed here.
#
# Button names:
#   up down left right  a b x y  l1 r1 l2 r2  start select menu  volup voldown
# Use `none` to switch a key off:   57 = none
#
# Delete a line (or the whole file) to go back to the built-in mapping.
";

/// What a listed key code does.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Bind {
    To(Btn),
    Off,
}

/// How a rebind changed things.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// The key already was that button; nothing changed.
    Already,
    /// The key now does the button. `swapped` is the button that used to own the
    /// key and has been given the key(s) the first button gave up.
    Bound { swapped: Option<Btn> },
}

/// The user's overrides on top of a defaults function. Pure, so the whole
/// remapping logic can be tested without a device.
#[derive(Default, Clone, PartialEq, Debug)]
pub struct Overrides(HashMap<u16, Bind>);

impl Overrides {
    pub fn parse(text: &str) -> Self {
        let mut out = HashMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty()
                || line.starts_with('#')
                || line.starts_with(';')
                || line.starts_with('[')
            {
                continue;
            }
            let Some((code, name)) = line.split_once('=') else {
                continue;
            };
            let Some(code) = parse_code(code.trim()) else {
                continue;
            };
            let name = name.split('#').next().unwrap_or("").trim();
            let bind = match name.to_ascii_lowercase().as_str() {
                "none" | "off" | "unbound" => Bind::Off,
                _ => match Btn::from_name(name) {
                    Some(btn) => Bind::To(btn),
                    None => continue,
                },
            };
            out.insert(code, bind);
        }
        Overrides(out)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// What `code` does, given the built-in map `defaults`.
    pub fn resolve(&self, code: u16, defaults: fn(u16) -> Option<Btn>) -> Option<Btn> {
        match self.0.get(&code) {
            Some(Bind::To(btn)) => Some(*btn),
            Some(Bind::Off) => None,
            None => defaults(code),
        }
    }

    /// Every key code that currently does `btn`, lowest first.
    pub fn keys_for(&self, btn: Btn, defaults: fn(u16) -> Option<Btn>) -> Vec<u16> {
        (0..SCAN)
            .filter(|&c| self.resolve(c, defaults) == Some(btn))
            .collect()
    }

    /// Make `code` the key for `btn`, and `btn` have that key only.
    ///
    /// Whatever button used to own `code` is not left without a key: it takes
    /// over the key(s) `btn` gave up, so two buttons trade places. If `code`
    /// belonged to nothing, `btn`'s old keys are simply switched off.
    pub fn rebind(&mut self, btn: Btn, code: u16, defaults: fn(u16) -> Option<Btn>) -> Outcome {
        let owner = self.resolve(code, defaults);
        if owner == Some(btn) {
            return Outcome::Already;
        }
        let given_up: Vec<u16> = self
            .keys_for(btn, defaults)
            .into_iter()
            .filter(|&c| c != code)
            .collect();
        self.0.insert(code, Bind::To(btn));
        for old in given_up {
            self.0.insert(
                old,
                match owner {
                    Some(prev) => Bind::To(prev),
                    None => Bind::Off,
                },
            );
        }
        self.prune(defaults);
        Outcome::Bound { swapped: owner }
    }

    pub fn reset(&mut self) {
        self.0.clear();
    }

    /// Drop entries that just restate the built-in map, so the file lists only
    /// real changes.
    fn prune(&mut self, defaults: fn(u16) -> Option<Btn>) {
        self.0.retain(|&code, bind| match (*bind, defaults(code)) {
            (Bind::To(b), Some(d)) => b != d,
            (Bind::Off, None) => false,
            _ => true,
        });
    }

    pub fn to_text(&self) -> String {
        let mut lines: Vec<(u16, Bind)> = self.0.iter().map(|(&c, &b)| (c, b)).collect();
        lines.sort_by_key(|&(c, _)| c);
        let mut out = String::from(HEADER);
        out.push('\n');
        for (code, bind) in lines {
            let name = match bind {
                Bind::To(btn) => btn.as_name(),
                Bind::Off => "none",
            };
            out.push_str(&format!("{code} = {name}    # {}\n", key_name(code)));
        }
        out
    }
}

fn parse_code(s: &str) -> Option<u16> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u16>().ok()
    }
}

/// A short, human label for a kernel key code. Keyboard-range codes get their
/// usual names; anything else is shown as a number so it can still be told apart.
pub fn key_name(code: u16) -> String {
    const LETTERS: [(u16, &str); 26] = [
        (30, "A"),
        (48, "B"),
        (46, "C"),
        (32, "D"),
        (18, "E"),
        (33, "F"),
        (34, "G"),
        (35, "H"),
        (23, "I"),
        (36, "J"),
        (37, "K"),
        (38, "L"),
        (50, "M"),
        (49, "N"),
        (24, "O"),
        (25, "P"),
        (16, "Q"),
        (19, "R"),
        (31, "S"),
        (20, "T"),
        (22, "U"),
        (47, "V"),
        (17, "W"),
        (45, "X"),
        (21, "Y"),
        (44, "Z"),
    ];
    if let Some((_, n)) = LETTERS.iter().find(|(c, _)| *c == code) {
        return format!("Key {n}");
    }
    let named = match code {
        1 => "Esc",
        14 => "Backspace",
        15 => "Tab",
        28 => "Enter",
        29 => "Left Ctrl",
        42 => "Left Shift",
        54 => "Right Shift",
        56 => "Left Alt",
        57 => "Space",
        97 => "Right Ctrl",
        100 => "Right Alt",
        102 => "Home",
        103 => "Up",
        104 => "Page Up",
        105 => "Left",
        106 => "Right",
        107 => "End",
        108 => "Down",
        109 => "Page Down",
        114 => "Vol -",
        115 => "Vol +",
        116 => "Power",
        _ => "",
    };
    if !named.is_empty() {
        return named.to_string();
    }
    match code {
        2..=10 => format!("Key {}", code - 1),
        11 => "Key 0".to_string(),
        0x100.. => format!("Pad {code:#x}"),
        _ => format!("Key #{code}"),
    }
}

// ---------------------------------------------------------------------------
// The live map.
// ---------------------------------------------------------------------------

struct Live {
    overrides: Overrides,
    root: Option<PathBuf>,
}

static LIVE: LazyLock<Mutex<Live>> = LazyLock::new(|| {
    Mutex::new(Live {
        overrides: Overrides::default(),
        root: None,
    })
});

fn live() -> std::sync::MutexGuard<'static, Live> {
    LIVE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Load `Config/buttons.ini`. Call early in startup, before input opens. Calling
/// it again re-reads the file.
pub fn load(root: &Path) {
    let text = std::fs::read_to_string(root.join(BUTTONS_FILE)).unwrap_or_default();
    let mut live = live();
    live.overrides = Overrides::parse(&text);
    live.root = Some(root.to_path_buf());
}

/// What a key code does right now: the user's binding, else the device default.
pub fn resolve(code: u16) -> Option<Btn> {
    live().overrides.resolve(code, code_to_btn)
}

/// The keys currently bound to `btn`, lowest code first.
pub fn keys_for(btn: Btn) -> Vec<u16> {
    live().overrides.keys_for(btn, code_to_btn)
}

/// Whether anything differs from the built-in map.
pub fn customised() -> bool {
    !live().overrides.is_empty()
}

/// Rebind `btn` to `code` and save. The change is live even if saving fails.
pub fn rebind(btn: Btn, code: u16) -> (Outcome, io::Result<()>) {
    let outcome = live().overrides.rebind(btn, code, code_to_btn);
    let saved = if outcome == Outcome::Already {
        Ok(())
    } else {
        save()
    };
    (outcome, saved)
}

/// Go back to the built-in map and save.
pub fn reset() -> io::Result<()> {
    live().overrides.reset();
    save()
}

/// Write the current overrides to `Config/buttons.ini`. Does nothing when no
/// root was loaded (host runs and tests).
pub fn save() -> io::Result<()> {
    let (text, path) = {
        let live = live();
        let Some(root) = &live.root else {
            return Ok(());
        };
        (live.overrides.to_text(), root.join(BUTTONS_FILE))
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Flushed to the card, not just handed to the kernel: the FAT card has no
    // journal and the device is often powered off right after a change, so a
    // plain write can be lost without any error.
    slot_store::atomic_write(&path, text.as_bytes())
}

// ---------------------------------------------------------------------------
// Capturing the next physical key, for the Buttons screen.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Capture {
    armed: bool,
    got: Option<u16>,
    /// The captured key's release must not reach the app as a stray button-up.
    swallow_up: Option<u16>,
}

static CAPTURE: Mutex<Capture> = Mutex::new(Capture {
    armed: false,
    got: None,
    swallow_up: None,
});

fn capture() -> std::sync::MutexGuard<'static, Capture> {
    CAPTURE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Start listening: the next ordinary key press is held back from the app and
/// handed to [`capture_take`] instead.
pub fn capture_arm() {
    let mut c = capture();
    c.armed = true;
    c.got = None;
}

/// Stop listening without a result.
pub fn capture_cancel() {
    let mut c = capture();
    c.armed = false;
    c.got = None;
}

/// The key captured since [`capture_arm`], if one was pressed.
pub fn capture_take() -> Option<u16> {
    capture().got.take()
}

/// Called for every key event by the evdev reader. Returns true when the event
/// was consumed by a capture and must not be delivered as a button.
///
/// Power, volume and lid keys are never captured, so they keep working (and can
/// never be bound away) while the screen is waiting for a key.
pub fn capture_offer(code: u16, value: i32) -> bool {
    let mut c = capture();
    if c.armed && value == 1 {
        let system = matches!(
            live().overrides.resolve(code, code_to_btn),
            Some(Btn::Power | Btn::VolUp | Btn::VolDown | Btn::Lid)
        );
        if system {
            return false;
        }
        c.armed = false;
        c.got = Some(code);
        c.swallow_up = Some(code);
        return true;
    }
    if c.swallow_up == Some(code) && value != 1 {
        if value == 0 {
            c.swallow_up = None;
        }
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // Defaults standing in for a device: 57 -> A, 29 -> B, 15 -> L1, 116 -> power.
    fn defaults(code: u16) -> Option<Btn> {
        match code {
            57 => Some(Btn::A),
            29 => Some(Btn::B),
            15 => Some(Btn::L1),
            116 => Some(Btn::Power),
            _ => None,
        }
    }

    #[test]
    fn parses_decimal_and_hex() {
        let m = Overrides::parse("109 = r1\n0x135 = l1\n# comment\nbad line\n57=a\n");
        assert_eq!(m.resolve(109, defaults), Some(Btn::R1));
        assert_eq!(m.resolve(0x135, defaults), Some(Btn::L1));
        assert_eq!(m.0.len(), 3);
    }

    #[test]
    fn ignores_unknown_names() {
        let m = Overrides::parse("1 = nope\n2 = b\n");
        assert_eq!(m.resolve(2, defaults), Some(Btn::B));
        assert_eq!(m.0.len(), 1);
    }

    #[test]
    fn none_switches_a_default_key_off() {
        let m = Overrides::parse("57 = none\n");
        assert_eq!(m.resolve(57, defaults), None);
        assert_eq!(
            m.resolve(29, defaults),
            Some(Btn::B),
            "other keys untouched"
        );
    }

    #[test]
    fn trailing_comments_are_ignored() {
        let m = Overrides::parse("109 = r1    # Page Down\n57 = none # space\n");
        assert_eq!(m.resolve(109, defaults), Some(Btn::R1));
        assert_eq!(m.resolve(57, defaults), None);
    }

    #[test]
    fn rebinding_an_unowned_key_moves_the_button_to_it() {
        let mut m = Overrides::default();
        assert_eq!(
            m.rebind(Btn::A, 200, defaults),
            Outcome::Bound { swapped: None }
        );
        assert_eq!(m.keys_for(Btn::A, defaults), vec![200]);
        assert_eq!(m.resolve(57, defaults), None, "A's old key is switched off");
    }

    #[test]
    fn rebinding_onto_an_owned_key_swaps_the_two_buttons() {
        let mut m = Overrides::default();
        // Put A on B's key.
        assert_eq!(
            m.rebind(Btn::A, 29, defaults),
            Outcome::Bound {
                swapped: Some(Btn::B)
            }
        );
        assert_eq!(m.resolve(29, defaults), Some(Btn::A));
        assert_eq!(m.resolve(57, defaults), Some(Btn::B), "B took A's old key");
        assert_eq!(m.keys_for(Btn::A, defaults), vec![29]);
        assert_eq!(m.keys_for(Btn::B, defaults), vec![57]);
    }

    #[test]
    fn rebinding_to_the_same_key_changes_nothing() {
        let mut m = Overrides::default();
        assert_eq!(m.rebind(Btn::A, 57, defaults), Outcome::Already);
        assert!(m.is_empty());
    }

    #[test]
    fn no_button_is_ever_left_without_a_key() {
        let mut m = Overrides::default();
        m.rebind(Btn::A, 29, defaults);
        m.rebind(Btn::L1, 57, defaults);
        m.rebind(Btn::B, 15, defaults);
        for b in [Btn::A, Btn::B, Btn::L1] {
            assert_eq!(
                m.keys_for(b, defaults).len(),
                1,
                "{b:?} must keep exactly one key"
            );
        }
    }

    #[test]
    fn swapping_back_restores_the_defaults_and_an_empty_file() {
        let mut m = Overrides::default();
        m.rebind(Btn::A, 29, defaults);
        m.rebind(Btn::A, 57, defaults);
        assert!(m.is_empty(), "back to defaults should list nothing: {m:?}");
    }

    #[test]
    fn the_saved_text_round_trips() {
        let mut m = Overrides::default();
        m.rebind(Btn::A, 29, defaults);
        m.rebind(Btn::L1, 200, defaults);
        let again = Overrides::parse(&m.to_text());
        assert_eq!(again, m);
        assert!(m.to_text().contains("<kernel-key-code> = <button>"));
    }

    #[test]
    fn key_names_are_readable() {
        assert_eq!(key_name(57), "Space");
        assert_eq!(key_name(30), "Key A");
        assert_eq!(key_name(2), "Key 1");
        assert_eq!(key_name(0x130), "Pad 0x130");
        assert_eq!(key_name(250), "Key #250");
    }

    // The global map and the capture are process-wide, so everything that
    // touches them lives in one test to stay deterministic.
    #[test]
    fn live_map_saves_reloads_and_captures() {
        let dir = tempfile::tempdir().expect("tempdir");
        load(dir.path());
        assert!(!customised());

        // A device default is still visible through the live map.
        let default_for_a = keys_for(Btn::A);
        assert!(!default_for_a.is_empty(), "A has a built-in key");

        let spare = (0x2f0..SCAN)
            .find(|&c| resolve(c).is_none())
            .expect("an unused code");
        let (outcome, saved) = rebind(Btn::A, spare);
        assert!(saved.is_ok());
        assert!(matches!(outcome, Outcome::Bound { .. }));
        assert_eq!(keys_for(Btn::A), vec![spare]);
        assert!(customised());

        // It reached the file, and a fresh load sees it.
        let text = std::fs::read_to_string(dir.path().join(BUTTONS_FILE)).unwrap();
        assert!(text.contains(&format!("{spare} = a")), "{text}");
        let left_over: Vec<_> = std::fs::read_dir(dir.path().join("Config"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n != "buttons.ini")
            .collect();
        assert!(
            left_over.is_empty(),
            "save left temp files behind: {left_over:?}"
        );
        load(dir.path());
        assert_eq!(keys_for(Btn::A), vec![spare]);

        // On the device path the live binding must beat the built-in default.
        #[cfg(feature = "flip")]
        {
            use super::super::evdev::{to_raw, Ev, EV_KEY};
            use slot_input::RawEvent;
            let press = Ev {
                kind: EV_KEY,
                code: spare,
                value: 1,
            };
            assert_eq!(to_raw(press), Some(RawEvent::Down(Btn::A)));
        }

        // Capture holds back the next ordinary press and its release, once.
        capture_arm();
        assert!(capture_offer(spare, 1), "press is captured");
        assert!(capture_offer(spare, 0), "its release is swallowed");
        assert!(!capture_offer(spare, 0), "only once");
        assert_eq!(capture_take(), Some(spare));
        assert_eq!(capture_take(), None);
        assert!(!capture_offer(spare, 1), "disarmed after one key");

        // System keys are never captured.
        capture_arm();
        let power = (0..SCAN).find(|&c| resolve(c) == Some(Btn::Power));
        if let Some(power) = power {
            assert!(!capture_offer(power, 1), "power passes through");
        }
        capture_cancel();
        assert!(!capture_offer(spare, 1));

        reset().expect("reset");
        assert!(!customised());
        assert_eq!(keys_for(Btn::A), default_for_a);
    }
}
