"""Synthetic ELF tests; no toolchain, emulator, or game files needed."""
import importlib.util
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("armv7-fix-vfp-attr.py")
spec = importlib.util.spec_from_file_location("attributes", SCRIPT)
attributes = importlib.util.module_from_spec(spec)
spec.loader.exec_module(attributes)


def elf(pairs, flags=0x5000400):
    entries = b"".join(attributes.encode(t, v) for t, v in pairs)
    block = b"\x01" + struct.pack("<I", 5 + len(entries)) + entries
    blob = b"A" + struct.pack("<I", 10 + len(block)) + b"aeabi\0" + block
    names = b"\0.shstrtab\0.ARM.attributes\0"
    header = bytearray(52)
    header[:6] = b"\x7fELF\x01\x01"
    struct.pack_into("<H", header, 18, 40)
    struct.pack_into("<I", header, 32, 52)
    struct.pack_into("<I", header, 36, flags)
    struct.pack_into("<HHH", header, 46, 40, 3, 1)
    names_at = 52 + 3 * 40
    section = lambda name, off, size: struct.pack(
        "<IIIIIIIIII", name, 0, 0, 0, off, size, 0, 0, 1, 0
    )
    return (header + bytes(40) + section(1, names_at, len(names))
            + section(11, names_at + len(names), len(blob)) + names + blob)


class AttributesTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "core.so"

    def run_patch(self, data):
        self.path.write_bytes(data)
        return attributes.patch(self.path)

    def test_patch_is_in_place_and_idempotent(self):
        for hint in (68, 42):
            original = elf([(6, b"\x0a"), (hint, b"\x01")])
            self.assertEqual(self.run_patch(original), "patched")
            result = self.path.read_bytes()
            self.assertEqual(len(original), len(result))
            self.assertEqual(result, elf([(6, b"\x0a"), (28, b"\x01")]))
            self.assertEqual(attributes.patch(self.path), "already has it")
            self.assertEqual(self.path.read_bytes(), result)

    def test_compatibility_string_and_multibyte_tag_survive(self):
        pairs = [(6, b"\x0a"), (32, b"\x01vendor\0"), (68, b"\x01"), (128, b"\x02")]
        self.assertEqual(self.run_patch(elf(pairs)), "patched")
        expected = [(6, b"\x0a"), (28, b"\x01"), (32, b"\x01vendor\0"), (128, b"\x02")]
        self.assertEqual(self.path.read_bytes(), elf(expected))

    def test_refused_inputs_are_unchanged(self):
        for data in (b"", b"not ELF", elf([(28, b"\x00")]),
                     elf([(68, b"\x01")], flags=0x5000200), elf([(6, b"\x0a")])):
            self.assertNotIn(self.run_patch(data), ("patched", "already has it"))
            self.assertEqual(self.path.read_bytes(), data)

    def test_cli_fails_on_refusal_and_corruption(self):
        for data in (elf([(6, b"\x0a")]), elf([(68, b"\x01")])[:-1]):
            self.path.write_bytes(data)
            run = subprocess.run([sys.executable, str(SCRIPT), str(self.path)], capture_output=True)
            self.assertNotEqual(run.returncode, 0)
            self.assertEqual(self.path.read_bytes(), data)

    def test_cli_accepts_already_correct_library(self):
        self.path.write_bytes(elf([(28, b"\x01")]))
        run = subprocess.run([sys.executable, str(SCRIPT), str(self.path)], capture_output=True)
        self.assertEqual(run.returncode, 0, run.stderr)


if __name__ == "__main__":
    unittest.main()
