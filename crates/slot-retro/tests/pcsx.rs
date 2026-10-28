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

/// The device core, built by `cores/pcsx/build.sh`. Linux only, and absent
/// unless it has been built, so these tests skip quietly elsewhere.
fn core(dir: &Path) -> Option<LibretroCore> {
    let so = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/pcsx_rearmed_libretro.so");
    if !so.exists() {
        eprintln!("no pcsx_rearmed core built, skipping");
        return None;
    }
    Some(
        LibretroCore::open_with(&so, dir, dir)
            .expect("the pcsx core is present but would not open"),
    )
}

/// A PlayStation executable that does nothing but loop: a header and two
/// instructions (`j` to itself, then a nop). Content the core can be pointed at
/// by path without it being anyone's game, and without needing a BIOS or disc.
fn spin_exe(dir: &Path) -> PathBuf {
    let put = |b: &mut Vec<u8>, at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    let mut exe = vec![0u8; 2048 * 2];
    exe[..8].copy_from_slice(b"PS-X EXE");
    put(&mut exe, 0x10, 0x8001_0000); // initial pc
    put(&mut exe, 0x18, 0x8001_0000); // load address
    put(&mut exe, 0x1c, 2048); // size of the text that follows the header
    put(&mut exe, 0x30, 0x801f_fff0); // initial sp
    let region = b"Sony Computer Entertainment Inc. for North America area";
    exe[0x4c..0x4c + region.len()].copy_from_slice(region);
    put(&mut exe, 2048, 0x0800_4000); // j 0x80010000
    put(&mut exe, 2052, 0); // nop
    let path = dir.join("spin.exe");
    std::fs::write(&path, exe).unwrap();
    path
}

#[test]
fn the_core_says_it_opens_discs_itself_by_path() {
    let _g = lock();
    let dir = scratch("pcsx-info");
    let Some(c) = core(&dir) else { return };
    let info = c.system_info();
    eprintln!("{info:?}");
    assert!(info.need_fullpath, "a disc core must be handed a path");
    for ext in ["cue", "chd", "m3u"] {
        assert!(
            info.extensions.iter().any(|e| e == ext),
            "no .{ext}: {info:?}"
        );
    }
}

#[test]
fn content_is_loaded_by_path_and_drawn_at_its_own_size() {
    let _g = lock();
    let dir = scratch("pcsx-disc");
    let Some(mut c) = core(&dir) else { return };
    let exe = spin_exe(&dir);

    c.set_native_video(true);
    let loaded = c.load(&exe);
    eprintln!("load: {loaded:?}");
    for _ in 0..120 {
        c.run_frame(ButtonMask::default());
    }
    let (w, h) = c.video_size();
    eprintln!(
        "video {w}x{h}, {} bytes, fps {:?}",
        c.video_xrgb8888().len(),
        c.av_info()
    );
    assert!(loaded.is_ok(), "{loaded:?}");
    assert_eq!(c.video_xrgb8888().len(), (w * h * 4) as usize);
    assert!(
        w > GBA_W || h > GBA_H,
        "{w}x{h} should be bigger than the GBA canvas, not cropped to it"
    );
}

