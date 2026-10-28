use slot_store::Platform;
use slot_ui::{console_opening_depth, console_slot, slot_front, Console, SLOT_H, SLOT_W};

fn alpha(strip: &[u8], x: u32, y: u32) -> u8 {
    strip[((y * SLOT_W + x) * 4 + 3) as usize]
}

fn rgb(strip: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * SLOT_W + x) * 4) as usize;
    [strip[i], strip[i + 1], strip[i + 2]]
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf29ce484222325u64, |h, x| {
        (h ^ *x as u64).wrapping_mul(0x100000001b3)
    })
}

#[test]
fn every_strip_is_the_size_of_the_cart_slots() {
    for console in Console::ALL {
        for light in [false, true] {
            let s = console_slot(console, light);
            let n = (SLOT_W * SLOT_H * 4) as usize;
            assert_eq!(
                (s.back.len(), s.front.len(), s.plugged.len()),
                (n, n, n),
                "{console:?}"
            );
        }
    }
}

#[test]
fn the_opening_is_where_the_cart_slots_is_so_a_game_clips_the_same_way() {
    let cart = slot_front();
    let cart_cols: Vec<u32> = (0..SLOT_W).filter(|&x| alpha(cart, x, 2) < 128).collect();
    let (c0, c1) = (cart_cols[0] as i32, *cart_cols.last().unwrap() as i32);
    for console in Console::ALL {
        let ours: Vec<u32> = (0..SLOT_W)
            .filter(|&x| console_opening_depth(console, x) > 2.0)
            .collect();
        let (o0, o1) = (ours[0] as i32, *ours.last().unwrap() as i32);
        assert!(
            (c0 - o0).abs() <= 6 && (c1 - o1).abs() <= 6,
            "{console:?}: cart {c0}..{c1}, console {o0}..{o1}"
        );
        let d = console_opening_depth(console, 360);
        assert!(d > 40.0 && d <= 44.0, "{console:?}: {d}");
    }
}

#[test]
fn the_front_is_cut_away_at_the_opening_and_solid_everywhere_else() {
    for console in Console::ALL {
        for light in [false, true] {
            let f = console_slot(console, light).front;
            assert_eq!(alpha(&f, 360, 20), 0, "{console:?}: the opening is clear");
            assert_eq!(alpha(&f, 360, 2), 0, "{console:?}");
            for x in [0, 30, 600, 700] {
                assert_eq!(alpha(&f, x, 10), 255, "{console:?}: plate at column {x}");
            }
            assert_eq!(
                alpha(&f, 360, SLOT_H - 3),
                255,
                "{console:?}: the lip under the opening"
            );
        }
    }
}

#[test]
fn the_back_is_solid_and_dark_and_the_plugged_slot_is_closed() {
    for console in Console::ALL {
        for light in [false, true] {
            let s = console_slot(console, light);
            for (x, y) in [(10, 5), (360, 20), (700, 50)] {
                assert_eq!(alpha(&s.back, x, y), 255, "{console:?}");
                let [r, g, b] = rgb(&s.back, x, y);
                assert!(
                    r < 90 && g < 90 && b < 90,
                    "{console:?}: the inside is dark: {r},{g},{b}"
                );
            }
            for (x, y) in [(360, 2), (360, 30), (10, 5), (700, 50)] {
                assert_eq!(
                    alpha(&s.plugged, x, y),
                    255,
                    "{console:?}: closed at ({x},{y})"
                );
            }
        }
    }
}

#[test]
fn the_light_theme_is_a_lighter_console() {
    for console in Console::ALL {
        let (d, l) = (
            console_slot(console, false).front,
            console_slot(console, true).front,
        );
        let sum = |s: &[u8]| rgb(s, 60, 30).iter().map(|&c| c as u32).sum::<u32>();
        assert!(sum(&l) > sum(&d), "{console:?}");
    }
}

#[test]
fn drawing_it_twice_gives_the_same_pixels() {
    for console in Console::ALL {
        let (a, b) = (console_slot(console, false), console_slot(console, false));
        assert_eq!((a.front, a.back, a.plugged), (b.front, b.back, b.plugged));
    }
}

