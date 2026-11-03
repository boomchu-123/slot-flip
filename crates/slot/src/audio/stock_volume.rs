//! The Mini Plus' hardware volume while slot owns the controls.
//!
//! The firmware's `keymon` cannot run beside slot: it mistakes the L2 shoulder
//! used for rewind for volume-down. The launcher stops it while slot runs, so
//! volume keys update both the AO gain and the firmware's saved step here.

use std::io::Write;
use std::path::PathBuf;

/// The console's own volume file.
pub const SYSTEM_FILE: &str = "/appconfigs/system.json";

/// The AO debug node accepts the same gain command the firmware uses.
pub const AO_FILE: &str = "/proc/mi_modules/mi_ao/mi_ao0";

/// The highest step the firmware's volume keys reach.
pub const STEPS: u8 = 20;

const MIN_DB: i32 = -60;
const FIRST_DB: i32 = -58;
const MAX_DB: i32 = -3;

/// The console's volume step from the text of `system.json`.
pub fn parse(text: &str) -> Option<u8> {
    // `"vol"` with its quotes, so `"bgmvol"` is not mistaken for it.
    let after = &text[text.find("\"vol\"")? + "\"vol\"".len()..];
    let after = after.trim_start().strip_prefix(':')?.trim_start();
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<u8>().ok().map(|step| step.min(STEPS))
}

/// A step as the percentage the volume bar draws.
pub fn percent(step: u8) -> u8 {
    (step.min(STEPS) as u16 * 100 / STEPS as u16) as u8
}

/// The gain measured from the stock handler: mute at zero, then -58 dB at step
/// one through -3 dB at step twenty. Intermediate values are linearly spaced.
fn gain(step: u8) -> i32 {
    let step = step.min(STEPS);
    if step == 0 {
        return MIN_DB;
    }
    let span = MAX_DB - FIRST_DB;
    FIRST_DB + (span * i32::from(step - 1) + i32::from(STEPS - 1) / 2)
        / i32::from(STEPS - 1)
}

fn replace_step(text: &str, step: u8) -> Option<String> {
    let at = text.find("\"vol\"")? + "\"vol\"".len();
    let colon = at + text[at..].find(':')? + 1;
    let begin = colon + text[colon..].find(|c: char| !c.is_whitespace())?;
    let end = begin
        + text[begin..]
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(text.len() - begin);
    Some(format!("{}{}{}", &text[..begin], step, &text[end..]))
}

/// Slot's view of the console's volume while the firmware handler is stopped.
pub struct Control {
    system: PathBuf,
    ao: PathBuf,
    step: u8,
}

impl Control {
    pub fn new(system: impl Into<PathBuf>, ao: impl Into<PathBuf>) -> Self {
        let system = system.into();
        let step = std::fs::read_to_string(&system)
            .ok()
            .and_then(|text| parse(&text))
            .unwrap_or(STEPS);
        Control {
            system,
            ao: ao.into(),
            step,
        }
    }

    /// Moves one stock step and writes both views of the setting. The in-memory
    /// value still moves if a write fails, so the HUD answers the key immediately;
    /// the errors are in the hardware log rather than hidden.
    pub fn adjust(&mut self, up: bool) {
        self.step = if up {
            self.step.saturating_add(1).min(STEPS)
        } else {
            self.step.saturating_sub(1)
        };
        if let Err(e) = self.write_system() {
            eprintln!("slot: volume: system step not saved: {e}");
        }
        if let Err(e) = self.write_gain() {
            eprintln!("slot: volume: hardware gain not set: {e}");
        }
    }

    pub fn percent(&self) -> u8 {
        percent(self.step)
    }

    fn write_system(&self) -> Result<(), String> {
        let text = std::fs::read_to_string(&self.system).map_err(|e| e.to_string())?;
        let text = replace_step(&text, self.step).ok_or("vol is missing")?;
        std::fs::write(&self.system, text).map_err(|e| e.to_string())
    }

    fn write_gain(&self) -> Result<(), String> {
        let db = gain(self.step);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&self.ao)
            .map_err(|e| e.to_string())?;
        writeln!(file, "set_ao_volume 0 {db}dB").map_err(|e| e.to_string())?;
        writeln!(file, "set_ao_mute {}", u8::from(self.step == 0)).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "{\n\t\"vol\":\t9,\n\t\"keymap\":\t\"L2,L,R2,R,X,A,B,Y\",\n\t\"mute\":\t1,\n\t\"bgmvol\":\t8,\n}";

    #[test]
    fn reads_the_step_and_not_the_music_volume() {
        assert_eq!(parse(FILE), Some(9));
        assert_eq!(parse("{\"bgmvol\": 8}"), None);
        assert_eq!(parse("{\"vol\":20}"), Some(20));
        assert_eq!(parse("{\"vol\" : 0 }"), Some(0));
    }

    #[test]
    fn the_bar_runs_from_empty_to_full() {
        assert_eq!(percent(0), 0);
        assert_eq!(percent(10), 50);
        assert_eq!(percent(STEPS), 100);
    }

    #[test]
    fn measured_stock_gain_points_are_kept() {
        assert_eq!(gain(0), -60);
        assert_eq!(gain(1), -58);
        assert_eq!(gain(8), -38);
        assert_eq!(gain(9), -35);
        assert_eq!(gain(10), -32);
        assert_eq!(gain(20), -3);
    }

    #[test]
    fn replacing_the_step_keeps_the_rest_of_the_firmware_file() {
        let replaced = replace_step(FILE, 12).unwrap();
        assert_eq!(parse(&replaced), Some(12));
        assert!(replaced.contains("\"bgmvol\":\t8"));
        assert!(replaced.contains("\"keymap\""));
    }

    #[test]
    fn control_starts_at_the_saved_step_and_moves_both_outputs() {
        let dir =
            std::env::temp_dir().join(format!("slot-stock-volume-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let system = dir.join("system.json");
        let ao = dir.join("ao");
        std::fs::write(&system, FILE).unwrap();
        std::fs::write(&ao, "").unwrap();
        let mut control = Control::new(&system, &ao);
        assert_eq!(control.percent(), 45, "the bar started full");
        control.adjust(true);
        assert_eq!(control.percent(), 50);
        assert_eq!(parse(&std::fs::read_to_string(&system).unwrap()), Some(10));
        let commands = std::fs::read_to_string(&ao).unwrap();
        assert!(commands.contains("set_ao_volume 0 -32dB"));
        assert!(commands.contains("set_ao_mute 0"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