/// One raw MODE2/2352 disc image that boots the do-nothing program: just enough
/// ISO 9660 (a volume descriptor, a root directory holding `PSX.EXE`) for the
/// core to accept it. Nothing of anyone's game.
fn blank_disc(label: &str) -> Vec<u8> {
    const SECTOR: usize = 2352;
    const DATA: usize = 24;
    let mut disc = vec![0u8; SECTOR * 300];
    for (lba, s) in disc.chunks_exact_mut(SECTOR).enumerate() {
        s[1..11].fill(0xff);
        s[15] = 2;
        // Form 1 sub-header, twice.
        s[18] = 8;
        s[22] = 8;
        let _ = lba;
    }
    let put = |disc: &mut Vec<u8>, lba: usize, at: usize, bytes: &[u8]| {
        let o = lba * SECTOR + DATA + at;
        disc[o..o + bytes.len()].copy_from_slice(bytes);
    };

    // The licence sector the console checks the region against.
    put(
        &mut disc,
        4,
        60,
        b"Sony Computer Entertainment Inc. for North America area",
    );

    // Primary volume descriptor, with the root directory at sector 20.
    put(&mut disc, 16, 0, &[1]);
    put(&mut disc, 16, 1, b"CD001");
    put(&mut disc, 16, 40, label.as_bytes());
    let mut root = [0u8; 34];
    root[0] = 34;
    root[2..6].copy_from_slice(&20u32.to_le_bytes());
    root[10..14].copy_from_slice(&4096u32.to_le_bytes());
    root[25] = 2;
    root[32] = 1;
    put(&mut disc, 16, 156, &root);

    // The root directory: one file, the program, at sector 22.
    let name = b"PSX.EXE;1";
    let mut file = vec![0u8; 33 + name.len() + 1];
    file[0] = file.len() as u8;
    file[2..6].copy_from_slice(&22u32.to_le_bytes());
    file[10..14].copy_from_slice(&4096u32.to_le_bytes());
    file[32] = name.len() as u8;
    file[33..33 + name.len()].copy_from_slice(name);
    put(&mut disc, 20, 0, &file);

    // The program itself: a header sector and a sector of code.
    let tmp = std::env::temp_dir().join(format!("slot-blank-{label}"));
    std::fs::create_dir_all(&tmp).unwrap();
    let exe = std::fs::read(spin_exe(&tmp)).unwrap();
    put(&mut disc, 22, 0, &exe[..2048]);
    put(&mut disc, 23, 0, &exe[2048..4096]);
    disc
}

/// `n` blank discs (each a `.cue` and a `.bin`) and an `.m3u` naming them in order.
fn blank_discs(dir: &Path, n: usize) -> PathBuf {
    let mut list = String::new();
    for i in 1..=n {
        let bin = format!("Blank (Disc {i}).bin");
        let cue = format!("Blank (Disc {i}).cue");
        std::fs::write(dir.join(&bin), blank_disc(&format!("BLANK{i}"))).unwrap();
        std::fs::write(
            dir.join(&cue),
            format!("FILE \"{bin}\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n"),
        )
        .unwrap();
        list.push_str(&format!("{cue}\n"));
    }
    let m3u = dir.join("Blank.m3u");
    std::fs::write(&m3u, list).unwrap();
    m3u
}

fn run(c: &mut LibretroCore, frames: u32) {
    for _ in 0..frames {
        c.run_frame(ButtonMask::default());
    }
}

#[test]
fn a_playlist_of_three_discs_is_three_discs_to_change_between() {
    let _g = lock();
    let dir = scratch("pcsx-discs");
    let Some(mut c) = core(&dir) else { return };
    let m3u = blank_discs(&dir, 3);
    c.load(&m3u).expect("the core would not open the playlist");
    assert_eq!(c.disc_count(), 3);
    assert_eq!(c.disc_index(), 0);
    assert!(!c.disc_changing());
}

#[test]
fn a_disc_change_opens_the_lid_switches_the_disc_and_closes_it_again() {
    let _g = lock();
    let dir = scratch("pcsx-swap");
    let Some(mut c) = core(&dir) else { return };
    c.load(&blank_discs(&dir, 3)).unwrap();
    run(&mut c, 10);

    assert!(c.change_disc(2));
    assert!(c.disc_changing(), "the change is under way");
    assert_eq!(c.disc_index(), 2, "the menu shows where it is going");

    // Not done until the lid has been open for a while.
    run(&mut c, slot_retro::SWAP_SWITCH_FRAME);
    assert!(c.disc_changing());
    run(
        &mut c,
        slot_retro::SWAP_CLOSE_FRAME - slot_retro::SWAP_SWITCH_FRAME + 1,
    );
    assert!(!c.disc_changing(), "the lid should be shut again");
    assert_eq!(c.disc_index(), 2);

    // And back to the first, with a second request retargeting the first.
    assert!(c.change_disc(1));
    assert!(c.change_disc(0));
    run(&mut c, slot_retro::SWAP_CLOSE_FRAME + 2);
    assert!(!c.disc_changing());
    assert_eq!(c.disc_index(), 0);
}

