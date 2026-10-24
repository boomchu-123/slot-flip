# Installing Slot on a Miyoo Flip

Two ways: unzip a folder onto the card, or run the installer that does it for you.
Both do the same thing, and neither needs MinUI, a firmware download, or a
reformatted card.

What you need: the Flip, its SD card (FAT32), a computer with a card reader, and
about two minutes.

---

## The quick way: drag and drop

1. **Download** `Slot-my355.zip` from
   [Releases](https://github.com/boomchu-123/slot-flip/releases/latest).
2. **Switch the Flip off** and move the SD card to your computer.
3. **Unzip the zip at the root of the card.** The root is the folder that already
   holds `Roms/` — the one the device sees as `/mnt/sdcard`. You should end up
   with `App`, `Tools` and `miyoo355` sitting there:

   ```
   <SD CARD>/App/Slot/
   <SD CARD>/Tools/my355/Slot.pak/
   <SD CARD>/miyoo355/app/MainUI
   ```

   Not a folder inside a folder: if you see `Slot-my355/` on the card with
   `App` inside it, unzip again one level up.
4. **Put the card back and switch the Flip on.** It starts straight into Slot.
5. **Eject a game with MENU** to get back to the shelf.

That's the whole install. Your Roms, Saves and anything else on the card are
untouched, and nothing in the device is modified.

## The script way

If you would rather not move folders around, the zip contains `install.sh`:

```sh
unzip Slot-my355.zip -d slot
cd slot
./install.sh
```

It finds your card, tells you what it is about to do, and asks before doing it. It
also:

```sh
./install.sh --check        # look at the card and report; change nothing
./install.sh --dry-run      # say what it would do; change nothing
./install.sh --stock-menu   # start in the stock menu instead of into Slot
```

It refuses a card that is not FAT32, has no room, or is read-only, and it copies
anything it replaces to `~/slot-flip-backups/` on your computer first. It never
formats, partitions or deletes, never asks for `sudo`, and never uses the network.
Windows has no shell for it yet: use the drag and drop steps.

---

## Where things go

Everything Slot owns lives in one folder, `Slot/`, which it creates on first
launch:

```
Slot/
  Games/   GBA GB GBC PS1 NES SNES MD SMS GG   your games, one folder per system
  Saves/   the same folders                    battery saves and memory cards
  States/  the same folders                    save states
  Labels/  cart and disc artwork, <game-name>.png
  BIOS/    optional: gba_bios.bin
  Config/  settings, the button map, boot-menu
  System/  the emulator cores
  slot.log the last run's log
```

Games must be **unzipped** (`.zip` and `.7z` are not read).

| System | Folder | Files |
| --- | --- | --- |
| Game Boy Advance | `Slot/Games/GBA` | `.gba` |
| Game Boy | `Slot/Games/GB` | `.gb` |
| Game Boy Color | `Slot/Games/GBC` | `.gbc` |
| PlayStation | `Slot/Games/PS1` | `.cue` + `.bin`, `.chd`, `.m3u`, `.pbp` |
| NES | `Slot/Games/NES` | `.nes`, `.fds`, `.unf`, `.unif` |
| Super Nintendo | `Slot/Games/SNES` | `.sfc`, `.smc`, `.swc`, `.fig` |
| Mega Drive / Genesis | `Slot/Games/MD` | `.md`, `.gen`, `.smd`, `.bin` |
| Master System | `Slot/Games/SMS` | `.sms` |
| Game Gear | `Slot/Games/GG` | `.gg` |

**BIOS** is optional. No system needs one to run; a GBA BIOS at
`Slot/BIOS/gba_bios.bin` only adds the real boot animation on Game Boy Advance.

**A multi-disc PlayStation game** goes in one `.m3u` playlist so it shows once on
the shelf; see the README's [PlayStation discs](../README.md#playstation-discs)
section.

## The buttons you need on day one

| Press | Does |
| --- | --- |
| **MENU** (tap) | the in-game menu: states, screen, settings |
| **MENU** (hold) | eject the cart, back to the shelf |
| **SELECT + X** | screenshot |
| **SELECT + D-pad Up/Down** | brightness |
| Volume keys | Slot's volume bar |
| Close the lid | sleep; powers off after the time set under **General > Lid Power Off** |

Everything else can be changed inside Slot under **Controls**, including
**Controls > Key Map**, which decides which physical key is which button and saves
to `Slot/Config/buttons.ini`.

## Getting the stock menu back

Slot starts by default. To start in the stock menu instead, create an empty file
`Slot/Config/boot-menu` on the card and reboot. Delete it to start in Slot again.
Removing the `miyoo355` folder does the same thing.

To remove Slot completely, delete `App/Slot`, `Tools/my355/Slot.pak` and
`miyoo355/app/MainUI`. Your `Slot` folder (games, saves, states) is separate; keep
it if you might come back.

## If you already use MinUI

The same zip installs the pak into `Tools/my355/Slot.pak`, so **Tools → Slot**
works as before, and your MinUI `auto.sh` can still boot straight in. On a MinUI
card the device boots MinUI, so the "starts in Slot" part of this page does not
apply there. Slot's own folder and files are the same either way.

## When something is wrong

Slot writes `Slot/slot.log` on every run, and the autoboot script writes
`slot-launch.log` at the card root. Read those first; they say what it was doing
when it stopped.

| What you see | What it usually is |
| --- | --- |
| No Slot in the App list | the folders are one level too deep, or the card is not FAT32 |
| It boots to the stock menu | `Slot/Config/boot-menu` exists, or `miyoo355` is missing |
| It boots to a black screen | the card's `Tools/my355/Slot.pak` is incomplete; re-unzip, or run `./install.sh` again |
| Buttons do nothing at all | the pad is switched off: check `slot.log` for `pad joy_type`; the installer's files handle this, a hand-copied pak older than v0.13.0 does not |
| A game shows a black screen but slot is running | usually a bad dump; try a known-good copy of the same game |
| Nothing happens on a PlayStation game | it needs the `.cue` with its `.bin` files, or a `.chd`; check `slot.log` for `core refused` |

Which games have actually been run is in
[COMPATIBILITY.md](../COMPATIBILITY.md); "boots" and "plays through" are tracked
separately there.

## Other devices

Slot's data folder and settings are device-independent, but the Flip is the only
device this fork supports. What a port needs is in
[PORTING.md](../PORTING.md), and the devices that could come next are in
[SYSTEMS.md](../SYSTEMS.md). The A30 and Mini Plus builds are experimental
pre-releases on the `experimental` branch.
