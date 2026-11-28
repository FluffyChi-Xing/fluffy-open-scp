"""Compare archived 1031 path splines and replay zoning coordinates offline."""

import argparse
from collections import Counter
import json
from math import dist, floor
from pathlib import Path
from statistics import median

from edgewater_surface_replay import height_at, read_heights


def controls_key(curve):
    return curve["entryId"], tuple(tuple(point) for point in curve["controls"])


def bezier_samples(controls):
    for i in range(65):
        t = i / 64
        weights = ((1 - t)**3, 3 * t * (1 - t)**2, 3 * t**2 * (1 - t), t**3)
        yield tuple(sum(weight * point[axis] for weight, point in zip(weights, controls))
                    for axis in range(3))


def inside(point, half_width):
    return max(abs(point[0]), abs(point[1])) < half_width


def zoning_point(point, minimum):
    axes = [axis - minimum for axis in point[:2]]
    raw = [floor(axis / 256) for axis in axes]
    query = [max(0, min(chunk, 7)) for chunk in raw]
    cells = [max(0, min(int((axis - chunk * 256) / 32), 8))
             for axis, chunk in zip(axes, query)]
    return {
        "raw_chunk": raw,
        "insertion_chunk_if_parcel_min_is_here": [max(0, min(chunk, 6)) for chunk in raw],
        "point_query_base_chunk": query,
        "query_subcell": cells,
        "nominal_subcell": [floor((axis % 256) / 32) for axis in axes],
    }


def poly_rectangle_left_edge(point, minimum, radius=12):
    # Clockwise axis-aligned probe: the upward left edge keeps columns to its right.
    # PolyQuery clips the bounding-box chunk range, then rasterizes in each query chunk.
    left, right = point[0] - radius, point[0] + radius
    first, last = [max(0, min(floor((x - minimum) / 256), 7)) for x in (left, right)]
    columns = {}
    for chunk in range(first, last + 1):
        local_left = left - (minimum + chunk * 256)
        columns[chunk] = max(0, 8 - max(0, int(local_left / 32)))
    return {"probe_x_bounds_m": [left, right], "columns_after_left_edge": columns}


