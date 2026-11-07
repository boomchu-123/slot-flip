//! The Mini Plus' volume belongs to the console, not to slot.
//!
//! The firmware's `keymon` listens to the same volume keys slot does and moves
//! the output gain itself, keeping the step in `system.json`. A second volume
//! inside slot stacks on top of that one: the bar stops at 100 while the sound
//! keeps getting louder, and a gain left at its lowest step leaves slot quiet
//! however far its own bar is raised. So on this console slot plays at full
//! scale and only shows where the console's volume is.

use std::io::Write;
use std::path::PathBuf;

/// The console's own volume file.
pub const SYSTEM_FILE: &str = "/appconfigs/system.json";

/// The output-gain node changed by the firmware's key monitor.
pub const AO_FILE: &str = "/proc/mi_modules/mi_ao/mi_ao0";

/// The highest step the firmware's volume keys reach.
pub const STEPS: u8 = 20;

/// How long after a key press to keep looking for the console's answer. The
/// firmware writes its step a moment after the key goes down, and keeps writing
/// while the key is held.
const WATCH_MS: u64 = 1_500;
const POLL_MS: u64 = 50;
/// `keymon` can write once more just after L2 is released.
const REWIND_SETTLE_MS: u64 = 500;

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

fn ao_db(text: &str) -> Option<i32> {
    let line = text.lines().find(|line| line.contains("LineOut"))?;
    let field = line.split_whitespace().nth(7)?;
    let digits: String = field
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    digits.parse().ok()
}

fn with_step(text: &str, step: u8) -> Option<String> {
    let at = text.find("\"vol\"")? + "\"vol\"".len();
    let colon = at + text[at..].find(':')? + 1;
    let begin = colon + text[colon..].find(|c: char| !c.is_whitespace())?;
    let end = begin
        + text[begin..]
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(text.len() - begin);
    Some(format!("{}{}{}", &text[..begin], step, &text[end..]))
}

struct RewindVolume {
    step: u8,
    db: Option<i32>,
    release_at: Option<u64>,
}

/// Follows the console's volume after each key press.
pub struct Follow {
    path: PathBuf,
    ao: PathBuf,
    step: Option<u8>,
    until: u64,
    next: u64,
    rewind: Option<RewindVolume>,
}

