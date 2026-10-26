#!/bin/sh
# slot. for the Miyoo Mini Plus (miyoomini), launched as a MinUI Tool.
#
# EXPERIMENTAL. The Mini Plus has no GPU, so this build draws with the software
# rasteriser on the CPU: the shelf and the menus work, and the handheld keeps
# all of its data in /mnt/SDCARD/Slot. Emulator cores for 32-bit ARM are NOT
# bundled yet, so a cart will not start until armhf cores are in
# Slot/System. Drop legally dumped ROMs into Slot/Games/<GBA|GB|GBC>/.
#
# Please send Slot/slot.log to the GitHub thread (see TESTERS.md).

DIR="$(dirname "$0")"
SD="${SDCARD_PATH:-/mnt/SDCARD}"
SLOT_ROOT="$SD/Slot"

# /config/lib and /customer/lib hold the vendor libraries (libmi_ao and the ones
# it needs) that the sound output loads; MinUI's launcher already has them on the
# path, and a launcher that does not should not make the sound silent.
export LD_LIBRARY_PATH="/usr/lib:/usr/miyoo/lib:/config/lib:/customer/lib:$LD_LIBRARY_PATH"
export SLOT_ROOT

# The Mini Plus LCD is fitted upside down relative to the framebuffer's scan
# order, so the output stage reverses both axes. Confirmed on hardware on
# 2026-10-25: without this the shelf is drawn inverted.
export SLOT_FB_ROTATE=180

LOG="$SLOT_ROOT/slot.log"

# Diagnostics are opt-in. Drop a file named DEBUG next to this script to turn on
# the verbose input trace while chasing a problem.
if [ -f "$DIR/DEBUG" ]; then
	export SLOT_TRACE=1
	export SLOT_TRACE_LATENCY=1
fi

# Which sound output to use, if the default gives trouble. Slot plays through the
# console's own vendor interface and falls back to ALSA by itself; a file
# Config/audio holding "alsa" makes it use ALSA straight away, so comparing the
# two is a one-line change and not a rebuild.
if [ -f "$SLOT_ROOT/Config/audio" ]; then
	SLOT_AUDIO="$(tr -cd 'a-zA-Z' < "$SLOT_ROOT/Config/audio")"
	[ -z "$SLOT_AUDIO" ] || export SLOT_AUDIO
fi

# Seed the emulator cores into Slot/System, replacing a copy that differs so a
# core fix in a new pak reaches an existing card. `cmp -s` is the test the Flip
# pak uses for the same reason.
mkdir -p "$SLOT_ROOT/System"
for c in mgba_libretro.so gpsp_libretro.so; do
	if [ -f "$DIR/System/$c" ] && ! cmp -s "$DIR/System/$c" "$SLOT_ROOT/System/$c"; then
		cp "$DIR/System/$c" "$SLOT_ROOT/System/$c"
	fi
done

cd "$DIR"
# The package name goes first so a report can never be mistaken for another
# build again: the first tester log was from the previous package and nothing
# in it said so.
printf 'slot: pak %s\n' "$(cat "$DIR/slot.version" 2>/dev/null || echo unknown)" > "$LOG"

# MinUI drops the CPU to its 504 MHz menu speed when it starts and nothing puts
# it back before a Tool runs, so without this the shelf, the software renderer
# and the emulator all run at under half the clock MinUI gives its own games.
# overclock.elf is MinUI's helper; without MinUI it is simply not there and the
# clock is left as the firmware set it. A file Config/cpu-khz overrides the
# speed, because a wrong guess here should be a one-line fix and not a rebuild.
# The helper writes SoC registers, so the number is kept inside the two speeds
# MinUI itself uses.
OVERCLOCK="$(command -v overclock.elf 2>/dev/null)"
[ -n "$OVERCLOCK" ] || [ ! -x "$SD/.system/miyoomini/bin/overclock.elf" ] ||
	OVERCLOCK="$SD/.system/miyoomini/bin/overclock.elf"
KHZ=""
[ ! -f "$SLOT_ROOT/Config/cpu-khz" ] || KHZ="$(tr -cd '0-9' < "$SLOT_ROOT/Config/cpu-khz")"
if [ -z "$KHZ" ] || [ "$KHZ" -lt 504000 ] || [ "$KHZ" -gt 1488000 ]; then
	KHZ="${CPU_SPEED_GAME:-1296000}"
fi
if [ -n "$OVERCLOCK" ]; then
	printf 'slot: cpu: asking for %s kHz\n' "$KHZ" >> "$LOG"
	"$OVERCLOCK" "$KHZ" >> "$LOG" 2>&1
else
	printf 'slot: cpu: no overclock.elf, the clock is left as it was\n' >> "$LOG"
fi

./slot --init-root "$SLOT_ROOT" >> "$LOG" 2>&1
./slot >> "$LOG" 2>&1
sync
