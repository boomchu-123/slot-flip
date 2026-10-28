use std::path::{Path, PathBuf};

use slot_store::{SlotState, WallpaperPick};

pub fn list(root: &Path) -> Vec<String> {
    let mut files = pngs(&root.join("Wallpapers"));
    files.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });
    files
}

pub fn pick(root: &Path, seed: u64) -> Option<PathBuf> {
    let dir = root.join("Wallpapers");
    let mut files = pngs(&dir);
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

fn pngs(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| !slot_store::is_hidden(p))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")))
        .filter_map(|p| Some(p.file_name()?.to_str()?.to_owned()))
        .collect()
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

/// Where fork releases up to v0.3.0 kept a pinned wallpaper: `background = <file>`.
const LEGACY_FILE: &str = "Config/slot.ini";
const LEGACY_KEY: &str = "background";

/// Wallpapers are chosen in the menu now and kept in `slot.state`. Carry an
/// older `background = <file>` pin across once so an upgrade does not quietly
/// change someone's wallpaper.
///
/// The pin is only applied when the player has not already picked something in
/// the menu, and the key is reset either way: left in place, choosing Default
/// later would be undone by the next boot. `random`, a missing key and a file
/// that no longer exists are all left to the menu's own defaults.
///
/// Returns true when `state` changed and should be saved.
pub fn carry_over_pin(root: &Path, state: &mut SlotState) -> bool {
    let Some(pinned) = slot_store::ini::value(root, LEGACY_FILE, LEGACY_KEY) else {
        return false;
    };
    let pinned = pinned.trim();
    if pinned.is_empty() || pinned.eq_ignore_ascii_case("random") {
        return false;
    }
    if !root.join("Wallpapers").join(pinned).is_file() {
        return false;
    }
    if let Err(e) = slot_store::ini::write(root, LEGACY_FILE, LEGACY_KEY, "random") {
        eprintln!("slot: slot.ini: {e}");
    }
    if state.wallpaper != WallpaperPick::Default {
        return false;
    }
    state.wallpaper = WallpaperPick::File(pinned.to_string());
    true
}
