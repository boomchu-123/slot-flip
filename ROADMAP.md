# Roadmap

What's planned for slot-flip, roughly in order. This is a hobby project, so
nothing here is a promise — but it's the honest current direction.

Want to add something? See **[Requesting a feature](#requesting-a-feature)**
at the bottom. Want to build one? Check
**[CONTRIBUTING.md](CONTRIBUTING.md)**, and comment on (or open) the issue.

Legend: ✅ done · 🚧 in progress · 📋 planned · 💡 wanted / needs discussion

---

## Milestones

Four stages, on purpose:

| Milestone | What it means |
| --- | --- |
| **1.0** | Both Miyoo devices — the **Flip** and the **Mini Plus** — each on the firmware it ships with and **no MinUI needed**: the nine systems that ship today, PlayStation working end to end, and an install a stranger can follow. On the current upstream base. |
| **1.x** | More systems, one at a time, after 1.0. They are additive and never block the 1.0 gate. |
| **2.0** | The platform layer: input, audio, power and display behind one seam per device, so slot runs on other Linux handhelds. Ports stop being a fork of the frontend, and the fork becomes easy to rebase onto upstream. |
| **after 2.0** | More devices, one at a time. Device count only grows once the platform layer exists and the next device is verified on hardware. |

Keeping them apart is deliberate. Systems are additive and can ship one at a time;
the platform layer rewrites the device seams and cannot. Mixed into one release,
a device port in progress would hold the systems work back, and a systems release
would keep re-shipping half-finished ports.

### 1.0 — both Miyoo devices, done properly

Set by the maintainer: the systems already supported must be solid, PlayStation
must work end to end, and installing it must not need a guide written by someone
who already knows the answer. In October 2026 the maintainer also moved the
**Miyoo Mini Plus into 1.0** rather than leaving it to 2.0, so 1.0 means slot
running on the Flip **and** the Mini Plus, each on the firmware it ships with and
with no MinUI. No other device is promised until 2.0.

**Install first, on the stock firmware.** A Flip out of the box works; Slot
should too. That was proven on hardware in October 2026: booted on the Flip's own
OS, Slot started from the stock launcher's **App** list, with input, sound, lid
sleep, brightness and re-entering games all working. So the install is **one zip
unzipped onto a FAT32 card**, which also makes the device start straight in Slot,
and MinUI is a second entry that keeps working rather than a requirement.
The Mini Plus is **not there yet**: it installs today as a MinUI Tool
(`Tools/miyoomini/Slot.pak`), and moving it to its own launcher is part of the
1.0 work, not an assumption.

That has three consequences worth stating:

- **No MinUI needed.** MinUI was archived in November 2025 and has no licence
  file, so it was a poor thing to depend on. The `Tools/my355/Slot.pak` entry stays
  for the people who use it, but it is no longer tested for each release and no
  longer part of the gate. The Mini Plus is still on that entry for now; its
  stock-launcher entry is 1.0 work.
- **Nothing on the device changes.** On the Flip, installing MinUI rewrites its
  internal system image; this writes only to the SD card, so uninstalling is
  deleting a folder.
- **A card that is already good stays good.** No reformatting, no firmware zip, no
  launcher to install first.

**Each stock launcher leaves something unset that MinUI used to do.** On the Flip
it is the pad: stock leaves it switched off (`joy_type` `-1`) where MinUI sets it,
and Slot had silently relied on that. The pak sets it for itself and puts stock's
value back on exit. The Mini Plus will have its own list of these, to be found on
the device.

**Upstream base first.** Ship on upstream slot's current release (v1.9.0 at the
time of writing), so 1.0 carries the fixes for card writes, a core that stops
answering, label caching and the undo rows. A system added on an old base is work
that has to be merged twice.

- **The nine systems that ship today** on **both devices** — GBA, GB, GBC, PS1,
  NES, SNES, MD, SMS, GG — each running a real game at the core's own frame rate
  with sound, on each device: ✅ on the Flip (see [COMPATIBILITY.md](COMPATIBILITY.md)),
  and on the Mini Plus a game from each system launched and played at full speed
  with sound.
