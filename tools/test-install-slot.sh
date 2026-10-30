#!/bin/sh
# Tests for tools/install-slot.sh over throwaway folders that stand in for a
# card. No device and no real SD card: every case asserts what changed and, just
# as importantly, what must not.
#
# The installer checks the filesystem with `mount` and the free space with `df`,
# so the tests put a stub for each first on PATH. That is the only thing they
# change about how it runs.
#
#   sh tools/test-install-slot.sh
#
# SC2015/SC2016/SC2295 are deliberate: `ok` and `no` are the two arms of every
# check, and the fixture scripts are written literally on purpose.
# shellcheck disable=SC2015,SC2016,SC2295
set -u

HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(dirname "$HERE")
INSTALL="$HERE/install-slot.sh"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

pass=0
fail=0
ok() { pass=$((pass + 1)); printf '  ok   %s\n' "$1"; }
no() { fail=$((fail + 1)); printf '  FAIL %s\n' "$1"; [ -n "${2:-}" ] && printf '       %s\n' "$2"; }

# --------------------------------------------------------------------- fixtures

# A stand-in release folder: the real layout, tiny stand-ins for the big files.
rel="$WORK/release"
mkdir -p "$rel/App/Slot" "$rel/Tools/my355/Slot.pak" "$rel/miyoo355/app"
printf '{"label": "slot."}\n' > "$rel/App/Slot/config.json"
printf '#!/bin/sh\nexec "$SD/Tools/my355/Slot.pak/launch.sh"\n' > "$rel/App/Slot/launch.sh"
printf '#!/bin/sh\n./slot\n' > "$rel/Tools/my355/Slot.pak/launch.sh"
printf 'not really a binary\n' > "$rel/Tools/my355/Slot.pak/slot"
printf '#!/bin/sh\nexec "$SD/Tools/my355/Slot.pak/launch.sh"\n' > "$rel/miyoo355/app/MainUI"
chmod 755 "$rel/App/Slot/launch.sh" "$rel/Tools/my355/Slot.pak/launch.sh" \
	"$rel/Tools/my355/Slot.pak/slot" "$rel/miyoo355/app/MainUI"
cp "$INSTALL" "$rel/install.sh"
chmod 755 "$rel/install.sh"

# `mount` that calls every folder a FAT card, and `df` that reports room.
stub="$WORK/stub"
mkdir -p "$stub"
cat > "$stub/mount" <<'EOF'
#!/bin/sh
printf '/dev/fake0 on / (ext4, local)\n'
printf '/dev/fake1 on %s (msdos, local, nodev, nosuid, noowners)\n' "$CARD_FOR_TESTS"
EOF
cat > "$stub/df" <<'EOF'
#!/bin/sh
printf 'Filesystem 1024-blocks Used Available Capacity Mounted on\n'
printf '/dev/fake1 999999 1 900000 1%% /media/CARD\n'
EOF
chmod 755 "$stub/mount" "$stub/df"

# `df` that reports a card with no room.
fullstub="$WORK/fullstub"
mkdir -p "$fullstub"
cp "$stub/mount" "$fullstub/mount"
cat > "$fullstub/df" <<'EOF'
#!/bin/sh
printf 'Filesystem 1024-blocks Used Available Capacity Mounted on\n'
printf '/dev/fake1 999999 999000 1 100%% /media/CARD\n'
EOF
chmod 755 "$fullstub/df"

new_card() { d="$WORK/card-$1"; mkdir -p "$d"; printf '%s' "$d"; }

install_into() { # card [extra args...]
	c="$1"; shift
	(cd "$rel" && PATH="$stub:$PATH" CARD_FOR_TESTS="$c" HOME="$WORK/home" \
		sh ./install.sh --card "$c" "$@" 2>&1)
}

snapshot() { (cd "$1" && find . -type f | sort | while IFS= read -r f; do printf '%s %s\n' "$f" "$(cksum "$f")"; done); }

echo "install-slot"

# 1. A fresh card gets everything.
c=$(new_card fresh)
mkdir -p "$c/Roms/GBA" "$c/Saves"
echo "game" > "$c/Roms/GBA/Some Game.gba"
out=$(install_into "$c" --yes)
[ -f "$c/Tools/my355/Slot.pak/slot" ] && [ -f "$c/App/Slot/config.json" ] \
	&& [ -f "$c/miyoo355/app/MainUI" ] && ok "fresh install copies the payload" \
	|| no "fresh install copies the payload" "$out"
[ "$(cat "$c/Roms/GBA/Some Game.gba")" = game ] && ok "leaves Roms alone" || no "leaves Roms alone"
[ ! -f "$c/Slot/Config/boot-menu" ] && ok "boots into slot by default (no menu flag)" \
	|| no "boots into slot by default (no menu flag)"
[ -x "$c/Tools/my355/Slot.pak/slot" ] && ok "the program is executable on the card" \
	|| no "the program is executable on the card"
case "$out" in *"Done. Slot is installed."*) ok "says it is done" ;; *) no "says it is done" "$out" ;; esac

