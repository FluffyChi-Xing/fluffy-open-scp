"""Compare decoded EdgeWaterBay region and city states with the aligned bootstrap."""

import argparse
from collections import Counter
import json
from pathlib import Path


CITY_ENVIRONMENT = 0x81CED36F
REGION_ENVIRONMENT = 0xA0B60DDE
SIDE = 256
STOCK_INDEX = range(64, 192)
STOCK_HALF_WIDTH = 1024
EXPANDED_HALF_WIDTH = 2048
LAYERS = {
    0x6F9F6AB3: "ground",
    0xAB4A0EA0: "surface_water",
    0x0F92DC56: "groundwater",
    0x1D52BE3E: "forest",
    0x28F896A4: "soil",
    0xD779C976: "coal",
    0xD74AF22B: "ore",
    0x76D10FF2: "oil",
}


def is_outer_cell(index):
    return index % SIDE not in STOCK_INDEX or index // SIDE not in STOCK_INDEX


def by_map_id(state):
    return {m["id"]: m["values"] for m in state["maps"] if m["size"] == SIDE}


def compare_maps(bootstrap, current):
    initial = by_map_id(bootstrap)
    after = by_map_id(current)
    differences = {}
    for map_id, name in LAYERS.items():
        changes = [
            index
            for index, (before, now) in enumerate(zip(initial[map_id], after[map_id], strict=True))
            if before != now
        ]
        differences[name] = {
            "nonzero_outer_at_bootstrap": sum(
                is_outer_cell(index) and value != 0
                for index, value in enumerate(initial[map_id])
            ),
            "nonzero_outer_at_save": sum(
                is_outer_cell(index) and value != 0
                for index, value in enumerate(after[map_id])
            ),
            "changed_outer": sum(is_outer_cell(index) for index in changes),
            "changed_inside_stock_box": sum(not is_outer_cell(index) for index in changes),
        }
    return differences


def delta_records(initial, current):
    earlier = Counter(json.dumps(row, sort_keys=True) for row in initial)
    added = Counter(json.dumps(row, sort_keys=True) for row in current) - earlier
    return [json.loads(row) for row in added.elements()]


def inside(point, half_width):
    return max(abs(point[0]), abs(point[1])) <= half_width


def compare(bootstrap, region, city):
    for state, environment, size in (
        (bootstrap, CITY_ENVIRONMENT, 256),
        (region, REGION_ENVIRONMENT, 128),
        (city, CITY_ENVIRONMENT, 256),
    ):
        if state["environment"] != environment or {m["size"] for m in state["maps"]} != {size}:
            raise ValueError(f"Wrong environment or map sizes for {environment:08X}")
    roads = []
    for curve in delta_records(bootstrap["curves"], city["curves"]):
        first, last = curve["controls"][0], curve["controls"][-1]
        if all(inside(point, EXPANDED_HALF_WIDTH) for point in (first, last)) and not all(
            inside(point, STOCK_HALF_WIDTH) for point in (first, last)
        ):
            roads.append({"path": curve["path"], "entryId": curve["entryId"], "first": first, "last": last})
    units = [
        {"id": unit["id"], "position": unit["position"]}
        for unit in delta_records(bootstrap["units"], city["units"])
        if inside(unit["position"], EXPANDED_HALF_WIDTH) and not inside(unit["position"], STOCK_HALF_WIDTH)
    ]
    return {
        "region_version": region["version"],
        "city_version": city["version"],
        "region_maps": len(region["maps"]),
        "city_maps": len(city["maps"]),
        "maps": compare_maps(bootstrap, city),
        "new_or_changed_outer_curves": roads,
        "new_or_changed_outer_units": units,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bootstrap", type=Path)
    parser.add_argument("region", type=Path)
    parser.add_argument("city", type=Path)
    args = parser.parse_args()
    documents = [json.loads(path.read_bytes()) for path in (args.bootstrap, args.region, args.city)]
    print(json.dumps(compare(*documents), indent=2))


if __name__ == "__main__":
    main()
