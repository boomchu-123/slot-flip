// End-to-end checks for the user-facing config added for the spec:
// configurable background, starter files, and the INI button remap.

use std::path::Path;

use slot::wallpaper;
use slot_input::Btn;
use tempfile::TempDir;

fn empty_root() -> TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    slot::root::ensure(d.path());
    d
}

fn write_png(root: &Path, name: &str) {
    // Not a real PNG; wallpaper::pick only checks the extension and that the
    // file exists, so a stub is enough to exercise selection.
    std::fs::write(root.join("Wallpapers").join(name), b"stub").expect("write wallpaper");
}

#[test]
fn ensure_writes_documented_starter_configs() {
    let d = empty_root();
    let slot_ini = d.path().join("Config/slot.ini");
    let buttons_ini = d.path().join("Config/buttons.ini");
    assert!(slot_ini.is_file(), "slot.ini was not created");
    assert!(buttons_ini.is_file(), "buttons.ini was not created");

    let slot_text = std::fs::read_to_string(&slot_ini).unwrap();
    assert!(
        slot_text.contains("background = random"),
        "slot.ini missing the documented default background"
    );
    assert!(
        std::fs::read_to_string(&buttons_ini)
            .unwrap()
            .contains("<kernel-key-code> = <button>"),
        "buttons.ini missing its format hint"
    );
}

#[test]
fn ensure_never_clobbers_edited_config() {
    let d = empty_root();
    let slot_ini = d.path().join("Config/slot.ini");
    std::fs::write(&slot_ini, "background = mine.png\n").unwrap();
    // A second ensure (as happens on every boot) must leave edits alone.
    slot::root::ensure(d.path());
    assert_eq!(
        std::fs::read_to_string(&slot_ini).unwrap(),
        "background = mine.png\n"
    );
}

#[test]
fn background_honours_a_named_file() {
    let d = empty_root();
    write_png(d.path(), "alpha.png");
    write_png(d.path(), "beta.png");
    std::fs::write(d.path().join("Config/slot.ini"), "background = beta.png\n").unwrap();

    // The seed would otherwise pick alpha (sorts first); the name wins.
    let picked = wallpaper::pick(d.path(), 0).expect("a background");
    assert_eq!(picked.file_name().unwrap(), "beta.png");
}

#[test]
fn background_random_deals_every_wallpaper_before_repeating() {
    let d = empty_root();
    for name in ["a.png", "b.png", "c.png"] {
        write_png(d.path(), name);
    }
    std::fs::write(d.path().join("Config/slot.ini"), "background = random\n").unwrap();

    // Upstream v1.6 deals a shuffled deck, so every wallpaper is shown once
    // before any repeats.
    let mut seen: Vec<String> = Vec::new();
    for _ in 0..3 {
        let name = wallpaper::pick(d.path(), 0)
            .expect("a background")
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(!seen.contains(&name), "{name} repeated before the deck ran out");
        seen.push(name);
    }
    seen.sort();
    assert_eq!(seen, ["a.png", "b.png", "c.png"]);

    // The deck refills once it is empty, so a fourth pick still works.
    assert!(wallpaper::pick(d.path(), 0).is_some());
}

#[test]
fn background_falls_back_when_named_file_is_missing() {
    let d = empty_root();
    write_png(d.path(), "only.png");
    std::fs::write(d.path().join("Config/slot.ini"), "background = gone.png\n").unwrap();
    let picked = wallpaper::pick(d.path(), 0).expect("a fallback background");
    assert_eq!(picked.file_name().unwrap(), "only.png");
}

#[test]
fn background_none_when_folder_is_empty() {
    let d = empty_root();
    assert!(wallpaper::pick(d.path(), 0).is_none());
}

#[test]
fn choices_lists_available_wallpapers_sorted() {
    let d = empty_root();
    write_png(d.path(), "zed.png");
    write_png(d.path(), "amy.png");
    assert_eq!(wallpaper::choices(d.path()), vec!["amy.png", "zed.png"]);
}

#[test]
fn button_names_round_trip() {
    for b in Btn::ALL {
        assert_eq!(Btn::from_name(b.as_name()), Some(b), "{b:?}");
    }
    // A few forgiving aliases.
    assert_eq!(Btn::from_name("L"), Some(Btn::L1));
    assert_eq!(Btn::from_name("HOME"), Some(Btn::Menu));
    assert_eq!(Btn::from_name("nope"), None);
}
