"""Bounded, external read-only traffic counter for the verified retail build.

The probe never injects code, installs hooks, pauses threads, calls game
functions, or writes to the target.  It reads the already identified game
object, vehicle handler and hash map and groups observations by ``simTick``.
The addresses are deliberately explicit because heap addresses are valid only
for the selected process lifetime.
"""
from __future__ import annotations

import argparse
from collections import OrderedDict
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import math
import struct
import sys
import time
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools" / "dynamic" / "ui-observer"))
from eawebkit_runtime import EXE_HASH, Reader

VEHICLE_HANDLER_RVA = 0x2DF58398 - 0x400000
VEHICLE_VTABLE_RVA = 0x921458
SIM_TICK_OFFSET = 0x17A0
VEHICLE_HASH_OFFSET = 0x1D0
VEHICLE_SPEED_OFFSET = 0x15C
VEHICLE_PATH_RECALC_OFFSET = 0x180
VEHICLE_STOP_TIME_OFFSET = 0x184
# The extracted cAgent layout places mType after the 0x30-byte spline rep and
# four acceleration/scale floats: agent+0x44.  The nearby +0x58 field is
# mUnitGraphicsState and is recorded separately because it is useful for
# distinguishing a display/asset state from the transport category.
VEHICLE_AGENT_TYPE_OFFSET = 0x44
VEHICLE_UNIT_GRAPHICS_STATE_OFFSET = 0x58
# The retail cAgent base is followed by the audio position and a fixed
# four-entry cPath array.  These fields are captured only in --per-agent mode
# so the aggregate counter keeps its original read volume and schema.
VEHICLE_AUDIO_POSITION_OFFSET = 0x88
VEHICLE_PATH_INFO_OFFSET = 0x9C
VEHICLE_PATH_STRIDE = 0x0C
VEHICLE_PATH_COUNT = 4
VEHICLE_DETAIL_BASE_OFFSET = 0x40
VEHICLE_DETAIL_END_OFFSET = 0x1D8
TRANSACTION_SINK_ENTRY_OFFSET = 0x30
# cTransactionInfo::mSinkEntryIndices is an EASTL fixed vector.  In this
# retail layout its begin/end pointers are immediately after the three
# operation/lifetime fields; the four-item inline storage starts at +0x20.
# The vector can spill to a heap buffer, so the pointers must be read from the
# target process rather than assuming inline storage.
TRANSACTION_SINK_INDICES_BEGIN_OFFSET = 0x0C
TRANSACTION_SINK_INDICES_END_OFFSET = 0x10
TRANSACTION_SINK_INDICES_CAPACITY_OFFSET = 0x14
TRANSACTION_SINK_INDICES_INLINE_OFFSET = 0x20
TRANSACTION_SINK_INDICES_MAX = 4
TRANSACTION_GROUP_OFFSET = 0x38
TRANSACTION_POSITION_OFFSET = 0x3C
TRANSACTION_VELOCITY_OFFSET = 0x48
TRANSACTION_SPEED_OFFSET = 0x54
TRANSACTION_AGE_OFFSET = 0x58
TRANSPORT_SHARED_OFFSET = 0x0C
TRANSACTIONS_BEGIN_OFFSET = 0x14
TRANSACTIONS_END_OFFSET = 0x18
TRANSACTION_STRIDE = 0x5C
TRANSACTION_TRANSPORT_ENTRY_OFFSET = 0x34
HASH_BUCKET_PTR_OFFSET = 0x04
HASH_BUCKET_COUNT_OFFSET = 0x08
HASH_NODE_NEXT_OFFSET = 0x1DC
DEFAULT_VTABLE = 0xD21458

