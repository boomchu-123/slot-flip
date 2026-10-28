use slot_store::{scan, Cart, Platform};
use slot_ui::{
    cart_box, cart_face, gb_cart_shadow, gb_shell_of, is_clear, label_rect, label_size, seated_box,
    CartFace, GbShell, Pak, MOUTH_W,
};
use tempfile::TempDir;

fn tmp_root() -> TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(d.path().join("Games/NES")).expect("games dir");
    d
}

fn pak(d: &TempDir, name: &str) -> Cart {
    std::fs::write(d.path().join("Games/NES").join(format!("{name}.nes")), "").expect("rom");
    scan(d.path())
        .unwrap()
        .into_iter()
        .find(|c| c.stem == name)
        .expect("the pak")
}

fn write_art(d: &TempDir, name: &str, w: u32, h: u32, rgb: [u8; 3]) {
    std::fs::create_dir_all(d.path().join("Labels/NES")).unwrap();
    let f = std::fs::File::create(d.path().join("Labels/NES").join(format!("{name}.png"))).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let px: Vec<u8> = (0..w * h).flat_map(|_| rgb).collect();
    enc.write_header().unwrap().write_image_data(&px).unwrap();
}

fn at(face: &CartFace, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * face.w + x) * 4) as usize;
    [
        face.rgba[i],
        face.rgba[i + 1],
        face.rgba[i + 2],
        face.rgba[i + 3],
    ]
}

#[test]
fn a_nes_game_is_a_drawn_pak_and_never_a_gba_or_game_boy_cart() {
    let d = tmp_root();
    let c = pak(&d, "Contra");
    assert_eq!(c.platform, Platform::Nes);
    assert_eq!(gb_shell_of(&c), Some(GbShell::Pak(Pak::Nes)));
    assert!(!is_clear(&c), "a Pak is solid plastic, never see-through");
}

#[test]
fn the_pak_is_portrait_and_goes_in_no_wider_than_the_slot_is() {
    let (w, h) = cart_box(Platform::Nes);
    assert!(h > w, "a Game Pak is taller than it is wide: {w}x{h}");
    let (sw, sh) = seated_box(Platform::Nes);
    assert!(
        (sw as f32) < MOUTH_W,
        "{sw} is wider than the slot ({MOUTH_W})"
    );
    // Scaled evenly, so it is not squashed on the way in.
    assert!((sw as f32 / sh as f32 - w as f32 / h as f32).abs() < 0.01);
}

#[test]
fn the_face_is_the_size_of_its_box_and_clear_outside_its_outline() {
    let d = tmp_root();
    let f = cart_face(&pak(&d, "Contra"));
    assert_eq!((f.w, f.h), cart_box(Platform::Nes));
    assert_eq!(f.rgba.len(), (f.w * f.h * 4) as usize);
    // The corners are rounded away, and the foot steps in from the sides.
    assert_eq!(at(&f, 0, 0)[3], 0, "the top corner is square");
    assert_eq!(at(&f, 2, f.h - 3)[3], 0, "the foot is as wide as the slab");
    assert_eq!(at(&f, f.w - 3, f.h - 3)[3], 0);
    assert_eq!(at(&f, f.w / 2, f.h / 2)[3], 255);
    assert_eq!(at(&f, f.w / 2, f.h - 4)[3], 255, "the connector is missing");
    assert_eq!(at(&f, 150, 150)[3], 255);
}

#[test]
fn the_slab_is_grey_plastic_and_the_foot_is_narrower() {
    let d = tmp_root();
    let f = cart_face(&pak(&d, "Contra"));
    let [r, g, b, _] = at(&f, 150, 250);
    assert!(
        r.abs_diff(g) < 12 && g.abs_diff(b) < 12,
        "neutral plastic: {r},{g},{b}"
    );
    assert!((90..210).contains(&r), "mid grey: {r}");
    let width = |y: u32| (0..f.w).filter(|&x| at(&f, x, y)[3] > 128).count();
    assert!(
        width(150) > width(f.h - 6) + 40,
        "no step down to the connector"
    );
}

#[test]
fn the_grips_are_ridged() {
    let d = tmp_root();
    let f = cart_face(&pak(&d, "Contra"));
    let column: Vec<u8> = (22..134).map(|y| at(&f, 26, y)[0]).collect();
    let (lo, hi) = (*column.iter().min().unwrap(), *column.iter().max().unwrap());
    assert!(hi - lo > 30, "the grip is a flat patch: {lo}..{hi}");
}