- **PlayStation, working end to end:** a timed heavy 3D game (✅ Tekken 3 holds the
  core's full 60 fps on a Flip), save integrity across reboots (✅ the memory card
  is read back after one), and a disc change (✅ exercised in the in-game menu on a
  three-disc game). The analog sticks' roughness at full deflection is a **known
  issue accepted for 1.0**, with its workaround in
  [COMPATIBILITY.md](COMPATIBILITY.md).
- **Field testing, not gates.** Playing every title through, a game *asking* for its
  second disc, judging each title's sound by ear, headphones, L3/R3 in a game, and a
  Flip that has never had MinUI in its flash are **not release gates**. They are
  what the first months of field reports are for, and the release notes say which of
  them have not been done. A gate that waits for someone to play nine systems to the
  end holds the release for weeks and proves less than a hundred owners playing what
  they already own.
- **An install a stranger can follow, on each device**: one zip unzipped at the
  card root, which also makes the device start in Slot; `install.sh` inside it for
  anyone who would rather not move folders by hand, with `--check` and `--dry-run`;
  and an install guide with screenshots. Each device's own firmware is the target;
  the MinUI entries keep working but are not part of the release check. **No
  Windows-specific installer:** unzipping onto a FAT32 card is the same drag and
  drop in Explorer as in Finder.
- **Compatibility is the bar, not system count**: [COMPATIBILITY.md](COMPATIBILITY.md)
  lists what has been played, what is known broken, and which BIOS is needed.
  🚧 Started: 38 titles across all nine systems were cold-booted on a Flip, 37 reach
  their own intro and one (Fire Emblem, GBA) draws nothing (a bad dump, not Slot).
  All nine were then launched and run on the shipping v0.15.0 build at the core's
  own frame rate, with the sound device confirmed running at 48 kHz (see
  [COMPATIBILITY.md](COMPATIBILITY.md)). On the Mini Plus, GBA, GB and GBC launch
  and play, with no title catalogue yet. "Boots" is still not "plays through": no
  game on the Flip has been played start to end, and sound has not been judged by
  ear. In-game saves are further along than that: every cartridge core now has a
  real-core test that its save RAM round-trips (fceumm, snes9x2005, Genesis Plus
  GX and PCSX-ReARMed), a GBA save was carried through a reboot on the device, and
  a PlayStation memory card was read back after one, so what is left there is the
  title-by-title play-through.
- **1.0 criteria:** all nine systems run a real game with sound at the core's own
  frame rate on the Flip (done) and on the Mini Plus; PlayStation has no failure
  that stops a game being played through on the Flip, the analog sticks' roughness
  at full deflection being a known issue accepted for 1.0 (see
  [COMPATIBILITY.md](COMPATIBILITY.md)); unzipping the release onto a fresh FAT32
  card gives a running Slot on **each device's stock firmware with no MinUI
  installed**, **and** `install.sh` does the same from a blank card, checked on
  hardware on both; the download passes `tools/check-pak-licences.sh`; the README
  table matches reality; and the release has been installed and run on both devices.

- **Stay free and open source.** No plans to sell. Donations never unlock
  anything. Non-commercial cores are bundled knowingly in the one free download
  (snes9x2005, Genesis Plus GX, and PicoDrive if it is added), with their notices.
- **Not a goal:** a desktop app. The `host` build is development and render-test
  tooling.

### The second device: Miyoo Mini Plus (`miyoomini`)

Same Miyoo family, no GPU, and the largest audience of the Miyoo handhelds. It was
moved into 1.0 in October 2026, while the maintainer's own unit was still on
order: the port should ship as part of 1.0 rather than wait for 2.0. The plan
before that was 2.0, and the roadmap said so; this is the change.

The display is the hard part: with no GPU, the CPU rasteriser that consumes the
same draw list as the GLES pipeline and blits to `/dev/fb0` **is** the renderer,
and it is done. The rest is the same shape as the Flip: a pak, a launch script,
buttons, power, backlight and cores.

- ✅ armhf build, pak, launch script, hardware and framebuffer probe
- ✅ Software compositor, drawn upright by default; the full key table is confirmed
  on hardware and every key is logged
