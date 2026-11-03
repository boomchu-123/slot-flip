#!/bin/sh
set -eu

here="$(cd "$(dirname "$0")" && pwd)"

device_cflags="-mcpu=cortex-a53"

# 32-bit ARM (the Miyoo Mini Plus and the A30): SLOT_CORE_PLATFORM=armv7 and
# SLOT_ARMV7_CC=<path to cores/armv7-zigcc> build with Zig for glibc 2.17, which
# both consoles can load. The aarch64 flags above do not apply to it.
cross_make=""
cross_recipe=""
if [ "${SLOT_CORE_PLATFORM:-}" = "armv7" ]; then
	: "${SLOT_ARMV7_CC:?set SLOT_ARMV7_CC to cores/armv7-zigcc for an armv7 build}"
	device_cflags=""
	cross_dir="$(cd "$(dirname "$SLOT_ARMV7_CC")" && pwd)"
	cross_make="CC=$cross_dir/armv7-zigcc CXX=$cross_dir/armv7-zigc++ AR=$cross_dir/armv7-zigar"
	cross_recipe="cross armv7, zig cc, glibc 2.17: "
fi

usage() {
	echo "usage: $0 stamp COMMIT | build COMMIT TARBALL WORKDIR OUT" >&2
	exit 2
}

sha256() {
	if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi |
		cut -d' ' -f1
}

# No patches yet; the glob stays literal when nothing matches, so skip it.
patches() {
	for p in "$here"/*.patch; do
		[ -e "$p" ] && echo "$p"
	done
	return 0
}

stamp() {
	echo "commit=$1"
	echo "source=https://github.com/libretro/snes9x2005/archive/$1.tar.gz"
	echo "recipe=${cross_recipe}make -f Makefile platform=unix USE_BLARGG_APU=1"
	echo "device_cflags=$device_cflags"
	patches | while read -r p; do
		echo "patch=$(basename "$p") sha256:$(sha256 "$p")"
	done
}

build() {
	commit="$1" tarball="$2" work="$3" out="$4"
	src="$work/snes9x2005-$commit"

	rm -rf "$work"
	mkdir -p "$work"
	tar -xzf "$tarball" -C "$work"
	if [ ! -f "$src/Makefile" ]; then
		echo "$tarball does not hold snes9x2005-$commit/Makefile" >&2
		exit 1
	fi

	patches | while read -r p; do
		git -C "$src" apply -p1 "$p"
	done

	CFLAGS="$device_cflags" make $cross_make -C "$src" -f Makefile platform=unix USE_BLARGG_APU=1 \
		-j"$(getconf _NPROCESSORS_ONLN)"

	mkdir -p "$(dirname "$out")"
	cp "$src/snes9x2005_plus_libretro.so" "$out"
	stamp "$commit" >"$out.meta"
}

case "${1:-}" in
stamp)
	[ $# -eq 2 ] && [ -n "$2" ] || usage
	stamp "$2"
	;;
build)
	[ $# -eq 5 ] && [ -n "$2" ] && [ -n "$3" ] && [ -n "$4" ] && [ -n "$5" ] || usage
	build "$2" "$3" "$4" "$5"
	;;
*)
	usage
	;;
esac
