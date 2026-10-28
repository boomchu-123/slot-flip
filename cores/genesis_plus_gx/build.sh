#!/bin/sh
set -eu

here="$(cd "$(dirname "$0")" && pwd)"

device_cflags="-mcpu=cortex-a53"

# Directories of the upstream archive that the libretro build never reads: prebuilt
# binaries for other platforms and other front ends' ports. The source shipped with
# the core leaves them out; everything the shipped binary is built from stays.
unused="builds gx psp2 gcw0 sdl wiki"

usage() {
	echo "usage: $0 stamp COMMIT | build COMMIT TARBALL WORKDIR OUT | trim COMMIT TARBALL OUT" >&2
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
	echo "source=https://github.com/libretro/Genesis-Plus-GX/archive/$1.tar.gz"
	echo "recipe=make -f Makefile.libretro platform=unix HAVE_CDROM=0"
	echo "device_cflags=$device_cflags"
	echo "shipped_source_omits=$unused"
	patches | while read -r p; do
		echo "patch=$(basename "$p") sha256:$(sha256 "$p")"
	done
}

build() {
	commit="$1" tarball="$2" work="$3" out="$4"
	src="$work/Genesis-Plus-GX-$commit"

	rm -rf "$work"
	mkdir -p "$work"
	tar -xzf "$tarball" -C "$work"
	if [ ! -f "$src/Makefile.libretro" ]; then
		echo "$tarball does not hold Genesis-Plus-GX-$commit/Makefile.libretro" >&2
		exit 1
	fi

	patches | while read -r p; do
		git -C "$src" apply -p1 "$p"
	done

	CFLAGS="$device_cflags" make -C "$src" -f Makefile.libretro platform=unix HAVE_CDROM=0 \
		-j"$(getconf _NPROCESSORS_ONLN)"

	mkdir -p "$(dirname "$out")"
	cp "$src/genesis_plus_gx_libretro.so" "$out"
	stamp "$commit" >"$out.meta"
}

# The archive that ships beside the core: the pinned one without what it never builds.
trim() {
	commit="$1" tarball="$2" out="$3"
	root="Genesis-Plus-GX-$commit"
	excludes=""
	for d in $unused; do
		excludes="$excludes --exclude=$root/$d"
	done
	mkdir -p "$(dirname "$out")"
	work="$(mktemp -d)"
	tar -xzf "$tarball" -C "$work"
	# shellcheck disable=SC2086
	tar -czf "$out" -C "$work" $excludes "$root"
	rm -rf "$work"
	tar -tzf "$out" | grep -q "^$root/Makefile.libretro$"
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
trim)
	[ $# -eq 4 ] && [ -n "$2" ] && [ -n "$3" ] && [ -n "$4" ] || usage
	trim "$2" "$3" "$4"
	;;
*)
	usage
	;;
esac
