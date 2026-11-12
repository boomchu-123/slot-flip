//! The Mini Plus' volume belongs to the console, not to slot.
//!
//! The firmware's `keymon` listens to the same volume keys slot does and moves
//! the output gain itself, keeping the step in `system.json`. A second volume
//! inside slot stacks on top of that one: the bar stops at 100 while the sound
//! keeps getting louder, and a gain left at its lowest step leaves slot quiet
//! however far its own bar is raised. So on this console slot plays at full
//! scale and only shows where the console's volume is.

use std::path::PathBuf;

/// The console's own volume file.
pub const SYSTEM_FILE: &str = "/appconfigs/system.json";

/// The highest step the firmware's volume keys reach.
pub const STEPS: u8 = 20;

/// How long after a key press to keep looking for the console's answer. The
/// firmware writes its step a moment after the key goes down, and keeps writing
/// while the key is held.
const WATCH_MS: u64 = 1_500;
const POLL_MS: u64 = 50;

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

/// Follows the console's volume after each key press.
pub struct Follow {
    path: PathBuf,
    step: Option<u8>,
    until: u64,
    next: u64,
}

impl Follow {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let step = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| parse(&text));
        Follow {
            path,
            step,
            until: 0,
            next: 0,
        }
    }

    fn read(&mut self) -> Option<u8> {
        let text = std::fs::read_to_string(&self.path).ok()?;
        parse(&text)
    }

    /// A volume key went down: look for the console's answer for a while.
    pub fn pressed(&mut self, now: u64) {
        if self.step.is_none() {
            self.step = self.read();
        }
        self.until = now + WATCH_MS;
        self.next = now + POLL_MS;
    }

    /// Looks again if a press is still being waited on. True if the console's
    /// step moved.
    pub fn poll(&mut self, now: u64) -> bool {
        if now >= self.until || now < self.next {
            return false;
        }
        self.next = now + POLL_MS;
        match self.read() {
            Some(step) if Some(step) != self.step => {
                self.step = Some(step);
                true
            }
            _ => false,
        }
    }

    /// Where the bar sits. Full if the file cannot be read, which is how the
    /// sound has always been before this was followed.
    pub fn percent(&self) -> u8 {
        self.step.map_or(100, percent)
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
    fn a_step_past_the_top_or_an_unreadable_one_is_not_trusted_blindly() {
        assert_eq!(parse("{\"vol\": 99}"), Some(STEPS));
        assert_eq!(parse("{\"vol\": }"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn the_bar_runs_from_empty_to_full() {
        assert_eq!(percent(0), 0);
        assert_eq!(percent(10), 50);
        assert_eq!(percent(STEPS), 100);
        assert_eq!(percent(STEPS + 5), 100);
    }

    #[test]
    fn follows_the_file_for_a_while_after_a_press_and_then_stops() {
        let dir = std::env::temp_dir().join(format!("slot-stock-volume-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("system.json");
        std::fs::write(&file, "{\"vol\": 4}").unwrap();
        let mut f = Follow::new(&file);
        assert_eq!(f.percent(), 20, "the saved step was not read at startup");
        assert!(!f.poll(0), "nothing is watched before a press");
        f.pressed(1_000);
        assert_eq!(f.percent(), 20);
        std::fs::write(&file, "{\"vol\": 5}").unwrap();
        assert!(!f.poll(1_000), "too soon after the press to look again");
        assert!(f.poll(1_000 + POLL_MS));
        assert_eq!(f.percent(), 25);
        assert!(!f.poll(1_000 + POLL_MS * 2), "unchanged is not news");
        std::fs::write(&file, "{\"vol\": 6}").unwrap();
        assert!(!f.poll(1_000 + WATCH_MS + 1), "the watch has ended");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_file_leaves_the_bar_full() {
        let mut f = Follow::new("/nonexistent/slot-system.json");
        f.pressed(0);
        assert_eq!(f.percent(), 100);
        assert!(!f.poll(POLL_MS));
    }
}
