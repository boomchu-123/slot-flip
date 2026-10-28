mod common;

use std::time::{Duration, Instant};

use common::core_lock;
use slot::audio::{AudioSink, StubSink};
use slot::core::apply_core_options;
use slot::emu::{CoreState, EmuHandle, Speed};
use slot_retro::LibretroCore;
use slot_store::{Core, Platform};

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(20);
    while !done() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_emulator_thread_reports_the_discs_and_changes_them_while_the_game_runs() {
    let _g = core_lock();
    let Some(so) = common::vendored_pcsx() else {
        return;
    };
    let d = common::tmp_root_with_real_carts(&[]);
    let m3u = common::blank_discs(d.path(), 3);
    let mut core = LibretroCore::open(&so).expect("the pcsx core is present but would not open");
    apply_core_options(
        &mut core,
        Core::Pcsx,
        Platform::Ps1,
        "auto",
        false,
        false,
        None,
    );
    let emu = EmuHandle::spawn(Box::new(core), m3u, StubSink::new().ring(), None, None);
    wait_for("the core to load", || emu.state() != CoreState::Loading);
    assert_eq!(emu.state(), CoreState::Ready);
    assert_eq!(emu.disc_count(), 3);
    assert_eq!(emu.disc_index(), 0);
    assert!(!emu.disc_changing());

    emu.set_speed(Speed::Normal);
    wait_for("the first frame", || emu.has_published());

    emu.change_disc(2);
    assert_eq!(emu.disc_index(), 2, "the choice is on show straight away");
    assert!(emu.disc_changing());

    wait_for("the lid to close again", || !emu.disc_changing());
    assert_eq!(emu.disc_index(), 2);

    emu.change_disc(1);
    wait_for("the second change", || {
        !emu.disc_changing() && emu.disc_index() == 1
    });
}

#[test]
fn a_change_to_a_disc_the_game_does_not_have_is_ignored() {
    let _g = core_lock();
    let Some(so) = common::vendored_pcsx() else {
        return;
    };
    let d = common::tmp_root_with_real_carts(&[]);
    let m3u = common::blank_discs(d.path(), 2);
    let core = LibretroCore::open(&so).expect("the pcsx core is present but would not open");
    let emu = EmuHandle::spawn(Box::new(core), m3u, StubSink::new().ring(), None, None);
    wait_for("the core to load", || emu.state() != CoreState::Loading);
    emu.change_disc(5);
    assert!(!emu.disc_changing());
    assert_eq!(emu.disc_index(), 0);
}

#[test]
fn a_game_that_is_not_a_disc_has_no_discs() {
    let _g = core_lock();
    let Some(so) = common::vendored_pcsx() else {
        return;
    };
    let d = common::tmp_root_with_real_carts(&[]);
    let exe = common::spin_exe(d.path());
    let core = LibretroCore::open(&so).expect("the pcsx core is present but would not open");
    let emu = EmuHandle::spawn(Box::new(core), exe, StubSink::new().ring(), None, None);
    wait_for("the core to load", || emu.state() != CoreState::Loading);
    assert!(emu.disc_count() < 2, "{}", emu.disc_count());
}