- ✅ armhf gpSP (GBA) and mGBA (GB/GBC) cores bundled, refreshed by `launch.sh`
- ✅ GBA, GB and GBC launch and play on hardware (exp-v0.4.0)
- ✅ POWER held shuts the device down
- ✅ The Game Boy and Game Boy Color cart slots line up (exp-v0.4.2)
- ✅ The brightness setting moves the panel across all nine levels (exp-v0.4.3,
  confirmed on exp-v0.4.4); it had only reached levels 1 to 3 because the steps
  spanned the PWM period instead of the 6 to 100 the panel uses. MinUI's own
  Menu + volume control did not update slot's number; exp-v0.4.5 follows it,
  **unconfirmed**
- ✅ **Speed.** Games run at close to full speed (exp-v0.4.3), measured: gpSP on
  GBA holds 57 to 58 of 59.7 frames a second, mGBA on GB and GBC about 60, mGBA on
  GBA about 49. MinUI leaves the CPU at its 504 MHz menu speed when a Tool starts;
  asking for 1296 MHz fixed it
- ✅ **Sound** (exp-v0.4.10, confirmed by a tester by ear on every core). It plays
  through `/dev/dsp`, which the console's `libpadsp` hands to the audio server;
  the vendor interface (`MI_AO`) stays refused while that server runs. Possible
  slight slowdown in gpSP, not measured. Delay, headphones and long play not
  checked
- 🚧 **Pokemon Emerald on mGBA crashes at load** (16 MB; gpSP and small ROMs are
  fine). exp-v0.4.11 loads ROMs of 8 MiB and up by path, **unconfirmed**
- 📋 **Install on the Mini Plus's own launcher.** Today it is a MinUI Tool
  (`Tools/miyoomini/Slot.pak`). 1.0 needs the same shape as the Flip: one zip at
  the card root, an entry in the launcher the device ships with, and no MinUI.
  The exact mechanism is to be found on the device, not assumed from the Flip's:
  the two launchers are not the same.
- 📋 **Which lines the stock launcher leaves unset**, the way `joy_type` is on the
  Flip, so `launch.sh` sets them for itself and restores them on exit.

The port is **not** on the `experimental` branch: it lives on
`feat/miyoomini-software`, cut from `flip`, and ships as `exp-v*` pre-releases
until the list above is clear. `experimental` is behind `flip` and now only holds
the earlier A30 work.

### 1.x — more systems, after 1.0

More emulators are cheap to add, but they are not a 1.0 gate. Each ships when it
has a licence that has been read, a real-core test, art, and a game that has run on
each device it ships for. **[SYSTEMS.md](SYSTEMS.md) is the register.**

Order is by cost, not ambition:

1. **No new core:** SG-1000 and Sega CD / Mega CD on Genesis Plus GX (a system
   entry, art, CD BIOS handling and a test), and Famicom Disk System on fceumm.
2. **Small cores:** PC Engine, Neo Geo Pocket, WonderSwan, Atari Lynx, Atari
   7800, Intellivision, Virtual Boy.
3. **Free-only:** Sega 32X on PicoDrive (non-commercial, like Genesis Plus GX).
4. **Only if it runs at full speed:** anything heavier is measured on the Flip
   before it is promised.

Every one of these is a credit to the core authors, not a reason to hold 1.0.

### Merging upstream, every release

**Start now, not at 2.0.** A scheduled job fetches the next upstream release,
merges it into a sync branch, checks it compiles and opens an issue with the
conflicts (or a ready-to-review branch). A human still reviews the result and
opens the PR, because a clean text merge can still be wrong. One release at a
time, never squash, and bump `build_info::UPSTREAM` in the merge.

The 2.0 restructure then moves the fork to a **pinned upstream base plus an
ordered patch layer**, so the same job becomes a bump and a reapply.

### 2.0 — the platform layer and the next device

The second half of "run on as many handhelds as realistic", after 1.0 has put two
devices on that list.

- **One seam per device**: input, audio, power and display, with a backend per
  device instead of `cfg(feature = "flip")` reaching into shared code; the
  frontend, store and UI stop knowing which device they are on.
- **Runtime detection where possible**: probe for the lid, LED, rumble motor, stick,
  battery and panel size; a missing one is a no-op, not an error. One binary should
  adapt to more than one panel, rather than compiling the layout in. Device facts a
  user can fix (key map, paths) live in a config file.
