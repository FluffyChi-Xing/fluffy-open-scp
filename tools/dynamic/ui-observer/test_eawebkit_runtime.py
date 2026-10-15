"""Checks for false object matches and corrupted/cyclic remote structures."""
import struct
import unittest

import eawebkit_runtime as runtime


class Memory:
    def __init__(self):
        self.blocks = {}

    def put(self, address, data):
        self.blocks[address] = data

    def words(self, address, *values):
        self.put(address, struct.pack("<" + "I" * len(values), *values))

    def read(self, address, size):
        for start, data in self.blocks.items():
            if start <= address and address + size <= start + len(data):
                return data[address - start:address - start + size]
        raise OSError("unreadable_memory")

    def u32(self, address):
        return struct.unpack("<I", self.read(address, 4))[0]

    def locate(self, address):
        return None


class RuntimeSurveyTests(unittest.TestCase):
    def view_memory(self):
        memory = Memory()
        # A real x86 view can reside above 2 GiB. The URI cache may be empty.
        view, private, page = 0xF0001000, 0xF0002000, 0xF0003000
        table = 0x3983C2B8
        memory.words(view, table, private)
        memory.words(private, view, page, private + 0x1C, private + 0x1C)
        memory.words(private + 0x234, 0)
        memory.words(page, 0xF0004000, 0)
        memory.words(0xF000400C, 0xF0005000)
        memory.words(0xF0005000, 0xF0006000)
        memory.words(0xF000601C, 0xF0007000)
        uri = "game://localhost/resource/WebKitPanel.html".encode("utf-16-le")
        memory.words(0xF0007004, len(uri) // 2, 0xF0008000)
        memory.put(0xF0008000, uri)
        return memory, view, private, table

    def test_high_address_view_reads_current_uri_even_with_empty_cache(self):
        memory, view, _, table = self.view_memory()
        result = runtime.validate_view(memory, view, table)
        self.assertEqual(result["cachedURI"], "")
        self.assertEqual(result["currentFrameURI"], "game://localhost/resource/WebKitPanel.html")
        self.assertTrue(result["currentURIVerified"])

    def test_incidental_vtable_word_is_rejected_without_reverse_pointer(self):
        memory, view, private, table = self.view_memory()
        data = bytearray(memory.blocks[private])
        struct.pack_into("<I", data, 0, view + 4)
        memory.put(private, bytes(data))
        self.assertIsNone(runtime.validate_view(memory, view, table))

    def test_corrupt_string_length_is_rejected_before_large_read(self):
        memory, view, _, table = self.view_memory()
        memory.words(0xF0007004, 0xFFFFFFFF, 0xF0008000)
        with self.assertRaisesRegex(ValueError, "invalid"):
            runtime.validate_view(memory, view, table)

    def test_callback_registry_cycle_is_rejected(self):
        memory = Memory()
        memory.words(0x100020, 0x200000, 1, 1)
        memory.words(0x200000, 0x300000)
        memory.words(0x300000, 0x400000, 0x40000F, 0x400010, 0, 0, 0x300000)
        memory.put(0x400000, b"PostGameCommand")
        with self.assertRaisesRegex(ValueError, "invalid"):
            runtime.registered_callbacks(memory, 0x100000)

    def test_localizations_match(self):
        self.assertEqual(runtime.TEXT["zh-CN"].keys(), runtime.TEXT["en-US"].keys())


if __name__ == "__main__":
    unittest.main()