# 2. --check and --dry-run change nothing at all.
c=$(new_card quiet)
mkdir -p "$c/Tools/my355/Slot.pak"
echo old > "$c/Tools/my355/Slot.pak/slot"
before=$(snapshot "$c")
install_into "$c" --check >/dev/null 2>&1
install_into "$c" --dry-run >/dev/null 2>&1
after=$(snapshot "$c")
[ "$before" = "$after" ] && ok "--check and --dry-run change nothing" || no "--check and --dry-run change nothing"

# 3. An update replaces the old files, and backs them up on the computer.
c=$(new_card update)
mkdir -p "$c/Tools/my355/Slot.pak"
echo "old binary" > "$c/Tools/my355/Slot.pak/slot"
out=$(install_into "$c" --yes)
cmp -s "$c/Tools/my355/Slot.pak/slot" "$rel/Tools/my355/Slot.pak/slot" \
	&& ok "update replaces the pak" || no "update replaces the pak" "$out"
bk=$(find "$WORK/home/slot-flip-backups" -type f -name slot 2>/dev/null | head -1)
[ -n "$bk" ] && [ "$(cat "$bk")" = "old binary" ] && ok "the replaced file is backed up on the computer" \
	|| no "the replaced file is backed up on the computer" "$out"
[ ! -d "$c/slot-flip-backups" ] && ok "no backup folder written onto the card" \
	|| no "no backup folder written onto the card"

# 4. Existing Slot data is never touched.
c=$(new_card data)
mkdir -p "$c/Slot/Saves/GBA" "$c/Slot/States/PS1/Game"
echo "a save" > "$c/Slot/Saves/GBA/Game.sav"
echo "a state" > "$c/Slot/States/PS1/Game/resume.state"
install_into "$c" --yes >/dev/null 2>&1
[ "$(cat "$c/Slot/Saves/GBA/Game.sav")" = "a save" ] && [ -f "$c/Slot/States/PS1/Game/resume.state" ] \
	&& ok "leaves games, saves and states alone" || no "leaves games, saves and states alone"

# 5. --stock-menu creates the escape file; a plain run removes it again.
c=$(new_card flag)
install_into "$c" --yes --stock-menu >/dev/null 2>&1
[ -f "$c/Slot/Config/boot-menu" ] && ok "--stock-menu creates the boot-menu flag" \
	|| no "--stock-menu creates the boot-menu flag"
install_into "$c" --yes >/dev/null 2>&1
[ ! -f "$c/Slot/Config/boot-menu" ] && ok "a plain run removes it again" \
	|| no "a plain run removes it again"

# 6. A card that is not FAT32 is refused, and nothing is written.
c=$(new_card notfat)
out=$(cd "$rel" && PATH="/usr/bin:/bin" HOME="$WORK/home" sh ./install.sh --card "$c" --yes 2>&1)
case "$out" in *not\ FAT32*) ok "refuses a card that is not FAT32" ;; *) no "refuses a card that is not FAT32" "$out" ;; esac
[ ! -e "$c/Tools" ] && [ ! -e "$c/App" ] && ok "the refused card is left empty" || no "the refused card is left empty"

# 7. A card with no room is refused.
c=$(new_card full)
out=$(cd "$rel" && PATH="$fullstub:$PATH" CARD_FOR_TESTS="$c" HOME="$WORK/home" sh ./install.sh --card "$c" --yes 2>&1)
case "$out" in *"free space"*) ok "refuses a card without room" ;; *) no "refuses a card without room" "$out" ;; esac
[ ! -e "$c/Tools" ] && ok "nothing copied to a full card" || no "nothing copied to a full card"

# 8. An incomplete zip is refused, and says to unpack the whole thing.
broken="$WORK/broken"
mkdir -p "$broken"
cp "$INSTALL" "$broken/install.sh"
cp -R "$rel/App" "$broken/App"
out=$(cd "$broken" && HOME="$WORK/home" sh ./install.sh --check 2>&1)
case "$out" in *"is missing"*) ok "an incomplete zip is refused" ;; *) no "an incomplete zip is refused" "$out" ;; esac

# 9. A volume name with spaces works, as real card names have them.
c="$WORK/card with spaces"
mkdir -p "$c"
install_into "$c" --yes >/dev/null 2>&1
[ -f "$c/Tools/my355/Slot.pak/slot" ] && ok "a card name with spaces works" || no "a card name with spaces works"

# 10. Answering no changes nothing.
c=$(new_card decline)
printf 'n\n' | (cd "$rel" && PATH="$stub:$PATH" CARD_FOR_TESTS="$c" HOME="$WORK/home" sh ./install.sh --card "$c" >/dev/null 2>&1)
[ ! -e "$c/Tools" ] && ok "answering no changes nothing" || no "answering no changes nothing"

