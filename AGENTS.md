# AGENTS.md

Working notes for anyone (human or AI agent) changing this repository. It
covers what is *specific to this fork*: how it differs from upstream, how to
sync with upstream safely, how to verify changes on real hardware, how to ship a
release, and the mistakes that have already cost time. For the basics of
contributing, building and review, read [CONTRIBUTING.md](CONTRIBUTING.md)
first; this file does not repeat it.

---

## 1. What this repo is

`slot-flip` is a fork of [slot](https://slot-cfw.fyi) (by Brandon T. Kowalski),
a cartridge-shelf frontend for GBA / GB / GBC. Upstream targets the Anbernic RG
SP. This fork ports it to the **Miyoo Flip (`my355`)**, installed as a **MinUI
Tool**, and carries experimental ports to other Miyoo devices.

| Branch | Role | Releases |
| --- | --- | --- |
| `flip` (default) | Stable Miyoo Flip build | `v*` tags, normal releases |
| `experimental` | Unverified device ports (`my282` A30, `miyoomini` Mini Plus) | `exp-v*` tags, **pre-releases** |

`experimental` is always `flip` plus port work. Stable must never regress
because of experimental work, so ports are not merged into `flip` until they are
verified on real hardware.

The project is GPL-3.0-or-later (same as upstream). Keep upstream credit intact.

### Version map

Fork versions are independent of upstream's. Record the upstream base every time
you release, and keep this table current.

Keep the fork's own `v0.x` numbers rather than adopting upstream's. The tag says
which fork release this is; the **release title and notes lead with the upstream
base** ("upstream slot 1.7.0"), which is what owners need when comparing against
upstream docs. Matching upstream's numbers would imply the same behaviour and a
one-for-one patch cadence the fork does not have.

| Fork release | Upstream base |
| --- | --- |
| `v0.1.0` | v1.4.0 |
| `v0.2.0` | v1.6.0 (+ gpSP JIT fix cherry-picked from v1.7.0) |
| `v0.3.0` | v1.6.0 (unchanged; adds the in-app Buttons remap) |
| `v0.4.0` | v1.7.0 (skeuomorphic redesign; Buttons rebuilt as menu pages) |
| `v0.5.0` | v1.8.0 (Controls; the remap moved to Controls > Key Map; rebuilt mGBA core) |
| `v0.6.0` | v1.8.0 (fork work only: About names the device, update word, forgiving cart labels) |
| `v0.7.0` | v1.8.0 (fork work only: PlayStation discs, console-style slot, PS1 pad, PCSX-ReARMed bundled) |
| `v0.8.0` | v1.8.0 (fork work only: NES games on a drawn Game Pak, fceumm bundled) |
| `v0.9.0` | v1.8.0 (fork work only: Super Nintendo on a drawn cart, snes9x2005 bundled, the systems table; the download is free-only) |
| `v0.9.1` | v1.8.0 (fork work only: a slot drawn as each console, Lid Power Off gains Never) |
| `v0.10.0` | v1.8.0 (fork work only: Sega Mega Drive / Genesis, Master System and Game Gear on Genesis Plus GX) |
| `v0.10.1` | v1.8.0 (fork work only: label scans keep their shape on the NES, SNES and Sega carts) |
| `v0.11.0` | v1.8.0 (fork work only: PlayStation memory cards are saved, discs can be changed, analog sticks as a DualShock behind a setting) |
| `v0.12.0` | v1.9.0 (upstream sync: screenshots and undo save/load on the shared pages, durable writes, core timeouts, label-cache stall; the stick clicks are L3/R3 on PlayStation; otherwise fork work unchanged) |
| `exp-v0.3.0` | v1.6.0 |

---

## 2. Ground rules for agents

- **An agent works for a human.** Per CONTRIBUTING, do not open unsolicited PRs
  on someone's behalf; make the change on a branch the human owns and let them
  decide. Open an issue with your reasoning if you are unsure.
- **Never claim something is tested when it is not.** Say exactly what ran
  (unit tests, a CI job, a real device) and what could not be exercised. A
  clean compile is not a working feature, and an emulator-less host run says
  nothing about the Mali GPU, ALSA, or the input nodes.
- **Measure, do not guess.** For performance or flakiness use the trace tooling
  in section 7 and compare against a known-good binary. One sample is noise.
- **Check the current branch before every commit** (`git branch --show-current`).
  Work has landed on the wrong branch more than once while moving changes
  between `flip` and `experimental`.
- **Stage explicit paths, then read `git diff --cached --stat`.** `git add -A`
  once committed ~290 MB of scratch build directories to a public branch and
  forced a history rewrite. `/target-*/` is ignored now; do not rely on that.
- **Do not rewrite or force-push protected branches** unless a human asked and
  you use `--force-with-lease=<branch>:<expected-sha>`. Prefer a follow-up
  commit. Cancel an in-flight release and delete its tag before fixing a bad
  tag, never re-point a published tag.
- **Keep personal and environment-specific details out of the repo**: home
  paths, device serials, account names, local backup locations, one person's ROM
  list or SD layout. Use placeholders (`<SD CARD>`, `<binary>`).
- **No ROMs, BIOS files or copyrighted cartridge art**, ever, in code, tests,
  docs or screenshots.
- Comments explain *why*, not *what*. Match surrounding style. Run `cargo fmt`.
- **Accommodate other devices where you reasonably can.** Do not make shared
  code Flip-only by accident, and do not break the experimental targets. See
  section 13.

---

## 3. Repo map

```
crates/
  slot/          the app: state machine (app.rs), frontend (frontend.rs),
                 device entry point (device_app.rs), host entry point
                 (host_app.rs), emulator thread (emu.rs), audio, input,
                 wallpaper, label cache, crash handler
  slot-gfx/      GL pipeline, compositor, surfaces (sdl.rs = Flip, fbdev.rs =
                 upstream default, miyoofb.rs = Mini Plus probe)
  slot-ui/       drawing: shelf, carts, quick menu, HUD, text, toasts
  slot-store/    persistence and config: slot.state, ini, cart scan, cores
  slot-power/    battery, backlight, LEDs, lid, rumble, read-only detection
  slot-input/    gesture detection and actions
  slot-retro/    libretro host and core loading
  slot-sfxcut/   dev tool for cutting sound effects (not shipped)
cores/           patches + build.sh for mGBA, gpSP and PCSX-ReARMed (see section 9)
flip/Tools/<platform>/Slot.pak/   the shippable pak: launch.sh + System/ cores
                 (platform = my355 | my282 | miyoomini)
card/, taskfile.yml, site/, CNAME   inherited from upstream (RG SP workflow,
                 upstream's website). Not part of the Flip release. Do not
                 publish the site or reuse the CNAME.
vendor/          locally built cores and source tarballs; git-ignored
```

Device coupling is deliberately small and feature-gated: Cargo features `flip`,
`my282`, `miyoomini` (and upstream's `device`). See [PORTING.md](PORTING.md) for
the seams. Anything Flip-specific must stay behind `cfg(feature = "flip")` (or
`cfg!`), so other targets are not affected.

Runtime layout on the SD card (`SLOT_ROOT`, normally `<SD>/Slot`):

```
Games/ Saves/ States/ Labels/ Wallpapers/ BIOS/   user content
Config/    slot.ini, buttons.ini (fork), slot.state, selected_core.txt, ...
System/    emulator cores, Cache/Labels (generated)
slot.log   current run's log;  crash/  kept crash logs;  input-trace.log (DEBUG)
```

MinUI exports the card as `/mnt/SDCARD` (a **symlink** to `/mnt/sdcard`).

---

## 4. How the fork differs from upstream

Everything here is fork-owned behaviour that a careless upstream merge can
silently delete. Section 5 turns this into a post-merge checklist.

| Area | What the fork does | Where |
| --- | --- | --- |
| Display | SDL2 + GLES2 over KMSDRM instead of fbdev/EGL. Loaded at runtime via `libloading`. | `slot-gfx/src/sdl.rs`, `device_app.rs` |
| Game texture | Full `TexImage2D` per frame plus BGRA to RGBA swizzle. The Flip's Mali driver ignores `TexSubImage2D` updates, so the picture freezes. Do not "optimise" this back. | `slot-gfx/src/pipeline.rs` |
| Emulator pacing | On the Flip the emulator **free-runs at ~60 fps** and the display shows the latest frame, instead of being locked to the display. The panel refreshes below 60 Hz, so locking starves audio and slows the game. | `slot/src/emu.rs` (`let lock = !cfg!(feature = "flip") && ...`) |
| Audio | Under `flip`, negotiates a rate the codec locks onto (48 kHz, then 44.1 kHz, then the core's) and runs the ring at the rate actually opened. Merged with upstream's release-after-silence and retry logic. | `slot/src/audio/alsa.rs` (`preferred_rates`, `open_pcm` returns `(pcm, rate)`) |
| Input | Face buttons from the gpio node; the gamepad node is used for the analog stick only (reading both double-fires). A/B and X/Y swapped to match printed labels. R2 stands in for R1 (some units have a dead R1). Right stick = fast-forward. | `slot/src/input/evdev.rs` |
| Key Map | `Config/buttons.ini` overrides the built-in *kernel key code* map without a rebuild. Upstream's **Controls** pages set what a logical button does; the fork's **Controls > Key Map** (labelled "Key Map", `QuickRow::Buttons`) sets which physical key is that button: pages D-pad / Face / Shoulders / Start & Select, A on a row waits for a key, the two swap if it was taken, then it saves. Names are prefixed `keymap_` in `App` because upstream's own capture uses `capture` and `binding_faces`. The map is a live `Mutex`, and `to_raw` offers each key to a one-shot capture first. Power, volume and lid keys are never captured. | `input/keymap.rs`, `buttons_editor.rs`, `slot-ui/src/quick_menu.rs` (`bind_at`, `KEY_MAP*`), `QuickMenu::bindings` |
| Shaders | Default **Off** for GBA and GB/GBC under the `flip`, `my282` and `miyoomini` features (they all enable `slot-store/flip`). Other shaders are still selectable. | `slot-store/src/slot_state.rs` |
| 4:3 fill and bezel | Upstream stretches 4:3 panels and draws an SP bezel. The Flip has its own bezel and the shelf was tuned at 3:2, so `upload_bezel` is a no-op under `flip`. | `slot/src/frontend.rs` |
| Power | Both LEDs (green running/charged, red low battery, both charging). Lid close sleeps immediately via the hall sensor. | `slot-power/src/device.rs`, `launch.sh` |
| Lid power-off | Upstream's **General > Lid Power Off** (`lid_off_min`, 1/3/5/10/15/30). The fork's old Sleep Timeout row is gone; a saved `sleep_timeout_min` is read once into `lid_off_min` (3 stays unset, 0 "Off" becomes 30) and never written back. | `slot-store/src/slot_state.rs` (`legacy_lid_off`) |
| Card repair | Upstream's SELECT + START repair is **off on the Flip** (`repair_possible`): its script ships in upstream's own device bundle and unmounts the card, which belongs to MinUI here. The red alarm still shows. | `slot/src/app.rs` |
| Wallpaper | `background = <file>` in `Config/slot.ini` pins one image; `random` (default) uses upstream's shuffled deck. | `slot/src/wallpaper.rs` |
| Read-only card alarm | Resolves symlinks before comparing against `/proc/mounts`; otherwise `/mnt/SDCARD` falls through to the read-only squashfs `/` and alarms on every boot. | `slot-power/src/device.rs` (`read_only`) |
| Paths | Data lives in `<SD>/Slot`, never MinUI's `Roms`/`Saves`. | `launch.sh` |
| Cart labels | A PNG in `Labels/<platform>/` **or** flat in `Labels/` pairs with a cart by name: the exact stem first, then case-insensitive, with region/revision tags in brackets and punctuation ignored (`tidy`). The platform folder wins, and a name that only looks similar never pairs. People add these by hand on a computer, so this lookup is deliberately forgiving; do not make it a fuzzy search. PNG only. | `slot-store/src/scan.rs` (`Labels`, `tidy`) |
| About + update word | The About sticker is repainted for this device: `MODEL NO. MY355`, the wordmark `MIYOO FLIP`, and the repo URL where upstream's "SEE README" points at a file the pak does not ship. It prints `VERSION <fork> (SLOT <upstream>)`, so `build_info::UPSTREAM` must be bumped with every sync (section 5). The first boot of a build newer than the card's `Config/slot.state` `seen_version` shows `Updated to <version>` once; a card with no `seen_version` (new install, or from before the key) is marked silently, which is also why tests that boot a card with a state file still see no banner. | `slot/src/build_info.rs`, `slot/src/app.rs` (`note_build`), `slot-ui/src/sticker.rs` |
| Frame size | Frames carry their own `(w, h)` instead of assuming the 240x160 GBA canvas, so a bigger picture is not cropped. `LibretroCore::set_native_video` is **off by default** (the Game Boy family is still centred in the canvas, byte-identical; a real-core test proves a GBA frame is the same in both modes) and on for PlayStation. The size travels `video_size` -> `Frames::publish_sized` -> `upload_game_sized`, and `GamePass` re-creates its texture when it changes. | `slot-retro/src/libretro.rs` (`copy_native`), `slot/src/frames.rs`, `slot/src/emu.rs` (`publish`), `slot-gfx/src/pipeline.rs` (`upload_sized`) |
| Path-only cores | `load()` asks the core (`retro_get_system_info`) whether it needs a path and, if so, does not read the file into RAM (a disc image is hundreds of MB). | `slot-retro/src/libretro.rs` (`content_bytes`) |
| Super Nintendo (v0.9.0) | `Platform::Snes` (`Games/SNES`: `.smc .sfc .swc .fig`) runs on `Core::Snes9x` = snes9x2005_plus (non-commercial, so the whole download is free-only; see section 9). Pad: `Spec.native_pad` with `SNES_EXTRAS` (X/Y; L/R are the GBA shoulders), so turbo is off; `input::table::on_native` only moves the shortcuts that sit on the pad's own buttons, so rewind and fast-forward stay on L2/R2. Cart: `slot-ui/src/pak.rs` draws both the NES Game Pak and the SNES cart from a `Design` (a new cartridge is a set of numbers); `GbShell::Pak(Pak)`. `slot-retro/tests/snes9x.rs` runs a hand-assembled 65816 program that paints the pad into the backdrop, so each of the 12 buttons is proven on the real core. The core accepts any junk file as a game, so there is no "not a SNES game" error. | `slot-store/src/{platform,core}.rs`, `slot/src/{core,input/mask,input/table}.rs`, `slot-ui/src/pak.rs` |
| Sega systems (v0.10.0) | `Platform::{Md, Sms, Gg}` (`Games/MD` `.md .gen .smd .bin`, `Games/SMS` `.sms`, `Games/GG` `.gg`) all run on `Core::Genesis` = Genesis Plus GX (`cores/genesis_plus_gx`, pinned commit, no patches; the core is ~12 MB stripped, about 6 MB of code and 6.5 MB of tables). It is non-commercial like snes9x2005, so `free-only` in `tools/check-pak-licences.sh`; its licence file ships as `genesis_plus_gx-LICENSE.txt` and the source as `genesis_plus_gx-src.tar.gz`, trimmed of what the libretro build never reads (`cores/genesis_plus_gx/build.sh trim`; the omitted directories are recorded in the `.meta`). The core picks a 3- or 6-button pad per game from the ROM header, so slot sets no option. Pad: the Mega Drive is `native_pad` with the SNES extras (libretro Y = MD A, B = B, A = C, X = Y, L = X, R = Z, SELECT = Mode); SMS and GG are `Pad::Compact` with turbo, as the NES. Carts: three `Design`s in `pak.rs` (`Pak::{Md, Sms, Gg}`), black plastic. Slots: `Console::{MegaDrive, MasterSystem, GameGear}` (black, silver / red band, ridges). `slot-retro/tests/genesis_plus_gx.rs` runs three hand-assembled programs (68000 x2, Z80 x2) on the real core, each painting the pad into colour 0 so every button is proven, including the six-button sequence and the SMS Pause NMI. The Mega Drive programs read the pad once per frame on purpose: the 6-button counter only resets after ~26 idle scanlines, so a program that polls repeatedly in vblank sees a 3-button pad. | `slot-store/src/{platform,core}.rs`, `slot/src/{core,input/mask}.rs`, `slot-ui/src/{pak,console_slot,cart,shell}.rs`, `cores/genesis_plus_gx/` |
| Label shape on drawn carts (v0.10.1, issue #30) | On the NES, SNES and Sega carts (`slot-ui/src/pak.rs`) a label scan keeps its own proportions instead of being centre-cropped to the generic recess: `art::label_cover` fits art whose width/height is within `LABEL_ASPECT` (1.0 to 2.4) as large as the recess allows, anything else (box art, banners) is still cropped to fill; `Design::face` shrinks and centres the recess to the art's size (`with_label_size`) and paints transparent label pixels over the plastic. Art travels as `LabelArt {rgba, w, h}` (`cart::label_for`); GBA, GB/GBC and discs are unchanged (their art is always the label's own size). The label cache header gained a version word and the art's own size (`HEAD` 32, `VERSION` 2), and an entry is only taken if it fits the label's room, so caches built the old way are rebuilt once. Bump `VERSION` whenever how a label is fitted changes. | `slot-ui/src/{art,cart,pak}.rs`, `slot/src/label_cache.rs` |
| Console slots (v0.9.1) | Each non-GBA system's slot is drawn as its console (`slot-ui/src/console_slot.rs`, issue #27): PlayStation (round lid, unchanged pixel for pixel; a test pins its hashes), NES (stripe, red light), Super Nintendo (ridges, purple RESET/EJECT), Game Boy and Game Boy Color (purple). `Console::of(platform)`; GBA is `None` and keeps upstream's traced strips. A console is a `Style` (palette per theme, `Opening` shape, dress, closed lid or groove); the opening keeps the cart slot's width, depth and position so games clip the same. `App::console_wanted` picks it from the seated game or the shelf in view; the frontend draws each console's strips on first need. To add a system's slot: a `Console` variant, a palette and `dress_*`, and `Console::of`. | `slot-ui/src/console_slot.rs`, `slot/src/{app,frontend}.rs` |
| NES (v0.8.0) | `Platform::Nes` (`Games/NES`: `.nes .fds .unf .unif`) runs on `Core::Fceumm` (GPL-2.0, `cores/fceumm`, bundled with its source and the LGPL-2.1 licence of its NTSC filter). The Game Pak is drawn (`slot-ui/src/pak.rs`): portrait, no spin, no glare. `Platform::native_video()` (PS1, NES) and `Platform::has_screen_options()` (the Game Boy family only) replace the PS1-only checks, so a new console system only has to answer those two. Turbo works as on a GBA (X/Y become pulsing A/B; no `native_pad`). `slot-retro/tests/fceumm.rs` drives a synthetic NES program that paints the controller port, so each button is proven on the real core. fceumm declares its options only after a game loads; slot sets none on it. | `slot-store/src/{platform,core}.rs`, `slot-ui/src/{nes_cart,cart,shelf,silhouette}.rs`, `slot/src/core.rs` |
| PlayStation (v0.7.0) | `Platform::Ps1` (`Games/PS1`: `.cue .chd .m3u .pbp`) runs on `Core::Pcsx` (bundled). A multi-disc game is shelved once, as its `.m3u`. No bezel, shader or colour correction, and the in-game menu hides those rows plus Turbo. The cart is a **disc**, drawn (`slot-ui/src/disc.rs`), not a bitmap: it spins one turn into the slot (`Draw::Turned`), the glare is a separate non-rotating layer, cover art is fitted to the circle with a jewel-case spine trimmed (`art::disc_cover`) and shows through a frosted hub. The slot is drawn as a console (`slot-ui/src/console_slot.rs`, built on first use by `ensure_console_slot`, chosen by `App::console_wanted`). **Pad:** `Pad::mask_for(.., Platform::Ps1)` adds X/Y (Triangle/Square) and L2/R2, `EmuHandle::set_native_pad` skips the turbo transform, and `Table::for_platform` moves fast-forward to SELECT+R2 and drops rewind and the palette keys (plain L2/R2/X/Y are game buttons). Mind the Flip's input quirks (`input/evdev.rs`): the physical right trigger arrives as `Btn::R1` and `Btn::R2` is the right stick pushed right, so PS R1 is the trigger and PS R2 is the stick. **Saves:** `apply_core_options` sets `pcsx_rearmed_memcard1=libretro` (card 2 `none`); left unset, slot answers "no value" for the option, PCSX logs `non memcard%d config?` and exposes no save RAM, so nothing ever reached `Saves/PS1`. The card is the core's save RAM (128 KiB) and travels the normal `.sav` path. **Discs:** `slot-retro` implements the libretro disk-control interface (`SET_DISK_CONTROL_INTERFACE` and the `_EXT` one; the older struct is only its first seven members). `LibretroCore::change_disc` opens the lid, `advance_swap` (called each frame) switches the disc at frame 45 and closes the lid at frame 90 (`SWAP_SWITCH_FRAME`, `SWAP_CLOSE_FRAME`), so the game polls an open lid; it only progresses while the core runs, so a change chosen in the (paused) menu finishes after resuming. `EmuHandle::{disc_count, disc_index, disc_changing, change_disc}` publish through atomics (a request stores its target at once, so the menu does not flicker back); `QuickRow::Disc` sits in the in-game PLAY page and is hidden under two discs; `Session::update` ferries `App::take_disc_request`. PCSX stores the disc index in its states and restores it on load. **Analog:** `Settings > Gameplay > Analog Sticks` (`analog_sticks`, default off). On, `Session::sync_pad` plugs a DualShock into port 1 (`RETRO_DEVICE_PSE_DUALSHOCK` = 517, re-sent after every load because PCSX resets its ports then) and passes `input::sticks::read()`; the gamepad node's axes are LX 0, LY 1, RX 3, RY 4 (+-32767) and 2/5 are the analog triggers (0..255); a circular dead zone of 7000 hides the rest offset (right stick at rest read -5096 / 1404). While sticks are the game's, the right stick no longer stands in for R2 (`sticks::taken`), so a unit with a dead R1 has no R2 and no SELECT+R2 fast-forward then. L3 and R3 are the two sticks pushed in: the gamepad node is the only one that reports them, so `device::click` reads `BTN_THUMBL`/`BTN_THUMBR` (0x13d/0x13e; 0x13a/0x13b/0x13c are SELECT/START/MENU, which the gamepad node repeats and which must stay unmapped there: a second button with MENU held cancels the tap and the eject hold) there, and `Pad::clicks` adds `ButtonMask::L3`/`L3` for `Platform::Ps1` only. They are fixed, not rebindable: no `Device` variant, so no extra Controls rows. A DualShock reports them; in digital mode PCSX ignores them. **PAL:** `emu::frame_period` paces a core that really runs at ~50 Hz at its own rate; 59.73 (GBA) and 60 are left exactly as they were. `Core::ALL` is the picker's two cores; `Core::EVERY` is all three. | `slot-store/src/{platform,core,scan}.rs`, `slot/src/core.rs`, `slot-ui/src/{cart,disc,console_slot,art}.rs`, `slot/src/input/{mask,table}.rs`, `slot/src/emu.rs` |
| Core delivery | `launch.sh` refreshes `Slot/System` cores whenever the pak's differ (`cmp -s`), so core fixes reach existing installs. | `Slot.pak/launch.sh` |
| GBA core | gpSP by default (ARM64 dynarec); mGBA has none. Per-game override in `selected_core.txt`. | `slot-store/src/core.rs` |
| Crash handler | Adds a 32-bit ARM register reader (upstream covers aarch64 and x86_64 only). | `slot/src/crash.rs` (experimental) |

Quick-menu structure: upstream owns the main list (General, Screen, Gameplay,
Controls, About). The fork adds one page, **Key Map**, as the last entry under
**Controls**. Tests assert row order and counts (section 6); some upstream tests
walk a page with Up/Down, so adding a row to an upstream page can break them.

---

## 5. Syncing with upstream

### Automated first pass

`.github/workflows/upstream-sync.yml` runs every Monday (and on demand). It
fetches the **next** `v1.*` release, merges exactly one release into
`chore/upstream-sync-X.Y.Z`, and either pushes the branch or aborts on conflict.
It then opens an issue: conflicts to resolve, or a branch ready to review. It
does **not** open the PR, because a PR made with `GITHUB_TOKEN` does not trigger
`pr-checks`, and a clean text merge can still be wrong. A human resolves, bumps
`build_info::UPSTREAM`, follows the checklist below and opens the PR.

### Setup

```sh
git remote add upstream https://github.com/BrandonKowalski/slot.git
git fetch upstream main
git fetch upstream 'refs/tags/v1.*:refs/tags/v1.*'
```

Fetch only `v1.*`. Upstream's early `v0.*` tags collide with this fork's own
`v0.*` tags and git will refuse to overwrite them.

### History is grafted

This repo's history was rewritten at some point and originally shared no
ancestry with upstream. A no-op `-s ours` merge of upstream `v1.4.0` (tree
unchanged) restored a real merge base. Consequences:

- `git merge vX.Y.Z` now works normally; the base is the last synced upstream tag.
- **Merge PRs with a merge commit, never squash or rebase.** Squashing discards
  the upstream parent and the next sync is back to unrelated histories.
- Sync **one upstream release at a time** (1.6, then 1.7). Cherry-picking a lone
  fix from a later release is fine (the gpSP JIT patch was taken this way).

### Procedure

```sh
git checkout -b sync/vX.Y flip
git merge vX.Y.Z --no-commit
git diff --name-only --diff-filter=U        # what conflicts
```

For **every** conflicted file, before resolving:

```sh
git diff <previous-upstream-tag> flip -- <file>   # what did the fork change here?
```

**Never resolve with `git checkout <upstream-tag> -- <file>` without that diff.**
That shortcut once discarded the fork's one change to `emu.rs` (the free-run
pacing) because the textual conflict was only a log line. The regression
looked like "shaders are slow" (game speed dropped to ~70%) and took
measurements to find.

Usual resolutions:

- `README.md`: keep the fork's (`git checkout --ours README.md`). Upstream
  rewrites its README every release. Pull in only facts that matter.
- `CHANGELOG.md`: take upstream's. The file tracks upstream versions; fork
  release notes live on the GitHub release (section 8).
- `.gitignore`: union.
- Tests that encode old behaviour (menu rows, wallpaper selection): update them
  to the new intended behaviour, do not delete them.
- Files upstream renames or moves (v1.6 renamed per-game configs `.ini` to
  `.txt` with an automatic migration): update README, INSTALL and PORTING.

### Textual-clean is not correct

A clean merge can still be broken. Build **every** feature set, including the
real 32-bit release builds on `experimental`:

```sh
cargo check -p slot --no-default-features --features flip
# on experimental, also:
cargo build --release -p slot --no-default-features --features my282      --target armv7-unknown-linux-gnueabihf
cargo build --release -p slot --no-default-features --features miyoomini  --target armv7-unknown-linux-gnueabihf
```

Real breakages found this way: a function split on `experimental` no longer saw
a variable upstream added upstream of the split, and a new upstream crash
handler had no 32-bit ARM support.

### Post-merge checklist

Confirm each fork behaviour from section 4 survived (and look at the diff, not
just the grep):

```sh
grep -n 'cfg!(feature = "flip")' crates/slot/src/emu.rs          # free-run lock
grep -n 'preferred_rates' crates/slot/src/audio/alsa.rs           # rate negotiation
grep -n 'DEFAULT_SHADER' crates/slot-store/src/slot_state.rs      # shaders Off
grep -n 'upload_bezel' crates/slot/src/frontend.rs                # no-op under flip
grep -n 'BACKGROUND_KEY' crates/slot/src/wallpaper.rs             # pinned background
grep -n 'SleepTimeout\|Buttons' crates/slot-ui/src/quick_menu.rs # fork menu rows
grep -n 'keymap::load' crates/slot/src/device_app.rs              # remap loaded before input
grep -n 'capture_offer' crates/slot/src/input/evdev.rs            # Buttons screen key capture
grep -n 'canonicalize' crates/slot-power/src/device.rs            # read-only check
grep -n 'SdlSurface\|mod sdl' crates/slot-gfx/src/lib.rs          # Flip surface exported
grep -n 'cmp -s' flip/Tools/my355/Slot.pak/launch.sh              # core refresh
grep -n 'UPSTREAM' crates/slot/src/build_info.rs                  # About says which upstream
grep -n 'fn copy_native' crates/slot-retro/src/libretro.rs        # native-size frames
grep -n 'publish_sized' crates/slot/src/emu.rs                    # frames carry their size
grep -n 'fn upload_sized' crates/slot-gfx/src/pipeline.rs         # texture follows the frame
grep -n 'fn content_bytes' crates/slot-retro/src/libretro.rs      # path-only cores
```

**Bump `build_info::UPSTREAM`** in the merge commit that takes the upstream
tag. It is what the About sticker prints, and nothing else records it.

**Cores are part of every sync.** Upstream adds patches to `cores/*/` without
saying so in the changelog, and the frontend can depend on them (v1.8's Palette
Auto needs its mGBA patch). Check the bundle against the patches:

```sh
git diff <previous-upstream-tag> <new-upstream-tag> --stat -- cores taskfile.yml
for p in cores/mgba/*.patch;  do b=$(basename $p); [ -f flip/Tools/my355/Slot.pak/System/licenses/mgba-$b ]  || echo "MISSING $b"; done
for p in cores/gpsp/*.patch;  do b=$(basename $p); [ -f flip/Tools/my355/Slot.pak/System/licenses/gpsp-$b ]  || echo "MISSING $b"; done
```

If a patch or pinned commit changed, rebuild the core (section 9) before
releasing. The `core_options` test loads `vendor/mgba_libretro.so` and fails when
that core is older than the frontend, so do not skip it. v0.4.0 shipped an mGBA
core that was missing one of v1.7's patches because this step was skipped.

**Also look for upstream device assumptions.** Scripts that ship only in
upstream's `card/` bundle (`repair.sh`, `lid.sh`) never reach the Flip pak, and
code that calls them or hard-codes another device's hardware (`rmmod 8821cs`)
needs a decision, not a blind merge.

Then run the full tests (section 6), then verify on a device (section 7). Open a
PR from the sync branch so `pr-checks` runs on the merged tree.

---

## 6. Building and testing

CI (`.github/workflows/pr.yml`, job `pr-checks`) runs on every PR to `flip` and
`experimental` and is a required check. It runs in `rust:1-bookworm` as root,
after `apt-get install libudev-dev libasound2-dev pkg-config`, and does:

1. `cargo check -p slot --no-default-features --features <f>` for each feature
   that exists on the branch (it detects them from `crates/slot/Cargo.toml`, so
   one workflow serves both branches).
2. Library tests for `slot slot-store slot-power slot-input slot-ui slot-retro
   slot-gfx`, then integration tests for `slot-store slot-power slot-ui`, then
   `cargo test -p slot --test evdev --test input --test platform`.

Reproduce locally with the same container; do not assume host behaviour matches.

Keep local test builds lean. Run them with `CARGO_PROFILE_DEV_DEBUG=line-tables-only`
(and one `CARGO_TARGET_DIR`, not several): a debug build of `target-test` had grown
to 26 GB, and `target` to 16 GB. Measured on `slot-ui`'s tests, that setting built in
19 s instead of 31 s into 980 MB instead of 1,692 MB, and panics and backtraces still
give the exact file, line and column. All it loses is stepping through code in a
debugger and inspecting locals; set it back to `2` for a single run if a bug needs
that. It does not touch release builds, so the Flip binary is unchanged. Changing the
setting leaves the old artefacts beside the new ones, so delete the old target
directory once when you switch.

Conventions that have proven useful:

- **A menu or settings change needs its tests updated.** Row order, counts and
  labels are asserted in `slot-ui/tests/quick_menu.rs`,
  `slot/tests/quick_menu.rs` and `slot/tests/render_quick_menu.rs`. They fail
  loudly when a row is added; update them to the new intended layout.
- **Tests for persisted state** live in `slot-store/tests/durability.rs`. Adding
  a `SlotState` field means touching the struct, `Default`, the write format
  string and its arguments, the parser, and the constructor, and extending the
  test fixture. Missing one silently drops the setting.
- **Do not assert on seeded randomness you do not own.** The wallpaper test now
  asserts "every image is dealt once before any repeats", not which file a seed
  picks.
- macOS-gated targets (`flip_smoke` example, `slot-gfx/tests/pipeline.rs`,
  `record`) will not build on Linux; exclude them.
- Hardware-only behaviour (GL driver quirks, ALSA, evdev, LEDs, lid) cannot be
  proven by unit tests. Say so, then use section 7.
- Fix the code or the test deliberately. Never weaken an assertion just to get
  green.

Docker builds for devices (see CONTRIBUTING for full commands): Flip is
aarch64 in `rust:1-bullseye`; the A30 and Mini Plus are **32-bit armhf** and
need `gcc-arm-linux-gnueabihf libc6-dev-armhf-cross` (without the latter, link
fails with `cannot find Scrt1.o`).

---

## 7. Verifying on a device

Use this whenever a change touches rendering, audio, input, pacing, power, or
upstream code.

### Deploy loop (Flip, over adb)

```sh
adb push <binary> <SD>/Tools/my355/Slot.pak/slot
adb shell chmod 755 <SD>/Tools/my355/Slot.pak/slot
adb reboot            # then wait ~75-100 s
adb shell 'pidof slot; tail -30 <SD>/Slot/slot.log'
```

- Back up the working binary first and compare `sha256sum` before and after a
  push so you know exactly what is running.
- This works best when the device auto-boots into Slot (see README, "Boot
  straight into Slot"). Do not try to start Slot by killing MinUI processes;
  reboot instead.
- The shell is BusyBox (`ps -p` is missing; `cmp`, `sha256sum` exist).
- Cores' own warnings and errors reach `slot.log` as `core[N]: ...` (N = 2 warn,
  3 error), each once; the format string is shown unexpanded (`%s`), because
  libretro's callback is variadic. Drop a `CORELOG` file in the pak for their
  routine messages too. This is how "core refused <file>" gets a reason.
- A disc image that will not open is usually the image. Before suspecting the
  core, run `chdman verify -i <file>.chd` on a computer (`brew install
  rom-tools`); it checks the SHA-1 the file carries.
- A clean boot log shows: the SDL video driver (KMSDRM), backlight/battery/rtc,
  audio at 48000 Hz, input nodes, and a first-frame time. A line containing
  `crash:` or `card: alarm` is a failure.

### Measuring performance and pacing

Create an empty file named `DEBUG` next to `launch.sh` in the pak to enable
tracing (it exports `SLOT_TRACE`). Under the `flip` feature the log then prints
every ~2 s:

```
heartbeat: <N> loops, <M> emu frames in 2.0s (...)
```

- **emu frames ~120 per 2 s = the game is running at full 60 fps.** This should
  stay at 120 regardless of shader or display load. If it drops, the emulator is
  being throttled by the display (see the pacing row in section 4).
- **loops** is the display refresh actually achieved. The Flip panel is below
  60 Hz, so roughly 85-95 loops per 2 s (about 45 Hz) is normal; shaders lower it
  a little, which is expected and does not affect game speed.
- Compare against a known-good binary on the **same game and settings**, and
  take several heartbeats. Treat single runs as noise.
- Remove `DEBUG` afterwards.

### Booting straight into a game (no input needed)

Slot resumes the cart named in `Config/slot.state`. Set
`cart=<rom name without extension>` and `cart_platform=gba|gb|gbc`, reboot, and
the game starts on its own, so you can script repeated boots for benchmarking or
to hunt a flaky crash. **Restore both keys to empty afterwards**, and clear any
shader or test settings you changed.

### Limits

- `/dev/fb0` reads black under KMSDRM, so there is no easy screenshot. Visual
  checks need a human looking at the screen; ask for a short, specific
  checklist rather than "does it look right".
- Intermittent crashes (a rare gpSP boot crash needed ~1 in 10 boots) cannot be
  proven fixed by a handful of clean runs. Prefer structural evidence (inspect
  the binary, read the patch) and report the sample size honestly.
- Test the **release artifact**, not just your local build: install the CI zip
  over the existing pak the way a user would, then boot.

---

## 8. Releases and patch notes

### Stable (`flip`)

1. Merge the work to `flip` through a PR with `pr-checks` green (merge commit).
2. Verify on a device (section 7), including a game with sound and a lid
   close/open.
3. Tag and push (annotated, with `-m`, so no editor opens):
   `git tag -a v0.N.0 -m "slot-flip v0.N.0" && git push origin v0.N.0`.
4. `release.yml` builds the aarch64 binary, packages `Slot.pak-my355.zip` and
   publishes the release with **generic static notes**.
5. **Replace the notes** with real patch notes (below):
   `gh release edit v0.N.0 --notes-file notes.md`.
6. Confirm the release is **Latest**, the zip contains what you expect (core
   hash, `launch.sh`, licence files), then install it on a device and boot.
7. Update the version map in section 1 and ROADMAP.md.

### Experimental (`experimental`)

Tag `exp-v0.N.0` on `experimental`. `experimental.yml` (this workflow exists
**only on that branch**) builds `Slot.pak-my282.zip` and
`Slot.pak-miyoomini.zip` and publishes a **pre-release**. `workflow_dispatch`
for it 404s because the file is not on the default branch; trigger it with the
tag. Check afterwards that the stable release is still marked Latest. These
builds are armhf and cannot run on a Flip, so say plainly what is unverified and
point testers to TESTERS.md.

If a tag was cut from a bad commit: cancel the run, delete the remote tag and
any release, fix, then tag again.

### Writing patch notes

Notes are for people who own the device, not for developers.

- **Lead with the upstream base** ("This release brings the fork up to upstream
  slot v1.6.0 (from v1.4.0)"), then group:
  - *From upstream* (carry upstream's wording, keep contributor thanks),
  - *Fork-specific*,
  - *Fixed*.
- **Only list what you verified**, in code or on a device. Re-read each bullet
  against the diff. State defaults precisely (for example "shaders default to
  Off on the Flip").
- **Call out migrations and one-time costs under "Upgrading"**: renamed config
  files, caches that rebuild on first boot, settings that move or reset, and
  what is untouched ("your `Slot` folder is not modified").
- Put anything that could surprise a player (behaviour change, removed option,
  new default) near the top, not buried.
- Plain language, short bullets, no commit-subject dumps, no internal jargon
  (say "the game now runs at full speed with shaders on", not "removed the
  display lock").
- Keep the standard footer: install steps, licences (mGBA MPL-2.0, gpSP
  GPL-2.0, source and patches in `System/licenses/`), fork/affiliation notice,
  "use your own legally dumped ROMs".
- `CHANGELOG.md` is upstream's changelog; do not hand-edit it. Fork notes live
  on the GitHub release.

---

## 9. Emulator cores

- Pinned commits live in `taskfile.yml` (`MGBA_COMMIT`, `GPSP_COMMIT`, `PCSX_COMMIT`).
- `cores/<core>/build.sh` applies **every `*.patch` in that folder** to the
  pinned source and writes a `.meta` stamp listing commit, recipe and patch
  hashes. Adding a patch is just dropping the file in.
- Built cores go to `vendor/` (ignored). The **stripped** copies in
  `flip/Tools/my355/Slot.pak/System/` and the licence bundle are committed so CI
  can package a release without building cores.
- When a core changes, update all of: the stripped `.so` in the pak,
  `System/licenses/<core>_libretro.so.meta`, and a copy of each new patch as
  `System/licenses/<core>-<patch>.patch`. This is GPL source-offer compliance,
  not housekeeping.
- Cores are aarch64 for the Flip. The experimental armhf targets do not bundle
  cores yet.
- `cores/pcsx` builds PCSX-ReARMed (GPL-2.0) with no patches. Build it in the
  device's bullseye box: that gives GLIBC_2.17 (the Flip's own libc is 2.36, so
  the libretro buildbot's prebuilt, which needs 2.34, would also load; building
  it ourselves is for the pinned, patchable, GPL-corresponding source). It is
  bundled in the pak since v0.7.0: the stripped `.so` (~1.8 MB), its `.meta`,
  `pcsx-GPL-2.0.txt` (the source's `COPYING`) and `pcsx-src.tar.gz` (~5.6 MB, the
  pinned source) in `System/licenses/`. That is ~7.5 MB on every install, mostly
  the source archive the GPL asks for; do not drop it without replacing it with a
  written offer. Re-strip with `strip --strip-unneeded` after every rebuild.
- **Licensing policy: free and open source, never for sale.** The owner does not
  plan to sell this, wants it to stay open source, and may offer donations that never
  unlock anything. On that basis non-commercial cores may be bundled in the one
  `Slot.pak`, **knowingly**: snes9x2005 (a Snes9x derivative) is, and its licence
  allows use, copying and distribution "for non-commercial purposes" with the notice
  kept, and excludes "charging money" and "commercial game bundles". So the download
  as a whole is free-only, and `System/licenses/NOTICE-NONCOMMERCIAL.txt` says so
  (and says which files to remove for a build that could be sold). Genesis Plus GX is
  stricter ("Redistributions may not be sold, nor may they be used in a commercial
  product or activity", and its copyright notice must be reproduced); it is bundled
  on the same footing, with its own line in the notice. The fork has no donation link
  of its own: `.github/FUNDING.yml` is inherited from upstream and points at upstream's
  author, so remove it, or keep any future donation link off the download itself, if
  that wording is ever read strictly. The rest ships
  under GPL-3.0-or-later (frontend), MPL-2.0 (mGBA), GPL-2.0 (gpSP, PCSX-ReARMed,
  fceumm), CC-BY-SA-4.0 (bezel), and OFL (fonts). The cores are separate programs
  the frontend loads at run time. This is a practical reading, not legal advice; a
  trademark review (the "slot." name, "Game Boy", "PlayStation") is still open.
  `tools/check-pak-licences.sh` (in `pr.yml` and `release.yml`) enforces it: a core
  not on its allowlist, an allowed core missing its licence text, `.meta` or source
  archive, or a `free-only` core without the notice, fails the build. To clear a new
  core, read its licence first, then add its line (marking it `free-only` if it
  forbids commercial use). History: v0.9.0 first built SNES as a separate add-on pack
  (`Core::is_addon`, an "Add-on pack needed" banner, `tools/check-addon.sh`) to
  keep the option to sell; that was removed in the same release once the owner chose
  to bundle everything. It is in git history (PR #26) if a core ever cannot be
  bundled.
- Because `launch.sh` refreshes cores by comparison, shipping a new core in a
  release is enough for it to reach existing installs.

---

## 10. Git and CI hygiene

- `flip` is both a **branch name and a directory** (`flip/Tools/...`). Bare
  `git log flip` is ambiguous: use `refs/heads/flip` or `flip --`.
- Both branches are protected by a ruleset (PR, code-owner review, `pr-checks`,
  no force-push/delete). The maintainer can bypass; everyone else uses PRs from
  forks. Workflow files for `pull_request` run from the PR's merge commit, so
  a workflow change must exist on the base branch.
- Docs that apply to both branches go to `flip` first, then are cherry-picked to
  `experimental`. Re-check which branch you are on before each step.
- `pr.yml` must keep working on both branches; it skips features that do not
  exist on the branch under test. Do not hard-code experimental features into a
  workflow that also runs on `flip`.
- Do not run CI-style steps that change ownership of the runner's mounts (a
  `chown -R /__w` broke the runner's own cleanup). The tests pass as root.
- Commit style: `type: summary` (`feat`, `fix`, `docs`, `ci`, `refactor`,
  `flip`, `power`, ...). Explain the why in the body, including measurements
  when the change is about performance.
- **Never commit or push secrets, credentials or personal data.** `.githooks/`
  (enabled with `sh tools/install-hooks.sh`, which sets a repo-local
  `core.hooksPath`) refuses private keys, tokens, local home paths, private IPs,
  key-like file names and any pattern in the untracked
  `.git/info/sensitive-patterns` (your own names, emails, paths, serials) on
  commit and push. The `secrets` workflow runs the same check plus gitleaks over
  the full history on every push and PR, and GitHub's secret scanning and push
  protection are enabled. **Never use `--no-verify`** (or `git commit -n`): it
  is a rule, not a preference. If it happens, the `post-commit` guard still
  scans the new commit and undoes it if it holds a finding (during a rebase or
  merge it can only warn), and CI is the backstop. If a real secret is
  committed, **rotate it** — deleting the file does not remove it from history. Deliberate test examples need
  `check-sensitive: allow` on their line. The maintainer's own strings live only
  in `.git/info/sensitive-patterns`, never in the repo; `tools/check-sensitive-test.sh`
  tests the scanner.

---

## 11. Documentation upkeep

Keep these in step with behaviour changes; a feature is not done until the docs
match:

| File | Update when |
| --- | --- |
| `README.md` | features, device support table, install, troubleshooting |
| `INSTALL.md` | install steps or folder layout changes |
| `PORTING.md` | a device seam moves; config file names change |
| `ROADMAP.md` | work completes or is added; link the issue |
| `TESTERS.md` (`experimental`) | the test runbook changes |

Rules of thumb:

- The README keeps the fork's identity and the explicit device-support table
  (stable vs experimental vs unsupported). Do not let an upstream merge replace
  it.
- Test instructions must be copy-pasteable and name the exact files to attach
  (`Slot/slot.log`, `Slot/input-trace.log`).
- New work gets an issue (templates in `.github/ISSUE_TEMPLATE/`); decisions that
  need discussion, such as taking upstream's visual redesign, go in an issue
  before code.
- Verify links and YAML after editing docs and workflows.

---

## 12. Known hardware facts (Miyoo Flip)

Useful when something looks like a software bug but is not:

- Panel is 640x480 (DSI) and refreshes below 60 Hz.
- GPU stack is Mali via KMSDRM; `TexSubImage2D` is unreliable (see section 4).
- The audio codec only locks onto standard rates; the core's native 32768 Hz is
  not accepted directly.
- Inputs: buttons on a gpio-keys node, volume on an adc-keys node (polled slowly
  by default; Slot raises the poll rate), analog sticks on a gamepad node.
- Closing the lid is read from a hall sensor under `/sys/devices/platform/`.
- Two LEDs (work/green, charger/red); the kernel drives the red one from the
  battery trigger unless `launch.sh` turns the trigger off.
- A bad ROM dump produces a black or white screen that looks like a renderer
  bug. Rule out the dump (compare against a known-good copy) before debugging
  the frontend.

Experimental targets: the A30 has a Mali-400 (reuses the GLES path, weak GPU, so
keep shaders off by default); the Mini Plus has **no GPU**, so its renderer is a
CPU compositor that does not exist yet and that build only probes `/dev/fb0`.
Both are 32-bit armhf.

---

## 13. Accommodating other devices

The goal of this repo is to run slot on as many MinUI handhelds as is
realistic. Every change should leave that door open. This is a habit more than a
task: it costs little when done while writing the code and a lot when retrofitted.

### Principles

- **Shared code stays device-neutral.** Put device facts behind a seam (a Cargo
  feature, a probe, or a config file) instead of hard-coding the Flip's values in
  code every device runs. If a fix helps everyone (the 32-bit ARM crash-handler
  registers are an example), do not gate it to one device.
- **Prefer runtime detection and configuration over a new compile feature.**
  Probe for the thing (a sysfs file, an input node's capabilities, an LED path)
  and degrade quietly when it is absent. A missing lid sensor, LED, rumble motor
  or analog stick must never be an error.
- **Configurable beats compiled.** Key codes, paths and per-device quirks that a
  user can fix in a file or in the app need no rebuild and no new release.
  `Config/buttons.ini` is the model; reach for the same shape when a new
  device-specific knob is needed.
- **Do not leak one unit's quirks into other devices.** The Flip's stand-in for a
  dead R1, its free-running emulator pacing and its codec rates are Flip facts
  behind `cfg(feature = "flip")`. A different device gets its own answer or the
  neutral default, not the Flip's.
- **Default to the conservative choice on weak hardware.** Shaders default Off on
  every ported device; a Mali-400 or a CPU-only framebuffer will not carry the
  Flip's extra work. Offer the heavy option, do not impose it.
- **Be honest about what is unverified.** An experimental build that compiles is
  not a working port. Never merge a port into `flip` until a person with the
  hardware has run it and sent a log.

### Before you merge a change, ask of each target

| Question | Why it bites |
| --- | --- |
| Does it build for 32-bit armhf? | `usize` and pointer width, 64-bit atomics, and `libc` struct fields differ (`arm_pc`, not `pc`). A clean x86 or aarch64 build proves nothing. |
| Does it assume a GPU? | The Mini Plus has none; its path is a CPU compositor, not GLES. |
| Does it assume a fast GPU? | The A30's Mali-400 is old GLES2 and slow. Keep per-frame work and shader cost low. |
| Does it assume key codes, paths or a platform slug? | Use `code_to_btn` and the keymap; take `SLOT_ROOT` from the environment (`launch.sh` sets it) and keep each device's paths inside its own pak, not in shared code. |
| Does it assume a lid, LEDs, a stick, rumble or a battery node? | These are optional per device. Absence must be a no-op. |
| Does it assume a screen size or aspect? | The Flip is 640x480 and the shelf was tuned at 3:2. Other panels differ. |
| Does it assume the Flip's audio codec or rates? | Negotiate and fall back; do not hard-code 48 kHz. |

Build every target you touch (section 5 lists the commands) and say in the PR
which ones you ran. If you cannot run something on hardware, say that too.

### Adding a new device

1. **Open an issue first** (device name, SoC, MinUI platform slug, GPU or not).
   Decide whether it reuses the GLES path (cheap, like the A30) or needs a new
   renderer (expensive, like the Mini Plus).
2. **Add the seams, not a fork.** A Cargo feature, `flip/Tools/<platform>/Slot.pak/`
   with `launch.sh`, a button map in `code_to_btn` (or rely on the user's
   `buttons.ini`), and the cross-build recipe. Follow [PORTING.md](PORTING.md).
3. **Keep CI honest.** `pr.yml` detects features from `crates/slot/Cargo.toml`,
   so a new feature is built automatically. Add the target to
   `experimental.yml` (that workflow lives on the `experimental` branch) so a
   tag produces its pak, and confirm the armhf or
   aarch64 build actually links.
4. **Ship it as experimental.** `exp-v*` pre-release, a row in the README's
   device table marked experimental, an entry in TESTERS.md that says exactly
   what to run and which files to attach, and an issue to track hardware
   verification.
5. **Promote only on evidence.** When a tester's `slot.log` (and
   `input-trace.log` for buttons) shows a clean boot, working input, display and
   audio, move it to stable through `flip` and update the README, ROADMAP and
   version map.

### Helping people who own the hardware

- Make failure **diagnosable**: logs that say what was probed and what was found,
  and the `DEBUG` trace, so a tester's report is enough to fix the port without
  the device in hand.
- Make wrong guesses **cheap to correct**: wrong button map, wrong path or wrong
  default should be fixable from a file or a menu, not a rebuild.
- State plainly in release notes what is unverified, and who to send logs to.
- The armhf builds cannot be run on a Flip. Do not claim they work; ask for a
  tester and give them a copy-pasteable request.

