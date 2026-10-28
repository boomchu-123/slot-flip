mod common;

use std::sync::Mutex;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot::input::keymap;
use slot_input::{Action, Btn};
use slot_store::{write_slot_state, SlotState};
use slot_ui::QuickRow;
use tempfile::TempDir;

// The key map and the key capture are process-wide, so these tests take turns.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

/// On the carousel with a clean key map, ready to open the menu.
fn on_carousel() -> (TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..SlotState::default()
        },
    )
    .expect("write slot.state");
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(
        matches!(a.phase(), Phase::Shelf),
        "not on the shelf after boot: {:?}",
        a.phase()
    );
    keymap::load(d.path());
    keymap::reset().expect("reset");
    keymap::capture_cancel();
    a.apply(Action::QuickMenu);
    (d, a)
}

/// Move down `n` rows on the page that is showing.
fn down_on(a: &mut App, page: &[QuickRow], row: QuickRow) {
    let at = page
        .iter()
        .position(|r| *r == row)
        .unwrap_or_else(|| panic!("{row:?} is not on that page"));
    for _ in 0..at {
        press(a, Btn::Down);
    }
    assert_eq!(a.quick_menu(), Some(row), "the bar never reached {row:?}");
}

/// Open Menu > Controls > Key Map > Face, where the face button rows are.
fn on_face_page(a: &mut App) {
    down_on(a, &QuickRow::MAIN, QuickRow::Controls);
    press(a, Btn::A);
    down_on(a, &QuickRow::CONTROL_PAGES, QuickRow::Buttons);
    press(a, Btn::A);
    down_on(a, &QuickRow::KEY_MAP, QuickRow::BindFace);
    press(a, Btn::A);
    assert_eq!(a.quick_menu(), Some(QuickRow::BindA), "Face did not open");
}

/// A key code nothing is bound to.
fn spare_key() -> u16 {
    (0x2f0..0x300)
        .find(|&c| keymap::resolve(c).is_none())
        .expect("an unused key code")
}

#[test]
fn the_face_page_lists_the_four_face_buttons() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (_d, mut a) = on_carousel();
    on_face_page(&mut a);
    assert_eq!(a.quick_menu(), Some(QuickRow::BindA));
}

#[test]
fn the_buttons_page_groups_the_controls_and_fits_one_screen() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (_d, mut a) = on_carousel();
    down_on(&mut a, &QuickRow::MAIN, QuickRow::Controls);
    press(&mut a, Btn::A);
    down_on(&mut a, &QuickRow::CONTROL_PAGES, QuickRow::Buttons);
    press(&mut a, Btn::A);
    // Six groups would overflow; the page is five rows and ends on the reset.
    assert_eq!(QuickRow::KEY_MAP.len(), 5);
    assert_eq!(
        QuickRow::KEY_MAP[4],
        QuickRow::BindReset,
        "Reset belongs at the end"
    );
    for row in QuickRow::KEY_MAP {
        assert_eq!(a.quick_menu(), Some(row));
        press(&mut a, Btn::Down);
    }
    // The menu wraps, so one more Down is back at the top of the page.
    assert_eq!(a.quick_menu(), Some(QuickRow::BindDpad));
}

#[test]
fn pressing_a_key_after_choosing_a_button_rebinds_and_saves_it() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (d, mut a) = on_carousel();
    let spare = spare_key();
    on_face_page(&mut a);

    press(&mut a, Btn::A);
    assert_eq!(a.keymap_waiting(), Some(4), "the A row is not waiting");
    let rev = a.keymap_rev();

    // The reader thread would do this when the key goes down.
    assert!(
        keymap::capture_offer(spare, 1),
        "the press was not captured"
    );
    a.tick_ms(a.now() + 1);

    assert_eq!(
        a.keymap_waiting(),
        None,
        "still waiting after the key arrived"
    );
    assert_ne!(a.keymap_rev(), rev, "the names have to be rebuilt");
    assert_eq!(keymap::keys_for(Btn::A), vec![spare]);
    assert_eq!(a.keymap_labels()[4], keymap::key_name(spare));

    let file = std::fs::read_to_string(d.path().join("Config/buttons.ini")).expect("buttons.ini");
    assert!(file.contains(&format!("{spare} = a")), "{file}");
    keymap::reset().expect("reset");
}

