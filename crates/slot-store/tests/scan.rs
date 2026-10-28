mod common;

use common::tmp_root;
use slot_store::{scan, Platform};
use tempfile::TempDir;

fn write_rom(d: &TempDir, rel: &str, title: &str) {
    let mut rom = vec![0u8; 0x100];
    rom[0xa0..0xa0 + title.len()].copy_from_slice(title.as_bytes());
    std::fs::write(d.path().join("Games").join(rel), rom).expect("write rom");
}

fn write_png(d: &TempDir, rel: &str) {
    let path = d.path().join("Labels").join(rel);
    std::fs::create_dir_all(path.parent().expect("label has a parent")).expect("create dir");
    std::fs::write(path, b"\x89PNG\r\n\x1a\n").expect("write png");
}

#[test]
fn a_png_in_labels_is_paired_to_its_rom_by_stem() {
    let d = tmp_root();
    write_rom(&d, "GBA/Pokemon Emerald.gba", "POKEMON EMER");
    write_png(&d, "GBA/Pokemon Emerald.png");
    write_rom(&d, "GBA/Advance Wars.gba", "ADVANCEWARS");
    let carts = scan(d.path()).unwrap();
    assert_eq!(carts.len(), 2);
    assert_eq!(carts[0].stem, "Advance Wars");
    assert!(carts[0].label.is_none());
    assert!(
        carts[1].label.is_some(),
        "a label in Labels/ was not picked up"
    );
    let mut emerald = carts[1].clone();
    emerald.read_header();
    assert_eq!(emerald.title, "POKEMON EMER");
}

#[test]
fn a_label_pairs_with_its_rom_whatever_the_case_of_its_name() {
    let d = tmp_root();
    write_rom(&d, "GBA/Pokemon Emerald.gba", "POKEMON EMER");
    write_png(&d, "GBA/pokemon emerald.PNG");
    let carts = scan(d.path()).unwrap();
    assert_eq!(
        carts[0].label.as_deref(),
        Some(d.path().join("Labels/GBA/pokemon emerald.PNG").as_path())
    );
}

#[test]
fn a_scan_reads_names_and_leaves_the_rom_headers_for_later() {
    let d = tmp_root();
    write_rom(&d, "GBA/Advance Wars.gba", "ADVANCEWARS");
    let mut cart = scan(d.path()).unwrap().remove(0);
    assert!(cart.title.is_empty());
    cart.read_header();
    assert_eq!(cart.title, "ADVANCEWARS");
}

#[test]
fn scan_ignores_non_gba_files() {
    let d = tmp_root();
    write_rom(&d, "GBA/Real.gba", "REAL");
    std::fs::write(d.path().join("Games/GBA/notes.txt"), "hi").unwrap();
    assert_eq!(scan(d.path()).unwrap().len(), 1);
}

#[test]
fn an_appledouble_sidecar_is_not_shelved_as_a_cart() {
    let d = tmp_root();
    write_rom(&d, "GBA/Metroid Fusion.gba", "METROID");
    write_rom(&d, "GBA/._Metroid Fusion.gba", "METROID");
    let carts = scan(d.path()).unwrap();
    assert_eq!(carts.len(), 1, "an AppleDouble sidecar reached the shelf");
    assert_eq!(carts[0].stem, "Metroid Fusion");
}

#[test]
fn header_title_of_a_truncated_rom_is_none_not_a_panic() {
    let d = tmp_root();
    std::fs::write(d.path().join("Games/GBA/Tiny.gba"), [0u8; 8]).unwrap();
    let mut cart = scan(d.path()).unwrap().remove(0);
    cart.read_header();
    assert!(cart.title.is_empty());
}

#[test]
fn a_header_title_that_is_not_text_is_dropped_rather_than_mangled() {
    let d = tmp_root();
    let mut rom = vec![0u8; 0x100];
    rom[0xa0..0xac].copy_from_slice(&[0xffu8; 12]);
    std::fs::write(d.path().join("Games/GBA/Garbage.gba"), rom).unwrap();
    let mut cart = scan(d.path()).unwrap().remove(0);
    cart.read_header();
    assert!(cart.title.is_empty());
}

#[test]
fn a_root_with_no_games_directory_scans_as_empty() {
    let d = tempfile::tempdir().unwrap();
    assert!(scan(d.path()).unwrap().is_empty());
}

#[test]
fn a_cart_takes_the_platform_of_the_folder_it_is_in() {
    let d = tmp_root();
    write_rom(&d, "GBA/Metroid Fusion.gba", "METROID");
    std::fs::write(d.path().join("Games/GB/Tetris.gb"), vec![0u8; 0x150]).unwrap();
    std::fs::write(d.path().join("Games/GBC/Chromatic.gbc"), vec![0u8; 0x150]).unwrap();

    let carts = scan(d.path()).unwrap();

    let by_stem = |s: &str| carts.iter().find(|c| c.stem == s).unwrap().platform;
    assert_eq!(by_stem("Metroid Fusion"), Platform::Gba);
    assert_eq!(by_stem("Tetris"), Platform::Gb);
    assert_eq!(by_stem("Chromatic"), Platform::Gbc);
}

