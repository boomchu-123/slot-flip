# Getting 1.0 on the Miyoo Mini Plus

A working plan for the second 1.0 device. It records what has been measured, what
the 1.0 gate still asks for, the order the remaining work should be done in, and
what "done" means. Numbers here are from a real console unless they say otherwise.

**Device:** Miyoo Mini Plus (`miyoomini`), kernel 4.9.84, glibc 2.28, no GPU —
the CPU compositor in `crates/slot-gfx/src/soft/` is the renderer — 640x480
panel, about **103 MB** of RAM usable by Linux. Stock firmware; slot boots from
the card with no MinUI.

**Branch:** `feat/miyoomini-software`. Everything below is or will be measured on
hardware driven over adb, not inferred from a build.

---

## 1. Where it stands

Checked on 2026-10-30 with the work in `miyoomini: keep the frontend's pictures
packed...` and `miyoomini: fit the memory the console has...`.

| System | Runs | Speed (core's rate) | Resident while playing |
| --- | --- | --- | --- |
| NES | yes | 50.0 fps (a 50 Hz game — correct) | 36.4 MB |
| SNES | yes | **54.2 fps** of 59.9 | 51.4 MB |
| Mega Drive | yes | **50.2 fps** of 59.9 | 45.4 MB |
| Master System | yes | 58.9 fps of 59.9 | 44.4 MB |
| Game Gear | yes | 59.6 fps of 59.9 | 44.8 MB |
| Game Boy | yes | 58.3 fps of 59.7 | 39.9 MB |
| Game Boy Color | yes | 55.4 fps of 59.7 | 42.7 MB |
| Game Boy Advance | yes | 58.3 fps of 59.7 | 56.3 MB |
| PlayStation | **barely** | ~25 fps, then thrashed, then killed | 73.3 MB, 2.6 MB spare |

Shelf, with no game: **36.6 MB** resident and 8.6 MB of texture pixels held.

Working today on this console: stock-launcher entry (`App/Slot`), booting
straight into slot from the card (`miyoo354/app/MainUI`, escape hatch
`Slot/Config/boot-menu`), input on the gpio node, OSS audio through the
console's own audio server, the nine systems loading, cartridge labels,
`Config/` and `slot.state` persistence, adb over Wi-Fi for testing.

Not yet checked on this device: a save surviving a reboot, save states, a disc
change, sound judged by ear, and a title-by-title catalogue.

### What each frame costs

The DEBUG trace reports the two halves of the picture path per frame:

- `game 7.2–7.8 ms` — scaling the core's frame into the 640x480 canvas
- `out 7.2–8.1 ms` — copying that canvas to the panel, row by row

That is ~15 ms of the 16.8 ms frame spent on the picture, on the same core the
emulator is using. It is why Mega Drive and Super Nintendo sit below 60.

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

## 2. Work

### A. Draw the game straight to the panel

**Why.** The canvas round trip costs ~7 ms a frame for nothing when the display
list is only the game (which is the case while playing). `Game::draw_direct`
already exists and does exactly this, but it only triggers when the picture and
the canvas are the same size — never true here, because the picture is fitted to
the panel with a margin.

**What.** Let the direct path place the picture on the panel itself: compute the
panel rect with the same `screen_rect`/power/base rules the canvas path uses,
paint the margin black once (the existing `clear_left` machinery already tracks
when that is needed), and scale the core's frame into it. Keep the canvas path
for everything with a menu, bezel, shader or screenshot drawn over the picture.

**Expected.** About 7 ms off each playing frame. Mega Drive and Super Nintendo
should reach their core rate; GBA should gain headroom; PS1 should gain the same
7 ms, which is the difference between thrashing and playing.

**Risk.** The picture's position, aspect and the upside-down panel must not
change. Verified by comparing framebuffer captures before and after for a GBA,
a PlayStation and a NES game, and by playing one of each on the device.

**Size.** Half a day including the comparison.

### B. PlayStation: memory first, speed second

PS1 now loads and reaches its first frame; it is the only system the kernel has
killed. It needs two separate things.

**B1. Memory.** ~20 MB of headroom is missing. In order of cost:

1. **Measure what the core holds.** `/proc/<pid>/smaps` during a PS1 game,
   grouped by mapping, to see whether the 73 MB is the core's own data, its
   dynarec cache, or slot's heap. One hour, and it decides the rest.
2. **Rebuild PCSX-ReARMed with a smaller dynarec cache** if that is where the
   memory is. Core change, so the pak's `.so`, its `.meta` and its licence/
   source bundle all move with it (see AGENTS.md section 9).
3. **Trim slot further** if it is slot's heap: the rewind budget (already 4 MiB
   here), the label cache, and any buffer sized for the Flip.

