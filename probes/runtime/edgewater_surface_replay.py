"""Replay an EdgeWaterBay saved road against the captured CPU terrain and shader UVs."""

import argparse
import json
from math import floor
from pathlib import Path
from struct import unpack


FACE = 512
CITY_WIDTH = 4096
STOCK_WIDTH = 2048
SEA_RAW = (-870 + 1024) * 32


def read_heights(path):
    raw = path.read_bytes()
    if path.suffix == ".f0":
        dimensions = unpack(">II", raw[4:12])
        if dimensions != (FACE, FACE):
            raise ValueError(f"Unexpected F0 dimensions in {path}: {dimensions}")
        raw = raw[20:]
    return unpack(f"<{FACE * FACE}H", raw)


def height_at(samples, x, y):
    # Nominal texel-center replay. The captured CPU raster is not a GPU texture dump.
    px, py = (
        min(FACE - 1, max(0, (axis / CITY_WIDTH + 0.5) * FACE - 0.5))
        for axis in (x, y)
    )
    left, top = floor(px), floor(py)
    right, bottom = min(left + 1, FACE - 1), min(top + 1, FACE - 1)
    sx, sy = px - left, py - top
    first = samples[top * FACE + left] * (1 - sx) + samples[top * FACE + right] * sx
    second = samples[bottom * FACE + left] * (1 - sx) + samples[bottom * FACE + right] * sx
    return (first * (1 - sy) + second * sy) / 32 - 1024


def compare_heights(reference, samples):
    errors = sorted(abs(a - b) for a, b in zip(reference, samples, strict=True))
    return {
        "p99_error_m": errors[floor(0.99 * (len(errors) - 1))] / 32,
        "over_50m": sum(error > 50 * 32 for error in errors),
        "reference_land_now_below_sea": sum(
            a >= SEA_RAW > b for a, b in zip(reference, samples, strict=True)
        ),
    }


def replay_road(city, path_id, reference, current):
    if city["environment"] != 0x81CED36F:
        raise ValueError("Save is not the EdgeWaterBay 1031 experiment")
    segments = [curve["controls"] for curve in city["curves"] if curve["path"] == path_id]
    if not segments:
        raise ValueError(f"Path {path_id} does not appear in the snapshot")
    endpoints = dict.fromkeys(tuple(p) for segment in segments for p in (segment[0], segment[-1]))
    for x, y, z in endpoints:
        legacy_uv = [axis / STOCK_WIDTH + 0.5 for axis in (x, y)]
        yield {
            "saved_world_xyz": [x, y, z],
            "legacy_read_uv": legacy_uv,
            "legacy_read_in_range": all(0 <= axis <= 1 for axis in legacy_uv),
            "expanded_read_uv": [axis / CITY_WIDTH + 0.5 for axis in (x, y)],
            "legacy_display_of_expanded_mark_xy": [axis * STOCK_WIDTH / CITY_WIDTH for axis in (x, y)],
            "reference_height_at_saved_xy_m": round(height_at(reference, x, y), 3),
            "captured_height_at_saved_xy_m": round(height_at(current, x, y), 3),
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path)
    parser.add_argument("previous", type=Path)
    parser.add_argument("current", type=Path)
    parser.add_argument("save_json", type=Path)
    parser.add_argument("--path-id", type=int, required=True)
    args = parser.parse_args()
    reference, previous, current = map(read_heights, (args.reference, args.previous, args.current))
    city = json.loads(args.save_json.read_bytes())
    print(json.dumps({
        "previous": compare_heights(reference, previous),
        "current": compare_heights(reference, current),
        "changed_between_captures": sum(a != b for a, b in zip(previous, current, strict=True)),
        "road": list(replay_road(city, args.path_id, reference, current)),
    }, indent=2))


if __name__ == "__main__":
    main()
