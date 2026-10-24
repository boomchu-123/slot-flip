# slot. — Miyoo Flip fork

A cartridge-style Game Boy / Game Boy Color / Game Boy Advance frontend for the
**Miyoo Flip (my355)**. Browse your games as cartridges on a shelf, slot one in,
and play — with sound, saves, and save states.

It runs on the **Flip's own firmware** as an app, and on **MinUI** as a Tool. The
stock-firmware install ships with the next release; today's download installs into
MinUI. Either way it keeps everything it owns in a single `Slot` folder and
**never touches your Roms or Saves**.


> **This is a fork of [slot](https://slot-cfw.fyi) by Brandon T. Kowalski**,
> which was originally built for the Anbernic RG SP. This fork ports it to the
> Miyoo Flip's hardware and adds a handful of features (below). All credit for
> the original design and frontend goes upstream — see
> [Credits](#credits--licence). It is licensed **GPL-3.0-or-later**, the same as
> upstream.

It keeps everything it owns in a single `Slot` folder and **never touches your
MinUI Roms or Saves**.
ROMs must be unzipped. slot does not read `.zip` or `.7z` files.

Upstream's user guide is at [slot-cfw.fyi](https://slot-cfw.fyi). Release notes
for this fork are in the [changelog](CHANGELOG.md).

## Device support

| Device | MinUI platform | Pak folder | Status | Download |
| --- | --- | --- | --- | --- |
| **Miyoo Flip** | `my355` | `Tools/my355/Slot.pak` (MinUI) or `App/Slot` (stock firmware) | ✅ **Supported** (stable) | [Latest release](https://github.com/boomchu-123/slot-flip/releases/latest) |
| **Miyoo A30** | `my282` | `Tools/my282/Slot.pak` | 🧪 **Experimental** — has a GPU, may work; unverified | [Pre-releases](https://github.com/boomchu-123/slot-flip/releases) |
| **Miyoo Mini Plus** | `miyoomini` | `Tools/miyoomini/Slot.pak` | 🧪 **Experimental — not working yet** (no GPU) | [Pre-releases](https://github.com/boomchu-123/slot-flip/releases) |
| Any other MinUI device | — | — | ❌ Not supported | — |

The Flip's stock firmware is supported too: Slot was booted on it and run from the
launcher's **App** list, with input, sound, lid sleep, brightness and re-entering
games working. That install path ships in the next release (see the
[roadmap](ROADMAP.md)); until then, use the MinUI path below.

**Why not other devices?** Each MinUI device loads tools only from its own
`Tools/<platform>/` folder and has different hardware (CPU architecture,
display, inputs). The Flip pak will simply **not appear** on a device whose
platform folder doesn't match, and even if copied across it won't run.

**The two experimental builds need testers** (I only own a Flip):

- **Miyoo A30 (`my282`)** has a Mali-400 GPU, so it uses slot's real graphics
  renderer and has a genuine chance of working — but it's **unverified** and the
  button map is a best guess.
- **Miyoo Mini Plus (`miyoomini`)** has **no GPU**, so its graphics path is still
  being written; that build only probes the hardware and **does not draw yet**.

If you have either device and want to help, grab a pre-release from the
[Releases](https://github.com/boomchu-123/slot-flip/releases) page and see the
[`experimental` branch](https://github.com/boomchu-123/slot-flip/tree/experimental)
and its `TESTERS.md` for exactly what to do.

## Watch the launch video

<div align="center">
  <a href="https://www.youtube.com/watch?v=-XE-_v9wOAk">
    <img src="https://i.ytimg.com/vi/-XE-_v9wOAk/maxresdefault.jpg"
         alt="Introducing: Slot Flip — launch video" width="720">
  </a>
</div>

---

## Features

### From slot (upstream)

These come from the original [slot](https://slot-cfw.fyi) and are the heart of
the experience — all credit to the upstream author:

- **Game Boy, Game Boy Color, and Game Boy Advance** in one cartridge-style
  carousel, one shelf per system.
- **Plays with sound, battery saves, and save states**, all stored under `Slot`.
- **Cartridge artwork** — a per-game label image on each cart, dropped in one
  folder and named after the game.
- **Wallpapers** — drop PNGs in `Wallpapers/` and it shuffles one in on boot.
- **Per-game core override** — pick mGBA or gpSP for a specific GBA title.
- **Quick menu** — fast-forward, colour correction, rumble, date & time, and the
  whole look and feel.

### New in this fork

Added on top of upstream for the Miyoo Flip (and portable to other MinUI
devices):

- **Miyoo Flip port** — SDL2 + GLES display, the Flip's button map, lid sensor,
  and power LED, packaged as a MinUI Tool.
- **Wallpaper in the menu** — pick one under **General > Wallpaper** (your own
  PNGs from `Slot/Wallpapers`, or Random, Default, Blur).
- **Controls and Key Map** — upstream's **Controls** pages set what each button
  does (in-game buttons, shortcuts, shelf keys). The fork adds a **Key Map**
  page under them for the physical keys themselves, so a device whose keys
  report differently can be fixed without a rebuild.
- **About shows both versions** — this build's version and the upstream slot
  release it follows, on a plate marked **MY355** with this fork's repo on it,
  plus a one-time "Updated to ..." word the first time a newer build runs on a
  card that has run an older one.
- **PlayStation discs** — a PS1 shelf where each game is a disc that spins into
  a slot drawn as a console. See [PlayStation discs](#playstation-discs).
- **Sega games** — Mega Drive / Genesis, Master System and Game Gear shelves, each
  game a drawn black cartridge. See [Sega games](#sega-games).
- **A slot for each console** — the slot a game slides into is drawn as the console
  it is for: a PlayStation, a NES, a Super Nintendo, a Mega Drive, a Master System,
  a Game Gear, a Game Boy, a Game Boy Color. GBA keeps upstream's slot. The slot changes as you move between shelves, in both
  the dark and light themes.
- **Lid-close sleep + auto power-off** — shut the lid and it sleeps instantly,
  then powers off after the time you choose under **General > Lid Power Off**
  (1–30 min, or **Never**).
- **Right power LED behaviour** — green while running, red at low battery, both
  while charging.
- **Portable config** — the button map and settings are data-driven, so the port
  can be adapted to other MinUI devices (see [INSTALL.md](INSTALL.md) /
  [PORTING.md](PORTING.md)).

## Compatibility

- **Device:** Miyoo Flip (`my355`), running **MinUI**.
- **Systems:** GBA (`.gba`), GB (`.gb`), GBC (`.gbc`), PlayStation
  (`.cue`, `.chd`, `.m3u`, `.pbp`; see [PlayStation discs](#playstation-discs)),
  NES (`.nes`, `.fds`, `.unf`; see [NES games](#nes-games)), Super Nintendo
  (`.sfc`, `.smc`; see [Super Nintendo games](#super-nintendo-games)), and the Sega
  Mega Drive / Genesis (`.md`, `.gen`, `.smd`, `.bin`), Master System (`.sms`) and
  Game Gear (`.gg`) (see [Sega games](#sega-games)).
- **Which games work:** see **[COMPATIBILITY.md](COMPATIBILITY.md)**. It lists what
  has been run on a Flip, what only boots, and what nobody has checked yet.
- Other MinUI devices can be supported with small changes — see
  [INSTALL.md](INSTALL.md).

---

## Install

The Flip's own launcher and MinUI are both supported, from one download. You need
the device, its SD card, a computer with a card reader, and about two minutes.

1. **Download** the latest `Slot-my355.zip` from this repo's
   [Releases](https://github.com/boomchu-123/slot-flip/releases/latest).
2. **Power off** the device and move the SD card to your computer.
3. **Unzip it at the root of the card** — the folder that already holds `Roms/` and
   `Saves/`, the one the device sees as `/mnt/sdcard`. You should end up with
   `App/Slot` and `Tools/my355/Slot.pak` on the card, not a folder inside a folder.
4. Put the card back and boot the device. Slot is in the **App** list of the
   Flip's own launcher, and under **Tools → Slot** in MinUI. Both entries run the
   same program; use whichever launcher the device starts in.
5. On first launch Slot creates a `Slot` folder at the root of the card and writes
   starter settings.
6. Power off, put the card back in your computer, and drop your **legally-dumped**
   ROMs into the matching folder:

   ```
   <SD CARD>/Slot/Games/GBA    (.gba)
   <SD CARD>/Slot/Games/GB     (.gb)
   <SD CARD>/Slot/Games/GBC    (.gbc)
   ```

   PlayStation, NES, Super Nintendo and the Sega systems get their own folder
   names; the [systems list](#compatibility) above has them.

7. Boot the device and open Slot — your carts appear on the shelf.

Nothing else on the card changes, and your launcher, its Roms and its Saves are
untouched. `Slot.pak-my355.zip` is the pak on its own, for a MinUI card where you
copy it to `Tools/my355/` by hand.

> **Don't have MinUI, and don't want it?** You don't need it. The Flip's own
> firmware runs Slot from step 1. MinUI is optional, and Slot does not change
> anything on the device either way.

### Boot straight into Slot (optional)

On the Flip's own firmware, Slot is one App in the launcher's list; there is no
supported way to boot straight into it yet, and the launcher's own settings are the
place to look. Under MinUI, skip its menu and land in Slot on boot by creating
`<SD CARD>/.userdata/my355/auto.sh` with:

```sh
#!/bin/sh
/mnt/SDCARD/Tools/my355/Slot.pak/launch.sh
```

MinUI runs this before its menu, so the device boots into Slot; quitting Slot
drops you back to MinUI. Delete `auto.sh` to undo it.

---

## The Slot folder

Everything Slot owns lives under `<SD CARD>/Slot`:

```
Slot/
  Games/   GBA GB GBC PS1 NES SNES MD SMS GG   your ROMs and discs, one folder per system
  Saves/   GBA GB GBC           battery saves
  States/  GBA GB GBC PS1 NES SNES MD SMS GG   save states
  Labels/  (GBA GB GBC PS1 NES SNES MD SMS GG) cart and disc artwork, <game-name>.png, any of these folders
  Wallpapers/              background images (PNG)
  Config/                  slot.ini, buttons.ini, and other settings
  System/                  emulator cores
  BIOS/                    optional GBA BIOS (gba_bios.bin)
```

### Cartridge artwork

Slot draws a label on every cart. Put a PNG anywhere in `Slot/Labels` and it
pairs with the game of that name:

```
Slot/Games/GBA/Pokemon Emerald.gba
Slot/Labels/Pokemon Emerald.png         one folder is enough
Slot/Labels/GBA/Pokemon Emerald.png     or sorted by system, as you like
```

- **Name it after the game.** Case is ignored, and so are region and revision
  tags in brackets: `Metroid Fusion (USA).gba` pairs with
  `Metroid Fusion.png`, and `Zelda (USA, Europe) (Rev 2).png` pairs with
  `Zelda.gbc`. Punctuation is ignored too, so a curly apostrophe from the web
  matches a straight one.
- **PNG only.** JPG and WebP images are not read; convert them first.
- **Any size or shape.** On a GBA, Game Boy or PlayStation cart, Slot scales and
  centre-crops the picture to the label, so a cartridge-label shot fills it best and
  box art loses its edges. On the NES, Super Nintendo and Sega carts a label scan
  keeps its own shape instead: see below.
- **The system's own folder wins**, so `Labels/GBA/<game>.png` overrides the
  same name in the flat `Labels/`.
- **New labels appear the next time Slot starts**, not while it is running.

If a label does not show up, it is nearly always one of three things: the file
is not a PNG, it is in a folder with another name (`Labels/gba` is fine,
`Labels/Game Boy Advance` is not), or the file's name is not the game's name —
a ROM renamed to `Emerald` will not pick up `Pokemon Emerald.png`.

---

## PlayStation discs

Put PlayStation games in `Slot/Games/PS1`. Slot reads `.cue` (with its `.bin`
files), `.chd`, `.m3u` and `.pbp`. They show on their own **PS1** shelf as discs,
and a disc spins into a slot drawn as a console when you start it. A multi-disc
game is shown once if you give it an `.m3u` playlist. Emulation is by
PCSX-ReARMed (GPL-2.0).

**Disc art.** Drop the front cover as `Slot/Labels/PS1/<game name>.png` (or flat
in `Labels/`, named after the game). It is printed across the disc. If it is a
PlayStation jewel-case cover, the black "PlayStation" spine down its left edge is
trimmed off first. A disc with no art gets a colour and its title.

**Buttons.** The face buttons sit where a PlayStation pad's do:

| PlayStation | On the Flip |
| --- | --- |
| ✕ Cross | bottom face button (B) |
| ○ Circle | right face button (A) |
| △ Triangle | top face button (X) |
| □ Square | left face button (Y) |
| L1 | left shoulder |
| L2 | left trigger |
| R1 | right trigger (the Flip port reads it as R1, see Notes for the Flip) |
| R2 | right analog stick pushed right |
| D-pad, Start, Select | the same |

R2 is a click of the right stick because the Flip port already gives the physical
right shoulder to R1. L2 and R2 are game buttons on a disc, so **fast-forward there
is SELECT + R2** (hold SELECT, push the right stick right) instead of the stick
alone, and there is no rewind. Turbo does not apply to discs. The Controls pages
still call the two main face buttons "A Button" and "B Button".

**Saving.** The game's own save (its memory card) is kept per game in
`Slot/Saves/PS1/<game>.sav`, one 128 KiB card, written with the other saves while
you play and when you eject or put the Flip to sleep. Card 2 is off.

**Changing discs.** A game with two or more discs (an `.m3u` playlist) gets a **Disc** row in the in-game menu, between States and
Settings. Left and Right pick the disc; close the menu and the game sees the lid
open, the disc change and the lid close, so a game that says "insert disc 2" carries
on by itself. The row only appears for a game with more than one disc.

**Analog sticks.** Off by default. Under **Settings > Gameplay > Analog Sticks**
the Flip plugs in a DualShock, and both sticks go to the game. The sticks have a
dead zone, because the Flip's rest a little off centre. With them on, pushing a
stick in is L3 or R3. The core starts a DualShock in digital mode; a game that uses
the sticks switches it to analog itself, and the core's own combination
L1 + R1 + SELECT switches it by hand. **While they are on, the right stick stops
being R2**, so a Flip whose right shoulder is dead then has no R2 and no
SELECT + R2 fast-forward (rebind fast-forward under Shortcuts if that matters).

**What is not there yet.** A screen shader, colour correction or bezel (they do
nothing for a disc, so those rows are hidden), and a second player. Games run
without a BIOS file on the card. How fast heavy 3D games run has not been measured;
the one game timed ran at full speed.

## NES games

Put NES games in `Slot/Games/NES` (`.nes`, `.fds`, `.unf`, `.unif`). They get their
own **NES** shelf, and each game is a grey Game Pak with a label. Emulation is by
fceumm (GPL-2.0).

**Label art.** Drop a picture named after the game in `Slot/Labels/NES/` (or flat in
`Labels/`). A game with no art gets a colour and its title. A scan of the real label,
such as ScreenScraper's `supporttexture` pictures, keeps its own proportions: the
label on the cart takes the scan's shape, with none of it cropped, and transparent
corners show the plastic. This holds on the Super Nintendo and Sega carts too, for
any picture between square and 2.4 times wider than tall. Anything else, such as a
tall box front or a banner, is centre-cropped to fill the label as before.

**Buttons.** A, B, Select, Start and the D-pad, exactly as on a Game Boy. Turbo A
and Turbo B (X and Y) work the same way too. The picture fills the 4:3 screen as a
NES did on a TV. A screen shader, colour correction and bezel do not apply to it, so
those menu rows are hidden for NES.

**What is not there yet.** Zapper and other special controllers, disk-side swapping
for Famicom Disk System games, and cheats. The overscan edges of the picture are
shown as the core draws them.

## Super Nintendo games

Put games (`.sfc`, `.smc`, `.swc`, `.fig`) in `Slot/Games/SNES`. A **SNES** shelf
appears, and each game is a grey cartridge with a label; put art in
`Slot/Labels/SNES/` as for the other systems. Emulation is by snes9x2005, which is
licensed for non-commercial use only: see [Credits & licence](#credits--licence).

**Buttons.** B, A, Y and X are the bottom, right, left and top buttons, L and R the
shoulders, as on a Super Nintendo pad. Because X and Y are game buttons here, Turbo
is off for SNES. Fast-forward and rewind stay on the lower triggers. The picture
fills the 4:3 screen; the shader, colour correction and bezel rows do not apply.

**What is not there yet.** The Super Scope and mouse, enhancement chips that the
core does not support, and cheats.

**Saves.** A game's battery save is kept in `Slot/Saves/SNES/<game>.sav`, and save
states in `Slot/States/SNES`. They are separate from MinUI's own `.srm` saves, so a
save made in MinUI does not carry over.

## Sega games

Put games in `Slot/Games/MD` (`.md`, `.gen`, `.smd`, `.bin`), `Slot/Games/SMS` (`.sms`)
and `Slot/Games/GG` (`.gg`). A shelf appears for each system that has games, and each
game is a black cartridge with a label; put art in `Slot/Labels/MD/` (or `SMS`, `GG`)
as for the other systems. All three run on one core, Genesis Plus GX, which is
licensed for non-commercial use only: see [Credits & licence](#credits--licence). The
`.bin` extension is only taken from the `MD` folder, so it never mixes with a
PlayStation disc's tracks.

**Buttons (Mega Drive).** The Mega Drive's A, B and C are the left, bottom and right
face buttons, which the Flip prints as **Y, B and A**. X, Y and Z of a six-button pad
are the left shoulder, the top button and the right shoulder, and Mode is SELECT. A
game gets a six-button pad when its header asks for one and a three-button pad
otherwise, as on the console, so there is nothing to switch. Because the face buttons
and shoulders are game buttons here, Turbo is off for the Mega Drive; fast-forward
and rewind stay on the lower triggers.

**Buttons (Master System and Game Gear).** Button 1 is the bottom button and Button 2
the right one. On a Master System START is the Pause button, and on a Game Gear it is
Start. Turbo works as on a NES.

**The picture** is shown at the size the core draws it and fills the 4:3 screen; the
shader, colour correction and bezel rows do not apply.

**What is not there.** Sega CD, 32X and the SG-1000 are not offered; nor are the
light gun, mouse, multitap or cheats. **Saves.** A game's battery save is kept in
`Slot/Saves/<system>/<game>.sav`, and save states in `Slot/States/<system>`. They are
separate from MinUI's own saves.

**What has and has not been tried.** Two games for each system have run on a Flip at
full speed and been looked at by the owner, and test programs that are nobody's game
run on the real core on a computer prove that each button reaches it as described
above. Sound over a long session, battery saves, and the many games that use
special hardware have not been checked.

## Settings

### Background

1. Put one or more PNGs in `Slot/Wallpapers/`.
2. Edit `Slot/Config/slot.ini`:

   ```ini
   background = mybackground.png
   ```

   Use `random` (the default) to shuffle through all of them on each boot. If a
   named file is missing, Slot falls back to a random one.

### Sleep & auto power-off

- **Close the lid → sleeps immediately.**
- While asleep, the device powers itself off after the time you pick in the
  menu under **General > Lid Power Off**: `1 / 3 / 5 / 10 / 15 / 30` minutes, or
  **Never**, which stays asleep until you open the lid (or the battery runs out).
  The choice is saved across reboots. Never is this fork's addition; upstream has
  no such choice.

### Button remapping

Slot ships with the correct map for the Flip, but you can rebind any button
without a rebuild. No file editing needed:

1. Open the quick menu and choose **Controls > Key Map**. It has a page per
   group of controls: **D-pad**, **Face Buttons**, **Shoulders**, and
   **Start & Select**, each listing its buttons with the key they are on.
   (The other **Controls** pages set what a button *does*; the Key Map says
   which physical key *is* that button.)
2. Move to the button you want to change and press **A**. Its key turns into
   **Press a key…**.
3. Press the physical key you want for it. It takes effect straight away and is
   saved.

- If that key already did something else, the two **swap**, so no button is ever
  left without a key. (Pressing the key a button is already on changes nothing.)
- Press no key for a few seconds and the change is cancelled.
- The power, volume and lid keys are never captured, so they can't be bound
  away.
- **Reset Key Map** at the bottom of the Key Map page restores the default map.
- If you rebind the button you use to confirm or go back, the screen follows the
  new key straight away. If you get stuck, delete `Slot/Config/buttons.ini` and
  restart Slot.

These pages save to `Slot/Config/buttons.ini`, and you can still edit that file
by hand. Each line is a kernel key code mapped to a button:

  ```ini
  # <kernel-key-code> = <button>
  109 = r1
  104 = l2
  57 = none      # switch a key off
  ```

  Codes may be decimal or hex (`0x135`). Button names:
  `up down left right  a b x y  l1 r1 l2 r2  start select menu  volup voldown`,
  or `none` to switch a key off. Anything you don't list keeps its built-in
  mapping. Slot rewrites the file when you change a binding in the screen, so
  hand-written comments are not kept.

To discover your codes, drop an empty file named `DEBUG` next to `launch.sh`,
launch once, and read `Slot/slot.log` — each press is logged with its code.

---

## Notes for the Flip

- **Display:** SDL2 + GLES2 (KMSDRM), which MinUI provides on the Flip.
- **Buttons:** the face buttons are mapped to the printed letters
  (A = right, B = bottom, X = top, Y = left). The right shoulder stands in for a
  dead R1 on some units; fast-forward is on the right analog stick.
- **Power LED:** Slot drives both of the Flip's lights — the green "work" light
  and the red "charger" light:
  - **Running / fully charged:** green
  - **Low battery (≤20%):** red
  - **Charging:** both green and red lit
- **GBA core:** gpSP by default (it has an ARM64 dynarec; mGBA does not). Per-game
  overrides live in `Slot/Config/selected_core.txt`.

## Installing on other MinUI devices

The only **supported** device is the Miyoo Flip. The Miyoo A30 and Mini Plus have
**experimental** builds — see [Device support](#device-support) above, and
[TESTERS.md](https://github.com/boomchu-123/slot-flip/blob/experimental/TESTERS.md)
for how to test them.

If you want to **port** slot to new hardware, see [INSTALL.md](INSTALL.md) for
the install model and [PORTING.md](PORTING.md) for the device seams (button
codes, LED/backlight paths, display/audio wiring, CPU architecture).

---

## Support & troubleshooting

First step for almost anything: drop an empty file named **`DEBUG`** next to
`launch.sh` in `Slot.pak`, launch once, reproduce the problem, then read
**`Slot/slot.log`**. It records the display/audio/input setup and every button
press, and most issues are obvious from it.

**Slot doesn't appear in MinUI's Tools menu.**
Check the folder is exactly `<SD CARD>/Tools/my355/Slot.pak` and contains
`launch.sh`, `slot`, and `System/`. The card must be readable (FAT32/exFAT as
MinUI expects).

**A game shows a black or white screen (but others work).**
That's almost always a **bad ROM dump**, not Slot — the same file will also fail
in other emulators. Re-dump the cartridge or replace it with a known-good copy
(verify against the No-Intro set). Some ROM *hacks* are also just broken.

**A GBA game runs slow, or crashes on load.**
gpSP is the default (it's fast on this chip). If a specific title misbehaves,
force mGBA for just that game by adding a line to
`Slot/Config/selected_core.txt`:

```ini
Pokemon Emerald = mgba
```

(use the ROM's name without the extension). mGBA is more accurate but slower.

**Saves aren't being kept / "Read-only file system" in the log.**
The SD card's filesystem has gone read-only, usually after a hard power-off or a
yank mid-write, which can corrupt FAT. Put the card in a computer and run a
filesystem check/repair (`fsck`/"Repair" on Windows/macOS). Always power the
device down cleanly. (Upstream slot can repair a read-only card itself with
SELECT + START; that is turned off on the Flip, where MinUI owns the card, so
use a computer.)

**The buttons are wrong.**
Open the quick menu → **Controls → Key Map** → the group, pick a button, press
**A**, then press the key you want for it. See [Button remapping](#button-remapping).

**Closing the lid doesn't sleep.**
Confirm you're on the latest build. If a future device reports its lid
differently, that's a porting detail — see [PORTING.md](PORTING.md).

**It won't boot / I want to undo a change.**
Everything Slot owns is under the `Slot` folder and the `Slot.pak` — deleting
either is harmless to MinUI. If you auto-boot Slot, delete
`<SD CARD>/.userdata/my355/auto.sh` to return to the normal MinUI menu.

**Getting help.** This is a personal fork; for issues specific to it,
[open a bug report](https://github.com/boomchu-123/slot-flip/issues/new/choose)
and include your `slot.log` (captured with the `DEBUG` file). For questions
about the original frontend's look and behaviour, upstream's guide at
[slot-cfw.fyi](https://slot-cfw.fyi) is the reference.

---

## Contributing & roadmap

Contributions are welcome — bug reports, feature requests, docs, code, and
especially **testing the experimental device builds**.

- **[CONTRIBUTING.md](CONTRIBUTING.md)** — how to build, test, and submit
  changes, plus the branch model.
- **[ROADMAP.md](ROADMAP.md)** — the 1.0 and 2.0 milestones, and how to request a feature.
- **[SYSTEMS.md](SYSTEMS.md)** — every system considered, the core it would use, its licence, and its status.
- **[Issues](https://github.com/boomchu-123/slot-flip/issues/new/choose)** —
  bug reports, feature requests, and experimental device test reports.
- **Security and privacy:** never paste keys, tokens or personal paths into an
  issue or PR. Contributors run `sh tools/install-hooks.sh` once per clone;
  `.githooks/` and CI refuse secrets and personal data. See
  [CONTRIBUTING.md](CONTRIBUTING.md#secrets-and-personal-data).

---

## Credits & licence

**slot. is by [Brandon T. Kowalski](https://slot-cfw.fyi)** — a bespoke,
Game-Boy-centric frontend for the Anbernic RG SP that evokes using a GBA SP as a
kid. All of the frontend's design, look, and feel are his. This repository is a
**fork** that ports it to the Miyoo Flip and adds the features listed above; it
is **not** affiliated with or endorsed by the original author.

- Upstream project & guide: [slot-cfw.fyi](https://slot-cfw.fyi)
- Upstream release notes: [CHANGELOG.md](CHANGELOG.md)
- Licence: **GPL-3.0-or-later** (see [LICENSE](LICENSE)), same as upstream. As a
  GPL fork, source stays open.

Emulation is by libretro cores: **mGBA** (MPL-2.0), **gpSP**, **PCSX-ReARMed** and
**fceumm** (GPL-2.0), **snes9x2005** and **Genesis Plus GX**. Their licences, and the
source of the ones that ask for it, are in `Slot.pak/System/licenses/`.

**This download is free and has to stay free.** snes9x2005, the Super Nintendo
core inside it, may be used, copied and shared for non-commercial purposes only, and
Genesis Plus GX, which plays the Sega systems, may not be sold or used in a
commercial product or activity. So you may use and share this download, but do not
sell it, charge for it, or bundle it with anything that is sold. Donations to the project are fine, and
never unlock anything. See `System/licenses/NOTICE-NONCOMMERCIAL.txt`. (A plain
reading of the licences, not legal advice.)

Game Boy, NES, Super Nintendo, Sega Genesis, Mega Drive, Master System, Game Gear
and PlayStation are trademarks of their owners; this project is not affiliated with
Nintendo, Sega or Sony. Use your own
legally-dumped ROMs.

**Other people's work on slot, credited.** Slot runs on the Flip's own firmware by
sitting in the launcher's `App` list, which is Miyoo's own mechanism; its field
names came from that firmware. Reading
[**slot-miyoo**](https://github.com/deanmarano/slot-miyoo), a separate community
port of slot to the Miyoo Mini Flip, Mini Plus and Mini, is what pointed at the
launcher's App list and at the fact that the Mini family shares one SoC. That is
credit for the idea, not for code: **no code from it is in this repository**. Its
notes on a CPU renderer, the SigmaStar audio and the battery are the starting point
planned for the Miyoo Mini Plus, and anything taken from it later will be named here
and in the files it lands in, under the GPL both projects share.

The upstream frontend was written with an AI assistant and reviewed by its
author; this Flip port was likewise done with an AI coding assistant.
