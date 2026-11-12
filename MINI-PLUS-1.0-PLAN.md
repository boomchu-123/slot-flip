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

**Last updated:** 2026-11-11.

**Status: GBA, GBC and the new release-critical controls pass; the device as a whole is
not yet releasable as 1.0.** GBA holds full speed through battles and GBC through
sustained action, both with rewind recording enabled. Exit Slot and the device-specific
General page work on the console. SNES still needs a clean trace, PlayStation is short,
and the end-to-end checks below remain open.

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
| Game Boy Color | mGBA | **full speed in sustained play** | 120-122 frames per two seconds, rewind on |
| Game Boy Advance | gpSP | **full speed in sustained play** | Unbound battles held 120-122 frames per two seconds; rewind-on held 120-123 |
| PlayStation | PCSX-ReARMed | **48 to 52** of 59.9 | short; 21 MB spare after four minutes |

The Mega Drive, Master System and Game Gear rows are from the zip with PicoDrive
installed the way a user would; the rest are from the exp-v0.5.0 build, and the
SNES and GBC rows were re-measured on 2026-10-31 for the rewind comparison.

**How far to trust this table.** GBA and GBC were measured during real sustained play
with live frame-time and audio counters and rewind recording on. Every other row is an
average over 40 to 50 seconds, usually at a title screen or intro, with no measure of how
evenly frames arrive. Treat those rows as upper bounds until section 3 item 2 is done for
each system.

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
- **Exit Slot** reaches the stock menu from **General > Exit Slot** on the Mini Plus;
  its one-shot handoff is consumed and the next reboot returns to Slot.
- **Device-fitting General page** shows Auto Sleep and hides Lid Power Off and Rumble
  on the Mini Plus. The About identity is implemented and covered by tests; its visual
  check remains open.
- **GBA sustained play** now passes. The Mini Plus holds its stock 1.2 GHz ceiling,
  separates frontend and emulator work across its two cores, uses gpSP's 32768 Hz audio
  rate, recovers short pacing misses, and records rewind at 10 snapshots a second.
  Unbound battles held full speed with flat audio counters, and actual rewind felt good.
- **GBC sustained play** now passes. Link's Awakening DX held 120-122 frames in all 89
  steady two-second windows (60.06 fps overall), with no dropped audio during normal
  play. The maintainer judged play and rewind good.

---

## 3. Remaining for 1.0, in order

Items 1 to 3 were added on 2026-11-01 and come before everything else, because
nothing below them can be judged, or even debugged away from the bench, without them.

1. **Quit Slot from inside Slot (both devices, every release).** *Built and run on the
   Mini Plus; **not yet run on the Flip**.* **General > Exit Slot** saves and hands over
   to the stock launcher for one session, leaving nothing on the card, so the next boot
   is Slot; Slot stays in the stock App list as the way back. Both autoboot wrappers
   (`miyoo354`, `miyoo355`) carry the change; the Flip's has only been read and tested
   in code. One console power-off during testing, after a hand-made trigger rather than
   the menu row, is unexplained. Done when it is run on the Flip too: Exit, set Wi-Fi in
   the stock menu, come back to Slot from the App list, reboot into Slot.
2. **Measure sustained play, then fix what it shows.** *GBA and GBC are done; every
   other system is open.* The trace now prints, while a game runs, emulator frames per two seconds,
   per-frame time (core and whole, with the worst frame) and the audio dropped and
   starved counts. On Pokemon Unbound from a mid-game resume, with the maintainer
   playing:
   - governor held at 1.2 GHz, frontend and emulator on separate cores, gpSP at
     32768 Hz: overworld about 104 to 118-121 and much better, but a two-battle trace
     averaged 115.0, worst 105, with starvation still growing (core only 8-12 ms)
   - pacing no longer discards a missed frame's time: 42 of 42 battle windows at
     120-122, flat audio; the temporary picture skip was then removed as unnecessary
   - rewind **on**: every second frame averaged 118.4 (worst 108, starvation growing),
     every fourth 120.9 (one underrun), **every sixth, 10 a second: 73 of 73 windows at
     120-123, flat audio**, and the maintainer's battle and rewind felt good
   Still to do: one game per system through its heavy parts with rewind **on**, and a
   32 MB GBA ROM against an 8 MB one to test whether the card is re-read during play
   (about 12 MB spare with the large ROM). A system is done only on that measurement,
   and the maintainer calling a game choppy is a failure whatever the average says.
3. **Settings and About that fit this console.** *General page done and looked at on
   the console; About not yet.* Auto Sleep is shown, and Lid Power Off and Rumble are
   hidden on the shelf menu (`App::hidden_rows`) and the in-game menu (`App::in_game_hidden`). The About
   sticker's model and wordmark come from `build_info` (`MY354`, `MIYOO MINI PLUS`) and
   are covered by tests, not yet looked at. The volume bar now follows the console's own
   step instead of a second volume of slot's (checked on the console). Every other row
   (brightness steps, battery, clock, shaders) is still to be reviewed once against the
   device.
4. **Make rewind cheaper on this console.** *Done for GBA and GBC:* `SNAPSHOT_EVERY` is
   6 on this device (10 snapshots a second, was 30). Not cleanly measured: SNES, and how
   many seconds of history 10 a second keep in the 4 MiB budget. If SNES still falls
   short, the next options are recording only while a rewind button is held, or keeping
   the snapshot off the emulator thread.
5. **Super Nintendo.** Yoshi's Island felt smooth to the maintainer with rewind on, and
   most traced windows were full speed, but fast-forward, rewind and volume testing
   contaminated that trace; several windows and audio counters were short. It needs one
   clean sustained run before it passes. Earlier, it was about 3 frames short with rewind off. Measured once: the
   non-Plus snes9x2005 build reached 53.0 against 51.4 with rewind on (small, one
   run). An ARM-mode build of the current core was built and not yet measured.
   snes9x2002 builds and loads for this console (glibc 2.7) and has not been run on
   a game; its licence and accuracy need reading before it could ship.
6. **PlayStation speed.** 48 to 52 of 59.9. The next real step is rendering on a
   second thread so the emulator and the picture pass stop sharing a core. The
   largest piece of work left, and not promised.
7. **Game Boy Color.** Done on mGBA by item 4; the Gambatte fallback is not needed.
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
    Before cutting it, the test console's leftovers must be cleared: the `DEBUG` file, the
    forced `cart=` resume in `Config/slot.state`, `slot.prev` and other deployment
    backups in the pak, and the maintainer's volume and rewind settings restored (they
    are, as of 2026-11-02). None of those belong in a zip.

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
