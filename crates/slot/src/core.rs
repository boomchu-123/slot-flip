use std::path::{Path, PathBuf};

use slot_retro::{LibretroCore, MockCore, RetroCore};
use slot_store::{Core, GbColors, Platform};

use crate::root;

const CORE_ENV: &str = "SLOT_CORE";

pub fn dylib_name(core: Core) -> String {
    let library = match core {
        // The core's short name is `pcsx`; the library keeps its upstream name.
        Core::Pcsx => "pcsx_rearmed",
        Core::Snes9x => "snes9x2005_plus",
        // The Mini Plus plays the Sega systems on PicoDrive, whose 68000 and Z80
        // cores are ARM assembly: Sonic the Hedgehog runs at 58 frames a second
        // there against 51 on Genesis Plus GX, and Master System and Game Gear at
        // 60.1 against 59. It is still `Core::Genesis` to the rest of Slot, which
        // only needs to know that one core plays all three.
        Core::Genesis if cfg!(feature = "miyoomini") => "picodrive",
        Core::Genesis => "genesis_plus_gx",
        other => other.as_str(),
    };
    format!("{library}_libretro.{}", std::env::consts::DLL_EXTENSION)
}

fn candidates(root: &Path, core: Core) -> Vec<PathBuf> {
    if let Some(named) = std::env::var_os(CORE_ENV) {
        return vec![PathBuf::from(named)];
    }
    let name = dylib_name(core);
    let mut paths = vec![root.join("System").join(&name)];
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        paths.push(dir.join(&name));
        paths.push(dir.join("vendor").join(&name));
    }
    paths.push(Path::new("vendor").join(&name));
    paths
}

pub struct Opened {
    pub core: Box<dyn RetroCore>,
    pub named: bool,
}

pub fn open_core(
    root: &Path,
    core: Core,
    platform: Platform,
    serial: &str,
    colour: bool,
    palette: Option<GbColors>,
    link: Option<u8>,
) -> Opened {
    let paths = candidates(root, core);
    match open_named(root, core, platform, serial, colour, palette, link, &paths) {
        Some(core) => Opened { core, named: true },
        None => {
            report_missing(core, &paths);
            Opened {
                core: Box::new(MockCore::new()),
                named: false,
            }
        }
    }
}

pub fn open_core_for(
    root: &Path,
    core: Core,
    serial: &str,
    colour: bool,
    paths: &[PathBuf],
) -> Box<dyn RetroCore> {
    open_core_for_palette(root, core, Platform::Gba, serial, colour, None, paths)
}

pub fn open_core_for_palette(
    root: &Path,
    core: Core,
    platform: Platform,
    serial: &str,
    colour: bool,
    palette: Option<GbColors>,
    paths: &[PathBuf],
) -> Box<dyn RetroCore> {
    open_named(root, core, platform, serial, colour, palette, None, paths).unwrap_or_else(|| {
        report_missing(core, paths);
        Box::new(MockCore::new())
    })
}

#[allow(clippy::too_many_arguments)]
fn open_named(
    root: &Path,
    core: Core,
    platform: Platform,
    serial: &str,
    colour: bool,
    palette: Option<GbColors>,
    link: Option<u8>,
    paths: &[PathBuf],
) -> Option<Box<dyn RetroCore>> {
    let bios = root::bios_dir(root);
    let saves = root::saves_dir(root);
    for path in paths {
        if !path.exists() {
            continue;
        }
        match LibretroCore::open_with(path, &bios, &saves) {
            Ok(mut opened) => {
                // A PlayStation or NES picture is not the GBA canvas the Game Boy
                // family is drawn into, so it keeps the size the core draws it at.
                opened.set_native_video(platform.native_video());
                apply_core_options(
                    &mut opened,
                    core,
                    platform,
                    serial,
                    root::has_real_bios(root),
                    colour,
                    palette,
                );
                if let Some(player) = link {
                    apply_link_options(&mut opened, core, player);
                }
                eprintln!("slot: core {}", path.display());
                return Some(Box::new(opened));
            }
            Err(e) => eprintln!("slot: {}: {e}", path.display()),
        }
    }
    None
}

pub fn palette_for(
    chosen: GbColors,
    platform: Platform,
    rom: &Path,
    link: Option<u8>,
) -> Option<GbColors> {
    // Only a Game Boy cart has a header to ask; a disc image's bytes are not one.
    (link.is_none() && matches!(platform, Platform::Gb | Platform::Gbc) && dmg_only(rom))
        .then_some(chosen)
}

pub fn dmg_only(rom: &Path) -> bool {
    use std::io::Read;
    let mut header = [0u8; 0x150];
    std::fs::File::open(rom)
        .and_then(|mut f| f.read_exact(&mut header))
        .is_ok_and(|_| header[0x143] & 0x80 == 0)
}

