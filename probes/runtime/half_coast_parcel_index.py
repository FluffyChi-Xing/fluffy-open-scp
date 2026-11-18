"""Read the retail Half Coast parcel index without modifying the game."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct

from half_coast_control_raster import ENVIRONMENT, EXE_HASHES, Reader


DATAVIEW_R2 = "c1ff066b5fb64bb288f4c0b1a29a17f9ef163a72ce10a725b1aa2c2089d0018b"
CHUNK_SIDE = 7
CHUNK_BYTES = 0x24
RECORD_BYTES = 0x28
MAX_SLOTS = 100_000


def identity(reader, base):
    wrapper = reader.u32(base + 0xBE0444)
    terrain = reader.u32(wrapper + 0x18)
    zoning = reader.u32(base + 0xBE0450)
    if not zoning or reader.u32(terrain + 0x27A20) != ENVIRONMENT:
        raise ValueError("Half Coast zoning game is not active")
    return terrain, zoning


def snapshot(reader, base):
    initial = identity(reader, base)
    # 0x6F124B takes &cZoningGame::mZoningGrid for the Init call.
    grid = initial[1] + 0xF8
    header = reader.read(grid, 0x71D)
    min_x, min_y, max_x, max_y, chunk_width = struct.unpack_from("<5f", header, 0x708)
    bounds = [min_x, min_y, max_x, max_y, chunk_width]
    if header[0x71C] != 1 or not all(map(math.isfinite, bounds)):
        raise ValueError("Invalid parcel grid bounds")
    if not (min_x < max_x and min_y < max_y and chunk_width == 256):
        raise ValueError("Unexpected retail parcel index sizing")
    records = []
    for index in range(CHUNK_SIDE**2):
        address = index * CHUNK_BYTES
        begin, end = struct.unpack_from("<II", header, address + 0x14)
        size = end - begin
        if size < 0 or size % RECORD_BYTES or size // RECORD_BYTES > MAX_SLOTS:
            raise ValueError(f"Invalid parcel chunk {index}")
        live = 0
        if size:
            if begin < 0x10000:
                raise ValueError(f"Missing parcel chunk {index}")
            data = reader.read(begin, size)
            live = sum(struct.unpack_from("<i", data, offset)[0] >= 0
                       for offset in range(0, size, RECORD_BYTES))
        records.append({"x": index % CHUNK_SIDE, "y": index // CHUNK_SIDE,
                        "slots": size // RECORD_BYTES, "occupied": live})
    if reader.read(grid, len(header)) != header or identity(reader, base) != initial:
        raise ValueError("Parcel index changed during capture")
    return {
        "playArea": [min_x, min_y, max_x, max_y],
        "indexedOriginRange": [min_x, min_y, min_x + CHUNK_SIDE * chunk_width,
                               min_y + CHUNK_SIDE * chunk_width],
        "chunkWidth": chunk_width,
        "chunks": records,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid", required=True, type=int)
    args = parser.parse_args()
    reader = Reader(args.pid)
    try:
        module = next(item for item in reader.modules
                      if item["name"].lower().startswith("simcity-halfcoast-8192"))
        fingerprint = hashlib.sha256(Path(module["path"]).read_bytes()).hexdigest()
        if fingerprint not in EXE_HASHES | {DATAVIEW_R2}:
            raise ValueError("Unsupported Half Coast executable")
        result = snapshot(reader, module["base"])
        print(json.dumps({"pid": args.pid, "sha256": fingerprint, **result}, indent=2))
    finally:
        reader.close()


if __name__ == "__main__":
    main()