TEXT = {
    "zh-CN": {
        "description": "按游戏 simTick 对车辆 hash-map 进行只读计数（不注入、不调用游戏函数）。",
        "pid": "SimCity 进程 ID", "game": "游戏对象地址（或使用 --seed）",
        "manager": "车辆 handler 地址", "output": "输出目录", "seed": "含 game 地址的既有 trace.jsonl",
        "ticks": "目标模拟 tick 跨度", "seconds": "墙钟上限（秒）", "interval": "采样间隔（毫秒）",
        "entry_filter": "仅观察这些 TransportEntry 索引（可重复）",
        "per_agent": "写出逐 Agent 细节快照",
        "trigger_sink": "发现某个当前 sink 后记录并继续指定 tick",
        "post_trigger_ticks": "触发后继续观察的 tick 数",
        "lang": "输出语言", "help": "显示帮助并退出", "error": "探针失败",
    },
    "en-US": {
        "description": "Count the vehicle hash map by simTick using external read-only memory reads.",
        "pid": "SimCity process ID", "game": "Game object address (or --seed)",
        "manager": "Vehicle handler address", "output": "Output directory", "seed": "Existing trace.jsonl containing game address",
        "ticks": "Target simulated tick span", "seconds": "Wall-clock limit (seconds)", "interval": "Sampling interval (milliseconds)",
        "entry_filter": "Only observe these transport-entry indexes (repeatable)",
        "per_agent": "Write per-agent detail snapshots",
        "trigger_sink": "Record when an agent selects this current sink and continue for the specified ticks",
        "post_trigger_ticks": "Ticks to observe after the trigger",
        "lang": "Output language", "help": "Show help and exit", "error": "Probe failed",
    },
}


def address(value: str) -> int:
    """Parse a 32-bit process address without accepting negative values."""
    parsed = int(value, 0)
    if not 0x10000 <= parsed < 0x100000000:
        raise argparse.ArgumentTypeError("address must be a 32-bit user address")
    return parsed


def game_from_seed(seed: Path) -> int:
    """Read one unique ``game`` pointer from a trace file or capture folder."""
    path = seed / "trace.jsonl" if seed.is_dir() else seed
    games: set[int] = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        row = json.loads(line)
        if row.get("game") is not None:
            games.add(address(str(row["game"])))
    if len(games) != 1:
        raise ValueError("seed must contain exactly one game address")
    return next(iter(games))


def finite_float(value: float) -> bool:
    return math.isfinite(value) and abs(value) < 1.0e7


def decode_agent_detail(raw: bytes, transaction_raw: bytes | None = None,
                        transaction_sink_indices: list[int] | None = None) -> dict[str, Any]:
    """Decode the stable, read-only detail window used by --per-agent.

    The candidate cAgent fields are emitted with explicit ``_candidate`` names
    because the retail build's field order differs from the development source
    in a few nearby slots.  The four path entries and transaction fields have
    direct layout evidence in the extracted headers.
    """
    expected = VEHICLE_DETAIL_END_OFFSET - VEHICLE_DETAIL_BASE_OFFSET
    if len(raw) < expected:
        raise ValueError("short_agent_detail_window")

    def i32(offset: int) -> int:
        return struct.unpack_from("<i", raw, offset - VEHICLE_DETAIL_BASE_OFFSET)[0]

    def u32(offset: int) -> int:
        return struct.unpack_from("<I", raw, offset - VEHICLE_DETAIL_BASE_OFFSET)[0]

    def f32(offset: int) -> float | None:
        value = struct.unpack_from("<f", raw, offset - VEHICLE_DETAIL_BASE_OFFSET)[0]
        return value if finite_float(value) else None

    path_segments = [
        i32(VEHICLE_PATH_INFO_OFFSET + VEHICLE_PATH_STRIDE * index)
        for index in range(VEHICLE_PATH_COUNT)
    ]
    detail: dict[str, Any] = {
        "cAgentCandidates": {
            "sinkIndex_candidate": i32(0x4C),
            "unitEntryIndex_candidate": i32(0x50),
            "unitSlot_candidate": i32(0x54),
            "unitGraphicsState_candidate": i32(0x58),
            "currentDest_candidate": i32(0x5C),
        },
        "audioPositionCandidate": [f32(VEHICLE_AUDIO_POSITION_OFFSET + 4 * axis) for axis in range(3)],
        "pathSegments": path_segments,
        "agentTailWords": {
            hex(offset): u32(offset) for offset in range(0x1C0, VEHICLE_DETAIL_END_OFFSET, 4)
        },
    }
    if transaction_raw is not None:
        if len(transaction_raw) < TRANSACTION_STRIDE:
            raise ValueError("short_transaction_window")

        def tx_i32(offset: int) -> int:
            return struct.unpack_from("<i", transaction_raw, offset)[0]

        def tx_f32(offset: int) -> float | None:
            value = struct.unpack_from("<f", transaction_raw, offset)[0]
            return value if finite_float(value) else None

        detail["transaction"] = {
            "sinkEntryIndex": tx_i32(TRANSACTION_SINK_ENTRY_OFFSET),
            "sinkEntryIndices": transaction_sink_indices,
            "groupIndex": tx_i32(TRANSACTION_GROUP_OFFSET),
            "position": [tx_f32(TRANSACTION_POSITION_OFFSET + 4 * axis) for axis in range(3)],
            "velocity": [tx_f32(TRANSACTION_VELOCITY_OFFSET + 4 * axis) for axis in range(3)],
            "speed": tx_f32(TRANSACTION_SPEED_OFFSET),
            "age": tx_f32(TRANSACTION_AGE_OFFSET),
        }
    return detail


