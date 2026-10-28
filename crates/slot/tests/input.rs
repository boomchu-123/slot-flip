use slot::input::HostInput;
use slot_input::{Btn, InputSource, RawEvent};
use winit::event::WindowEvent;
use winit::keyboard::KeyCode;

fn edge(h: &mut HostInput, code: KeyCode, pressed: bool) -> Vec<RawEvent> {
    h.key(code, pressed, false);
    h.poll(0)
}

#[test]
fn the_host_keymap_matches_the_documented_layout() {
    let map = [
        (KeyCode::ArrowUp, Btn::Up),
        (KeyCode::ArrowDown, Btn::Down),
        (KeyCode::ArrowLeft, Btn::Left),
        (KeyCode::ArrowRight, Btn::Right),
        (KeyCode::KeyZ, Btn::A),
        (KeyCode::KeyX, Btn::B),
        (KeyCode::KeyC, Btn::X),
        (KeyCode::KeyA, Btn::L1),
        (KeyCode::KeyS, Btn::R1),
        (KeyCode::KeyQ, Btn::L2),
        (KeyCode::KeyW, Btn::R2),
        (KeyCode::Enter, Btn::Start),
        (KeyCode::ShiftRight, Btn::Select),
        (KeyCode::Tab, Btn::Menu),
        (KeyCode::Equal, Btn::VolUp),
        (KeyCode::Minus, Btn::VolDown),
        (KeyCode::Backslash, Btn::Power),
    ];
    let mut h = HostInput::new();
    for (code, btn) in map {
        assert_eq!(edge(&mut h, code, true), vec![RawEvent::Down(btn)]);
        assert_eq!(edge(&mut h, code, false), vec![RawEvent::Up(btn)]);
    }
    assert!(edge(&mut h, KeyCode::KeyP, true).is_empty());
}

#[test]
fn key_repeat_does_not_re_press_the_button() {
    let mut h = HostInput::new();
    assert_eq!(
        edge(&mut h, KeyCode::Tab, true),
        vec![RawEvent::Down(Btn::Menu)]
    );
    h.key(KeyCode::Tab, true, true);
    h.key(KeyCode::Tab, true, true);
    assert!(
        h.poll(0).is_empty(),
        "autorepeat would re-arm the menu hold and double tap windows"
    );
}

#[test]
fn the_lid_key_toggles_because_the_host_has_no_hinge() {
    let mut h = HostInput::new();
    assert_eq!(
        edge(&mut h, KeyCode::BracketRight, true),
        vec![RawEvent::Down(Btn::Lid)]
    );
    assert!(edge(&mut h, KeyCode::BracketRight, false).is_empty());
    assert_eq!(
        edge(&mut h, KeyCode::BracketRight, true),
        vec![RawEvent::Up(Btn::Lid)]
    );
    assert_eq!(
        edge(&mut h, KeyCode::BracketRight, true),
        vec![RawEvent::Down(Btn::Lid)]
    );
}

#[test]
fn the_two_reflex_keys_no_longer_end_a_session() {
    let mut h = HostInput::new();
    for reflex in [KeyCode::Escape, KeyCode::KeyL] {
        assert!(
            edge(&mut h, reflex, true).is_empty(),
            "{reflex:?} is bound again, and it can end a live link"
        );
    }
}

#[test]
fn a_window_that_loses_focus_lets_go_of_the_keys_held_in_it() {
    let mut h = HostInput::new();
    assert_eq!(
        edge(&mut h, KeyCode::KeyZ, true),
        vec![RawEvent::Down(Btn::A)]
    );
    assert_eq!(
        edge(&mut h, KeyCode::ArrowRight, true),
        vec![RawEvent::Down(Btn::Right)]
    );
    assert_eq!(
        edge(&mut h, KeyCode::KeyZ, false),
        vec![RawEvent::Up(Btn::A)]
    );
    h.on_window_event(&WindowEvent::Focused(false));
    assert_eq!(
        h.poll(0),
        vec![RawEvent::Up(Btn::Right)],
        "the key still down when the window went away was never released"
    );
    h.on_window_event(&WindowEvent::Focused(false));
    assert!(h.poll(0).is_empty());
    h.on_window_event(&WindowEvent::Focused(true));
    assert!(h.poll(0).is_empty());
}

