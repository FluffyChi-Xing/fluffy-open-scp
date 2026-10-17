#!/usr/bin/env python3
"""Analyze plaintext cStreamXOR capture fragments grouped by stream object."""
from __future__ import annotations

import collections
import argparse
import json
from pathlib import Path

from delta_action import DeltaDecodeError, decode_action_header


def load_rows(root: Path):
    rows = []
    for meta_path in root.glob("*_write.bin.json"):
        try:
            meta = json.loads(meta_path.read_text(encoding="utf-8"))
            seq = int(meta_path.name.split("_", 1)[0])
            data = (root / meta["data"]).read_bytes()
            rows.append((seq, meta["meta"]["self"], meta["meta"]["phase"], data))
        except (KeyError, ValueError, OSError, json.JSONDecodeError):
            continue
    return sorted(rows)


def decode_candidates(data: bytes, *, stream_version: int):
    result = []
    for offset in range(max(0, len(data) - 3)):
        try:
            h = decode_action_header(data, offset, stream_version=stream_version)
        except DeltaDecodeError:
            continue
        if h.action_id < 32:
            result.append({"offset": offset, "id": h.action_id, "data1": h.data1, "data2": h.data2})
    return result


def analyze(root: Path, *, stream_version: int):
    rows = load_rows(root)
    groups = collections.defaultdict(list)
    for seq, obj, phase, data in rows:
        groups[obj].append((seq, phase, data))
    streams = []
    all_candidates = []
    for obj, parts in sorted(groups.items(), key=lambda item: min(x[0] for x in item[1])):
        parts.sort()
        data = b"".join(x[2] for x in parts)
        candidates = decode_candidates(data, stream_version=stream_version)
        all_candidates.extend(candidates)
        streams.append({
            "object": obj,
            "writes": len(parts),
            "bytes": len(data),
            "lengths": dict(collections.Counter(len(x[2]) for x in parts)),
            "phase_start": parts[0][1] if parts else None,
            "phase_end": parts[-1][1] if parts else None,
            "action_candidates": candidates,
        })
    return {
        "interpretation": "byte-pattern candidates only; XOR state snapshots are not a proven action stream",
        "stream_version_assumption": stream_version,
        "write_rows": len(rows),
        "plaintext_bytes": sum(len(x[3]) for x in rows),
        "stream_count": len(streams),
        "action_candidate_count": len(all_candidates),
        "action_id_counts": dict(collections.Counter(x["id"] for x in all_candidates)),
        "parameter_counts": {
            f"{item[0]}:{item[1]}:{item[2]}": count
            for item, count in collections.Counter(
                (x["id"], x["data1"], x["data2"]) for x in all_candidates
            ).items()
        },
        "streams": streams,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--stream-version', type=int, required=True, help='Inner action-stream version, NOT MFS envelope version 4; this remains an assumption for XOR candidates')
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[2] / "tmp/dynamic/live/delta-xor/capture")
    args = parser.parse_args()
    if args.stream_version < 0: parser.error('stream version must be non-negative')
    root = args.root
    report = analyze(root, stream_version=args.stream_version)
    out = root / f"analysis-candidates-v{args.stream_version}.json"
    out.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
    print(json.dumps({k: report[k] for k in ("write_rows", "plaintext_bytes", "stream_count", "action_candidate_count", "action_id_counts")}, indent=2))


if __name__ == "__main__":
    main()
