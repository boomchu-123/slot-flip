use slot_retro::{ButtonMask, LibretroCore, RetroCore, GBA_H, GBA_W};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

static CORE_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    CORE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

/// The device core, built by `cores/snes9x2005/build.sh`. Linux only, and absent
/// unless it has been built, so these tests skip quietly elsewhere.
fn core(dir: &Path) -> Option<LibretroCore> {
    let so = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x2005_plus_libretro.so");
    if !so.exists() {
        eprintln!("no snes9x2005 core built, skipping");
        return None;
    }
    Some(
        LibretroCore::open_with(&so, dir, dir)
            .expect("the snes9x2005 core is present but would not open"),
    )
}

/// A Super Nintendo program that is nobody's game. Each frame, once the
/// auto-joypad read is done and the picture is in vertical blank, it paints the
/// backdrop colour from the pad: JOY1H (B Y Select Start Up Down Left Right) is
/// the low byte of the colour and JOY1L (A X L R) the high one, shifted right once,
/// so the picture says what the core was handed.
fn rom(dir: &Path, name: &str) -> PathBuf {
    rom_with(dir, name, false)
}

/// `rom` with the cartridge's save RAM declared and written to, or without.
fn rom_with(dir: &Path, name: &str, sram: bool) -> PathBuf {
    let mut code: Vec<u8> = vec![
        0x78, // sei
        0xa9, 0x8f, 0x8d, 0x00, 0x21, // lda #$8f ; sta $2100 (forced blank)
        0xa9, 0x01, 0x8d, 0x00, 0x42, // lda #1   ; sta $4200 (auto-joypad on)
        0xa9, 0x0f, 0x8d, 0x00, 0x21, // lda #$0f ; sta $2100 (screen on)
    ];
    if sram {
        // Point the data bank at $70, where a LoROM cartridge's save RAM lives,
        // write a signature at each end of the 8 KiB, then put the bank back so
        // the rest of the program addresses zero page and registers as before.
        code.extend([
            0xa9, 0x70, 0x48, 0xab, // lda #$70 ; pha ; plb
            0xa9, 0xa5, 0x8d, 0x00, 0x00, // lda #$a5 ; sta $0000
            0xa9, 0x5a, 0x8d, 0x01, 0x00, // lda #$5a ; sta $0001
            0xa9, 0x12, 0x8d, 0xff, 0x1f, // lda #$12 ; sta $1fff
            0xa9, 0x00, 0x48, 0xab, // lda #0 ; pha ; plb
        ]);
    }
    let again = 0x8000 + code.len() as u16;
    code.extend([
        0xad,
        0x12,
        0x42,
        0x10,
        0xfb, // wait: lda $4212 ; bpl wait (until vblank)
        0xad,
        0x12,
        0x42,
        0x29,
        0x01,
        0xd0,
        0xf9, // busy: lda $4212 ; and #1 ; bne busy
        0xad,
        0x19,
        0x42,
        0xaa, // lda $4219 ; tax
        0xad,
        0x18,
        0x42,
        0x4a,
        0xa8, // lda $4218 ; lsr a ; tay
        0x9c,
        0x21,
        0x21, // stz $2121
        0x8e,
        0x22,
        0x21, // stx $2122
        0x8c,
        0x22,
        0x21, // sty $2122
        0xad,
        0x12,
        0x42,
        0x30,
        0xfb, // out: lda $4212 ; bmi out (until vblank ends)
        0x4c,
        again as u8,
        (again >> 8) as u8, // jmp again
    ]);
    let mut file = vec![0u8; 0x8000];
    file[..code.len()].copy_from_slice(&code);
    // LoROM header at $7fc0.
    let title = b"SLOT PAD TEST        ";
    file[0x7fc0..0x7fc0 + 21].copy_from_slice(title);
    file[0x7fd5] = 0x20; // LoROM, slow
    file[0x7fd6] = 0x00; // ROM only
    file[0x7fd7] = 0x05; // 32 KB
    file[0x7fd8] = if sram { 0x03 } else { 0x00 }; // no RAM, or 8 KiB of save RAM
    file[0x7fd9] = 0x01; // USA
    file[0x7fda] = 0x33;
    for vector in [
        0x7fe4, 0x7fe6, 0x7fe8, 0x7fea, 0x7fee, 0x7ff4, 0x7ff8, 0x7ffa, 0x7ffc, 0x7ffe,
    ] {
        file[vector..vector + 2].copy_from_slice(&0x8000u16.to_le_bytes());
    }
    // The checksum is what tells the core this is LoROM: the four checksum bytes
    // always add to 0x1fe, so the sum of the rest plus that is the checksum.
    let sum: u32 = file
        .iter()
        .enumerate()
        .filter(|(i, _)| !(0x7fdc..0x7fe0).contains(i))
        .map(|(_, b)| *b as u32)
        .sum::<u32>()
        + 0x1fe;
    let sum = (sum & 0xffff) as u16;
    file[0x7fdc..0x7fde].copy_from_slice(&(sum ^ 0xffff).to_le_bytes());
    file[0x7fde..0x7fe0].copy_from_slice(&sum.to_le_bytes());
    let path = dir.join(name);
    std::fs::write(&path, file).unwrap();
    path
}

