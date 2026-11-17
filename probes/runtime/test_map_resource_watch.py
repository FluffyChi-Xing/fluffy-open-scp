import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

from map_resource_watch import Grid, capture, changes, sample


COAL = 0xD779C976


class FakeReader:
    def __init__(self):
        self.regions = {}

    def words(self, address, *values):
        self.regions[address] = struct.pack(f"<{len(values)}I", *values)

    def read(self, address, length):
        for start, data in self.regions.items():
            if start <= address and address + length <= start + len(data):
                return data[address - start:address - start + length]
        raise OSError("address absent")

    def u32(self, address):
        return struct.unpack("<I", self.read(address, 4))[0]


class MapResourceWatchTest(unittest.TestCase):
    def test_reads_map_by_script_index_and_rejects_changed_city(self):
        reader = FakeReader()
        base, game, scripts, graphics = 0x400000, 0x10000, 0x20000, 0x30000
        reader.words(game, base + 0x91B2F0)
        reader.words(game + 0x10, scripts)
        reader.words(scripts, 0x40000)
        reader.words(0x400D0, base + 0x1F76B0)
        reader.words(graphics, base + 0x91F8A4, 0, 0, game)
        reader.words(game + 0x3D4, 0xB6)
        reader.words(game + 0x17A0, 47)
        reader.words(game + 0x18A4, 0x50000, 0x50004)
        reader.words(game + 0x18D4, 0x51000, 0x51004)
        reader.words(0x50000, 0x12952639)
        reader.words(0x51000, 0x3F000000)
        reader.words(game + 0x544, 0x60000, 0x603C0)
        reader.words(scripts + 0x154, 0x62000, 0x62008)
        reader.words(0x62000, COAL, 1)
        reader.words(0x601E0, 1, 0, 2, 1, 0x63000, 0x63008)
        reader.words(0x63000, 500, 200)

        args = (reader, base, game, scripts, graphics, 0xB6)
        self.assertEqual(sample(*args), (47, 43200.0, {
            COAL: Grid(2, 1, struct.pack("<ii", 500, 200)),
        }))
        reader.words(graphics, base + 0x91F8A4, 0, 0, 0xA0000)
        with self.assertRaisesRegex(ValueError, "asset_city_changed"):
            sample(*args)

    def test_logs_only_changes_including_zero_net_redistribution(self):
        before = {COAL: Grid(2, 1, struct.pack("<ii", 500, 200))}
        after = {COAL: Grid(2, 1, struct.pack("<ii", 400, 300))}
        self.assertEqual(list(changes(before, before)), [])
        reader = FakeReader()
        reader.words(0x103D4, 0xB6)
        state = dict(ready=False, samples=0, skippedSamples=0)
        with tempfile.TemporaryDirectory() as path, patch(
            "map_resource_watch.sample", side_effect=[(1, 100.0, before), (2, 101.0, after)]
        ), patch("map_resource_watch.time.monotonic", side_effect=[0, 0, 1, 3]), patch(
            "map_resource_watch.time.sleep"
        ):
            capture(reader, 0x400000, (0x10000, 0x20000, 0x30000), 2, Path(path), state, lambda: None)
            row, = [json.loads(line) for line in (Path(path) / "trace.jsonl").read_text().splitlines()]
            self.assertEqual((row["name"], row["simTicks"], row["environment"]), ("coal", 2, "000000B6"))
            self.assertEqual((row["changedCells"], row["increased"], row["decreased"], row["amountDelta"]),
                             (2, 100, 100, 0))
            self.assertEqual(row["cellExamples"], [dict(x=0, y=0, delta=-100), dict(x=1, y=0, delta=100)])
