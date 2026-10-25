"""External read-only watcher for global crime resource bins.

The probe reads the already loaded ``cEcoGameScripts`` object through
ReadProcessMemory.  It does not inject code, call game functions, suspend the
game, or write to the target process.  Resource IDs come from the shipped UI
registry and are mapped to the live ``resourceIndex`` table before reading the
global ``cEcoResourceBins`` vector.

This is a state probe, not an engine event log.  A positive delta proves that
the corresponding global bin changed between two observations; it does not by
itself identify which rule or vehicle caused that change.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import struct
import sys
import time
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools" / "dynamic" / "ui-observer"))
from eawebkit_runtime import EXE_HASH, Reader

SIM_TICK_OFFSET = 0x17A0
SCRIPTS_OFFSET = 0x10
RESOURCE_MAP_OFFSET = 0x90
# Retail cEcoGame's embedded cGlobalBinSet exposes cEcoResourceBins::mData
# begin/end at these object offsets.  The preceding word is the fixed-vector
# allocator field; keeping the direct offsets avoids conflating that field
# with the mData pointers in this retail layout.
GLOBAL_BINS_DATA_BEGIN_OFFSET = 0x478
GLOBAL_BINS_DATA_END_OFFSET = 0x47C
RESOURCE_MAP_PAIR_STRIDE = 8
GLOBAL_BIN_STRIDE = 12
MAX_RESOURCE_MAP_ENTRIES = 100000
MAX_GLOBAL_BINS = 100000

# Values are the exact IDs exported by the retail UI registry.
CRIME_RESOURCE_IDS: dict[str, int] = {
    "CriminalsArrested": 3969035757,
    "CriminalsArrestedPerDay": 1304762412,
    "CriminalsIncarcerated": 3192894076,
    "CriminalsIncarcerated_French": 0,  # filled only when present in the map
    "CriminalsAtLarge": 323121511,
    "CriminalsAtLargePerDay": 2137315242,
    "CriminalsRehabilitated": 2960242855,
    "CriminalsRehabilitatedToday": 1663983338,
    "CriminalsRehabilitatedPerDay": 1984162026,
    "CriminalsReleasedOvercrowding": 122084851,
    "TradeTrackingCriminalsCaught": 542997796,
    "TradeTrackingCriminalsAtLarge": 169350308,
    "RegionalCriminalsIncarcerated": 3319542441,
}


def parse_address(value: str) -> int:
    result = int(value, 0)
    if not 0x10000 <= result < 0x100000000:
        raise argparse.ArgumentTypeError("address must be a 32-bit user address")
    return result


def vector(reader: Reader, address: int, stride: int, limit: int) -> tuple[int, int, bytes]:
    raw = reader.read(address, 8)
    begin, end = struct.unpack("<II", raw)
    if begin < 0x10000 or end < begin or (end - begin) % stride:
        raise ValueError("invalid_vector_bounds")
    if (end - begin) // stride > limit:
        raise ValueError("vector_limit_exceeded")
    return begin, end, raw


def resource_indices(reader: Reader, scripts: int) -> dict[str, int]:
    begin, end, _ = vector(reader, scripts + RESOURCE_MAP_OFFSET,
                            RESOURCE_MAP_PAIR_STRIDE, MAX_RESOURCE_MAP_ENTRIES)
    pairs = struct.iter_unpack("<II", reader.read(begin, end - begin))
    by_id = {resource_id: index for resource_id, index in pairs}
    result: dict[str, int] = {}
    for name, resource_id in CRIME_RESOURCE_IDS.items():
        if resource_id and resource_id in by_id:
            result[name] = by_id[resource_id]
    if not result:
        raise ValueError("crime_resource_ids_not_loaded")
    return result


def read_global_bins(reader: Reader, game: int, scripts: int,
                     wanted: dict[str, int]) -> tuple[dict[str, dict[str, int | None]], dict[str, Any]]:
    # ``scripts`` is retained for the signature and evidence record because
    # the resource-index map belongs to cEcoGameScripts; the live amounts are
    # owned by cEcoGame's GlobalBinSet.
    del scripts
    begin = reader.u32(game + GLOBAL_BINS_DATA_BEGIN_OFFSET)
    end = reader.u32(game + GLOBAL_BINS_DATA_END_OFFSET)
    if begin < 0x10000 or end < begin or (end - begin) % GLOBAL_BIN_STRIDE:
        raise ValueError("invalid_global_bins_vector")
    count = (end - begin) // GLOBAL_BIN_STRIDE
    if count > MAX_GLOBAL_BINS:
        raise ValueError("global_bins_limit_exceeded")
    raw = reader.read(begin, end - begin)
    wanted_by_index = {index: name for name, index in wanted.items()}
    found: dict[str, dict[str, int | None]] = {}
    for position, (resource_index, next_index, amount, capacity) in enumerate(
            struct.iter_unpack("<HHii", raw)):
        name = wanted_by_index.get(resource_index)
        if name is not None:
            found[name] = {
                "resourceIndex": resource_index,
                "binPosition": position,
                "amount": amount,
                "capacity": capacity,
                "nextIndex": next_index,
            }
    return found, {"begin": hex(begin), "end": hex(end), "count": count}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid", required=True, type=int)
    parser.add_argument("--game", required=True, type=parse_address)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--seconds", type=int, default=600)
    parser.add_argument("--ticks", type=int, default=1800)
    parser.add_argument("--interval-ms", type=int, default=20)
    args = parser.parse_args()
    if args.pid <= 0 or not 1 <= args.seconds <= 1800 or not 1 <= args.ticks <= 10000:
        parser.error("pid positive; seconds 1..1800; ticks 1..10000")
    if not 5 <= args.interval_ms <= 1000:
        parser.error("interval-ms must be 5..1000")
    args.out.mkdir(parents=True, exist_ok=False)
    status: dict[str, Any] = {
        "schema": "openscp.crime-resource-watch-status.v1",
        "pid": args.pid, "game": hex(args.game), "mode": "external-read-only",
        "readOnly": True, "remoteCalls": False, "requestedSeconds": args.seconds,
        "requestedTicks": args.ticks, "intervalMs": args.interval_ms,
        "startedUtc": datetime.now(timezone.utc).isoformat(), "samples": 0,
        "errors": [], "limitations": [
            "global-bin snapshots are non-atomic",
            "positive or negative deltas are state changes, not rule attribution",
            "resource IDs are mapped from the installed retail UI registry",
            "heap addresses are valid only for this process lifetime",
        ],
    }
    reader: Reader | None = None
    previous: dict[str, int] | None = None
    first_tick: int | None = None
    last_tick: int | None = None
    try:
        reader = Reader(args.pid)
        module = next((item for item in reader.modules
                       if item["name"].lower() == "simcity.exe"), None)
        if module is None:
            raise ValueError("simcity_module_missing")
        exe_hash = hashlib.sha256(Path(module["path"]).read_bytes()).hexdigest()
        if exe_hash != EXE_HASH:
            raise ValueError("unsupported_exe")
        status.update({"exe": module, "exeSha256": exe_hash, "moduleBase": hex(module["base"])})
        scripts = reader.u32(args.game + SCRIPTS_OFFSET)
        if not 0x10000 <= scripts < 0x100000000:
            raise ValueError("invalid_scripts_pointer")
        wanted = resource_indices(reader, scripts)
        status.update({"scripts": hex(scripts), "resourceIndices": wanted,
                       "resourceIds": {name: CRIME_RESOURCE_IDS[name] for name in wanted},
                       "globalBinSet": {"gameDataBeginOffset": hex(GLOBAL_BINS_DATA_BEGIN_OFFSET),
                                        "gameDataEndOffset": hex(GLOBAL_BINS_DATA_END_OFFSET)}})
        deadline = time.monotonic() + args.seconds
        with (args.out / "snapshots.jsonl").open("w", encoding="utf-8") as trace:
            while time.monotonic() < deadline:
                tick_before = reader.u32(args.game + SIM_TICK_OFFSET)
                current_indices = resource_indices(reader, scripts)
                bins, vector_info = read_global_bins(reader, args.game, scripts, current_indices)
                tick_after = reader.u32(args.game + SIM_TICK_OFFSET)
                amounts = {name: int(data["amount"]) for name, data in bins.items()}
                deltas = ({name: amount - previous[name] for name, amount in amounts.items()
                           if name in previous and amount != previous[name]}
                           if previous is not None else {})
                row = {
                    "utc": datetime.now(timezone.utc).isoformat(),
                    "simTick": tick_after, "tickBefore": tick_before,
                    "tickAfter": tick_after, "sameTick": tick_before == tick_after,
                    "scripts": hex(scripts), "globalBins": vector_info,
                    "resources": bins, "deltas": deltas,
                }
                trace.write(json.dumps(row, ensure_ascii=False) + "\n")
                trace.flush()
                if first_tick is None:
                    first_tick = tick_after
                if last_tick is not None and tick_after < last_tick:
                    raise ValueError("sim_tick_rewound")
                last_tick = tick_after
                previous = amounts
                status["samples"] += 1
                if first_tick is not None and tick_after - first_tick >= args.ticks:
                    status["stopReason"] = "tick_target_reached"
                    break
                time.sleep(min(args.interval_ms / 1000.0,
                               max(0.0, deadline - time.monotonic())))
            else:
                status["stopReason"] = "wall_time_limit"
    except KeyboardInterrupt:
        status["stopReason"] = "keyboard_interrupt"
    except Exception as exc:
        status["errors"].append(type(exc).__name__ + ":" + str(exc))
        status["stopReason"] = "validation_error"
    finally:
        if reader is not None:
            reader.close()
        status.update({"firstObservedTick": first_tick, "lastObservedTick": last_tick,
                       "finishedUtc": datetime.now(timezone.utc).isoformat()})
        (args.out / "status.json").write_text(json.dumps(status, ensure_ascii=False, indent=2) + "\n",
                                               encoding="utf-8")
        print(json.dumps({"out": str(args.out), "samples": status["samples"],
                          "firstTick": first_tick, "lastTick": last_tick,
                          "stopReason": status.get("stopReason"),
                          "errors": status["errors"]}, ensure_ascii=False), flush=True)
    return int(bool(status["errors"]))


if __name__ == "__main__":
    raise SystemExit(main())
