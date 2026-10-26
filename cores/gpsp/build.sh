#!/bin/sh
set -eu

here="$(cd "$(dirname "$0")" && pwd)"

device_cflags="-mcpu=cortex-a53"

# arm64 is the Flip, the recipe every committed .meta records. armv7 is the
# 32-bit pair (Mini Plus, A30): the same core with its ARM dynarec, built by a
# cross compiler the caller puts on CC/CXX (arm-linux-gnueabihf-gcc and -g++).
# Set SLOT_CORE_PLATFORM=armv7; leaving it unset must keep the arm64 stamp
# byte-identical, because CI compares the committed meta against it.
make_platform=arm64
if [ "${SLOT_CORE_PLATFORM:-arm64}" = "armv7" ]; then
	make_platform=classic_armv7_a7
	device_cflags=""
fi

usage() {
	echo "usage: $0 stamp COMMIT | build COMMIT TARBALL WORKDIR OUT" >&2
	exit 2
}

sha256() {
	if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi |
		cut -d' ' -f1
}

stamp() {
	echo "commit=$1"
	echo "source=https://github.com/libretro/gpsp/archive/$1.tar.gz"
	echo "recipe=make platform=$make_platform"
	echo "device_cflags=$device_cflags"
	for p in "$here"/*.patch; do
		echo "patch=$(basename "$p") sha256:$(sha256 "$p")"
	done
}

build() {
	commit="$1" tarball="$2" work="$3" out="$4"
	src="$work/gpsp-$commit"

	rm -rf "$work"
	mkdir -p "$work"
	tar -xzf "$tarball" -C "$work"
	if [ ! -f "$src/Makefile" ]; then
		echo "$tarball does not hold gpsp-$commit/Makefile" >&2
		exit 1
	fi

	for p in "$here"/*.patch; do
		git -C "$src" apply -p1 "$p"
	done

	# The armv7 block adds `-flto=4 -fwhole-program`, and with the dynarec in
	# the same binary that combination segfaults on first frame (measured under
	# qemu; the arm64 recipe's -flto=auto does not). Drop those three flags for
	# armv7 only.
	if [ "$make_platform" = "classic_armv7_a7" ]; then
		sed -i 's/-flto=4 -fwhole-program -fuse-linker-plugin //' "$src/Makefile"
	fi

	CFLAGS="$device_cflags" make -C "$src" platform="$make_platform" \
		GIT_VERSION="\"$(printf %s "$commit" | cut -c1-7)\"" \
		-j"$(getconf _NPROCESSORS_ONLN)"

	mkdir -p "$(dirname "$out")"
	cp "$src/gpsp_libretro.so" "$out"
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