#[test]
fn a_disc_that_is_already_in_is_not_changed() {
    let _g = lock();
    let dir = scratch("pcsx-swap-same");
    let Some(mut c) = core(&dir) else { return };
    c.load(&blank_discs(&dir, 2)).unwrap();
    assert!(c.change_disc(0));
    assert!(!c.disc_changing(), "nothing to open the lid for");
}

#[test]
fn there_is_no_disc_to_change_to_that_the_game_does_not_have() {
    let _g = lock();
    let dir = scratch("pcsx-swap-range");
    let Some(mut c) = core(&dir) else { return };
    c.load(&blank_discs(&dir, 2)).unwrap();
    assert!(!c.change_disc(2));
    assert!(!c.disc_changing());
}

#[test]
fn a_single_disc_game_has_nothing_to_change() {
    let _g = lock();
    let dir = scratch("pcsx-single");
    let Some(mut c) = core(&dir) else { return };
    let m3u = blank_discs(&dir, 1);
    c.load(&m3u).unwrap();
    assert!(c.disc_count() < 2);
    assert!(!c.change_disc(0));
}

const CARD: usize = 128 * 1024;

#[test]
fn a_memory_card_is_only_save_ram_when_the_core_is_told_to_manage_it_that_way() {
    let _g = lock();
    let dir = scratch("pcsx-card-default");
    let Some(mut c) = core(&dir) else { return };
    c.load(&spin_exe(&dir)).unwrap();
    assert_eq!(
        c.save_ram(),
        None,
        "with no option the card is not handed to the frontend, so nothing would be saved"
    );
}

#[test]
fn a_libretro_memory_card_is_one_blank_card_the_frontend_can_keep_and_give_back() {
    let _g = lock();
    let dir = scratch("pcsx-card");
    let Some(mut c) = core(&dir) else { return };
    c.set_option("pcsx_rearmed_memcard1", "libretro");
    c.set_option("pcsx_rearmed_memcard2", "none");
    c.load(&spin_exe(&dir)).unwrap();

    let blank = c.save_ram().expect("no save ram with the card managed");
    assert_eq!(blank.len(), CARD, "a PlayStation card is 128 KiB");

    let mut kept = blank.clone();
    kept[..2].copy_from_slice(b"MC");
    kept[CARD - 1] = 0x5a;
    c.load_save_ram(&kept).unwrap();
    assert_eq!(c.save_ram().as_deref(), Some(&kept[..]));
}

#[test]
fn a_memory_card_comes_back_whole_in_the_next_session() {
    let _g = lock();
    let dir = scratch("pcsx-card-session");
    let mut kept = vec![0u8; CARD];
    kept[..2].copy_from_slice(b"MC");
    kept[0x1234] = 0x77;
    {
        let Some(mut c) = core(&dir) else { return };
        c.set_option("pcsx_rearmed_memcard1", "libretro");
        c.load(&spin_exe(&dir)).unwrap();
        c.load_save_ram(&kept).unwrap();
        for _ in 0..30 {
            c.run_frame(ButtonMask::default());
        }
        assert_eq!(c.save_ram().as_deref(), Some(&kept[..]));
    }
    let Some(mut c) = core(&dir) else { return };
    c.set_option("pcsx_rearmed_memcard1", "libretro");
    c.load(&spin_exe(&dir)).unwrap();
    c.load_save_ram(&kept).unwrap();
    assert_eq!(c.save_ram().as_deref(), Some(&kept[..]));
}

#[test]
fn a_missing_disc_is_an_error_not_a_crash() {
    let _g = lock();
    let dir = scratch("pcsx-missing");
    let Some(mut c) = core(&dir) else { return };
    assert!(c.load(&dir.join("nothing-here.cue")).is_err());
}
