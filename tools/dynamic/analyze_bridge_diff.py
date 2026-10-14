#!/usr/bin/env python3
"""Compare observed bridge results around exact records in a native trace."""
from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path


def compact(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def analyze(path: Path, anchors: list[int], window_ms: int) -> dict:
    calls: dict[int, tuple[str, int]] = {}
    records = []
    markers = {}
    diagnostics = Counter()
    for line in path.open(encoding="utf-8"):
        row = json.loads(line)
        sequence = row["sequence"]
        if sequence in anchors:
            markers[sequence] = {
                "time": row["time"], "channel": row["channel"],
                "name": row.get("name"), "arguments": row.get("arguments"),
            }
        if row.get("channel") == "invoke" and row.get("name") == "RequestGameData":
            args = row.get("arguments", [])
            arg = args[0] if args else {}
            if arg.get("type") != "string" or arg.get("truncated") or arg.get("status") != "flat":
                diagnostics["unusable_query"] += 1
                continue
            calls[sequence] = (arg["text"], row["time"])
        elif row.get("channel") == "result" and row.get("name") == "RequestGameData":
            call = calls.pop(row.get("callSequence"), None)
            if call is None:
                diagnostics["unmatched_result"] += 1
                continue
            key, time = call
            result = row.get("result", {})
            if result.get("truncated") or result.get("status") in ("depth_limit", "value_budget"):
                diagnostics["limited_result"] += 1
            records.append((time, row["sequence"], key, result))
    diagnostics["unmatched_call"] = len(calls)
    windows = []
    for anchor in anchors:
        if anchor not in markers:
            raise ValueError(f"anchor {anchor} not found")
        center = markers[anchor]["time"]
        before, after = {}, {}
        for time, sequence, key, value in records:
            item = {"sequence": sequence, "time": time, "value": value}
            if center - window_ms <= time < center:
                before[key] = item
            elif center < time <= center + window_ms and key not in after:
                after[key] = item
        changed = []
        for key in sorted(before.keys() & after.keys()):
            if compact(before[key]["value"]) != compact(after[key]["value"]):
                changed.append({"query": key, "before": before[key], "after": after[key]})
        windows.append({
            "anchorSequence": anchor, "anchor": markers[anchor], "windowMs": window_ms,
            "beforeQueries": len(before), "afterQueries": len(after),
            "changed": changed, "onlyBefore": sorted(before.keys() - after.keys()),
            "onlyAfter": sorted(after.keys() - before.keys()),
        })
    return {"source": str(path), "diagnostics": dict(diagnostics), "windows": windows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", type=Path)
    parser.add_argument("--anchors", type=int, nargs="+", required=True)
    parser.add_argument("--window-ms", type=int, default=2000)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = analyze(args.trace, args.anchors, args.window_ms)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    for window in result["windows"]:
        print(f"{window['anchorSequence']}: {len(window['changed'])} changed, "
              f"{len(window['onlyBefore'])} only before, {len(window['onlyAfter'])} only after")
    print("diagnostics:", result["diagnostics"])


if __name__ == "__main__":
    main()
