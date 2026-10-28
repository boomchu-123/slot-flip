mod common;

use common::core_lock;
use slot::core::apply_core_options;
use slot_retro::{LibretroCore, RetroCore};
use slot_store::{Core, Platform};

#[test]
fn a_playstation_game_gets_a_memory_card_that_is_kept_with_its_saves() {
    let _g = core_lock();
    let Some(so) = common::vendored_pcsx() else {
        return;
    };
    let d = common::tmp_root_with_real_carts(&[]);
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
    core.load(&common::spin_exe(d.path())).unwrap();
    let card = core
        .save_ram()
        .expect("the card was not handed to the frontend, so no save would ever be written");
    assert_eq!(card.len(), 128 * 1024);
}