fn report_missing(core: Core, paths: &[PathBuf]) {
    eprintln!(
        "slot: no {} core found, running the mock test pattern instead. Looked in: {}",
        core.as_str(),
        paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
}

pub fn colour_option(
    which: Core,
    platform: Platform,
    on: bool,
) -> Option<(&'static str, &'static str)> {
    match which {
        Core::Mgba => Some((
            "mgba_color_correction",
            if on && platform == Platform::Gb {
                "GBC"
            } else if on {
                "Auto"
            } else {
                "OFF"
            },
        )),
        Core::Gpsp => Some((
            "gpsp_color_correction",
            if on { "enabled" } else { "disabled" },
        )),
        // A console picture has no handheld-LCD colour to correct.
        Core::Pcsx | Core::Fceumm | Core::Snes9x | Core::Genesis => None,
    }
}

pub fn apply_link_options(core: &mut LibretroCore, which: Core, player: u8) {
    if which != Core::Mgba {
        return;
    }
    core.set_option("mgba_link", "on");
    core.set_option("mgba_link_player", &player.to_string());
    eprintln!("slot: core: link mode on, player {player}");
}

/// What PCSX-ReARMed is told, every time. A core's own default never applies here:
/// slot answers "no value" for an option it has not set, and the core then keeps
/// its zero, which is not its documented default. That has already cost PlayStation
/// its memory cards (v0.11) and its analog toggle, so each option that matters is
/// named, and `pcsx_options_name_what_matters` fails if one goes missing.
pub fn pcsx_options() -> &'static [(&'static str, &'static str)] {
    &[
        // The dynarec is what lets these handhelds run PlayStation games at full
        // speed (without one mGBA ran ~20x slower), so say so rather than trust a
        // default.
        ("pcsx_rearmed_drc", "enabled"),
        // A real BIOS from BIOS/ when there is one, the built-in replacement when not.
        ("pcsx_rearmed_bios", "auto"),
        // Card 1 is handed to the frontend as save RAM, so it is kept per game with
        // the other saves. Left unset, the core asks for the option, gets nothing and
        // exposes no save RAM at all, so a game's save never reaches the saves
        // folder. Card 2 is off, not the core's shared file, which would write into
        // the saves folder outside anything the frontend tracks.
        ("pcsx_rearmed_memcard1", "libretro"),
        ("pcsx_rearmed_memcard2", "none"),
        // A DualShock starts in digital mode, where the core ignores the sticks until
        // the game switches the pad to analog. This is the combination that switches
        // it by hand, as on a real pad's analog button: both sticks pressed in. Left
        // unset the combination is dead (the core's own default, L1+R1+SELECT, never
        // applies), and L1+R1+SELECT would collide with slot's SELECT shortcuts
        // anyway. It does nothing unless Analog Sticks is on, because only then is
        // the pad a DualShock.
        ("pcsx_rearmed_analog_combo", "l3+r3"),
        // Left unset, the core assumes "square" bounds: it scales each axis by 1.27
        // and clips, so full deflection arrives at about 79% of the stick's travel
        // and steering is coarse. Slot already applies a round dead zone and keeps
        // the stick's own range, which is what "circle" passes through untouched.
        ("pcsx_rearmed_analog_axis_modifier", "circle"),
    ]
}

