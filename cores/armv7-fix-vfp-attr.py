#!/usr/bin/env python3
"""Mark a 32-bit ARM hard-float shared library as such, in place.

Zig's linker drops the ARM build attribute Tag_ABI_VFP_args when any input lacks
it, and its bundled runtime objects do. The library is still hard-float (its
ELF flags say so), but glibc's loader checks the attribute and refuses the file
with a misleading "cannot open shared object file". This writes the tag back.

The section cannot grow, so the two bytes for it come from the last optional
hint at the end of the list (Tag_Virtualization_use, else Tag_MPextension_use),
which a loader never reads. The tags stay in ascending order. Does nothing to a
file that already has the tag or is not an ARM ELF file with attributes.
"""
import struct
import sys

VFP_ARGS = 28  # Tag_ABI_VFP_args
HINTS = (68, 42)  # Tag_Virtualization_use, Tag_MPextension_use
NTBS = {4, 5, 32, 65, 67}  # tags whose value is a NUL-terminated string


def uleb(data, i):
    value = shift = 0
    while True:
        byte = data[i]
        i += 1
        value |= (byte & 0x7F) << shift
        shift += 7
        if not byte & 0x80:
            return value, i


def parse(attrs):
    """The (tag, raw bytes of value) pairs of a file-attributes block."""
    out, i = [], 0
    while i < len(attrs):
        tag, i = uleb(attrs, i)
        start = i
        if tag in NTBS:
            i = attrs.index(0, i) + 1
        else:
            _, i = uleb(attrs, i)
        out.append((tag, attrs[start:i]))
    return out


def encode(tag, value):
    assert tag < 128
    return bytes([tag]) + value


def patch(path):
    with open(path, "rb") as f:
        data = bytearray(f.read())
    if data[:4] != b"\x7fELF" or data[4] != 1 or struct.unpack_from("<H", data, 18)[0] != 40:
        return "not a 32-bit ARM ELF file"
    shoff, = struct.unpack_from("<I", data, 32)
    shentsize, shnum, shstrndx = struct.unpack_from("<HHH", data, 46)
    if shoff == 0 or shnum == 0:
        return "no section headers"

    def section(i):
        return struct.unpack_from("<IIIIIIIIII", data, shoff + shentsize * i)

    names = section(shstrndx)[4]
    for i in range(shnum):
        s = section(i)
        end = data.index(0, names + s[0])
        if data[names + s[0] : end] == b".ARM.attributes":
            off, size = s[4], s[5]
            break
    else:
        return "no .ARM.attributes"

    blob = bytes(data[off : off + size])
    if blob[:1] != b"A":
        return "unknown attributes format"
    vendor_end = blob.index(0, 5) + 1
    if blob[5 : vendor_end - 1] != b"aeabi":
        return "no aeabi attributes"
    # sub-section: tag 1 (file), 4-byte length counting the tag and itself
    if blob[vendor_end] != 1:
        return "no file attributes"
    sub_len, = struct.unpack_from("<I", blob, vendor_end + 1)
    if vendor_end + sub_len != len(blob):
        return "more than one attributes block"
    head = blob[: vendor_end + 5]
    pairs = parse(blob[vendor_end + 5 :])
    if any(t == VFP_ARGS for t, _ in pairs):
        return "already has it"
    for hint in HINTS:
        if any(t == hint for t, _ in pairs):
            pairs = [(t, v) for t, v in pairs if t != hint]
            break
    else:
        return "no hint to give up for the tag"
    pairs.append((VFP_ARGS, b"\x01"))
    pairs.sort(key=lambda p: p[0])
    new = head + b"".join(encode(t, v) for t, v in pairs)
    if len(new) != len(blob):
        return f"size changed ({len(blob)} to {len(new)}), left alone"
    data[off : off + size] = new
    with open(path, "wb") as f:
        f.write(data)
    return "patched"


if __name__ == "__main__":
    for p in sys.argv[1:]:
        print(f"{p}: {patch(p)}", file=sys.stderr)