# 11. The payload the tests used matches the repo, so a renamed folder is caught.
for f in App/Slot/config.json App/Slot/launch.sh Tools/my355/Slot.pak/launch.sh \
	miyoo355/app/MainUI; do
	src="$REPO/flip/$f"
	case "$f" in miyoo355/*) src="$REPO/flip/autoboot/$f" ;; esac
	[ -f "$src" ] && ok "repo has ${src#$REPO/}" || no "repo has ${src#$REPO/}"
done
# The frontend binary is git-ignored: CI and `task` build it, the release
# workflow drops it into the pak. Present or absent is both correct here.
if [ -f "$REPO/flip/Tools/my355/Slot.pak/slot" ]; then
	ok "the pak has a slot binary to ship"
else
	ok "no slot binary in the tree (ignored; the release build supplies it)"
fi

# ---------------------------------------------------------- Miyoo Mini Plus

# The same release shape for the other 1.0 device: the installer picks the
# payload out of the zip, so one script serves both.
relmm="$WORK/release-mm"
mkdir -p "$relmm/App/Slot" "$relmm/Tools/miyoomini/Slot.pak" "$relmm/miyoo354/app"
printf '{"label": "slot."}\n' > "$relmm/App/Slot/config.json"
printf '#!/bin/sh\nexec "$SD/Tools/miyoomini/Slot.pak/launch.sh"\n' > "$relmm/App/Slot/launch.sh"
printf '#!/bin/sh\n./slot\n' > "$relmm/Tools/miyoomini/Slot.pak/launch.sh"
printf 'not really a binary\n' > "$relmm/Tools/miyoomini/Slot.pak/slot"
printf '#!/bin/sh\necho hand over to slot\n' > "$relmm/miyoo354/app/MainUI"
chmod 755 "$relmm/App/Slot/launch.sh" "$relmm/Tools/miyoomini/Slot.pak/launch.sh" \
	"$relmm/Tools/miyoomini/Slot.pak/slot" "$relmm/miyoo354/app/MainUI"
cp "$INSTALL" "$relmm/install.sh"
chmod 755 "$relmm/install.sh"

install_mm() { # card [extra args...]
	c="$1"; shift
	(cd "$relmm" && PATH="$stub:$PATH" CARD_FOR_TESTS="$c" HOME="$WORK/home" \
		sh ./install.sh --card "$c" "$@" 2>&1)
}

# 12. A fresh Mini Plus card gets the payload, and the stock launcher is kept
#     beside the hand-over script: without that copy there is no escape hatch.
c=$(new_card mmfresh)
mkdir -p "$c/miyoo354/app"
printf 'stock launcher binary\n' > "$c/miyoo354/app/MainUI"
chmod 755 "$c/miyoo354/app/MainUI"
out=$(install_mm "$c" --yes)
[ -f "$c/Tools/miyoomini/Slot.pak/slot" ] && [ -f "$c/App/Slot/config.json" ] \
	&& ok "the Mini Plus payload is copied" || no "the Mini Plus payload is copied" "$out"
[ "$(cat "$c/miyoo354/app/MainUI.stock" 2>/dev/null)" = "stock launcher binary" ] \
	&& ok "the stock launcher is kept as MainUI.stock" || no "the stock launcher is kept as MainUI.stock" "$out"
cmp -s "$c/miyoo354/app/MainUI" "$relmm/miyoo354/app/MainUI" \
	&& ok "the hand-over script replaces MainUI" || no "the hand-over script replaces MainUI" "$out"
[ -x "$c/miyoo354/app/MainUI.stock" ] && ok "MainUI.stock stays executable" \
	|| no "MainUI.stock stays executable" "$out"

# 13. A second run must not overwrite the kept launcher with the hand-over script.
before=$(cat "$c/miyoo354/app/MainUI.stock")
install_mm "$c" --yes >/dev/null 2>&1
[ "$(cat "$c/miyoo354/app/MainUI.stock")" = "$before" ] \
	&& ok "reinstalling keeps the original MainUI.stock" || no "reinstalling keeps the original MainUI.stock"

# 14. --check and --dry-run change nothing on a Mini Plus card either.
c=$(new_card mmquiet)
mkdir -p "$c/miyoo354/app" "$c/Slot/Saves/GBA"
printf 'stock\n' > "$c/miyoo354/app/MainUI"
echo "a save" > "$c/Slot/Saves/GBA/Game.sav"
before=$(snapshot "$c")
install_mm "$c" --check >/dev/null 2>&1
install_mm "$c" --dry-run >/dev/null 2>&1
[ "$before" = "$(snapshot "$c")" ] && ok "Mini Plus --check and --dry-run change nothing" \
	|| no "Mini Plus --check and --dry-run change nothing"

# 15. The repo carries the Mini Plus files the zip is made from.
for f in flip/App-miyoomini/Slot/config.json flip/App-miyoomini/Slot/launch.sh \
	flip/autoboot/miyoo354/app/MainUI flip/Tools/miyoomini/Slot.pak/launch.sh; do
	[ -f "$REPO/$f" ] && ok "repo has $f" || no "repo has $f"
done

echo ""
printf '%s passed, %s failed\n' "$pass" "$fail"
[ "$fail" -eq 0 ]
