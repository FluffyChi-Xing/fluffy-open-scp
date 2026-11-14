"""Read-only retail asset snapshots with per-instance identity and game time."""

import argparse
import ctypes as ctypes
import hashlib
import json
import math
from pathlib import Path
import struct
import time

from school_resource_watch import Reader, EXE_HASH, vector
from eawebkit_runtime import MemoryRegion

# Retail PeriodTime RVA 0x1e40b0, UnitFromRef RVA 0x1e3c80.
GAME_VTABLE = 0x91B2F0
GRAPHICS_GAME_VTABLE = 0x91F8A4
INTERIORS_VTABLE = 0x925070
DAY_PERIOD = 0x12952639
INTERIOR_LIGHTS = 0x100000000
VEHICLE_RESOURCES = {
    0x0AB4DFE1: ("vehicles", "carW1"),
    0x0AB4DFE2: ("vehicles", "carW2"),
    0x0AB4DFE3: ("vehicles", "carW3"),
}
RESOURCES = {
    533812918: ("workers", "totalWorkers"),
    1825323535: ("workers", "workerW1"),
    1825323532: ("workers", "workerW2"),
    1825323533: ("workers", "workerW3"),
    444274501: ("tourists", "touristW1"),
    444274502: ("tourists", "touristW2"),
    444274503: ("tourists", "touristW3"),
    1400428281: ("lighting", "workersNeeded"),
    70888836: ("lighting", "totalResidents"),
    INTERIOR_LIGHTS: ("lighting", "interiorLights"),
    **VEHICLE_RESOURCES,
}
CITY_RESOURCES = {
    2391494160: ("utilities", "power"),
    1970301135: ("utilities", "powerToTransmit"),
    3989257529: ("utilities", "powerToSend"),
    0xBA111A2A: ("utilities", "outputBin"),
    3469052172: ("utilities", "coalStock"),
    3212172292: ("utilities", "oilStock"),
    1541996840: ("utilities", "coalBurned"),
    2688949744: ("utilities", "oilBurned"),
    2654748154: ("utilities", "water"),
    1087599423: ("utilities", "waterInMap"),
    0xCD1DDC35: ("utilities", "sewage"),
    0x7CB23A4E: ("utilities", "garbage"),
    688112595: ("utilities", "waitingRoom"),
    2801080940: ("utilities", "openCells"),
    1253104808: ("vehicles", "policeCarStock"),
    116259821: ("utilities", "studentCount"),
}


def game_scripts(reader, game, base):
    if reader.u32(game) != base + GAME_VTABLE:
        raise ValueError("asset_city_changed")
    scripts = reader.u32(game + 0x10)
    if reader.u32(reader.u32(scripts) + 0xD0) != base + 0x1F76B0:
        raise ValueError("asset_city_changed")
    return scripts


def day_clock(reader, game):
    ids, end, ids_header = vector(reader, game + 0x18A4, 4, 16)
    times, finish, times_header = vector(reader, game + 0x18D4, 4, 16)
    periods = [value for (value,) in struct.iter_unpack("<I", reader.read(ids, end - ids))]
    if periods.count(DAY_PERIOD) != 1 or end - ids != finish - times:
        raise ValueError("asset_clock_unavailable")
    fraction, = struct.unpack("<f", reader.read(times + periods.index(DAY_PERIOD) * 4, 4))
    if not math.isfinite(fraction):
        raise ValueError("asset_clock_unavailable")
    return (fraction % 1) * 86400, ids_header, times_header


