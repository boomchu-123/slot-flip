#!/bin/sh
# slot. for the Miyoo Flip (my355).
#
# Runs from MinUI's Tools menu (`Tools/my355/Slot.pak`) and from the Flip's own
# launcher (`App/Slot`, which hands over to this script).
#
# Keeps all of its data in /mnt/sdcard/Slot so it never touches anyone else's
# Roms or Saves. Drop legally dumped ROMs into Slot/Games/<GBA|GB|GBC|...>/.

DIR="$(dirname "$0")"
SD="${SDCARD_PATH:-/mnt/sdcard}"
SLOT_ROOT="$SD/Slot"

export LD_LIBRARY_PATH="/usr/lib:/usr/miyoo/lib:$LD_LIBRARY_PATH"
export SLOT_ROOT

# First run on a card that has never held slot: the log's own folder does not
# exist yet, and a failed redirection would throw away the one thing worth
# reading when a first start goes wrong.
mkdir -p "$SLOT_ROOT"

LOG="$SLOT_ROOT/slot.log"

# Diagnostics are opt-in. Drop a file named DEBUG next to this script to turn on
# the verbose input/pacing trace while chasing a problem.
if [ -f "$DIR/DEBUG" ]; then
	export SLOT_TRACE=1
	export SLOT_TRACE_LATENCY=1
fi

# Emulator cores report problems (a game they will not open, and why) in the log by
# default. Drop a file named CORELOG here to see their routine messages too.
[ -f "$DIR/CORELOG" ] && export SLOT_CORE_LOG=1

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
for c in mgba_libretro.so gpsp_libretro.so pcsx_rearmed_libretro.so fceumm_libretro.so snes9x2005_plus_libretro.so genesis_plus_gx_libretro.so; do
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

# The pad has to be switched on before slot can read it. MinUI sets this to 0
# before it launches a tool; the Flip's own launcher leaves it at -1 ("none"),
# and then the kernel reports no button presses at all -- the game runs and
# nothing responds, with nothing in the log to say why. Switch it on for this
# run only, and hand the launcher's own value back on the way out so its menus
# keep working.
JOY_TYPE=/sys/class/miyooio_chr_dev/joy_type
JOY_NOTE=""
JOY_WAS=""
if [ -w "$JOY_TYPE" ]; then
	JOY_WAS=$(cut -d' ' -f1 "$JOY_TYPE" 2>/dev/null)
	if [ "$JOY_WAS" = "-1" ]; then
		if echo 0 > "$JOY_TYPE" 2>/dev/null; then
			JOY_NOTE="slot: pad joy_type was $JOY_WAS (off, stock launcher); set to 0 for this run"
		fi
	fi
fi

cd "$DIR" || exit 1
./slot --init-root "$SLOT_ROOT" > "$LOG" 2>&1
[ -n "$JOY_NOTE" ] && echo "$JOY_NOTE" >> "$LOG"
./slot >> "$LOG" 2>&1
sync

# Put the pad back the way the launcher left it. Only when this run changed it.
if [ -n "$JOY_NOTE" ]; then
	echo "$JOY_WAS" > "$JOY_TYPE" 2>/dev/null
fi
