# Compatibility

What has actually been run on a Miyoo Flip, and what has not. This page is the
bar for 1.0: it lists results, not hopes, and it says plainly what each result
does and does not prove.

**Checked on:** slot-flip v0.12.0, a Miyoo Flip (`my355`) running MinUI, 2026-10-24.
**Games:** 38 titles across the nine systems, each cold-booted on the device.

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

**No BIOS was needed.** Every system on this page booted with an empty `Slot/BIOS`
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

## Checked on the device, beyond booting

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

## Not checked yet

Do not assume any of these work until a row above says so.

- **Playing a game through.** Nothing on this page was played past its intro.
- **Sound.** Screenshots cannot hear. Audio was not judged for any title.
- **Speed in heavy scenes.** Frame rate was not measured per game. PlayStation in
  particular has only one game timed (see the README).
- **In-game saving** for each title, and resuming from a save after a reboot.
- **A disc change** on a multi-disc game (Final Fantasy VII, Metal Gear Solid):
  only disc 1 was booted.
- **Undo Load**, the screenshot gallery, and L3/R3 reaching a PlayStation game.
- **Game Boy Color colour games beyond the three above**, Super Game Boy borders,
  and Master System / Game Gear titles that need a particular region or mapper.
- **Anything on the Miyoo A30 or Mini Plus.** Those builds are experimental and
  live on the `experimental` branch. See [TESTERS.md](https://github.com/boomchu-123/slot-flip/blob/experimental/TESTERS.md).

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