#[test]
fn the_pad_presses_the_console_buttons_bound_to_what_is_held() {
    use slot::input::Pad;
    use slot_input::Action;
    use slot_retro::ButtonMask;
    use slot_store::{Binding, Controls, Device, GameButton, Modifier};
    let mut c = Controls::default();
    c.set_game(GameButton::TurboA, None);
    c.set_game(GameButton::TurboB, None);
    let mut pad = Pad::default();
    pad.apply(Action::GbaDown(Btn::X));
    assert_eq!(
        pad.mask(&c).0,
        0,
        "X reached the game with nothing bound to it"
    );
    c.set_game(GameButton::A, Binding::plain(Device::X));
    assert_eq!(pad.mask(&c).0, ButtonMask::A);
    pad.apply(Action::GbaDown(Btn::A));
    assert_eq!(
        pad.mask(&c).0,
        ButtonMask::A,
        "A still pressed A after A moved to X"
    );
    pad.apply(Action::GbaUp(Btn::X));
    assert_eq!(pad.mask(&c).0, 0);
    c.set_game(GameButton::B, Binding::with(Modifier::Menu, Device::Y));
    pad.apply(Action::GbaDown(Btn::Y));
    assert_eq!(pad.mask(&c).0, 0, "plain Y pressed a MENU+Y binding");
    pad.apply(Action::MenuChordDown(Btn::Y));
    assert_eq!(pad.mask(&c).0, ButtonMask::B);
}

#[test]
fn turbo_a_and_b_fire_from_whatever_they_are_bound_to() {
    use slot::input::Pad;
    use slot_input::Action;
    use slot_retro::ButtonMask;
    use slot_store::{Binding, Controls, Device, GameButton};
    let mut c = Controls::default();
    let mut pad = Pad::default();
    pad.apply(Action::GbaDown(Btn::X));
    assert_eq!(pad.mask(&c).turbo(0).0, ButtonMask::A);
    assert_eq!(pad.mask(&c).turbo(3).0, 0);
    assert_eq!(pad.mask(&c).without_turbo().0, 0);
    pad.apply(Action::GbaUp(Btn::X));
    pad.apply(Action::GbaDown(Btn::Y));
    assert_eq!(pad.mask(&c).turbo(0).0, ButtonMask::B);
    pad.apply(Action::GbaUp(Btn::Y));
    c.set_game(GameButton::TurboA, Binding::plain(Device::L2));
    pad.apply(Action::GbaDown(Btn::X));
    assert_eq!(pad.mask(&c).turbo(0).0, 0, "X kept turbo A after it moved");
    pad.apply(Action::GbaDown(Btn::L2));
    assert_eq!(pad.mask(&c).turbo(0).0, ButtonMask::A);
}

mod disc_pad {
    use slot::input::Pad;
    use slot_input::{Action, Btn, Context, Gestures, RawEvent};
    use slot_retro::ButtonMask;
    use slot_store::{CarouselKeys, Controls, Device, Platform, Shortcut, ShortcutKeys};

    fn held(btns: &[Btn]) -> Pad {
        let mut pad = Pad::default();
        for b in btns {
            pad.apply(Action::GbaDown(*b));
        }
        pad
    }

    fn mask(btns: &[Btn], platform: Platform) -> u16 {
        held(btns).mask_for(&Controls::default(), platform).0
    }

    #[test]
    fn the_face_buttons_are_where_a_playstation_pad_has_them() {
        // Bottom (B) is Cross, right (A) is Circle: libretro's B and A.
        assert_eq!(mask(&[Btn::B], Platform::Ps1), ButtonMask::B);
        assert_eq!(mask(&[Btn::A], Platform::Ps1), ButtonMask::A);
        // Top (X) is Triangle and left (Y) is Square: libretro's X and Y.
        assert_eq!(mask(&[Btn::X], Platform::Ps1), ButtonMask::X);
        assert_eq!(mask(&[Btn::Y], Platform::Ps1), ButtonMask::Y);
    }

    #[test]
    fn the_lower_shoulders_are_l2_and_r2_and_the_upper_are_l1_and_r1() {
        assert_eq!(mask(&[Btn::L2], Platform::Ps1), ButtonMask::L2);
        assert_eq!(mask(&[Btn::R2], Platform::Ps1), ButtonMask::R2);
        assert_eq!(mask(&[Btn::L1], Platform::Ps1), ButtonMask::L);
        assert_eq!(mask(&[Btn::R1], Platform::Ps1), ButtonMask::R);
    }

