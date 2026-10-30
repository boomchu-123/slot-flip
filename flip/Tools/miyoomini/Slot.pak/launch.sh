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
export LD_LIBRARY_PATH="/usr/lib:/usr/miyoo/lib:/config/lib:/customer/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
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
# console's audio server (the /dev/dsp the launcher's own sound uses, answered by
# the preloaded libpadsp), then tries the vendor audio calls, then ALSA. A file
# Config/audio holding "alsa" makes it use ALSA straight away; "vendor" tries the
# vendor calls first; "takeover" makes it stop the audio server and take the
# sound device, which silences the console's own sounds until slot exits.
# Comparing them is a one-line change, not a rebuild.
if [ -f "$SLOT_ROOT/Config/audio" ]; then
	SLOT_AUDIO="$(tr -cd 'a-zA-Z' < "$SLOT_ROOT/Config/audio")"
	[ -z "$SLOT_AUDIO" ] || export SLOT_AUDIO
fi

# Give mGBA the ROM's path and not its bytes, for a ROM of 8 MiB or more. It
# copies what it is given into a memory map of its own, and the console has too
# little memory for that on a large ROM (Pokemon Emerald, 16 MB, crashed it the
# moment it loaded). Smaller ROMs load as before.
export SLOT_LOAD_BY_PATH=mGBA

# The Genesis core carries 35 MB of zero-filled memory and the PlayStation core
# 23 MB. On a console with about 100 MB the kernel's default heuristic refuses
# to map that once the frontend is resident ("cannot map zero-fill pages"), so
# three of the nine systems never load. Zero pages cost nothing until a core
# writes to them; the price is that real exhaustion ends in the kernel's
# out-of-memory kill rather than a failed load.
[ ! -w /proc/sys/vm/overcommit_memory ] || echo 1 > /proc/sys/vm/overcommit_memory

# Seed the emulator cores into Slot/System, replacing a copy that differs so a
# core fix in a new pak reaches an existing card. `cmp -s` is the test the Flip
# pak uses for the same reason.
mkdir -p "$SLOT_ROOT/System"
for c in mgba_libretro.so gpsp_libretro.so pcsx_rearmed_libretro.so fceumm_libretro.so snes9x2005_plus_libretro.so genesis_plus_gx_libretro.so; do
	if [ -f "$DIR/System/$c" ] && ! cmp -s "$DIR/System/$c" "$SLOT_ROOT/System/$c"; then
		cp "$DIR/System/$c" "$SLOT_ROOT/System/$c"
	fi
done

cd "$DIR"
# Keep the last run's log: a report of a crash is usually about the run before
# this one, which relaunching would otherwise overwrite.
[ ! -s "$LOG" ] || mv -f "$LOG" "$LOG.prev"
# The package name goes first so a report can never be mistaken for another
# build again: the first tester log was from the previous package and nothing
# in it said so.
printf 'slot: pak %s\n' "$(cat "$DIR/slot.version" 2>/dev/null || echo unknown)" > "$LOG"

# What the launcher that started us has set up. This runs under MinUI today and
# is meant to run from the console's own launcher too, which gives an app a
# different environment, so the report says which one this was: the library
# path and preload the sound output depends on, whether the vendor audio server
# is running, and what the CPU is doing before and after anything here touches it.
{
	printf 'slot: launcher: %s\n' "$([ -n "$SYSTEM_PATH" ] && echo "MinUI ($SYSTEM_PATH)" || echo "not MinUI (no SYSTEM_PATH)")"
	printf 'slot: launcher: PATH=%s\n' "$PATH"
	printf 'slot: launcher: LD_PRELOAD=%s\n' "${LD_PRELOAD:-}"
	printf 'slot: launcher: LD_LIBRARY_PATH=%s\n' "$LD_LIBRARY_PATH"
	printf 'slot: launcher: audioserver %s\n' "$(pidof audioserver audioserver.mod 2>/dev/null | tr '\n' ' ' | grep . || echo 'not running')"
	printf 'slot: launcher: kernel %s\n' "$(uname -r 2>/dev/null)"
	for f in scaling_governor scaling_cur_freq scaling_max_freq; do
		printf 'slot: launcher: cpu %s %s\n' "$f" "$(cat /sys/devices/system/cpu/cpufreq/policy0/$f 2>/dev/null || echo unreadable)"
	done
} >> "$LOG" 2>&1

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
printf 'slot: launcher: cpu now %s kHz\n' "$(cat /sys/devices/system/cpu/cpufreq/policy0/scaling_cur_freq 2>/dev/null || echo unreadable)" >> "$LOG"

# glibc keeps freed memory in per-thread arenas and sizes text/icons by the
# heap, which on this console is the difference between a PlayStation game
# running and the kernel killing the frontend. Measured on the Mini Plus: the
# shelf rested at 38.8 MB and plays out at 32.3 with these three; two arenas is
# enough for nine threads and still beats one on the emulator's own thread.
export MALLOC_ARENA_MAX=2
# Above 64 KiB every image goes to its own mapping, so freeing a label or a
# shelf page gives the memory back instead of leaving it in the heap.
export MALLOC_MMAP_THRESHOLD_=65536
export MALLOC_TRIM_THRESHOLD_=65536

./slot --init-root "$SLOT_ROOT" >> "$LOG" 2>&1
./slot >> "$LOG" 2>&1
sync
