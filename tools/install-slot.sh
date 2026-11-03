#!/bin/sh
# Install slot on a Miyoo handheld's SD card.
#
# Lives in the release zip next to the folders it copies, so run it from there:
#
#   ./install.sh                find the card, say what it will do, ask, do it
#   ./install.sh --check        look at the card and report; change nothing
#   ./install.sh --dry-run      say what it would do; change nothing
#   ./install.sh --card PATH    use this folder as the card
#   ./install.sh --yes          do not ask (still never formats or deletes)
#   ./install.sh --stock-menu   boot to the stock menu instead of into slot
#   ./install.sh --no-backup    do not copy replaced files to this computer
#
# The zip carries one device's payload, and this script finds it:
#
#   Miyoo Flip      App/Slot, Tools/my355/Slot.pak, miyoo355/app/MainUI
#   Miyoo Mini Plus App/Slot, Tools/miyoomini/Slot.pak, miyoo354/app/MainUI
#
# It copies those three, so the device boots straight into slot on its own
# firmware. Nothing on the device is changed except files under those folders;
# user data under Slot/ is never touched.
#
# On the Flip the firmware's own launcher is left alone and the card entry only
# runs when that folder is present. On the Mini Plus the stock launcher binary is
# kept beside the hand-over script as MainUI.stock, and Slot/Config/boot-menu
# brings it back.
#
# It never formats, partitions, deletes or writes to a raw device. Files it would
# replace are copied to this computer first, under ~/slot-flip-backups.
set -eu

# macOS writes "._name" files onto FAT cards to carry each file's extended
# attributes (Linux never does). They are litter on a device that never reads
# them, so the ones this script's own copy creates are removed again, inside the
# folders it wrote and nowhere else.
COPYFILE_DISABLE=1
export COPYFILE_DISABLE

HERE=$(cd "$(dirname "$0")" && pwd)
CARD=""
ASSUME_YES=0
DRY=0
CHECK=0
STOCK_MENU=0
BACKUP=1

DEVICE=""
APP="App"
PAK=""
BOOT_DIR=""
BOOT_ENTRY=""
PAYLOAD=""
KEY_FILES=""

say() { printf '%s\n' "$*"; }
err() { printf 'install: %s\n' "$*" >&2; }
die() { err "$*"; exit 1; }

usage() {
	cat <<'EOF'
Install slot on a Miyoo handheld SD card.

  ./install.sh                find the card, say what it will do, ask, do it
  ./install.sh --check        look at the card and report; change nothing
  ./install.sh --dry-run      say what it would do; change nothing
  ./install.sh --card PATH    use this folder as the card
  ./install.sh --yes          do not ask (still never formats or deletes)
  ./install.sh --stock-menu   boot to the stock menu instead of into slot
  ./install.sh --no-backup    do not copy replaced files to this computer

Never formats, partitions or deletes anything. Files it replaces are copied to
this computer first, under ~/slot-flip-backups.
EOF
	exit 0
}

while [ $# -gt 0 ]; do
	case "$1" in
	--check) CHECK=1 ;;
	--dry-run) DRY=1 ;;
	--yes|-y) ASSUME_YES=1 ;;
	--stock-menu) STOCK_MENU=1 ;;
	--no-backup) BACKUP=0 ;;
	--card) shift; [ $# -gt 0 ] || die "--card needs a path"; CARD=$1 ;;
	--card=*) CARD=${1#--card=} ;;
	-h|--help) usage ;;
	*) die "unknown option: $1 (try --help)" ;;
	esac
	shift
done

# ------------------------------------------------------------- what we carry

# One installer for both 1.0 devices: which payload the zip holds says which
# device it is for, so there is nothing to choose between downloads.
if [ -d "$HERE/Tools/miyoomini/Slot.pak" ]; then
	DEVICE="Miyoo Mini Plus"
	PAK="Tools/miyoomini/Slot.pak"
	BOOT_DIR="miyoo354"
	BOOT_ENTRY="miyoo354/app/MainUI"