#[test]
fn the_label_recess_holds_the_art_edge_to_edge() {
    let d = tmp_root();
    write_art(&d, "Contra", 120, 96, [200, 20, 20]);
    let c = pak(&d, "Contra");
    let (lw, lh) = label_size(&c);
    let f = cart_face(&c);
    let (x, y, w, h) = label_rect(Some(GbShell::Pak(Pak::Nes)));
    let (px, py) = ((x * f.w as f32) as u32, (y * f.h as f32) as u32);
    for (dx, dy) in [(2, 2), (lw / 2, lh / 2), (lw - 3, lh - 3)] {
        let [r, g, b, a] = at(&f, px + dx, py + dy);
        assert!(
            r > 150 && g < 60 && b < 60 && a == 255,
            "({dx},{dy}) is {r},{g},{b}"
        );
    }
    assert!((w * f.w as f32) as u32 == lw && (h * f.h as f32) as u32 == lh);
}

#[test]
fn a_pak_with_no_art_is_coloured_by_its_title_and_the_title_is_printed() {
    let d = tmp_root();
    let a = cart_face(&pak(&d, "Sky Racer"));
    let b = cart_face(&pak(&d, "Midnight Garden"));
    assert_ne!(
        at(&a, 60, 50),
        at(&b, 60, 50),
        "two titles got the same label colour"
    );
    let bg = at(&a, 60, 50);
    let differs = |x: u32, y: u32| {
        let p = at(&a, x, y);
        (0..3).map(|c| p[c].abs_diff(bg[c]) as u32).sum::<u32>() > 90
    };
    let ink_rows = (50..200)
        .filter(|&y| (60..240).any(|x| differs(x, y)))
        .count();
    assert!(ink_rows > 10, "no title was printed on the label");
}

#[test]
fn art_that_is_the_wrong_size_for_the_label_is_refitted_not_smeared() {
    let d = tmp_root();
    write_art(&d, "Tall", 359, 512, [30, 160, 30]);
    let f = cart_face(&pak(&d, "Tall"));
    let (x, y, _, _) = label_rect(Some(GbShell::Pak(Pak::Nes)));
    let [r, g, b, a] = at(
        &f,
        (x * f.w as f32) as u32 + 100,
        (y * f.h as f32) as u32 + 80,
    );
    assert!(g > 120 && r < 80 && b < 80 && a == 255, "{r},{g},{b}");
}

#[test]
fn the_shadow_is_the_paks_outline() {
    let s = gb_cart_shadow(GbShell::Pak(Pak::Nes));
    let (w, h) = cart_box(Platform::Nes);
    assert_eq!((s.w, s.h), (w, h));
    let alpha = |x: u32, y: u32| s.rgba[((y * s.w + x) * 4 + 3) as usize];
    assert_eq!(alpha(0, 0), 0);
    assert_eq!(alpha(s.w / 2, s.h / 2), 255);
}

#[test]
fn the_other_systems_are_unchanged_by_it() {
    assert_eq!(cart_box(Platform::Gba), (360, 212));
    assert_ne!(
        gb_shell_of(&Cart {
            platform: Platform::Gb,
            stem: "x".into(),
            rom: "Games/GB/x.gb".into(),
            label: None,
            code: String::new(),
            shell: None,
            title: String::new(),
        }),
        Some(GbShell::Pak(Pak::Nes))
    );
}

fn snes_pak(d: &TempDir, name: &str) -> Cart {
    std::fs::create_dir_all(d.path().join("Games/SNES")).unwrap();
    std::fs::write(d.path().join("Games/SNES").join(format!("{name}.sfc")), "").unwrap();
    scan(d.path())
        .unwrap()
        .into_iter()
        .find(|c| c.stem == name)
        .expect("the snes cart")
}

#[test]
fn a_super_nintendo_game_is_its_own_drawn_cart_not_a_game_pak() {
    let d = tmp_root();
    let c = snes_pak(&d, "Chrono");
    assert_eq!(c.platform, Platform::Snes);
    assert_eq!(gb_shell_of(&c), Some(GbShell::Pak(Pak::Snes)));
    assert!(!is_clear(&c));
    assert_ne!(cart_box(Platform::Snes), cart_box(Platform::Nes));
    let (w, h) = cart_box(Platform::Snes);
    assert!(w > h, "a SNES cart is wider than it is tall: {w}x{h}");
}

