#!/bin/sh
# Test only the stock App entry's handoff, never a real device or audio backend.
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/slot-mini-app.XXXXXX")"
trap 'rm -rf "$work"' EXIT HUP INT TERM
card="$work/card with spaces"
mkdir -p "$card/Slot/Saves" "$card/Slot/Config"
printf 'existing save\n' > "$card/Slot/Saves/keep.sav"
printf 'existing settings\n' > "$card/Slot/Config/keep.txt"
app="$root/flip/App-miyoomini/Slot/launch.sh"

# Missing pak: a useful error, no change to saves/settings.
if SDCARD_PATH="$card" sh "$app"; then
    echo 'missing pak unexpectedly succeeded' >&2
    exit 1
fi
grep -q 'pak was not copied' "$card/Slot/slot.log"
grep -q 'pak was not copied' "$card/slot-launch.log"

# An installed pak is executed directly; preserve the card path and status.
mkdir -p "$card/Tools/miyoomini/Slot.pak"
cat > "$card/Tools/miyoomini/Slot.pak/launch.sh" <<'PAK'
#!/bin/sh
printf '%s\n' "$SDCARD_PATH" > "$SDCARD_PATH/handoff"
exit 23
PAK
chmod +x "$card/Tools/miyoomini/Slot.pak/launch.sh"
status=0
SDCARD_PATH="$card" sh "$app" || status=$?
test "$status" = 23
test "$(cat "$card/handoff")" = "$card"
test "$(cat "$card/Slot/Saves/keep.sav")" = 'existing save'
test "$(cat "$card/Slot/Config/keep.txt")" = 'existing settings'
echo 'Mini Plus App handoff: missing pak, spaced path, exit status and user data: pass'
