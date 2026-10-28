#!/bin/sh
# slot. for the Miyoo A30 (my282), launched as a MinUI Tool.
#
# EXPERIMENTAL. The A30 has a Mali-400 GPU, so unlike the Mini Plus it uses the
# existing GLES renderer (via the fbdev/EGL surface). This has a real chance of
# working, but is unverified on hardware. If anything is wrong — black screen,
# wrong buttons, crash — grab slot.log (and input-trace.log with a DEBUG file)
# and share it. See TESTERS.md.
#
# Keeps all of its data in /mnt/SDCARD/Slot so it never touches your MinUI Roms
# or Saves. Drop legally dumped ROMs into Slot/Games/<GBA|GB|GBC>/.

DIR="$(dirname "$0")"
SD="${SDCARD_PATH:-/mnt/SDCARD}"
SLOT_ROOT="$SD/Slot"

# Mali-400 GLES lives in the Miyoo vendor lib dirs on this device; include the
# common locations so the EGL/GLES loader finds libEGL/libGLESv2.
export LD_LIBRARY_PATH="/usr/lib:/usr/miyoo/lib:/config/lib:$LD_LIBRARY_PATH"
export SLOT_ROOT

# The A30 exposes a portrait framebuffer. Clockwise output was upside down in
# the hardware report; use the opposite quarter-turn, preserving overrides.
export SLOT_ROTATE="${SLOT_ROTATE:-ccw}"

LOG="$SLOT_ROOT/slot.log"

# Diagnostics are opt-in. Drop a file named DEBUG next to this script to turn on
# the verbose input/pacing trace while chasing a problem.
if [ -f "$DIR/DEBUG" ]; then
	export SLOT_TRACE=1
	export SLOT_TRACE_LATENCY=1
fi

# Optional isolation toggles. NO_CORE runs the built-in mock, SILENT uses a stub
# audio sink, TEXTEST shows a moving pattern instead of the game image — useful
# for proving the display path on new hardware.
[ -f "$DIR/NO_CORE" ] && export SLOT_NO_CORE=1
[ -f "$DIR/SILENT" ]  && export SLOT_SILENT=1
[ -f "$DIR/TEXTEST" ] && export SLOT_TEXTEST=1

# Seed the emulator cores into Slot/System on first run. NOTE: the A30 is 32-bit
# ARM (armhf); it needs armhf cores, which are not bundled yet.
mkdir -p "$SLOT_ROOT/System"
for c in mgba_libretro.so gpsp_libretro.so; do
	if [ ! -f "$SLOT_ROOT/System/$c" ] && [ -f "$DIR/System/$c" ]; then
		cp "$DIR/System/$c" "$SLOT_ROOT/System/$c"
	fi
done

cd "$DIR"

# The package and the device's C library go first, so a report says which build
# it came from and why a binary that cannot load was refused.
printf 'slot: pak %s\n' "$(cat "$DIR/slot.version" 2>/dev/null || echo unknown)" > "$LOG"
{
	printf 'slot: device: kernel %s\n' "$(uname -r 2>/dev/null)"
	for f in /lib/libc.so.6 /lib/libc-*.so /lib/ld-*.so; do
		[ -e "$f" ] && printf 'slot: device: %s\n' "$(ls -l "$f" 2>/dev/null)"
	done
	printf 'slot: device: LD_LIBRARY_PATH=%s\n' "$LD_LIBRARY_PATH"
} >> "$LOG" 2>&1

./slot --init-root "$SLOT_ROOT" >> "$LOG" 2>&1
./slot >> "$LOG" 2>&1
sync
