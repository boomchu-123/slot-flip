# Merge plan: one branch for 1.0

**Delete this file, and the notice at the top of `README.md`, in the final merge.**

This branch (`merge/flip-into-experimental`) is a trial. It is `experimental` with
`flip` merged in, so that the work of combining the branches is done and checked
before anything real is touched. Nothing here has run on a device.

## The three lines of work

| Branch | What it holds |
| --- | --- |
| `flip` | The Flip, the nine systems, the current upstream base. The 1.0 base. |
| `experimental` | The Miyoo A30 target (`my282`), an **old probe-only** Mini Plus build, the experimental workflow, `TESTERS.md`. |
| `feat/miyoomini-software` | The **real** Mini Plus port: software renderer, sound, the pak with cores. Built on `flip`. No `my282`. |

`experimental` and `feat/miyoomini-software` are two different answers for the Mini
Plus. Only `feat/miyoomini-software` is the one to keep. The A30 is the only thing
`experimental` has that the other does not.

## State of this branch

- `flip` is fully merged in (0 commits behind). Conflicts were resolved as follows:
  `AGENTS.md`, `ROADMAP.md`, `README.md` took flip's version whole; `.gitignore`
  kept both sides; `device_app.rs` kept both sides, with flip's additions gated
  `cfg(not(feature = "miyoomini"))`.
- Checked: `cargo check` clean for `flip`, `miyoomini`, `my282`; the CI test set
  passes; armhf release builds link (A30 needs glibc 2.17 at most, Mini Plus 2.28).

## Next step: merge `feat/miyoomini-software` into this branch

Do it on a new branch off this one, not on this one.

```sh
git fetch --all
git switch -c merge/mini-into-experimental merge/flip-into-experimental
git merge --no-commit --no-ff feat/miyoomini-software
```

A dry run on this branch gives **five** conflicts. Resolve them like this:

| File | Resolution |
| --- | --- |
| `crates/slot-gfx/Cargo.toml` | Take the Mini Plus side: `software = ["dep:libc"]` and `miyoomini = ["software"]`. |
| `crates/slot-gfx/src/lib.rs` | Take the Mini Plus side. Drop the `miyoofb` module and the `probe_fb` / `FbInfo` re-export, and delete `crates/slot-gfx/src/miyoofb.rs` (the old probe; nothing else uses it). |
| `crates/slot/Cargo.toml` | Take the Mini Plus `miyoomini = ["slot-gfx/software", "slot-store/flip"]` line, and **keep experimental's `my282 = ["slot-store/flip"]`**. |
| `crates/slot/src/input/evdev.rs` | Take the Mini Plus side for `miyoomini` (its codes were measured; the Select/Start lines are the same mapping in another order). **Keep the whole `my282` table** (`code_to_btn` and `WANTED`), and widen the generic-map `cfg` so it also excludes `my282`: `all(not(feature = "flip"), not(feature = "miyoomini"), not(feature = "my282"))`. |
| `flip/Tools/miyoomini/Slot.pak/launch.sh` | Take the Mini Plus side whole. Experimental's is the old probe launcher. |

### The one that does not show up as a conflict

`crates/slot/src/device_app.rs` **merges without a conflict and comes out wrong.**
On the Mini Plus branch the `miyoomini` feature runs the normal `run_gles` path
over the software backend. On this branch, experimental's gating still routes
`miyoomini` to the old `run_miyoomini` probe, which prints "software renderer not
yet implemented" and exits. Git cannot tell, so the build compiles and the Mini
Plus pak would silently do nothing.

Fix: after the merge, take the Mini Plus branch's `device_app.rs` whole.

```sh
git checkout feat/miyoomini-software -- crates/slot/src/device_app.rs
```

`my282` needs nothing there: it takes the `not(flip)` path and uses `FbdevSurface`,
as it did before. Then confirm the file differs from the Mini Plus branch's only if
you meant it to:

```sh
git diff feat/miyoomini-software -- crates/slot/src/device_app.rs   # expect empty
```

### Files that only one side has, and what to do about them

- `.github/workflows/experimental.yml` (experimental only). The Mini Plus pak is
  built and uploaded by hand today and ships cores and licence files that this
  workflow does not package. Decide whether CI should build the real pak, or leave
  the workflow to the A30 only, before the final merge. Do not let it keep
  publishing a Mini Plus pak without cores.
- `TESTERS.md`, `PORTING.md`, `CONTRIBUTING.md` (experimental): still say the Mini
  Plus "does not draw" and is probe-only. Update them to match the Mini Plus
  branch.