def summary(values):
    if not values:
        return None
    values = sorted(values)
    return {
        "median_abs_m": round(values[(len(values) - 1) // 2], 3),
        "p95_abs_m": round(values[floor(0.95 * (len(values) - 1))], 3),
        "over_5m": sum(value > 5 for value in values),
        "over_20m": sum(value > 20 for value in values),
    }


def compare(save, baseline, heights):
    curves = {controls_key(curve): list(bezier_samples(curve["controls"]))
              for curve in save["curves"]}
    count = Counter(controls_key(curve) for curve in save["curves"])
    samples = []
    paths_outside = 0
    entry_counts = Counter()
    for key, multiplicity in (count - baseline).items():
        points = curves[key]
        if all(inside(point, 2048) for point in points) and any(
            not inside(point, 1024) for point in points
        ):
            entry_counts[f"{key[0]:08X}"] += multiplicity
            paths_outside += multiplicity
            samples.extend([point for point in points if not inside(point, 1024)] * multiplicity)

    units = []
    for unit in save["units"]:
        if unit["id"] not in (0xFB5AA2CE, 0x990FA812):
            continue
        point = unit["position"]
        nearest = min(
            (dist(point[:2], sample[:2]), f"{key[0]:08X}", sample)
            for key, points in curves.items() for sample in points
        )
        units.append({
            "id": f'{unit["id"]:08X}',
            "xyz": point,
            "nearest_curve_replayed": nearest,
            "stock_or_orion_xy": zoning_point(point, -1024),
            "centered_4096": zoning_point(point, -2048),
            "synthetic_poly_left_edge_stock_or_orion": poly_rectangle_left_edge(point, -1024),
            "synthetic_poly_left_edge_centered_4096": poly_rectangle_left_edge(point, -2048),
        })
    ground = next(layer for layer in save["maps"] if layer["id"] == 0x6F9F6AB3)
    outer_cells = [value for i, value in enumerate(ground["values"])
                   if ground["size"] == 256 and
                   (not 64 <= i % 256 < 192 or not 64 <= i // 256 < 192)]
    return {
        "total_segments": sum(count.values()),
        "new_or_changed_segments_with_outer_samples": paths_outside,
        "outer_entry_counts": entry_counts,
        "outer_sample_count": len(samples),
        "outer_sample_saved_z_vs_initial_f0": summary(
            [abs(z - height_at(heights, x, y)) for x, y, z in samples]
        ),
        "ground_size": ground["size"],
        "outer_ground_nonzero": sum(value != 0 for value in outer_cells),
        "groundfix_units": units,
    }


def compare_captures(saves, captures, baseline, heights):
    keys = iter(Counter(controls_key(curve) for curve in saves[name]["curves"]) - baseline
                for name in captures)
    shared = next(keys)
    for case_keys in keys:
        shared &= case_keys
    points = []
    anomalies = {name: [] for name in captures}
    segments = 0
    for (entry, controls), multiplicity in shared.items():
        sampled = list(bezier_samples(controls))
        if all(inside(point, 2048) for point in sampled):
            outer = [point for point in sampled if not inside(point, 1024)]
            if outer:
                points.extend(outer * multiplicity)
                segments += multiplicity
            for x, y, z in outer:
                reference = height_at(heights, x, y)
                if abs(z - reference) > 1:
                    continue
                for name, physics in captures.items():
                    actual = height_at(physics, x, y)
                    if abs(z - actual) > 20:
                        anomalies[name].extend([{
                            "entry_id": f"{entry:08X}",
                            "controls": controls,
                            "xyz": (x, y, z),
                            "f0_m": reference,
                            "capture_m": actual,
                            "f0_at_legacy_uv_clamped_m": height_at(heights, 2 * x, 2 * y),
                            "legacy_uv": (x / 2048 + 0.5, y / 2048 + 0.5),
                            "physics_texel_center_xy": ((x + 2048) / 8 - 0.5, (y + 2048) / 8 - 0.5),
                        }] * multiplicity)
    near_initial = [(x, y, z) for x, y, z in points if abs(z - height_at(heights, x, y)) <= 1]
    anomaly_candidate_errors = {
        name: {
            "median_to_f0_at_same_xy_m": median(abs(row["capture_m"] - row["f0_m"]) for row in rows),
            "median_to_f0_at_clamped_legacy_uv_m": median(
                abs(row["capture_m"] - row["f0_at_legacy_uv_clamped_m"]) for row in rows
            ),
        }
        for name, rows in anomalies.items() if rows
    }
    return {
        "shared_outer_segments": segments,
        "shared_outer_samples": len(points),
        "initial_f0": summary([abs(z - height_at(heights, x, y)) for x, y, z in points]),
        "saved_z_vs_captured_physics": {
            name: summary([abs(z - height_at(physics, x, y)) for x, y, z in points])
            for name, physics in captures.items()
        },
        "within_1m_of_initial_f0": len(near_initial),
        "within_1m_of_f0_but_over_20m_from_capture": {
            name: len(rows) for name, rows in anomalies.items()
        },
        "anomaly_samples": anomalies,
        "anomaly_candidate_errors": anomaly_candidate_errors,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path)
    parser.add_argument("bootstrap", type=Path)
    parser.add_argument("saves", type=Path, nargs="+")
    parser.add_argument("--physics", action="append", default=[], metavar="SAVE_STEM=PATH")
    args = parser.parse_args()
    heights = read_heights(args.reference)
    bootstrap = json.loads(args.bootstrap.read_bytes())
    baseline = Counter(controls_key(curve) for curve in bootstrap["curves"])
    saves = {}
    for path in args.saves:
        save = json.loads(path.read_bytes())
        if save["environment"] != 0x81CED36F:
            raise ValueError(f"Not a 1031 city snapshot: {path}")
        saves[path.stem] = save
    results = {name: compare(save, baseline, heights) for name, save in saves.items()}
    if args.physics:
        captures = {}
        for item in args.physics:
            name, path = item.split("=", 1)
            captures[name] = read_heights(Path(path))
        results["shared_capture_comparison"] = compare_captures(saves, captures, baseline, heights)
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