#[test]
fn taking_a_key_that_another_button_has_swaps_the_two() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (_d, mut a) = on_carousel();
    let b_key = keymap::keys_for(Btn::B);
    let a_key = keymap::keys_for(Btn::A);
    assert_eq!((b_key.len(), a_key.len()), (1, 1), "defaults changed");
    on_face_page(&mut a);

    press(&mut a, Btn::A);
    assert!(keymap::capture_offer(b_key[0], 1));
    a.tick_ms(a.now() + 1);

    assert_eq!(keymap::keys_for(Btn::A), b_key);
    assert_eq!(
        keymap::keys_for(Btn::B),
        a_key,
        "B must not be left keyless"
    );
    let labels = a.keymap_labels();
    assert_eq!(labels[4], keymap::key_name(b_key[0]));
    assert_eq!(labels[5], keymap::key_name(a_key[0]));
    keymap::reset().expect("reset");
}

#[test]
fn no_key_within_the_wait_cancels_and_changes_nothing() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (_d, mut a) = on_carousel();
    let before = keymap::keys_for(Btn::A);
    on_face_page(&mut a);

    press(&mut a, Btn::A);
    assert_eq!(a.keymap_waiting(), Some(4));
    a.tick_ms(a.now() + slot::buttons_editor::CAPTURE_MS as u64 + 1);

    assert_eq!(a.keymap_waiting(), None);
    assert_eq!(keymap::keys_for(Btn::A), before);
    assert_eq!(
        a.quick_menu(),
        Some(QuickRow::BindA),
        "the menu left the row on its own"
    );
    assert!(
        !keymap::capture_offer(spare_key(), 1),
        "the key capture was left armed after the timeout"
    );
}

#[test]
fn nothing_else_acts_while_waiting_for_a_key() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (_d, mut a) = on_carousel();
    on_face_page(&mut a);
    press(&mut a, Btn::A);

    press(&mut a, Btn::B);
    press(&mut a, Btn::Down);
    assert_eq!(
        a.quick_menu(),
        Some(QuickRow::BindA),
        "the menu moved while waiting"
    );
    keymap::capture_cancel();
}

#[test]
fn closing_the_lid_while_waiting_stops_the_capture() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (_d, mut a) = on_carousel();
    on_face_page(&mut a);
    press(&mut a, Btn::A);

    a.apply(Action::LidClose);
    assert!(
        matches!(a.phase(), Phase::Doze { .. }),
        "the lid did not sleep the app: {:?}",
        a.phase()
    );
    a.tick_ms(a.now() + 1);
    assert!(
        !keymap::capture_offer(spare_key(), 1),
        "keys are still being swallowed after the menu was left"
    );
    keymap::capture_cancel();
}

#[test]
fn reset_restores_the_built_in_map() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (d, mut a) = on_carousel();
    let default_a = keymap::keys_for(Btn::A);
    on_face_page(&mut a);
    press(&mut a, Btn::A);
    assert!(keymap::capture_offer(spare_key(), 1));
    a.tick_ms(a.now() + 1);
    assert!(keymap::customised());

    // Back up to the Buttons page, then down to Reset Buttons.
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::BindFace));
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::Buttons));
    press(&mut a, Btn::A);
    down_on(&mut a, &QuickRow::KEY_MAP, QuickRow::BindReset);
    press(&mut a, Btn::A);

    assert!(!keymap::customised());
    assert_eq!(keymap::keys_for(Btn::A), default_a);
    let file = std::fs::read_to_string(d.path().join("Config/buttons.ini")).expect("buttons.ini");
    assert!(
        !file
            .lines()
            .any(|l| l.trim().chars().next().is_some_and(|c| c.is_ascii_digit())),
        "reset left bindings in the file:\n{file}"
    );
}

#[test]
fn b_walks_back_from_a_page_to_its_parent_and_the_main_page_to_the_shelf() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (_d, mut a) = on_carousel();
    on_face_page(&mut a);
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::BindFace));
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::Buttons));
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::Controls));
    press(&mut a, Btn::B);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
}

#[test]
fn opening_the_menu_again_starts_on_the_top_row() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (_d, mut a) = on_carousel();
    press(&mut a, Btn::Down);
    press(&mut a, Btn::B);
    a.apply(Action::QuickMenu);
    assert_eq!(a.quick_menu(), Some(QuickRow::MAIN[0]));
}
