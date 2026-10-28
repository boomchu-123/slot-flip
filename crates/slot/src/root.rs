use std::path::{Path, PathBuf};

pub const DIRS: [&str; 29] = [
    "BIOS",
    "Bezels",
    "Bezels/GBA",
    "Bezels/GB",
    "Bezels/GBC",
    "Config",
    "Games",
    "Games/GBA",
    "Games/GB",
    "Games/GBC",
    "Games/PS1",
    "Games/NES",
    "Labels",
    "Labels/GBA",
    "Labels/GB",
    "Labels/GBC",
    "Labels/PS1",
    "Labels/NES",
    "Logs",
    "Saves",
    "Saves/GBA",
    "Saves/GB",
    "Saves/GBC",
    "States",
    "States/GBA",
    "States/GB",
    "States/GBC",
    "System",
    "Wallpapers",
];

pub fn ensure(root: &Path) {
    for sub in DIRS {
        let _ = std::fs::create_dir_all(root.join(sub));
    }
    if let Err(e) = slot_store::move_config(root) {
        eprintln!("slot: config: {e}");
    }
    write_starter(root, "Config/slot.ini", STARTER_SLOT_INI);
    write_starter(root, "Config/buttons.ini", STARTER_BUTTONS_INI);
}

/// Write a documented starter config the first time only. Never clobbers a
/// file the user has edited.
fn write_starter(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    if path.exists() {
        return;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, body);
}

const STARTER_SLOT_INI: &str = "\
# Slot settings. Lines beginning with # are ignored.

# Nothing is needed here. Put PNG files in the Wallpapers folder next to this
# one, then choose one in Slot under Menu > General > Wallpaper.
";

const STARTER_BUTTONS_INI: &str = "\
# Button remap for Slot.
#
# You do not need to edit this by hand: open Menu > Controls > Key Map in Slot, pick a
# button and press the key you want for it. Slot writes this file for you.
#
# Each line is: <kernel-key-code> = <button>
# Codes may be decimal (109) or hex (0x135). Any code you do NOT list keeps the
# built-in mapping for your device, so you only need to list what you change.
#
# Button names:
#   up down left right  a b x y  l1 r1 l2 r2  start select menu  volup voldown
# Use `none` to switch a key off:   57 = none
#
# To find your device's codes, run the input trace build and read the log, or
# check your platform's evdev key codes.
#
# Example (Miyoo Flip: the right shoulder sends KEY_PAGEDOWN = 109, bound to R1
# because that unit's R1 is dead):
#   109 = r1
#   104 = l2
";

pub fn sweep_temps(root: &Path) {
    for sub in ["Config", "Saves", "States"] {
        slot_store::sweep_temps(&root.join(sub));
    }
}

pub fn bios_dir(root: &Path) -> PathBuf {
    root.join("BIOS")
}

const BIOS_FILE: &str = "gba_bios.bin";

const BIOS_BYTES: u64 = 16 * 1024;
const BIOS_FIRST_BYTE: u8 = 0x18;

pub fn has_real_bios(root: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(bios_dir(root).join(BIOS_FILE)) else {
        return false;
    };
    if !f.metadata().is_ok_and(|m| m.len() == BIOS_BYTES) {
        return false;
    }
    let mut first = [0u8; 1];
    std::io::Read::read_exact(&mut f, &mut first).is_ok() && first[0] == BIOS_FIRST_BYTE
}

pub fn logs_dir(root: &Path) -> PathBuf {
    root.join("Logs")
}

pub fn saves_dir(root: &Path) -> PathBuf {
    root.join("Saves")
}