def discover_game(reader, base, stop):
    deadline = time.monotonic() + 15
    pattern = struct.pack("<I", base + GAME_VTABLE)
    graphics_pattern = struct.pack("<I", base + GRAPHICS_GAME_VTABLE)
    address = 0
    candidates = {}
    graphics = set()
    while address < 0x100000000:
        if stop.exists():
            return None
        if time.monotonic() >= deadline:
            raise ValueError("asset_scan_timeout")
        region = MemoryRegion()
        if not reader.k.VirtualQueryEx(reader.handle, address, ctypes.byref(region), ctypes.sizeof(region)):
            break
        begin = region.BaseAddress or 0
        if begin + region.RegionSize <= address:
            break
        address = begin + region.RegionSize
        if region.State != 0x1000 or region.Type != 0x20000 or region.Protect & 0x101 or not region.Protect & 0xEE:
            continue
        for start in range(max(0x10000, begin), min(address, 0x100000000), 1048576):
            if stop.exists():
                return None
            if time.monotonic() >= deadline:
                raise ValueError("asset_scan_timeout")
            try:
                data = reader.read(start, min(address - start, 1048576, 0x100000000 - start))
            except OSError:
                continue
            graphics.update(start + match for match in find_aligned(data, graphics_pattern, start))
            for offset in find_aligned(data, pattern, start):
                game = start + offset
                try:
                    scripts = game_scripts(reader, game, base)
                    day_clock(reader, game)
                    vector(reader, game + 0x554, 0x380, 100000)
                    candidates[game] = scripts
                except (OSError, ValueError):
                    pass
    return select_city(reader, base, candidates, graphics)


def select_city(reader, base, candidates, graphics):
    # A PID can hold multiple cEcoGames. The retail cGraphicsGame's mGame
    # pointer (+0xC) identifies the city it renders.
    owners = {}
    for graphics_game in graphics:
        try:
            if reader.u32(graphics_game) != base + GRAPHICS_GAME_VTABLE:
                continue
            game = reader.u32(graphics_game + 0xC)
            if game not in candidates or game_scripts(reader, game, base) != candidates[game]:
                continue
            begin, end, _ = vector(reader, graphics_game + 0x10C, 0x128, 100000)
            if begin != end and reader.u32(graphics_game + 0xC) == game:
                owners.setdefault(game, []).append(graphics_game)
        except (ValueError, OSError):
            continue
    if len(owners) == 1:
        game, renderers = next(iter(owners.items()))
        return game, candidates[game], renderers[0] if len(renderers) == 1 else None
    if len(candidates) > 1:
        raise ValueError("asset_city_ambiguous")
    if not candidates:
        raise ValueError("asset_city_missing")
    game, scripts = next(iter(candidates.items()))
    return game, scripts, None


def find_aligned(data, pattern, start):
    offset = data.find(pattern)
    while offset >= 0:
        if (start + offset) % 4 == 0:
            yield offset
        offset = data.find(pattern, offset + 1)


def read_interiors(reader, graphics, game, base, slot):
    # Retail UpdateUnits (0x280587–0x280628) follows the active city's model
    # vector by unit slot. Update (0x41cdf0) writes target/actual at +10/+14.
    if reader.u32(graphics) != base + GRAPHICS_GAME_VTABLE or reader.u32(graphics + 0xC) != game:
        return None
    start, end, header = vector(reader, graphics + 0x10C, 0x128, 100000)
    model = start + slot * 0x128
    if model >= end:
        return None
    controller = reader.u32(model + 0x84)
    if not controller or reader.u32(controller) != base + INTERIORS_VTABLE:
        return None
    raw = reader.read(controller, 0x1E)
    target, actual, intact = struct.unpack_from("<3f", raw, 0x10)
    if not all(math.isfinite(value) and 0 <= value <= 1 for value in (target, actual, intact)):
        return None
    checks = [(graphics + 0x10C, header), (model + 0x84, struct.pack("<I", controller)),
              (graphics + 0xC, struct.pack("<I", game)), (controller, raw)]
    return dict(target=round(target * 100), actual=round(actual * 100), intact=round(intact * 100),
                drivenByActive=bool(raw[0x1D]), allLitWhenActive=bool(raw[0x1C])), checks


