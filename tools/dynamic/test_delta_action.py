import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from delta_action import DeltaDecodeError, decode_action_header


def encode(action_id, data1, data2):
    raw1 = 0xFFFF if data1 == -1 else 0xFFFE if not 0 <= data1 < 0xFFFE else data1
    raw2 = 0x3F if data2 == -1 else 0x3E if not 0 <= data2 < 0x3E else data2
    header = raw1 | (((action_id | 0xC0) << 8 | raw2) << 16)
    extra = (struct.pack(">i", data1) if raw1 == 0xFFFE else b"")
    extra += struct.pack(">i", data2) if raw2 == 0x3E else b""
    return struct.pack(">I", header) + extra


class DeltaActionTests(unittest.TestCase):
    def test_inline_fields(self):
        result = decode_action_header(encode(7, 123, 12), stream_version=26)
        self.assertEqual((result.action_id, result.data1, result.data2), (7, 123, 12))
        self.assertEqual(result.header_size, 4)

    def test_extended_and_negative_fields(self):
        result = decode_action_header(encode(4, 70000, -1), stream_version=26)
        self.assertEqual((result.action_id, result.data1, result.data2), (4, 70000, -1))
        self.assertEqual(result.header_size, 8)

    def test_invalid_marker(self):
        with self.assertRaises(DeltaDecodeError):
            decode_action_header(b"\x00\x00\x00\x00", stream_version=27)

    def test_truncated_extension(self):
        with self.assertRaises(DeltaDecodeError):
            decode_action_header(encode(4, 70000, 1)[:4], stream_version=26)

    def test_retail_62_and_63_are_inline_not_markers(self):
        # Independent bytes derived from the retail encoder's shifts/ORs.
        for value in (62, 63, 253):
            data = bytes([0xC1, value, 0, 0])
            result = decode_action_header(data, stream_version=27)
            self.assertEqual((result.data1, result.data2, result.header_size), (0, value, 4))
        self.assertEqual(decode_action_header(bytes.fromhex('c13f0000'), stream_version=26).data2, -1)

    def test_retail_extended_and_sentinel(self):
        result = decode_action_header(bytes.fromhex('c1fe00000000012c'), stream_version=27)
        self.assertEqual((result.data2, result.header_size), (300, 8))
        self.assertEqual(decode_action_header(bytes.fromhex('c1ff0000'), stream_version=27).data2, -1)
        with self.assertRaises(DeltaDecodeError):
            decode_action_header(bytes.fromhex('c1fe0000'), stream_version=27)

    def test_both_extensions_preserve_order(self):
        result = decode_action_header(bytes.fromhex('c1fefffe000111700000012c'), stream_version=27)
        self.assertEqual((result.data1, result.data2, result.header_size), (70000, 300, 12))

    def test_version_is_required(self):
        with self.assertRaises(TypeError):
            decode_action_header(bytes.fromhex('c1000000'))
        with self.assertRaises(DeltaDecodeError):
            decode_action_header(bytes.fromhex('c1000000'), stream_version=-1)


if __name__ == "__main__":
    unittest.main()
