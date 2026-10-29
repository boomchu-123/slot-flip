# Testing the Miyoo A30 and Mini Plus ports

Both ports are experimental. This integration branch is a trial, not a
hardware-verified release. Use the device-specific pre-releases for testing;
do not install the other device's binary or the Flip's aarch64 cores.

## Evidence so far

| Part | A30 (`my282`) | Mini Plus (`miyoomini`) |
| --- | --- | --- |
| Display | Upright shelf and menus confirmed from exp-a30-v0.3.4 | Software renderer runs games |
| Buttons | Events decoded correctly from exp-a30-v0.3.6; printed labels still need a game test | Working on hardware |
| Sound | Game audio not yet confirmed | Heard by a tester on exp-v0.4.10 |
| Cores | Six cores covering nine systems packaged in exp-a30-v0.3.8 | Six cores covering nine systems packaged in exp-v0.4.13 |
| Real games | Awaiting the first GBA report | GBA/GB/GBC tried; other six systems unverified |
| Stock-launcher installation | Unverified | Unverified; required for 1.0 |

The new cores passed ARM integration tests under QEMU on a computer. This does
not establish device compatibility, speed, memory use, or sound. Mini Plus GBA
defaults to gpSP: Emerald works there but still crashes on mGBA after loading.
See [COMPATIBILITY.md](COMPATIBILITY.md) for the evidence and limits.

## Test a release

1. Download the correct device's zip from the
   [pre-releases](https://github.com/boomchu-123/slot-flip/releases).
2. On an existing MinUI installation, replace only
   `Tools/my282/Slot.pak` (A30) or `Tools/miyoomini/Slot.pak` (Mini Plus).
   Keep the card's separate `Slot` data folder, including saves and settings.
3. Run **Tools → Slot** once to create data folders. Put your own legally
   dumped games in `Slot/Games/<system>/`, then launch again.
   The folders are `GBA`, `GB`, `GBC`, `NES`, `SNES`, `MD`, `SMS`, `GG`, `PS1`.
4. Play for a minute or two. Check picture, printed button labels, sound and
   speed. Try saving, ejecting and reopening a game if it runs.
5. Attach `Slot/slot.log` to the device's issue and say which game, core and
   build you used. Check its first line names the expected release.
   For a crash followed by another launch, also attach `Slot/slot.log.prev`
   on the Mini Plus.

For input problems, include `Slot/input-trace.log`. The A30 launcher enables
input tracing automatically. An empty `DEBUG` file next to `launch.sh` enables
additional tracing; remove it after collecting a report.

- [A30 reports](https://github.com/boomchu-123/slot-flip/issues/2): does a game
  start, do the printed button labels match, is there sound, and is the picture
  centered? A photo helps diagnose the reported clock-screen black bar.
- [Mini Plus reports](https://github.com/boomchu-123/slot-flip/issues/3): the
  heartbeat lines report memory use, and OSS summaries report audio accepted
  per wall-clock second. If music sounds slow, note when and whether gameplay
  also slows down. These counters do not measure the speaker's physical rate.

## Release automation on this trial branch

The experimental workflow is manual and build-only. It refuses to upload a pak
without all six cores and their licences. The complete bundles currently live
in the published device releases; they have not all been committed to the
device directories. Do not turn automatic publishing back on until packaging
has a reproducible source for those bundles and the hardware checks are done.