class TickAccumulator:
    """Pure aggregation used by the runtime loop and unit tests."""

    def __init__(self) -> None:
        self.rows: "OrderedDict[int, dict[str, Any]]" = OrderedDict()
        # Transaction keys are only unique within one transport handler.
        self.previous: dict[tuple[str, int], tuple[int, float]] = {}
        self.total_samples = 0
        self.total_node_observations = 0
        self.total_speed_changes = 0
        self.total_path_recompute_changes = 0
        self.total_stop_time_changes = 0
        self.total_read_errors = 0

    def add(self, tick: int, snapshot: dict[str, Any]) -> None:
        row = self.rows.setdefault(tick, {
            "simTick": tick, "samples": 0, "nodeObservations": 0,
            "nodeCountSum": 0, "nodeCountMax": 0, "validAgentCountMax": 0,
            "activeAgentCountSum": 0, "nonzeroSpeedCountMax": 0,
            "speedChangeCount": 0, "newAgentCount": 0, "removedAgentCount": 0,
            "pathRecomputeChangeCount": 0, "stopTimeChangeCount": 0,
            "addressChangeCount": 0, "nonzeroSpeedMax": 0,
            "unstableHeaderSamples": 0, "readErrors": 0, "tickRaceSamples": 0,
            "agentTypeCounts": {}, "agentTypeNonzeroSpeedCounts": {},
            "unitGraphicsStateCounts": {},
            "transportEntryIndexCounts": {},
        })
        handler = str(snapshot.get("manager", "default"))
        current: dict[tuple[str, int], tuple[int, float, float, float]] = {}
        for item in snapshot.get("agents", []):
            key = int(item["key"])
            agent = int(item["agent"], 0) if isinstance(item["agent"], str) else int(item["agent"])
            current[(str(item.get("handler", handler)), key)] = (
                agent, float(item["speed"]), float(item.get("pathRecomputeRemaining", 0.0)),
                float(item.get("stopTime", 0.0)))
        changes = sum(1 for key, (agent, speed, _, _) in current.items()
                      if key in self.previous and self.previous[key][0] == agent and speed != self.previous[key][1])
        path_changes = sum(1 for key, (agent, _, path, _) in current.items()
                           if key in self.previous and self.previous[key][0] == agent and path != self.previous[key][2])
        stop_changes = sum(1 for key, (agent, _, _, stop) in current.items()
                           if key in self.previous and self.previous[key][0] == agent and stop != self.previous[key][3])
        address_changes = sum(1 for key, (agent, _, _, _) in current.items()
                              if key in self.previous and self.previous[key][0] != agent)
        new = len(set(current) - set(self.previous))
        removed = len(set(self.previous) - set(current))
        row["samples"] += 1
        row["nodeObservations"] += len(snapshot.get("agents", []))
        row["nodeCountSum"] += int(snapshot.get("nodeCount", 0))
        row["nodeCountMax"] = max(row["nodeCountMax"], int(snapshot.get("nodeCount", 0)))
        row["validAgentCountMax"] = max(row["validAgentCountMax"], len(snapshot.get("agents", [])))
        row["activeAgentCountSum"] += len(snapshot.get("agents", []))
        row["nonzeroSpeedCountMax"] = max(row["nonzeroSpeedCountMax"],
                                           sum(1 for _, speed, _, _ in current.values() if speed != 0.0))
        row["speedChangeCount"] += changes
        row["pathRecomputeChangeCount"] += path_changes
        row["stopTimeChangeCount"] += stop_changes
        row["newAgentCount"] += new
        row["removedAgentCount"] += removed
        row["addressChangeCount"] += address_changes
        row["nonzeroSpeedMax"] = max(row["nonzeroSpeedMax"], sum(1 for _, speed, _, _ in current.values() if speed != 0.0))
        type_counts = row["agentTypeCounts"]
        type_nonzero = row["agentTypeNonzeroSpeedCounts"]
        for item in snapshot.get("agents", []):
            type_key = str(int(item.get("agentType", -1)))
            type_counts[type_key] = type_counts.get(type_key, 0) + 1
            graphics_key = str(int(item.get("unitGraphicsState", -1)))
            graphics_counts = row["unitGraphicsStateCounts"]
            graphics_counts[graphics_key] = graphics_counts.get(graphics_key, 0) + 1
            transport_key = str(int(item.get("transportEntryIndex", -1)))
            transport_counts = row["transportEntryIndexCounts"]
            transport_counts[transport_key] = transport_counts.get(transport_key, 0) + 1
            if float(item["speed"]) != 0.0:
                type_nonzero[type_key] = type_nonzero.get(type_key, 0) + 1
        row["unstableHeaderSamples"] += int(not snapshot.get("headerStable", False))
        row["readErrors"] += int(snapshot.get("readErrors", 0))
        row["tickRaceSamples"] += int(snapshot.get("tickBefore") != snapshot.get("tickAfter"))
        self.previous = current
        self.total_samples += 1
        self.total_node_observations += len(snapshot.get("agents", []))
        self.total_speed_changes += changes
        self.total_path_recompute_changes += path_changes
        self.total_stop_time_changes += stop_changes
        self.total_read_errors += int(snapshot.get("readErrors", 0))

    def finalized(self, first_tick: int | None = None) -> list[dict[str, Any]]:
        result = []
        previous_tick = None
        for tick, row in self.rows.items():
            out = dict(row)
            out["nodeCountMean"] = row["nodeCountSum"] / row["samples"] if row["samples"] else 0.0
            out["gapBefore"] = 0 if previous_tick is None else max(0, tick - previous_tick - 1)
            result.append(out)
            previous_tick = tick
        return result


