#!/bin/sh
set -eu

here="$(cd "$(dirname "$0")" && pwd)"

device_cflags="-flto=auto"

# armv7 cross-builds for the 32-bit pair. Unset keeps the host recipe the
# committed .meta records, byte for byte.
cross_armv7=0
if [ "${SLOT_CORE_PLATFORM:-}" = "armv7" ]; then
	cross_armv7=1
	# The armv7 recipe passes its own flags to cmake; -flto=auto is the
	# aarch64 host build's and must not be recorded as if it applied.
	device_cflags=""
fi

usage() {
	echo "usage: $0 stamp COMMIT | build COMMIT WORKDIR OUT" >&2
	exit 2
}

sha256() {
	if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi |
		cut -d' ' -f1
}

stamp() {
	echo "commit=$1"
	echo "source=https://github.com/libretro/mgba/tree/$1"
	echo "device_cflags=$device_cflags"
	[ "$cross_armv7" = 1 ] && echo "recipe=cross armv7"
	for p in "$here"/*.patch; do
		echo "patch=$(basename "$p") sha256:$(sha256 "$p")"
	done
}

build() {
	commit="$1" work="$2" out="$3"
	src="$work/mgba"
	obj="$work/build"

	if ! command -v cmake >/dev/null 2>&1; then
		if command -v apt-get >/dev/null 2>&1; then
			sed -i '/bullseye-security/d' /etc/apt/sources.list
			apt-get -o Acquire::Check-Valid-Until=false update -qq
			apt-get install -y -qq --no-install-recommends cmake >/dev/null
		else
			echo "building mGBA needs cmake: brew install cmake" >&2
			exit 1
		fi
	fi

	mkdir -p "$src"
	[ -d "$src/.git" ] || git init -q "$src"
	git -C "$src" cat-file -e "$commit^{commit}" 2>/dev/null ||
		git -C "$src" fetch -q --depth 1 https://github.com/libretro/mgba "$commit"
	git -C "$src" checkout -q --force --detach "$commit"
	git -C "$src" clean -q -fdx
	for p in "$here"/*.patch; do
		git -C "$src" apply "$p"
	done

	cflags=""
	if [ "$(uname -s)-$(uname -m)" = "Linux-aarch64" ]; then
		cflags="$device_cflags"
	fi
	set -- -DLIBMGBA_ONLY=ON -DBUILD_LIBRETRO=ON -DCMAKE_BUILD_TYPE=Release
	if [ "$cross_armv7" = 1 ]; then
		armflags="-march=armv7ve -mfpu=neon-vfpv4 -mfloat-abi=hard"
		set -- "$@" -DCMAKE_SYSTEM_NAME=Linux -DCMAKE_SYSTEM_PROCESSOR=arm \
			-DCMAKE_C_COMPILER=arm-linux-gnueabihf-gcc \
			-DCMAKE_CXX_COMPILER=arm-linux-gnueabihf-g++ \
			-DCMAKE_C_FLAGS="$armflags" -DCMAKE_CXX_FLAGS="$armflags"
	else
		set -- "$@" -DCMAKE_C_FLAGS="$cflags"
	fi
	cmake -S "$src" -B "$obj" "$@" >/dev/null
	cmake --build "$obj" --target mgba_libretro --parallel "$(getconf _NPROCESSORS_ONLN)" >/dev/null

	for ext in dylib so; do
		if [ -f "$obj/mgba_libretro.$ext" ]; then
			mkdir -p "$(dirname "$out")"
			cp "$obj/mgba_libretro.$ext" "$out"
			stamp "$commit" >"$out.meta"
			return
		fi
	done
	echo "cmake finished without producing mgba_libretro" >&2
	exit 1
}

case "${1:-}" in
stamp)
	[ $# -eq 2 ] && [ -n "$2" ] || usage
	stamp "$2"
	;;
build)
	[ $# -eq 4 ] && [ -n "$2" ] && [ -n "$3" ] && [ -n "$4" ] || usage
	build "$2" "$3" "$4"
	;;
*)
	usage
	;;
esac
