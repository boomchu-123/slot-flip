use slot_store::{scan, Cart, Platform};
use slot_ui::{
    cart_box, cart_face, disc_glare, disc_spin, gb_cart_shadow, gb_shell_of, is_clear, label_rect,
    seated_box, CartFace, Draw, GbShell, SlotChrome, TexId, DISC_D, MOUTH_W, OUT_W,
};
use tempfile::TempDir;

fn tmp_root() -> TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(d.path().join("Games/PS1")).expect("games dir");
    d
}

fn disc(d: &TempDir, name: &str) -> Cart {
    std::fs::write(d.path().join("Games/PS1").join(format!("{name}.cue")), "").expect("cue");
    scan(d.path())
        .unwrap()
        .into_iter()
        .find(|c| c.stem == name)
        .expect("the disc")
}

fn write_cover(d: &TempDir, name: &str, rgb: [u8; 3]) {
    std::fs::create_dir_all(d.path().join("Labels/PS1")).unwrap();
    let f = std::fs::File::create(d.path().join("Labels/PS1").join(format!("{name}.png"))).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), 64, 64);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let px: Vec<u8> = (0..64 * 64).flat_map(|_| rgb).collect();
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

const C: u32 = DISC_D / 2;

#[test]
fn a_disc_is_round_with_a_real_hole_you_can_see_through() {
    let d = tmp_root();
    let f = cart_face(&disc(&d, "Game"));
    assert_eq!((f.w, f.h), (DISC_D, DISC_D));
    assert_eq!(at(&f, 0, 0)[3], 0, "the corner is outside the circle");
    assert_eq!(at(&f, DISC_D - 1, DISC_D - 1)[3], 0);
    assert_eq!(at(&f, C, C)[3], 0, "the spindle hole is open");
    assert_eq!(at(&f, C, 6)[3], 255, "just inside the rim is solid");
    // The boundary is a ring of partial coverage, not a staircase: a circle of this
    // radius crosses several hundred pixels.
    let soft = f
        .rgba
        .chunks_exact(4)
        .filter(|p| (1..255).contains(&p[3]))
        .count();
    assert!(
        soft > 400,
        "the rim is not anti-aliased: {soft} soft pixels"
    );
}

#[test]
fn the_clear_hub_is_frosted_plastic_and_the_print_is_solid() {
    let d = tmp_root();
    let f = cart_face(&disc(&d, "Game"));
    let hub = at(&f, C + (0.17 * C as f32) as u32, C)[3];
    assert!(
        (180..255).contains(&hub),
        "the clamp ring is clear plastic, not opaque print: {hub}"
    );
    assert_eq!(at(&f, C + (0.6 * C as f32) as u32, C)[3], 255);
}

#[test]
fn cover_art_is_printed_across_the_disc() {
    let d = tmp_root();
    write_cover(&d, "Red", [200, 20, 20]);
    let f = cart_face(&disc(&d, "Red"));
    let [r, g, b, a] = at(&f, C + (0.6 * C as f32) as u32, C);
    assert_eq!(a, 255);
    assert!(
        r > 150 && g < 60 && b < 60,
        "the cover shows through: {r},{g},{b}"
    );
    let [r, g, b, _] = at(&f, C, C + (0.7 * C as f32) as u32);
    assert!(r > 150 && g < 60 && b < 60, "and on every side of the disc");
}

#[test]
fn a_disc_with_no_cover_is_coloured_by_its_title_and_the_title_is_set_on_it() {
    let d = tmp_root();
    let f = cart_face(&disc(&d, "Midnight Garden Adventures"));
    let flat = at(&f, C + (0.6 * C as f32) as u32, C);
    // Somewhere in the lower half the title's ink differs from the flat colour.
    let inked = (0..DISC_D / 3)
        .flat_map(|dy| {
            (0..DISC_D / 2).map(move |dx| (C - DISC_D / 4 + dx, C + (0.4 * C as f32) as u32 + dy))
        })
        .any(|(x, y)| {
            let p = at(&f, x, y);
            p[3] == 255 && p[..3].iter().zip(&flat).any(|(a, b)| a.abs_diff(*b) > 80)
        });
    assert!(inked, "no title was set on the disc");
}

#[test]
fn a_disc_fits_the_slot_and_is_square() {
    let (w, h) = cart_box(Platform::Ps1);
    assert_eq!((w, h), (DISC_D, DISC_D));
    let (sw, sh) = seated_box(Platform::Ps1);
    assert_eq!(sw, sh, "round when seated too");
    assert!(
        (sw as f32) < MOUTH_W,
        "{sw} does not go in a {MOUTH_W} slot"
    );
}

