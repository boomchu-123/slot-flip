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

export LD_LIBRARY_PATH="/usr/lib:/usr/miyoo/lib:$LD_LIBRARY_PATH"
export SLOT_ROOT

LOG="$SLOT_ROOT/slot.log"

# Diagnostics are opt-in. Drop a file named DEBUG next to this script to turn on
# the verbose input trace while chasing a problem.
if [ -f "$DIR/DEBUG" ]; then
	export SLOT_TRACE=1
	export SLOT_TRACE_LATENCY=1
fi

# Seed the emulator cores into Slot/System on first run, if the pak carries any.
mkdir -p "$SLOT_ROOT/System"
for c in mgba_libretro.so gpsp_libretro.so; do
	if [ ! -f "$SLOT_ROOT/System/$c" ] && [ -f "$DIR/System/$c" ]; then
		cp "$DIR/System/$c" "$SLOT_ROOT/System/$c"
	fi
done

cd "$DIR"
./slot --init-root "$SLOT_ROOT" > "$LOG" 2>&1
./slot >> "$LOG" 2>&1
sync