fn centre(c: &LibretroCore) -> [u8; 3] {
    let (w, h) = c.video_size();
    let px = c.video_xrgb8888();
    let i = (((h / 2) * w + w / 2) * 4) as usize;
    // Frames are BGRX.
    [px[i + 2], px[i + 1], px[i]]
}

fn colour_with(c: &mut LibretroCore, held: u16) -> [u8; 3] {
    for _ in 0..8 {
        c.run_frame(ButtonMask(held));
    }
    centre(c)
}

fn started(dir: &Path, name: &str) -> Option<LibretroCore> {
    let mut c = core(dir)?;
    c.set_native_video(true);
    c.load(&rom(dir, name)).expect("a SNES program loads");
    Some(c)
}

#[test]
fn the_core_takes_super_nintendo_files() {
    let _g = lock();
    let dir = scratch("snes-info");
    let Some(c) = core(&dir) else { return };
    let info = c.system_info();
    eprintln!("{info:?}");
    for ext in ["smc", "sfc"] {
        assert!(
            info.extensions.iter().any(|e| e == ext),
            "no .{ext}: {info:?}"
        );
    }
}

#[test]
fn a_super_nintendo_picture_is_drawn_at_its_own_size_and_not_cropped_to_the_gba_canvas() {
    let _g = lock();
    let dir = scratch("snes-video");
    let Some(mut c) = started(&dir, "idle.sfc") else {
        return;
    };
    for _ in 0..120 {
        c.run_frame(ButtonMask::default());
    }
    let (w, h) = c.video_size();
    eprintln!("video {w}x{h}, fps {:?}", c.av_info());
    assert_eq!(c.video_xrgb8888().len(), (w * h * 4) as usize);
    assert_eq!(w, 256, "a SNES line is 256 pixels wide");
    assert!((224..=239).contains(&h), "{h}");
    assert!(
        w > GBA_W && h > GBA_H,
        "{w}x{h} is not bigger than the GBA canvas"
    );
    let fps = c.av_info().fps;
    assert!(
        (59.0..61.0).contains(&fps) || (49.0..51.0).contains(&fps),
        "{fps}"
    );
}

#[test]
fn the_game_boy_canvas_path_is_unchanged_for_the_same_core() {
    let _g = lock();
    let dir = scratch("snes-canvas");
    let Some(mut c) = core(&dir) else { return };
    c.set_native_video(false);
    c.load(&rom(&dir, "idle.sfc"))
        .expect("a SNES program loads");
    for _ in 0..30 {
        c.run_frame(ButtonMask::default());
    }
    assert_eq!(
        c.video_size(),
        (GBA_W, GBA_H),
        "native video leaked into the default path"
    );
}

#[test]
fn it_makes_sound() {
    let _g = lock();
    let dir = scratch("snes-audio");
    let Some(mut c) = started(&dir, "idle.sfc") else {
        return;
    };
    let mut samples = 0;
    for _ in 0..60 {
        c.run_frame(ButtonMask::default());
        samples += c.take_audio().len();
    }
    assert!(samples > 50_000, "only {samples} samples in 60 frames");
}

#[test]
fn each_super_nintendo_button_reaches_the_core_as_its_own_button() {
    let _g = lock();
    let dir = scratch("snes-pad");
    let Some(mut c) = started(&dir, "pad.sfc") else {
        return;
    };
    let rest = colour_with(&mut c, 0);
    let mut seen = vec![rest];
    for (name, bit) in [
        ("B", ButtonMask::B),
        ("A", ButtonMask::A),
        ("X", ButtonMask::X),
        ("Y", ButtonMask::Y),
        ("L", ButtonMask::L),
        ("R", ButtonMask::R),
        ("Select", ButtonMask::SELECT),
        ("Start", ButtonMask::START),
        ("Up", ButtonMask::UP),
        ("Down", ButtonMask::DOWN),
        ("Left", ButtonMask::LEFT),
        ("Right", ButtonMask::RIGHT),
    ] {
        let shown = colour_with(&mut c, bit);
        assert!(
            !seen.contains(&shown),
            "{name} did nothing, or looked like another button: {shown:?} (seen {seen:?})"
        );
        seen.push(shown);
        assert_eq!(
            colour_with(&mut c, 0),
            rest,
            "{name} stayed down after it was let go"
        );
    }
}