#[test]
fn a_disc_is_never_see_through_and_has_its_own_shadow() {
    let d = tmp_root();
    let cart = disc(&d, "Game");
    assert_eq!(gb_shell_of(&cart), Some(GbShell::Disc));
    assert!(!is_clear(&cart));
    let shadow = gb_cart_shadow(GbShell::Disc);
    assert_eq!((shadow.w, shadow.h), (DISC_D, DISC_D));
    assert_eq!(shadow.rgba[3], 0, "the corner of the shadow is empty");
    assert_eq!(
        shadow.rgba[((C * DISC_D + C) * 4 + 3) as usize],
        0,
        "and so is the hole"
    );
    // The favourite star sits on the disc, not off its edge.
    let (lx, ly, lw, lh) = label_rect(Some(GbShell::Disc));
    let (cx, cy) = ((lx + lw) - 0.5, ly - 0.5);
    assert!(
        (cx * cx + cy * cy).sqrt() < 0.5,
        "the star would hang off the disc"
    );
    assert!(lh > 0.0);
}

#[test]
fn the_spin_is_one_whole_turn_that_starts_and_ends_upright() {
    assert_eq!(disc_spin(0.0), 0.0);
    assert!((disc_spin(1.0) - std::f32::consts::TAU).abs() < 1e-5);
    let mut last = -1.0;
    for i in 0..=20 {
        let a = disc_spin(i as f32 / 20.0);
        assert!(a >= last, "it never turns back on its way in");
        last = a;
    }
    assert!(
        disc_spin(0.5) > 3.0 && disc_spin(0.5) < 3.3,
        "half way is half a turn"
    );
    assert_eq!(disc_spin(-1.0), 0.0);
    assert!((disc_spin(7.0) - disc_spin(1.0)).abs() < 1e-5, "clamped");
}

#[test]
fn the_glare_is_one_layer_that_leaves_the_hub_and_the_hole_clear() {
    let g = disc_glare();
    assert_eq!((g.w, g.h), (DISC_D, DISC_D));
    assert_eq!(at(&g, C, C)[3], 0, "no glare in the hole");
    assert_eq!(
        at(&g, C + (0.15 * C as f32) as u32, C)[3],
        0,
        "none on the clear hub"
    );
    assert!(
        g.rgba.chunks_exact(4).any(|p| p[3] > 10),
        "there is a glare"
    );
    assert!(
        g.rgba.chunks_exact(4).all(|p| p[3] < 60),
        "and it is subtle"
    );
}

fn chrome(cart: &Cart, seat: f32, glare: Option<TexId>) -> Vec<Draw> {
    let mut out = Vec::new();
    SlotChrome {
        glare,
        cart,
        face: Some(TexId::from_raw(1)),
        rest: (OUT_W as f32 - DISC_D as f32) / 2.0,
        scale: 1.0,
        seat,
        alert: None,
        dim: 0.0,
        screen: 0.0,
        game: false,
        over: false,
        behind: [None, None],
        slot: None,
        canvas: slot_ui::Canvas::FRAMED,
    }
    .draw(&mut out);
    out
}

fn face_draw(out: &[Draw]) -> Option<&Draw> {
    out.iter()
        .find(|d| matches!(d, Draw::Turned { tex, .. } | Draw::Tex { tex, .. } if *tex == TexId::from_raw(1)))
}

#[test]
fn a_disc_turns_as_the_slot_takes_it_and_is_upright_on_the_shelf() {
    let d = tmp_root();
    let cart = disc(&d, "Game");
    assert!(
        matches!(face_draw(&chrome(&cart, 0.0, None)), Some(Draw::Tex { .. })),
        "on the shelf it is drawn as it is, upright"
    );
    match face_draw(&chrome(&cart, 0.5, None)) {
        Some(Draw::Turned { turn, .. }) => {
            assert!(*turn > 1.0, "half way in it has turned: {turn}")
        }
        other => panic!("mid-insertion a disc must be drawn turned: {other:?}"),
    }
}

#[test]
fn the_glare_is_drawn_over_the_face_and_does_not_turn_with_it() {
    let d = tmp_root();
    let cart = disc(&d, "Game");
    let glare = TexId::from_raw(2);
    let out = chrome(&cart, 0.4, Some(glare));
    let face_at = out
        .iter()
        .position(|d| matches!(d, Draw::Turned { .. }))
        .unwrap();
    let glare_at = out
        .iter()
        .position(|d| matches!(d, Draw::Tex { tex, .. } if *tex == glare))
        .expect("the glare is drawn");
    assert!(glare_at > face_at, "glare goes over the print");
    assert!(
        matches!(out[glare_at], Draw::Tex { .. }),
        "a plain Tex: the glare is not turned"
    );
}

