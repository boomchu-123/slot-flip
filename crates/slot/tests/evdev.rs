use std::fs;
use std::path::Path;

use slot::input::evdev::{decode, has_bit, pick_devices, to_raw, EVENT_BYTES};
use slot_input::{Btn, RawEvent};

fn packet(kind: u16, code: u16, value: i32) -> [u8; EVENT_BYTES] {
    let mut b = [0x11u8; EVENT_BYTES];
    let h = EVENT_BYTES - 8;
    b[h] = kind as u8;
    b[h + 1] = (kind >> 8) as u8;
    b[h + 2] = code as u8;
    b[h + 3] = (code >> 8) as u8;
    b[h + 4..h + 8].copy_from_slice(&value.to_le_bytes());
    b
}

const EV_KEY: u16 = 0x01;
const EV_SW: u16 = 0x05;
const EV_SYN: u16 = 0x00;
const CODE_R1: u16 = 0x135;
const KEY_VOLUMEUP: u16 = 115;
const SW_LID: u16 = 0x00;

#[test]
fn a_key_press_decodes_from_the_evdev_byte_layout() {
    let down = decode(&packet(EV_KEY, CODE_R1, 1)).and_then(to_raw);
    assert_eq!(down, Some(RawEvent::Down(Btn::R1)));
    let up = decode(&packet(EV_KEY, CODE_R1, 0)).and_then(to_raw);
    assert_eq!(up, Some(RawEvent::Up(Btn::R1)));
    let vol = decode(&packet(EV_KEY, KEY_VOLUMEUP, 1)).and_then(to_raw);
    assert_eq!(vol, Some(RawEvent::Down(Btn::VolUp)));
}

#[test]
fn autorepeat_is_not_a_press() {
    assert_eq!(decode(&packet(EV_KEY, CODE_R1, 2)).and_then(to_raw), None);
}

#[test]
fn the_lid_arrives_as_a_switch_rather_than_a_key() {
    let shut = decode(&packet(EV_SW, SW_LID, 1)).and_then(to_raw);
    assert_eq!(shut, Some(RawEvent::Down(Btn::Lid)));
    let open = decode(&packet(EV_SW, SW_LID, 0)).and_then(to_raw);
    assert_eq!(open, Some(RawEvent::Up(Btn::Lid)));
    assert_ne!(
        decode(&packet(EV_KEY, SW_LID, 1)).and_then(to_raw),
        Some(RawEvent::Down(Btn::Lid))
    );
}

#[test]
fn frame_markers_and_unmapped_codes_are_dropped() {
    assert_eq!(decode(&packet(EV_SYN, 0, 0)).and_then(to_raw), None);
    assert_eq!(decode(&packet(EV_KEY, 0x2ff, 1)).and_then(to_raw), None);
}

#[test]
fn a_short_read_decodes_to_nothing_rather_than_panicking() {
    assert!(decode(&[0u8; EVENT_BYTES - 1]).is_none());
    assert!(decode(&[]).is_none());
}

#[test]
fn a_capability_bitmask_is_indexed_from_the_last_word() {
    use slot_power::has_bit_in;
    assert!(has_bit_in("10 0", 68, 64));
    assert!(!has_bit_in("10 0", 4, 64));
    assert!(has_bit("8", 3));
    assert!(!has_bit("", 0));
    assert!(!has_bit("0 0 0", 300));
}

