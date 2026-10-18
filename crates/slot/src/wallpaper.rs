use std::path::{Path, PathBuf};

use slot_store::ini;

/// Settings file holding the user's general Slot preferences.
pub const SETTINGS_FILE: &str = "Config/slot.ini";
/// Key in SETTINGS_FILE naming the background image to use.
pub const BACKGROUND_KEY: &str = "background";

fn is_png(p: &Path) -> bool {
    !slot_store::is_hidden(p) && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png"))
}

fn all(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join("Wallpapers"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_png(p))
        .collect();
    files.sort();
    files
}

/// Pick the background image to show.
///
/// If `Config/slot.ini` names a `background` (a filename under `Wallpapers/`),
/// that image is used. The special value `random` (or a missing key) falls back
/// to upstream's shuffled deck, so none repeats until all have been shown.
pub fn pick(root: &Path, seed: u64) -> Option<PathBuf> {
    if let Some(choice) = ini::value(root, SETTINGS_FILE, BACKGROUND_KEY) {
        let choice = choice.trim();
        if !choice.is_empty() && !choice.eq_ignore_ascii_case("random") {
            let named = root.join("Wallpapers").join(choice);
            if named.is_file() {
                return Some(named);
            }
            // Named file is gone: fall through to the shuffle rather than
            // showing nothing.
        }
    }
    let dir = root.join("Wallpapers");
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| !slot_store::is_hidden(p))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")))
        .filter_map(|p| Some(p.file_name()?.to_str()?.to_owned()))
        .collect();
    if files.is_empty() {
        return None;
    }
    files.sort();
    let mut rng = Rng(seed);
    let path = root.join("Config").join("wallpaper.deck");
    let (mut shown, mut queued) = read_deck(&path);
    shown.retain(|n| files.contains(n));
    queued.retain(|n| files.contains(n));
    for n in &files {
        if !shown.contains(n) && !queued.contains(n) {
            let at = rng.below(queued.len() + 1);
            queued.insert(at, n.clone());
        }
    }
    if queued.is_empty() {
        let last = shown.pop();
        queued = files;
        for i in (1..queued.len()).rev() {
            queued.swap(i, rng.below(i + 1));
        }
        if queued.len() > 1 && last.as_ref() == queued.first() {
            let end = queued.len() - 1;
            queued.swap(0, end);
        }
        shown.clear();
    }
    let next = queued.remove(0);
    shown.push(next.clone());
    if let Err(e) = slot_store::atomic_write(&path, write_deck(&shown, &queued).as_bytes()) {
        eprintln!("slot: wallpaper.deck: {e}");
    }
    Some(dir.join(next))
}

fn read_deck(path: &Path) -> (Vec<String>, Vec<String>) {
    let Some(text) = std::fs::read(path)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
    else {
        return (Vec::new(), Vec::new());
    };
    let mut lines = text.lines();
    let Some(shown) = lines.next().and_then(|l| l.parse::<usize>().ok()) else {
        return (Vec::new(), Vec::new());
    };
    let mut order: Vec<String> = lines.map(str::to_owned).collect();
    let queued = order.split_off(shown.min(order.len()));
    (order, queued)
}

fn write_deck(shown: &[String], queued: &[String]) -> String {
    let mut text = format!("{}\n", shown.len());
    for n in shown.iter().chain(queued) {
        text.push_str(n);
        text.push('\n');
    }
    text
}

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        (z % n as u64) as usize
    }
}

/// List the available background filenames (just the file names, not full
/// paths) so a settings menu can offer them.
pub fn choices(root: &Path) -> Vec<String> {
    all(root)
        .iter()
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(str::to_string))
        .collect()
}
