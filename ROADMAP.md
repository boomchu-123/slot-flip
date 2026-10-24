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
| **1.0** | The nine systems that ship today, PlayStation working end to end, and an install a stranger can follow. One device, done properly, on the current upstream base. |
| **1.x** | More systems, one at a time, after 1.0. They are additive and never block the 1.0 gate. |
| **2.0** | The platform layer: input, audio, power and display behind one seam per device, so slot runs on other Linux handhelds. Ports stop being a fork of the frontend, and the fork becomes easy to rebase onto upstream. |
| **after 2.0** | More devices, one at a time. Device count only grows once the platform layer exists and the second device is verified on hardware. |

Keeping them apart is deliberate. Systems are additive and can ship one at a time;
the platform layer rewrites the device seams and cannot. Mixed into one release,
a device port in progress would hold the systems work back, and a systems release
would keep re-shipping half-finished ports.

### 1.0 — the Flip, done properly

Set by the maintainer: the systems already supported must be solid, PlayStation
must work end to end, and installing it must not need a guide written by
someone who already knows the answer. This is the MinUI-on-Miyoo niche: the Flip
first, PlayStation done properly, and no other device promised until 2.0.

**Upstream base first.** Ship on upstream slot's current release (v1.9.0 at the
time of writing), so 1.0 carries the fixes for card writes, a core that stops
answering, label caching and the undo rows. A system added on an old base is work
that has to be merged twice.

- **The nine systems that ship today**, each with a real game played through:
  GBA, GB, GBC, PS1, NES, SNES, MD, SMS, GG.
- **PlayStation to the end:** a game actually asking for its second disc, a timed
  heavy 3D game, save integrity across reboots, and the analog/clicks test.
- **An install a stranger can follow**: MinUI detection, a macOS/Linux installer,
  first-run checks, and an install guide with screenshots. 📋 Design written, not
  built: **[INSTALLER.md](INSTALLER.md)**.
- **Compatibility is the bar, not system count**: [COMPATIBILITY.md](COMPATIBILITY.md)
  lists what has been played, what is known broken, and which BIOS is needed.
  🚧 Started: 38 titles across all nine systems were cold-booted on a Flip, 37 reach
  their own intro and one (Fire Emblem, GBA) draws nothing. "Boots" is not "plays
  through": playing games through, sound, speed and in-game saves are still open.
- **1.0 criteria:** all nine systems pass their games; PlayStation has no known
  failure; the installer takes a blank card to a running Slot on the Flip; the
  download passes `tools/check-pak-licences.sh`; the README table matches
  reality; and the release has been run on the Flip.
- **Stay free and open source.** No plans to sell. Donations never unlock
  anything. Non-commercial cores are bundled knowingly in the one free download
  (snes9x2005, Genesis Plus GX, and PicoDrive if it is added), with their notices.
- **Not a goal:** a desktop app. The `host` build is development and render-test
  tooling.

### 1.x — more systems, after 1.0

More emulators are cheap to add, but they are not a 1.0 gate. Each ships when it
has a licence that has been read, a real-core test, art, and a game that has run
on the Flip. **[SYSTEMS.md](SYSTEMS.md) is the register.**

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

### 2.0 — the platform layer and the second device

The second half of "run on as many handhelds as realistic", deliberately after 1.0.

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
- **Each port ships as an experimental pre-release** until someone with the
  hardware sends a clean `slot.log`; only then does it move to stable. The
  existing `experimental` branch and `exp-v*` tags become the 2.0 line.
- **The second device: Miyoo Mini Plus** (`miyoomini`). Same Miyoo family, no GPU,
  the largest audience of the Miyoo handhelds; it needs a CPU compositor, which is
  exactly the kind of seam 2.0 is for.
- **2.0 criteria:** the platform layer is in place, the Mini Plus is verified on
  hardware, the Flip still passes its own checks, and no `cfg(feature = ...)` for a
  device is left in shared code.

### After 2.0 — more devices, one at a time

Device count grows only after the platform layer and the second device are done.
Order is by audience and by how much the port reuses what already exists:

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

**Next**
- 💡 **Card repair on MinUI.** Upstream's SELECT + START repair is switched off
  on the Flip (its script is written for upstream's own devices and unmounts the
  card, which MinUI owns). A safe, tested version for the Flip needs its own
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

## Experimental — other Miyoo devices (the 2.0 line)

These are the start of **2.0**, and stay on the `experimental` branch as `exp-v*`
**pre-releases** until verified on hardware. The stable `flip` branch keeps
shipping 1.x system work in the meantime; nothing here blocks 1.0.

> **Status:** `experimental` is still on upstream **v1.6**, behind the stable
> branch. It needs the v1.8 sync, the Key Map, and the armhf builds re-checked
> before testers get any of it. Until then, treat these ports as stale.

### Miyoo A30 (`my282`)

Has a Mali-400 GPU, so it reuses the existing GLES renderer.

- ✅ armhf build, pak, launch script, input map (best guess)
- 🚧 **Verify on hardware** — untested; needs a tester's `slot.log`
- 📋 Confirm/correct the button map from `input-trace.log`
- 📋 Bundle armhf emulator cores
- 💡 Shader tweaks if Mali-400's older GLES2 misbehaves

### Miyoo Mini Plus (`miyoomini`)

No GPU, so it needs a software renderer.

- ✅ armhf build, pak, launch script, hardware/framebuffer probe
- 🚧 **Software compositor** — CPU rasteriser that consumes the same `Draw`
  list as the GLES pipeline and blits to `/dev/fb0` (RGB565)
- 📋 Wire the renderer into the frontend and verify the shelf draws
- 📋 Bundle armhf emulator cores
- 📋 Confirm the button map from `input-trace.log`

---

## Wanted — needs discussion

Ideas that would be nice, but aren't committed. Open an issue to push one
forward or volunteer.

- 💡 **More MinUI devices** — e.g. Anbernic RG35XX/RG35XX Plus, TrimUI Smart
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

- ❌ **Non-MinUI installs** (OnionOS, stock Miyoo firmware, ArkOS…). Slot is a
  MinUI Tool; other launchers need their own integration.
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
| `device: a30` / `device: mini plus` | Specific to an experimental device |
| `porting` | Adding/adapting a device |
| `wontfix` | Out of scope (see *Not planned*) |
