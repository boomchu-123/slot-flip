use slot_store::Platform;

#[test]
fn every_platform_has_a_directory() {
    let names: Vec<&str> = Platform::ALL.iter().map(|p| p.dir_name()).collect();
    assert_eq!(
        names,
        vec!["GBA", "GBC", "GB", "PS1", "NES", "SNES", "MD", "SMS", "GG"]
    );
}

#[test]
fn every_platform_is_a_shelf_of_its_own() {
    assert_eq!(
        Platform::ALL,
        [
            Platform::Gba,
            Platform::Gbc,
            Platform::Gb,
            Platform::Ps1,
            Platform::Nes,
            Platform::Snes,
            Platform::Md,
            Platform::Sms,
            Platform::Gg
        ]
    );
    for (i, p) in Platform::ALL.iter().enumerate() {
        assert!(
            !Platform::ALL[..i].contains(p),
            "{p:?} is named twice in the ring, so two shelves would hold one platform"
        );
    }
}

#[test]
fn each_platform_takes_only_its_own_extensions() {
    assert!(Platform::Gba.accepts(std::path::Path::new("Metroid Fusion.gba")));
    assert!(!Platform::Gba.accepts(std::path::Path::new("Tetris.gb")));
    assert!(Platform::Gb.accepts(std::path::Path::new("Tetris.gb")));
    assert!(Platform::Gb.accepts(std::path::Path::new("Tetris.gbc")));
    assert!(!Platform::Gb.accepts(std::path::Path::new("Metroid Fusion.gba")));
    assert!(Platform::Gba.accepts(std::path::Path::new("Shrek.GBA")));
}

#[test]
fn a_playstation_takes_disc_images_and_playlists_but_not_their_track_files() {
    let ps1 = Platform::Ps1;
    for ok in ["Crash.cue", "Crash.CHD", "FF7.m3u", "Game.pbp"] {
        assert!(ps1.accepts(std::path::Path::new(ok)), "{ok}");
    }
    // A cue sheet's .bin is a track of a game already on the shelf, not a game.
    for no in ["Crash.bin", "Crash.iso", "Metroid Fusion.gba", "Tetris.gb"] {
        assert!(!ps1.accepts(std::path::Path::new(no)), "{no}");
    }
    assert!(!Platform::Gba.accepts(std::path::Path::new("Crash.cue")));
}

#[test]
fn each_platform_runs_only_the_cores_that_can() {
    use slot_store::Core;
    for p in [Platform::Gba, Platform::Gb, Platform::Gbc] {
        assert!(Core::Mgba.runs(p), "{p:?}");
        assert!(!Core::Pcsx.runs(p), "{p:?}");
    }
    assert!(Core::Gpsp.runs(Platform::Gba));
    assert!(!Core::Gpsp.runs(Platform::Gb));
    assert!(Core::Pcsx.runs(Platform::Ps1));
    assert!(!Core::Mgba.runs(Platform::Ps1), "mGBA cannot play a disc");
    assert!(!Core::Gpsp.runs(Platform::Ps1));
    assert_eq!(Core::default_for(Platform::Ps1), Core::Pcsx);
}

#[test]
fn a_nes_takes_its_own_files_and_runs_on_fceumm_alone() {
    use slot_store::Core;
    for ok in ["Contra.nes", "Zelda.NES", "Disk.fds", "Hack.unf"] {
        assert!(Platform::Nes.accepts(std::path::Path::new(ok)), "{ok}");
    }
    for no in ["Contra.sfc", "Tetris.gb", "Crash.cue", "Fusion.gba"] {
        assert!(!Platform::Nes.accepts(std::path::Path::new(no)), "{no}");
    }
    assert!(!Platform::Gba.accepts(std::path::Path::new("Contra.nes")));
    assert!(Core::Fceumm.runs(Platform::Nes));
    for p in [Platform::Gba, Platform::Gb, Platform::Gbc, Platform::Ps1] {
        assert!(!Core::Fceumm.runs(p), "{p:?}");
    }
    for core in [Core::Mgba, Core::Gpsp, Core::Pcsx] {
        assert!(!core.runs(Platform::Nes), "{core:?} cannot play a NES game");
    }
    assert_eq!(Core::default_for(Platform::Nes), Core::Fceumm);
    assert_eq!(Core::parse("fceumm"), Some(Core::Fceumm));
    assert_eq!(Core::parse(Core::Fceumm.as_str()), Some(Core::Fceumm));
    assert_eq!(Platform::Nes.picture(), (256, 240));
}