#[test]
fn a_gba_rom_in_the_game_boy_folder_does_not_appear() {
    let d = tmp_root();
    std::fs::write(d.path().join("Games/GB/Wrong.gba"), vec![0u8; 0x150]).unwrap();
    assert!(scan(d.path()).unwrap().is_empty());
}

#[test]
fn an_unreadable_platform_folder_does_not_take_the_rest_of_the_library_with_it() {
    let d = tmp_root();
    write_rom(&d, "GBA/Metroid Fusion.gba", "METROID");
    std::fs::write(d.path().join("Games/GB/Tetris.gb"), vec![0u8; 0x150]).unwrap();
    std::fs::remove_dir(d.path().join("Games/GBC")).unwrap();
    std::fs::write(d.path().join("Games/GBC"), b"not a directory").unwrap();

    let carts = scan(d.path()).expect("one bad folder must not fail the whole scan");

    assert_eq!(carts.len(), 2, "the readable shelves were lost too");
    assert!(carts.iter().any(|c| c.stem == "Metroid Fusion"));
    assert!(carts.iter().any(|c| c.stem == "Tetris"));
}

#[test]
fn the_same_stem_on_two_platforms_is_two_carts_with_two_labels() {
    let d = tmp_root();
    write_rom(&d, "GBA/Tetris.gba", "TETRIS");
    std::fs::write(d.path().join("Games/GB/Tetris.gb"), vec![0u8; 0x150]).unwrap();
    std::fs::create_dir_all(d.path().join("Labels/GB")).unwrap();
    std::fs::write(d.path().join("Labels/GB/Tetris.png"), b"\x89PNG\r\n\x1a\n").unwrap();

    let carts = scan(d.path()).unwrap();

    assert_eq!(carts.len(), 2);
    let gb = carts.iter().find(|c| c.platform == Platform::Gb).unwrap();
    let gba = carts.iter().find(|c| c.platform == Platform::Gba).unwrap();
    assert!(gb.label.is_some(), "the Game Boy label was not found");
    assert!(
        gba.label.is_none(),
        "the GBA cart borrowed the Game Boy label"
    );
}

/// The label a cart ended up with, by ROM name, for the tests below.
fn label_of(d: &TempDir, stem: &str) -> Option<String> {
    let carts = scan(d.path()).unwrap();
    let cart = carts.iter().find(|c| c.stem == stem).expect("cart scanned");
    cart.label
        .as_ref()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
}

#[test]
fn every_label_in_one_flat_folder_pairs_without_sorting_them_by_system() {
    let d = tmp_root();
    write_rom(&d, "GBA/Metroid Fusion.gba", "METROID");
    std::fs::write(d.path().join("Games/GB/Tetris.gb"), vec![0u8; 0x150]).unwrap();
    write_png(&d, "Metroid Fusion.png");
    write_png(&d, "Tetris.png");

    assert_eq!(
        label_of(&d, "Metroid Fusion").as_deref(),
        Some("Metroid Fusion.png")
    );
    assert_eq!(label_of(&d, "Tetris").as_deref(), Some("Tetris.png"));
}

#[test]
fn the_systems_own_folder_beats_one_image_shoved_in_the_flat_one() {
    let d = tmp_root();
    write_rom(&d, "GBA/Metroid Fusion.gba", "METROID");
    write_png(&d, "GBA/Metroid Fusion.png");
    write_png(&d, "Metroid Fusion.png");

    let carts = scan(d.path()).unwrap();
    assert_eq!(
        carts[0].label.as_deref(),
        Some(d.path().join("Labels/GBA/Metroid Fusion.png").as_path())
    );
}

#[test]
fn a_label_named_after_the_game_ignores_region_and_revision_tags() {
    let d = tmp_root();
    write_rom(&d, "GBA/Kirby - Nightmare in Dream Land (USA).gba", "KIRBY");
    write_png(&d, "GBA/Kirby - Nightmare in Dream Land.png");
    std::fs::write(d.path().join("Games/GBC/Zelda.gbc"), vec![0u8; 0x150]).unwrap();
    write_png(&d, "GBC/Zelda (USA, Europe) (Rev 2) (SGB Enhanced).png");

    assert_eq!(
        label_of(&d, "Kirby - Nightmare in Dream Land (USA)").as_deref(),
        Some("Kirby - Nightmare in Dream Land.png")
    );
    assert_eq!(
        label_of(&d, "Zelda").as_deref(),
        Some("Zelda (USA, Europe) (Rev 2) (SGB Enhanced).png")
    );
}

#[test]
fn a_label_with_a_curly_apostrophe_pairs_with_a_rom_that_has_a_straight_one() {
    let d = tmp_root();
    std::fs::write(
        d.path()
            .join("Games/GB/Kirby's Dream Land (USA, Europe).gb"),
        vec![0u8; 0x150],
    )
    .unwrap();
    write_png(&d, "GB/Kirby\u{2019}s Dream Land.png");

    assert_eq!(
        label_of(&d, "Kirby's Dream Land (USA, Europe)").as_deref(),
        Some("Kirby\u{2019}s Dream Land.png")
    );
}