- `flip/Tools/my282/Slot.pak/` (experimental only): launcher only, **no armhf
  cores**, so the A30 pak still cannot play a game.
- `flip/Tools/miyoomini/Slot.pak/`, `flip/App-miyoomini/`, `crates/slot-gfx/src/soft/`,
  `crates/slot/src/audio/miao.rs`, `miyoo.rs` (Mini Plus branch only): these
  arrive as-is.

## Done on `merge/mini-into-experimental`

This branch is the second merge, already carried out and checked, so the real one
is a repeat of it. It is `merge/flip-into-experimental`, then the A30 branch
(`fix/a30-input-discovery`), then the Mini Plus branch (`fix/miyoomini-capability-words`),
which already holds the sound, large-ROM, logging, armv7-core and capability-word work.
If either device branch gets new commits, **re-merge them onto a fresh branch from
`merge/flip-into-experimental`**; do not patch this one by hand.

Conflicts met, and what was done (the five in the table above, plus these):

| File | Resolution |
| --- | --- |
| `crates/slot-power/src/lib.rs` | Union of the export lists (flip's additions plus `has_bit_in`). |
| `crates/slot/tests/evdev.rs` (A30 merge) | Keep both sides; the shared closing braces interleave, so a textual "keep both" breaks the file. |
| `crates/slot/tests/evdev.rs` (Mini Plus merge) | Keep the Mini Plus `a_mask_is_read_in_the_words_the_kernel_printed`, which is a superset, and drop the A30 copy of the same name. |
| `COMPATIBILITY.md`, `ROADMAP.md` | Take the Mini Plus side for the Mini Plus hunks (sound is confirmed; the mGBA limit is recorded). |
| `crates/slot/src/input/evdev.rs` | As in the table: Mini Plus table for `miyoomini`, whole `my282` table kept, the generic `cfg` widened to exclude `my282`. |

**Silent hazard, found by building for armv7, not by `cargo check` on the host:**
`crates/slot/src/crash.rs` gets a second `registers` function for
`target_arch = "arm"` (both lines of work added one, in different places). It
merges without a conflict and then fails to compile for any 32-bit ARM target
with E0428. Delete one. A host `cargo check` does not see it.

Both `has_bit_in` blocks (docs and code) are textually identical on the two
branches on purpose; if one is edited, edit the other the same way or the merge
conflicts again.

Checked on this branch, none on a device: `cargo check` for `flip`, `miyoomini`,
`my282`; the evdev integration tests (28); the `--lib` tests for all three
features, and for `my282` and `miyoomini` run as armv7 under qemu-user (121 each);
release builds: Mini Plus (armv7, no probe string, software renderer present),
A30 (zigbuild, glibc 2.17), Flip.

## Checks after the second merge

Run all of these; none was run on a device.

1. `cargo check -p slot --no-default-features --features X` is clean, with no
   warnings, for `flip`, `miyoomini` and `my282`.
2. The CI test set passes (all `--lib` tests, the integration tests, and the
   `flip`-feature tests). The Mini Plus build also needs
   `cargo test -p slot --lib --no-default-features --features miyoomini`.
3. armhf release builds link, with the glibc floor enforced:
   - A30: `cargo zigbuild ... --features my282 --target armv7-unknown-linux-gnueabihf.2.17`
   - Mini Plus: `... --features miyoomini --target armv7-unknown-linux-gnueabihf.2.28`

   `readelf -V` on each must list nothing newer than that.
4. **The Mini Plus binary must not contain `software renderer not yet implemented`**
   (`strings <binary> | grep -c "not yet implemented"` is `0`). This is the check
   for the `device_app.rs` hazard above.
5. The Flip is not disturbed: `git diff origin/flip -- crates/` should be
   explainable entirely by `my282` and `miyoomini` work, and the `flip`-feature
   tests must pass. Read that diff; do not assume it.
6. Pak licence check: `tools/check-pak-licences.sh`.

## Before the final merge into `flip`

- Wait for the hardware reports first. The Mini Plus sound fix and the A30
  rebuild are both unconfirmed, and the testers' logs may change code on
  `feat/miyoomini-software` or `experimental`. If they do, re-merge rather than
  patching this branch.
- Still open for the 1.0 gate on the Mini Plus: the stock-launcher install, armhf
  cores for NES, SNES, MD, SMS, GG and PS1, and the nine-system sweep. See
  `ROADMAP.md`.
- Open one PR into `flip`. Do not rewrite `flip`'s history. Delete this file and
  the README notice in it.