#[test]
fn only_the_game_boy_family_has_a_screen_to_shade_and_a_gba_canvas_to_sit_on() {
    for p in [Platform::Gba, Platform::Gb, Platform::Gbc] {
        assert!(p.has_screen_options(), "{p:?}");
        assert!(!p.native_video(), "{p:?}");
    }
    for p in [
        Platform::Ps1,
        Platform::Nes,
        Platform::Snes,
        Platform::Md,
        Platform::Sms,
        Platform::Gg,
    ] {
        assert!(!p.has_screen_options(), "{p:?}");
        assert!(p.native_video(), "{p:?}");
    }
}

#[test]
fn every_systems_table_entry_is_consistent() {
    use slot_store::Core;
    let mut dirs = Vec::new();
    for p in Platform::ALL {
        let s = p.spec();
        assert!(!s.name.is_empty() && !s.dir.is_empty(), "{p:?}");
        assert!(
            !dirs.contains(&s.dir),
            "{p:?} shares a folder with another system"
        );
        dirs.push(s.dir);
        assert!(!s.extensions.is_empty(), "{p:?} takes no files");
        assert!(
            s.extensions.iter().all(|e| e
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())),
            "{p:?}: extensions are matched case-insensitively and written lower case"
        );
        assert!(s.picture.0 > 0 && s.picture.1 > 0, "{p:?}");
        assert!(!s.cores.is_empty(), "{p:?} has no core");
        for core in s.cores {
            assert!(core.runs(p), "{p:?} lists {core:?}, which then refuses it");
            assert!(Core::EVERY.contains(core), "{core:?} is not in Core::EVERY");
        }
        assert!(
            s.cores.contains(&Core::default_for(p)),
            "{p:?} defaults to a core it does not list"
        );
        assert_eq!(p.has_screen_options(), !s.native_video);
    }
}

#[test]
fn every_core_runs_something_and_nothing_runs_on_a_core_that_does_not_list_it() {
    use slot_store::Core;
    for core in Core::EVERY {
        assert!(
            Platform::ALL.iter().any(|p| core.runs(*p)),
            "{core:?} runs no system"
        );
        for p in Platform::ALL {
            assert_eq!(
                core.runs(p),
                p.spec().cores.contains(&core),
                "{core:?} on {p:?}"
            );
        }
    }
}

#[test]
fn the_sega_systems_share_one_core_and_each_takes_only_its_own_files() {
    use slot_store::Core;
    let take = |p: Platform, name: &str| p.accepts(std::path::Path::new(name));
    for ok in ["Sonic.md", "Sonic.GEN", "Sonic.smd", "Sonic.bin"] {
        assert!(take(Platform::Md, ok), "{ok}");
    }
    assert!(take(Platform::Sms, "Alex Kidd.sms") && take(Platform::Gg, "Columns.gg"));
    for (p, no) in [
        (Platform::Md, "Alex Kidd.sms"),
        (Platform::Md, "Columns.gg"),
        (Platform::Sms, "Sonic.md"),
        (Platform::Sms, "Columns.gg"),
        (Platform::Gg, "Alex Kidd.sms"),
        (Platform::Gg, "Sonic.bin"),
        (Platform::Gba, "Sonic.md"),
        (Platform::Snes, "Sonic.bin"),
        (Platform::Ps1, "Sonic.md"),
    ] {
        assert!(!take(p, no), "{p:?} took {no}");
    }
    for p in [Platform::Md, Platform::Sms, Platform::Gg] {
        assert!(Core::Genesis.runs(p), "{p:?}");
        assert_eq!(Core::default_for(p), Core::Genesis, "{p:?}");
        assert!(p.native_video(), "{p:?}");
        assert!(p.can_rewind(), "{p:?}");
    }
    for p in [
        Platform::Gba,
        Platform::Gb,
        Platform::Gbc,
        Platform::Ps1,
        Platform::Nes,
        Platform::Snes,
    ] {
        assert!(!Core::Genesis.runs(p), "Genesis Plus GX cannot play {p:?}");
    }
    for core in [
        Core::Mgba,
        Core::Gpsp,
        Core::Pcsx,
        Core::Fceumm,
        Core::Snes9x,
    ] {
        assert!(!core.runs(Platform::Md) && !core.runs(Platform::Sms) && !core.runs(Platform::Gg));
    }
    assert_eq!(Core::parse("genesis"), Some(Core::Genesis));
    assert_eq!(Core::parse(Core::Genesis.as_str()), Some(Core::Genesis));
    assert_eq!(Platform::Md.picture(), (320, 224));
    assert_eq!(Platform::Sms.picture(), (256, 192));
    assert_eq!(Platform::Gg.picture(), (160, 144));
}

#[test]
fn only_the_mega_drive_hands_the_core_its_pad_as_the_console_has_it() {
    assert!(Platform::Md.native_pad());
    assert!(!Platform::Sms.native_pad() && !Platform::Gg.native_pad());
    assert!(!Platform::Nes.native_pad());
}
