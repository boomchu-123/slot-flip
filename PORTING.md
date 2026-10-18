# Porting & extending Slot

This fork is organised so that most per-device and per-user changes happen in
config or in one small file, not scattered across the UI. This is the map of
the seams.

## Where device coupling lives

| Concern | File | Notes |
| --- | --- | --- |
| Button key codes | `crates/slot/src/input/evdev.rs` (`code_to_btn`) | Built-in default map, selected by the `flip` Cargo feature. Users override per-code in `Slot/Config/buttons.ini` with no rebuild. |
| Display surface | `crates/slot/src/device_app.rs` + `crates/slot-gfx` | `flip` uses an SDL2 GLES2 surface; the default uses fbdev. |
| LED / backlight / lib paths | `flip/Tools/my355/Slot.pak/launch.sh` | Shell only. These sysfs paths are device-specific. |
| Audio | `crates/slot/src/audio` | ALSA at 48 kHz on the Flip. |

## Adding a new device

1. **Launch script.** Copy `flip/Tools/my355/Slot.pak/launch.sh` into a new
   `Tools/<platform>/Slot.pak/` and adjust the LED and `LD_LIBRARY_PATH`
   lines for that hardware.
2. **Buttons.** Launch once with a `DEBUG` file present and read the key codes
   from `Slot/slot.log`. Either ship a `buttons.ini` with the pak or add a new
   default map behind a Cargo feature in `code_to_btn`.
3. **Build.** Build `slot` for the device's architecture. The `flip` feature is
   the template for an SDL2 target; add a sibling feature if a new device needs
   different display/audio wiring.

## Worked examples: the experimental Miyoo targets

Two sibling features live on the `experimental` branch. They illustrate the two
things that are easy to get wrong when adding a device, and the big fork in the
road (does it have a GPU?).

Both are **32-bit ARM (Cortex-A7)**, so they build for
`armv7-unknown-linux-gnueabihf`, **not** aarch64 like the Flip. The committed
Flip cores are aarch64 and will not run on them; they need armhf cores.

### `my282` — Miyoo A30 (has a GPU)

The A30 has a **Mali-400 GPU**, so it reuses slot's existing GLES renderer
through the fbdev/EGL surface (the same path the upstream default uses). It needs
no new renderer — just a feature that routes it to `run_gles`, its own button
map, and an armhf build. This is the cheaper, more promising target.

### `miyoomini` — Miyoo Mini Plus (no GPU)

The SSD202D has **no GPU**, so neither the Flip's GLES2 surface nor the EGL fbdev
surface works — it has only a raw `/dev/fb0` (RGB565). A working port needs a
**software** compositor, which is still TODO. Its build probes the framebuffer,
logs it, and exits.

### Where each seam lives

| Concern | `my282` (A30) | `miyoomini` (Mini Plus) |
| --- | --- | --- |
| Feature in `crates/slot/Cargo.toml` | `my282` | `miyoomini` |
| Button map (`code_to_btn`, `crates/slot/src/input/evdev.rs`) | best-guess default | native L1/R1/L2/R2, no stick/lid |
| Renderer path (`crates/slot/src/device_app.rs`) | `run_gles` + `FbdevSurface` | `run_miyoomini` (probe only) |
| Framebuffer probe | n/a | `crates/slot-gfx/src/miyoofb.rs` |
| Pak + launch script | `flip/Tools/my282/Slot.pak/` | `flip/Tools/miyoomini/Slot.pak/` |
| CI (armhf build + pre-release) | `.github/workflows/experimental.yml` (matrix) | same |

The Mini Plus's remaining work is a CPU rasteriser that consumes the same `Draw`
list the GLES pipeline uses and blits to the framebuffer. See `TESTERS.md` for
current status of both.

## User-facing config (data-driven, no rebuild)

| File | Purpose |
| --- | --- |
| `Slot/Config/slot.ini` | General settings. Currently `background = <file|random>`. |
| `Slot/Config/buttons.ini` | Per-code button remap. |
| `Slot/Config/theme.txt` | Housing / recess / opening / edge colours (hex). |
| `Slot/Config/video_mode.txt` | Per-game aspect (set in-app). |
| `Slot/Config/selected_core.txt` | Per-game core override. |

Starter `slot.ini` and `buttons.ini` are written on first run by
`crates/slot/src/root.rs` (`ensure` → `write_starter`). They are only created
when absent, so hand edits are never clobbered.

## Adding a new setting

To add a new `slot.ini` key:

1. Read it with `slot_store::ini::value(root, wallpaper::SETTINGS_FILE, "key")`.
   (`background` in `crates/slot/src/wallpaper.rs` is the worked example.)
2. Document it in the `STARTER_SLOT_INI` string in `root.rs` so the starter
   file explains it.

## Adding a quick-menu row

The quick menu is a fixed list of `QuickRow` variants in
`crates/slot-ui/src/quick_menu.rs`:

1. Add the variant to `QuickRow` and `QuickRow::ALL`, give it a `label`, and
   decide whether it `opens()` a sub-screen or cycles a value.
2. Handle it in `crates/slot/src/app.rs`: `quick_value` (value shown),
   `open_quick_row` (if it opens), and `change_setting` (if it cycles).
3. If it opens a screen, add a `Phase` variant, a render arm in `app.rs`, and a
   face builder (see `Phase::Buttons` / `buttons_face` for a minimal example).
