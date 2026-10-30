# Systems and licences

Every system slot has looked at, what would run it, and what its licence allows.
This is the audit behind the 1.0 plan in [ROADMAP.md](ROADMAP.md): a core is only
added after its licence has been read, and a system is only **supported** once it
has run on a Flip.

Legend: ✅ bundled and released · 🧪 runs, needs a real-core test and a pak build ·
📋 candidate, licence checked, not started · 🔎 licence or feasibility needs reading ·
❌ not viable on this hardware

## What is supported now

| System | Core | Licence | Since |
| --- | --- | --- | --- |
| Game Boy Advance | gpSP (default), mGBA | GPL-2.0 / MPL-2.0 | v0.1 |
| Game Boy, Game Boy Color | mGBA | MPL-2.0 | v0.1 |
| PlayStation | PCSX-ReARMed | GPL-2.0 | v0.7.0 |
| Nintendo Entertainment System, Famicom Disk System | fceumm | GPL-2.0 | v0.8.0 |
| Super Nintendo | snes9x2005 Plus | **non-commercial** (free-only) | v0.9.0 |
| Mega Drive / Genesis, Master System, Game Gear | Genesis Plus GX (Flip), PicoDrive (Mini Plus) | **non-commercial** (free-only) | v0.10.0 |

The two free-only cores mean the download as a whole must stay free; see
`NOTICE-NONCOMMERCIAL.txt` and `tools/check-pak-licences.sh`. Nothing is sold.

## Cores already bundled, systems still free

These need no new core and no new licence, so each is a small piece of work:
a `Spec` in `slot-store/src/platform.rs`, cart art in `slot-ui`, a pad mapping,
and a real-core test.

| System | Core | Needs | Status |
| --- | --- | --- | --- |
| Sega CD / Mega CD | Genesis Plus GX | a BIOS in `Slot/BIOS`, `.cue`/`.chd`/`.m3u` on the existing disc path (the PS1 code already does this), disc art | 📋 no new licence |
| SG-1000 / SC-3000 | Genesis Plus GX | cart art (`Games/SG`), 2-button pad, file name only | 📋 no new licence |
| Famicom Disk System | fceumm (already) | `disksys.rom` in `Slot/BIOS`; `.fds` is already an accepted extension, so this is mostly a test and a doc line | 🧪 needs checking on hardware |

Genesis Plus GX does **not** emulate the 32X (its README lists Genesis / Mega Drive,
Sega/Mega CD, Master System, Game Gear and SG-1000).

## Cores to add, licence checked

Licences read from each upstream repository's own licence file (GitHub's detected
licence shown; the file itself is the authority and is shipped with the core).

| System | Core (libretro repo) | Licence | Size/risk on a Flip | Status |
| --- | --- | --- | --- | --- |
| PC Engine / TurboGrafx-16 (HuCard) | beetle-pce-fast | GPL-2.0 | light, 2D | 📋 |
| Neo Geo Pocket, NGPC | beetle-ngp | GPL-2.0 | very light | 📋 |
| WonderSwan, WonderSwan Color | beetle-wswan | GPL-2.0 | very light | 📋 |
| Atari Lynx | beetle-lynx | GPL-2.0 | light | 📋 |
| Virtual Boy | beetle-vb | GPL-2.0 | moderate (red/black, 3D-ish) | 📋 |
| Atari 7800 | ProSystem | GPL-2.0 | light | 📋 |
| Intellivision | FreeIntv | GPL-2.0+ | light, awkward pad | 📋 |
| Sega 32X | PicoDrive | **non-commercial** (free-only) | moderate | 📋 same footing as GPGX |
| PC Engine CD | beetle-pce (full) | GPL-2.0 | heavier; needs a BIOS | 🔎 speed |
| Atari 2600 | stella2014 | 🔎 no licence detected by the host | light | 🔎 read the file first |
| MSX, ColecoVision | blueMSX | 🔎 mixed BSD/GPL/freeware, per file | light | 🔎 per-file audit before any use |

## Not viable, or not planned

| System | Core | Why |
| --- | --- | --- |
| Nintendo 64 | mupen64plus-next (GPL-2.0) | no evidence of full speed on a Flip; would need a dynarec and measurement first |
| Sega Saturn | beetle-saturn (GPL-2.0) | far too slow |
| Dreamcast | Flycast (GPL-2.0) | far too slow |
| PlayStation Portable | PPSSPP (GPL-2.0) | far too slow |
| 3DO | Opera | too slow, and catalogue overlap with PS1 is small |
| Arcade (MAME, FBNeo) | — | a different product (ROM sets, per-game config), not a shelf of consoles |
| MS-DOS, Amiga, Atari ST, C64, ZX Spectrum | — | computers need a keyboard and a mouse; out of scope for a handheld shelf |
| Nintendo DS, 3DS | — | too slow |
| PlayStation 2, GameCube, Wii | — | too slow |

## What adding a system costs

1. Read the core's licence; if it forbids commercial use, add it to the
   `free-only` list and to `NOTICE-NONCOMMERCIAL.txt`.
2. Add a `Platform` variant and a `Spec` (folder, extensions, picture size, pad,
   header, playlists) in `slot-store`.
3. Build the core in the device container, strip it, and ship the `.so` with its
   `.meta`, licence text and source archive (GPL compliance, not housekeeping).
4. Add the shelf art (a drawn cart or disc, and a console slot where it earns one).
5. Write a real-core test that proves the pad (a synthetic program that paints the
   controller port into the picture is the pattern used for NES, SNES and Sega).
6. Run a real game on a Flip, then add the README and this table.
