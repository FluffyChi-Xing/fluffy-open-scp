"""Log changes in the rendered city's EcoMap grids using ReadProcessMemory."""

import argparse
import hashlib
import json
from pathlib import Path
import struct
import time
from typing import NamedTuple

from asset_watch import GRAPHICS_GAME_VTABLE, day_clock, discover_game, game_scripts
from school_resource_watch import EXE_HASH, Reader, vector


LAYERS = {
    0xD779C976: "coal",
    0xD74AF22B: "ore",
    0x76D10FF2: "oil",
    0x0F92DC56: "cityWaterTable",
    0x5B9DB0F3: "wind",
    0x1D716F2E: "radiation",
    0xF69DF1C6: "groundPollution",
}
LAND_VALUE_LAYERS = {
    0xCC5539CC: "residentialLandValueW2",
    0xCC5539CD: "residentialLandValueW3",
}

# Retail EXE (EXE_HASH), not the development PDB layout. GetMapsFromIDs
# 0x5D01F0 uses IndexFromMapID 0x5F7750 and the map vector at game+0x544.
# Map initialization at 0x616AC0 reads cellsX/Y at +8/+C, data vector at +0x10.
MAPS_OFFSET = 0x544
MAP_STRIDE = 0x1E0
MAP_INDEX_OFFSET = 0x154
TICK_OFFSET = 0x17A0
MAX_AXIS = 512


class Grid(NamedTuple):
    width: int
    height: int
    data: bytes


def assert_city(reader, base, game, scripts, graphics, environment):
    if (game_scripts(reader, game, base) != scripts
            or reader.u32(graphics) != base + GRAPHICS_GAME_VTABLE
            or reader.u32(graphics + 0xC) != game
            or reader.u32(game + 0x3D4) != environment):
        raise ValueError("asset_city_changed")


def sample(reader, base, game, scripts, graphics, environment, layers=LAYERS):
    assert_city(reader, base, game, scripts, graphics, environment)
    tick = reader.u32(game + TICK_OFFSET)
    clock, _, _ = day_clock(reader, game)
    begin, end, maps_header = vector(reader, game + MAPS_OFFSET, MAP_STRIDE, 512)
    lookup, lookup_end, lookup_header = vector(reader, scripts + MAP_INDEX_OFFSET, 8, 512)
    lookup_data = reader.read(lookup, lookup_end - lookup)
    maps = {}
    descriptors = []
    for map_id, index in struct.iter_unpack("<Ii", lookup_data):
        if map_id not in layers:
            continue
        if map_id in maps or not 0 <= index < (end - begin) // MAP_STRIDE:
            raise ValueError("map_capture_layout")
        address = begin + index * MAP_STRIDE
        header = reader.read(address, 0x18)
        slot, _, width, height, start, stop = struct.unpack("<6I", header)
        if (slot != index or not 1 <= width <= MAX_AXIS or not 1 <= height <= MAX_AXIS
                or start < 0x10000 or stop - start != width * height * 4):
            raise ValueError("map_capture_layout")
        grid = Grid(width, height, reader.read(start, stop - start))
        maps[map_id] = grid
        descriptors.append((address, header, start, grid.data))
    if not maps:
        raise ValueError("map_capture_layout")

    # Require the arrays and descriptors to agree on an immediate second pass.
    # This is still a sampled comparison, not an atomic simulation event.
    for address, header, start, data in descriptors:
        if reader.read(address, len(header)) != header or reader.read(start, len(data)) != data:
            return None
    if (reader.read(game + MAPS_OFFSET, 8) != maps_header
            or reader.read(scripts + MAP_INDEX_OFFSET, 8) != lookup_header
            or reader.read(lookup, len(lookup_data)) != lookup_data
            or reader.u32(game + TICK_OFFSET) != tick):
        return None
    assert_city(reader, base, game, scripts, graphics, environment)
    return tick, clock, maps


def changes(previous, current, layers=LAYERS):
    for map_id, grid in current.items():
        old = previous.get(map_id)
        if old is None or old[:2] != grid[:2] or old.data == grid.data:
            continue
        increased = decreased = count = 0
        examples = []
        before_cells = struct.iter_unpack("<i", old.data)
        after_cells = struct.iter_unpack("<i", grid.data)
        for index, ((before,), (after,)) in enumerate(zip(before_cells, after_cells)):
            delta = after - before
            if not delta:
                continue
            count += 1
            increased += max(0, delta)
            decreased += max(0, -delta)
            if len(examples) < 8:
                examples.append(dict(x=index % grid.width, y=index // grid.width, delta=delta))
        yield dict(
            channel="map.resource", semantic="resource_change", name=layers[map_id],
            mapId=map_id, width=grid.width, height=grid.height, changedCells=count,
            increased=increased, decreased=decreased, amountDelta=increased - decreased,
            cellExamples=examples,
        )


def capture(reader, base, binding, seconds, directory, state, save, layers=LAYERS):
    game, scripts, graphics = binding
    if not graphics:
        raise ValueError("map_capture_city_required")
    environment = reader.u32(game + 0x3D4)
    if not environment:
        raise ValueError("map_capture_city_required")
    stop = directory.with_suffix(".stop")
    deadline = time.monotonic() + seconds
    previous, last_tick, sequence = {}, None, 0
    with (directory / "trace.jsonl").open("w", encoding="utf-8") as trace:
        while time.monotonic() < deadline and not stop.exists():
            try:
                result = sample(reader, base, game, scripts, graphics, environment, layers)
            except OSError:
                result = None
            if result is None:
                state["skippedSamples"] += 1
            else:
                tick, clock, maps = result
                if last_tick is not None and (tick < last_tick or maps.keys() != previous.keys()
                                              or any(maps[key][:2] != previous[key][:2] for key in maps)):
                    raise ValueError("asset_city_changed")
                for row in changes(previous, maps, layers):
                    sequence += 1
                    trace.write(json.dumps(dict(
                        row, sequence=sequence, simTicks=tick, gameClockSeconds=clock,
                        environment=f"{environment:08X}",
                    )) + "\n")
                trace.flush()
                previous, last_tick = maps, tick
                state.update(ready=True, samples=state["samples"] + 1, tracked=len(maps),
                             gameClockSeconds=clock, records=sequence)
            save()
            time.sleep(1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--seconds", type=int, choices=range(5, 301), required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--land-value", action="store_true")
    args = parser.parse_args()
    directory = args.directory
    directory.mkdir(exist_ok=False)
    state = dict(ready=False, samples=0, skippedSamples=0, records=0)

    def save():
        pending = directory / "status.tmp"
        pending.write_text(json.dumps(state), encoding="utf-8")
        try:
            pending.replace(directory / "status.json")
        except PermissionError:
            # The application may have the current status file open.
            pass

    reader = None
    try:
        save()
        reader = Reader(args.pid)
        module = next((item for item in reader.modules if item["name"].lower() == "simcity.exe"), None)
        if not module or hashlib.sha256(Path(module["path"]).read_bytes()).hexdigest() != EXE_HASH:
            raise ValueError("unsupported_exe")
        binding = discover_game(reader, module["base"], directory.with_suffix(".stop"))
        if binding:
            capture(reader, module["base"], binding, args.seconds, directory, state, save,
                    LAND_VALUE_LAYERS if args.land_value else LAYERS)
    except OSError:
        state["error"] = "map_capture_read_failed"
    except (RuntimeError, ValueError) as error:
        state["error"] = str(error)
    finally:
        if reader:
            reader.close()
        save()


if __name__ == "__main__":
    main()
