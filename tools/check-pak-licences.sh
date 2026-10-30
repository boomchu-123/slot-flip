#!/bin/sh
# Fails if the pak ships an emulator core that is not on the allowlist below, or an
# allowed core without its licence text and source, or a non-commercial core without
# the notice that says the whole download must stay free.
#
# slot-flip is free and not for sale, so a core with a non-commercial licence may be
# bundled, but only knowingly: the line for it below says so, the notice has to ship,
# and nobody may sell the download. To allow a new core you must have read its
# licence, then add a line below and ship its licence text and source with it.
set -eu

pak="${1:-flip/Tools/my355/Slot.pak}"
sys="$pak/System"
lic="$sys/licenses"

# core file | licence text that must ship with it | source archive that must ship (or -)
#   | free-only if the licence forbids commercial use, else -
ALLOWED='
mgba_libretro.so|mgba-MPL-2.0.txt|-|-
gpsp_libretro.so|gpsp-GPL-2.0.txt|gpsp-src.tar.gz|-
pcsx_rearmed_libretro.so|pcsx-GPL-2.0.txt|pcsx-src.tar.gz|-
fceumm_libretro.so|fceumm-GPL-2.0.txt|fceumm-src.tar.gz|-
snes9x2005_plus_libretro.so|snes9x2005-copyright.txt|snes9x2005-src.tar.gz|free-only
genesis_plus_gx_libretro.so|genesis_plus_gx-LICENSE.txt|genesis_plus_gx-src.tar.gz|free-only
picodrive_libretro.so|picodrive-COPYING.txt|picodrive-src.tar.gz|free-only
'

fail=0
free_only=0
for so in "$sys"/*_libretro.so; do
	[ -e "$so" ] || continue
	name="$(basename "$so")"
	row="$(printf '%s\n' "$ALLOWED" | grep "^$name|" || true)"
	if [ -z "$row" ]; then
		echo "NOT ALLOWED: $name is not a core cleared for the pak (see this script)" >&2
		fail=1
		continue
	fi
	licence="$(echo "$row" | cut -d'|' -f2)"
	source="$(echo "$row" | cut -d'|' -f3)"
	kind="$(echo "$row" | cut -d'|' -f4)"
	[ "$kind" = "free-only" ] && free_only=1
	{ [ -s "$lic/$licence" ] || { echo "MISSING OR EMPTY: $lic/$licence for $name" >&2; fail=1; }; }
	[ -s "$lic/$name.meta" ] || { echo "MISSING: $lic/$name.meta for $name" >&2; fail=1; }
	if [ "$source" != "-" ]; then
		[ -s "$lic/$source" ] || { echo "MISSING: $lic/$source for $name" >&2; fail=1; }
	fi
done

if [ "$free_only" = 1 ]; then
	notice="$lic/NOTICE-NONCOMMERCIAL.txt"
	if ! grep -qi "non-commercial" "$notice" 2>/dev/null; then
		echo "MISSING: $notice, which a non-commercial core needs shipped beside it" >&2
		fail=1
	fi
fi
exit "$fail"
