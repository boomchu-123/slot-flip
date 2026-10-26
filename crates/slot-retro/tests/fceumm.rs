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

/// The device core, built by `cores/fceumm/build.sh`. Linux only, and absent
/// unless it has been built, so these tests skip quietly elsewhere.
fn core(dir: &Path) -> Option<LibretroCore> {
    let so = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/fceumm_libretro.so");
    if !so.exists() {
        eprintln!("no fceumm core built, skipping");
        return None;
    }
    Some(
        LibretroCore::open_with(&so, dir, dir)
            .expect("the fceumm core is present but would not open"),
    )
}

/// A NES program that is nobody's game: it reads the controller port every
/// frame and paints the backdrop colour from which buttons are down, so the
/// picture says what the core was handed. `shift` drops that many buttons off
/// the bottom first, to reach the last ones in a colour of their own.
///
/// The port reads A, B, Select, Start, Up, Down, Left, Right, in that order.
fn rom(dir: &Path, name: &str, shift: usize) -> PathBuf {
    let mut code: Vec<u8> = vec![
        0x78, // sei
        0xa2, 0xff, // ldx #$ff
        0x9a, // txs
    ];
    let again = 0x8000 + code.len() as u16;
    code.extend([
        0xa9, 0x01, 0x8d, 0x16, 0x40, // lda #1 ; sta $4016  (strobe)
        0xa9, 0x00, 0x8d, 0x16, 0x40, // lda #0 ; sta $4016
        0xa2, 0x08, // ldx #8
        0xa9, 0x00, 0x85, 0x00, // lda #0 ; sta $00
        // read: lda $4016 ; and #1 ; lsr ; ror $00 ; dex ; bne read
        0xad, 0x16, 0x40, 0x29, 0x01, 0x4a, 0x66, 0x00, 0xca, 0xd0, 0xf5, //
        0xa5, 0x00, // lda $00
    ]);
    code.extend(std::iter::repeat_n(0x4a, shift)); // lsr a
    code.extend([
        0x29,
        0x3f, // and #$3f
        0xaa, // tax
        0xa9,
        0x3f,
        0x8d,
        0x06,
        0x20, // lda #$3f ; sta $2006
        0xa9,
        0x00,
        0x8d,
        0x06,
        0x20, // lda #0   ; sta $2006
        0x8e,
        0x07,
        0x20, // stx $2007  (the backdrop colour)
        0x4c,
        again as u8,
        (again >> 8) as u8, // jmp again
    ]);
    let mut prg = vec![0u8; 16 * 1024];
    prg[..code.len()].copy_from_slice(&code);
    for vector in [0x3ffa, 0x3ffc, 0x3ffe] {
        prg[vector..vector + 2].copy_from_slice(&0x8000u16.to_le_bytes());
    }
    let mut file = vec![0x4e, 0x45, 0x53, 0x1a, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    file.extend(prg);
    file.extend(vec![0u8; 8 * 1024]);
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
    for _ in 0..6 {
        c.run_frame(ButtonMask(held));
    }
    centre(c)
}

#[test]
fn the_core_takes_nes_files_and_reads_them_itself() {
    let _g = lock();
    let dir = scratch("fceumm-info");
    let Some(c) = core(&dir) else { return };
    let info = c.system_info();
    eprintln!("{info:?}");
    for ext in ["nes", "fds", "unf"] {
        assert!(
            info.extensions.iter().any(|e| e == ext),
            "no .{ext}: {info:?}"
        );
    }
}

#[test]
fn a_nes_picture_is_drawn_at_its_own_size_and_not_cropped_to_the_gba_canvas() {
    let _g = lock();
    let dir = scratch("fceumm-video");
    let Some(mut c) = core(&dir) else { return };
    c.set_native_video(true);
    c.load(&rom(&dir, "idle.nes", 0))
        .expect("a NES program loads");
    for _ in 0..120 {
        c.run_frame(ButtonMask::default());
    }
    let (w, h) = c.video_size();
    eprintln!("video {w}x{h}, fps {:?}", c.av_info());
    assert_eq!(c.video_xrgb8888().len(), (w * h * 4) as usize);
    assert_eq!(w, 256, "a NES line is 256 pixels wide");
    assert!((224..=240).contains(&h), "{h}");
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
    let dir = scratch("fceumm-canvas");
    let Some(mut c) = core(&dir) else { return };
    c.set_native_video(false);
    c.load(&rom(&dir, "idle.nes", 0))
        .expect("a NES program loads");
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
    let dir = scratch("fceumm-audio");
    let Some(mut c) = core(&dir) else { return };
    c.set_native_video(true);
    c.load(&rom(&dir, "idle.nes", 0)).unwrap();
    let mut samples = 0;
    for _ in 0..60 {
        c.run_frame(ButtonMask::default());
        samples += c.take_audio().len();
    }
    // About a second of stereo audio.
    assert!(samples > 60_000, "only {samples} samples in 60 frames");
}

#[test]
fn each_nes_button_reaches_the_core_as_its_own_button() {
    let _g = lock();
    let dir = scratch("fceumm-pad");
    let Some(mut c) = core(&dir) else { return };
    c.set_native_video(true);
    c.load(&rom(&dir, "pad.nes", 0)).unwrap();
    let rest = colour_with(&mut c, 0);
    let mut seen = vec![rest];
    for (name, bit) in [
        ("A", ButtonMask::A),
        ("B", ButtonMask::B),
        ("Select", ButtonMask::SELECT),
        ("Start", ButtonMask::START),
        ("Up", ButtonMask::UP),
        ("Down", ButtonMask::DOWN),
    ] {
        let shown = colour_with(&mut c, bit);
        assert!(
            !seen.contains(&shown),
            "{name} did nothing, or looked like another button: {shown:?}"
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
fn left_and_right_reach_the_core_too() {
    let _g = lock();
    let dir = scratch("fceumm-pad2");
    let Some(mut c) = core(&dir) else { return };
    c.set_native_video(true);
    // Two buttons off the bottom, so Select and Start take the lowest colours and
    // Left and Right the highest, none of them sharing one with another.
    c.load(&rom(&dir, "pad2.nes", 2)).unwrap();
    let rest = colour_with(&mut c, 0);
    let left = colour_with(&mut c, ButtonMask::LEFT);
    let right = colour_with(&mut c, ButtonMask::RIGHT);
    let select = colour_with(&mut c, ButtonMask::SELECT);
    assert!(
        left != rest && right != rest && left != right,
        "{rest:?} {left:?} {right:?}"
    );
    assert!(select != rest && select != left && select != right);
}

#[test]
fn the_turbo_keys_do_not_press_a_button_of_their_own() {
    let _g = lock();
    let dir = scratch("fceumm-turbo");
    let Some(mut c) = core(&dir) else { return };
    c.set_native_video(true);
    c.load(&rom(&dir, "pad.nes", 0)).unwrap();
    let rest = colour_with(&mut c, 0);
    // X and Y never reach a NES pad: the frontend turns them into pulsing A and B.
    assert_eq!(colour_with(&mut c, ButtonMask::X), rest);
    assert_eq!(colour_with(&mut c, ButtonMask::Y), rest);
}

#[test]
fn a_state_saves_and_loads_back() {
    let _g = lock();
    let dir = scratch("fceumm-state");
    let Some(mut c) = core(&dir) else { return };
    c.set_native_video(true);
    c.load(&rom(&dir, "pad.nes", 0)).unwrap();
    let held = colour_with(&mut c, ButtonMask::A);
    let state = c.serialize().expect("a state");
    assert!(!state.is_empty());
    colour_with(&mut c, 0);
    c.unserialize(&state).expect("the state loads back");
    for _ in 0..2 {
        c.run_frame(ButtonMask(ButtonMask::A));
    }
    assert_eq!(centre(&c), held);
}

#[test]
fn a_file_that_is_not_a_nes_game_is_an_error_not_a_crash() {
    let _g = lock();
    let dir = scratch("fceumm-bad");
    let Some(mut c) = core(&dir) else { return };
    let junk = dir.join("junk.nes");
    std::fs::write(&junk, b"this is not a ROM").unwrap();
    assert!(c.load(&junk).is_err());
    assert!(c.load(&dir.join("nothing-here.nes")).is_err());
}

/// A battery-backed NES cart: the same hand-assembled program, but the header
/// says the cartridge keeps its save RAM, and the program writes a signature
/// into it at $6000 and $7FFF, the two ends of the 8 KiB the NES gives a cart.
fn battery_rom(dir: &Path, name: &str) -> PathBuf {
    let mut code: Vec<u8> = vec![
        0x78, // sei
        0xa2, 0xff, // ldx #$ff
        0x9a, // txs
        0xa9, 0xa5, // lda #$a5
        0x8d, 0x00, 0x60, // sta $6000
        0xa9, 0x5a, // lda #$5a
        0x8d, 0x01, 0x60, // sta $6001
        0xa9, 0x12, // lda #$12
        0x8d, 0xff, 0x7f, // sta $7fff
    ];
    let here = 0x8000 + code.len() as u16;
    code.extend([0x4c, here as u8, (here >> 8) as u8]); // jmp here
    let mut prg = vec![0u8; 16 * 1024];
    prg[..code.len()].copy_from_slice(&code);
    for vector in [0x3ffa, 0x3ffc, 0x3ffe] {
        prg[vector..vector + 2].copy_from_slice(&0x8000u16.to_le_bytes());
    }
    let mut header = [0u8; 16];
    header[..4].copy_from_slice(&[0x4e, 0x45, 0x53, 0x1a]); // iNES
    header[4] = 1; // 16 KiB of program
    header[5] = 1; // 8 KiB of character
                   // Bit 1 of byte 6 is the battery, which is what makes the core expose this
                   // cartridge's save RAM at all.
    header[6] = 0b0000_0010;
    let mut file = header.to_vec();
    file.extend(prg);
    file.extend(vec![0u8; 8 * 1024]);
    let path = dir.join(name);
    std::fs::write(&path, file).unwrap();
    path
}

#[test]
fn a_battery_cart_offers_the_save_ram_the_game_wrote() {
    let _g = lock();
    let dir = scratch("fceumm-sram");
    let Some(mut c) = core(&dir) else { return };
    c.load(&battery_rom(&dir, "battery.nes")).unwrap();
    for _ in 0..20 {
        c.run_frame(ButtonMask::default());
    }
    let sav = c.save_ram().expect("a battery cart has save ram");
    assert_eq!(sav.len(), 8 * 1024, "the NES gives a cart 8 KiB");
    assert_eq!(&sav[0..2], &[0xa5, 0x5a], "what the program wrote at $6000");
    assert_eq!(sav[0x1fff], 0x12, "and at the far end, $7fff");
}

#[test]
fn a_saved_cart_battery_comes_back_in_a_new_session() {
    let _g = lock();
    let dir = scratch("fceumm-sram-reload");
    let Some(mut c) = core(&dir) else { return };
    let rom = battery_rom(&dir, "battery.nes");
    c.load(&rom).unwrap();
    for _ in 0..20 {
        c.run_frame(ButtonMask::default());
    }
    let kept = c.save_ram().expect("save ram");
    let mut edited = kept.clone();
    edited[0x100] = 0x77;
    // What the frontend writes to the card and hands back next boot.
    let path = dir.join("battery.sav");
    std::fs::write(&path, &edited).unwrap();
    drop(c);

    let mut next = core(&dir).expect("a second session");
    next.load(&rom).unwrap();
    next.load_save_ram(&std::fs::read(&path).unwrap())
        .expect("the kept save is accepted");
    let back = next.save_ram().expect("save ram");
    assert_eq!(back, edited, "the whole 8 KiB came back untouched");
    for _ in 0..5 {
        next.run_frame(ButtonMask::default());
    }
    assert_eq!(
        &next.save_ram().unwrap()[0..2],
        &[0xa5, 0x5a],
        "the game still sees what it wrote"
    );
}