#[test]
fn a_mask_is_read_in_the_words_the_kernel_printed() {
    use slot_power::has_bit_in;
    // A 32-bit kernel prints 32-bit words, and reading them as 64-bit words
    // found the wrong bit. Bit 0x74 (116): word 3 from the end on a 32-bit kernel, word 1 on a 64-bit one.
    assert!(has_bit_in("100000 0 0 0", 116, 32));
    assert!(!has_bit_in("100000 0 0 0", 116, 64));
    assert!(has_bit_in("10000000000000 0", 116, 64));
    // Bit 0x130 (304): word 9 from the end on a 32-bit kernel, word 4 on a 64-bit one.
    assert!(has_bit_in("10000 0 0 0 0 0 0 0 0 0", 304, 32));
    assert!(has_bit_in("1000000000000 0 0 0 0", 304, 64));
    assert!(!has_bit_in("1000000000000 0 0 0 0", 304, 32));
    // The Mini Plus's own button node, as its kernel printed it in the tester's
    // log: every key slot wants is in it. Read as 64-bit words only seven of the
    // eighteen are found, which is how it worked by luck until now.
    let mask = "1c1682 0 3000400 3014c002";
    let wanted = [
        103, 108, 105, 106, 57, 29, 42, 56, 18, 20, 15, 14, 28, 97, 1, 115, 114, 116,
    ];
    assert!(wanted.iter().all(|b| has_bit_in(mask, *b, 32)));
    assert_eq!(
        wanted.iter().filter(|b| has_bit_in(mask, **b, 64)).count(),
        7
    );
    // This build reads them in its own word size, which is the kernel's.
    assert_eq!(has_bit("8", 3), has_bit_in("8", 3, usize::BITS as u16));
}

fn write(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, body).unwrap();
}

fn input_tree() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let (dev, sys) = (d.path().join("dev/input"), d.path().join("sys/class/input"));
    for node in ["event0", "event1", "event2", "mice"] {
        write(&dev.join(node), "");
    }
    write(&sys.join("event0/device/name"), "axp2202-pek\n");
    write(&sys.join("event0/device/capabilities/key"), "0\n");
    write(&sys.join("event1/device/name"), "gpio-keys\n");
    write(
        &sys.join("event1/device/capabilities/key"),
        "1000000000000 0 0 0 0\n",
    );
    write(&sys.join("event2/device/name"), "gpio-keys-lid\n");
    write(&sys.join("event2/device/capabilities/key"), "0\n");
    write(&sys.join("event2/device/capabilities/sw"), "1\n");
    d
}

#[test]
fn devices_are_picked_by_capability_and_never_by_position() {
    let d = input_tree();
    let picked = pick_devices(
        &d.path().join("dev/input"),
        &d.path().join("sys/class/input"),
    );
    let names: Vec<_> = picked
        .iter()
        .filter_map(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["event1", "event2"]);
}

#[test]
fn an_input_node_with_no_sysfs_entry_is_skipped_rather_than_opened() {
    let d = tempfile::tempdir().unwrap();
    write(&d.path().join("dev/input/event0"), "");
    let picked = pick_devices(
        &d.path().join("dev/input"),
        &d.path().join("sys/class/input"),
    );
    assert!(picked.is_empty(), "{picked:?}");
}

#[test]
fn the_survey_reports_the_nodes_that_were_not_opened_too() {
    let d = input_tree();
    let lines = slot::input::trace::survey(
        &d.path().join("dev/input"),
        &d.path().join("sys/class/input"),
    );
    let line = |node: &str| {
        lines
            .iter()
            .find(|l| l.contains(node))
            .unwrap_or_else(|| panic!("{node} was left out of the survey: {lines:?}"))
            .clone()
    };
    assert!(line("event0").contains("skipped"), "{}", line("event0"));
    assert!(
        line("event0").contains("axp2202-pek"),
        "the survey did not name the node: {}",
        line("event0")
    );
    assert!(line("event1").contains("opened"), "{}", line("event1"));
    assert!(line("event2").contains("opened"), "{}", line("event2"));
}

#[test]
fn an_unmapped_code_is_traced_by_number_rather_than_dropped() {
    let ev = |code| slot::input::evdev::Ev {
        kind: EV_KEY,
        code,
        value: 1,
    };
    let mapped = slot::input::trace::event_line("event1", 12, ev(CODE_R1));
    assert!(mapped.contains("R1"), "{mapped}");
    let unmapped = slot::input::trace::event_line("event1", 13, ev(0x2ff));
    assert!(unmapped.contains("0x2ff"), "{unmapped}");
    assert!(
        unmapped.contains("unmapped"),
        "an unknown code has to say so, or it looks like nothing arrived: {unmapped}"
    );
}

