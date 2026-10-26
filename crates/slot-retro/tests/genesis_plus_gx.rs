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

/// The device core, built by `cores/genesis_plus_gx/build.sh`. Linux only, and absent
/// unless it has been built, so these tests skip quietly elsewhere.
fn core(dir: &Path) -> Option<LibretroCore> {
    let so = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/genesis_plus_gx_libretro.so");
    if !so.exists() {
        eprintln!("no genesis_plus_gx core built, skipping");
        return None;
    }
    Some(
        LibretroCore::open_with(&so, dir, dir)
            .expect("the genesis_plus_gx core is present but would not open"),
    )
}

// Three programs that are nobody's games. Each one, once a frame in vertical blank,
// paints colour 0 (the backdrop, which is the whole picture) from the pad, so the
// picture says what the core was handed. The bytes were assembled from the listings
// beside them: the Z80 ones from the comments, the 68000 ones instruction by
// instruction (the text on each line is what the bytes before it encode).
//
// Master System (Z80): Up Down Left Right Button1 Button2 are bits 0..5 of a
// --BBGGRR colour, so Up and Down are red 1 and 2, Left and Right green 1 and 2,
// Button1 and Button2 blue 1 and 2. Start is the Pause NMI: each one flips a flag,
// and with it set the colour is a fixed grey no single button makes.
//
// Game Gear (Z80): colour 0 is 12-bit: Up Down Left Right are red 1 2 4 8, Button1
// and Button2 green 1 and 2, and Start (port 0, bit 7) blue 1.
//
// Mega Drive (68000), two programs. A reads a 3-button pad and spreads its eight
// bits (Up Down Left Right B C A Start) over the colour: red 1 2 4, green 1 2 4,
// blue 1 2. B reads the six-button sequence (seven TH writes) and shows Z Y X Mode
// as red 1 2 4 and green 1.
// The Master System program:  head 9 nmi 11 body 62; header 544d522053454741000000000000004c
const SMS_HEAD: &[u8] = &[0xf3, 0xed, 0x56, 0x31, 0xf0, 0xdf, 0xc3, 0x00, 0x01];
const SMS_NMI: &[u8] = &[
    0xf5, 0x3a, 0x00, 0xc0, 0x3c, 0x32, 0x00, 0xc0, 0xf1, 0xed, 0x45,
];
const SMS_BODY: &[u8] = &[
    0x3e, 0x04, 0xd3, 0xbf, 0x3e, 0x80, 0xd3, 0xbf, 0x3e, 0x40, 0xd3, 0xbf, 0x3e, 0x81, 0xd3, 0xbf,
    0xaf, 0xd3, 0xbf, 0x3e, 0x87, 0xd3, 0xbf, 0xaf, 0x32, 0x00, 0xc0, 0xdb, 0xbf, 0xe6, 0x80, 0x28,
    0xfa, 0x3a, 0x00, 0xc0, 0xe6, 0x01, 0x28, 0x04, 0x06, 0x2a, 0x18, 0x06, 0xdb, 0xdc, 0x2f, 0xe6,
    0x3f, 0x47, 0xaf, 0xd3, 0xbf, 0x3e, 0xc0, 0xd3, 0xbf, 0x78, 0xd3, 0xbe, 0x18, 0xdd,
];
// The Game Gear program:  head 9 nmi 0 body 50; header 544d522053454741000000000000006c
const GG_HEAD: &[u8] = &[0xf3, 0xed, 0x56, 0x31, 0xf0, 0xdf, 0xc3, 0x00, 0x01];
const GG_BODY: &[u8] = &[
    0x3e, 0x04, 0xd3, 0xbf, 0x3e, 0x80, 0xd3, 0xbf, 0x3e, 0x40, 0xd3, 0xbf, 0x3e, 0x81, 0xd3, 0xbf,
    0xdb, 0xbf, 0xe6, 0x80, 0x28, 0xfa, 0xdb, 0xdc, 0x2f, 0xe6, 0x3f, 0x47, 0xdb, 0x00, 0x2f, 0xe6,
    0x80, 0x07, 0x4f, 0xaf, 0xd3, 0xbf, 0x3e, 0xc0, 0xd3, 0xbf, 0x78, 0xd3, 0xbe, 0x79, 0xd3, 0xbe,
    0x18, 0xde,
];
const MD_A: &[(&[u8], &str)] = &[
    (&[0x46, 0xfc, 0x27, 0x00], "move.w #$2700,sr"),
    (&[0x43, 0xf9, 0x00, 0xc0, 0x00, 0x04], "lea $C00004,a1"),
    (&[0x41, 0xf9, 0x00, 0xc0, 0x00, 0x00], "lea $C00000,a0"),
    (&[0x45, 0xf9, 0x00, 0xa1, 0x00, 0x03], "lea $A10003,a2"),
    (&[0x32, 0xbc, 0x80, 0x04], "move.w #$8004,(a1)"),
    (&[0x32, 0xbc, 0x81, 0x44], "move.w #$8144,(a1)"),
    (&[0x32, 0xbc, 0x87, 0x00], "move.w #$8700,(a1)"),
    (&[0x32, 0xbc, 0x8f, 0x02], "move.w #$8F02,(a1)"),
    (
        &[0x13, 0xfc, 0x00, 0x40, 0x00, 0xa1, 0x00, 0x09],
        "move.b #$40,$A10009",
    ),
    (&[0x14, 0xbc, 0x00, 0x40], "move.b #$40,(a2)"),
    (&[0x30, 0x11], "main: move.w (a1),d0"),
    (&[0x08, 0x00, 0x00, 0x03], "btst #3,d0"),
    (&[0x67, 0xf8], "beq.s main"),
    (&[0x14, 0xbc, 0x00, 0x40], "move.b #$40,(a2)"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x12, 0x12], "move.b (a2),d1"),
    (&[0x14, 0xbc, 0x00, 0x00], "move.b #$00,(a2)"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x14, 0x12], "move.b (a2),d2"),
    (&[0x14, 0xbc, 0x00, 0x40], "move.b #$40,(a2)"),
    (&[0x46, 0x01], "not.b d1"),
    (&[0x46, 0x02], "not.b d2"),
    (&[0x02, 0x41, 0x00, 0x3f], "andi.w #$3F,d1"),
    (&[0x02, 0x42, 0x00, 0x30], "andi.w #$30,d2"),
    (&[0xe5, 0x4a], "lsl.w #2,d2"),
    (&[0x82, 0x42], "or.w d2,d1"),
    (&[0x38, 0x01], "move.w d1,d4"),
    (&[0x02, 0x44, 0x00, 0x07], "andi.w #7,d4"),
    (&[0xd8, 0x44], "add.w d4,d4"),
    (&[0x3a, 0x01], "move.w d1,d5"),
    (&[0x02, 0x45, 0x00, 0x38], "andi.w #$38,d5"),
    (&[0xe5, 0x4d], "lsl.w #2,d5"),
    (&[0x3c, 0x01], "move.w d1,d6"),
    (&[0x02, 0x46, 0x00, 0xc0], "andi.w #$C0,d6"),
    (&[0xe7, 0x4e], "lsl.w #3,d6"),
    (&[0x88, 0x45], "or.w d5,d4"),
    (&[0x88, 0x46], "or.w d6,d4"),
    (
        &[0x22, 0xbc, 0xc0, 0x00, 0x00, 0x00],
        "move.l #$C0000000,(a1)",
    ),
    (&[0x30, 0x84], "move.w d4,(a0)"),
    (&[0x30, 0x11], "wend: move.w (a1),d0"),
    (&[0x08, 0x00, 0x00, 0x03], "btst #3,d0"),
    (&[0x66, 0xf8], "bne.s wend"),
    (&[0x60, 0x00, 0xff, 0x9a], "bra.w main"),
];
const MD_B: &[(&[u8], &str)] = &[
    (&[0x46, 0xfc, 0x27, 0x00], "move.w #$2700,sr"),
    (&[0x43, 0xf9, 0x00, 0xc0, 0x00, 0x04], "lea $C00004,a1"),
    (&[0x41, 0xf9, 0x00, 0xc0, 0x00, 0x00], "lea $C00000,a0"),
    (&[0x45, 0xf9, 0x00, 0xa1, 0x00, 0x03], "lea $A10003,a2"),
    (&[0x32, 0xbc, 0x80, 0x04], "move.w #$8004,(a1)"),
    (&[0x32, 0xbc, 0x81, 0x44], "move.w #$8144,(a1)"),
    (&[0x32, 0xbc, 0x87, 0x00], "move.w #$8700,(a1)"),
    (&[0x32, 0xbc, 0x8f, 0x02], "move.w #$8F02,(a1)"),
    (
        &[0x13, 0xfc, 0x00, 0x40, 0x00, 0xa1, 0x00, 0x09],
        "move.b #$40,$A10009",
    ),
    (&[0x14, 0xbc, 0x00, 0x40], "move.b #$40,(a2)"),
    (&[0x30, 0x11], "main: move.w (a1),d0"),
    (&[0x08, 0x00, 0x00, 0x03], "btst #3,d0"),
    (&[0x67, 0xf8], "beq.s main"),
    (&[0x14, 0xbc, 0x00, 0x40], "move.b #$40,(a2)"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x14, 0xbc, 0x00, 0x00], "move.b #$00,(a2)"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x14, 0xbc, 0x00, 0x40], "move.b #$40,(a2)"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x14, 0xbc, 0x00, 0x00], "move.b #$00,(a2)"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x14, 0xbc, 0x00, 0x40], "move.b #$40,(a2)"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x14, 0xbc, 0x00, 0x00], "move.b #$00,(a2)"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x14, 0xbc, 0x00, 0x40], "move.b #$40,(a2)"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x4e, 0x71], "nop"),
    (&[0x12, 0x12], "move.b (a2),d1"),
    (&[0x46, 0x01], "not.b d1"),
    (&[0x02, 0x41, 0x00, 0x0f], "andi.w #$0F,d1"),
    (&[0x38, 0x01], "move.w d1,d4"),
    (&[0x02, 0x44, 0x00, 0x07], "andi.w #7,d4"),
    (&[0xd8, 0x44], "add.w d4,d4"),
    (&[0x3a, 0x01], "move.w d1,d5"),
    (&[0x02, 0x45, 0x00, 0x08], "andi.w #8,d5"),
    (&[0xe5, 0x4d], "lsl.w #2,d5"),
    (&[0x88, 0x45], "or.w d5,d4"),
    (
        &[0x22, 0xbc, 0xc0, 0x00, 0x00, 0x00],
        "move.l #$C0000000,(a1)",
    ),
    (&[0x30, 0x84], "move.w d4,(a0)"),
    (&[0x30, 0x11], "wend: move.w (a1),d0"),
    (&[0x08, 0x00, 0x00, 0x03], "btst #3,d0"),
    (&[0x66, 0xf8], "bne.s wend"),
    (&[0x60, 0x00, 0xff, 0x78], "bra.w main"),
];