pub fn apply_core_options(
    core: &mut LibretroCore,
    which: Core,
    platform: Platform,
    serial: &str,
    bios: bool,
    colour: bool,
    palette: Option<GbColors>,
) {
    // PCSX-ReARMed names and values this differently, and fceumm has no such option;
    // a console runs at full speed on the Flip, so each keeps its own defaults.
    if !matches!(
        which,
        Core::Pcsx | Core::Fceumm | Core::Snes9x | Core::Genesis
    ) {
        core.set_option(&format!("{}_frameskip", which.as_str()), "auto");
    }
    if which == Core::Mgba {
        core.set_option("mgba_sgb_borders", "OFF");
        core.set_option("mgba_force_gbp", "ON");
        match palette {
            Some(p) => {
                core.set_option("mgba_gb_model", "Game Boy");
                core.set_option("mgba_gb_colors_preset", "0");
                core.set_option("mgba_gb_colors", p.core_name());
            }
            None => {
                core.set_option("mgba_gb_model", "Autodetect");
                core.set_option("mgba_gb_colors_preset", "1");
                core.set_option("mgba_gb_colors", "GBC Dark Green →A");
            }
        }
        if let Some((key, value)) = colour_option(which, platform, colour) {
            core.set_option(key, value);
        }
    }
    if which == Core::Pcsx {
        for (key, value) in pcsx_options() {
            core.set_option(key, value);
        }
    }
    if which == Core::Gpsp {
        core.set_option("gpsp_serial", serial);
        core.set_option("gpsp_rtc_time_source", "system");
        if bios {
            core.set_option("gpsp_boot_mode", "bios");
        }
        if let Some((key, value)) = colour_option(which, platform, colour) {
            core.set_option(key, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn pcsx_options_name_what_matters() {
        let has = |k: &str, v: &str| pcsx_options().contains(&(k, v));
        assert!(has("pcsx_rearmed_memcard1", "libretro"), "memory cards");
        assert!(has("pcsx_rearmed_memcard2", "none"));
        assert!(has("pcsx_rearmed_drc", "enabled"), "the dynarec");
        assert!(
            has("pcsx_rearmed_analog_combo", "l3+r3"),
            "the DualShock analog toggle is dead unless it is set explicitly"
        );
        assert!(
            has("pcsx_rearmed_analog_axis_modifier", "circle"),
            "unset, the core clips the sticks at 79% of their travel"
        );
    }

    #[test]
    fn no_pcsx_option_is_set_twice() {
        let mut keys: Vec<_> = pcsx_options().iter().map(|(k, _)| *k).collect();
        keys.sort_unstable();
        let n = keys.len();
        keys.dedup();
        assert_eq!(
            keys.len(),
            n,
            "an option is listed twice, so the last wins silently"
        );
    }

    #[test]
    fn the_playstation_core_keeps_its_upstream_library_name() {
        assert!(dylib_name(Core::Pcsx).starts_with("pcsx_rearmed_libretro."));
        assert!(dylib_name(Core::Mgba).starts_with("mgba_libretro."));
        assert!(dylib_name(Core::Gpsp).starts_with("gpsp_libretro."));
    }

    #[test]
    fn a_console_core_has_no_colour_correction_to_set() {
        assert_eq!(colour_option(Core::Pcsx, Platform::Ps1, true), None);
        assert_eq!(colour_option(Core::Pcsx, Platform::Ps1, false), None);
    }

    #[test]
    fn each_core_spells_colour_correction_its_own_way() {
        assert_eq!(
            colour_option(Core::Mgba, Platform::Gba, true),
            Some(("mgba_color_correction", "Auto"))
        );
        assert_eq!(
            colour_option(Core::Mgba, Platform::Gba, false),
            Some(("mgba_color_correction", "OFF"))
        );
        assert_eq!(
            colour_option(Core::Gpsp, Platform::Gba, true),
            Some(("gpsp_color_correction", "enabled"))
        );
        assert_eq!(
            colour_option(Core::Gpsp, Platform::Gba, false),
            Some(("gpsp_color_correction", "disabled"))
        );
    }

    #[test]
    fn mgba_uses_gbc_correction_for_game_boy_games() {
        assert_eq!(
            colour_option(Core::Mgba, Platform::Gb, true),
            Some(("mgba_color_correction", "GBC"))
        );
    }

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn candidates_search_the_named_cores_own_filename_only() {
        let _g = lock();
        std::env::remove_var(CORE_ENV);

        let root = Path::new("/root");
        let mgba = candidates(root, Core::Mgba);
        let gpsp = candidates(root, Core::Gpsp);
        assert_ne!(mgba, gpsp, "the two cores searched the same paths");
        assert!(!mgba.is_empty());
        assert!(!gpsp.is_empty());
        for path in &mgba {
            let name = path.file_name().unwrap().to_str().unwrap();
            assert!(name.starts_with("mgba_libretro"), "{name} is not mGBA's");
        }
        for path in &gpsp {
            let name = path.file_name().unwrap().to_str().unwrap();
            assert!(name.starts_with("gpsp_libretro"), "{name} is not gpSP's");
        }
    }

    #[test]
    fn candidates_search_the_roots_own_system_directory() {
        let _g = lock();
        std::env::remove_var(CORE_ENV);
        let root = Path::new("/some/content/root");
        assert_eq!(
            candidates(root, Core::Gpsp)[0],
            root.join("System").join(dylib_name(Core::Gpsp)),
        );
    }

    #[test]
    fn the_env_override_ignores_which_core_was_asked_for() {
        let _g = lock();
        std::env::set_var(CORE_ENV, "/dev/null/named-core");
        let root = Path::new("/root");
        let mgba = candidates(root, Core::Mgba);
        let gpsp = candidates(root, Core::Gpsp);
        std::env::remove_var(CORE_ENV);
        assert_eq!(mgba, vec![PathBuf::from("/dev/null/named-core")]);
        assert_eq!(
            mgba, gpsp,
            "the override stopped winning for one of the cores"
        );
    }
}

#[cfg(test)]
mod library_names {
    use super::*;

    #[test]
    fn the_sega_core_is_picodrive_on_the_mini_plus_and_genesis_plus_gx_elsewhere() {
        let want = if cfg!(feature = "miyoomini") {
            "picodrive"
        } else {
            "genesis_plus_gx"
        };
        assert_eq!(
            dylib_name(Core::Genesis),
            format!("{want}_libretro.{}", std::env::consts::DLL_EXTENSION)
        );
    }

    #[test]
    fn the_other_cores_keep_their_library_names() {
        let ext = std::env::consts::DLL_EXTENSION;
        assert_eq!(dylib_name(Core::Pcsx), format!("pcsx_rearmed_libretro.{ext}"));
        assert_eq!(
            dylib_name(Core::Snes9x),
            format!("snes9x2005_plus_libretro.{ext}")
        );
        assert_eq!(dylib_name(Core::Fceumm), format!("fceumm_libretro.{ext}"));
    }
}