#[test]
fn a_trace_writes_the_survey_to_the_card() {
    let d = input_tree();
    let card = tempfile::tempdir().unwrap();
    let input = slot::input::DeviceInput::open_in(
        &d.path().join("dev/input"),
        &d.path().join("sys/class/input"),
        card.path(),
        true,
    );
    drop(input);
    let log = std::fs::read_to_string(card.path().join("Logs/input-trace.log"))
        .expect("no trace file reached the card");
    assert!(log.contains("event0 skipped"), "{log}");
    assert!(log.contains("event1 opened"), "{log}");
    assert!(log.contains("gpio-keys-lid"), "{log}");
}

#[test]
fn no_trace_file_is_written_unless_one_was_asked_for() {
    let d = input_tree();
    let card = tempfile::tempdir().unwrap();
    let input = slot::input::DeviceInput::open_in(
        &d.path().join("dev/input"),
        &d.path().join("sys/class/input"),
        card.path(),
        false,
    );
    drop(input);
    assert!(!card.path().join("Logs/input-trace.log").exists());
}

#[test]
fn the_pad_is_polled_as_fast_as_the_kernel_tick_allows() {
    let d = input_tree();
    let sys = d.path().join("sys/class/input");
    write(&sys.join("event1/device/poll"), "20\n");
    write(&sys.join("event0/device/poll"), "20\n");
    let card = tempfile::tempdir().unwrap();
    let input =
        slot::input::DeviceInput::open_in(&d.path().join("dev/input"), &sys, card.path(), false);
    drop(input);
    let poll = |node: &str| std::fs::read_to_string(sys.join(node).join("device/poll")).unwrap();
    assert_eq!(
        poll("event1").trim(),
        "10",
        "the pad is still polled at 20 ms"
    );
    assert_eq!(
        poll("event0").trim(),
        "20",
        "a node slot does not read was changed"
    );
}

#[test]
fn a_pad_already_polled_faster_is_left_alone() {
    let d = input_tree();
    let sys = d.path().join("sys/class/input");
    write(&sys.join("event1/device/poll"), "5\n");
    let card = tempfile::tempdir().unwrap();
    drop(slot::input::DeviceInput::open_in(
        &d.path().join("dev/input"),
        &sys,
        card.path(),
        false,
    ));
    let poll = std::fs::read_to_string(sys.join("event1/device/poll")).unwrap();
    assert_eq!(poll.trim(), "5");
}

#[test]
fn the_pad_maps_by_the_codes_the_device_sends() {
    let expected = [
        (0x130, Btn::A),
        (0x131, Btn::B),
        (0x132, Btn::Y),
        (0x133, Btn::X),
        (0x134, Btn::L1),
        (0x135, Btn::R1),
        (0x136, Btn::Select),
        (0x137, Btn::Start),
        (0x138, Btn::Menu),
        (0x13a, Btn::L2),
        (0x13b, Btn::R2),
        (115, Btn::VolUp),
        (114, Btn::VolDown),
        (116, Btn::Power),
    ];
    for (code, want) in expected {
        assert_eq!(
            slot::input::evdev::code_to_btn(code),
            Some(want),
            "code {code:#x}"
        );
    }
}

#[test]
fn the_menu_buttons_second_code_is_swallowed() {
    assert_eq!(slot::input::evdev::code_to_btn(0x162), None);
}

