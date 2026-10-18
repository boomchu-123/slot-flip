#!/bin/sh
# slot. for the Miyoo Flip (my355), launched as a MinUI Tool.
#
# Keeps all of its data in /mnt/sdcard/Slot so it never touches your MinUI Roms
# or Saves. Drop legally dumped ROMs into Slot/Games/<GBA|GB|GBC>/.

DIR="$(dirname "$0")"
SD="${SDCARD_PATH:-/mnt/sdcard}"
SLOT_ROOT="$SD/Slot"

export LD_LIBRARY_PATH="/usr/lib:/usr/miyoo/lib:$LD_LIBRARY_PATH"
export SLOT_ROOT

LOG="$SLOT_ROOT/slot.log"

# Diagnostics are opt-in. Drop a file named DEBUG next to this script to turn on
# the verbose input/pacing trace while chasing a problem.
if [ -f "$DIR/DEBUG" ]; then
	export SLOT_TRACE=1
	export SLOT_TRACE_LATENCY=1
fi

# Optional isolation toggles (drop an empty file with the matching name in the
# pak). NO_CORE runs the built-in mock, SILENT uses a stub audio sink, TEXTEST
# shows a moving pattern instead of the game image.
[ -f "$DIR/NO_CORE" ] && export SLOT_NO_CORE=1
[ -f "$DIR/SILENT" ]  && export SLOT_SILENT=1
[ -f "$DIR/TEXTEST" ] && export SLOT_TEXTEST=1

# Keep the emulator cores in Slot/System in step with the ones shipped in this
# pak: seeded on first run, and refreshed whenever an update changes a core, so
# core fixes actually reach people who already have Slot installed.
mkdir -p "$SLOT_ROOT/System"
for c in mgba_libretro.so gpsp_libretro.so; do
	if [ -f "$DIR/System/$c" ] && ! cmp -s "$DIR/System/$c" "$SLOT_ROOT/System/$c"; then
		cp "$DIR/System/$c" "$SLOT_ROOT/System/$c"
	fi
done

# Turn the green power light on and stop the kernel driving the charger light
# red from the battery trigger.
echo none > /sys/class/leds/charger/trigger 2>/dev/null
echo 0    > /sys/class/leds/charger/brightness 2>/dev/null
echo none > /sys/class/leds/work/trigger 2>/dev/null
echo 255  > /sys/class/leds/work/brightness 2>/dev/null

cd "$DIR"
./slot --init-root "$SLOT_ROOT" > "$LOG" 2>&1
./slot >> "$LOG" 2>&1
sync
