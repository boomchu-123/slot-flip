use slot_retro::{ButtonMask, LibretroCore, RetroCore, GBA_H, GBA_W};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

static CORE_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    CORE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn dylib() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor")
        .join(if cfg!(target_os = "macos") {
            "mgba_libretro.dylib"
        } else {
            "mgba_libretro.so"
        })
}

fn core() -> Option<LibretroCore> {
    match LibretroCore::open(&dylib()) {
        Ok(c) => Some(c),
        Err(_) => {
            eprintln!("no core available on this host, skipping");
            None
        }
    }
}

/// A header-only Game Boy ROM that spins in place: enough for the core to run
/// and draw a 160x144 picture. Not a dump of anything.
fn gb_rom() -> PathBuf {
    const LOGO: [u8; 48] = [
        0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00,
        0x0D, 0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD,
        0xD9, 0x99, 0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB,
        0xB9, 0x33, 0x3E,
    ];
    let mut rom = vec![0u8; 0x8000];
    rom[0x100..0x104].copy_from_slice(&[0x00, 0xC3, 0x50, 0x01]); // nop; jp $0150
    rom[0x104..0x134].copy_from_slice(&LOGO);
    rom[0x134..0x13A].copy_from_slice(b"SLOTGB");
    let sum = rom[0x134..=0x14C]
        .iter()
        .fold(0u8, |x, b| x.wrapping_sub(*b).wrapping_sub(1));
    rom[0x14D] = sum;
    rom[0x150..0x152].copy_from_slice(&[0x18, 0xFE]); // jr -2
    let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join("native-video.gb");
    std::fs::write(&p, rom).expect("write test rom");
    p
}

fn gba_rom() -> PathBuf {
    const CODE: [u32; 15] = [
        0xe3a00404, 0xe3a01c04, 0xe3811003, 0xe5801000, 0xe3a02406, 0xe3a03000, 0xe1d040b6,
        0xe35400a0, 0x1afffffc, 0xe2833001, 0xe1c230b0, 0xe1d040b6, 0xe35400a0, 0x0afffffc,
        0xeafffff6,
    ];
    let mut rom = vec![0u8; 0x8000];
    rom[0..4].copy_from_slice(&0xea00002eu32.to_le_bytes());
    rom[0xa0..0xac].copy_from_slice(b"SLOT TEST\0\0\0");
    rom[0xac..0xb0].copy_from_slice(b"SLTE");
    rom[0xb0..0xb2].copy_from_slice(b"00");
    rom[0xb2] = 0x96;
    let sum = rom[0xa0..0xbd].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    rom[0xbd] = 0u8.wrapping_sub(sum).wrapping_sub(0x19);
    for (i, w) in CODE.iter().enumerate() {
        let o = 0xc0 + i * 4;
        rom[o..o + 4].copy_from_slice(&w.to_le_bytes());
    }
    let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join("native-video.gba");
    std::fs::write(&p, rom).expect("write test rom");
    p
}

fn run(c: &mut LibretroCore, frames: usize) {
    for _ in 0..frames {
        c.run_frame(ButtonMask::default());
    }
}

#[test]
fn a_game_boy_picture_is_the_canvas_by_default_and_its_own_size_in_native_mode() {
    let _g = lock();
    let Some(mut c) = core() else { return };
    c.load(&gb_rom()).unwrap();

    run(&mut c, 10);
    assert_eq!(c.video_size(), (GBA_W, GBA_H), "default keeps the canvas");
    assert_eq!(c.video_xrgb8888().len(), (GBA_W * GBA_H * 4) as usize);

    c.set_native_video(true);
    run(&mut c, 10);
    assert_eq!(c.video_size(), (160, 144), "native keeps the core's size");
    assert_eq!(c.video_xrgb8888().len(), 160 * 144 * 4);

    c.set_native_video(false);
    assert_eq!(
        c.video_size(),
        (GBA_W, GBA_H),
        "off goes back to the canvas"
    );
    assert_eq!(c.video_xrgb8888().len(), (GBA_W * GBA_H * 4) as usize);
    run(&mut c, 10);
    assert_eq!(c.video_size(), (GBA_W, GBA_H));
}

#[test]
fn a_gba_frame_is_byte_identical_in_both_modes() {
    let _g = lock();
    let rom = gba_rom();

    let Some(mut c) = core() else { return };
    c.load(&rom).unwrap();
    run(&mut c, 90);
    let canvas = c.video_xrgb8888().to_vec();
    drop(c);

    let mut c = core().expect("the core was there a moment ago");
    c.set_native_video(true);
    c.load(&rom).unwrap();
    run(&mut c, 90);
    assert_eq!(c.video_size(), (GBA_W, GBA_H));
    assert_eq!(
        c.video_xrgb8888(),
        &canvas[..],
        "native mode must not change what a GBA game draws"
    );
}