#[test]
fn the_d_pad_arrives_as_a_hat_rather_than_as_keys() {
    let mut hat = slot::input::evdev::Hat::default();
    let ev = |code, value| slot::input::evdev::Ev {
        kind: slot::input::evdev::EV_ABS,
        code,
        value,
    };
    assert_eq!(hat.feed(ev(0x11, -1)), vec![RawEvent::Down(Btn::Up)]);
    assert_eq!(hat.feed(ev(0x11, 0)), vec![RawEvent::Up(Btn::Up)]);
    assert_eq!(hat.feed(ev(0x11, 1)), vec![RawEvent::Down(Btn::Down)]);
    assert_eq!(hat.feed(ev(0x11, 0)), vec![RawEvent::Up(Btn::Down)]);
    assert_eq!(hat.feed(ev(0x10, -1)), vec![RawEvent::Down(Btn::Left)]);
    assert_eq!(hat.feed(ev(0x10, 0)), vec![RawEvent::Up(Btn::Left)]);
    assert_eq!(hat.feed(ev(0x10, 1)), vec![RawEvent::Down(Btn::Right)]);
    assert_eq!(hat.feed(ev(0x10, 0)), vec![RawEvent::Up(Btn::Right)]);
}

#[test]
fn a_hat_swung_end_to_end_releases_before_it_presses() {
    let mut hat = slot::input::evdev::Hat::default();
    let ev = |code, value| slot::input::evdev::Ev {
        kind: slot::input::evdev::EV_ABS,
        code,
        value,
    };
    assert_eq!(hat.feed(ev(0x10, -1)), vec![RawEvent::Down(Btn::Left)]);
    assert_eq!(
        hat.feed(ev(0x10, 1)),
        vec![RawEvent::Up(Btn::Left), RawEvent::Down(Btn::Right)]
    );
    assert!(hat.feed(ev(0x10, 1)).is_empty());
}

#[test]
fn the_node_carrying_only_the_power_key_is_opened() {
    let d = tempfile::tempdir().unwrap();
    let (dev, sys) = (d.path().join("dev/input"), d.path().join("sys/class/input"));
    write(&dev.join("event0"), "");
    write(&sys.join("event0/device/name"), "axp2202-pek\n");
    write(
        &sys.join("event0/device/capabilities/key"),
        "10000000000000 0\n",
    );
    assert_eq!(
        pick_devices(&dev, &sys).len(),
        1,
        "the power node was skipped"
    );
}

#[test]
fn a_hat_axis_is_traced_as_the_axis_it_is() {
    let line = slot::input::trace::event_line(
        "event1",
        9,
        slot::input::evdev::Ev {
            kind: slot::input::evdev::EV_ABS,
            code: 0x11,
            value: -1,
        },
    );
    assert!(line.contains("hat0y"), "{line}");
    assert!(!line.contains("ignored"), "{line}");
}

fn abs(code: u16, value: i32) -> slot::input::evdev::Ev {
    slot::input::evdev::Ev {
        kind: slot::input::evdev::EV_ABS,
        code,
        value,
    }
}

#[test]
fn the_left_stick_presses_the_d_pad_past_half_tilt() {
    let mut hat = slot::input::evdev::Hat::default();
    hat.steer_with_stick(true);
    assert!(hat.feed(abs(0x03, -1500)).is_empty());
    assert_eq!(hat.feed(abs(0x03, -3000)), vec![RawEvent::Down(Btn::Up)]);
    assert!(hat.feed(abs(0x03, -4000)).is_empty());
    assert_eq!(hat.feed(abs(0x03, 100)), vec![RawEvent::Up(Btn::Up)]);
    assert_eq!(hat.feed(abs(0x02, 4096)), vec![RawEvent::Down(Btn::Right)]);
    assert_eq!(
        hat.feed(abs(0x02, -4096)),
        vec![RawEvent::Up(Btn::Right), RawEvent::Down(Btn::Left)]
    );
}

#[test]
fn with_the_stick_off_it_presses_nothing() {
    let mut hat = slot::input::evdev::Hat::default();
    assert!(hat.feed(abs(0x03, -4096)).is_empty());
    assert!(hat.feed(abs(0x02, 4096)).is_empty());
}