**B2. Speed.** A PS1 frame must fit a 16.8 ms budget together with the render.
With A done this may be enough. If not, the next real step is to render on a
second thread so the emulator and the picture pass stop sharing one core. That
is a change to pacing and needs care: one frame in flight, the existing torn-frame
guards, and the `direct_game` path already writing to the panel.

**Expectation, stated honestly.** PS1 at full speed on this console is not
promised by this plan. Steps A and B1 are expected to make PS1 playable; B2 is
what full speed needs, and it is the largest single piece of work left.

### C. Package the install

The mechanism works; the packaging does not exist yet.

- `Slot-miyoomini.zip` at the card root: `App/Slot`, `Tools/miyoomini/Slot.pak`,
  `miyoo354/app/MainUI` (the autoboot entry) and `install.sh`.
- `install.sh` for this device: copies those three, `--check` verifies a card
  already has them, `--dry-run` says what it would do, and it must not touch
  `Slot/` user data or anything outside the three paths. `tools/test-install-slot.sh`
  gains a Mini Plus case for the same guarantees the Flip's installer has.
- The escape hatch and the "pak missing → stock menu" behaviour stay as they are,
  and `install.sh` must not overwrite a `MainUI.stock` that is already there.
- One release job builds `Slot-miyoomini.zip` the same way the Flip's is built,
  and `tools/check-pak-licences.sh` runs on it.

**Size.** Half a day, mostly the installer test cases.

### D. Documentation

- README device table: Mini Plus moves from "MinUI Tool, pre-release" to the
  stock-firmware install with no MinUI.
- INSTALL.md: the Mini Plus section, matching the zip's contents, with the
  autoboot and the `Slot/Config/boot-menu` escape.
- COMPATIBILITY.md: replace the Mini Plus section with the table in section 1,
  say what was measured (frame counts, memory, kernel kills) and what was not.
- ROADMAP.md: tick the Mini Plus lines that are true and no more.

### E. Verify on the device, as a stranger would

Run against the **release zip**, on a card laid out the way the installer leaves
it:

1. Unzip onto a fresh FAT32 card; boot; slot comes up with no MinUI and no menu
   press.
2. Create `Slot/Config/boot-menu`; reboot; the stock menu comes up; delete it;
   reboot; slot comes back.
3. One game per system for at least a minute, at the core's rate: frame counts
   from the heartbeat, and the picture checked.
4. PlayStation end to end: a heavy 3D game for five minutes, the memory card read
   back after a power cycle, and a disc change in the in-game menu on a
   multi-disc game.
5. Battery saves: save in a game, power off with slot's own Power Off, reboot,
   confirm the save is there.
6. `install.sh --check` on the populated card and `--dry-run` on an empty one.

---

## 3. Order

1. **A** (direct to panel) — buys speed everywhere and shrinks PS1's problem.
2. **B1** (PS1 memory measurement, then the core rebuild if that is the cause).
3. **E** once A and B1 are in, so the sweep is run against something close to
   what ships.
4. **C** and **D**, then **E** again against the release zip.
5. **B2** only if PS1 is still short of the core rate after the above.

A and B1 are the ones that decide whether 1.0 on this device is a week of work
or two. B2 is the only item that could take considerably longer.

## 4. Constraints found on this console (do not regress)

- **Overcommit must be on** (`vm.overcommit_memory=1`), set by the pak's
  `launch.sh`. The Genesis core carries 35 MB of zero-fill and the PlayStation
  core 23 MB; with the default heuristic the loader refused them and three
  systems never started.
- **Memory is the constraint, not CPU alone.** A texture set that costs 30 MB on
  a GPU device is 30 MB of RAM here. Textures are kept packed and the shelf is
  shed while a game plays; that is what took the shelf from ~60 MB to ~37 MB.
- **Rewind is 4 MiB on this feature** (20 MiB elsewhere). At 20 MiB a Super
  Nintendo game grew until the kernel killed the frontend at 74 MB.
- **The clock picker starts in this decade** when the clock has lost its year, or
  a new owner scrolls the year up ten times on first boot.
- **`/dev/fb0` can be read for a picture**, unlike the Flip's KMSDRM path, so
  visual checks can be made from adb by capturing the framebuffer.
- **adb over Wi-Fi** works from the card itself (`App/ADB`), and is how the
  sweep is driven: set `cart`/`cart_platform` in `Config/slot.state`, kill slot,
  let the card's boot loop restart it, read `slot.log`.

## 5. Definition of done

- The table in section 1 has every system at its core's rate with sound, checked
  on the device, and PlayStation says what it does on a five-minute run.
- PS1 end to end: memory card across a reboot and a disc change, both checked on
  the device.
- `Slot-miyoomini.zip` installs on a fresh FAT32 card with no MinUI, boots into
  slot, and `install.sh --check`/`--dry-run` behave.
- README, INSTALL, COMPATIBILITY and ROADMAP agree with what was measured.
- The release zip itself was installed and run on the console.