elif [ -d "$HERE/Tools/my355/Slot.pak" ]; then
	DEVICE="Miyoo Flip"
	PAK="Tools/my355/Slot.pak"
	BOOT_DIR="miyoo355"
	BOOT_ENTRY="miyoo355/app/MainUI"
else
	die "$HERE/Tools is missing its device pak (miyoomini or my355): unpack the whole zip, then run install.sh from inside it"
fi

PAYLOAD="App $PAK $BOOT_DIR"
KEY_FILES="App/Slot/config.json App/Slot/launch.sh $PAK/launch.sh $PAK/slot $BOOT_ENTRY"

for d in $PAYLOAD; do
	[ -d "$HERE/$d" ] || die "$HERE/$d is missing: unpack the whole zip, then run install.sh from inside it"
done
for f in $KEY_FILES; do
	[ -f "$HERE/$f" ] || die "$HERE/$f is missing: the zip is incomplete"
done

hash() {
	if command -v sha256sum >/dev/null 2>&1; then
		sha256sum "$1" | cut -d' ' -f1
	elif command -v shasum >/dev/null 2>&1; then
		shasum -a 256 "$1" | cut -d' ' -f1
	else
		cksum "$1" | tr ' ' '_'
	fi
}

# ------------------------------------------------------------------ the card

fs_type() {
	# macOS: "/dev/disk4s1 on /Volumes/NO NAME (msdos, local, ...)"
	# Linux: "/dev/sdb1 on /media/x type vfat (rw,...)"
	line=$(mount 2>/dev/null | grep -F " on $1 " | head -1 || true)
	case "$line" in
	*"(msdos"*|*"(vfat"*|*"(exfat"*|*"type msdos"*|*"type vfat"*|*"type exfat"*) echo fat ;;
	"") echo unknown ;;
	*) echo other ;;
	esac
}