#[test]
fn every_pak_design_fits_the_slot_and_draws_to_its_own_box() {
    for (platform, pak) in [
        (Platform::Nes, Pak::Nes),
        (Platform::Snes, Pak::Snes),
        (Platform::Md, Pak::Md),
        (Platform::Sms, Pak::Sms),
        (Platform::Gg, Pak::Gg),
    ] {
        let d = pak.design();
        assert_eq!(cart_box(platform), (d.w, d.h), "{platform:?}");
        let (sw, sh) = seated_box(platform);
        assert!(
            (sw as f32) < MOUTH_W,
            "{platform:?}: {sw} is wider than the slot"
        );
        assert!(
            (sw as f32 / sh as f32 - d.w as f32 / d.h as f32).abs() < 0.01,
            "{platform:?} is squashed on the way in"
        );
        // The label sits inside the slab and the art fits it exactly.
        let (x, y, w, h) = d.label;
        assert!(
            x + w <= d.w && y + h <= d.h,
            "{platform:?}: label is off the cart"
        );
        assert_eq!(pak.mask().len(), (d.w * d.h) as usize);
        let frac = pak.label_fraction();
        assert!((frac.2 * d.w as f32 - w as f32).abs() < 0.5, "{platform:?}");
    }
}

#[test]
fn a_snes_cart_has_a_label_a_narrower_foot_and_square_free_corners() {
    let d = tmp_root();
    // The recess's own proportions, so the art fills it edge to edge.
    write_art_in(&d, "SNES", "Chrono", 228, 168, [20, 20, 200]);
    let c = snes_pak(&d, "Chrono");
    let (lw, lh) = label_size(&c);
    let f = cart_face(&c);
    assert_eq!((f.w, f.h), cart_box(Platform::Snes));
    assert_eq!(at(&f, 0, 0)[3], 0, "the top corner is square");
    assert_eq!(at(&f, 2, f.h - 3)[3], 0, "the foot is as wide as the slab");
    assert_eq!(at(&f, f.w / 2, f.h / 2)[3], 255);
    assert_eq!(at(&f, f.w / 2, f.h - 4)[3], 255, "the connector is missing");
    let (x, y, _, _) = label_rect(Some(GbShell::Pak(Pak::Snes)));
    let (px, py) = ((x * f.w as f32) as u32, (y * f.h as f32) as u32);
    for (dx, dy) in [(2, 2), (lw / 2, lh / 2), (lw - 3, lh - 3)] {
        let [r, g, b, a] = at(&f, px + dx, py + dy);
        assert!(
            b > 150 && r < 60 && g < 60 && a == 255,
            "({dx},{dy}) is {r},{g},{b}"
        );
    }
    let [r, g, b, _] = at(&f, f.w / 2, 220);
    assert!(
        r.abs_diff(g) < 12 && g.abs_diff(b) < 12 && (110..230).contains(&r),
        "{r},{g},{b}"
    );
}

#[test]
fn the_snes_cart_is_not_the_same_picture_as_the_nes_one() {
    let nes = Pak::Nes.design();
    let snes = Pak::Snes.design();
    assert_ne!((nes.w, nes.h), (snes.w, snes.h));
    assert_ne!(Pak::Nes.mask(), Pak::Snes.mask());
}

fn write_art_in(d: &TempDir, system: &str, name: &str, w: u32, h: u32, rgb: [u8; 3]) {
    std::fs::create_dir_all(d.path().join("Labels").join(system)).unwrap();
    let f = std::fs::File::create(
        d.path()
            .join("Labels")
            .join(system)
            .join(format!("{name}.png")),
    )
    .unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let px: Vec<u8> = (0..w * h).flat_map(|_| rgb).collect();
    enc.write_header().unwrap().write_image_data(&px).unwrap();
}

fn sega_cart(d: &TempDir, system: &str, file: &str) -> Cart {
    std::fs::create_dir_all(d.path().join("Games").join(system)).unwrap();
    std::fs::write(d.path().join("Games").join(system).join(file), "").unwrap();
    scan(d.path())
        .unwrap()
        .into_iter()
        .find(|c| file.starts_with(&c.stem))
        .expect("the sega cart")
}