fn sms_rom(dir: &Path, name: &str, head: &[u8], nmi: &[u8], body: &[u8], region: u8) -> PathBuf {
    let mut file = vec![0u8; 0x8000];
    file[..head.len()].copy_from_slice(head);
    file[0x66..0x66 + nmi.len()].copy_from_slice(nmi);
    file[0x100..0x100 + body.len()].copy_from_slice(body);
    file[0x7ff0..0x7ff8].copy_from_slice(b"TMR SEGA");
    file[0x7fff] = region;
    let path = dir.join(name);
    std::fs::write(&path, file).unwrap();
    path
}

fn md_rom(dir: &Path, name: &str, program: &[(&[u8], &str)]) -> PathBuf {
    md_rom_with(dir, name, program, false)
}

/// `md_rom` with the cartridge's save RAM declared in the header, or without.
fn md_rom_with(dir: &Path, name: &str, program: &[(&[u8], &str)], sram: bool) -> PathBuf {
    let mut file = vec![0u8; 0x20000];
    let (idle, start) = (0x200usize, 0x202usize);
    file[0..4].copy_from_slice(&0x00ff_0000u32.to_be_bytes());
    file[4..8].copy_from_slice(&(start as u32).to_be_bytes());
    for vector in 2..64 {
        file[vector * 4..vector * 4 + 4].copy_from_slice(&(idle as u32).to_be_bytes());
    }
    let mut header = Vec::new();
    header.extend(b"SEGA MEGA DRIVE ");
    header.extend(b"(C)SLOT 2026.OCT");
    header.extend(format!("{:<48}", "SLOT PAD TEST").bytes());
    header.extend(format!("{:<48}", "SLOT PAD TEST").bytes());
    header.extend(b"GM 00000000-00\0\0");
    // The I/O support field: "6" is what makes the core give the game a six-button pad.
    header.extend(format!("{:<16}", "J6").bytes());
    header.extend(0u32.to_be_bytes());
    header.extend(0x1ffffu32.to_be_bytes());
    header.extend(0x00ff_0000u32.to_be_bytes());
    header.extend(0x00ff_ffffu32.to_be_bytes());
    header.extend(std::iter::repeat_n(b' ', 12 + 12 + 40));
    header.extend(format!("{:<16}", "JUE").bytes());
    file[0x100..0x100 + header.len()].copy_from_slice(&header);
    if sram {
        // "RA" and a window is how a Mega Drive cart says it has battery-backed
        // save RAM. It sits at $200000, just past this 128 KiB ROM, and the end
        // is the last byte of 8 KiB.
        file[0x1b0..0x1b2].copy_from_slice(b"RA");
        file[0x1b4..0x1b8].copy_from_slice(&0x0020_0000u32.to_be_bytes());
        file[0x1b8..0x1bc].copy_from_slice(&0x0020_1fffu32.to_be_bytes());
    }
    file[idle..idle + 2].copy_from_slice(&[0x4e, 0x73]);
    let mut at = start;
    for (bytes, _) in program {
        file[at..at + bytes.len()].copy_from_slice(bytes);
        at += bytes.len();
    }
    let path = dir.join(name);
    std::fs::write(&path, file).unwrap();
    path
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Console {
    MegaDriveThree,
    MegaDriveSix,
    MasterSystem,
    GameGear,
}

fn rom(dir: &Path, which: Console) -> PathBuf {
    match which {
        Console::MegaDriveThree => md_rom(dir, "pad3.md", MD_A),
        Console::MegaDriveSix => md_rom(dir, "pad6.md", MD_B),
        Console::MasterSystem => sms_rom(dir, "pad.sms", SMS_HEAD, SMS_NMI, SMS_BODY, 0x4c),
        Console::GameGear => sms_rom(dir, "pad.gg", GG_HEAD, &[], GG_BODY, 0x6c),
    }
}

fn started(dir: &Path, which: Console) -> Option<LibretroCore> {
    let mut c = core(dir)?;
    c.set_native_video(true);
    c.load(&rom(dir, which)).expect("the program loads");
    Some(c)
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

/// The one colour channel that is lit, and how brightly: `(0|1|2, value)` for red,
/// green or blue. Panics if the picture is black or two channels are lit, which
/// would mean the program saw more than one button.
fn only(rgb: [u8; 3]) -> (usize, u8) {
    let lit: Vec<usize> = (0..3).filter(|&i| rgb[i] != 0).collect();
    assert_eq!(lit.len(), 1, "expected one channel lit, got {rgb:?}");
    (lit[0], rgb[lit[0]])
}

const RED: usize = 0;
const GREEN: usize = 1;
const BLUE: usize = 2;

/// Presses each button on its own and checks it lights the channel and level the
/// program is built to show for it, brighter for the higher level, and that letting
/// go puts the picture back.
fn check(c: &mut LibretroCore, buttons: &[(&str, u16, usize, u8)]) {
    let rest = colour_with(c, 0);
    assert_eq!(
        rest,
        [0, 0, 0],
        "the picture is not black with nothing held"
    );
    let mut levels: Vec<(usize, u8, u8)> = Vec::new();
    for &(name, bit, channel, level) in buttons {
        let (got_channel, value) = only(colour_with(c, bit));
        assert_eq!(
            got_channel, channel,
            "{name} lit the wrong channel ({got_channel}, wanted {channel})"
        );
        levels.push((channel, level, value));
        assert_eq!(
            colour_with(c, 0),
            rest,
            "{name} stayed down after it was let go"
        );
    }
    // On one channel a higher level has to be a brighter value.
    for (i, a) in levels.iter().enumerate() {
        for b in &levels[i + 1..] {
            if a.0 == b.0 {
                assert_eq!(
                    a.1.cmp(&b.1),
                    a.2.cmp(&b.2),
                    "levels {} and {} on channel {} came out in the wrong order ({} and {})",
                    a.1,
                    b.1,
                    a.0,
                    a.2,
                    b.2
                );
            }
        }
    }
}

#[test]
fn the_core_takes_every_sega_file() {
    let _g = lock();
    let dir = scratch("gpgx-info");
    let Some(c) = core(&dir) else { return };
    let info = c.system_info();
    eprintln!("{info:?}");
    for ext in ["md", "gen", "smd", "bin", "sms", "gg"] {
        assert!(
            info.extensions.iter().any(|e| e == ext),
            "no .{ext}: {info:?}"
        );
    }
}

#[test]
fn each_system_is_drawn_at_its_own_size_and_not_cropped_to_the_gba_canvas() {
    let _g = lock();
    let dir = scratch("gpgx-video");
    for (which, w_ok, h_ok) in [
        (Console::MegaDriveThree, vec![256, 320], vec![224, 240]),
        (Console::MasterSystem, vec![256], vec![192, 224, 240]),
        (Console::GameGear, vec![160], vec![144]),
    ] {
        let Some(mut c) = started(&dir, which) else {
            return;
        };
        for _ in 0..120 {
            c.run_frame(ButtonMask::default());
        }
        let (w, h) = c.video_size();
        eprintln!("{which:?}: video {w}x{h}, {:?}", c.av_info());
        assert_eq!(c.video_xrgb8888().len(), (w * h * 4) as usize);
        assert!(w_ok.contains(&w), "{which:?}: width {w}");
        assert!(h_ok.contains(&h), "{which:?}: height {h}");
        let fps = c.av_info().fps;
        assert!(
            (59.0..61.0).contains(&fps) || (49.0..51.0).contains(&fps),
            "{which:?}: {fps}"
        );
    }
}

#[test]
fn the_game_boy_canvas_path_is_unchanged_for_the_same_core() {
    let _g = lock();
    let dir = scratch("gpgx-canvas");
    let Some(mut c) = core(&dir) else { return };
    c.set_native_video(false);
    c.load(&rom(&dir, Console::MegaDriveThree))
        .expect("a Mega Drive program loads");
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
    let dir = scratch("gpgx-audio");
    for which in [Console::MegaDriveThree, Console::MasterSystem] {
        let Some(mut c) = started(&dir, which) else {
            return;
        };
        let mut samples = 0;
        for _ in 0..60 {
            c.run_frame(ButtonMask::default());
            samples += c.take_audio().len();
        }
        assert!(
            samples > 50_000,
            "{which:?}: only {samples} samples in 60 frames"
        );
    }
}

#[test]
fn a_three_button_mega_drive_pad_reads_a_b_c_from_the_left_bottom_and_right_buttons() {
    let _g = lock();
    let dir = scratch("gpgx-md3");
    let Some(mut c) = started(&dir, Console::MegaDriveThree) else {
        return;
    };
    check(
        &mut c,
        &[
            ("Up", ButtonMask::UP, RED, 1),
            ("Down", ButtonMask::DOWN, RED, 2),
            ("Left", ButtonMask::LEFT, RED, 4),
            ("Right", ButtonMask::RIGHT, GREEN, 1),
            // Mega Drive B is the bottom button, as on the Flip.
            ("B", ButtonMask::B, GREEN, 2),
            // C is the right-hand one, which the Flip prints as A.
            ("C (the Flip's A)", ButtonMask::A, GREEN, 4),
            // A is the left-hand one, which the Flip prints as Y.
            ("A (the Flip's Y)", ButtonMask::Y, BLUE, 1),
            ("Start", ButtonMask::START, BLUE, 2),
        ],
    );
}

#[test]
fn a_six_button_mega_drive_pad_adds_x_y_z_and_mode_on_the_top_button_and_the_shoulders() {
    let _g = lock();
    let dir = scratch("gpgx-md6");
    let Some(mut c) = started(&dir, Console::MegaDriveSix) else {
        return;
    };
    check(
        &mut c,
        &[
            // Z is the right shoulder, Y the top button, X the left shoulder.
            ("Z (the Flip's R)", ButtonMask::R, RED, 1),
            ("Y (the Flip's X)", ButtonMask::X, RED, 2),
            ("X (the Flip's L)", ButtonMask::L, RED, 4),
            ("Mode (Select)", ButtonMask::SELECT, GREEN, 1),
        ],
    );
}

#[test]
fn the_lower_triggers_are_not_a_mega_drive_button() {
    let _g = lock();
    let dir = scratch("gpgx-md-l2");
    let Some(mut c) = started(&dir, Console::MegaDriveThree) else {
        return;
    };
    assert_eq!(colour_with(&mut c, ButtonMask::L2), [0, 0, 0]);
    assert_eq!(colour_with(&mut c, ButtonMask::R2), [0, 0, 0]);
}

#[test]
fn a_master_system_pad_has_two_buttons_on_the_bottom_and_right_face_buttons() {
    let _g = lock();
    let dir = scratch("gpgx-sms");
    let Some(mut c) = started(&dir, Console::MasterSystem) else {
        return;
    };
    check(
        &mut c,
        &[
            ("Up", ButtonMask::UP, RED, 1),
            ("Down", ButtonMask::DOWN, RED, 2),
            ("Left", ButtonMask::LEFT, GREEN, 1),
            ("Right", ButtonMask::RIGHT, GREEN, 2),
            ("Button 1", ButtonMask::B, BLUE, 1),
            ("Button 2", ButtonMask::A, BLUE, 2),
        ],
    );
}

#[test]
fn start_is_the_master_systems_pause_and_it_toggles() {
    let _g = lock();
    let dir = scratch("gpgx-sms-pause");
    let Some(mut c) = started(&dir, Console::MasterSystem) else {
        return;
    };
    assert_eq!(colour_with(&mut c, 0), [0, 0, 0]);
    // Held down and let go: the pause flag is now set, and stays set.
    colour_with(&mut c, ButtonMask::START);
    let paused = colour_with(&mut c, 0);
    assert_ne!(paused, [0, 0, 0], "Start did not reach the Pause line");
    // Grey, give or take what 5- and 6-bit channels can say.
    assert!(
        paused[0].abs_diff(paused[1]) < 8 && paused[1].abs_diff(paused[2]) < 8,
        "the pause colour is the grey the program shows: {paused:?}"
    );
    assert_eq!(colour_with(&mut c, 0), paused, "it did not stay paused");
    colour_with(&mut c, ButtonMask::START);
    assert_eq!(
        colour_with(&mut c, 0),
        [0, 0, 0],
        "a second Start did not undo it"
    );
}

#[test]
fn a_game_gear_pad_has_two_buttons_and_a_start() {
    let _g = lock();
    let dir = scratch("gpgx-gg");
    let Some(mut c) = started(&dir, Console::GameGear) else {
        return;
    };
    check(
        &mut c,
        &[
            ("Up", ButtonMask::UP, RED, 1),
            ("Down", ButtonMask::DOWN, RED, 2),
            ("Left", ButtonMask::LEFT, RED, 4),
            ("Right", ButtonMask::RIGHT, RED, 8),
            ("Button 1", ButtonMask::B, GREEN, 1),
            ("Button 2", ButtonMask::A, GREEN, 2),
            ("Start", ButtonMask::START, BLUE, 1),
        ],
    );
}

#[test]
fn a_state_saves_and_loads_back_on_each_system() {
    let _g = lock();
    let dir = scratch("gpgx-state");
    for which in [
        Console::MegaDriveThree,
        Console::MasterSystem,
        Console::GameGear,
    ] {
        let Some(mut c) = started(&dir, which) else {
            return;
        };
        let held = colour_with(&mut c, ButtonMask::A);
        let state = c.serialize().expect("a state");
        assert!(!state.is_empty(), "{which:?}");
        colour_with(&mut c, 0);
        c.unserialize(&state).expect("the state loads back");
        for _ in 0..3 {
            c.run_frame(ButtonMask(ButtonMask::A));
        }
        assert_eq!(centre(&c), held, "{which:?}");
    }
}

#[test]
fn a_missing_file_is_an_error_and_a_junk_one_never_crashes_the_core() {
    let _g = lock();
    let dir = scratch("gpgx-bad");
    let Some(mut c) = core(&dir) else { return };
    assert!(c.load(&dir.join("nothing-here.md")).is_err());
    for name in ["junk.md", "junk.sms", "junk.gg"] {
        let junk = dir.join(name);
        std::fs::write(&junk, vec![0x5au8; 0x8000]).unwrap();
        let loaded = c.load(&junk);
        eprintln!("{name}: {loaded:?}");
        if loaded.is_ok() {
            for _ in 0..30 {
                c.run_frame(ButtonMask::default());
            }
            let (w, h) = c.video_size();
            assert_eq!(c.video_xrgb8888().len(), (w * h * 4) as usize);
        }
    }
}

/// A Mega Drive program that writes a signature to each end of the cart's save
/// RAM and then idles.
const MD_SRAM: &[(&[u8], &str)] = &[
    (&[0x46, 0xfc, 0x27, 0x00], "move.w #$2700,sr"),
    (
        &[0x13, 0xfc, 0x00, 0xa5, 0x00, 0x20, 0x00, 0x00],
        "move.b #$a5,$00200000",
    ),
    (
        &[0x13, 0xfc, 0x00, 0x5a, 0x00, 0x20, 0x00, 0x01],
        "move.b #$5a,$00200001",
    ),
    (
        &[0x13, 0xfc, 0x00, 0x12, 0x00, 0x20, 0x1f, 0xff],
        "move.b #$12,$00201fff",
    ),
    (&[0x60, 0xfe], "bra.s *"),
];

#[test]
fn a_mega_drive_cart_offers_the_save_ram_the_game_wrote() {
    let _g = lock();
    let dir = scratch("genesis-sram");
    let Some(mut c) = core(&dir) else { return };
    c.load(&md_rom_with(&dir, "save.md", MD_SRAM, true))
        .unwrap();
    for _ in 0..20 {
        c.run_frame(ButtonMask::default());
    }
    let sav = c.save_ram().expect("a cart with save RAM offers it");
    assert!(sav.len() >= 8 * 1024, "8 KiB was declared: {}", sav.len());
    assert_eq!(&sav[0..2], &[0xa5, 0x5a], "what the program wrote");
    assert_eq!(sav[0x1fff], 0x12, "and the far end of it");
}

#[test]
fn a_saved_mega_drive_cart_comes_back_in_a_new_session() {
    let _g = lock();
    let dir = scratch("genesis-sram-reload");
    let Some(mut c) = core(&dir) else { return };
    let rom = md_rom_with(&dir, "save.md", MD_SRAM, true);
    c.load(&rom).unwrap();
    for _ in 0..20 {
        c.run_frame(ButtonMask::default());
    }
    let mut kept = c.save_ram().expect("save ram");
    kept[0x100] = 0x77;
    let path = dir.join("save.sav");
    std::fs::write(&path, &kept).unwrap();
    drop(c);

    let mut next = core(&dir).expect("a second session");
    next.load(&rom).unwrap();
    next.load_save_ram(&std::fs::read(&path).unwrap())
        .expect("the kept save is accepted");
    // Genesis Plus GX reports the cart's declared 8 KiB until a game has
    // written to it, then reports the largest window it would allow (64 KiB on
    // this cart). The bytes are what matter: the save slot kept has to come
    // back at the same offsets, with the rest left as the core fills it.
    let back = next.save_ram().expect("save ram");
    assert!(back.len() >= kept.len(), "{} < {}", back.len(), kept.len());
    assert_eq!(
        &back[..kept.len()],
        &kept[..],
        "every byte that was saved came back"
    );
    for _ in 0..5 {
        next.run_frame(ButtonMask::default());
    }
    assert_eq!(&next.save_ram().unwrap()[0..2], &[0xa5, 0x5a]);
}

#[test]
fn a_mega_drive_cart_without_save_ram_has_none_to_keep() {
    let _g = lock();
    let dir = scratch("genesis-no-sram");
    let Some(mut c) = core(&dir) else { return };
    c.load(&md_rom(&dir, "pad3.md", MD_A)).unwrap();
    for _ in 0..10 {
        c.run_frame(ButtonMask::default());
    }
    let sav = c.save_ram();
    assert!(
        sav.is_none() || sav.as_deref() == Some(&[][..]),
        "a plain cart is not offered as a save: {sav:?}"
    );
}