#[test]
fn turning_the_stick_off_mid_tilt_lets_go_on_its_next_move() {
    let mut hat = slot::input::evdev::Hat::default();
    hat.steer_with_stick(true);
    assert_eq!(hat.feed(abs(0x02, 4096)), vec![RawEvent::Down(Btn::Right)]);
    hat.steer_with_stick(false);
    assert_eq!(hat.feed(abs(0x02, 4000)), vec![RawEvent::Up(Btn::Right)]);
}

#[test]
fn the_d_pad_held_through_a_stick_flick_stays_held() {
    let mut hat = slot::input::evdev::Hat::default();
    hat.steer_with_stick(true);
    assert_eq!(hat.feed(abs(0x11, -1)), vec![RawEvent::Down(Btn::Up)]);
    assert!(hat.feed(abs(0x03, -4096)).is_empty());
    assert!(hat.feed(abs(0x03, 0)).is_empty());
    assert_eq!(hat.feed(abs(0x11, 0)), vec![RawEvent::Up(Btn::Up)]);
}

#[test]
fn only_the_skus_nextui_lists_with_a_left_stick_have_one() {
    use slot::input::evdev::has_left_stick;
    for t in [
        "rg34xxsp",
        "rg35xxh",
        "rg35xxpro",
        "rg40xxh",
        "rg40xxv",
        "rgcubexx",
    ] {
        assert!(has_left_stick(t), "{t}");
    }
    for t in ["rgsp", "rg35xxsp", "rg35xxplus", "rg34xx", "rg28xx", ""] {
        assert!(!has_left_stick(t), "{t}");
    }
}

#[test]
fn an_event_is_read_at_the_size_this_kernel_writes_it() {
    // A key press is the key event plus its SYN report, back to back.
    let mut read = Vec::new();
    read.extend_from_slice(&packet(EV_KEY, 57, 1));
    read.extend_from_slice(&packet(EV_SYN, 0, 0));
    let evs: Vec<_> = read
        .chunks_exact(EVENT_BYTES)
        .filter_map(decode)
        .map(|e| (e.kind, e.code, e.value))
        .collect();
    assert_eq!(evs, vec![(EV_KEY, 57, 1), (EV_SYN, 0, 0)]);
    #[cfg(target_pointer_width = "32")]
    assert_eq!(EVENT_BYTES, 16);
    #[cfg(not(target_pointer_width = "32"))]
    assert_eq!(EVENT_BYTES, 24);
}

// The masks below are what the A30's kernel printed in a tester's log
// (exp-a30-v0.3.5): 32-bit words, four nodes, the buttons on one of them.
#[test]
fn the_a30s_own_masks_show_every_button_slot_maps() {
    use slot_power::has_bit_in;
    let keys = "c1682 0 3000400 3014c002";
    let buttons = [
        103, 108, 105, 106, 57, 29, 42, 56, 18, 20, 15, 14, 97, 28, 1, 115, 114,
    ];
    for code in buttons {
        assert!(has_bit_in(keys, code, 32), "key {code} missing from {keys}");
    }
    // Read as 64-bit words, which is what it was before, almost none are there.
    let found = buttons.iter().filter(|c| has_bit_in(keys, **c, 64)).count();
    assert!(found < buttons.len() / 2, "{found} found as 64-bit words");

    // The power key is on a node of its own.
    assert!(has_bit_in("8000 100000 0 0 0", 116, 32));
    // The headset node carries only the volume keys (114 and 115) of the map,
    // and the temperature sensor none.
    let headset = "4 80000 0 0 c0000 0 0 0";
    for code in buttons.iter().chain(&[116]) {
        let volume = *code == 114 || *code == 115;
        assert_eq!(has_bit_in(headset, *code, 32), volume, "{code}");
        assert!(!has_bit_in("0", *code, 32), "{code}");
    }
}
