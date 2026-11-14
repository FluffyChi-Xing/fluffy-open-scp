import struct
import unittest

from asset_watch import GAME_VTABLE, GRAPHICS_GAME_VTABLE, INTERIORS_VTABLE, changes, sample, select_city, vehicle_inventory


class Memory:
    def __init__(self):
        self.blocks = {}

    def put(self, address, fmt, *values):
        self.blocks[address] = struct.pack(fmt, *values)

    def read(self, address, size):
        for start, data in self.blocks.items():
            if start <= address and address + size <= start + len(data):
                return data[address - start:address - start + size]
        raise OSError("unmapped")

    def u32(self, address):
        return struct.unpack("<I", self.read(address, 4))[0]


class AssetWatchTest(unittest.TestCase):
    def test_selects_rendered_city_from_two_candidates_in_one_process(self):
        mem = Memory()
        base, rendered, other, scripts = 0x400000, 0x10000, 0x20000, 0x30000
        graphics = 0x40000
        candidates = {other: scripts, rendered: scripts}
        for game in candidates:
            mem.put(game, "<I", base + GAME_VTABLE)
            mem.put(game + 0x10, "<I", scripts)
        mem.put(scripts, "<I", 0x50000)
        mem.put(0x500D0, "<I", base + 0x1F76B0)
        mem.put(graphics, "<I", base + GRAPHICS_GAME_VTABLE)
        mem.put(graphics + 0x10C, "<II", 0x60000, 0x60128)
        mem.put(graphics + 0xC, "<I", rendered)
        self.assertEqual(select_city(mem, base, candidates, [graphics]), (rendered, scripts, graphics))
        self.assertEqual(select_city(mem, base, {other: scripts}, [graphics]), (other, scripts, None))
        with self.assertRaisesRegex(ValueError, "asset_city_ambiguous"):
            select_city(mem, base, candidates, [])

        mem.put(graphics + 0xC, "<I", other)
        mem.put(rendered + 0x17A0, "<I", 25)
        with self.assertRaisesRegex(ValueError, "asset_city_changed"):
            sample(mem, rendered, scripts, graphics, base, 1)

    def test_vehicle_inventory_distinguishes_a_missing_bin_and_a_zero_count(self):
        car = 0x0AB4DFE1
        empty = {(4, 7, car): {"amount": 0, "capacity": 12}}
        parked = {(4, 7, car): {"amount": 2, "capacity": 12}}
        self.assertEqual(vehicle_inventory({}), {})
        self.assertEqual(vehicle_inventory(empty), {"carW1": 0})
        self.assertEqual(vehicle_inventory(parked), {"carW1": 2})
        self.assertEqual(list(changes(empty, parked, 3, 36000, 99))[0]["amountDelta"], 2)
        self.assertEqual(list(changes(parked, empty, 4, 36001, 99))[0]["amountDelta"], -2)
        self.assertNotIn("amountDelta", list(changes({}, parked, 3, 36000, 99))[0])
        self.assertNotIn("amountDelta", list(changes(parked, {}, 4, 36001, 99))[0])

    def test_reads_identity_clock_local_resources_and_controller(self):
        mem = Memory()
        base, game, scripts, graphics, iid = 0x400000, 0x10000, 0x20000, 0x30000, 0xDDD772B5
        mem.put(game, "<I", base + GAME_VTABLE)
        mem.put(game + 0x10, "<I", scripts)
        mem.put(scripts, "<I", 0x40000)
        mem.put(0x400D0, "<I", base + 0x1F76B0)
        mem.put(game + 0x17A0, "<I", 25)
        mem.put(game + 0x18A4, "<II", 0x50000, 0x50004)
        mem.put(0x50000, "<I", 0x12952639)
        mem.put(game + 0x18D4, "<II", 0x50100, 0x50104)
        mem.put(0x50100, "<f", 0.75)
        mem.put(scripts + 0x90, "<II", 0x50200, 0x50210)
        mem.put(0x50200, "<IiIi", 533812918, 31, 444274501, 42)
        mem.put(game + 0x584, "<II", 0x50300, 0x50304)
        mem.put(0x50300, "<I", 99)
        mem.put(game + 0x554, "<II", 0x60000, 0x60380)
        unit = bytearray(0x380)
        struct.pack_into("<I", unit, 12, iid)
        struct.pack_into("<II", unit, 0x210, 0x70000, 0x70018)
        mem.blocks[0x60000] = unit
        mem.put(0x70000, "<HHiiHHii", 31, 0, 5, 30, 42, 0, 0, 18)
        mem.put(graphics, "<I", base + GRAPHICS_GAME_VTABLE)
        mem.put(graphics + 0xC, "<I", game)
        mem.put(graphics + 0x10C, "<II", 0x80000, 0x80128)
        mem.put(0x80084, "<I", 0x81000)
        mem.put(0x81000, "<I12x3fBB", base + INTERIORS_VTABLE, 0.85, 0.8, 1, 1, 0)

        tick, clock, members, values = sample(mem, game, scripts, graphics, base, iid)
        self.assertEqual((tick, clock, members), (25, 64800, {(0, 99)}))
        rows = {row["name"]: row for row in changes({}, values, tick, clock, iid)}
        self.assertEqual(rows["totalWorkers"]["after"], {"amount": 5, "capacity": 30})
        self.assertEqual(rows["touristW1"]["after"], {"amount": 0, "capacity": 18})
        self.assertEqual(rows["interiorLights"]["after"]["target"], 85)
        self.assertEqual(rows["interiorLights"]["after"]["actual"], 80)
        self.assertEqual(list(changes(values, values, tick, clock, iid)), [])

        mem.put(0x50300, "<I", 100)
        _, _, _, replacement = sample(mem, game, scripts, graphics, base, iid)
        updates = list(changes(values, replacement, tick, clock, iid))
        self.assertEqual(len(updates), 6)
        self.assertTrue(all(row["after"] is None for row in updates if row["unitStamp"] == 99))
        self.assertFalse(sample(mem, game, scripts, graphics, base, iid + 1)[2])


if __name__ == "__main__":
    unittest.main()