impl Follow {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self::with_ao(path, AO_FILE)
    }

    pub fn with_ao(path: impl Into<PathBuf>, ao: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let step = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| parse(&text));
        Follow {
            path,
            ao: ao.into(),
            step,
            until: 0,
            next: 0,
            rewind: None,
        }
    }

    fn read(&mut self) -> Option<u8> {
        let text = std::fs::read_to_string(&self.path).ok()?;
        parse(&text)
    }

    /// A volume key went down: look for the console's answer for a while.
    pub fn pressed(&mut self, now: u64) {
        self.until = now + WATCH_MS;
        self.next = now + POLL_MS;
    }

    /// Remembers the real volume while L2 is held. The stock key monitor also
    /// sees that shoulder and sometimes mistakes it for volume-down; keeping the
    /// exact saved file and gain lets slot undo only that accidental change.
    pub fn rewind_started(&mut self, now: u64) {
        let Ok(system) = std::fs::read_to_string(&self.path) else {
            return;
        };
        let Some(step) = parse(&system) else {
            return;
        };
        let db = std::fs::read_to_string(&self.ao)
            .ok()
            .and_then(|text| ao_db(&text));
        self.step = Some(step);
        self.rewind = Some(RewindVolume {
            step,
            db,
            release_at: None,
        });
        self.next = now + POLL_MS;
    }

    pub fn rewind_stopped(&mut self, now: u64) {
        if let Some(rewind) = &mut self.rewind {
            rewind.release_at = Some(now + REWIND_SETTLE_MS);
        }
    }

    /// Looks again if a press is still being waited on. True if the console's
    /// step moved.
    pub fn poll(&mut self, now: u64) -> bool {
        if now < self.next || (now >= self.until && self.rewind.is_none()) {
            return false;
        }
        self.next = now + POLL_MS;
        let finish_rewind = self
            .rewind
            .as_ref()
            .and_then(|rewind| rewind.release_at)
            .is_some_and(|at| now >= at);
        self.protect_rewind_volume();
        if finish_rewind {
            self.rewind = None;
        }
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

    fn protect_rewind_volume(&mut self) {
        let Some(rewind) = &self.rewind else {
            return;
        };
        if let Ok(system) = std::fs::read_to_string(&self.path) {
            if parse(&system) != Some(rewind.step) {
                let restored = with_step(&system, rewind.step);
                if let Some(restored) = restored {
                    if let Err(e) = std::fs::write(&self.path, restored) {
                        eprintln!("slot: volume: could not restore the rewind volume: {e}");
                    } else {
                        eprintln!("slot: volume: restored step {} after rewind", rewind.step);
                    }
                }
            }
        }
        let gain_moved = rewind.db.is_some()
            && std::fs::read_to_string(&self.ao)
                .ok()
                .and_then(|text| ao_db(&text))
                != rewind.db;
        if gain_moved {
            if let Err(e) = self.restore_gain(rewind.db.unwrap(), rewind.step == 0) {
                eprintln!("slot: volume: could not restore the rewind gain: {e}");
            }
        }
        self.step = Some(rewind.step);
    }

    fn restore_gain(&self, db: i32, muted: bool) -> std::io::Result<()> {
        let mut ao = std::fs::OpenOptions::new().write(true).open(&self.ao)?;
        writeln!(ao, "set_ao_volume 0 {db}dB")?;
        writeln!(ao, "set_ao_mute {}", u8::from(muted))
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
    fn replacing_volume_does_not_replace_another_firmware_setting() {
        let text = "{\"vol\": 8, \"brightness\": 1}";
        let restored = with_step(text, 10).unwrap();
        assert_eq!(parse(&restored), Some(10));
        assert!(restored.contains("\"brightness\": 1"));
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

    #[test]
    fn an_l2_volume_change_is_restored_without_owning_volume_normally() {
        let dir = std::env::temp_dir().join(format!("slot-rewind-volume-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let system = dir.join("system.json");
        let ao = dir.join("ao");
        std::fs::write(&system, "{\"vol\": 10, \"brightness\": 7}").unwrap();
        std::fs::write(
            &ao,
            "AoDev SampR BitWidth SondMod PtNum bMute VolumeDb bStart\nLineOut 48000 16bit stereo 768 0 (-32 ,-32 ) 1\n",
        )
        .unwrap();
        let mut f = Follow::with_ao(&system, &ao);
        f.rewind_started(1_000);

        std::fs::write(&system, "{\"vol\": 8, \"brightness\": 7}").unwrap();
        std::fs::write(
            &ao,
            "AoDev SampR BitWidth SondMod PtNum bMute VolumeDb bStart\nLineOut 48000 16bit stereo 768 0 (-38 ,-38 ) 1\n",
        )
        .unwrap();
        assert!(!f.poll(1_000 + POLL_MS));
        assert_eq!(parse(&std::fs::read_to_string(&system).unwrap()), Some(10));
        let commands = std::fs::read_to_string(&ao).unwrap();
        assert!(commands.contains("set_ao_volume 0 -32dB"));
        assert!(commands.contains("set_ao_mute 0"));
        assert_eq!(f.percent(), 50);

        f.rewind_stopped(1_100);
        assert!(!f.poll(1_100 + REWIND_SETTLE_MS));
        std::fs::write(&system, "{\"vol\": 9}").unwrap();
        assert!(!f.poll(1_100 + REWIND_SETTLE_MS + POLL_MS));
        assert_eq!(parse(&std::fs::read_to_string(&system).unwrap()), Some(9));
        std::fs::remove_dir_all(&dir).ok();
    }
}
