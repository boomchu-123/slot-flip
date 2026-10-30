# Compatibility

What has actually been run on the two 1.0 devices, the **Miyoo Flip** and the
**Miyoo Mini Plus**, and what has not. This page is the bar for 1.0: it lists
results, not hopes, and it says plainly what each result does and does not prove.

**Checked on:**

- **Games:** slot-flip v0.12.0, a Miyoo Flip (`my355`) running MinUI, 2026-10-24.
  38 titles across the nine systems, each cold-booted — and **not re-run since**,
  so treat the tables as v0.12.0 evidence.
- **Device checks:** slot-flip v0.15.0 on the Flip's own firmware, 2026-10-25. See
  [Checked on the device](#checked-on-the-device-beyond-booting).
- **Miyoo Mini Plus:** the unreleased build after exp-v0.4.14, on a Mini Plus on its
  own firmware with no MinUI, 2026-10-30, driven over adb. Earlier button, display
  and sound checks were made on exp-v0.4.x as a MinUI Tool. See
  [Miyoo Mini Plus](#miyoo-mini-plus-miyoomini).

## What the results mean

| Result | Meaning |
| --- | --- |
| ✅ **Boots** | Cold-booted on the real device, on the core Slot picks for that system. The game reached its own logo, title or intro, and the picture was changing or showing the game's art. Checked from screenshots taken by Slot itself. |
| ❌ **No picture** | The core opened the game and kept running, but nothing was drawn. |
| ⬜ **Not checked** | Nobody has recorded a result. This is not a failure. |

**"Boots" is a low bar, on purpose.** It does not tell you that the game plays all
the way through, that its sound is right, that it holds full speed in its heavy
scenes, or that its in-game save works. Those are tracked separately below, and
until a row says so, assume they are **not** known.

**The game tables below are the Flip only**, from v0.12.0. The Mini Plus has its
own section near the end.

**No BIOS was needed.** Every system on the Flip booted with an empty `Slot/BIOS`
folder. The only BIOS Slot looks for is the optional GBA one (`gba_bios.bin`),
which just adds the real boot logo.

## Game Boy Advance (19 titles)

Run on gpSP, the default GBA core, except where a row says otherwise. Every one of
these was cold-booted.

| Game | Result | What was seen |
| --- | --- | --- |
| Avatar: The Last Airbender | ✅ Boots | Publisher logo, then the intro |
| Donkey Kong Country | ✅ Boots | Intro scene |
| Final Fantasy I & II: Dawn of Souls | ✅ Boots | Title screen |
| Final Fantasy VI Advance | ✅ Boots | Stopped at the game's own "press any button" health warning (no input was sent) |
| Final Fantasy Tactics Advance | ✅ Boots | Opening credits |
| Fire Emblem | ❌ No picture | Black for 30 s on both gpSP and mGBA. See [known problems](#known-problems) |
| Kirby & The Amazing Mirror | ✅ Boots | Title screen, then the intro |
| Kirby: Nightmare in Dream Land | ✅ Boots | Title screen |
| The Legend of Zelda: A Link to the Past & Four Swords | ✅ Boots | File select screen (run on mGBA, a per-game core choice, not gpSP) |
| The Legend of Zelda: The Minish Cap | ✅ Boots | Title screen |
| Metroid Fusion | ✅ Boots | Intro |
| Mother 1+2 (fan translation) | ✅ Boots | The translation team's notice screen |
| Mother 3 | ✅ Boots | Stopped at the game's own health warning |
| Pokémon Emerald | ✅ Boots | Intro |
| Pokémon FireRed | ✅ Boots | Intro |
| Pokémon LeafGreen | ✅ Boots | Intro |
| Pokémon Mystery Dungeon: Red Rescue Team | ✅ Boots | Stopped at the game's own health warning |
| Pokémon Ruby | ✅ Boots | Logo, then the intro |
| Pokémon Sapphire | ✅ Boots | Logo, then the intro |

Three of these (Final Fantasy VI Advance, Mother 3, Mystery Dungeon) were still on
the game's "press any button" screen when the shots were taken. That is the game
waiting for you, so the result is a weaker "boots" than the rest.

## Game Boy (2) and Game Boy Color (3)

Run on mGBA.

| Game | System | Result | What was seen |
| --- | --- | --- | --- |
| Kirby's Dream Land | GB | ✅ Boots | Title screen |
| Pokémon Red | GB | ✅ Boots | Intro |
| The Legend of Zelda: Link's Awakening DX | GBC | ✅ Boots | Intro (the storm at sea) |
| The Legend of Zelda: Oracle of Ages | GBC | ✅ Boots | Intro |
| Pokémon Crystal | GBC | ✅ Boots | Intro |

## PlayStation (5)

Run on PCSX-ReARMed, from `.chd` discs and `.m3u` playlists.

| Game | Result | What was seen |
| --- | --- | --- |
| Crash Bandicoot | ✅ Boots | Studio logo and intro |
| Final Fantasy VII (3 discs, `.m3u`) | ✅ Boots | Disc 1: publisher logo, then the opening credits |
| Gran Turismo 2 | ✅ Boots | Title menu |
| Metal Gear Solid (2 discs, `.m3u`) | ✅ Boots | Disc 1: fade-in of the first title card (the frame was dark, but changing) |
| Tekken 3 | ✅ Boots | Opening movie |

## Nintendo Entertainment System (2), Super Nintendo (1)

NES on fceumm, Super Nintendo on snes9x2005.

| Game | System | Result | What was seen |
| --- | --- | --- | --- |
| The Legend of Zelda | NES | ✅ Boots | Title screen, then the story scroll |
| Zelda II: The Adventure of Link | NES | ✅ Boots | Title screen |
| Chrono Trigger | SNES | ✅ Boots | Opening intro (the clock) |

## Sega: Mega Drive (2), Master System (2), Game Gear (2)

All three on Genesis Plus GX.

| Game | System | Result | What was seen |
| --- | --- | --- | --- |
| Sonic the Hedgehog | MD | ✅ Boots | Title screen |
| Castlevania: Bloodlines | MD | ✅ Boots | Publisher logo |
| Phantasy Star | SMS | ✅ Boots | Title screen |
| Road Rash | SMS | ✅ Boots | Publisher logos, then the intro |
| Sonic the Hedgehog: Triple Trouble | GG | ✅ Boots | Intro scene |
| Shinobi II: The Silent Fury | GG | ✅ Boots | Intro, then the title screen |

## Known problems

- **Fire Emblem (GBA) shows no picture.** The core opens the ROM, finds its 128 KiB
  save and keeps running, but the screen stays black for the full 30 s that was
  watched, on both gpSP and mGBA. The cartridge header looks right (game code,
  fixed bytes, 16 MiB). Because two unrelated emulators agree, the likeliest cause
  is the ROM file, but that has **not** been proven: it needs a second, known-good
  dump of the same game to settle it. If you can run this title, please report
  what you see.

- **Analog steering is rough at the extremes (PlayStation).** With Analog Sticks on, a
  hard slam or a fast sweep from one end to the other is sometimes not registered by
  the game. Seen in Gran Turismo 2, where the car then does nothing. The core was
  logged reading the sticks every frame and being handed full deflection in both
  directions, so Slot is delivering the value; why the game ignores it is **not known**,
  and it was not tried in a second analog game. Gentle and moderate steering works.
  Leave Analog Sticks off to use the left stick as a D-pad instead.
  **Accepted as a known issue for 1.0:** it is not one of the release checks, and
  the workaround is the default (Analog Sticks off).

- **The Mini Plus cannot run large GBA games with a clock chip on mGBA.** Pokemon
  Emerald (16 MB) closes slot on mGBA, and gpSP plays the same game. Two steps, as
  the reports showed them:
  - exp-v0.4.10 crashed as the game loaded: mGBA copies what it is given into a
    memory map of its own, and the map failed. exp-v0.4.11 gives mGBA the file's
    path instead for ROMs of 8 MiB and up, and the crash at load **is gone**
    (confirmed by the next report).
  - It then crashes a moment later, in mGBA's `_pristineCow`. For a cartridge with
    GPIO hardware (Emerald's clock), mGBA makes a private copy of the ROM in a
    32 MB map and does not check that the map succeeded. The log at that point
    shows 103 MB total, about 30 MB available and slot holding 62 MB, so the map
    most likely fails for lack of memory. That is a reading of the kernel's
    behaviour, **not a measurement**, and the cartridge having GPIO is likewise
    inferred, not checked.
  - **Accepted as a known limit for now.** GBA defaults to gpSP, which plays it,
    and mGBA's own job on this console is GB and GBC, which are small. How much
    of slot's 62 MB is its own is being measured (the heartbeat lines now carry
    it); it also decides how much room the PS1 core will have.

## Checked on the device, beyond booting

### On the Flip, v0.12.0 (MinUI)

These were run on a Flip with the build that became v0.12.0, on the games named.

- **Each system loads its own core:** GBA (Metroid Fusion), PlayStation (Crash
  Bandicoot), Super Nintendo (Chrono Trigger), NES (Zelda) and Mega Drive (Sonic)
  were each booted and confirmed to load the core meant for them.
- **Screenshots** (SELECT + X) write a PNG on GBA and PlayStation, and the boot
  results in the tables above were gathered with them.
- **Save state, then Undo Save** removes the state, on GBA and PlayStation.
- **PlayStation memory card:** Crash Bandicoot opens its 128 KiB card.
- **The left stick as a D-pad** (Analog Sticks off) walks Crash Bandicoot. The
  Flip never did this before this release: it was not registered as having a left
  stick, and the code used another device's axis numbers.
- **Hold POWER** powers the Flip off (it could not before: Slot never opened the power
  button's input node).
- **Lid close and open**, and the MENU button (tap opens the in-game menu, hold
  closes the game), were checked by a person on Crash Bandicoot.

### All nine systems on the Flip, v0.15.0

Run on the shipping v0.15.0 asset on a Flip with its own firmware, booted from the
card into Slot with no launcher daemon. Each row is a cold boot of a real game,
with the emulator's frame count and the sound device sampled from Slot's own trace
and the kernel: not an impression, and not screenshots alone.

| System | Game | Core | Emulator fps | Sound device |
| --- | --- | --- | --- | --- |
| GBA | Pokemon Emerald | gpSP | 60.0 of 59.7 | running, 10 of 10 samples |
| GB | Kirby's Dream Land | mGBA | 60.0 of 59.7 | running, 10 of 10 |
| GBC | Pokemon Crystal | mGBA | 60.0 of 59.7 | running, 10 of 10 |
| NES | The Legend of Zelda | fceumm | 60.1 of 60.1 | running, 10 of 10 |
| SNES | Chrono Trigger | snes9x2005 | 60.0 of 59.9 | running, 10 of 10 |
| MD | Sonic the Hedgehog | Genesis Plus GX | 60.1 of 59.9 | running, 10 of 10 |
| SMS | Phantasy Star | Genesis Plus GX | 60.0 of 59.9 | running, 10 of 10 |
| GG | Sonic Triple Trouble | Genesis Plus GX | 60.0 of 59.9 | running, 9 of 10 |
| PS1 | Crash Bandicoot | PCSX-ReARMed | 59.4 of 60.0 | running, 10 of 10 |

"Running" is `/proc/asound/card0/pcm0p/sub0` in state `RUNNING` with
`S16_LE`, two channels and 48000 Hz. It is the *stream*, not the sound: nobody has
still listened to a Flip title and judged it. Slot closes the device after three
seconds of silence on purpose, so a game sitting on a silent screen (Chrono
Trigger's name entry, for one) shows it closed; that is not a fault.

What this does **not** say: that any of these was played through. GBA was the only
one taken further than its opening screens (below). The rest were launched and run
to confirm the core, the picture, the frame rate and the sound stream.

- **GBA in-game save, across a power cycle:** Pokemon Emerald was played through
  its intro and named, saved from the game's own Start menu, ejected, and the Flip
  rebooted. On the next cold boot the game's continue screen listed the same save
  (the name entered, 0:05), with the resume state moved out of the way first so the
  battery save alone was what the game started from. That is the whole path: the
  game's save, the card file, the reboot, and the game reading it back.
- **PlayStation memory card, across a power cycle:** Final Fantasy VII, cold
  booted from its three-disc playlist with no resume state, showed `Continue?` on
  its title screen: the game had read the memory card on the card, after a reboot.
- **Disc change, on a real game:** in Final Fantasy VII (a three-disc `.m3u`), the
  in-game menu's Disc row stepped the disc, and Slot's log records the emulator
  taking each change (`slot: emu: changing to disc 2 of 3`). The game restarted on
  the disc chosen. What is still missing is a game *asking* for its second disc
  part-way through, which means playing to the end of one.
- **Analog sticks reach the core:** with Analog Sticks on, left-stick movement
  written to the gamepad node came back in Slot's own trace as peaks of 31042 and
  -32767, read by the core about 480 times every two seconds. The values were
  injected rather than pushed by hand, so this proves the path from the pad node to
  the core, not the feel of a real stick.

### On the Flip, v0.15.0 (its own firmware, no launcher daemon)

Checked by hand by the maintainer; this is the build that ships as stable.

- **Power:** holding POWER shuts the Flip off. It could not before, because Slot
  never opened the power button's own input node.
- **Sound:** plays, and the codec's output path is set by Slot rather than by
  MinUI's `keymon` (which is no longer started). No phantom volume-down when the
  lid opens, and the light no longer switches off on lid close.
- **Left stick as a D-pad** walks Crash Bandicoot again.
- **The Gameplay page scrolls** instead of overflowing.
- **Analog sticks** work, but are still rough at the extremes (known problem above).
- **PlayStation speed, measured:** Tekken 3, on the shipping v0.15.0 asset (the
  installed binary's SHA-256 is the one in `Slot-my355.zip`), booted straight
  into the game on the stock firmware. Twelve consecutive samples had 120 to 122
  emulator frames every 2.0 s, against the core's own 60.000 fps, with the CPU at
  1992 MHz and nothing missed. Full speed in a 3D game, measured from Slot's trace
  rather than watched.
- **Audio, measured:** while that game ran, the sound card was open and running at
  48000 Hz, S16_LE, two channels, 10 ms periods, and the codec's `Playback Path`
  read `SPK` — set by Slot with no launcher daemon running. That is the stream, not
  the sound: nobody has listened to a Flip title and judged it (see below).

Not verified on v0.15.0: headphones routing (the code path is there, the jack was
not tried), L3/R3 reaching a game, and the analog sticks in Final Fantasy VII.

## Miyoo Mini Plus (`miyoomini`)

No GPU: the picture is drawn by the CPU rasteriser, not the Flip's GLES path, and
the console has about 100 MB of memory in all. **Not 1.0-ready:** all nine systems
run, but Mega Drive, Super Nintendo, Game Boy Color and PlayStation are short of full speed.

**Checked on:** the build after exp-v0.4.14 (unreleased), a Mini Plus on its own
firmware with no MinUI, 2026-10-30. Slot started from the card's own boot entry
(`miyoo354/app/MainUI`), every run came from the console's cold boot or a restart of
Slot, and the figures are the emulator's own frame count and the process's resident
memory read from `/proc`, over a 40 second run after the first frame. One game per
system; nothing was played through.

| System | Game | Frames a second | The core's rate | Resident (peak) | Spare memory (lowest) |
| --- | --- | --- | --- | --- | --- |
| NES | Asterix (Europe) | 50.1 | 50 (a PAL game) | 30.1 MB | 49.2 MB |
| Super Nintendo | Chrono Trigger | **51.1** | about 60 | 46.8 MB | 27.7 MB |
| Mega Drive | Sonic the Hedgehog | **51.0** | 59.9 | 43.9 MB | 29.6 MB |
| Master System | Phantasy Star | 58.6 | about 60 | 45.1 MB | 28.2 MB |
| Game Gear | Shinobi II | 59.7 | about 60 | 44.9 MB | 28.4 MB |
| Game Boy | Pokemon Red | 58.3 | 59.7 | 31.8 MB | 43.2 MB |
| Game Boy Color | Link's Awakening DX | **56.2** | 59.7 | 37.7 MB | 37.1 MB |
| Game Boy Advance | Pokemon Emerald | 58.7 | 59.7 | 53.1 MB | 20.8 MB |
| PlayStation | Crash Bandicoot | **51.9** | 59.9 | 54.3 MB | 20.9 MB |

Bold is short of the core's rate by more than a frame in twenty. A picture was read
back from the panel for each, and each was a real picture, not a blank. **Super
Nintendo's rewind** costs it about six frames a second (57.4 with it off, measured
once on `performance`); Mega Drive does not change with it.

- **PlayStation runs and stays up.** It is the system the console could not hold
  before: the core reserved a 16 MB translation cache and the frontend held 30 MB
  of pictures it was not drawing, so the kernel killed the game before its first
  frame, and later thrashed (a 2.9 second frame) at 2.6 MB spare. The core is now
  built with a 4 MiB cache, pictures that are not on screen are kept compressed, and
  allocator limits stop freed memory piling up. A four minute run on Crash
  Bandicoot, with the shelf's label art loaded and `DEBUG` tracing on, ended with
  21 MB spare, a real picture on the panel and no kernel kill. **At 52 frames a second it is
  playable, not full speed.**
- **Where the time goes.** The picture path costs about 2.5 ms a frame (it was
  about 7 ms for the game plus 7 ms to copy it out); the shortfall is the emulator
  thread itself, measured at about 16 ms of CPU per Mega Drive frame. A rebuild of
  the Genesis core with this project's recipe came out byte-for-byte the same as the
  one already on the card, so the core is not mis-built; why it is slow here is open.
- **Install on the console's own launcher, no MinUI.** `App/Slot` is the launcher
  entry and `miyoo354/app/MainUI` boots straight into Slot, with the console's own
  launcher kept as `MainUI.stock`. The release zip was built from the committed
  tree, and its `install.sh` was run on the console itself (`--check`, `--dry-run`,
  then a real install over an existing one, under the console's own BusyBox shell):
  the installed files match the zip byte for byte, `MainUI.stock` survived, and
  the next cold boot reached Slot by itself. The table above is that install's
  run; an earlier run on the same build, before the install, agreed to within about
  a frame. **Not done:** installing onto a card that has never had Slot, and
  getting to the stock menu through `boot-menu` by hand on a fresh card (the flag
  was used once to stop Slot for the install, and removed by the installer).
- **Display, buttons, power, brightness, sound** were checked on exp-v0.4.x as a
  MinUI Tool (below) and have not been re-checked one by one on this build. The
  nine runs were started through Slot's state file, not with buttons, and the sound
  was not listened to. Mega Drive's audio queue under-ran heavily in the runs (the
  core produces frames more slowly than the sound card consumes them), so expect it
  to sound wrong until the speed is fixed.

Earlier results, kept because they are still the evidence for buttons, display and
sound:

- **Display:** upright with no setting to change, and the shelf and menu draw.
- **Buttons:** every button registers, and the whole map was confirmed from the
  device's own key log — Menu, D-pad, A/B/X/Y, L1/R1/L2/R2, Start, Select, POWER,
  volume up and down.
- **Power:** holding POWER shuts the device down.
- **Menu:** opens and responds; the quick menu felt responsive to the person
  using it.
- **Cart slot:** the Game Boy and Game Boy Color slots line up (exp-v0.4.2). Other
  consoles' slots are drawn the same way but not looked at.
- **Brightness (exp-v0.4.4):** the nine levels all look different. Following the
  firmware's own control (exp-v0.4.5) is tested against a fake panel and **not yet
  confirmed on a device**.
- **Sound (exp-v0.4.10):** works on every core the tester tried: "it sounds good to
  me in all cores". It plays through `/dev/dsp`, which the console's `libpadsp` hands
  to the audio server; the vendor interface (`MI_AO`) is refused for as long as that
  server runs. Not judged: how late the sound is, headphones, and long play.

**Not checked, or known not to work:**

- **Resume works; in-game saves and a disc change are untried.** Stopping Slot
  cleanly (SIGTERM) writes `resume.state` and the next start reads it back; this was
  seen for Pokemon Emerald (426 KB) and Crash Bandicoot (4.4 MB) on 2026-10-30. No
  game was played to a save point, so a battery save, a PlayStation memory-card write
  and a disc change have **not** been exercised on this console (the memory card
  and disc change work on the Flip). Quick-menu save states were not used either.
- **Pokemon Emerald on mGBA** still fails a moment after loading (it asks for a 32 MB
  map with about 30 MB spare). gpSP, the default, plays it.
- **A second PlayStation game.** Only Crash Bandicoot was run; Tekken 3 and the
  other four on the card are untried.
- **Buttons on a 32-bit kernel (exp-v0.4.14).** The kernel prints capability masks in
  32-bit words and slot read them as 64-bit ones. Tested in code, **not on a device
  as a separate change**.

## Not checked yet

Do not assume any of these work until a row above says so. From 1.0 on, these are
what field reports settle: the release notes say which have not been done, and an
owner saying what they saw is how a row here moves. None of them is a reason to
hold a release back.

- **Playing a game through.** On the Flip, GBA was played into its first town and
  saved, but no game on any system has been played from start to end. The other
  eight systems were launched and run, not played. The Mini Plus games were played
  briefly rather than through.
- **Sound.** Screenshots cannot hear. The stream was confirmed running at 48 kHz
  with the speaker path set on the Flip (see above), but no title on either device
  has been listened to and judged. The Mini Plus has sound (exp-v0.4.10, by ear, one tester) but its delay has not been measured.
- **Speed in heavy scenes.** Frame rate was not measured per game. On the Flip,
  Tekken 3 is the one PlayStation game timed (60 fps, above); Gran Turismo 2 is
  not, and no NES, SNES or Sega title has been timed.
- **In-game saving for each title**, and resuming from a save after a reboot. The
  plumbing itself is tested against the real cores: fceumm, snes9x2005, Genesis Plus
  GX and PCSX-ReARMed each hand their save RAM over and take it back
  (`crates/slot-retro/tests/`). What no test can show is that a particular game
  writes its save when it says it does, which is the play-through's job.
- **A game asking for its second disc.** The change itself works on the device in
  Final Fantasy VII (above), but reaching a point where the game asks for the disc
  means playing to the end of one.
- **Undo Load**, the screenshot gallery, and L3/R3 reaching a PlayStation game. The
  sticks themselves do reach the core (above).
- **Game Boy Color colour games beyond the three above**, Super Game Boy borders,
  and Master System / Game Gear titles that need a particular region or mapper.
- **Most of the Miyoo A30.** That build is still experimental. A tester has seen
  the shelf and menus upright (exp-a30-v0.3.4) and every button arriving and being
  named (exp-a30-v0.3.6), which is where the 16-byte input-event and capability
  word-size fixes came from. **No game has been played on an A30**: exp-a30-v0.3.8
  is the first build with cores, and whether each printed button label matches its
  mapping, the speed, the sound and the shelf's black bar are all open. The screen
  also sits a few pixels high, with no cause found in the code.

## Adding to this page

A result is welcome from anyone, with the game, what you saw, and the Slot version
(About, in the in-game menu). Open a
[bug report](https://github.com/boomchu-123/slot-flip/issues/new?template=bug_report.yml)
for something that does not work and attach `Slot/slot.log`. For something that does,
a short issue naming the game is enough.

Please say whether you **played** it or only **booted** it. They are different
claims, and this page keeps them apart.

Games are your own legally obtained dumps. This project ships no games, BIOS files
or cartridge art.