#[test]
fn the_lower_triggers_are_not_a_super_nintendo_button() {
    let _g = lock();
    let dir = scratch("snes-l2");
    let Some(mut c) = started(&dir, "pad.sfc") else {
        return;
    };
    let rest = colour_with(&mut c, 0);
    assert_eq!(colour_with(&mut c, ButtonMask::L2), rest);
    assert_eq!(colour_with(&mut c, ButtonMask::R2), rest);
}

#[test]
fn a_state_saves_and_loads_back() {
    let _g = lock();
    let dir = scratch("snes-state");
    let Some(mut c) = started(&dir, "pad.sfc") else {
        return;
    };
    let held = colour_with(&mut c, ButtonMask::A);
    let state = c.serialize().expect("a state");
    assert!(!state.is_empty());
    colour_with(&mut c, 0);
    c.unserialize(&state).expect("the state loads back");
    for _ in 0..3 {
        c.run_frame(ButtonMask(ButtonMask::A));
    }
    assert_eq!(centre(&c), held);
}

#[test]
fn a_missing_file_is_an_error_and_a_junk_one_never_crashes_the_core() {
    let _g = lock();
    let dir = scratch("snes-bad");
    let Some(mut c) = core(&dir) else { return };
    assert!(c.load(&dir.join("nothing-here.sfc")).is_err());
    // This core takes any file that has some bytes in it and treats it as a cart
    // with nothing on it, so what matters is that running it is safe.
    let junk = dir.join("junk.sfc");
    std::fs::write(&junk, vec![0x5au8; 0x8000]).unwrap();
    let loaded = c.load(&junk);
    eprintln!("junk file: {loaded:?}");
    if loaded.is_ok() {
        for _ in 0..30 {
            c.run_frame(ButtonMask::default());
        }
        let (w, h) = c.video_size();
        assert_eq!(c.video_xrgb8888().len(), (w * h * 4) as usize);
    }
}

#[test]
fn a_super_nintendo_cart_offers_the_save_ram_the_game_wrote() {
    let _g = lock();
    let dir = scratch("snes-sram");
    let Some(mut c) = core(&dir) else { return };
    c.load(&rom_with(&dir, "saving.sfc", true)).unwrap();
    for _ in 0..20 {
        c.run_frame(ButtonMask::default());
    }
    let sav = c.save_ram().expect("a cart with save RAM offers it");
    assert_eq!(sav.len(), 8 * 1024, "the header asked for 8 KiB");
    assert_eq!(&sav[0..2], &[0xa5, 0x5a], "what the program wrote");
    assert_eq!(sav[0x1fff], 0x12, "and the far end of it");
}

#[test]
fn a_saved_super_nintendo_cart_comes_back_in_a_new_session() {
    let _g = lock();
    let dir = scratch("snes-sram-reload");
    let Some(mut c) = core(&dir) else { return };
    let rom = rom_with(&dir, "saving.sfc", true);
    c.load(&rom).unwrap();
    for _ in 0..20 {
        c.run_frame(ButtonMask::default());
    }
    let mut kept = c.save_ram().expect("save ram");
    kept[0x100] = 0x77;
    let path = dir.join("saving.sav");
    std::fs::write(&path, &kept).unwrap();
    drop(c);

    let mut next = core(&dir).expect("a second session");
    next.load(&rom).unwrap();
    next.load_save_ram(&std::fs::read(&path).unwrap())
        .expect("the kept save is accepted");
    assert_eq!(next.save_ram().expect("save ram"), kept);
    for _ in 0..5 {
        next.run_frame(ButtonMask::default());
    }
    assert_eq!(&next.save_ram().unwrap()[0..2], &[0xa5, 0x5a]);
}

#[test]
fn a_cart_without_save_ram_has_none_to_keep() {
    let _g = lock();
    let dir = scratch("snes-no-sram");
    let Some(mut c) = core(&dir) else { return };
    c.load(&rom(&dir, "plain.sfc")).unwrap();
    for _ in 0..10 {
        c.run_frame(ButtonMask::default());
    }
    let sav = c.save_ram();
    assert!(
        sav.is_none() || sav.as_deref() == Some(&[][..]),
        "a plain cart is not offered as a save: {sav:?}"
    );
}