#[test]
fn each_console_looks_like_itself_and_not_like_another() {
    let mut seen: Vec<(Console, u64)> = Vec::new();
    for console in Console::ALL {
        for light in [false, true] {
            let h = fnv(&console_slot(console, light).front);
            assert!(
                !seen.iter().any(|(c, x)| *x == h && *c != console),
                "{console:?} is drawn exactly like another console"
            );
            seen.push((console, h));
        }
    }
}

#[test]
fn only_gba_keeps_the_slot_traced_from_upstream() {
    assert_eq!(Console::of(Platform::Gba), None);
    let mut all: Vec<Console> = Platform::ALL
        .iter()
        .filter_map(|p| Console::of(*p))
        .collect();
    assert_eq!(all.len(), Platform::ALL.len() - 1);
    all.sort_by_key(|c| c.index());
    all.dedup();
    assert_eq!(all.len(), Console::ALL.len(), "two systems share a console");
}

#[test]
fn the_playstation_is_exactly_what_it_was_before_the_other_consoles_existed() {
    // Hashes of the strips as drawn by the PlayStation-only code in v0.9.0.
    let golden: [(bool, u64, u64, u64); 2] = [
        (
            false,
            0x09ca20161ca384b4,
            0xff0a3882f2bcf968,
            0xd022b20784c1a9bb,
        ),
        (
            true,
            0x6b6069065faf277a,
            0x52bd3624e214a1ca,
            0x714416957d693d55,
        ),
    ];
    for (light, back, front, plugged) in golden {
        let s = console_slot(Console::Ps1, light);
        assert_eq!(fnv(&s.back), back, "back, light={light}");
        assert_eq!(fnv(&s.front), front, "front, light={light}");
        assert_eq!(fnv(&s.plugged), plugged, "plugged, light={light}");
    }
}

#[test]
fn the_playstation_is_grey_plastic_with_a_green_power_light() {
    let f = console_slot(Console::Ps1, false).front;
    let [r, g, b] = rgb(&f, 60, 30);
    assert!(r.abs_diff(g) < 12 && g.abs_diff(b) < 12, "{r},{g},{b}");
    assert!((90..210).contains(&r), "mid grey: {r}");
    let [lr, lg, lb] = rgb(&f, 138, 29);
    assert!(lg > lr + 40 && lg > lb + 40, "green: {lr},{lg},{lb}");
}

#[test]
fn the_nes_has_a_red_light_and_a_dark_stripe_along_its_front() {
    let f = console_slot(Console::Nes, false).front;
    let [r, g, b] = rgb(&f, 138, 27);
    assert!(
        r > g + 50 && r > b + 50,
        "the power light is red: {r},{g},{b}"
    );
    let plate = rgb(&f, 30, 30);
    let band = rgb(&f, 30, 51);
    let sum = |c: [u8; 3]| c.iter().map(|&v| v as u32).sum::<u32>();
    assert!(
        sum(band) + 150 < sum(plate),
        "no stripe: {band:?} on {plate:?}"
    );
    // The stripe is the console's, not the opening's: it runs under the opening too.
    assert!(sum(rgb(&f, 360, 51)) + 150 < sum(plate));
}

#[test]
fn the_super_nintendo_has_ridges_either_side_and_purple_reset_and_eject() {
    let f = console_slot(Console::Snes, false).front;
    let column: Vec<u8> = (12..40).map(|y| rgb(&f, 79, y)[0]).collect();
    let (lo, hi) = (*column.iter().min().unwrap(), *column.iter().max().unwrap());
    let ridged: Vec<u8> = (0..30).map(|n| rgb(&f, 78 + n, 25)[0]).collect();
    let (rlo, rhi) = (*ridged.iter().min().unwrap(), *ridged.iter().max().unwrap());
    assert!(
        rhi - rlo > 25,
        "the ridges are a flat patch: {rlo}..{rhi} ({lo}..{hi})"
    );
    // RESET's cap: blue-violet, not the grey of POWER's.
    let [r, g, b] = rgb(&f, 525, 24);
    assert!(b > g + 20 && r > g, "RESET is not purple: {r},{g},{b}");
    let [pr, pg, pb] = rgb(&f, 157, 24);
    assert!(pb.abs_diff(pg) < 14, "POWER is grey: {pr},{pg},{pb}");
}