#[test]
fn each_sega_system_is_its_own_drawn_black_cart() {
    for (system, file, platform, pak) in [
        ("MD", "Sonic.md", Platform::Md, Pak::Md),
        ("SMS", "Alex Kidd.sms", Platform::Sms, Pak::Sms),
        ("GG", "Columns.gg", Platform::Gg, Pak::Gg),
    ] {
        let d = tmp_root();
        let c = sega_cart(&d, system, file);
        assert_eq!(c.platform, platform);
        assert_eq!(gb_shell_of(&c), Some(GbShell::Pak(pak)), "{platform:?}");
        assert!(!is_clear(&c), "{platform:?}");
        let f = cart_face(&c);
        assert_eq!((f.w, f.h), cart_box(platform), "{platform:?}");
        let (w, h) = (f.w, f.h);
        assert_eq!(at(&f, 0, 0)[3], 0, "{platform:?}: the top corner is square");
        assert_eq!(at(&f, w / 2, h / 2)[3], 255, "{platform:?}");
        // The plastic is black: dark between the label and the edge, and not coloured.
        let [r, g, b, a] = at(&f, 8, h / 2);
        assert!(
            a == 255 && r < 90 && g < 90 && b < 90 && r.abs_diff(b) < 16,
            "{platform:?}: the plastic is {r},{g},{b}"
        );
    }
}

#[test]
fn the_sega_carts_differ_from_each_other_and_from_the_nintendo_ones() {
    let all = Pak::ALL;
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a.mask(), b.mask(), "{a:?} and {b:?} have the same outline");
            let (da, db) = (a.design(), b.design());
            assert_ne!(
                (da.w, da.h),
                (db.w, db.h),
                "{a:?} and {b:?} are the same size"
            );
        }
    }
    // A Game Gear cart is the smallest of them: it is a handheld's.
    let area = |p: Pak| p.design().w * p.design().h;
    assert!(all.iter().all(|p| area(Pak::Gg) <= area(*p)));
}

#[test]
fn a_sega_cart_takes_its_cover_art_in_its_own_label() {
    let d = tmp_root();
    // The recess's own proportions, so the art fills it edge to edge.
    write_art_in(&d, "MD", "Sonic", 256, 176, [200, 20, 20]);
    let c = sega_cart(&d, "MD", "Sonic.md");
    let (lw, lh) = label_size(&c);
    let f = cart_face(&c);
    let (x, y, _, _) = label_rect(Some(GbShell::Pak(Pak::Md)));
    let (px, py) = ((x * f.w as f32) as u32, (y * f.h as f32) as u32);
    for (dx, dy) in [(2, 2), (lw / 2, lh / 2), (lw - 3, lh - 3)] {
        let [r, g, b, a] = at(&f, px + dx, py + dy);
        assert!(
            r > 150 && g < 60 && b < 60 && a == 255,
            "({dx},{dy}) is {r},{g},{b}"
        );
    }
}

/// Writes an RGBA PNG whose pixel at (x, y) is `px(x, y)`.
fn write_png(path: &std::path::Path, w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 4]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let f = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            data.extend(px(x, y));
        }
    }
    enc.write_header().unwrap().write_image_data(&data).unwrap();
}

/// A cart for `system` whose label is a PNG of `w`x`h` made by `px`.
fn cart_with_scan(
    d: &TempDir,
    system: &str,
    file: &str,
    w: u32,
    h: u32,
    px: impl Fn(u32, u32) -> [u8; 4],
) -> Cart {
    sega_or_nintendo_cart(d, system, file);
    let stem = file.rsplit_once('.').unwrap().0;
    let label = d
        .path()
        .join("Labels")
        .join(system)
        .join(format!("{stem}.png"));
    write_png(&label, w, h, px);
    scan(d.path())
        .unwrap()
        .into_iter()
        .find(|c| c.stem == stem)
        .expect("the cart with its label")
}

fn sega_or_nintendo_cart(d: &TempDir, system: &str, file: &str) -> Cart {
    std::fs::create_dir_all(d.path().join("Games").join(system)).unwrap();
    std::fs::write(d.path().join("Games").join(system).join(file), "").unwrap();
    scan(d.path())
        .unwrap()
        .into_iter()
        .find(|c| file.starts_with(&c.stem))
        .expect("the cart")
}

const RED: [u8; 4] = [220, 20, 20, 255];
const BLUE: [u8; 4] = [20, 20, 220, 255];

