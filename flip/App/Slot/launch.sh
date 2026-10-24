#!/bin/sh
# Started by the Flip's own launcher from its App list. The frontend itself
# lives in the pak; this only hands over, so there is one launch script to
# maintain and MinUI's Tools entry runs exactly the same code.
#
# The stock launcher runs this through /tmp/cmd_to_run.sh and returns to its
# menu when it exits.

SD="${SDCARD_PATH:-/mnt/sdcard}"
PAK="$SD/Tools/my355/Slot.pak"

if [ ! -x "$PAK/launch.sh" ]; then
	# Say so somewhere the user can find it, rather than exiting into a menu
	# that looks like nothing happened.
	msg="slot: $PAK/launch.sh is missing; the pak was not copied to the card"
	mkdir -p "$SD/Slot" 2>/dev/null
	echo "$msg" >> "$SD/Slot/slot.log" 2>/dev/null
	echo "$msg" >> "$SD/slot-launch.log" 2>/dev/null
	exit 1
fi

exec "$PAK/launch.sh"