    #[test]
    fn a_game_boy_pad_is_exactly_what_it_was() {
        for platform in [Platform::Gba, Platform::Gb, Platform::Gbc] {
            // X and Y are still the turbo keys, and the shoulders still do nothing extra.
            assert_eq!(mask(&[Btn::X], platform), ButtonMask::X);
            assert_eq!(mask(&[Btn::Y], platform), ButtonMask::Y);
            assert_eq!(mask(&[Btn::L2], platform), 0);
            assert_eq!(mask(&[Btn::R2], platform), 0);
        }
        let all = [
            Btn::A,
            Btn::B,
            Btn::X,
            Btn::Y,
            Btn::L1,
            Btn::R1,
            Btn::L2,
            Btn::R2,
        ];
        assert_eq!(
            held(&all).mask(&Controls::default()).0,
            held(&all).mask_for(&Controls::default(), Platform::Gba).0
        );
    }

    #[test]
    fn a_disc_reaches_the_core_unchanged_where_a_cart_gets_turbo() {
        // The emulator turns X into a pulsing A for a cart; for a disc it must not.
        let pad = mask(&[Btn::X], Platform::Ps1);
        assert_eq!(
            ButtonMask(pad).turbo(3).0,
            0,
            "what turbo would have done to Triangle"
        );
        assert_eq!(pad, ButtonMask::X);
    }

    #[test]
    fn every_button_at_once_is_every_playstation_button() {
        let all = [
            Btn::Up,
            Btn::Down,
            Btn::Left,
            Btn::Right,
            Btn::A,
            Btn::B,
            Btn::X,
            Btn::Y,
            Btn::L1,
            Btn::R1,
            Btn::L2,
            Btn::R2,
            Btn::Start,
            Btn::Select,
        ];
        let m = mask(&all, Platform::Ps1);
        for bit in [
            ButtonMask::UP,
            ButtonMask::DOWN,
            ButtonMask::LEFT,
            ButtonMask::RIGHT,
            ButtonMask::A,
            ButtonMask::B,
            ButtonMask::X,
            ButtonMask::Y,
            ButtonMask::L,
            ButtonMask::R,
            ButtonMask::L2,
            ButtonMask::R2,
            ButtonMask::START,
            ButtonMask::SELECT,
        ] {
            assert_ne!(m & bit, 0, "{bit:#06x} is missing from {m:#06x}");
        }
    }

    #[test]
    fn the_stick_clicks_are_l3_and_r3_on_a_disc_and_nothing_else() {
        assert_eq!(mask(&[Btn::L3], Platform::Ps1), ButtonMask::L3);
        assert_eq!(mask(&[Btn::R3], Platform::Ps1), ButtonMask::R3);
        assert_eq!(
            mask(&[Btn::L3], Platform::Gba) | mask(&[Btn::R3], Platform::Gba),
            0,
            "no Game Boy core has stick clicks"
        );
        assert_eq!(
            mask(&[Btn::L3], Platform::Snes) | mask(&[Btn::R3], Platform::Snes),
            0
        );
    }

    fn entries(platform: Platform) -> Vec<slot_input::Entry> {
        slot::input::Table::for_platform(
            &Controls::default(),
            &ShortcutKeys::default(),
            &CarouselKeys::default(),
            true,
            platform,
        )
        .entries()
    }

    fn fires(entries: &[slot_input::Entry], held: &[Btn], press: Btn, want: Action) -> bool {
        let mut g = Gestures::new();
        g.set_entries(entries.to_vec());
        g.set_context(Context::Game);
        let mut out = Vec::new();
        for b in held {
            out.extend(g.feed(RawEvent::Down(*b), 0));
        }
        out.extend(g.feed(RawEvent::Down(press), 10));
        out.extend(g.tick(400));
        out.contains(&want)
    }

    #[test]
    fn on_a_cart_plain_r2_is_fast_forward_as_ever() {
        assert!(fires(
            &entries(Platform::Gba),
            &[],
            Btn::R2,
            Action::FfStart
        ));
        assert!(fires(
            &entries(Platform::Gba),
            &[],
            Btn::L2,
            Action::RewindStart
        ));
    }