#[test]
fn a_wide_label_scan_keeps_its_shape_and_none_of_it_is_cropped() {
    for (system, file, platform, pak) in [
        ("NES", "Contra.nes", Platform::Nes, Pak::Nes),
        ("SNES", "Chrono.sfc", Platform::Snes, Pak::Snes),
        ("MD", "Sonic.md", Platform::Md, Pak::Md),
    ] {
        let d = tmp_root();
        // Twice as wide as tall, red on the left edge and blue on the right edge.
        let c = cart_with_scan(&d, system, file, 800, 400, |x, _| {
            if x < 40 {
                RED
            } else if x >= 760 {
                BLUE
            } else {
                [240, 240, 240, 255]
            }
        });
        let design = pak.design();
        let (_, _, lw, lh) = design.label;
        let art = slot_ui::label_for(platform, c.label.as_deref().unwrap(), lw, lh).unwrap();
        assert_eq!(art.w, lw, "{platform:?}: a 2:1 scan fills the width");
        assert!(
            art.h < lh,
            "{platform:?}: and is shorter: {}x{}",
            art.w,
            art.h
        );
        assert!(
            (art.w as f32 / art.h as f32 - 2.0).abs() < 0.05,
            "{platform:?}: its shape was lost: {}x{}",
            art.w,
            art.h
        );
        // Both edges of the scan survive on the cart, which a centre-crop would lose.
        let f = cart_face(&c);
        let (x, y, _, _) = label_rect(Some(GbShell::Pak(pak)));
        let (px, py) = (
            (x * f.w as f32).round() as u32,
            (y * f.h as f32).round() as u32,
        );
        let (ox, oy) = (px, py + (lh - art.h) / 2);
        let mid = oy + art.h / 2;
        let [r, _, b, a] = at(&f, ox + 3, mid);
        assert!(
            r > 150 && b < 90 && a == 255,
            "{platform:?}: left edge {r},{b}"
        );
        let [r, _, b, a] = at(&f, ox + art.w - 4, mid);
        assert!(
            b > 150 && r < 90 && a == 255,
            "{platform:?}: right edge {r},{b}"
        );
        // Above the shrunk label, in what the generic recess would have covered, is
        // the cart's plastic and not the art.
        let [r, g, b, _] = at(&f, ox + art.w / 2, py + 2);
        assert!(
            r.abs_diff(g) < 30 && g.abs_diff(b) < 30,
            "{platform:?}: the scan spilled over the recess: {r},{g},{b}"
        );
    }
}

#[test]
fn art_that_is_not_shaped_like_a_label_is_still_cropped_to_fill_it() {
    let d = tmp_root();
    // Box art is taller than it is wide; a banner is far wider than a label.
    for (w, h) in [(512, 720), (1200, 200)] {
        let c = cart_with_scan(&d, "NES", "Contra.nes", w, h, |_, _| RED);
        let (lw, lh) = label_size(&c);
        let art = slot_ui::label_for(Platform::Nes, c.label.as_deref().unwrap(), lw, lh).unwrap();
        assert_eq!((art.w, art.h), (lw, lh), "{w}x{h}");
        assert_eq!(art.rgba.len(), (lw * lh * 4) as usize);
    }
}

#[test]
fn a_scan_the_shape_of_the_recess_fills_all_of_it() {
    let d = tmp_root();
    let (_, _, lw, lh) = Pak::Nes.design().label;
    let c = cart_with_scan(&d, "NES", "Contra.nes", lw * 3, lh * 3, |_, _| RED);
    let art = slot_ui::label_for(Platform::Nes, c.label.as_deref().unwrap(), lw, lh).unwrap();
    assert_eq!((art.w, art.h), (lw, lh));
}

#[test]
fn a_scan_with_transparent_corners_shows_the_plastic_through_them() {
    let d = tmp_root();
    // The left quarter is transparent red; the rest is solid blue.
    let c = cart_with_scan(&d, "NES", "Contra.nes", 300, 240, |x, _| {
        if x < 75 {
            [220, 20, 20, 0]
        } else {
            BLUE
        }
    });
    let (lw, lh) = label_size(&c);
    let art = slot_ui::label_for(Platform::Nes, c.label.as_deref().unwrap(), lw, lh).unwrap();
    let f = cart_face(&c);
    let (x, y, _, _) = label_rect(Some(GbShell::Pak(Pak::Nes)));
    let (px, py) = (
        (x * f.w as f32).round() as u32,
        (y * f.h as f32).round() as u32,
    );
    let (ox, oy) = (px + (lw - art.w) / 2, py + (lh - art.h) / 2);
    let [r, g, b, a] = at(&f, ox + 6, oy + art.h / 2);
    assert!(
        a == 255 && r.abs_diff(g) < 30 && g.abs_diff(b) < 30 && r < 200,
        "the transparent part is not plastic: {r},{g},{b}"
    );
    let [r, _, b, _] = at(&f, ox + art.w - 10, oy + art.h / 2);
    assert!(b > 150 && r < 90, "the solid part is lost: {r},{b}");
}
