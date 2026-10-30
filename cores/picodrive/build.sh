#!/bin/sh
# PicoDrive, for the Miyoo Mini Plus (32-bit ARM). It plays Mega Drive, Master
# System and Game Gear, which Genesis Plus GX also does, and on that console the two
# differ by a lot: measured on a Mini Plus, Sonic the Hedgehog runs at 58 frames a
# second under PicoDrive against 51 under Genesis Plus GX, and Master System and
# Game Gear at 60.1 against 59. PicoDrive's CPU cores are written in ARM assembly.
#
# There is only a 32-bit ARM build: SLOT_CORE_PLATFORM=armv7 and
# SLOT_ARMV7_CC=<path to cores/armv7-zigcc>, as for the other cores. It also needs
# gcc-arm-linux-gnueabihf (see cores/picodrive/cc for why) and a host g++, which
# builds the generator that writes the 68000 core's assembly.
set -eu

here="$(cd "$(dirname "$0")" && pwd)"

usage() {
	echo "usage: $0 stamp COMMIT | fetch COMMIT OUT.tar.gz | build COMMIT TARBALL WORKDIR OUT" >&2
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
	echo "source=https://github.com/libretro/picodrive at $1, with its submodules (cores/picodrive/build.sh fetch)"
	echo "recipe=cross armv7, zig cc, glibc 2.17, assembly by GNU as: make -f Makefile.libretro platform=unix"
	echo "device_cflags="
	patches | while read -r p; do
		echo "patch=$(basename "$p") sha256:$(sha256 "$p")"
	done
}

# GitHub's archive of a commit leaves out the submodules (the 68000 core, libchdr,
# emu2413 and others), and the build needs them. This is the same archive, with
# them, and without the git metadata.
fetch() {
	commit="$1" out="$2"
	tmp="$(mktemp -d)"
	git clone -q https://github.com/libretro/picodrive "$tmp/picodrive"
	git -C "$tmp/picodrive" checkout -q "$commit"
	git -C "$tmp/picodrive" submodule update -q --init --recursive
	mkdir -p "$tmp/pack"
	mv "$tmp/picodrive" "$tmp/pack/picodrive-$commit"
	find "$tmp/pack" -name '.git*' -prune -exec rm -rf {} +
	mkdir -p "$(dirname "$out")"
	tar -czf "$out" -C "$tmp/pack" "picodrive-$commit"
	rm -rf "$tmp"
}

build() {
	commit="$1" tarball="$2" work="$3" out="$4"
	src="$work/picodrive-$commit"

	[ "${SLOT_CORE_PLATFORM:-}" = "armv7" ] || {
		echo "PicoDrive is built only for 32-bit ARM: set SLOT_CORE_PLATFORM=armv7" >&2
		exit 2
	}
	: "${SLOT_ARMV7_CC:?set SLOT_ARMV7_CC to cores/armv7-zigcc}"
	cross_dir="$(cd "$(dirname "$SLOT_ARMV7_CC")" && pwd)"

	rm -rf "$work"
	mkdir -p "$work"
	tar -xzf "$tarball" -C "$work"
	if [ ! -f "$src/Makefile.libretro" ]; then
		echo "$tarball does not hold picodrive-$commit/Makefile.libretro" >&2
		exit 1
	fi

	patches | while read -r p; do
		git -C "$src" apply -p1 "$p"
	done

	# The compilers go in the environment, not on the command line: a command-line
	# CXX would be handed down to the generator's own make and override the host
	# compiler it has to be built with.
	CC="$here/cc" CXX="$cross_dir/armv7-zigc++" AR="$cross_dir/armv7-zigar" \
		make -C "$src" -f Makefile.libretro platform=unix \
		CYCLONE_CC=gcc CYCLONE_CXX=g++ \
		-j"$(getconf _NPROCESSORS_ONLN)"

	mkdir -p "$(dirname "$out")"
	cp "$src/picodrive_libretro.so" "$out"
	stamp "$commit" >"$out.meta"
}

case "${1:-}" in
stamp)
	[ $# -eq 2 ] && [ -n "$2" ] || usage
	stamp "$2"
	;;
fetch)
	[ $# -eq 3 ] && [ -n "$2" ] && [ -n "$3" ] || usage
	fetch "$2" "$3"
	;;
build)
	[ $# -eq 5 ] && [ -n "$2" ] && [ -n "$3" ] && [ -n "$4" ] && [ -n "$5" ] || usage
	build "$2" "$3" "$4" "$5"
	;;
*)
	usage
	;;
esac
