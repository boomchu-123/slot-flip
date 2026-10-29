#!/bin/sh
# slot. for the Miyoo A30 (my282), launched as a MinUI Tool.
#
# EXPERIMENTAL. The A30 has a Mali-400 GPU, so unlike the Mini Plus it uses the
# existing GLES renderer (via the fbdev/EGL surface). This has a real chance of
# working, but is unverified on hardware. If anything is wrong — black screen,
# wrong buttons, crash — grab slot.log and input-trace.log and share them. See
# TESTERS.md.
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
# the verbose pacing trace while chasing a problem.
if [ -f "$DIR/DEBUG" ]; then
	export SLOT_TRACE=1
	export SLOT_TRACE_LATENCY=1
fi

# The raw input trace is always on while the buttons are unconfirmed, so a report
# never lacks the one file that says what the device sends. It writes
# Slot/input-trace.log: every input node, then every event with its code.
export SLOT_TRACE_INPUT=1

# Optional isolation toggles. NO_CORE runs the built-in mock, SILENT uses a stub
# audio sink, TEXTEST shows a moving pattern instead of the game image — useful
# for proving the display path on new hardware.
[ -f "$DIR/NO_CORE" ] && export SLOT_NO_CORE=1
[ -f "$DIR/SILENT" ]  && export SLOT_SILENT=1
[ -f "$DIR/TEXTEST" ] && export SLOT_TEXTEST=1

# Seed the emulator cores into Slot/System, replacing a copy that differs so a
# core fix in a new pak reaches an existing card. The A30 is 32-bit ARM (armhf)
# with an old C library (2.23), so only cores built for that are bundled: gpSP
# (GBA) today. mGBA needs a newer C library than the device has and is not.
mkdir -p "$SLOT_ROOT/System"
for c in mgba_libretro.so gpsp_libretro.so; do
	if [ -f "$DIR/System/$c" ] && ! cmp -s "$DIR/System/$c" "$SLOT_ROOT/System/$c"; then
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
	# What the kernel says its input devices are, so a report needs no shell.
	printf 'slot: device: input devices\n'
	cat /proc/bus/input/devices 2>/dev/null | sed 's/^/slot: device:   /'
} >> "$LOG" 2>&1

# A report that ends in a forced power-off loses what was not yet flushed to
# the card (an earlier one came back with an empty log), so flush as it runs.
( while sleep 3; do sync; done ) &
SYNCER=$!

./slot --init-root "$SLOT_ROOT" >> "$LOG" 2>&1
./slot >> "$LOG" 2>&1
kill "$SYNCER" 2>/dev/null
sync