    #[test]
    fn on_a_disc_plain_r2_and_l2_are_left_to_the_game() {
        let e = entries(Platform::Ps1);
        assert!(
            !fires(&e, &[], Btn::R2, Action::FfStart),
            "R2 still fast-forwards"
        );
        assert!(
            !fires(&e, &[], Btn::L2, Action::RewindStart),
            "L2 still rewinds"
        );
    }

    #[test]
    fn on_a_disc_fast_forward_is_select_and_r2() {
        let e = entries(Platform::Ps1);
        assert!(fires(&e, &[Btn::Select], Btn::R2, Action::FfStart));
        assert!(!fires(&e, &[Btn::Select], Btn::L2, Action::RewindStart));
    }

    #[test]
    fn no_shortcut_on_a_disc_takes_a_plain_pad_button() {
        for e in entries(Platform::Ps1) {
            // Shelf-only keys (Favorite) are not on the pad while a game is up.
            if e.contexts.meets(slot_input::Contexts::GAME) && e.chord.modifier.is_none() {
                assert!(
                    ![Btn::X, Btn::Y, Btn::L2, Btn::R2].contains(&e.chord.button),
                    "{:?} sits on a PlayStation button",
                    e.action
                );
            }
        }
    }

    #[test]
    fn a_shortcut_the_user_put_elsewhere_is_left_alone_on_a_disc() {
        let mut keys = ShortcutKeys::default();
        keys.set(
            Shortcut::FastForward,
            slot_store::Binding::plain(Device::Down),
        );
        let e = slot::input::Table::for_platform(
            &Controls::default(),
            &keys,
            &CarouselKeys::default(),
            true,
            Platform::Ps1,
        )
        .entries();
        assert!(fires(&e, &[], Btn::Down, Action::FfStart));
    }
}

mod snes_pad {
    use slot::input::Pad;
    use slot_input::{Action, Btn, Context, Gestures, RawEvent};
    use slot_retro::ButtonMask;
    use slot_store::{CarouselKeys, Controls, Platform, ShortcutKeys};

    fn mask(btns: &[Btn], platform: Platform) -> u16 {
        let mut pad = Pad::default();
        for b in btns {
            pad.apply(Action::GbaDown(*b));
        }
        pad.mask_for(&Controls::default(), platform).0
    }

    fn entries(platform: Platform) -> Vec<slot_input::Entry> {
        slot::input::Table::for_platform(
            &Controls::default(),
            &ShortcutKeys::default(),
            &CarouselKeys::default(),
            true,
            platform,
        )
        .entries()
    }

    fn fires(entries: &[slot_input::Entry], held: &[Btn], press: Btn, want: Action) -> bool {
        let mut g = Gestures::new();
        g.set_entries(entries.to_vec());
        g.set_context(Context::Game);
        let mut out = Vec::new();
        for b in held {
            out.extend(g.feed(RawEvent::Down(*b), 0));
        }
        out.extend(g.feed(RawEvent::Down(press), 10));
        out.extend(g.tick(400));
        out.contains(&want)
    }

    #[test]
    fn the_four_face_buttons_and_both_shoulders_are_where_a_super_nintendo_pad_has_them() {
        // libretro's SNES pad: B bottom, A right, Y left, X top.
        assert_eq!(mask(&[Btn::B], Platform::Snes), ButtonMask::B);
        assert_eq!(mask(&[Btn::A], Platform::Snes), ButtonMask::A);
        assert_eq!(mask(&[Btn::X], Platform::Snes), ButtonMask::X);
        assert_eq!(mask(&[Btn::Y], Platform::Snes), ButtonMask::Y);
        assert_eq!(mask(&[Btn::L1], Platform::Snes), ButtonMask::L);
        assert_eq!(mask(&[Btn::R1], Platform::Snes), ButtonMask::R);
    }

    #[test]
    fn the_lower_shoulders_are_not_game_buttons_on_a_super_nintendo() {
        assert_eq!(mask(&[Btn::L2], Platform::Snes), 0);
        assert_eq!(mask(&[Btn::R2], Platform::Snes), 0);
    }

