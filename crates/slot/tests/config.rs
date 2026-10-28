// End-to-end checks for the user-facing config: starter files, carrying an old
// pinned background across to the menu's wallpaper setting, and button names.

use std::path::Path;

use slot::wallpaper::carry_over_pin;
use slot_input::Btn;
use slot_store::{read_slot_state, SlotState, WallpaperPick};
use tempfile::TempDir;

fn empty_root() -> TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    slot::root::ensure(d.path());
    d
}

fn write_png(root: &Path, name: &str) {
    // Not a real PNG; the migration only checks that the file exists.
    std::fs::write(root.join("Wallpapers").join(name), b"stub").expect("write wallpaper");
}

fn pin(root: &Path, text: &str) {
    std::fs::write(root.join("Config/slot.ini"), text).expect("write slot.ini");
}

fn pinned(root: &Path) -> Option<String> {
    slot_store::ini::value(root, "Config/slot.ini", "background")
}

#[test]
fn ensure_writes_documented_starter_configs() {
    let d = empty_root();
    let slot_ini = d.path().join("Config/slot.ini");
    let buttons_ini = d.path().join("Config/buttons.ini");
    assert!(slot_ini.is_file(), "slot.ini was not created");
    assert!(buttons_ini.is_file(), "buttons.ini was not created");

    assert!(
        std::fs::read_to_string(&slot_ini)
            .unwrap()
            .contains("Menu > General > Wallpaper"),
        "slot.ini does not say where wallpapers are chosen"
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
fn an_old_pinned_background_becomes_the_menus_wallpaper() {
    let d = empty_root();
    write_png(d.path(), "alpha.png");
    write_png(d.path(), "beta.png");
    pin(d.path(), "background = beta.png\n");

    let mut state = SlotState::default();
    assert!(
        carry_over_pin(d.path(), &mut state),
        "nothing was carried over"
    );
    assert_eq!(state.wallpaper, WallpaperPick::File("beta.png".into()));
}

#[test]
fn the_pin_is_carried_over_only_once() {
    let d = empty_root();
    write_png(d.path(), "beta.png");
    pin(d.path(), "background = beta.png\n");

    let mut state = SlotState::default();
    assert!(carry_over_pin(d.path(), &mut state));
    assert_eq!(
        pinned(d.path()).as_deref(),
        Some("random"),
        "the key was not retired"
    );

    // The player later goes back to the default in the menu. The next boot must
    // not undo that.
    state.wallpaper = WallpaperPick::Default;
    assert!(!carry_over_pin(d.path(), &mut state));
    assert_eq!(state.wallpaper, WallpaperPick::Default);
}

#[test]
fn a_choice_already_made_in_the_menu_wins_over_the_old_pin() {
    let d = empty_root();
    write_png(d.path(), "beta.png");
    pin(d.path(), "background = beta.png\n");

    let mut state = SlotState {
        wallpaper: WallpaperPick::Random,
        ..SlotState::default()
    };
    assert!(!carry_over_pin(d.path(), &mut state));
    assert_eq!(state.wallpaper, WallpaperPick::Random);
    assert_eq!(
        pinned(d.path()).as_deref(),
        Some("random"),
        "left to undo a later choice"
    );
}

#[test]
fn random_empty_and_missing_pins_change_nothing() {
    for text in [
        "background = random\n",
        "background = Random\n",
        "background =\n",
        "\n",
    ] {
        let d = empty_root();
        write_png(d.path(), "beta.png");
        pin(d.path(), text);
        let mut state = SlotState::default();
        assert!(!carry_over_pin(d.path(), &mut state), "{text:?}");
        assert_eq!(state.wallpaper, WallpaperPick::Default, "{text:?}");
    }
}

#[test]
fn a_pin_to_a_file_that_is_gone_is_left_alone() {
    let d = empty_root();
    pin(d.path(), "background = gone.png\n");
    let mut state = SlotState::default();
    assert!(!carry_over_pin(d.path(), &mut state));
    assert_eq!(state.wallpaper, WallpaperPick::Default);
    // Not retired, so it still applies if the file is put back.
    assert_eq!(pinned(d.path()).as_deref(), Some("gone.png"));
    write_png(d.path(), "gone.png");
    assert!(carry_over_pin(d.path(), &mut state));
}

#[test]
fn booting_with_an_old_pin_saves_it_as_the_wallpaper_setting() {
    let d = empty_root();
    write_png(d.path(), "beta.png");
    pin(d.path(), "background = beta.png\n");

    let _app = slot::app::App::boot(d.path());
    assert_eq!(
        read_slot_state(d.path()).wallpaper,
        WallpaperPick::File("beta.png".into()),
        "the pin did not reach slot.state"
    );
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
