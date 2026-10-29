# Testers wanted — Miyoo A30 and Miyoo Mini Plus

The A30 reaches the clock screen on hardware, upright, but its buttons did
nothing. The trace from a retest showed why: the A30's kernel is 32-bit and writes
input events at 16 bytes each, and slot read them as 24, so every event was
garbled and none matched a button. exp-a30-v0.3.6 reads them at the right size.
`Slot/input-trace.log` is always written now (no `DEBUG` file needed) with every
input node and every raw event. On a retest, press each button once, then attach
`Slot/slot.log` and `Slot/input-trace.log`.

There are two **experimental** builds beyond the stable Miyoo Flip release. I
only own a Flip, so these ports depend on logs from people who own the other
devices. This page explains where each stands and how to help.

| Device | Platform | GPU | Likely to work? |
| --- | --- | --- | --- |
| Miyoo A30 | `my282` | Mali-400 ✅ | Maybe — uses the real GLES renderer, unverified |
| Miyoo Mini Plus | `miyoomini` | none ❌ | Not yet — needs a software renderer (TODO) |

## Status

| Part | A30 (`my282`) | Mini Plus (`miyoomini`) |
| --- | --- | --- |
| Builds (32-bit ARM / armhf) | ✅ | ✅ |
| Appears in MinUI **Tools** | ✅ expected | ✅ expected |
| Button map | 🟡 every button reaches slot and is named (exp-a30-v0.3.6); that each label matches the printed button is unchecked | 🟡 from MinUI's codes, unverified |
| **Draws to the screen** | ✅ the shelf and menus draw, upright (exp-a30-v0.3.4 on) | ❌ not implemented (probe only) |
| Sound / saves / save states | 🟡 untested: needs a game | ❌ blocked on the renderer |
| Emulator cores bundled | 🟡 gpSP (GBA) from exp-a30-v0.3.7; mGBA needs a newer C library than the A30 has | ❌ needs armhf builds |

### Why the two differ

The Flip (RK3566) has a Mali GPU and runs slot's OpenGL ES renderer.

- **The A30 (Allwinner A33) also has a Mali-400 GPU**, so it can use that same
  GLES renderer via the fbdev/EGL surface. It has a real chance of working — the
  unknowns are the exact EGL config, whether Mali-400's older GLES2 needs shader
  tweaks, and the button codes.
- **The Mini Plus (SigmaStar SSD202D) has no GPU** — just a raw framebuffer
  (`/dev/fb0`, typically 640×480 RGB565). slot's renderer is GLES-based, so the
  Mini Plus needs a brand-new **software** compositor that isn't written yet.
  Its build does the honest thing: it probes the framebuffer, logs what it sees,
  and exits — instead of a silent black screen.

## How to help

1. Install MinUI on your device.
2. Download the matching zip from the
   [pre-releases](https://github.com/boomchu-123/slot-flip/releases) (tagged
   `exp-v…`, marked *Pre-release*):
   - A30 → `Slot.pak-my282.zip` → copy to `<SD CARD>/Tools/my282/Slot.pak`
   - Mini Plus → `Slot.pak-miyoomini.zip` → copy to `<SD CARD>/Tools/miyoomini/Slot.pak`
3. Create an empty file named `DEBUG` inside that `Slot.pak` folder (next to
   `launch.sh`) so it records a full log and your button presses to
   `Slot/input-trace.log`.
4. Boot MinUI, open **Tools → Slot**.
   - A30: note exactly what you see — the shelf, a black screen, or a crash back
     to MinUI — and whether the buttons do the right thing.
   - Mini Plus: it will flash and return to MinUI (expected right now).
5. Press every button a couple of times (if it stays open), then quit.
6. Collect `Slot/slot.log` and `Slot/input-trace.log` and post them on the GitHub
   thread or open an issue — say which device you have.

## What I'm looking for in the log

**A30 (`my282`):**

- The surface/EGL lines: whether the GLES context and window were created, the
  panel size, and any `gl context:` / `shader:` errors.
- Whether anything rendered, or it failed at surface creation.
- The `input …` lines and the `input-trace.log` codes (to fix the button map).

**Mini Plus (`miyoomini`):**

- The `miyoomini fb0 …` line: resolution, bpp, pitch, and R/G/B bit offsets
  (confirms RGB565 vs something else).
- Any `framebuffer probe failed: …` line (fb node or ioctl differs).
- The `input …` lines showing which `eventN` nodes were found.

Thanks for helping. This is how the Flip port got finished too — run it, read
the log, fix, repeat.
