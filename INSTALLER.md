# The installer

`install.sh` sits inside the release zip next to the folders it copies. It exists
for people who would rather run one command than work out which folder goes where.

```
./install.sh            find the card, check it, say what it will do, ask, copy
./install.sh --check    look at the card and report; change nothing
./install.sh --dry-run  say what it would do; change nothing
./install.sh --card P   use P as the card
./install.sh --yes      do not ask (still never formats or deletes)
./install.sh --stock-menu   start in the stock menu instead of into slot
./install.sh --no-backup    do not copy replaced files to this computer
```

It is a POSIX shell script on purpose: it needs nothing but `sh`, `cp`, `find`,
`df`, `mount` and a checksum tool, all of which macOS and Linux already have. No
build step, no code signing, no runtime to install.

## What it does

1. **Finds the card.** Every mounted FAT32/exFAT volume that is writable. One is
   named and used; several are listed and you pick. It never guesses silently.
2. **Checks it.** FAT32/exFAT, enough free space, writable. A non-FAT card is
   refused with the reason, and it says the card must be reformatted by the user,
   because this project does not format cards.
3. **Says what it will do** — new install or update, what will be replaced, and
   that an existing `Slot/` folder is left alone.
4. **Backs up what it replaces** to `~/slot-flip-backups/<date>/` on the computer,
   never onto the card. Only files that differ are copied.
5. **Copies** `App`, `Tools` and `miyoo355`, then **verifies** the five files whose
   arrival proves it worked (both launch scripts, `config.json`, the `slot` binary
   and the autoboot script) by comparing checksums with the release.
6. **Sets the boot target**: into Slot by default; `--stock-menu` creates
   `Slot/Config/boot-menu` instead, and a plain run removes it.

## What it will never do

- Format, partition or relabel a card, or write to a raw device.
- Delete anything, or touch `Roms`, `Saves`, `Bios` or the contents of `Slot/`.
- Ask for `sudo`.
- Talk to the network. Everything it installs comes from the zip you downloaded.

The only files it removes are the `._*` metadata files that macOS creates when it
writes to a FAT card, inside the folders it just copied, so the card is not left
with dozens of useless hidden files.

## Two things worth knowing

- **macOS leaves hidden `._*` files on FAT cards** for its own metadata. The
  installer removes the ones its own copy produces; anything you create yourself
  (a game folder, say) may still get them. The device ignores them.
- **It replaces `<card>/miyoo355/app/MainUI`**, which on a stock card is the
  launcher's card-side entry. The launcher itself lives in the device's flash, so
  the stock menu is still reachable — through `Slot/Config/boot-menu`, and through
  the installer's `--stock-menu`.

## Testing

- `sh tools/test-install-slot.sh` — 24 cases over throwaway card folders: fresh
  install, update with backup, data left alone, `--check`/`--dry-run` changing
  nothing, a non-FAT card, a full card, an incomplete zip, a volume name with
  spaces, answering no, and the boot-menu flag. It runs in CI on every PR.
- Run by hand on a real FAT32 volume made with `hdiutil` (macOS): installed,
  verified, and left zero `._*` files. Case-insensitivity and FAT's lack of unix
  permissions behave as the device expects.
- **Not tested on Windows** — it has no `sh`. That needs its own version, and it is
  the next piece of work here.

## What this replaced

An earlier design fetched MinUI from GitHub at install time, pinned by hash, because
Slot used to be a MinUI Tool. Slot now runs on the Flip's own firmware, so the
installer has no network code, no MinUI step and no consent question about the
device's internal flash. That design is in git history; nothing of it is left in
the script.