    #[test]
    fn x_and_y_reach_the_core_as_buttons_and_never_become_turbo() {
        let pad = mask(&[Btn::X, Btn::Y], Platform::Snes);
        assert_eq!(pad, ButtonMask::X | ButtonMask::Y);
        assert_eq!(ButtonMask(pad).turbo(3).0, 0, "turbo would have eaten them");
    }

    #[test]
    fn every_super_nintendo_button_at_once() {
        let all = [
            Btn::Up,
            Btn::Down,
            Btn::Left,
            Btn::Right,
            Btn::A,
            Btn::B,
            Btn::X,
            Btn::Y,
            Btn::L1,
            Btn::R1,
            Btn::Start,
            Btn::Select,
        ];
        let m = mask(&all, Platform::Snes);
        for bit in [
            ButtonMask::UP,
            ButtonMask::DOWN,
            ButtonMask::LEFT,
            ButtonMask::RIGHT,
            ButtonMask::A,
            ButtonMask::B,
            ButtonMask::X,
            ButtonMask::Y,
            ButtonMask::L,
            ButtonMask::R,
            ButtonMask::START,
            ButtonMask::SELECT,
        ] {
            assert_ne!(m & bit, 0, "{bit:#06x} is missing from {m:#06x}");
        }
    }

    #[test]
    fn fast_forward_and_rewind_keep_their_keys_because_the_triggers_are_free() {
        let e = entries(Platform::Snes);
        assert!(
            fires(&e, &[], Btn::R2, Action::FfStart),
            "R2 lost fast-forward"
        );
        assert!(
            fires(&e, &[], Btn::L2, Action::RewindStart),
            "L2 lost rewind"
        );
    }

    #[test]
    fn no_shortcut_takes_x_or_y_while_a_super_nintendo_game_is_up() {
        for e in entries(Platform::Snes) {
            if e.contexts.meets(slot_input::Contexts::GAME) && e.chord.modifier.is_none() {
                assert!(
                    ![Btn::X, Btn::Y].contains(&e.chord.button),
                    "{:?} sits on X or Y",
                    e.action
                );
            }
        }
    }

    #[test]
    fn a_mega_drive_pad_is_read_like_a_super_nintendo_one() {
        // libretro's Genesis pad: Y is the Mega Drive's A, B its B, A its C; X, L and R
        // are its Y, X and Z. They are the same bits as the SNES's.
        for (btn, bit) in [
            (Btn::B, ButtonMask::B),
            (Btn::A, ButtonMask::A),
            (Btn::X, ButtonMask::X),
            (Btn::Y, ButtonMask::Y),
            (Btn::L1, ButtonMask::L),
            (Btn::R1, ButtonMask::R),
            (Btn::Select, ButtonMask::SELECT),
            (Btn::Start, ButtonMask::START),
        ] {
            assert_eq!(mask(&[btn], Platform::Md), bit, "{btn:?}");
        }
        assert_eq!(mask(&[Btn::L2], Platform::Md), 0);
        assert_eq!(mask(&[Btn::R2], Platform::Md), 0);
        let pad = mask(&[Btn::X, Btn::Y], Platform::Md);
        assert_eq!(ButtonMask(pad).turbo(3).0, 0, "turbo would have eaten them");
    }

    #[test]
    fn a_mega_drive_keeps_fast_forward_and_rewind_and_gives_no_shortcut_x_or_y() {
        let e = entries(Platform::Md);
        assert!(
            fires(&e, &[], Btn::R2, Action::FfStart),
            "R2 lost fast-forward"
        );
        assert!(
            fires(&e, &[], Btn::L2, Action::RewindStart),
            "L2 lost rewind"
        );
        for e in e {
            if e.contexts.meets(slot_input::Contexts::GAME) && e.chord.modifier.is_none() {
                assert!(
                    ![Btn::X, Btn::Y].contains(&e.chord.button),
                    "{:?} sits on X or Y",
                    e.action
                );
            }
        }
    }

    #[test]
    fn the_game_boy_family_nes_and_the_master_system_and_game_gear_are_untouched() {
        for platform in [
            Platform::Gba,
            Platform::Gb,
            Platform::Gbc,
            Platform::Nes,
            Platform::Sms,
            Platform::Gg,
        ] {
            assert_eq!(mask(&[Btn::L2], platform), 0, "{platform:?}");
            assert_eq!(mask(&[Btn::X], platform), ButtonMask::X, "{platform:?}");
        }
    }
}
