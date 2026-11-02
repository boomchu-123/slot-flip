# Getting 1.0 on the Miyoo Mini Plus

A working plan for the second 1.0 device. It records what has been measured, what
the 1.0 gate still asks for, the order the remaining work should be done in, and
what "done" means. Numbers here are from a real console unless they say otherwise.

**Device:** Miyoo Mini Plus (`miyoomini`), kernel 4.9.84, glibc 2.28, no GPU, so
the CPU compositor in `crates/slot-gfx/src/soft/` is the renderer. 640x480 panel,
about **103 MB** of RAM usable by Linux. Stock firmware; Slot boots from the card
with no MinUI.

**Branch:** `fix/miyoomini-capability-words` (PR #76), carrying the port that began
on `feat/miyoomini-software`. Everything below is, or is to be, measured on hardware
driven over adb, not inferred from a build.

**Last updated:** 2026-11-02.

**Status: not releasable as 1.0.** The first real play (2026-11-01, a 32 MB GBA hack)
was choppy throughout, and there is no way to leave Slot for the console's own menu
(Wi-Fi setup, for one). Sections 1 and 3 say what that changes.

---

## 1. Where it stands

Published: **`exp-v0.5.0`**, a pre-release (not Latest) with `Slot-miyoomini.zip`,
the zip that was built locally and hardware-tested. It predates PicoDrive, so it
still plays Mega Drive at 51 fps. The next pre-release should carry the work below.

Measured on a console, one game per system, 40 to 50 seconds each, emulator frames
a second from Slot's own trace:

| System | Core | Speed (core's rate) | Notes |
| --- | --- | --- | --- |
| NES | fceumm | 50.1 of 50 | a PAL game, so correct |
| SNES | snes9x2005 Plus | **51.7** of ~60 (56.6 with rewind off) | short |
| Mega Drive | **PicoDrive** | 58.2 of 60 | was 51.0 on Genesis Plus GX |
| Master System | PicoDrive | 60.1 of 60 | was 58.6 |
| Game Gear | PicoDrive | 60.1 of 60 | was 59.7 |
| Game Boy | mGBA | 58.3 of 59.7 | |
| Game Boy Color | mGBA | **55.9** of 59.7 (59.9 with rewind off) | short |
| Game Boy Advance | gpSP | 58.7 of 59.7 | **disputed**: a 32 MB hack ran 54.2 at its title (rewind off), 59.8 for a second one, and was choppy in play |
| PlayStation | PCSX-ReARMed | **48 to 52** of 59.9 | short; 21 MB spare after four minutes |

The Mega Drive, Master System and Game Gear rows are from the zip with PicoDrive
installed the way a user would; the rest are from the exp-v0.5.0 build, and the
SNES and GBC rows were re-measured on 2026-10-31 for the rewind comparison.

**How far to trust this table.** Every row is an average over 40 to 50 seconds,
usually at a title screen or intro, with no measure of how evenly frames arrive and no
audio underrun count (those print only when a game ends, and the sweep killed Slot).
The two 32 MB hacks were measured with rewind **off** by accident, and a player runs
with it on. The GBA row failed its first real test: the table said full speed, the
player said choppy everywhere. Treat the whole table as an upper bound until section 3
item 2 is done.

**What each frame costs.** The picture path is about 2.5 ms a frame (it was about
15). What is left is the emulator thread itself. Genesis Plus GX took about 16 ms of
CPU for a Mega Drive frame, and a byte-identical rebuild showed the build was not at
fault; PicoDrive's ARM-assembly CPU cores are what fixed it.

**Rewind is a real cost on this console.** Recording history costs Super Nintendo
about five frames a second and Game Boy Color about four. With it off, Game Boy
Color is at full speed and Super Nintendo reaches 56.6. It is on by default.

### The gate this has to meet

From `ROADMAP.md`, 1.0 means, for **each** device:

1. The nine systems each run a real game **at the core's own frame rate, with
   sound**, on the firmware the device ships with.
2. PlayStation works **end to end**: full speed on a heavy 3D game, a memory
   card read back after a reboot, and a disc change.
3. **One zip** unzipped at the card root gives a running Slot on the stock
   firmware with **no MinUI installed**, plus `install.sh` with `--check` and
   `--dry-run`, checked on hardware.
4. `tools/check-pak-licences.sh` passes, the README table matches reality, and
   the release itself has been installed and run on the device.

Not gates: playing every title through, judging sound by ear, L3/R3 in a game.

---

## 2. Done

- **Picture path** (was plan item A): the plain scale copies bytes instead of
  multiplying 16-bit lanes, and the game pass fell from about 7.4 ms to 2.0 to
  2.4 ms. Verified by reading back the panel for every system.
- **PlayStation memory** (was B1): the core's translation cache is 4 MiB instead of
  16 (a build-time patch in `cores/pcsx`), pictures not on screen are kept packed,
  and the allocator is limited in `launch.sh`. It no longer gets killed.
- **Install** (was C): `Slot-miyoomini.zip`, one `install.sh` for both devices (34
  installer tests), `App/Slot`, and `miyoo354/app/MainUI` booting into Slot with the
  console's own launcher kept as `MainUI.stock`. A fresh install from the zip, the
  `Slot/Config/boot-menu` escape and the way back were all run on the console.
- **Docs** (was D): README, INSTALL, COMPATIBILITY and ROADMAP describe this device.
- **Release job**: CI builds both device zips. It failed twice on a missing `App/`
  directory in the Mini Plus job and passes since `943f3d2`; `pr-checks` and
  `secrets` pass on the branch.
- **Mega Drive, Master System, Game Gear on PicoDrive** (`cores/picodrive`). It is
  built with Zig and glibc 2.17, with its assembly sent to GNU as; its source and
  licence ship in the pak and the notice names it. The Flip still uses Genesis Plus
  GX.
- **Resume states** round-trip across a restart (Pokemon Emerald, Crash Bandicoot).

---

## 3. Remaining for 1.0, in order

Items 1 to 3 were added on 2026-11-01 and come before everything else, because
nothing below them can be judged, or even debugged away from the bench, without them.

1. **Quit Slot from inside Slot (both devices, every release).** *Built; run on the
   Mini Plus, not on the Flip.* An Exit row in the
   menu that hands over to the stock launcher for this session and leaves nothing on
   the card, so the next boot is Slot. Both autoboot wrappers must tell "Slot exited,
   start it again" from "the player asked to leave"; Slot stays in the stock App list
   as the way back. Check whether the Flip wrapper's fall-back on a non-zero exit
   already gives most of this. Until it ships, INSTALL.md tells people to create
   `Slot/Config/boot-menu` before they travel. Done when it is run on both consoles:
   Exit, set Wi-Fi in the stock menu, come back to Slot from the App list, reboot into
   Slot.
2. **Measure sustained play, then fix what it shows.** *Started: the CPU governor,
   core pinning and gpSP's sound rate took a 32 MB hack from about 104 to 118-121
   frames per two seconds.
   The maintainer found overworld play much better and playable, but battles still
   felt a little slow. A 100-second trace covering two battles confirmed it: 115.0 of
   120 frames per two seconds on average, 105 worst, while audio starvation kept
   growing. Rewind was off. The core itself averaged 8-12 ms, exposing small missed
   deadlines being discarded by the software pacer. After letting a cheap frame recover
   that time, a second battle trace held all 42 windows at 120-122 with no new audio
   starvation. Battles felt much better but perhaps slightly slow; 44 of 5,100 pictures
   had been repeated by the earlier adaptive skip, which is removed for the next test.
   The per-system sweep is not redone.* Add to the trace, while a game
   runs: emulator frames per two seconds over several minutes (with the worst window),
   audio dropped and starved counts, and frame time spread. Then take one game per
   system into its heavy parts (overworld, battle, a 3D scene) with rewind **on**, and
   compare a 32 MB GBA ROM with an 8 MB one to test whether the card is being
   re-read during play (about 12 MB of memory spare with a 32 MB ROM). A system is
   done only on that measurement. The maintainer calling a game choppy is a failure
   whatever the average says.
3. **Settings and About that fit this console.** *The General page was checked on the
   console: Auto Sleep is shown, while Lid Power Off and Rumble are hidden. The About
   sticker is done in code and has not been looked at on the console.* Today the About sticker prints
   `MODEL NO. MY355` and `MIYOO FLIP`, Lid Power Off is offered with no lid, and the
   Rumble row is shown (no motor on this console, to be confirmed). Give each device
   one description of itself (name, model, which rows apply), hide what cannot work,
   add an **auto sleep** setting that works without a lid, and review every other
   row once against the device. Test: a menu test per device that lists the rows it
   shows.
4. **Make rewind cheaper on this console.** It is the largest known cost left and
   the most certain win. Options, cheapest first: snapshot less often than every
   second presented frame here (`SNAPSHOT_EVERY` in `emu.rs`), record only while a
   rewind button is held (nothing to rewind to then, so decide on behaviour), or
   keep the snapshot off the emulator thread. Measure SNES and GBC with and without,
   and keep a few seconds of history if possible.
5. **Super Nintendo.** Still about 3 frames short with rewind off. Measured once: the
   non-Plus snes9x2005 build reached 53.0 against 51.4 with rewind on (small, one
   run). An ARM-mode build of the current core was built and not yet measured.
   snes9x2002 builds and loads for this console (glibc 2.7) and has not been run on
   a game; its licence and accuracy need reading before it could ship.
6. **PlayStation speed.** 48 to 52 of 59.9. The next real step is rendering on a
   second thread so the emulator and the picture pass stop sharing a core. The
   largest piece of work left, and not promised.
7. **Game Boy Color.** Probably closed by item 4. A Gambatte build for this console
   exists and has not been measured, as a fallback.
8. **Check on the console what has not been checked:**
   - in-game battery saves across a restart, and PicoDrive save RAM
   - a PlayStation memory-card write read back after a power cycle
   - a PlayStation disc change on a multi-disc game, and a second PlayStation game
   - quick-menu save states
   - sound by ear on each core (Mega Drive especially, after the core change) and
     headphones
9. **Fix three failing `miyoomini` app tests**:
   `a_game_boy_color_game_keeps_its_own_colour_and_shader`,
   `the_open_cart_rests_over_the_shelf_with_its_lid_turned` and
   `the_shader_shortcut_steps_the_seated_carts_shader_both_ways_and_names_it`.
10. **Find what the stock launcher leaves unset**, as `joy_type` is on the Flip, so
   `launch.sh` sets it for itself and restores it on exit.
11. **Cut the next pre-release** with PicoDrive and whatever speed work lands, re-run
   the nine-system sweep and the fresh-install check against that zip, and update the
   table in COMPATIBILITY.md from it.

Known consequences of PicoDrive to mention in the notes: a Mega Drive resume state
written by Genesis Plus GX is refused by PicoDrive and set aside, and the installer
never deletes, so a card that had the earlier build keeps an unused
`genesis_plus_gx_libretro.so` in its pak.

## 4. Constraints found on this console (do not regress)

- **Overcommit must be on** (`vm.overcommit_memory=1`), set by the pak's
  `launch.sh`. Cores carry large zero-fill areas; with the default heuristic the
  loader refused them and systems never started.
- **Memory is the constraint, not CPU alone.** A texture set that costs 30 MB on
  a GPU device is 30 MB of RAM here. Textures are kept packed and the shelf is
  shed while a game plays.
- **Rewind is 4 MiB on this feature** (20 MiB elsewhere). At 20 MiB a Super
  Nintendo game grew until the kernel killed the frontend at 74 MB.
- **Cores are built with Zig for glibc 2.17** (`cores/armv7-zigcc`), and anything
  assembled from old-style ARM assembly needs GNU as (`cores/picodrive/cc`).
- **The clock picker starts in this decade** when the clock has lost its year.
- **`/dev/fb0` can be read for a picture**, so visual checks can be made from adb.
- **adb over Wi-Fi** works from the card itself (`App/ADB`) and is how the sweep is
  driven: set `cart`/`cart_platform` in `Config/slot.state`, kill slot, let the
  card's boot loop restart it, read `slot.log`. The link can drop for minutes, so a
  sweep that reports nothing ran may mean no connection, not a failure.

## 5. Definition of done

- Slot can be quit to the console's own menu from inside, and the way back works.
- The menu and About screen describe this console and offer only what it has.
- Every system in the table above holds its core's rate in sustained play with
  default settings (rewind on), with sound and no audio underruns, checked on the
  device, and PlayStation says what it does on a five-minute run.
- PS1 end to end: memory card across a reboot and a disc change, both checked on
  the device.
- `Slot-miyoomini.zip` installs on a fresh FAT32 card with no MinUI, boots into
  Slot, and `install.sh --check`/`--dry-run` behave.
- README, INSTALL, COMPATIBILITY and ROADMAP agree with what was measured.
- The release zip itself was installed and run on the console.
