import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from delta_container import DeltaContainerError, parse_records


def record(delta_id: int, payload: bytes, tick: int = 100, kind: int = 7) -> bytes:
    size = 24 + len(payload)
    return b"\xff\x02\x03\x00" + struct.pack(">IQII", size, delta_id, tick, kind) + payload


class DeltaContainerTests(unittest.TestCase):
    def test_parses_version_and_records(self):
        data = struct.pack("<I", 4) + record(10, b"abc") + record(12, b"defg")
        version, rows = parse_records(data)
        self.assertEqual(version, 4)
        self.assertEqual([(r.delta_id, r.size, r.payload) for r in rows], [(10, 27, b"abc"), (12, 28, b"defg")])

    def test_preserves_backwards_ids_in_file_order(self):
        _, rows = parse_records(struct.pack("<I", 4) + record(10, b"") + record(9, b""))
        self.assertEqual([r.delta_id for r in rows], [10, 9])

    def test_rejects_truncated_record(self):
        with self.assertRaises(DeltaContainerError):
            parse_records(struct.pack("<I", 4) + record(10, b"abc")[:-1])


if __name__ == "__main__":
    unittest.main()