#[test]
fn the_game_boy_color_is_purple_plastic_and_the_game_boy_is_grey() {
    let gbc = console_slot(Console::GameBoyColor, false).front;
    let [r, g, b] = rgb(&gbc, 30, 30);
    assert!(b > g + 30 && r > g, "not purple: {r},{g},{b}");
    let gb = console_slot(Console::GameBoy, false).front;
    let [r, g, b] = rgb(&gb, 30, 30);
    assert!(r.abs_diff(b) < 20, "the Game Boy is not grey: {r},{g},{b}");
}

#[test]
fn the_playstation_opening_curves_and_the_others_are_flat_floored() {
    // Near the wall the PlayStation's half ellipse has barely any depth; the flat
    // floors are already close to full, and the Super Nintendo's corner is rounder
    // than the NES's.
    let near = (359.5 - 127.0 + 6.0) as u32;
    let ps1 = console_opening_depth(Console::Ps1, near);
    let nes = console_opening_depth(Console::Nes, near);
    let snes = console_opening_depth(Console::Snes, near);
    assert!(ps1 < 20.0, "ps1 {ps1}");
    assert!(nes > 40.0, "nes {nes}");
    assert!(snes < nes && snes > ps1, "snes {snes}");
    for console in Console::ALL {
        assert!(console_opening_depth(console, 0) == 0.0);
        assert!(console_opening_depth(console, 719) == 0.0);
    }
}

#[test]
fn the_sega_consoles_are_black_plastic_each_with_its_own_dressing() {
    let sum = |c: [u8; 3]| c.iter().map(|&v| v as u32).sum::<u32>();
    for console in [Console::MegaDrive, Console::MasterSystem, Console::GameGear] {
        let f = console_slot(console, false).front;
        let plate = rgb(&f, 30, 30);
        assert!(sum(plate) < 3 * 90, "{console:?} is not black: {plate:?}");
        assert!(
            plate[0].abs_diff(plate[2]) < 16,
            "{console:?} is tinted: {plate:?}"
        );
    }
    // A Master System's band along the front, and its buttons, are red.
    let sms = console_slot(Console::MasterSystem, false).front;
    let [r, g, b] = rgb(&sms, 30, 51);
    assert!(r > g + 60 && r > b + 60, "no red band: {r},{g},{b}");
    let [r, g, b] = rgb(&sms, 157, 24);
    assert!(r > g + 60 && r > b + 60, "POWER is not red: {r},{g},{b}");
    // A Mega Drive's band is silver, not red.
    let md = console_slot(Console::MegaDrive, false).front;
    let [r, g, b] = rgb(&md, 30, 51);
    assert!(
        r.abs_diff(b) < 24 && r > 100,
        "the band is not silver: {r},{g},{b}"
    );
    // A Game Gear is ridged at its ends like a handheld; the Mega Drive is not.
    let gg = console_slot(Console::GameGear, false).front;
    let ridged = |f: &[u8]| {
        let v: Vec<u8> = (0..30).map(|n| rgb(f, 60 + n, 25)[0]).collect();
        v.iter().max().unwrap() - v.iter().min().unwrap()
    };
    assert!(
        ridged(&gg) > 14,
        "no ridges on the Game Gear: {}",
        ridged(&gg)
    );
    assert!(ridged(&md) < ridged(&gg));
    // Red power lights on all three.
    for f in [&sms, &md, &gg] {
        let [r, g, b] = rgb(f, 138, 27);
        let [r2, g2, b2] = rgb(f, 138, 29);
        assert!(
            (r > g + 40 && r > b + 40) || (r2 > g2 + 40 && r2 > b2 + 40),
            "no red light: {r},{g},{b} / {r2},{g2},{b2}"
        );
    }
}
