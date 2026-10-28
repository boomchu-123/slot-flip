mod common;

use common::StubSnapshot;
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{write_slot_state, Platform, SlotState};
use slot_ui::{QuickRow, QuickValue};

fn playing(platform: Platform) -> (tempfile::TempDir, App) {
    let d = tempfile::tempdir().unwrap();
    for sub in slot::root::DIRS {
        std::fs::create_dir(d.path().join(sub)).unwrap();
    }
    let dir = d.path().join("Games").join(platform.dir_name());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("Sky Racer.{}", platform.extensions()[0])),
        "",
    )
    .unwrap();
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Sky Racer".into()),
            cart_platform: Some(platform),
            clock_set: true,
            utc_offset_min: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let mut a = App::boot(d.path());
    a.set_snapshot(StubSnapshot::boxed());
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
    (d, a)
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn on_disc_row(a: &mut App) {
    a.apply(Action::QuickMenu);
    for _ in 0..8 {
        if a.in_game_menu() == Some(QuickRow::Disc) {
            return;
        }
        press(a, Btn::Down);
    }
    panic!("never reached the Disc row: {:?}", a.in_game_menu());
}

#[test]
fn a_game_on_one_disc_has_no_disc_row() {
    let (_d, mut a) = playing(Platform::Ps1);
    a.set_discs(1, 0);
    assert!(a.in_game_hidden().contains(&QuickRow::Disc));
    a.apply(Action::QuickMenu);
    for _ in 0..10 {
        assert_ne!(a.in_game_menu(), Some(QuickRow::Disc));
        press(&mut a, Btn::Down);
    }
}

#[test]
fn a_game_on_three_discs_offers_the_disc_row_between_states_and_settings() {
    let (_d, mut a) = playing(Platform::Ps1);
    a.set_discs(3, 0);
    assert!(!a.in_game_hidden().contains(&QuickRow::Disc));
    // States hides itself until a state has been written, so check the row's place
    // in the page and then walk the rows the menu actually offers on a fresh game.
    let at = |row| QuickRow::PLAY.iter().position(|r| *r == row).unwrap();
    assert!(at(QuickRow::States) < at(QuickRow::Disc));
    assert!(at(QuickRow::Disc) < at(QuickRow::Settings));
    a.apply(Action::QuickMenu);
    assert_eq!(a.in_game_menu(), Some(QuickRow::Resume));
    press(&mut a, Btn::Down);
    assert_eq!(a.in_game_menu(), Some(QuickRow::Disc));
    press(&mut a, Btn::Down);
    assert_eq!(a.in_game_menu(), Some(QuickRow::Settings));
}

#[test]
fn the_row_shows_which_disc_is_in() {
    let (_d, mut a) = playing(Platform::Ps1);
    a.set_discs(3, 0);
    assert_eq!(a.quick_value(QuickRow::Disc), Some(QuickValue::level(1)));
    a.set_discs(3, 2);
    assert_eq!(a.quick_value(QuickRow::Disc), Some(QuickValue::level(3)));
    assert_eq!(QuickValue::level(3).text(), "3");
}

#[test]
fn right_asks_for_the_next_disc_and_left_for_the_one_before() {
    let (_d, mut a) = playing(Platform::Ps1);
    a.set_discs(3, 0);
    on_disc_row(&mut a);

    press(&mut a, Btn::Right);
    assert_eq!(a.take_disc_request(), Some(1));
    assert_eq!(a.take_disc_request(), None, "asked for once");
    assert_eq!(a.quick_value(QuickRow::Disc), Some(QuickValue::level(2)));

    press(&mut a, Btn::Left);
    assert_eq!(a.take_disc_request(), Some(0));
}

#[test]
fn the_discs_wrap_round_in_both_directions() {
    let (_d, mut a) = playing(Platform::Ps1);
    a.set_discs(3, 2);
    on_disc_row(&mut a);
    press(&mut a, Btn::Right);
    assert_eq!(
        a.take_disc_request(),
        Some(0),
        "after the last comes the first"
    );

    a.set_discs(3, 0);
    press(&mut a, Btn::Left);
    assert_eq!(
        a.take_disc_request(),
        Some(2),
        "before the first comes the last"
    );
}

#[test]
fn the_choice_stays_on_show_until_it_has_been_passed_on() {
    let (_d, mut a) = playing(Platform::Ps1);
    a.set_discs(3, 0);
    on_disc_row(&mut a);
    press(&mut a, Btn::Right);
    // The emulator has not heard yet and still says disc 1.
    a.set_discs(3, 0);
    assert_eq!(
        a.quick_value(QuickRow::Disc),
        Some(QuickValue::level(2)),
        "the number must not jump back before the emulator has the request"
    );
    assert_eq!(a.take_disc_request(), Some(1));
    a.set_discs(3, 1);
    assert_eq!(a.quick_value(QuickRow::Disc), Some(QuickValue::level(2)));
}

#[test]
fn a_press_with_nothing_to_change_asks_for_nothing() {
    let (_d, mut a) = playing(Platform::Ps1);
    a.set_discs(3, 0);
    on_disc_row(&mut a);
    a.set_discs(1, 0);
    press(&mut a, Btn::Right);
    assert_eq!(a.take_disc_request(), None);
}

#[test]
fn pressing_a_on_the_row_does_nothing_but_the_menu_stays_where_it_is() {
    let (_d, mut a) = playing(Platform::Ps1);
    a.set_discs(2, 0);
    on_disc_row(&mut a);
    press(&mut a, Btn::A);
    assert_eq!(a.in_game_menu(), Some(QuickRow::Disc));
    assert_eq!(a.take_disc_request(), None);
}