#[test]
fn a_lowercase_system_folder_is_found_like_any_other() {
    let d = tmp_root();
    write_rom(&d, "GBA/Metroid Fusion.gba", "METROID");
    write_png(&d, "gba/Metroid Fusion.png");

    assert_eq!(
        label_of(&d, "Metroid Fusion").as_deref(),
        Some("Metroid Fusion.png")
    );
}

#[test]
fn artwork_for_another_game_is_never_borrowed() {
    let d = tmp_root();
    write_rom(&d, "GBA/Final Fantasy 6 Advance.gba", "FF6");
    write_png(&d, "GBA/Final Fantasy 1 + 2 - Dawn of Souls.png");
    write_png(&d, "GBA/Fire Emblem.png");

    assert!(
        label_of(&d, "Final Fantasy 6 Advance").is_none(),
        "a cart took artwork for a different game"
    );
}

#[test]
fn an_image_that_is_not_a_png_is_left_alone() {
    let d = tmp_root();
    write_rom(&d, "GBA/Metroid Fusion.gba", "METROID");
    std::fs::create_dir_all(d.path().join("Labels/GBA")).unwrap();
    std::fs::write(d.path().join("Labels/GBA/Metroid Fusion.jpg"), b"junk").unwrap();

    assert!(
        label_of(&d, "Metroid Fusion").is_none(),
        "a file the decoder cannot read was taken as a label"
    );
}

fn write_file(d: &TempDir, rel: &str, body: &str) {
    let path = d.path().join("Games").join(rel);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("create dir");
    std::fs::write(path, body).expect("write file");
}

fn stems(carts: &[slot_store::Cart]) -> Vec<&str> {
    carts.iter().map(|c| c.stem.as_str()).collect()
}

#[test]
fn a_playstation_game_is_shelved_by_its_disc_image_and_not_its_track_files() {
    let d = tmp_root();
    write_file(&d, "PS1/Crash.cue", "FILE \"Crash.bin\" BINARY\n");
    write_file(&d, "PS1/Crash.bin", "not a cart");
    write_file(&d, "PS1/Spyro.chd", "chd");
    let carts = scan(d.path()).unwrap();
    assert_eq!(stems(&carts), ["Crash", "Spyro"]);
    assert!(carts.iter().all(|c| c.platform == Platform::Ps1));
}

#[test]
fn a_multi_disc_game_is_one_cart_its_playlist_not_one_per_disc() {
    let d = tmp_root();
    write_file(&d, "PS1/FF7 (Disc 1).cue", "");
    write_file(&d, "PS1/FF7 (Disc 2).cue", "");
    write_file(&d, "PS1/FF7 (Disc 3).chd", "");
    write_file(
        &d,
        "PS1/FF7.m3u",
        "# three discs\nFF7 (Disc 1).cue\n\nFF7 (Disc 2).cue\r\nFF7 (Disc 3).chd\n",
    );
    write_file(&d, "PS1/Crash.cue", "");
    let carts = scan(d.path()).unwrap();
    assert_eq!(stems(&carts), ["Crash", "FF7"]);
    assert!(carts[1].rom.to_string_lossy().ends_with("FF7.m3u"));
}

#[test]
fn a_playlist_matches_its_discs_whatever_the_case_or_path_style() {
    let d = tmp_root();
    write_file(&d, "PS1/Game (Disc 1).CUE", "");
    write_file(&d, "PS1/Game (Disc 2).cue", "");
    write_file(
        &d,
        "PS1/Game.m3u",
        "discs\\game (disc 1).cue\n./Game (Disc 2).cue\n",
    );
    let carts = scan(d.path()).unwrap();
    assert_eq!(stems(&carts), ["Game"]);
}

#[test]
fn a_disc_no_playlist_names_stays_on_the_shelf() {
    let d = tmp_root();
    write_file(&d, "PS1/A (Disc 1).cue", "");
    write_file(&d, "PS1/A.m3u", "A (Disc 1).cue\n");
    write_file(&d, "PS1/B.cue", "");
    let carts = scan(d.path()).unwrap();
    assert_eq!(stems(&carts), ["A", "B"]);
}

#[test]
fn a_playlist_in_another_systems_folder_hides_nothing() {
    let d = tmp_root();
    write_file(&d, "GBA/Emerald.gba", "x");
    write_file(&d, "GB/Tetris.gb", "x");
    write_file(&d, "GB/Tetris.m3u", "Emerald.gba\n");
    let carts = scan(d.path()).unwrap();
    assert_eq!(stems(&carts), ["Emerald", "Tetris"]);
}

#[test]
fn a_disc_image_is_not_read_as_a_game_boy_rom() {
    let d = tmp_root();
    // Bytes at the Game Boy title offset that would read as a title if misread.
    let mut body = vec![b' '; 0x200];
    body[0x134..0x13d].copy_from_slice(b"NOT A GB!");
    write_file(&d, "PS1/Crash.cue", &String::from_utf8(body).unwrap());
    let mut cart = scan(d.path()).unwrap().remove(0);
    cart.read_header();
    assert_eq!(cart.title, "", "a disc image has no header to read");
}
