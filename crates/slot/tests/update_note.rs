mod common;

use common::{tmp_root_with_carts, write_gb_cart};
use slot::app::App;
use slot::build_info::Build;
use slot_store::{read_slot_state, write_slot_state, SlotState};
use slot_ui::Toast;

/// A card slot has run on: it has a state file, and no build recorded in it
/// (every fork release before this one).
fn clocked(d: &tempfile::TempDir) {
    let s = SlotState {
        clock_set: true,
        ..Default::default()
    };
    write_slot_state(d.path(), &s).unwrap();
}

fn card() -> tempfile::TempDir {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_gb_cart(&d, "Tetris", "TETRIS");
    d
}

fn updated(a: &App) -> bool {
    matches!(
        a.toast(),
        Some(Toast::Updated(v)) if v == Build::current().version
    )
}

#[test]
fn a_card_that_records_an_older_build_is_told_a_new_one_arrived() {
    let d = card();
    let mut s = SlotState {
        clock_set: true,
        ..Default::default()
    };
    s.seen_version = "0.4.0".into();
    write_slot_state(d.path(), &s).unwrap();

    let a = App::boot(d.path());

    assert!(
        updated(&a),
        "booting a newer build said nothing: {:?}",
        a.toast()
    );
    assert_eq!(
        read_slot_state(d.path()).seen_version,
        Build::current().version
    );
}

#[test]
fn a_card_from_before_the_build_was_recorded_is_taken_as_it_found_it() {
    let d = card();
    clocked(&d);

    let a = App::boot(d.path());

    assert!(
        a.toast().is_none(),
        "a card with nothing to compare against was greeted: {:?}",
        a.toast()
    );
    assert_eq!(
        read_slot_state(d.path()).seen_version,
        Build::current().version,
        "the card must be marked so the next build can greet it"
    );
}

#[test]
fn the_next_boot_of_the_same_build_says_nothing() {
    let d = card();
    let mut s = SlotState {
        clock_set: true,
        ..Default::default()
    };
    s.seen_version = "0.4.0".into();
    write_slot_state(d.path(), &s).unwrap();
    App::boot(d.path());

    let a = App::boot(d.path());

    assert!(
        a.toast().is_none(),
        "the greeting repeated: {:?}",
        a.toast()
    );
}

#[test]
fn a_fresh_card_is_not_told_it_was_updated() {
    let d = card();

    let a = App::boot(d.path());

    assert!(
        a.toast().is_none(),
        "a first install was greeted as an update: {:?}",
        a.toast()
    );
    assert_eq!(
        read_slot_state(d.path()).seen_version,
        Build::current().version,
        "the fresh card must be marked so the next boot stays quiet"
    );
}
