# Installer (proposal)

**Status: a design, not built.** Nothing here exists yet. It is written down first
so it can be argued with before any code touches an SD card.

**Goal:** a stranger with a Miyoo Flip and a Mac or Linux computer gets from "I have
a card" to "Slot is on it" without reading a guide written by someone who already
knows the answer. macOS and Linux first; Windows later.

**The rule that outranks the goal:** the installer must never lose anyone's data. A
slightly harder install is better than one that can damage a card.

## What was checked

These are facts read from the real files, not assumptions.

| Fact | Where it comes from | Why it matters |
| --- | --- | --- |
| MinUI is **archived**. The repository went read-only on 2025-11-27 and every device is marked "Legacy". The last release, `v20251127-1`, will not change. | MinUI's README and release list | We can pin one exact file by hash and it can never be swapped under us. If the release is ever taken down, there is no mirror (see below). |
| MinUI has **no licence file**. GitHub reports none. | Repository metadata | We may not redistribute it. The installer downloads it from the original release at install time. |
| The official base zip (`MinUI-20251127-1-base.zip`, 40 MB) matches GitHub's published SHA-256 `b1a5ffef…1632`. | Downloaded and hashed | The pin is checkable by anyone. |
| The Flip install is: copy the `miyoo355` folder and `MinUI.zip` (without unzipping it) to the card root, card in the **right-hand slot**, beneath the power button. | The `README.txt` inside the zip | This is the official procedure the installer automates. |
| The card should be a reputable brand, **freshly formatted FAT32 (MBR)**. | Same `README.txt` | We check for it. We never format. |
| On first boot MinUI **rewrites the Flip's internal system image**: it dumps `/dev/mtd3`, swaps in its own `runmiyoo.sh`, and flashes it back. If that hook is already there it skips this. | `miyoo355/app/my355/init.sh` | This changes the device, not only the card. The installer must say so plainly and ask first. How to get back to stock firmware has **not** been verified by this project. |
| MinUI is installed for the Flip when `.system/my355/paks/MinUI.pak/launch.sh` exists on the card. Its Flip `install.sh` does nothing, and `Tools/` is not in its zip. | Inside `MinUI.zip` | A reliable "MinUI is already here" check. |
| Unpacked, MinUI is about 183 MB, and `MinUI.zip` (38 MB) stays on the card until first boot. The Slot pak is about 38 MB on the card (17.5 MB to download). | Measured | Free-space check: about 300 MB for a fresh install, 60 MB for Slot alone. |

## Decisions I need from you

1. **Shell script, not a compiled program.** A POSIX `sh` script runs on macOS and
   Linux with only tools they already have (`curl`, `unzip`, `shasum`/`sha256sum`).
   A Rust binary would add a build matrix and code signing for little gain now.
   Windows, which has no `sh`, is where a compiled installer starts to pay off.
2. **Ship the installer inside the release, with the pak.** One download,
   `Install-Slot-vX.zip`, holding `install.sh`, `Slot.pak/` and a `SHA256SUMS`. The
   only thing fetched over the network is MinUI. That keeps the trust root as one
   thing, the release you chose to download.
3. **Install MinUI for people who lack it, with explicit consent.** This is the
   decision that matters most, because of the internal-image change above. My
   recommendation is yes: say what will happen, require a typed `yes`, and default
   to no. The alternative is to link to MinUI's own instructions and stop.

## The flow

```
install.sh                     interactive, plain English
install.sh --dry-run           show everything it would do, change nothing
install.sh --check             inspect a card and report problems; changes nothing
install.sh --card /path        skip detection
install.sh --minui-zip FILE    use a file you already downloaded (still hash-checked)
```

1. **Find the card.** Look for removable, mounted volumes (macOS `diskutil info`;
   Linux `lsblk` and `/proc/mounts`). If there is exactly one, show it and ask. If
   there are several, list them and ask which. Never pick silently. Refuse anything
   that is the system disk, is not a mounted volume's root, is read-only, or is not
   FAT32 or exFAT.
2. **Look at what is on it** and decide the mode:

   | The card has | Mode | Does |
   | --- | --- | --- |
   | no MinUI | **Fresh** | Phase 1: MinUI, then stop (see below) |
   | MinUI for another device only (`.system` but no `my355`) | **Add the Flip** | Copy the `miyoo355` bootstrap and `MinUI.zip`, as MinUI's own notes say, then stop |
   | MinUI for the Flip, no Slot | **Add Slot** | Install `Slot.pak` |
   | Slot already there | **Update** | Replace `Slot.pak`; show old and new version; leave `Slot/` alone |

3. **Show the plan** (every file it would add, every file it would overwrite, the
   space needed), then ask. `--yes` skips only the final question, never the typed
   `yes` for the MinUI step.
4. **Back up** any file it is about to overwrite (below).
5. **Copy, then verify**: compare sizes and hashes of what was written, `sync`.
6. **Tell the person the next step**: eject the card, which slot, what to expect.

### Fresh cards take two runs, on purpose

Phase 1 puts MinUI on the card and ends with: "Put the card in the Flip's right-hand
slot and switch it on. MinUI installs itself, and the screen changes. When it shows
a menu, switch off, bring the card back and run this again to add Slot."

Putting Slot on in the same run would be simpler, but it relies on MinUI's first
boot leaving a pre-placed `Tools/my355/Slot.pak` alone. Nothing in its scripts
removes it, but that has not been **tested**, and a blank card is needed to test it.
If a test shows it is safe, one run can replace two later. Until then the
two-run version is the one that cannot go wrong.

