import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from traffic_tick_watch import (
    TickAccumulator,
    VEHICLE_DETAIL_BASE_OFFSET,
    VEHICLE_DETAIL_END_OFFSET,
    VEHICLE_PATH_INFO_OFFSET,
    decode_agent_detail,
    find_trigger_agents,
    address,
)


class TrafficTickWatchTests(unittest.TestCase):
    def test_address_requires_32_bit_user_pointer(self):
        self.assertEqual(address("0x2df58398"), 0x2DF58398)
        with self.assertRaises(Exception):
            address("0x1000")
        with self.assertRaises(Exception):
            address("0x100000000")

    def test_aggregates_changes_and_tick_gap(self):
        acc = TickAccumulator()
        acc.add(100, {"nodeCount": 2, "headerStable": True, "agents": [
            {"key": 1, "agent": "0x10001", "speed": 0.0},
            {"key": 2, "agent": "0x10002", "speed": 2.0},
        ]})
        acc.add(102, {"nodeCount": 2, "headerStable": False, "readErrors": 1, "agents": [
            {"key": 1, "agent": "0x10003", "speed": 1.0},
            {"key": 3, "agent": "0x10004", "speed": 4.0},
        ]})
        rows = acc.finalized()
        self.assertEqual([row["simTick"] for row in rows], [100, 102])
        self.assertEqual(rows[1]["gapBefore"], 1)
        self.assertEqual(rows[1]["addressChangeCount"], 1)
        self.assertEqual(rows[1]["newAgentCount"], 1)
        self.assertEqual(rows[1]["removedAgentCount"], 1)
        self.assertEqual(rows[1]["speedChangeCount"], 0)
        self.assertEqual(rows[1]["unstableHeaderSamples"], 1)
        self.assertEqual(acc.total_read_errors, 1)

    def test_tracks_path_and_stop_field_changes(self):
        acc = TickAccumulator()
        base = {"key": 1, "agent": "0x10001", "speed": 1.0,
                "pathRecomputeRemaining": 4.0, "stopTime": 0.0}
        acc.add(10, {"nodeCount": 1, "headerStable": True, "agents": [base]})
        changed = dict(base, pathRecomputeRemaining=3.5, stopTime=0.25)
        acc.add(11, {"nodeCount": 1, "headerStable": True, "agents": [changed]})
        row = acc.finalized()[1]
        self.assertEqual(row["pathRecomputeChangeCount"], 1)
        self.assertEqual(row["stopTimeChangeCount"], 1)
        self.assertEqual(acc.total_path_recompute_changes, 1)
        self.assertEqual(acc.total_stop_time_changes, 1)

    def test_decodes_per_agent_path_and_transaction_detail(self):
        import struct

        raw = bytearray(VEHICLE_DETAIL_END_OFFSET - VEHICLE_DETAIL_BASE_OFFSET)

        def put_i32(offset, value):
            struct.pack_into("<i", raw, offset - VEHICLE_DETAIL_BASE_OFFSET, value)

        def put_f32(offset, value):
            struct.pack_into("<f", raw, offset - VEHICLE_DETAIL_BASE_OFFSET, value)

        put_i32(0x4C, 17)
        put_i32(0x50, 23)
        put_i32(0x54, 29)
        put_i32(0x58, 2)
        put_i32(0x5C, 31)
        put_f32(0x88, 10.0)
        put_f32(0x8C, 20.0)
        put_f32(0x90, 30.0)
        for index, value in enumerate((101, 102, 103, 104)):
            put_i32(VEHICLE_PATH_INFO_OFFSET + index * 0x0C, value)

        tx = bytearray(0x5C)
        struct.pack_into("<i", tx, 0x30, 41)
        struct.pack_into("<i", tx, 0x38, 43)
        struct.pack_into("<fff", tx, 0x3C, 1.0, 2.0, 3.0)
        struct.pack_into("<fff", tx, 0x48, 4.0, 5.0, 6.0)
        struct.pack_into("<f", tx, 0x54, 7.0)
        struct.pack_into("<f", tx, 0x58, 8.0)

        detail = decode_agent_detail(bytes(raw), bytes(tx), [12, 41, 99])
        self.assertEqual(detail["cAgentCandidates"]["sinkIndex_candidate"], 17)
        self.assertEqual(detail["pathSegments"], [101, 102, 103, 104])
        self.assertEqual(detail["audioPositionCandidate"], [10.0, 20.0, 30.0])
        self.assertEqual(detail["transaction"]["sinkEntryIndex"], 41)
        self.assertEqual(detail["transaction"]["sinkEntryIndices"], [12, 41, 99])
        self.assertEqual(detail["transaction"]["groupIndex"], 43)
        self.assertEqual(detail["transaction"]["position"], [1.0, 2.0, 3.0])

    def test_trigger_requires_selected_sink_not_candidate_list(self):
        snapshot = {"agents": [
            {"key": 1, "agent": "0x10001", "handler": "0x1",
             "transportEntryIndex": 50,
             "detail": {"transaction": {"sinkEntryIndex": 1122,
                                           "sinkEntryIndices": [323, 1122, 109, 5]}}},
            {"key": 2, "agent": "0x10002", "handler": "0x1",
             "transportEntryIndex": 50,
             "detail": {"transaction": {"sinkEntryIndex": 323,
                                           "sinkEntryIndices": [323, 1122, 109, 5]}}},
        ]}
        matches = find_trigger_agents(snapshot, 323)
        self.assertEqual([item["key"] for item in matches], [2])


if __name__ == "__main__":
    unittest.main()