def sample(reader, game, scripts, graphics, base, iid, resources=RESOURCES):
    tick = reader.u32(game + 0x17A0)
    if game_scripts(reader, game, base) != scripts:
        raise ValueError("asset_city_changed")
    if graphics and (reader.u32(graphics) != base + GRAPHICS_GAME_VTABLE
                     or reader.u32(graphics + 0xC) != game):
        raise ValueError("asset_city_changed")
    clock, ids_header, times_header = day_clock(reader, game)
    rb, re, resource_header = vector(reader, scripts + 0x90, 8, 20000)
    resource_data = reader.read(rb, re - rb)
    by_index = {index: resource for resource, index in struct.iter_unpack("<Ii", resource_data)}
    ub, ue, unit_header = vector(reader, game + 0x554, 0x380, 100000)
    sb, se, stamp_header = vector(reader, game + 0x584, 4, 100000)
    if (ue - ub) // 0x380 != (se - sb) // 4:
        return None
    values = {}
    matches = set()
    checks = []
    # Read unit descriptors in chunks to avoid a system call for each city slot.
    for start in range(ub, ue, 1024 * 0x380):
        units = reader.read(start, min(ue - start, 1024 * 0x380))
        for offset in range(0, len(units), 0x380):
            if (
                struct.unpack_from("<I", units, offset + 12)[0] != iid
                or units[offset + 0x65]
            ):
                continue
            unit = start + offset
            slot = (unit - ub) // 0x380
            stamp = reader.u32(sb + slot * 4)
            matches.add((slot, stamp))
            bin_header = reader.read(unit + 0x210, 8)
            if bin_header == bytes(8):
                bins = b""
            else:
                bb, be, bin_header = vector(reader, unit + 0x210, 12, 65535)
                bins = reader.read(bb, be - bb)
            for index, _, amount, capacity in struct.iter_unpack("<HHii", bins):
                resource = by_index.get(index)
                if resource in resources:
                    values[(slot, stamp, resource)] = {"amount": amount, "capacity": capacity}
            if graphics:
                try:
                    interiors = read_interiors(reader, graphics, game, base, slot)
                    if interiors:
                        values[(slot, stamp, INTERIOR_LIGHTS)], bindings = interiors
                        try:
                            if any(reader.read(at, len(expected)) != expected for at, expected in bindings):
                                del values[(slot, stamp, INTERIOR_LIGHTS)]
                        except OSError:
                            del values[(slot, stamp, INTERIOR_LIGHTS)]
                except (OSError, ValueError):
                    pass
            checks.extend(((unit + 0x210, bin_header), (unit + 12, struct.pack("<I", iid)), (unit + 0x65, b"\0"),
                           (sb + slot * 4, struct.pack("<I", stamp))))
    checks.extend(((game + 0x554, unit_header), (game + 0x584, stamp_header),
                   (game + 0x18A4, ids_header), (game + 0x18D4, times_header),
                   (scripts + 0x90, resource_header)))
    if any(reader.read(at, len(header)) != header for at, header in checks):
        return None
    if game_scripts(reader, game, base) != scripts or reader.u32(game + 0x17A0) != tick:
        return None
    if graphics and reader.u32(graphics + 0xC) != game:
        raise ValueError("asset_city_changed")
    return tick, clock, matches, values


def changes(previous, current, tick, clock, iid, resources=RESOURCES):
    for key in sorted(previous.keys() | current.keys()):
        before, after = previous.get(key), current.get(key)
        if before == after:
            continue
        slot, stamp, resource = key
        metric, name = resources[resource]
        source = "graphics-interiors" if resource == INTERIOR_LIGHTS else "unit-local"
        row = dict(channel="asset.resource", semantic="resource_change", source=source,
                   unitEntryId=iid, unitSlot=slot, unitStamp=stamp,
                   resourceId=resource if source == "unit-local" else None,
                   gameClockSeconds=clock, gameClockSource="eco_game.day_period", simTicks=tick,
                   metric=metric, name=name,
                   before=before, after=after)
        if metric == "vehicles" and before is not None and after is not None:
            row["amountDelta"] = after["amount"] - before["amount"]
        yield row