## What it will never do

- Format, partition or relabel a card, or write to a raw device.
- Delete anything on the card. It adds files, and replaces only the ones it backed up.
- Touch the contents of `Roms`, `Saves`, `Bios` or the `Slot` data folder. It may
  create the empty folders MinUI and Slot expect if they are missing.
- Talk to anything but the one pinned MinUI address. No telemetry, no update check
  unless asked (`--check-update`, which only reads this repository's latest release).
- Run with `sudo`, or ask for it.
- Install MinUI without a typed `yes` after the explanation.

## Fetching and trust

- The MinUI address and SHA-256 are **pinned** in one small file in the repo, so a
  change to them shows up in review (`tools/review-pr.py` already flags `tools/`).
- HTTPS only, redirects allowed (GitHub sends releases through its CDN), the hash
  checked **before** anything is unpacked. A mismatch deletes the download and stops.
- A scheduled check confirms the pinned hash still equals the digest GitHub reports
  for the release, so a quiet change on their side is noticed.
- **If the release is ever removed**, the installer says so and offers
  `--minui-zip` with a file the user has. The pinned hash still applies, so a copy
  from anywhere is safe to use. We cannot host a mirror without a licence.
- Slot's own files are not downloaded: they ship in the installer zip. Release notes
  carry the zip's SHA-256 so it can be compared by hand. GitHub build attestations
  would let anyone verify it came from this repository's workflow; that is a good
  hardening step for later, not part of the first version.

## Backups

Before overwriting any existing file, copy it to a timestamped folder on the
**computer** (`~/slot-flip-backups/<date-time>/`, same relative paths), and print
where it is. In the common case, an update, that is the old `Slot.pak`; on a fresh
card there is usually nothing to back up. A restore is `cp -R`, and `--restore
<folder>` does it. Nothing is ever backed up onto the card, so a failing card cannot
take its own backup with it.

For a fresh MinUI install on a card that already holds other things, the plan says
in bold that it only **adds** files, and recommends a copy of the card first. A
whole-card image is too large and too platform-specific to automate honestly.

## Checking a card (`--check`)

Changes nothing and prints a report short enough to paste into an issue, with the
volume name left out. It looks for the mistakes people actually make:

- MinUI not installed, or installed for a different device.
- `Slot.pak` in the wrong folder (it must be `Tools/my355/Slot.pak`), or missing a
  core, `launch.sh` or the `slot` binary; or a version older than the installer's.
- Games in `Slot/Games/<folder>` with the wrong folder name or an extension that
  system does not read.
- Card formatted exFAT or NTFS, or nearly full.
- A card that is read-only (the little lock switch on an adapter).
- "It is in the wrong slot": a hint when MinUI looks installed but the person says
  nothing happens.

## Packaging

- `tools/install/install.sh`, the pins file, and `Install Slot.command` for macOS
  (double-click opens Terminal and runs the script). A downloaded `.command` file is
  quarantined, so the first run needs right-click, Open. The docs say so, because
  pretending otherwise would waste someone's first impression.
- `release.yml` builds `Install-Slot-vX.zip` next to the existing
  `Slot.pak-my355.zip`, which stays for people who prefer to drag and drop.
- `INSTALL.md` is rewritten around two cases, "I have MinUI" and "I don't". Its
  current text is out of date: it still describes a Game Boy only app.

## Testing

What can be proven without hardware, and will run in CI on every PR:

- `shellcheck` over the script.
- A test harness over **fake card folders**: fresh, MinUI present, Slot present
  (older), another device's MinUI, read-only, too full, hash mismatch (a tampered
  zip), an interrupted copy followed by a rerun, and a name that differs only by
  case. Each asserts both what changed and what **must not** change.
- A dry run must change nothing: assert the folder is byte-identical afterwards.
- The pins check against GitHub's reported digest.

What needs a real volume, and will be run before release on macOS: a real FAT32
volume made with `hdiutil`, to catch what folders hide (no symlinks or execute bits,
case-insensitive names, hidden-dot folders such as `.system` and `.tmp_update`).

What only a person with a Flip can confirm, and will be listed as unconfirmed until
then: that a card the installer prepared boots MinUI and then Slot on a real device,
including the fresh two-run path on a blank card. The release notes will say which
of those were run.

## Not in the first version

- **Windows.** Needs a compiled or PowerShell installer and drive-letter handling.
  The script is written as small steps (find, inspect, plan, back up, copy, verify)
  so a port can reuse the plan.
- **One-run fresh installs**, until a blank card has proven it safe.
- **Other devices.** The Mini Plus comes with 2.0; the installer then learns each
  device's own folder, as MinUI's notes list them.
- **A graphical installer.** Possibly with Windows.
- **Formatting help.** The installer will link to a short, correct "format as FAT32"
  note per OS but never do it.

## Risks

- **The consent step is the weakest point.** People click through. The wording has
  to say what happens to the device in a sentence, and the default has to be no.
- **MinUI is archived.** It will not get fixes, and a Flip firmware change could
  break it. This project can document that; it cannot fix MinUI.
- **Pinning to one release** means a card that already has a different MinUI build
  is left alone: the installer never replaces a MinUI that is already there, which is
  what the "MinUI present" modes do. Only a card with no MinUI gets the pinned one.