- **A core built per CPU**: aarch64 for the Flip, armhf for the Miyoo family, then
  whatever a new device needs.
- **Keep the fork rebaseable**: pin upstream slot as the base and keep this fork's
  changes as a small, ordered patch layer instead of a long divergent history, so
  the next upstream release is a bump and a reapply rather than a large merge. A
  patch that no longer applies is a hard error, not a silent drop.
- **Each new port ships as an experimental pre-release** until someone with the
  hardware sends a clean `slot.log`; only then does it move to stable. The `exp-v*`
  tags are that line.
- **Credit the ports that came before.** Other community ports of slot to Miyoo
  hardware exist, and where one has already solved part of a port (a CPU renderer,
  an audio sink, a battery readout) the right move is to read it, credit it and
  check its licence, not to start again. Anything taken in arrives through review
  like any outside code.
- **2.0 criteria:** the platform layer is in place, both 1.0 devices still pass
  their own checks, and no `cfg(feature = ...)` for a device is left in shared code.

### After 2.0 — more devices, one at a time

Device count grows only after the platform layer exists and the next device is
verified on hardware. Order is by audience and by how much the port reuses what
already exists:

1. **Miyoo A30** (`my282`) — cheap, has a GPU, reuses the GLES path the Flip
   already has, and shares the Miyoo input and power family.
2. **One Anbernic H700 device** (RG35XXSP or RG SP) — a different SoC and OS;
   worth doing once the seams are proven on the Miyoo family.
3. **TrimUI Brick / Smart Pro, Powkiddy RGB30** — only when someone with the
   hardware can test.

Every device in this list needs a person with that device. A build that has never
run on the hardware is "builds, untested", not supported, and stays a pre-release.

---

## Stable — Miyoo Flip (`my355`)

The Flip build is the supported release. It tracks upstream slot one release at
a time; the fork's own work is the Flip port itself and a few small additions.

**Done**
- ✅ SDL2 + GLES2 display, Flip button map, lid sleep, power LED, 48 kHz audio
- ✅ Synced to upstream slot **v1.8.0**: the redesigned shelf and menus, in-game
  menu, favorites and recents, in-app wallpaper, bezels, **Controls** (in-game
  buttons, shortcuts and shelf keys, with chords and clash warnings), and the
  new palette and audio fixes