def vehicle_inventory(readings):
    counts = {}
    for (_, _, resource), value in readings.items():
        if resource in VEHICLE_RESOURCES:
            _, name = VEHICLE_RESOURCES[resource]
            counts[name] = counts.get(name, 0) + value["amount"]
    return counts


def capture(reader, game, scripts, graphics, base, iid, seconds, directory, state, save, resources=RESOURCES):
    deadline = time.monotonic() + seconds
    stop = directory.with_suffix(".stop")
    previous, last_tick, sequence = {}, None, 0
    vehicle_added = vehicle_removed = 0
    max_records = 20_000
    with (directory / "trace.jsonl").open("w", encoding="utf-8") as trace:
        while time.monotonic() < deadline and not stop.exists():
            try:
                result = sample(reader, game, scripts, graphics, base, iid, resources)
            except OSError:
                result = None
            if result is None:
                state["skippedSamples"] += 1
            else:
                tick, clock, matches, current = result
                if last_tick is not None and tick < last_tick:
                    raise ValueError("asset_city_changed")
                # Graphics may update independently of the simulation tick.
                # An unstable read must not generate a fake controller removal.
                prior_readings = {
                    key: value for key, value in previous.items()
                    if key[2] == INTERIOR_LIGHTS and key[:2] in matches
                }
                controller_count = sum(resource == INTERIOR_LIGHTS for _, _, resource in current)
                readings = prior_readings | current
                for row in changes(previous, readings, tick, clock, iid, resources):
                    if sequence >= max_records:
                        state["stopReason"] = "asset_record_limit"
                        return
                    sequence += 1
                    trace.write(json.dumps(dict(row, sequence=sequence)) + "\n")
                    delta = row.get("amountDelta", 0)
                    vehicle_added += max(0, delta)
                    vehicle_removed += max(0, -delta)
                trace.flush()
                previous, last_tick = readings, tick
                state.update(
                    ready=True, matched=len(matches), tracked=len(current), gameClockSeconds=clock,
                    controllers=controller_count,
                    vehicles=dict(counts=vehicle_inventory(current), added=vehicle_added,
                                  removed=vehicle_removed),
                    metricCounts={
                        metric: sum(resources[resource][0] == metric for _, _, resource in current)
                        for metric in {metric for metric, _ in resources.values()}
                    },
                    samples=state["samples"] + 1,
                )
            save()
            time.sleep(0.5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--seconds", type=int, choices=range(5, 301), required=True)
    parser.add_argument("--unit-entry", type=int, required=True)
    parser.add_argument("--city-metrics", action="store_true")
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    directory = args.directory
    directory.mkdir(exist_ok=False)
    state = dict(ready=False, samples=0, skippedSamples=0, unitEntryId=args.unit_entry)

    def save():
        pending = directory / "status.tmp"
        pending.write_text(json.dumps(state), encoding="utf-8")
        try:
            pending.replace(directory / "status.json")
        except PermissionError:
            # A Windows status reader may hold the destination briefly.
            pass

    stop = directory.with_suffix(".stop")
    reader = None
    try:
        save()
        reader = Reader(args.pid)
        module = next((item for item in reader.modules if item["name"].lower() == "simcity.exe"), None)
        if not module or hashlib.sha256(Path(module["path"]).read_bytes()).hexdigest() != EXE_HASH:
            raise ValueError("unsupported_exe")
        binding = discover_game(reader, module["base"], stop)
        if binding:
            resources = RESOURCES | CITY_RESOURCES if args.city_metrics else RESOURCES
            capture(reader, *binding, module["base"], args.unit_entry, args.seconds, directory, state, save, resources)
    except (OSError, RuntimeError, ValueError) as error:
        state["error"] = str(error)
    finally:
        if reader:
            reader.close()
        save()


if __name__ == "__main__":
    main()