class TrafficSampler:
    def __init__(self, reader: Reader, game: int, managers: list[int], vtable: int,
                 max_nodes: int = 20000, per_agent: bool = False,
                 detail_transport_entries: set[int] | None = None):
        self.reader, self.game, self.managers = reader, game, managers
        self.vtable, self.max_nodes, self.per_agent = vtable, max_nodes, per_agent
        self.detail_transport_entries = detail_transport_entries

    def tick(self) -> int:
        return self.reader.u32(self.game + SIM_TICK_OFFSET)

    def capture_manager(self, manager: int) -> dict[str, Any]:
        """Read one bounded, non-atomic vehicle snapshot."""
        errors: list[str] = []
        transaction_begin = None
        transaction_count = None
        try:
            shared = self.reader.u32(manager + TRANSPORT_SHARED_OFFSET)
            transaction_begin = self.reader.u32(shared + TRANSACTIONS_BEGIN_OFFSET)
            transaction_end = self.reader.u32(shared + TRANSACTIONS_END_OFFSET)
            transaction_bytes = transaction_end - transaction_begin
            if not (0x10000 <= transaction_begin < 0x100000000
                    and 0 <= transaction_bytes <= 0x10000000
                    and transaction_bytes % TRANSACTION_STRIDE == 0):
                raise ValueError("invalid_transaction_vector")
            transaction_count = transaction_bytes // TRANSACTION_STRIDE
        except (OSError, ValueError) as exc:
            errors.append(type(exc).__name__ + ":" + str(exc))
        map_address = manager + VEHICLE_HASH_OFFSET
        try:
            header_before = self.reader.read(map_address, 12)
            bucket_ptr, bucket_count = struct.unpack_from("<II", header_before, HASH_BUCKET_PTR_OFFSET)
            if not 0x10000 <= bucket_ptr < 0x100000000 or not 1 <= bucket_count <= 65536:
                raise ValueError("invalid_vehicle_hash_header")
            buckets = self.reader.read(bucket_ptr, bucket_count * 4)
        except (OSError, ValueError, struct.error) as exc:
            return {"manager": hex(manager), "agents": [], "nodeCount": 0, "mapHeaderValid": False, "headerStable": False, "readErrors": 1,
                    "errors": [str(exc)]}
        agents: list[dict[str, Any]] = []
        seen: set[int] = set()
        raw_nodes = 0
        for bucket_index, (node,) in enumerate(struct.iter_unpack("<I", buckets)):
            while node:
                if len(seen) >= self.max_nodes:
                    errors.append("node_limit")
                    break
                if node in seen:
                    errors.append("node_cycle")
                    break
                seen.add(node)
                raw_nodes += 1
                try:
                    raw = self.reader.read(node, 8)
                    key, _inline_agent_vtable = struct.unpack("<II", raw)
                    # The EASTL node stores the cVehicleAgent inline at node+4;
                    # node+4 is the vtable word, not a second-level pointer.
                    agent = node + 4
                    if key % bucket_count != bucket_index:
                        errors.append("bucket_mismatch")
                    if self.reader.u32(agent) != self.vtable:
                        errors.append("agent_vtable_mismatch")
                    else:
                        speed = struct.unpack("<f", self.reader.read(agent + VEHICLE_SPEED_OFFSET, 4))[0]
                        path_recompute = struct.unpack("<f", self.reader.read(agent + VEHICLE_PATH_RECALC_OFFSET, 4))[0]
                        stop_time = struct.unpack("<f", self.reader.read(agent + VEHICLE_STOP_TIME_OFFSET, 4))[0]
                        agent_type = self.reader.u32(agent + VEHICLE_AGENT_TYPE_OFFSET)
                        graphics_state = self.reader.u32(agent + VEHICLE_UNIT_GRAPHICS_STATE_OFFSET)
                        transport_entry_index = -1
                        transaction_raw = None
                        if transaction_begin is not None and transaction_count is not None and 0 <= key < transaction_count:
                            transaction_entry = transaction_begin + key * TRANSACTION_STRIDE
                            transport_entry_index = struct.unpack(
                                "<i", self.reader.read(transaction_entry + TRANSACTION_TRANSPORT_ENTRY_OFFSET, 4)
                            )[0]
                            capture_detail = self.per_agent and (
                                not self.detail_transport_entries
                                or transport_entry_index in self.detail_transport_entries
                            )
                            if capture_detail:
                                transaction_raw = self.reader.read(transaction_entry, TRANSACTION_STRIDE)
                        if not finite_float(speed) or not finite_float(path_recompute) or not finite_float(stop_time):
                            errors.append("invalid_speed")
                        else:
                            item = {"key": key, "agent": hex(agent), "speed": speed,
                                    "pathRecomputeRemaining": path_recompute, "stopTime": stop_time,
                                    "agentType": agent_type,
                                    "unitGraphicsState": graphics_state,
                                    "transportEntryIndex": transport_entry_index}
                            if transaction_raw is not None:
                                agent_raw = self.reader.read(
                                    agent + VEHICLE_DETAIL_BASE_OFFSET,
                                    VEHICLE_DETAIL_END_OFFSET - VEHICLE_DETAIL_BASE_OFFSET,
                                )
                                sink_indices: list[int] | None = None
                                try:
                                    begin, end = struct.unpack_from(
                                        "<II", transaction_raw, TRANSACTION_SINK_INDICES_BEGIN_OFFSET
                                    )
                                    byte_count = end - begin
                                    if not (0x10000 <= begin < 0x100000000
                                            and 0x10000 <= end < 0x100000000
                                            and 0 <= byte_count <= TRANSACTION_SINK_INDICES_MAX * 4
                                            and byte_count % 4 == 0):
                                        raise ValueError("invalid_sink_indices_vector")
                                    if byte_count:
                                        sink_indices = list(struct.unpack(
                                            "<%dI" % (byte_count // 4), self.reader.read(begin, byte_count)
                                        ))
                                    else:
                                        sink_indices = []
                                except (OSError, ValueError, struct.error) as exc:
                                    errors.append("sink_indices:" + type(exc).__name__)
                                item["detail"] = decode_agent_detail(
                                    agent_raw, transaction_raw, sink_indices
                                )
                            agents.append(item)
                    node = self.reader.u32(node + HASH_NODE_NEXT_OFFSET)
                except (OSError, ValueError, struct.error) as exc:
                    errors.append(type(exc).__name__)
                    break
        try:
            header_after = self.reader.read(map_address, 12)
        except OSError:
            header_after = b""
        try:
            reported_agent_count = self.reader.u32(map_address + 0x0C)
        except OSError:
            reported_agent_count = None
            errors.append("element_count_unreadable")
        return {"manager": hex(manager), "agents": agents, "nodeCount": raw_nodes,
                "transactionBegin": hex(transaction_begin) if transaction_begin is not None else None,
                "transactionCount": transaction_count,
                "reportedAgentCount": reported_agent_count, "validAgentCount": len(agents),
                "mapHeaderValid": True,
                "headerStable": header_before == header_after, "readErrors": len(errors),
                "errors": errors}

    def capture(self) -> dict[str, Any]:
        """Read every configured vehicle handler once for this tick."""
        handlers = [self.capture_manager(manager) for manager in self.managers]
        return {
            "handlers": handlers,
            "nodeCount": sum(int(item.get("nodeCount", 0)) for item in handlers),
            "validAgentCount": sum(int(item.get("validAgentCount", 0)) for item in handlers),
            "readErrors": sum(int(item.get("readErrors", 0)) for item in handlers),
            "mapHeaderValid": all(item.get("mapHeaderValid", False) for item in handlers),
            "headerStable": all(item.get("headerStable", False) for item in handlers),
        }


def find_trigger_agents(snapshot: dict[str, Any], trigger_sink: int | None) -> list[dict[str, Any]]:
    """Return agents whose *selected* sink equals the trigger, not just candidates."""
    if trigger_sink is None:
        return []
    matches: list[dict[str, Any]] = []
    for agent in snapshot.get("agents", []):
        transaction = (agent.get("detail") or {}).get("transaction") or {}
        if transaction.get("sinkEntryIndex") == trigger_sink:
            matches.append({
                "key": agent.get("key"),
                "agent": agent.get("agent"),
                "handler": agent.get("handler"),
                "transportEntryIndex": agent.get("transportEntryIndex"),
                "sinkEntryIndex": trigger_sink,
                "sinkEntryIndices": transaction.get("sinkEntryIndices"),
            })
    return matches


def finalize_summary(status: dict[str, Any], accumulator: TickAccumulator, rows: list[dict[str, Any]]) -> dict[str, Any]:
    ticks = [row["simTick"] for row in rows]
    first, last = (ticks[0], ticks[-1]) if ticks else (None, None)
    span = (last - first + 1) if first is not None and last >= first else 0
    return {
        "schema": "openscp.traffic-tick-count.v1", "mode": "external-read-only",
        "firstObservedTick": first, "lastObservedTick": last,
        "observedTickCount": len(ticks), "tickSpan": span,
        "missingTickCount": max(0, span - len(ticks)),
        "totalSamples": accumulator.total_samples,
        "totalNodeObservations": accumulator.total_node_observations,
        "totalSpeedChanges": accumulator.total_speed_changes,
        "totalPathRecomputeChanges": accumulator.total_path_recompute_changes,
        "totalStopTimeChanges": accumulator.total_stop_time_changes,
        "totalReadErrors": accumulator.total_read_errors,
        "meanNodeObservationsPerSample": accumulator.total_node_observations / accumulator.total_samples if accumulator.total_samples else 0.0,
        "rows": len(rows), "stopReason": status.get("stopReason"),
        "trigger": status.get("trigger"),
        "interpretation": "One aggregate row is emitted per observed simTick. Counts are validated external observations and speed transitions, not engine log records or a complete atomic city snapshot.",
    }


def parser_for(lang: str) -> argparse.ArgumentParser:
    labels = TEXT[lang]
    parser = argparse.ArgumentParser(description=labels["description"])
    parser.add_argument("--lang", choices=TEXT, default=lang, help=labels["lang"])
    parser.add_argument("--pid", required=True, type=int, help=labels["pid"])
    parser.add_argument("--game", type=address, help=labels["game"])
    parser.add_argument("--seed", type=Path, help=labels["seed"])
    parser.add_argument("--vehicle-manager", required=True, type=address, action="append", help=labels["manager"] + "（可重复，覆盖多个 handler）")
    parser.add_argument("--transport-entry-index", type=int, action="append", default=[], help=labels["entry_filter"])
    parser.add_argument("--per-agent", action="store_true", help=labels["per_agent"])
    parser.add_argument("--trigger-sink", type=int, help=labels["trigger_sink"])
    parser.add_argument("--post-trigger-ticks", type=int, default=30, help=labels["post_trigger_ticks"])
    parser.add_argument("--out", required=True, type=Path, help=labels["output"])
    parser.add_argument("--ticks", type=int, default=60, help=labels["ticks"])
    parser.add_argument("--seconds", type=int, default=180, help=labels["seconds"])
    parser.add_argument("--interval-ms", type=int, default=50, help=labels["interval"])
    parser.add_argument("--vehicle-vtable", type=address, default=DEFAULT_VTABLE)
    return parser


def main() -> int:
    pre = argparse.ArgumentParser(add_help=False)
    pre.add_argument("--lang", choices=TEXT, default="zh-CN")
    selected, _ = pre.parse_known_args()
    args = parser_for(selected.lang).parse_args()
    if (args.pid <= 0 or not 1 <= args.ticks <= 600 or not 1 <= args.seconds <= 900
            or not 5 <= args.interval_ms <= 1000 or not 0 <= args.post_trigger_ticks <= 600):
        parser_for(args.lang).error("pid positive; ticks 1..600; seconds 1..900; interval-ms 5..1000; post-trigger-ticks 0..600")
    if args.trigger_sink is not None and not args.per_agent:
        parser_for(args.lang).error("--trigger-sink requires --per-agent")
    if args.game is None:
        if args.seed is None:
            parser_for(args.lang).error("provide --game or --seed")
        args.game = game_from_seed(args.seed)
    args.out.mkdir(parents=True, exist_ok=False)
    started = datetime.now(timezone.utc).isoformat()
    status: dict[str, Any] = {
        "schema": "openscp.traffic-tick-count-status.v1", "pid": args.pid,
        "game": hex(args.game), "vehicleManagers": [hex(item) for item in args.vehicle_manager],
        "vehicleVtable": hex(args.vehicle_vtable), "agentTypeOffset": hex(VEHICLE_AGENT_TYPE_OFFSET),
        "pathRecomputeOffset": hex(VEHICLE_PATH_RECALC_OFFSET), "stopTimeOffset": hex(VEHICLE_STOP_TIME_OFFSET),
        "unitGraphicsStateOffset": hex(VEHICLE_UNIT_GRAPHICS_STATE_OFFSET),
        "transactionTransportEntryOffset": hex(TRANSACTION_TRANSPORT_ENTRY_OFFSET),
        "transactionStride": TRANSACTION_STRIDE,
        "perAgentDetailOffsets": {
            "detailBase": hex(VEHICLE_DETAIL_BASE_OFFSET),
            "detailEnd": hex(VEHICLE_DETAIL_END_OFFSET),
            "audioPositionCandidate": hex(VEHICLE_AUDIO_POSITION_OFFSET),
            "pathInfo": hex(VEHICLE_PATH_INFO_OFFSET),
            "pathStride": hex(VEHICLE_PATH_STRIDE),
            "transactionSinkEntryIndex": hex(TRANSACTION_SINK_ENTRY_OFFSET),
            "transactionSinkIndicesBegin": hex(TRANSACTION_SINK_INDICES_BEGIN_OFFSET),
            "transactionSinkIndicesEnd": hex(TRANSACTION_SINK_INDICES_END_OFFSET),
            "transactionSinkIndicesInline": hex(TRANSACTION_SINK_INDICES_INLINE_OFFSET),
            "transactionSinkIndicesMax": TRANSACTION_SINK_INDICES_MAX,
            "transactionGroupIndex": hex(TRANSACTION_GROUP_OFFSET),
            "transactionPosition": hex(TRANSACTION_POSITION_OFFSET),
            "transactionVelocity": hex(TRANSACTION_VELOCITY_OFFSET),
            "transactionSpeed": hex(TRANSACTION_SPEED_OFFSET),
            "transactionAge": hex(TRANSACTION_AGE_OFFSET),
        },
        "mode": "external-read-only",
        "readOnly": True, "remoteCalls": False, "requestedTicks": args.ticks,
        "requestedSeconds": args.seconds, "intervalMs": args.interval_ms,
        "transportEntryIndexFilter": args.transport_entry_index, "perAgent": args.per_agent,
        "triggerSink": args.trigger_sink, "postTriggerTicks": args.post_trigger_ticks,
        "startedUtc": started, "errors": [], "samples": 0, "handlerSnapshots": 0,
        "limitations": ["non-atomic hash-map snapshots", "heap addresses are process-lifetime specific",
                        "observations are not engine log records", "only explicitly validated vehicle handlers are sampled",
                        "one aggregate row is committed per observed simTick",
                        "agentType is read from retail cAgent+0x44; values still require category confirmation",
                        "transaction transport index is read from the shared transaction vector; vector snapshots are non-atomic",
                        "per-agent detail fields are point-in-time reads and candidate cAgent slots retain explicit uncertainty"],
    }
    reader: Reader | None = None
    accumulator = TickAccumulator()
    first_tick: int | None = None
    last_tick: int | None = None
    trigger_tick: int | None = None
    trigger_agents: list[dict[str, Any]] = []
    try:
        reader = Reader(args.pid)
        module = next((item for item in reader.modules if item["name"].lower() == "simcity.exe"), None)
        if not module:
            raise ValueError("simcity_module_missing")
        exe_sha = hashlib.sha256(Path(module["path"]).read_bytes()).hexdigest()
        if exe_sha != EXE_HASH:
            raise ValueError("unsupported_exe")
        expected_vtable = module["base"] + VEHICLE_VTABLE_RVA
        if args.vehicle_vtable != expected_vtable:
            raise ValueError(f"vehicle_vtable_mismatch:{args.vehicle_vtable:#x}!={expected_vtable:#x}")
        status.update({"exe": module, "exeSha256": exe_sha, "moduleBase": hex(module["base"])})
        sampler = TrafficSampler(reader, args.game, args.vehicle_manager, args.vehicle_vtable,
                                 per_agent=args.per_agent,
                                 detail_transport_entries=set(args.transport_entry_index) or None)
        detail_file = (args.out / "agent-samples.jsonl").open("w", encoding="utf-8") if args.per_agent else None
        deadline = time.monotonic() + args.seconds
        status["ready"] = True
        while time.monotonic() < deadline:
            before = sampler.tick()
            if last_tick is not None and before == last_tick:
                time.sleep(min(args.interval_ms / 1000.0, max(0.0, deadline - time.monotonic())))
                continue
            snapshot = sampler.capture()
            after = sampler.tick()
            if not snapshot.get("mapHeaderValid", False):
                raise ValueError("vehicle_manager_hash_header_unreadable:" + ";".join(snapshot.get("errors", [])))
            observed = after
            if first_tick is None:
                first_tick = observed
            if last_tick is not None and observed < last_tick:
                raise ValueError("sim_tick_rewound")
            last_tick = observed
            flattened = []
            for handler_snapshot in snapshot.get("handlers", []):
                manager = handler_snapshot.get("manager", "default")
                for agent in handler_snapshot.get("agents", []):
                    flattened.append({**agent, "handler": manager})
            snapshot["agents"] = flattened
            if args.transport_entry_index:
                allowed = set(args.transport_entry_index)
                snapshot["unfilteredAgentCount"] = len(flattened)
                snapshot["agents"] = [item for item in flattened
                                      if int(item.get("transportEntryIndex", -1)) in allowed]
            snapshot["tickBefore"] = before
            snapshot["tickAfter"] = after
            matches = find_trigger_agents(snapshot, args.trigger_sink)
            if matches and trigger_tick is None:
                trigger_tick = observed
                trigger_agents = matches
                status["trigger"] = {"simTick": observed, "agents": matches}
            accumulator.add(observed, snapshot)
            if detail_file is not None:
                detail_file.write(json.dumps({
                    "schema": "openscp.traffic-agent-snapshot.v1",
                    "simTick": observed,
                    "tickBefore": before,
                    "tickAfter": after,
                    "headerStable": snapshot.get("headerStable", False),
                    "agents": snapshot["agents"],
                }, ensure_ascii=False) + "\n")
                detail_file.flush()
            status["samples"] += 1
            status["handlerSnapshots"] += len(snapshot.get("handlers", []))
            requested_span_reached = first_tick is not None and observed - first_tick >= args.ticks - 1
            post_trigger_span_reached = trigger_tick is not None and observed - trigger_tick >= args.post_trigger_ticks
            if post_trigger_span_reached or (args.trigger_sink is None and requested_span_reached):
                status["stopReason"] = "tick_span_reached"
                break
            time.sleep(min(args.interval_ms / 1000.0, max(0.0, deadline - time.monotonic())))
        else:
            status["stopReason"] = "wall_time_limit"
    except KeyboardInterrupt:
        status["stopReason"] = "keyboard_interrupt"
    except Exception as exc:
        status["errors"].append(str(exc))
        status["stopReason"] = "validation_error"
    finally:
        if 'detail_file' in locals() and detail_file is not None:
            detail_file.close()
        rows = accumulator.finalized(first_tick)
        (args.out / "samples.jsonl").write_text("\n".join(json.dumps(row, ensure_ascii=False) for row in rows) + ("\n" if rows else ""), encoding="utf-8")
        status.update({"firstObservedTick": first_tick, "lastObservedTick": last_tick,
                       "finishedUtc": datetime.now(timezone.utc).isoformat()})
        summary = finalize_summary(status, accumulator, rows)
        (args.out / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        (args.out / "status.json").write_text(json.dumps(status, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if reader is not None:
            reader.close()
        print(json.dumps({"out": str(args.out), "samples": status["samples"], "ticks": len(rows),
                          "stopReason": status.get("stopReason"), "errors": status["errors"]}, ensure_ascii=False), flush=True)
    return int(bool(status["errors"]))


if __name__ == "__main__":
    raise SystemExit(main())