- ✅ **Key Map** under Controls: which physical key each button is, saved to
  `buttons.ini` ([#8](https://github.com/boomchu-123/slot-flip/issues/8))
- ✅ Lid power-off timer (upstream's **General > Lid Power Off**; the old Sleep
  Timeout setting carries over once)
- ✅ Shaders default to Off on the Flip
- ✅ **About shows both versions**: the fork build and the upstream slot release
  it follows, plus a one-time "Updated to ..." word after an update
- ✅ **Cart labels that are forgiving to add**: one flat `Labels/` folder works,
  and a PNG named after the game pairs even when the ROM has region or revision
  tags
- ✅ **Released v0.5.0**: the v1.8.0 sync, Key Map under Controls, and the rebuilt
  mGBA core
- ✅ **Released v0.6.0**: the About plate names the Flip, the one-time update word,
  and the forgiving cart labels

- ✅ **Released v0.7.0**: PlayStation discs. A PS1 shelf, discs that spin into a
  console-style slot with cover art, a PlayStation pad on the Flip's buttons, and
  the PCSX-ReARMed core bundled ([#20](https://github.com/boomchu-123/slot-flip/issues/20))

- ✅ **Released v0.8.0**: NES games on a drawn Game Pak, with fceumm bundled

- ✅ **Released v0.9.0**: Super Nintendo on a drawn cart, snes9x2005 bundled (the download is now free-only)

- ✅ **Released v0.9.1**: the slot is drawn as each console ([#27](https://github.com/boomchu-123/slot-flip/issues/27)), and Lid Power Off gains **Never**

- ✅ **Released v0.10.0**: Sega Mega Drive / Genesis, Master System and Game Gear on drawn black carts, with Genesis Plus GX bundled

- ✅ **Released v0.10.1**: label scans keep their own shape on the NES, SNES and Sega carts ([#30](https://github.com/boomchu-123/slot-flip/issues/30))

- ✅ **Released v0.11.0**: PlayStation memory cards are kept with the other saves, a multi-disc game can change disc from the in-game menu, and the sticks can be passed to a game as a DualShock's (off by default)
- ✅ **Released v0.12.0** (upstream v1.9.0): durable card writes, core timeouts, label-cache
  stall, undo save/load, screenshots, bezels from the card, and the PlayStation stick
  clicks as L3/R3. Checked on a Flip; the notes list what was not
- ✅ **Released v0.13.0**: the Flip's own firmware as the main install (`App/Slot` in
  `Slot-my355.zip`), an optional boot-straight-into-slot zip, and the pad fix
  (`joy_type`) moved into the pak's `launch.sh`. Both launchers checked on hardware;
  headphones and a factory-fresh Flip are still unverified, and the notes say so.

**Next — the Flip side of 1.0**
- ✅ **Released v0.14.0**: one zip that installs and autoboots, plus `install.sh`
  (`--check`, `--dry-run`, `--stock-menu`). Checked on hardware: the installer's
  output on a real FAT32 card is byte-identical to the zip, and a Flip boots from
  it straight into Slot.
- ✅ **Released v0.15.0**: runs on the Flip's own firmware with **no launcher daemon
  doing its work**: the power node is opened, the codec path (SPK/HP) is set by slot
  instead of `keymon`, the left stick walks as a D-pad again, the analog toggle and
  axis bounds are set explicitly, and the Gameplay page scrolls. `INSTALL.md` is
  rewritten around "unzip onto your card". Checked on a Flip by hand: power off,
  sound, the light on lid close, no phantom volume, the left stick, and the menu.
  Not verified: headphones routing, L3/R3 toggling the pad, FF7 analog.
- ✅ **PlayStation speed and the sound stream measured** on that release asset:
  Tekken 3 holds the core's full 60 fps in a 3D scene (120 to 122 emulator frames
  every 2 s, twelve samples), and the card is open at 48 kHz stereo with the
  speaker path set by Slot. Measured from Slot's trace, not watched. See
  [COMPATIBILITY.md](COMPATIBILITY.md).
- ✅ **Every one of the nine systems launched and run on that build**: each on a
  real game, cold booted, at the core's own frame rate (59.4 to 60.1 fps) with the
  sound device running at 48 kHz. A GBA save was then carried through a reboot and
  read back by the game, a PlayStation memory card was read back after one, and the
  in-game menu's disc change was exercised on a three-disc PlayStation game. Still
  open: playing a game through, a game *asking* for its second disc, and judging the
  sound by ear.
- ✅ **Released v0.15.1**: the log names what holds the sound device when ALSA
  cannot open it, and the compatibility page carries the 1.0 checks (nine systems
  run, saves across a reboot, a disc change). No change to how a game plays.
- 📋 **Analog steering at the extremes** is a **known issue, accepted for 1.0**
  (see [COMPATIBILITY.md](COMPATIBILITY.md) and the README's workaround): full
  slams and end-to-end sweeps are sometimes ignored by Gran Turismo 2 even though
  the core is handed the right value. Tried and kept: a wider "all the way" edge,
  peak capture between reads, a short hold, swing-aware logic. Untried: capping the
  extreme values, a second analog game to tell a game quirk from a pipeline fault.
- 💡 **Factory-fresh Flip**: this is the one install path never tested on a Flip
  that has never had MinUI's boot hook in its flash. It should be the same stock
  script, but it is not proven. One more tester settles it.

- 💡 **Card repair on MinUI.** Upstream's SELECT + START repair is switched off
  on the Flip (its script is written for upstream's own devices and unmounts the
  card, which the launcher owns). A safe, tested version for the Flip needs its own
  design; for now a read-only card is repaired from a computer. See
  [#17](https://github.com/boomchu-123/slot-flip/issues/17)
- 💡 Cartridge-label artwork: fetch real label images automatically (blocked
  on a ScreenScraper developer ID; currently you supply your own labels)
- 🚧 **More systems** ([#20](https://github.com/boomchu-123/slot-flip/issues/20)).
  PlayStation is out in v0.7.0 and NES in v0.8.0. PlayStation memory cards are kept and
  multi-disc games can change disc (v0.11.0), and the sticks can go to a game (a
  DualShock, off by default, with the stick clicks as L3/R3). Still to do for
  PlayStation: a game actually asking for its second disc (they tend to ask late),
  and timing a heavy 3D game. Super Nintendo is built (v0.9.0), with a core
  whose licence is non-commercial, so the download as a whole is free-only (see
  `AGENTS.md`, section 9). The Sega systems (Mega Drive / Genesis, Master System, Game
  Gear) are out in v0.10.0. Next: the cheaper 2D systems, one core at a time, each only
  once someone has games for it to try. Nothing here is promised until it runs on
  hardware

**Housekeeping that has bitten before**
- Rebuild the mGBA and gpSP cores whenever a sync changes `cores/` (v0.4.0
  shipped a core missing one of v1.7's patches). `AGENTS.md` has the checklist.

---

## Experimental — Miyoo A30 (the 2.0 line)

The A30 is the only device left in the experimental column. It stays an `exp-v*`
**pre-release** until verified on hardware; nothing here blocks 1.0.

### Miyoo A30 (`my282`)

Has a Mali-400 GPU, so it reuses the existing GLES renderer.

- ✅ armhf build, pak, launch script, input map (best guess)
- 🚧 **Verify on hardware** — untested; needs a tester's `slot.log`
- 📋 Confirm/correct the button map from `input-trace.log`
- 📋 Bundle armhf emulator cores
- 💡 Shader tweaks if Mali-400's older GLES2 misbehaves

---

## Wanted — needs discussion

Ideas that would be nice, but aren't committed. Open an issue to push one
forward or volunteer.

- 💡 **More devices, after 2.0** — e.g. Anbernic RG35XX/RG35XX Plus, TrimUI Smart
  Pro / Brick, Powkiddy RGB30. Each is its own SoC + display path, so it's a
  real port, not a repackage. If a device has a GPU, it's usually much cheaper
  (see the A30). Device support table is in the [README](README.md).
- 💡 **NextUI** — a MinUI-based custom OS. Because it keeps MinUI's
  `Tools/<platform>/*.pak` model (and the same `my355` slug on the Flip),
  Slot's Flip pak is expected to be **drop-in with no code changes** — it just
  needs someone on NextUI to verify. See
  [#7](https://github.com/boomchu-123/slot-flip/issues/7).
- 💡 **Theme presets** — ship a few ready-made `theme.txt` looks.
- 💡 **More wallpaper options** in-app.
- 💡 **Docs**: short video/GIF walkthroughs per feature.

---

## Not planned

To set expectations:

- ❌ **Other custom firmware and launchers** (OnionOS, Allium, ArkOS…). On each
  device the stock launcher and MinUI are both supported, because both were run on
  hardware. Another launcher needs its own integration and someone to test it; the
  stock launcher is the one that ships on the device.
- ❌ **A Windows installer.** The release is a zip you unzip onto the card, which is
  the same drag and drop in Explorer as in Finder. `install.sh` exists for people
  who would rather not move folders by hand, not because the files need it. A
  second installer is another thing to test and another way to install a build
  nobody else has, for no gain.
- ❌ **Bundling or sourcing ROMs/BIOS.** Bring your own legally dumped files.
- ❌ **Non-Miyoo hardware with no GPU** (framebuffer-only) beyond the Mini Plus
  case, unless someone wants to own the port.

---

## Requesting a feature

1. **Check this roadmap and the open issues** — it may already be listed.
2. Open a **[feature request](.github/ISSUE_TEMPLATE/feature_request.yml)**.
   Say what you want, why, and which device it's for.
3. If you want to build it, say so in the issue — that moves it up the list.
   Device ports especially need someone who can test on the hardware.

Feature requests are triaged with these labels:

| Label | Meaning |
| --- | --- |
| `enhancement` | A new feature or improvement |
| `help wanted` | Up for grabs — I'd welcome a PR |
| `good first issue` | Approachable for a newcomer |
| `device: a30` / `device: mini plus` | Specific to one device's port |
| `porting` | Adding/adapting a device |
| `wontfix` | Out of scope (see *Not planned*) |
