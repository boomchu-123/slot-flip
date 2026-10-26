#!/bin/sh
# Started by the Miyoo Mini Plus' own launcher from its App list. The frontend
# itself lives in the pak; this only hands over, so there is one launch script
# to maintain and the MinUI Tools entry runs exactly the same code.

SD="${SDCARD_PATH:-/mnt/SDCARD}"
PAK="$SD/Tools/miyoomini/Slot.pak"

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