#[test]
fn a_cart_never_spins_and_never_gets_a_glare() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Games/GBA")).unwrap();
    std::fs::write(d.path().join("Games/GBA/Emerald.gba"), vec![0u8; 0x100]).unwrap();
    let cart = scan(d.path()).unwrap().remove(0);
    let glare = TexId::from_raw(2);
    let out = chrome(&cart, 0.5, Some(glare));
    assert!(
        !out.iter().any(|d| matches!(d, Draw::Turned { .. })),
        "a cart goes straight in"
    );
    assert!(!out
        .iter()
        .any(|d| matches!(d, Draw::Tex { tex, .. } if *tex == glare)));
}

fn write_png(path: &std::path::Path, w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 3]) {
    let f = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let px = &px;
    let rgb: Vec<u8> = (0..h)
        .flat_map(|y| (0..w).flat_map(move |x| px(x, y)))
        .collect();
    enc.write_header().unwrap().write_image_data(&rgb).unwrap();
}

/// The first few pixels along the middle of the left edge of a fitted cover.
fn left_edge(cover: &[u8], d: u32) -> Vec<[u8; 3]> {
    (1..6)
        .map(|x| {
            let i = (((d / 2) * d + x) * 4) as usize;
            [cover[i], cover[i + 1], cover[i + 2]]
        })
        .collect()
}

const RED: [u8; 3] = [200, 20, 20];

#[test]
fn a_black_spine_is_trimmed_from_a_cover_before_it_goes_on_the_disc() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("spined.png");
    // 200 wide: a black strip 21 wide (10.5%) with white lettering in it.
    write_png(&p, 200, 200, |x, y| match (x, y) {
        (8..=12, 40..=160) => [255, 255, 255],
        (0..=20, _) => [0, 0, 0],
        _ => RED,
    });
    let cover = slot_ui::disc_cover(&p, 100).expect("a cover");
    for px in left_edge(&cover, 100) {
        assert!(
            px[0] > 150 && px[1] < 60,
            "the spine is still on the disc: {px:?}"
        );
    }
}

#[test]
fn dark_artwork_with_no_spine_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("night.png");
    // Dark, but not solid black: a night sky.
    write_png(&p, 200, 200, |x, _| if x < 60 { [30, 24, 48] } else { RED });
    let cover = slot_ui::disc_cover(&p, 100).expect("a cover");
    let left = left_edge(&cover, 100);
    assert!(
        left.iter().all(|px| px[0] < 60 && px[2] > 30),
        "dark art was cut: {left:?}"
    );
}

#[test]
fn a_ragged_black_edge_is_not_mistaken_for_a_spine() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("ragged.png");
    // Black on only some rows, as artwork that happens to be dark there would be.
    write_png(&p, 200, 200, |x, y| {
        if x < 40 && y % 5 < 3 {
            [0, 0, 0]
        } else {
            RED
        }
    });
    let cover = slot_ui::disc_cover(&p, 100).expect("a cover");
    let dark = (0..100u32)
        .filter(|&y| {
            let i = ((y * 100 + 2) * 4) as usize;
            cover[i] < 60
        })
        .count();
    assert!(
        dark > 30,
        "the picture was trimmed when it was not a spine: {dark}"
    );
}

#[test]
fn a_cover_is_always_fitted_to_a_square_and_a_tiny_one_is_not_trimmed() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("tiny.png");
    write_png(&p, 32, 32, |_, _| [0, 0, 0]);
    let cover = slot_ui::disc_cover(&p, 64).expect("a cover");
    assert_eq!(cover.len(), 64 * 64 * 4);
    assert!(
        cover.chunks_exact(4).all(|p| p[..3] == [0, 0, 0]),
        "nothing to trim here"
    );
}

#[test]
fn carts_still_use_the_plain_cover_fit() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("cart.png");
    // The same black strip: on a cart's label this is the artwork, and stays.
    write_png(&p, 200, 200, |x, _| if x <= 20 { [0, 0, 0] } else { RED });
    let cover = slot_ui::cover_for(Platform::Gba, &p, 100, 100).expect("a cover");
    let left = left_edge(&cover, 100);
    assert!(
        left.iter().all(|px| px[0] < 40),
        "a cart's artwork was trimmed: {left:?}"
    );
}

#[test]
fn the_hub_shows_the_cover_through_it() {
    let d = tmp_root();
    write_cover(&d, "Red", [200, 20, 20]);
    let f = cart_face(&disc(&d, "Red"));
    let [r, g, b, a] = at(&f, C + (0.17 * C as f32) as u32, C);
    assert!(a > 180);
    assert!(
        r > g + 30 && r > b + 30,
        "the frosted hub is not tinted by the art: {r},{g},{b}"
    );
}