candidates() {
	if [ -d /Volumes ]; then
		for d in /Volumes/*; do
			[ -d "$d" ] && [ -w "$d" ] && [ "$(fs_type "$d")" = fat ] || continue
			printf '%s\n' "$d"
		done
	fi
	if [ -r /proc/mounts ]; then
		while read -r _dev mnt fstype _rest; do
			case "$fstype" in vfat|msdos|exfat) ;; *) continue ;; esac
			case "$mnt" in /|/boot|/boot/*|/proc*|/sys*) continue ;; esac
			[ -w "$mnt" ] && printf '%s\n' "$mnt"
		done < /proc/mounts
	fi
}

pick_card() {
	if [ -n "$CARD" ]; then
		[ -d "$CARD" ] || die "no such folder: $CARD"
		CARD=$(cd "$CARD" && pwd)
		return
	fi
	candidates | sort -u > "$TMP/cards"
	count=$(grep -c . "$TMP/cards" || true)
	case "$count" in
	0) die "no FAT32 card found. Plug the card into this computer and try again, or use --card PATH." ;;
	1) CARD=$(head -1 "$TMP/cards"); say "Card: $CARD" ;;
	*)
		say "More than one card looks possible. Which one is the $DEVICE's?"
		i=1
		while IFS= read -r c; do say "  $i) $c"; i=$((i + 1)); done < "$TMP/cards"
		printf 'Number: '
		read -r answer
		CARD=$(sed -n "${answer}p" "$TMP/cards" 2>/dev/null || true)
		[ -n "$CARD" ] || die "no card chosen"
		;;
	esac
}

free_kb() { df -Pk "$1" 2>/dev/null | awk 'NR==2 {print $4}'; }

need_kb() {
	total=0
	for d in $PAYLOAD; do
		total=$((total + $(du -sk "$HERE/$d" | cut -f1)))
	done
	echo $((total + 2048)) # a little room to work
}

problems() {
	p=""
	[ "$(fs_type "$CARD")" = fat ] || p="$p not-fat"
	avail=$(free_kb "$CARD" || echo 0)
	[ "${avail:-0}" -ge "$(need_kb)" ] || p="$p too-full"
	[ -w "$CARD" ] || p="$p not-writable"
	printf '%s' "$p"
}

report() {
	avail=$(free_kb "$CARD" || echo 0)
	say ""
	say "Device:     $DEVICE"
	say "Card:       $CARD"
	say "Filesystem: $(fs_type "$CARD")"
	say "Free space: $((avail / 1024)) MB free, about $(( $(need_kb) / 1024 )) MB needed"
	if [ -f "$CARD/$PAK/slot" ]; then
		say "Slot:       already on this card (this will update it)"
	else
		say "Slot:       new install"
	fi
	if [ -f "$CARD/$BOOT_ENTRY" ]; then
		say "Boot:       a card launcher entry is already there (will be replaced)"
	else
		say "Boot:       will be added, so the device starts in Slot"
	fi
	if [ "$DEVICE" = "Miyoo Mini Plus" ] && [ -f "$CARD/miyoo354/app/MainUI" ] \
		&& [ ! -f "$CARD/miyoo354/app/MainUI.stock" ]; then
		say "Stock menu: kept as miyoo354/app/MainUI.stock"
	fi
	if [ -d "$CARD/Slot" ]; then
		say "Your data:  Slot/ stays as it is (games, saves, states untouched)"
	fi
	if [ -d "$CARD/.tmp_update" ]; then
		say ""
		say "Note: this card boots MinUI. Slot installs and still runs from MinUI's"
		say "      Tools menu, but starting in Slot is a stock-firmware feature, so"
		say "      that part will not happen on this card."
	fi
	case "$(problems)" in
	"") say ""; say "Checks:     ok"; return 0 ;;
	*not-fat*) err "the card is not FAT32/exFAT. The $DEVICE reads FAT32: reformat it on your own computer (this script will not)." ;;
	*too-full*) err "not enough free space on the card." ;;
	*not-writable*) err "the card is read-only (check the little lock switch on the adapter)." ;;
	esac
	return 1
}

# ------------------------------------------------------------------ the copy

collect_replaced() {
	: > "$TMP/replaced"
	for d in $PAYLOAD; do
		( cd "$HERE" && find "$d" -type f ) | while IFS= read -r rel; do
			dst="$CARD/$rel"
			if [ -f "$dst" ] && [ "$(hash "$dst")" != "$(hash "$HERE/$rel")" ]; then
				printf '%s\n' "$rel" >> "$TMP/replaced"
			fi
		done
	done
}

back_up_replaced() {
	[ -s "$TMP/replaced" ] || return 0
	if [ "$BACKUP" -eq 0 ]; then
		say "Backup:     skipped (--no-backup)"
		return 0
	fi
	dir="$HOME/slot-flip-backups/$(date +%Y-%m-%d-%H%M%S)"
	while IFS= read -r rel; do
		mkdir -p "$dir/$(dirname "$rel")"
		cp -p "$CARD/$rel" "$dir/$rel"
	done < "$TMP/replaced"
	say "Backup:     the files this replaces are in"
	say "            $dir"
}

# The Mini Plus boots whatever <card>/miyoo354/app/MainUI is. Replacing the
# firmware's launcher with the hand-over script means keeping the original on the
# card too, or the escape hatch has nothing to run.
keep_stock_launcher() {
	[ "$DEVICE" = "Miyoo Mini Plus" ] || return 0
	live="$CARD/miyoo354/app/MainUI"
	[ -f "$live" ] || return 0
	[ -f "$CARD/miyoo354/app/MainUI.stock" ] && return 0
	[ "$(hash "$live")" = "$(hash "$HERE/$BOOT_ENTRY")" ] && return 0
	cp -p "$live" "$CARD/miyoo354/app/MainUI.stock" || return 1
	say "Stock menu: kept as miyoo354/app/MainUI.stock"
}

copy_payload() {
	keep_stock_launcher || return 1
	for d in $PAYLOAD; do
		mkdir -p "$CARD/$d"
		cp -R "$HERE/$d/." "$CARD/$d/" || return 1
	done
	chmod 755 "$CARD/$PAK/launch.sh" \
		"$CARD/$PAK/slot" \
		"$CARD/App/Slot/launch.sh" \
		"$CARD/$BOOT_ENTRY" 2>/dev/null || true
	drop_appledouble
}

# Only inside the folders this script wrote, and only the metadata files it
# just caused: nothing else on the card is touched.
drop_appledouble() {
	for d in $PAYLOAD; do
		[ -d "$CARD/$d" ] || continue
		find "$CARD/$d" -name '._*' -type f -exec rm -f {} + 2>/dev/null || true
		rm -f "$CARD/._$d" 2>/dev/null || true
	done
}

place_menu_flag() {
	if [ "$STOCK_MENU" -eq 1 ]; then
		mkdir -p "$CARD/Slot/Config"
		: > "$CARD/Slot/Config/boot-menu"
		say "Boot:       stock menu (Slot/Config/boot-menu created)"
	else
		rm -f "$CARD/Slot/Config/boot-menu" 2>/dev/null || true
	fi
}

verify() {
	bad=0
	for f in $KEY_FILES; do
		if [ ! -f "$CARD/$f" ]; then
			err "missing after copying: $f"
			bad=1
		elif [ "$(hash "$CARD/$f")" != "$(hash "$HERE/$f")" ]; then
			err "the copy of $f differs from the zip"
			bad=1
		fi
	done
	[ -x "$CARD/$PAK/slot" ] || { err "$PAK/slot is not executable"; bad=1; }
	if [ "$DEVICE" = "Miyoo Mini Plus" ] && [ -f "$CARD/miyoo354/app/MainUI.stock" ]; then
		[ -x "$CARD/miyoo354/app/MainUI.stock" ] || { err "miyoo354/app/MainUI.stock is not executable"; bad=1; }
	fi
	[ "$bad" -eq 0 ]
}

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

pick_card
report || exit 1

if [ "$CHECK" -eq 1 ]; then
	say ""
	say "Nothing was changed (--check)."
	exit 0
fi

collect_replaced

if [ "$DRY" -eq 1 ]; then
	say ""
	if [ -s "$TMP/replaced" ]; then
		say "Would replace, backing each up first:"
		sed 's/^/  /' "$TMP/replaced"
	else
		say "Would replace nothing already on the card."
	fi
	say "Dry run: nothing was changed."
	exit 0
fi

say ""
say "Copy Slot onto the card now? Nothing is deleted or reformatted, and the"
say "files it replaces are copied to this computer first."
if [ "$ASSUME_YES" -eq 0 ]; then
	printf 'Continue? [y/N] '
	read -r answer
	case "$answer" in y|Y|yes|YES) ;; *) say "Nothing was changed."; exit 0 ;; esac
fi

back_up_replaced || die "could not make the backup; nothing was changed"
copy_payload || die "copying failed; nothing has been deleted"
if [ ! -d "$CARD/Slot/Config" ]; then
	mkdir -p "$CARD/Slot/Config" 2>/dev/null || true
	rm -f "$CARD/._Slot" "$CARD/Slot/._Config" 2>/dev/null || true
fi
place_menu_flag
sync 2>/dev/null || true

if ! verify; then
	err ""
	err "Something did not copy correctly. Run this again, or copy the folders by"
	err "hand; nothing has been deleted."
	exit 1
fi

say ""
say "Done. Slot is installed."
say ""
say "1. Eject the card properly, put it back in the $DEVICE, switch it on."
say "2. It should start in Slot. MENU opens the in-game menu; games go"
say "   in Slot/Games/<GBA|GB|GBC|PS1|NES|SNES|MD|SMS|GG>."
say "3. For the stock menu instead, create an empty file"
say "   Slot/Config/boot-menu on the card and reboot."
