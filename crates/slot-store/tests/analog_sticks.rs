use slot_store::{read_slot_state, write_slot_state, SlotState};

#[test]
fn the_sticks_start_off_so_nothing_changes_for_anyone_until_they_ask() {
    assert!(!SlotState::default().analog_sticks);
}

#[test]
fn turning_the_sticks_on_survives_a_restart() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    let s = SlotState {
        analog_sticks: true,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    assert!(read_slot_state(d.path()).analog_sticks);
}

#[test]
fn a_card_written_before_the_setting_reads_with_the_sticks_off() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    write_slot_state(
        d.path(),
        &SlotState {
            analog_sticks: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    let path = d.path().join("Config/slot.state");
    let old: String = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with("analog_sticks="))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(&path, old).unwrap();
    assert!(!read_slot_state(d.path()).analog_sticks);
}
